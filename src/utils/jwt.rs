use crate::{
    config::constants::{
        DEFAULT_JWT_AUDIENCE, DEFAULT_JWT_ISSUER, DEFAULT_JWT_SECRET, JWT_ACCESS_TOKEN_EXPIRATION_SECS,
        JWT_LEEWAY_SECS, JWT_REFRESH_TOKEN_EXPIRATION_SECS, MIN_JWT_SECRET_LEN,
    },
    errors::{AppError, BusinessError},
};
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lỗi cấu hình JWT lúc boot — đều fatal: thiếu secret hoặc secret yếu thì
/// server không được start (fail-closed thay vì ký token forge được).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JwtConfigError {
    /// Thiếu `JWT_SECRET` (không còn fallback production).
    #[error("JWT_SECRET is required but was not set")]
    MissingSecret,
    /// Secret ngắn hơn ngưỡng brute-force HS256.
    #[error("JWT_SECRET is {len} bytes, minimum is {min} bytes")]
    SecretTooShort {
        /// Độ dài secret hiện tại (byte).
        len: usize,
        /// Ngưỡng tối thiểu ([`MIN_JWT_SECRET_LEN`]).
        min: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
    pub aud: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: String,
    pub jti: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
    pub aud: String,
}

#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub issuer: String,
    pub audience: String,
    pub access_token_expiration_secs: usize,
    pub refresh_token_expiration_secs: usize,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_string()),
            issuer: std::env::var("JWT_ISSUER").unwrap_or_else(|_| DEFAULT_JWT_ISSUER.to_string()),
            audience: std::env::var("JWT_AUDIENCE").unwrap_or_else(|_| DEFAULT_JWT_AUDIENCE.to_string()),
            access_token_expiration_secs: JWT_ACCESS_TOKEN_EXPIRATION_SECS,
            refresh_token_expiration_secs: JWT_REFRESH_TOKEN_EXPIRATION_SECS,
        }
    }
}

impl JwtConfig {
    /// Đọc cấu hình JWT từ env, fail-closed khi thiếu secret yếu.
    ///
    /// `JWT_SECRET` bắt buộc và ≥ [`MIN_JWT_SECRET_LEN`] byte (boot fail nếu không);
    /// `JWT_ISSUER`/`JWT_AUDIENCE` giữ fallback vì không phải secret. Sync vì chỉ
    /// đọc env + validate độ dài trong RAM, không có I/O.
    ///
    /// # Errors
    ///
    /// Trả [`JwtConfigError::MissingSecret`] khi thiếu `JWT_SECRET`,
    /// [`JwtConfigError::SecretTooShort`] khi secret yếu.
    pub fn from_env() -> Result<Self, JwtConfigError> {
        let secret = std::env::var("JWT_SECRET").map_err(|_| JwtConfigError::MissingSecret)?;
        Self::new(
            secret,
            std::env::var("JWT_ISSUER").unwrap_or_else(|_| DEFAULT_JWT_ISSUER.to_string()),
            std::env::var("JWT_AUDIENCE").unwrap_or_else(|_| DEFAULT_JWT_AUDIENCE.to_string()),
        )
    }

    /// Dựng config từ secret tường minh, validate độ dài tối thiểu.
    ///
    /// Sync vì chỉ so sánh độ dài trong RAM, không có I/O (`Result` đã `#[must_use]` sẵn).
    ///
    /// # Errors
    ///
    /// Trả [`JwtConfigError::SecretTooShort`] khi secret ngắn hơn [`MIN_JWT_SECRET_LEN`] byte.
    pub fn new(secret: String, issuer: String, audience: String) -> Result<Self, JwtConfigError> {
        if secret.len() < MIN_JWT_SECRET_LEN {
            return Err(JwtConfigError::SecretTooShort { len: secret.len(), min: MIN_JWT_SECRET_LEN });
        }
        Ok(Self {
            secret,
            issuer,
            audience,
            access_token_expiration_secs: JWT_ACCESS_TOKEN_EXPIRATION_SECS,
            refresh_token_expiration_secs: JWT_REFRESH_TOKEN_EXPIRATION_SECS,
        })
    }

    /// Dựng `Validation` pin cứng HS256 + leeway + bắt buộc `exp`/`iss`/`aud`.
    ///
    /// Pin `alg` chống algorithm-confusion (token `none`/RS256 lạ bị từ chối);
    /// `required_spec_claims` tường minh thay vì rely default của crate.
    /// Pure constructor nên sync + `#[must_use]`, không `async`.
    #[must_use]
    fn validation(&self) -> Validation {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = JWT_LEEWAY_SECS;
        validation.required_spec_claims = ["exp", "iss", "aud"].into_iter().map(str::to_string).collect();
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation
    }

    /// Ký access token 15 phút cho user.
    ///
    /// # Errors
    ///
    /// Trả `Unauthorized` khi sign JWT thất bại.
    pub fn gen_access_token(&self, user_id: Uuid) -> Result<String, AppError> {
        let now = usize::try_from(Utc::now().timestamp()).unwrap_or_default();
        let claims = AccessClaims {
            sub: user_id.to_string(),
            token_type: "access".to_string(),
            iat: now,
            exp: now + self.access_token_expiration_secs,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
        };

        encode(&Header::new(Algorithm::HS256), &claims, &EncodingKey::from_secret(self.secret.as_bytes()))
            .map_err(|_| AppError::Business(BusinessError::Unauthorized))
    }

    /// Ký refresh token 7 ngày gắn `jti` cho user.
    ///
    /// # Errors
    ///
    /// Trả `Unauthorized` khi sign JWT thất bại.
    pub fn gen_refresh_token(&self, user_id: Uuid, jti: &str) -> Result<String, AppError> {
        let now = usize::try_from(Utc::now().timestamp()).unwrap_or_default();
        let claims = RefreshClaims {
            sub: user_id.to_string(),
            jti: jti.to_string(),
            token_type: "refresh".to_string(),
            iat: now,
            exp: now + self.refresh_token_expiration_secs,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
        };

        encode(&Header::new(Algorithm::HS256), &claims, &EncodingKey::from_secret(self.secret.as_bytes()))
            .map_err(|_| AppError::Business(BusinessError::Unauthorized))
    }

    /// Verify access token đúng issuer/audience và đúng `type`.
    ///
    /// # Errors
    ///
    /// Trả `Unauthorized` khi token sai, hết hạn hoặc sai loại.
    pub fn verify_access_token(&self, token: &str) -> Result<AccessClaims, AppError> {
        let validation = self.validation();

        let token_data = decode::<AccessClaims>(token, &DecodingKey::from_secret(self.secret.as_bytes()), &validation)
            .map_err(|_| AppError::Business(BusinessError::Unauthorized))?;

        if token_data.claims.token_type != "access" {
            return Err(AppError::Business(BusinessError::Unauthorized));
        }

        Ok(token_data.claims)
    }

    /// Verify refresh token đúng issuer/audience và đúng `type`.
    ///
    /// # Errors
    ///
    /// Trả `Unauthorized` khi token sai, hết hạn hoặc sai loại.
    pub fn verify_refresh_token(&self, token: &str) -> Result<RefreshClaims, AppError> {
        let validation = self.validation();

        let token_data = decode::<RefreshClaims>(token, &DecodingKey::from_secret(self.secret.as_bytes()), &validation)
            .map_err(|_| AppError::Business(BusinessError::Unauthorized))?;

        if token_data.claims.token_type != "refresh" {
            return Err(AppError::Business(BusinessError::Unauthorized));
        }

        Ok(token_data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_access_token_lifecycle() {
        let config = JwtConfig::default();
        let user_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));

        let token = config.gen_access_token(user_id).unwrap();
        let claims = config.verify_access_token(&token).unwrap();

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.token_type, "access");
    }

    #[test]
    fn test_jwt_refresh_token_lifecycle() {
        let config = JwtConfig::default();
        let user_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
        let jti = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string();

        let token = config.gen_refresh_token(user_id, &jti).unwrap();
        let claims = config.verify_refresh_token(&token).unwrap();

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.jti, jti);
        assert_eq!(claims.token_type, "refresh");
    }

    #[test]
    fn test_invalid_token_fails() {
        let config = JwtConfig::default();
        assert!(config.verify_access_token("invalid.token.here").is_err());
    }

    #[test]
    fn test_new_rejects_short_secret() {
        // `matches!` thay vì `unwrap_err` vì `JwtConfig` cố ý không `Debug` (chứa secret).
        assert!(matches!(
            JwtConfig::new("short".to_string(), "iss".to_string(), "aud".to_string()),
            Err(JwtConfigError::SecretTooShort { len: 5, min: MIN_JWT_SECRET_LEN })
        ));
    }

    #[test]
    fn test_new_accepts_long_secret_and_roundtrips() {
        let secret = "s".repeat(MIN_JWT_SECRET_LEN);
        let config = JwtConfig::new(secret, "iss".to_string(), "aud".to_string()).unwrap();
        let user_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
        let token = config.gen_access_token(user_id).unwrap();
        let claims = config.verify_access_token(&token).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
    }

    #[test]
    fn test_cross_secret_verify_fails() {
        let issuer = "iss".to_string();
        let audience = "aud".to_string();
        let signer = JwtConfig::new("a".repeat(MIN_JWT_SECRET_LEN), issuer.clone(), audience.clone()).unwrap();
        let verifier = JwtConfig::new("b".repeat(MIN_JWT_SECRET_LEN), issuer, audience).unwrap();
        let user_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
        let token = signer.gen_access_token(user_id).unwrap();
        assert!(verifier.verify_access_token(&token).is_err());
    }
}
