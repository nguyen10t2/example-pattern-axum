use crate::{
    config::constants::{DEFAULT_FRONTEND_URL, GOOGLE_CALLBACK_PATH},
    errors::{AppError, BusinessError, SystemError},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub name: String,
    pub picture: Option<String>,
    pub email: String,
    #[serde(default)]
    pub email_verified: bool,
}

#[derive(Clone)]
pub struct GoogleOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
}

impl Default for GoogleOAuthConfig {
    fn default() -> Self {
        Self {
            client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
            client_secret: std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
            redirect_uri: std::env::var("GOOGLE_REDIRECT_URI")
                .unwrap_or_else(|_| format!("{DEFAULT_FRONTEND_URL}{GOOGLE_CALLBACK_PATH}")),
        }
    }
}

impl GoogleOAuthConfig {
    /// Đọc cấu hình OAuth Google từ env (có fallback dev).
    #[must_use]
    pub fn from_env() -> Self {
        Self::default()
    }

    /// Dựng URL authorize Google (PKCE `S256`) cho frontend redirect.
    ///
    /// # Errors
    ///
    /// Returns an internal configuration error when OAuth credentials are absent
    /// or the fixed authorization endpoint cannot be parsed.
    pub fn generate_auth_url(&self, state: &str, code_challenge: &str) -> Result<String, AppError> {
        if self.client_id.is_empty() || self.client_secret.is_empty() {
            return Err(AppError::System(SystemError::Internal("Google OAuth is not configured".to_string())));
        }

        let mut url = reqwest::Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
            .map_err(|e| AppError::System(SystemError::Internal(format!("invalid Google authorize URL: {e}"))))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("scope", "openid profile email")
            .append_pair("state", state)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url.into())
    }

    /// Exchange the short-lived authorization code using the original PKCE verifier.
    ///
    /// # Errors
    ///
    /// Returns `Unauthorized` when Google rejects the code and a system error for
    /// transport failures or malformed successful responses.
    pub async fn exchange_code(&self, code: &str, code_verifier: &str) -> Result<String, AppError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| AppError::System(SystemError::Reqwest(e)))?;
        let params = [
            ("code", code),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier),
        ];
        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::System(SystemError::Reqwest(e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Business(BusinessError::Unauthorized));
        }

        resp.json::<GoogleTokenResponse>()
            .await
            .map(|body| body.access_token)
            .map_err(|e| AppError::System(SystemError::Reqwest(e)))
    }

    /// Lấy profile Google bằng access token (I/O mạng, phải async).
    ///
    /// # Errors
    ///
    /// Trả `Unauthorized` khi Google từ chối, lỗi hệ thống khi parse response.
    pub async fn fetch_user_info(&self, access_token: &str) -> Result<GoogleUserInfo, AppError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| AppError::System(SystemError::Reqwest(e)))?;
        let resp = client
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AppError::System(SystemError::Reqwest(e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Business(BusinessError::Unauthorized));
        }

        let user_info = resp.json::<GoogleUserInfo>().await.map_err(|e| AppError::System(SystemError::Reqwest(e)))?;

        Ok(user_info)
    }
}

/// Sinh state chống CSRF cho flow OAuth.
#[must_use]
pub fn generate_state() -> String {
    random_base64url(32)
}

/// Sinh code verifier cho flow PKCE.
#[must_use]
pub fn generate_code_verifier() -> String {
    // 32 random bytes encode to 43 URL-safe characters, satisfying RFC 7636.
    random_base64url(32)
}

/// Derive the RFC 7636 S256 code challenge sent to the authorization endpoint.
#[must_use]
pub fn generate_code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn random_base64url(byte_len: usize) -> String {
    let mut bytes = vec![0_u8; byte_len];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_verifier_and_challenge_are_rfc_7636_compatible() {
        let verifier = generate_code_verifier();
        let challenge = generate_code_challenge(&verifier);

        assert_eq!(verifier.len(), 43);
        assert_eq!(challenge.len(), 43);
        assert!(verifier.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'));
    }

    #[test]
    fn authorization_url_percent_encodes_configuration_and_nonce() {
        let config = GoogleOAuthConfig {
            client_id: "client id".to_string(),
            client_secret: "secret".to_string(),
            redirect_uri: "https://example.com/callback?a=b".to_string(),
        };
        let generated = config.generate_auth_url("state/value", "challenge+").unwrap();
        let url = reqwest::Url::parse(&generated).unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();

        assert_eq!(params.get("client_id").map(String::as_str), Some("client id"));
        assert_eq!(params.get("redirect_uri").map(String::as_str), Some("https://example.com/callback?a=b"));
        assert_eq!(params.get("state").map(String::as_str), Some("state/value"));
        assert_eq!(params.get("code_challenge").map(String::as_str), Some("challenge+"));
    }
}
