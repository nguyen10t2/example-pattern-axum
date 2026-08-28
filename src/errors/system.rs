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

    #[error("Internal server error: {0}")]
    Internal(String),
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
