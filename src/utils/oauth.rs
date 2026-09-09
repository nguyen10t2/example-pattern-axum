use crate::{
    config::constants::{DEFAULT_FRONTEND_URL, GOOGLE_CALLBACK_PATH},
    errors::{AppError, BusinessError, SystemError},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    #[must_use]
    pub fn generate_auth_url(&self, state: &str, code_challenge: &str) -> String {
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id={}&redirect_uri={}&scope=openid%20profile%20email&state={}&code_challenge={}&code_challenge_method=S256",
            self.client_id, self.redirect_uri, state, code_challenge
        )
    }

    /// Lấy profile Google bằng access token (I/O mạng, phải async).
    ///
    /// # Errors
    ///
    /// Trả `Unauthorized` khi Google từ chối, lỗi hệ thống khi parse response.
    pub async fn fetch_user_info(&self, access_token: &str) -> Result<GoogleUserInfo, AppError> {
        let client = reqwest::Client::new();
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
    Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string()
}

/// Sinh code verifier cho flow PKCE.
#[must_use]
pub fn generate_code_verifier() -> String {
    Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string()
}
