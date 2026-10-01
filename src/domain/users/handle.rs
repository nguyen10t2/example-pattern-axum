use axum::{
    Router,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use std::net::SocketAddr;
use std::sync::LazyLock;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    config::constants::{
        DEFAULT_COOKIE_SECURE, DEFAULT_FRONTEND_URL, OAUTH_COOKIE_MAX_AGE_SECS, OAUTH_SUCCESS_QUERY,
        RATE_LIMIT_CHANGE_PASSWORD_IP_MAX, RATE_LIMIT_CHANGE_PASSWORD_USER_MAX, RATE_LIMIT_EMAIL_WINDOW,
        RATE_LIMIT_FORGOT_PASSWORD_OTP_IP_MAX, RATE_LIMIT_REQUEST_OTP_EMAIL_MAX, RATE_LIMIT_REQUEST_OTP_IP_MAX,
        RATE_LIMIT_SIGNIN_IP_MAX, RATE_LIMIT_VERIFY_OTP_IP_MAX, RATE_LIMIT_WINDOW, REFRESH_COOKIE_NAME,
    },
    domain::users::request::{
        ChangePasswordRequest, ForgotPasswordOtpRequest, GoogleCallbackQuery, RequestOtpRequest, ResetPasswordRequest,
        SignInRequest, SignUpRequest, UpdateUserRequest, VerifyOtpRequest,
    },
    domain::users::response::{AuthResponse, UserResponse},
    errors::{AppError, BusinessError},
    middleware::{AuthUser, RequestLang, ValidatedJson, ValidatedPath, ValidatedQuery, extract_client_ip},
    responses::SuccessResponse,
    state::AppState,
    utils::{
        cache::REFRESH_TOKEN_EXPIRATION,
        i18n::t_simple,
        oauth::{generate_code_challenge, generate_code_verifier, generate_state},
    },
};

/// Gắn cờ `Secure` cho cookie khi `COOKIE_SECURE=true|1` (bắt buộc ở production HTTPS).
/// Đọc một lần lúc boot (static) vì env không đổi lúc runtime.
static COOKIE_SECURE: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("COOKIE_SECURE").map_or(DEFAULT_COOKIE_SECURE, |v| v.eq_ignore_ascii_case("true") || v == "1")
});

/// `SameSite` cho refresh cookie, đọc từ `COOKIE_SAMESITE` (`strict|lax|none`,
/// mặc định `strict`). Deploy cross-site thật (FE Cloudflare + BE Render) bắt
/// buộc `none` + `COOKIE_SECURE=true`, ngược lại browser không gửi cookie trên
/// fetch cross-site → F5 logout + rác session Redis.
/// Đọc một lần lúc boot (static) vì env không đổi lúc runtime.
static COOKIE_SAMESITE: LazyLock<SameSite> =
    LazyLock::new(|| parse_cookie_samesite(std::env::var("COOKIE_SAMESITE").ok().as_deref()));

/// Lỗi cấu hình cookie lúc boot — đều fatal: caller log rồi exit (fail-fast),
/// vì cookie sai thì auth gãy hoàn toàn mà không báo lỗi rõ ràng lúc runtime.
#[derive(Debug, thiserror::Error)]
pub enum CookieConfigError {
    /// `SameSite=None` mà thiếu `Secure` thì browser từ chối cookie luôn.
    #[error("COOKIE_SAMESITE=none requires COOKIE_SECURE=true (browsers reject SameSite=None without Secure)")]
    SameSiteNoneWithoutSecure,
}

/// Parse giá trị `COOKIE_SAMESITE`: `lax`/`none` (case-insensitive, trim),
/// còn lại (kể cả unset) về `Strict` — fail-closed.
///
/// Sync + pure (không đọc env) để unit test không chạm env thật.
fn parse_cookie_samesite(raw: Option<&str>) -> SameSite {
    match raw.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("lax") => SameSite::Lax,
        Some("none") => SameSite::None,
        _ => SameSite::Strict,
    }
}

/// Kiểm tra combo `SameSite` + `Secure` có hợp lệ không (pure để dễ test).
///
/// Sync vì chỉ so sánh enum/bool trong RAM, không có I/O.
///
/// # Errors
///
/// Trả [`CookieConfigError::SameSiteNoneWithoutSecure`] khi `none` mà không `Secure`.
fn validate_cookie_combo(samesite: SameSite, secure: bool) -> Result<(), CookieConfigError> {
    if samesite == SameSite::None && !secure {
        return Err(CookieConfigError::SameSiteNoneWithoutSecure);
    }
    Ok(())
}

/// Kiểm tra cấu hình cookie từ env lúc boot (fail-fast, gọi ở `main` trước khi serve).
///
/// Sync vì chỉ đọc 2 static đã parse sẵn lúc boot, không có I/O.
///
/// # Errors
///
/// Trả [`CookieConfigError`] khi combo `COOKIE_SAMESITE`/`COOKIE_SECURE` không hợp lệ.
pub fn validate_cookie_env() -> Result<(), CookieConfigError> {
    validate_cookie_combo(*COOKIE_SAMESITE, *COOKIE_SECURE)
}

/// Dựng cookie refresh chuẩn (`HttpOnly` + `SameSite=Strict` + `Secure` theo env).
///
/// Gom 3 điểm dựng rời rạc (signin/refresh/callback) về một chỗ để không lệch
/// attribute. Sync vì chỉ dựng struct trong RAM, không có I/O.
fn build_refresh_cookie(refresh_token: String) -> Cookie<'static> {
    let mut cookie = Cookie::new(REFRESH_COOKIE_NAME, refresh_token);
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(*COOKIE_SECURE);
    cookie.set_same_site(*COOKIE_SAMESITE);
    let expires = OffsetDateTime::now_utc() + time::Duration::seconds(REFRESH_TOKEN_EXPIRATION.cast_signed());
    cookie.set_expires(expires);
    cookie
}

/// Dựng cookie xóa refresh (cùng name/path/secure để browser match đúng entry).
///
/// Sync vì chỉ dựng struct trong RAM, không có I/O.
fn build_remove_refresh_cookie() -> Cookie<'static> {
    let mut cookie = Cookie::new(REFRESH_COOKIE_NAME, "");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_secure(*COOKIE_SECURE);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_max_age(time::Duration::seconds(0));
    cookie
}

/// Dựng routes user: nhóm public (OTP, login, OAuth) + nhóm protected sau auth.
pub fn user_router(state: AppState) -> Router<AppState> {
    let public_routes = Router::new()
        .route("/request-otp", post(handle_request_otp))
        .route("/verify-otp", post(handle_verify_otp))
        .route("/signup", post(handle_signup))
        .route("/forgot-password/otp", post(handle_forgot_password_otp))
        .route("/forgot-password/reset", post(handle_reset_password))
        .route("/signin", post(handle_signin))
        .route("/refresh", post(handle_refresh))
        .route("/signout", post(handle_signout))
        .route("/auth/google", get(handle_google_auth))
        .route("/auth/google/callback", get(handle_google_callback));

    let protected_routes = Router::new()
        .route("/me", get(handle_get_me).patch(handle_update_me))
        .route("/me/password", post(handle_change_password))
        .route("/id/{id}", get(handle_get_user_by_id))
        .route("/email/{email}", get(handle_get_user_by_email))
        .route_layer(axum::middleware::from_fn_with_state(state, crate::middleware::require_auth));

    public_routes.merge(protected_routes)
}

/// Gửi OTP đăng ký (giới hạn theo IP).
///
/// # Errors
///
/// Trả `TooManyRequests` khi vượt rate-limit, lỗi nghiệp vụ từ service nếu có.
pub async fn handle_request_otp(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    ValidatedJson(body): ValidatedJson<RequestOtpRequest>,
) -> Result<SuccessResponse<()>, AppError> {
    let ip = extract_client_ip(&headers, peer_addr.ip()).to_string();
    if !state.rate_limiter.check_ip_limit("request-otp", &ip, RATE_LIMIT_REQUEST_OTP_IP_MAX, RATE_LIMIT_WINDOW).await? {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }
    if !state
        .rate_limiter
        .check_email_limit("request-otp", &body.email, RATE_LIMIT_REQUEST_OTP_EMAIL_MAX, RATE_LIMIT_EMAIL_WINDOW)
        .await?
    {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }

    state.user_service.request_otp(&body.email, lang).await?;
    Ok(SuccessResponse::message_only(t_simple("OTP_SENT", lang)))
}

/// Kiểm tra OTP mà không tiêu thụ (cho FE verify sớm trước khi nhập tiếp).
///
/// # Errors
///
/// Trả `TooManyRequests` khi vượt rate-limit, `InvalidOtp` khi mã sai/hết hạn/bị hủy.
pub async fn handle_verify_otp(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    ValidatedJson(body): ValidatedJson<VerifyOtpRequest>,
) -> Result<SuccessResponse<bool>, AppError> {
    let ip = extract_client_ip(&headers, peer_addr.ip()).to_string();
    if !state.rate_limiter.check_ip_limit("verify-otp", &ip, RATE_LIMIT_VERIFY_OTP_IP_MAX, RATE_LIMIT_WINDOW).await? {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }

    state.user_service.verify_otp(&body.email, &body.otp, body.purpose).await?;
    Ok(SuccessResponse::with_message(true, t_simple("OTP_VERIFIED", lang)))
}

/// Đăng ký user mới, trả `201 Created`.
///
/// # Errors
///
/// Trả lỗi nghiệp vụ từ service (`InvalidOtp`, email trùng, ...).
pub async fn handle_signup(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    ValidatedJson(body): ValidatedJson<SignUpRequest>,
) -> Result<(StatusCode, SuccessResponse<UserResponse>), AppError> {
    let user = state.user_service.sign_up(body).await?;
    Ok(SuccessResponse::created(user, t_simple("USER_CREATED", lang)))
}

/// Gửi OTP quên mật khẩu (giới hạn theo IP).
///
/// # Errors
///
/// Trả `TooManyRequests` khi vượt rate-limit.
pub async fn handle_forgot_password_otp(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    ValidatedJson(body): ValidatedJson<ForgotPasswordOtpRequest>,
) -> Result<SuccessResponse<()>, AppError> {
    let ip = extract_client_ip(&headers, peer_addr.ip()).to_string();
    if !state
        .rate_limiter
        .check_ip_limit("forgot-password-otp", &ip, RATE_LIMIT_FORGOT_PASSWORD_OTP_IP_MAX, RATE_LIMIT_WINDOW)
        .await?
    {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }
    if !state
        .rate_limiter
        .check_email_limit(
            "forgot-password-otp",
            &body.email,
            RATE_LIMIT_REQUEST_OTP_EMAIL_MAX,
            RATE_LIMIT_EMAIL_WINDOW,
        )
        .await?
    {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }

    state.user_service.request_forgot_password_otp(&body.email, lang).await?;
    Ok(SuccessResponse::message_only(t_simple("OTP_SENT", lang)))
}

/// Reset mật khẩu bằng OTP đã gửi qua email.
///
/// # Errors
///
/// Trả `InvalidOtp` khi OTP sai/hết hạn.
pub async fn handle_reset_password(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    ValidatedJson(body): ValidatedJson<ResetPasswordRequest>,
) -> Result<SuccessResponse<()>, AppError> {
    state.user_service.reset_password(body).await?;
    Ok(SuccessResponse::message_only(t_simple("PASSWORD_RESET", lang)))
}

/// Đăng nhập, set refresh token vào cookie `http_only`.
///
/// # Errors
///
/// Trả `TooManyRequests` khi vượt rate-limit, `InvalidCredentials` khi sai thông tin.
pub async fn handle_signin(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    jar: CookieJar,
    ValidatedJson(body): ValidatedJson<SignInRequest>,
) -> Result<(CookieJar, SuccessResponse<AuthResponse>), AppError> {
    let ip = extract_client_ip(&headers, peer_addr.ip()).to_string();
    if !state.rate_limiter.check_ip_limit("signin", &ip, RATE_LIMIT_SIGNIN_IP_MAX, RATE_LIMIT_WINDOW).await? {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }

    let tokens = state.user_service.sign_in(body).await?;

    let jar = jar.add(build_refresh_cookie(tokens.refresh_token));
    Ok((
        jar,
        SuccessResponse::with_message(AuthResponse { access_token: tokens.access_token }, t_simple("SIGNED_IN", lang)),
    ))
}

/// Xoay cặp token từ refresh cookie, set cookie mới.
///
/// # Errors
///
/// Trả `InvalidSession` khi thiếu cookie hoặc session hết hạn/bị thu hồi.
pub async fn handle_refresh(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    jar: CookieJar,
) -> Result<(CookieJar, SuccessResponse<AuthResponse>), AppError> {
    let refresh_token = jar.get(REFRESH_COOKIE_NAME).map(Cookie::value);
    let tokens = state.user_service.refresh(refresh_token).await?;

    let jar = jar.add(build_refresh_cookie(tokens.refresh_token));
    Ok((
        jar,
        SuccessResponse::with_message(
            AuthResponse { access_token: tokens.access_token },
            t_simple("TOKEN_REFRESHED", lang),
        ),
    ))
}

/// Đăng xuất: thu hồi session và xóa refresh cookie.
///
/// # Errors
///
/// Luôn `Ok` — sign-out là idempotent.
pub async fn handle_signout(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    jar: CookieJar,
) -> Result<(CookieJar, SuccessResponse<()>), AppError> {
    let refresh_token = jar.get(REFRESH_COOKIE_NAME).map(Cookie::value);
    state.user_service.sign_out(refresh_token).await?;

    let jar = jar.add(build_remove_refresh_cookie());
    Ok((jar, SuccessResponse::message_only(t_simple("SIGNED_OUT", lang))))
}

/// Bắt đầu OAuth Google: lưu state/verifier vào cookie rồi redirect sang Google.
///
/// # Errors
///
/// Luôn `Ok` — bước này chưa gọi I/O nào có thể lỗi.
pub async fn handle_google_auth(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, Redirect), AppError> {
    let oauth_state = generate_state();
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);

    let auth_url = state.google_oauth.generate_auth_url(&oauth_state, &code_challenge)?;

    let mut state_cookie = Cookie::new("google_oauth_state", oauth_state);
    state_cookie.set_path("/");
    state_cookie.set_http_only(true);
    state_cookie.set_secure(*COOKIE_SECURE);
    state_cookie.set_max_age(time::Duration::seconds(OAUTH_COOKIE_MAX_AGE_SECS));
    state_cookie.set_same_site(SameSite::Lax);

    let mut verifier_cookie = Cookie::new("google_oauth_code_verifier", code_verifier);
    verifier_cookie.set_path("/");
    verifier_cookie.set_http_only(true);
    verifier_cookie.set_secure(*COOKIE_SECURE);
    verifier_cookie.set_max_age(time::Duration::seconds(OAUTH_COOKIE_MAX_AGE_SECS));
    verifier_cookie.set_same_site(SameSite::Lax);

    let jar = jar.add(state_cookie).add(verifier_cookie);
    Ok((jar, Redirect::temporary(&auth_url)))
}

/// Xử lý callback Google: đối chiếu state, đăng nhập rồi redirect về frontend.
///
/// Access token KHÔNG đi qua URL (tránh lộ qua log/history/referer): backend chỉ
/// set refresh cookie rồi redirect với flag `OAUTH_SUCCESS_QUERY`, frontend tự gọi
/// `POST /refresh` (cookie tự gửi) để lấy access token.
///
/// # Errors
///
/// Trả `Unauthorized` khi thiếu/khác state, code lỗi, hoặc Google từ chối.
pub async fn handle_google_callback(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<GoogleCallbackQuery>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let code = query.code.ok_or(AppError::Business(BusinessError::Unauthorized))?;
    let state_param = query.state.ok_or(AppError::Business(BusinessError::Unauthorized))?;

    let stored_state = jar.get("google_oauth_state").map(|c| c.value().to_string());
    if stored_state.as_deref() != Some(&state_param) {
        return Err(AppError::Business(BusinessError::Unauthorized));
    }

    let code_verifier = jar
        .get("google_oauth_code_verifier")
        .map(Cookie::value)
        .ok_or(AppError::Business(BusinessError::Unauthorized))?;
    let access_token = state.google_oauth.exchange_code(&code, code_verifier).await?;
    let google_user = state.google_oauth.fetch_user_info(&access_token).await?;
    let tokens = state.user_service.sign_in_with_google(google_user).await?;

    let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_else(|_| DEFAULT_FRONTEND_URL.to_string());
    let redirect_url = format!("{frontend_url}/login?{OAUTH_SUCCESS_QUERY}");

    let jar = jar
        .remove(Cookie::from("google_oauth_state"))
        .remove(Cookie::from("google_oauth_code_verifier"))
        .add(build_refresh_cookie(tokens.refresh_token));

    Ok((jar, Redirect::temporary(&redirect_url)).into_response())
}

/// Lấy profile của chính mình (từ JWT).
///
/// # Errors
///
/// Trả `UserNotFound` khi user không còn tồn tại.
pub async fn handle_get_me(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    AuthUser(user_id): AuthUser,
) -> Result<SuccessResponse<UserResponse>, AppError> {
    let user = state.user_service.find_by_id(user_id).await?;
    Ok(SuccessResponse::with_message(user, t_simple("USER_FOUND", lang)))
}

/// Cập nhật profile của chính mình.
///
/// # Errors
///
/// Trả lỗi DB khi ghi thất bại.
pub async fn handle_update_me(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    AuthUser(user_id): AuthUser,
    ValidatedJson(body): ValidatedJson<UpdateUserRequest>,
) -> Result<SuccessResponse<UserResponse>, AppError> {
    let user = state.user_service.update(user_id, body).await?;
    Ok(SuccessResponse::with_message(user, t_simple("USER_UPDATED", lang)))
}

/// Đổi mật khẩu (giới hạn theo user và IP).
///
/// # Errors
///
/// Trả `TooManyRequests` khi vượt rate-limit, `InvalidCredentials` khi sai mật khẩu cũ.
pub async fn handle_change_password(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    AuthUser(user_id): AuthUser,
    headers: HeaderMap,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    ValidatedJson(body): ValidatedJson<ChangePasswordRequest>,
) -> Result<SuccessResponse<()>, AppError> {
    let ip = extract_client_ip(&headers, peer_addr.ip()).to_string();
    if !state
        .rate_limiter
        .check_mixed_limit(
            "change-password",
            user_id,
            &ip,
            RATE_LIMIT_CHANGE_PASSWORD_USER_MAX,
            RATE_LIMIT_CHANGE_PASSWORD_IP_MAX,
            RATE_LIMIT_WINDOW,
        )
        .await?
    {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }

    state.user_service.change_password(user_id, body).await?;
    Ok(SuccessResponse::message_only(t_simple("PASSWORD_CHANGED", lang)))
}

/// Lấy user theo id.
///
/// # Errors
///
/// Trả `UserNotFound` khi id không tồn tại.
pub async fn handle_get_user_by_id(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    AuthUser(_): AuthUser,
    ValidatedPath(id): ValidatedPath<Uuid>,
) -> Result<SuccessResponse<UserResponse>, AppError> {
    let user = state.user_service.find_by_id(id).await?;
    Ok(SuccessResponse::with_message(user, t_simple("USER_FOUND", lang)))
}

/// Lấy user theo email.
///
/// # Errors
///
/// Trả `UserNotFound` khi email không tồn tại.
pub async fn handle_get_user_by_email(
    State(state): State<AppState>,
    RequestLang(lang): RequestLang,
    AuthUser(_): AuthUser,
    ValidatedPath(email): ValidatedPath<String>,
) -> Result<SuccessResponse<UserResponse>, AppError> {
    let user = state.user_service.find_by_email(&email).await?;
    Ok(SuccessResponse::with_message(user, t_simple("USER_FOUND", lang)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cookie_samesite_defaults_strict() {
        assert_eq!(parse_cookie_samesite(None), SameSite::Strict);
        assert_eq!(parse_cookie_samesite(Some("")), SameSite::Strict);
        assert_eq!(parse_cookie_samesite(Some("bogus")), SameSite::Strict);
    }

    #[test]
    fn test_parse_cookie_samesite_lax_and_none_case_insensitive() {
        for raw in ["lax", "LAX", " Lax "] {
            assert_eq!(parse_cookie_samesite(Some(raw)), SameSite::Lax);
        }
        for raw in ["none", "NONE", " None "] {
            assert_eq!(parse_cookie_samesite(Some(raw)), SameSite::None);
        }
    }

    #[test]
    fn test_validate_cookie_combo_rejects_none_without_secure() {
        assert!(validate_cookie_combo(SameSite::None, false).is_err());
        assert!(matches!(
            validate_cookie_combo(SameSite::None, false).unwrap_err(),
            CookieConfigError::SameSiteNoneWithoutSecure
        ));
    }

    #[test]
    fn test_validate_cookie_combo_accepts_valid_combos() {
        assert!(validate_cookie_combo(SameSite::None, true).is_ok());
        assert!(validate_cookie_combo(SameSite::Strict, false).is_ok());
        assert!(validate_cookie_combo(SameSite::Strict, true).is_ok());
        assert!(validate_cookie_combo(SameSite::Lax, false).is_ok());
    }
}
