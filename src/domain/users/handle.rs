use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    config::constants::{
        DEFAULT_FRONTEND_URL, OAUTH_COOKIE_MAX_AGE_SECS, RATE_LIMIT_CHANGE_PASSWORD_IP_MAX,
        RATE_LIMIT_CHANGE_PASSWORD_USER_MAX, RATE_LIMIT_FORGOT_PASSWORD_OTP_IP_MAX, RATE_LIMIT_REQUEST_OTP_IP_MAX,
        RATE_LIMIT_SIGNIN_IP_MAX, RATE_LIMIT_WINDOW,
    },
    domain::users::request::{
        ChangePasswordRequest, ForgotPasswordOtpRequest, GoogleCallbackQuery, RequestOtpRequest, ResetPasswordRequest,
        SignInRequest, SignUpRequest, UpdateUserRequest,
    },
    domain::users::response::{AuthResponse, UserResponse},
    errors::{AppError, BusinessError},
    middleware::{AuthUser, RequestLang, ValidatedJson, ValidatedPath, ValidatedQuery, extract_client_ip},
    responses::SuccessResponse,
    state::AppState,
    utils::{
        cache::REFRESH_TOKEN_EXPIRATION,
        i18n::t_simple,
        oauth::{generate_code_verifier, generate_state},
    },
};

/// Dựng routes user: nhóm public (OTP, login, OAuth) + nhóm protected sau auth.
pub fn user_router(state: AppState) -> Router<AppState> {
    let public_routes = Router::new()
        .route("/request-otp", post(handle_request_otp))
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
    ValidatedJson(body): ValidatedJson<RequestOtpRequest>,
) -> Result<SuccessResponse<()>, AppError> {
    let ip = extract_client_ip(&headers);
    if !state.rate_limiter.check_ip_limit("request-otp", &ip, RATE_LIMIT_REQUEST_OTP_IP_MAX, RATE_LIMIT_WINDOW).await {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }

    state.user_service.request_otp(&body.email, lang).await?;
    Ok(SuccessResponse::message_only(t_simple("OTP_SENT", lang)))
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
    ValidatedJson(body): ValidatedJson<ForgotPasswordOtpRequest>,
) -> Result<SuccessResponse<()>, AppError> {
    let ip = extract_client_ip(&headers);
    if !state
        .rate_limiter
        .check_ip_limit("forgot-password-otp", &ip, RATE_LIMIT_FORGOT_PASSWORD_OTP_IP_MAX, RATE_LIMIT_WINDOW)
        .await
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
    jar: CookieJar,
    ValidatedJson(body): ValidatedJson<SignInRequest>,
) -> Result<(CookieJar, SuccessResponse<AuthResponse>), AppError> {
    let ip = extract_client_ip(&headers);
    if !state.rate_limiter.check_ip_limit("signin", &ip, RATE_LIMIT_SIGNIN_IP_MAX, RATE_LIMIT_WINDOW).await {
        return Err(AppError::Business(BusinessError::TooManyRequests));
    }

    let tokens = state.user_service.sign_in(body).await?;

    let mut refresh_cookie = Cookie::new("refreshCookie", tokens.refresh_token);
    refresh_cookie.set_path("/");
    refresh_cookie.set_http_only(true);
    refresh_cookie.set_same_site(SameSite::Strict);
    let expires = OffsetDateTime::now_utc() + time::Duration::seconds(REFRESH_TOKEN_EXPIRATION.cast_signed());
    refresh_cookie.set_expires(expires);

    let jar = jar.add(refresh_cookie);
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
    let refresh_token = jar.get("refreshCookie").map(Cookie::value);
    let tokens = state.user_service.refresh(refresh_token).await?;

    let mut refresh_cookie = Cookie::new("refreshCookie", tokens.refresh_token);
    refresh_cookie.set_path("/");
    refresh_cookie.set_http_only(true);
    refresh_cookie.set_same_site(SameSite::Strict);
    let expires = OffsetDateTime::now_utc() + time::Duration::seconds(REFRESH_TOKEN_EXPIRATION.cast_signed());
    refresh_cookie.set_expires(expires);

    let jar = jar.add(refresh_cookie);
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
    let refresh_token = jar.get("refreshCookie").map(Cookie::value);
    state.user_service.sign_out(refresh_token).await?;

    let mut remove_cookie = Cookie::new("refreshCookie", "");
    remove_cookie.set_path("/");
    remove_cookie.set_max_age(time::Duration::seconds(0));

    let jar = jar.add(remove_cookie);
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

    let auth_url = state.google_oauth.generate_auth_url(&oauth_state, &code_verifier);

    let mut state_cookie = Cookie::new("google_oauth_state", oauth_state);
    state_cookie.set_path("/");
    state_cookie.set_http_only(true);
    state_cookie.set_max_age(time::Duration::seconds(OAUTH_COOKIE_MAX_AGE_SECS));
    state_cookie.set_same_site(SameSite::Lax);

    let mut verifier_cookie = Cookie::new("google_oauth_code_verifier", code_verifier);
    verifier_cookie.set_path("/");
    verifier_cookie.set_http_only(true);
    verifier_cookie.set_max_age(time::Duration::seconds(OAUTH_COOKIE_MAX_AGE_SECS));
    verifier_cookie.set_same_site(SameSite::Lax);

    let jar = jar.add(state_cookie).add(verifier_cookie);
    Ok((jar, Redirect::temporary(&auth_url)))
}

/// Xử lý callback Google: đối chiếu state, đăng nhập rồi redirect về frontend kèm token.
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

    let google_user = state.google_oauth.fetch_user_info(&code).await?;
    let tokens = state.user_service.sign_in_with_google(google_user).await?;

    let mut refresh_cookie = Cookie::new("refreshCookie", tokens.refresh_token);
    refresh_cookie.set_path("/");
    refresh_cookie.set_http_only(true);
    refresh_cookie.set_same_site(SameSite::Strict);
    let expires = OffsetDateTime::now_utc() + time::Duration::seconds(REFRESH_TOKEN_EXPIRATION.cast_signed());
    refresh_cookie.set_expires(expires);

    let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_else(|_| DEFAULT_FRONTEND_URL.to_string());
    let redirect_url = format!("{}/login?token={}", frontend_url, tokens.access_token);

    let jar = jar
        .remove(Cookie::from("google_oauth_state"))
        .remove(Cookie::from("google_oauth_code_verifier"))
        .add(refresh_cookie);

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
    ValidatedJson(body): ValidatedJson<ChangePasswordRequest>,
) -> Result<SuccessResponse<()>, AppError> {
    let ip = extract_client_ip(&headers);
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
        .await
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
