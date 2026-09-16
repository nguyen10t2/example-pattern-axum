use argon2::Argon2;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    config::constants::MAX_SESSIONS_PER_USER,
    domain::{
        Currency,
        users::{
            entity::{NewUserEntity, UpdateUserEntity},
            mapper::UserMapper,
            repository::UserRepository,
            request::{
                ChangePasswordRequest, OtpPurpose, ResetPasswordRequest, SignInRequest, SignUpRequest,
                UpdateUserRequest,
            },
            response::{TokensResponse, UserResponse},
        },
    },
    errors::{AppError, BusinessError, map_unique_violation},
    utils::{
        cache::{
            CACHE_EXPIRATION, Cache, CacheStore, CacheStoreExt, NewRefreshSession, REFRESH_TOKEN_EXPIRATION,
            RefreshRotation, RevokeRefreshSession, RotationOutcome, refresh_token_key, session_list_key,
            user_profile_key,
        },
        email::Mailer,
        email_template::OtpEmail,
        hash::{hash_password, verify_password},
        i18n::Lang,
        jwt::JwtConfig,
        oauth::GoogleUserInfo,
        otp,
        random::generate_otp,
    },
};
use sqlx::PgPool;

pub struct UserService<R: UserRepository> {
    repo: R,
    cache: Arc<Cache>,
    argon2: Arc<Argon2<'static>>,
    jwt_config: JwtConfig,
    mailer: Mailer,
    pool: PgPool,
}

impl<R: UserRepository> UserService<R> {
    /// Ghép repo, cache, JWT và mailer thành service user.
    pub const fn new(
        repo: R,
        cache: Arc<Cache>,
        argon2: Arc<Argon2<'static>>,
        jwt_config: JwtConfig,
        mailer: Mailer,
        pool: PgPool,
    ) -> Self {
        Self { repo, cache, argon2, jwt_config, mailer, pool }
    }

    /// Gửi OTP đăng ký qua email, lưu cache 2 phút.
    ///
    /// # Errors
    ///
    /// Trả `UserAlreadyExists` nếu email đã có tài khoản.
    pub async fn request_otp(&self, email: &str, lang: Lang) -> Result<(), AppError> {
        let existing_user = self.repo.find_by_email(&self.pool, email).await?;
        if existing_user.is_some() {
            return Err(AppError::Business(BusinessError::UserAlreadyExists));
        }

        let otp = generate_otp();
        otp::store(&self.cache, &otp::signup_key(email), otp.clone()).await?;
        self.mailer.send(email, &OtpEmail::new(otp), lang).await;

        Ok(())
    }

    /// Kiểm tra OTP mà không tiêu thụ (cho FE verify sớm).
    ///
    /// Sai quá `MAX_OTP_ATTEMPTS` lần thì hủy mã, bắt xin lại.
    ///
    /// # Errors
    ///
    /// Trả `InvalidOtp` khi mã sai, hết hạn hoặc đã bị hủy do sai nhiều lần.
    pub async fn verify_otp(&self, email: &str, otp: &str, purpose: OtpPurpose) -> Result<(), AppError> {
        let key = match purpose {
            OtpPurpose::Signup => otp::signup_key(email),
            OtpPurpose::Reset => otp::reset_key(email),
        };
        otp::check(&self.cache, &key, otp).await
    }

    /// Tạo user mới sau khi đối chiếu OTP.
    ///
    /// # Errors
    ///
    /// Trả `InvalidOtp` khi OTP sai/hết hạn, `Conflict` khi email trùng, lỗi hash/DB khi ghi.
    pub async fn sign_up(&self, data: SignUpRequest) -> Result<UserResponse, AppError> {
        let key = otp::signup_key(&data.email);
        otp::check(&self.cache, &key, &data.otp).await?;

        let password_hash = hash_password(&self.argon2, data.password).await?;
        let user_id = Uuid::now_v7();

        let new_user = NewUserEntity {
            id: user_id,
            full_name: data.full_name,
            email: data.email.to_lowercase(),
            email_verified: true,
            password_hash: Some(password_hash),
            google_id: None,
            avatar_url: None,
            phone: None,
            phone_verified: false,
            preferred_currency: Currency::VND,
            is_active: true,
        };

        let created_user = self
            .repo
            .create(&self.pool, &new_user)
            .await
            .map_err(|err| map_unique_violation(err, &[("email", &new_user.email)]))?;

        self.cache.delete(&key).await?;

        info!(email = %created_user.email, user_id = %created_user.id, "User signed up");
        Ok(UserMapper::to_response(created_user))
    }

    /// Đăng nhập email/password, trả cặp access + refresh token.
    ///
    /// # Errors
    ///
    /// Trả `InvalidCredentials` khi email không tồn tại hoặc sai password.
    pub async fn sign_in(&self, data: SignInRequest) -> Result<TokensResponse, AppError> {
        let user = self.repo.find_by_email(&self.pool, &data.email).await?;
        let Some(user) = user else {
            warn!(email = %data.email, "Failed sign-in attempt: Email not found");
            return Err(AppError::Business(BusinessError::InvalidCredentials));
        };

        let Some(password_hash) = &user.password_hash else {
            return Err(AppError::Business(BusinessError::InvalidCredentials));
        };

        let is_valid = verify_password(&self.argon2, data.password, password_hash.clone()).await?;
        if !is_valid {
            warn!(email = %data.email, user_id = %user.id, "Failed sign-in attempt: Invalid password");
            return Err(AppError::Business(BusinessError::InvalidCredentials));
        }

        let tokens = self.create_session(user.id).await?;
        info!(email = %user.email, user_id = %user.id, "User signed in");
        Ok(tokens)
    }

    /// Đăng nhập Google: link vào account cùng email nếu có, không thì tạo mới.
    ///
    /// # Errors
    ///
    /// Trả `EmailNotVerified` khi Google chưa verify email này.
    pub async fn sign_in_with_google(&self, google_user: GoogleUserInfo) -> Result<TokensResponse, AppError> {
        if !google_user.email_verified {
            return Err(AppError::Business(BusinessError::EmailNotVerified));
        }

        let user = match self.repo.find_by_google_id(&self.pool, &google_user.sub).await? {
            Some(mut user) => {
                if let Some(ref avatar) = google_user.picture
                    && user.avatar_url.as_ref() != Some(avatar)
                {
                    user = self
                        .repo
                        .update(
                            &self.pool,
                            user.id,
                            &UpdateUserEntity { avatar_url: Some(avatar.clone()), ..Default::default() },
                        )
                        .await?;
                }
                user
            }
            None => {
                if let Some(user) = self.repo.find_by_email(&self.pool, &google_user.email).await? {
                    let updated = self
                        .repo
                        .update(
                            &self.pool,
                            user.id,
                            &UpdateUserEntity {
                                google_id: Some(google_user.sub.clone()),
                                avatar_url: user.avatar_url.or(google_user.picture),
                                ..Default::default()
                            },
                        )
                        .await?;
                    info!(user_id = %updated.id, email = %google_user.email, "Linked Google account to existing user");
                    updated
                } else {
                    let new_user = NewUserEntity {
                        id: Uuid::now_v7(),
                        full_name: google_user.name,
                        email: google_user.email.to_lowercase(),
                        email_verified: true,
                        password_hash: None,
                        google_id: Some(google_user.sub),
                        avatar_url: google_user.picture,
                        phone: None,
                        phone_verified: false,
                        preferred_currency: Currency::VND,
                        is_active: true,
                    };
                    let created = self.repo.create(&self.pool, &new_user).await?;
                    info!(user_id = %created.id, email = %created.email, "Created new user via Google Sign-In");
                    created
                }
            }
        };

        let tokens = self.create_session(user.id).await?;
        info!(email = %user.email, user_id = %user.id, "User signed in with Google");
        Ok(tokens)
    }

    /// Đăng xuất: thu hồi refresh token khỏi cache (idempotent với token lạ/hết hạn).
    ///
    /// Để `async` vì thu hồi session là I/O Redis thật.
    ///
    /// # Errors
    ///
    /// Trả lỗi hệ thống (cache unavailable → 503) khi Redis lỗi — fail-closed,
    /// không được báo "đã đăng xuất" giả trong khi session vẫn sống.
    pub async fn sign_out(&self, refresh_token: Option<&str>) -> Result<(), AppError> {
        let Some(token) = refresh_token else {
            return Ok(());
        };

        if let Ok(claims) = self.jwt_config.verify_refresh_token(token) {
            let refresh_key = refresh_token_key(&claims.jti);
            let session_key = session_list_key(&claims.sub);
            let revoke = RevokeRefreshSession {
                key: &refresh_key,
                session_key: &session_key,
                jti: &claims.jti,
                ttl_secs: REFRESH_TOKEN_EXPIRATION,
            };
            self.cache.revoke_refresh_session(&revoke).await?;
            if let Ok(user_id) = Uuid::parse_str(&claims.sub) {
                self.cache.delete_best_effort(&user_profile_key(user_id)).await;
            }
        }

        Ok(())
    }

    /// Xoay refresh token: cấp cặp mới, thu hồi token cũ (chống replay).
    ///
    /// Rotation chạy nguyên tử trong cache backend (1 round trip). Chữ ký đúng
    /// nhưng key đã mất (replay sau khi xoay hoặc dùng token đã revoke) thì
    /// revoke cả family rồi mới báo lỗi — token đánh cắp không dùng lại được.
    ///
    /// Để `async` vì rotation + revoke là I/O Redis thật.
    ///
    /// # Errors
    ///
    /// Trả `InvalidSession` khi thiếu token, token hết hạn, đã bị thu hồi hoặc replay.
    /// Trả lỗi hệ thống (cache unavailable → 503) khi Redis lỗi — fail-closed.
    pub async fn refresh(&self, old_refresh_token: Option<&str>) -> Result<TokensResponse, AppError> {
        let Some(token) = old_refresh_token else {
            return Err(AppError::Business(BusinessError::InvalidSession));
        };

        let claims = self
            .jwt_config
            .verify_refresh_token(token)
            .map_err(|_| AppError::Business(BusinessError::InvalidSession))?;

        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Business(BusinessError::InvalidSession))?;

        let new_jti = Uuid::now_v7().to_string();
        // Ký trước, xoay sau: xoay lỗi (503) thì session cũ còn nguyên, client retry được.
        let new_access_token = self.jwt_config.gen_access_token(user_id)?;
        let new_refresh_token = self.jwt_config.gen_refresh_token(user_id, &new_jti)?;

        let old_key = refresh_token_key(&claims.jti);
        let new_key = refresh_token_key(&new_jti);
        let session_key = session_list_key(&claims.sub);
        let rotation = RefreshRotation {
            old_key: &old_key,
            new_key: &new_key,
            session_key: &session_key,
            subject: &claims.sub,
            old_jti: &claims.jti,
            new_jti: &new_jti,
            ttl_secs: REFRESH_TOKEN_EXPIRATION,
        };

        if self.cache.rotate_refresh_token(&rotation).await? == RotationOutcome::Stale {
            self.revoke_family(&claims.sub).await?;
            return Err(AppError::Business(BusinessError::InvalidSession));
        }

        Ok(TokensResponse { access_token: new_access_token, refresh_token: new_refresh_token })
    }

    /// Thu hồi toàn bộ sessions của subject (dùng khi phát hiện replay refresh token).
    ///
    /// Để `async` vì đọc + xóa Redis là I/O mạng thật.
    ///
    /// # Errors
    ///
    /// Trả lỗi hệ thống (cache unavailable → 503) khi Redis lỗi — fail-closed:
    /// không revoke được thì không được báo `InvalidSession` nhẹ nhàng.
    async fn revoke_family(&self, subject: &str) -> Result<(), AppError> {
        let session_key = session_list_key(subject);
        let active_sessions: Vec<String> = self.cache.get(&session_key).await?.unwrap_or_default();
        let mut gone: Vec<String> = active_sessions.into_iter().map(|jti| refresh_token_key(&jti)).collect();
        gone.push(session_key);
        self.cache.delete_many(&gone).await?;
        Ok(())
    }

    /// Lấy user theo id, ưu tiên cache 1 phút.
    ///
    /// # Errors
    ///
    /// Trả `UserNotFound` khi id không tồn tại.
    pub async fn find_by_id(&self, id: Uuid) -> Result<UserResponse, AppError> {
        let key = user_profile_key(id);
        if let Some(cached) = self.cache.get_best_effort::<UserResponse>(&key).await {
            return Ok(cached);
        }

        let user = self.repo.find_by_id(&self.pool, id).await?;
        let user = user.ok_or_else(|| AppError::Business(BusinessError::UserNotFound(id.to_string())))?;

        let response = UserMapper::to_response(user);
        self.cache.set_best_effort(&key, &response, CACHE_EXPIRATION).await;

        Ok(response)
    }

    /// Lấy user theo email (đọc thẳng DB, không cache).
    ///
    /// # Errors
    ///
    /// Trả `UserNotFound` khi email không tồn tại.
    pub async fn find_by_email(&self, email: &str) -> Result<UserResponse, AppError> {
        let user = self.repo.find_by_email(&self.pool, email).await?;
        let user = user.ok_or_else(|| AppError::Business(BusinessError::UserNotFound(email.to_string())))?;
        Ok(UserMapper::to_response(user))
    }

    /// Cập nhật profile rồi refresh cache user.
    ///
    /// # Errors
    ///
    /// Trả lỗi DB khi ghi thất bại.
    pub async fn update(&self, id: Uuid, data: UpdateUserRequest) -> Result<UserResponse, AppError> {
        let update_entity = UpdateUserEntity {
            full_name: data.full_name,
            is_active: data.is_active,
            phone: data.phone.map(Some),
            preferred_currency: data.preferred_currency,
            ..Default::default()
        };

        let updated_user = self.repo.update(&self.pool, id, &update_entity).await?;
        let response = UserMapper::to_response(updated_user);

        let key = user_profile_key(id);
        self.cache.set_best_effort(&key, &response, CACHE_EXPIRATION).await;

        Ok(response)
    }

    /// Xóa mềm user rồi xóa cache.
    ///
    /// # Errors
    ///
    /// Trả `UserNotFound` khi id không tồn tại.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = self.repo.soft_delete(&self.pool, id).await?;
        if result.is_none() {
            return Err(AppError::Business(BusinessError::UserNotFound(id.to_string())));
        }

        let key = user_profile_key(id);
        self.cache.delete_best_effort(&key).await;
        Ok(())
    }

    /// Đổi password sau khi verify mật khẩu cũ, thu hồi mọi session đang active.
    ///
    /// # Errors
    ///
    /// Trả `UserNotFound` khi user không tồn tại, `InvalidCredentials` khi sai mật khẩu cũ.
    pub async fn change_password(&self, user_id: Uuid, data: ChangePasswordRequest) -> Result<(), AppError> {
        let user = self.repo.find_by_id(&self.pool, user_id).await?;
        let user = user.ok_or_else(|| AppError::Business(BusinessError::UserNotFound(user_id.to_string())))?;

        let Some(password_hash) = user.password_hash else {
            return Err(AppError::Business(BusinessError::UserNotFound(user_id.to_string())));
        };

        let is_valid = verify_password(&self.argon2, data.old_password, password_hash).await?;
        if !is_valid {
            return Err(AppError::Business(BusinessError::InvalidCredentials));
        }

        let new_hash = hash_password(&self.argon2, data.new_password).await?;
        self.repo
            .update(&self.pool, user_id, &UpdateUserEntity { password_hash: Some(new_hash), ..Default::default() })
            .await?;

        // Invalidate active sessions — fail-closed: không đọc được sessions thì
        // không được báo đổi pass thành công trong khi session cũ vẫn sống.
        let session_key = session_list_key(&user_id.to_string());
        let active_sessions: Vec<String> = self.cache.get(&session_key).await?.unwrap_or_default();

        let mut gone = vec![user_profile_key(user_id), session_key];
        gone.extend(active_sessions.into_iter().map(|jti| refresh_token_key(&jti)));
        self.cache.delete_many(&gone).await?;

        info!(user_id = %user_id, "User changed password");
        Ok(())
    }

    /// Gửi OTP quên mật khẩu. Luôn `Ok` kể cả email lạ để chống dò email.
    ///
    /// # Errors
    ///
    /// Luôn `Ok` — không có lỗi nghiệp vụ.
    pub async fn request_forgot_password_otp(&self, email: &str, lang: Lang) -> Result<(), AppError> {
        let user = self.repo.find_by_email(&self.pool, email).await?;
        if user.is_none() {
            return Ok(()); // Prevent email enumeration
        }

        let otp = generate_otp();
        otp::store(&self.cache, &otp::reset_key(email), otp.clone()).await?;
        self.mailer.send(email, &OtpEmail::new(otp), lang).await;

        Ok(())
    }

    /// Reset password bằng OTP, thu hồi mọi session đang active.
    ///
    /// # Errors
    ///
    /// Trả `InvalidOtp` khi OTP sai/hết hạn, `UserNotFound` khi email không tồn tại.
    pub async fn reset_password(&self, data: ResetPasswordRequest) -> Result<(), AppError> {
        let key = otp::reset_key(&data.email);
        otp::check(&self.cache, &key, &data.otp).await?;

        let user = self.repo.find_by_email(&self.pool, &data.email).await?;
        let user = user.ok_or_else(|| AppError::Business(BusinessError::UserNotFound(data.email.clone())))?;

        let new_hash = hash_password(&self.argon2, data.new_password).await?;
        self.repo
            .update(&self.pool, user.id, &UpdateUserEntity { password_hash: Some(new_hash), ..Default::default() })
            .await?;

        // Invalidate all active sessions — fail-closed như `change_password`.
        let session_key = session_list_key(&user.id.to_string());
        let active_sessions: Vec<String> = self.cache.get(&session_key).await?.unwrap_or_default();

        let mut gone = vec![key, user_profile_key(user.id), session_key];
        gone.extend(active_sessions.into_iter().map(|jti| refresh_token_key(&jti)));
        self.cache.delete_many(&gone).await?;

        info!(user_id = %user.id, "User reset password");
        Ok(())
    }

    /// Tạo cặp token và lưu session; quá số session tối đa thì đuổi session cũ nhất.
    ///
    /// # Errors
    ///
    /// Trả lỗi khi sign JWT thất bại.
    async fn create_session(&self, user_id: Uuid) -> Result<TokensResponse, AppError> {
        let jti = Uuid::now_v7().to_string();
        let access_token = self.jwt_config.gen_access_token(user_id)?;
        let refresh_token = self.jwt_config.gen_refresh_token(user_id, &jti)?;

        // Ghi session nguyên tử (đuổi cũ nhất khi đầy) — fail-closed: đã ký token
        // nhưng chưa lưu session thì token vô dụng, phải báo 503 thay vì trả về.
        let subject = user_id.to_string();
        let key = refresh_token_key(&jti);
        let session_key = session_list_key(&subject);
        let new_session = NewRefreshSession {
            key: &key,
            session_key: &session_key,
            subject: &subject,
            jti: &jti,
            ttl_secs: REFRESH_TOKEN_EXPIRATION,
            max_sessions: MAX_SESSIONS_PER_USER,
        };
        self.cache.create_refresh_session(&new_session).await?;

        Ok(TokensResponse { access_token, refresh_token })
    }
}
