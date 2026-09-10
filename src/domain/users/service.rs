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
            request::{ChangePasswordRequest, ResetPasswordRequest, SignInRequest, SignUpRequest, UpdateUserRequest},
            response::{TokensResponse, UserResponse},
        },
    },
    errors::{AppError, BusinessError, map_unique_violation},
    utils::{
        cache::{CACHE_EXPIRATION, Cache, CacheStore, CacheStoreExt, OTP_EXPIRATION, REFRESH_TOKEN_EXPIRATION},
        email::Mailer,
        email_template::OtpEmail,
        hash::{hash_password, verify_password},
        i18n::Lang,
        jwt::JwtConfig,
        oauth::GoogleUserInfo,
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
        let key = format!("otp:{}", email.to_lowercase());

        self.cache.set(&key, &otp, OTP_EXPIRATION).await;
        self.mailer.send(email, &OtpEmail::new(otp), lang).await;

        Ok(())
    }

    /// Tạo user mới sau khi đối chiếu OTP.
    ///
    /// # Errors
    ///
    /// Trả `InvalidOtp` khi OTP sai/hết hạn, `Conflict` khi email trùng, lỗi hash/DB khi ghi.
    pub async fn sign_up(&self, data: SignUpRequest) -> Result<UserResponse, AppError> {
        let key = format!("otp:{}", data.email.to_lowercase());
        let cached_otp: Option<String> = self.cache.get(&key).await;

        match cached_otp {
            Some(otp) if otp == data.otp => {}
            _ => return Err(AppError::Business(BusinessError::InvalidOtp)),
        }

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

        self.cache.delete(&key).await;

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

    /// Đăng xuất: thu hồi refresh token khỏi cache (idempotent, token lạ thì bỏ qua).
    ///
    /// # Errors
    ///
    /// Luôn `Ok` — không có lỗi nghiệp vụ.
    pub async fn sign_out(&self, refresh_token: Option<&str>) -> Result<(), AppError> {
        let Some(token) = refresh_token else {
            return Ok(());
        };

        if let Ok(claims) = self.jwt_config.verify_refresh_token(token) {
            let refresh_key = format!("refreshToken:{}", claims.jti);
            let user_key = format!("user:{}", claims.sub);
            let session_key = format!("sessions:{}", claims.sub);

            let active_sessions: Vec<String> = self.cache.get(&session_key).await.unwrap_or_default();
            let updated_sessions: Vec<String> = active_sessions.into_iter().filter(|jti| jti != &claims.jti).collect();

            if updated_sessions.is_empty() {
                self.cache.delete_many(&[refresh_key, user_key, session_key]).await;
            } else {
                self.cache.delete_many(&[refresh_key, user_key]).await;
                self.cache.set(&session_key, &updated_sessions, REFRESH_TOKEN_EXPIRATION).await;
            }
        }

        Ok(())
    }

    /// Xoay refresh token: cấp cặp mới, thu hồi token cũ (chống replay).
    ///
    /// # Errors
    ///
    /// Trả `InvalidSession` khi thiếu token, token hết hạn hoặc đã bị thu hồi.
    pub async fn refresh(&self, old_refresh_token: Option<&str>) -> Result<TokensResponse, AppError> {
        let Some(token) = old_refresh_token else {
            return Err(AppError::Business(BusinessError::InvalidSession));
        };

        let claims = self
            .jwt_config
            .verify_refresh_token(token)
            .map_err(|_| AppError::Business(BusinessError::InvalidSession))?;

        let old_key = format!("refreshToken:{}", claims.jti);
        let cached_user_id: Option<String> = self.cache.get(&old_key).await;
        if cached_user_id.is_none() {
            return Err(AppError::Business(BusinessError::InvalidSession));
        }

        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Business(BusinessError::InvalidSession))?;

        let new_jti = Uuid::now_v7().to_string();
        let new_key = format!("refreshToken:{new_jti}");

        let session_key = format!("sessions:{}", claims.sub);
        let active_sessions: Vec<String> = self.cache.get(&session_key).await.unwrap_or_default();

        let mut updated_sessions: Vec<String> = active_sessions.into_iter().filter(|jti| jti != &claims.jti).collect();
        updated_sessions.push(new_jti.clone());

        let new_access_token = self.jwt_config.gen_access_token(user_id)?;
        let new_refresh_token = self.jwt_config.gen_refresh_token(user_id, &new_jti)?;

        self.cache.delete(&old_key).await;
        self.cache.set(&new_key, &claims.sub, REFRESH_TOKEN_EXPIRATION).await;
        self.cache.set(&session_key, &updated_sessions, REFRESH_TOKEN_EXPIRATION).await;

        Ok(TokensResponse { access_token: new_access_token, refresh_token: new_refresh_token })
    }

    /// Lấy user theo id, ưu tiên cache 1 phút.
    ///
    /// # Errors
    ///
    /// Trả `UserNotFound` khi id không tồn tại.
    pub async fn find_by_id(&self, id: Uuid) -> Result<UserResponse, AppError> {
        let key = format!("user:{id}");
        if let Some(cached) = self.cache.get::<UserResponse>(&key).await {
            return Ok(cached);
        }

        let user = self.repo.find_by_id(&self.pool, id).await?;
        let user = user.ok_or_else(|| AppError::Business(BusinessError::UserNotFound(id.to_string())))?;

        let response = UserMapper::to_response(user);
        self.cache.set(&key, &response, CACHE_EXPIRATION).await;

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

        let key = format!("user:{id}");
        self.cache.set(&key, &response, CACHE_EXPIRATION).await;

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

        let key = format!("user:{id}");
        self.cache.delete(&key).await;
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

        // Invalidate active sessions
        let session_key = format!("sessions:{user_id}");
        let active_sessions: Vec<String> = self.cache.get(&session_key).await.unwrap_or_default();

        let mut gone = vec![format!("user:{user_id}"), session_key];
        gone.extend(active_sessions.into_iter().map(|jti| format!("refreshToken:{jti}")));
        self.cache.delete_many(&gone).await;

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
        let key = format!("forgot_otp:{}", email.to_lowercase());

        self.cache.set(&key, &otp, OTP_EXPIRATION).await;
        self.mailer.send(email, &OtpEmail::new(otp), lang).await;

        Ok(())
    }

    /// Reset password bằng OTP, thu hồi mọi session đang active.
    ///
    /// # Errors
    ///
    /// Trả `InvalidOtp` khi OTP sai/hết hạn, `UserNotFound` khi email không tồn tại.
    pub async fn reset_password(&self, data: ResetPasswordRequest) -> Result<(), AppError> {
        let key = format!("forgot_otp:{}", data.email.to_lowercase());
        let cached_otp: Option<String> = self.cache.get(&key).await;

        match cached_otp {
            Some(otp) if otp == data.otp => {}
            _ => return Err(AppError::Business(BusinessError::InvalidOtp)),
        }

        let user = self.repo.find_by_email(&self.pool, &data.email).await?;
        let user = user.ok_or_else(|| AppError::Business(BusinessError::UserNotFound(data.email.clone())))?;

        let new_hash = hash_password(&self.argon2, data.new_password).await?;
        self.repo
            .update(&self.pool, user.id, &UpdateUserEntity { password_hash: Some(new_hash), ..Default::default() })
            .await?;

        // Invalidate all active sessions
        let session_key = format!("sessions:{}", user.id);
        let active_sessions: Vec<String> = self.cache.get(&session_key).await.unwrap_or_default();

        let mut gone = vec![key, format!("user:{}", user.id), session_key];
        gone.extend(active_sessions.into_iter().map(|jti| format!("refreshToken:{jti}")));
        self.cache.delete_many(&gone).await;

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

        let key = format!("refreshToken:{jti}");
        let session_key = format!("sessions:{user_id}");

        let mut active_sessions: Vec<String> = self.cache.get(&session_key).await.unwrap_or_default();
        active_sessions.push(jti);
        if active_sessions.len() > MAX_SESSIONS_PER_USER {
            let drain_count = active_sessions.len() - MAX_SESSIONS_PER_USER;
            let gone: Vec<String> =
                active_sessions.drain(0..drain_count).map(|old_jti| format!("refreshToken:{old_jti}")).collect();
            self.cache.delete_many(&gone).await;
        }

        self.cache.set(&key, &user_id.to_string(), REFRESH_TOKEN_EXPIRATION).await;
        self.cache.set(&session_key, &active_sessions, REFRESH_TOKEN_EXPIRATION).await;

        Ok(TokensResponse { access_token, refresh_token })
    }
}
