/// Lỗi hạ tầng hệ thống (DB, cache, mail, ...).
///
/// Mọi variant mặc định map sang `500`, trừ [`SystemError::Cache`] (dependency bắt
/// buộc cho auth) map sang `503` để client/LB phân biệt "sập tạm thời, retry được"
/// với "lỗi server".
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("Password hashing failed: {0}")]
    Hash(#[from] argon2::password_hash::Error),

    #[error("Internal task execution failed: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("HTTP client error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Email address error: {0}")]
    EmailAddress(#[from] lettre::address::AddressError),

    #[error("Email build error: {0}")]
    EmailBuild(#[from] lettre::error::Error),

    #[error("Email delivery failed: {0}")]
    EmailDelivery(#[from] lettre::transport::smtp::Error),

    #[error("Internal server error: {0}")]
    Internal(String),

    /// Cache (Redis) lỗi trên path critical (session/OTP/revoke) — fail-closed 503.
    #[error("cache unavailable: {0}")]
    Cache(#[from] crate::utils::cache::CacheError),
}

impl SystemError {
    /// Map lỗi hệ thống sang HTTP status (`const` vì chỉ match discriminant).
    #[must_use]
    pub const fn status_code(&self) -> axum::http::StatusCode {
        match self {
            Self::Cache(_) => axum::http::StatusCode::SERVICE_UNAVAILABLE,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_error_display() {
        let db_err = sqlx::Error::RowNotFound;
        let err = SystemError::from(db_err);
        let msg = err.to_string();
        assert!(msg.contains("Database error"), "got: {msg}");
    }
}
