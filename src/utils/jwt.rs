use crate::{
    config::constants::{
        DEFAULT_JWT_AUDIENCE, DEFAULT_JWT_ISSUER, DEFAULT_JWT_SECRET, JWT_ACCESS_TOKEN_EXPIRATION_SECS,
        JWT_REFRESH_TOKEN_EXPIRATION_SECS,
    },
    errors::{AppError, BusinessError},
};
use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub fn from_env() -> Self {
        Self::default()
    }

    pub fn gen_access_token(&self, user_id: Uuid) -> Result<String, AppError> {
        let now = Utc::now().timestamp() as usize;
        let claims = AccessClaims {
            sub: user_id.to_string(),
            token_type: "access".to_string(),
            iat: now,
            exp: now + self.access_token_expiration_secs,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
        };

        encode(&Header::default(), &claims, &EncodingKey::from_secret(self.secret.as_bytes()))
            .map_err(|_| AppError::Business(BusinessError::Unauthorized))
    }

    pub fn gen_refresh_token(&self, user_id: Uuid, jti: &str) -> Result<String, AppError> {
        let now = Utc::now().timestamp() as usize;
        let claims = RefreshClaims {
            sub: user_id.to_string(),
            jti: jti.to_string(),
            token_type: "refresh".to_string(),
            iat: now,
            exp: now + self.refresh_token_expiration_secs,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
        };

        encode(&Header::default(), &claims, &EncodingKey::from_secret(self.secret.as_bytes()))
            .map_err(|_| AppError::Business(BusinessError::Unauthorized))
    }

    pub fn verify_access_token(&self, token: &str) -> Result<AccessClaims, AppError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);

        let token_data = decode::<AccessClaims>(token, &DecodingKey::from_secret(self.secret.as_bytes()), &validation)
            .map_err(|_| AppError::Business(BusinessError::Unauthorized))?;

        if token_data.claims.token_type != "access" {
            return Err(AppError::Business(BusinessError::Unauthorized));
        }

        Ok(token_data.claims)
    }

    pub fn verify_refresh_token(&self, token: &str) -> Result<RefreshClaims, AppError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);

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
}
