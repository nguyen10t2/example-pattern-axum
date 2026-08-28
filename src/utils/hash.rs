use argon2::{
    Argon2, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

use crate::errors::SystemError;

fn hash_password_blocking(argon2: &Argon2<'_>, password: &[u8]) -> Result<String, SystemError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2.hash_password(password, &salt)?;
    Ok(hash.to_string())
}

fn verify_password_blocking(argon2: &Argon2<'_>, password: &[u8], hash: &str) -> Result<bool, SystemError> {
    let parsed_hash = argon2::PasswordHash::new(hash)?;
    Ok(argon2.verify_password(password, &parsed_hash).is_ok())
}

pub async fn hash_password(argon2: &Argon2<'static>, password: String) -> Result<String, SystemError> {
    let argon2 = argon2.clone();
    tokio::task::spawn_blocking(move || hash_password_blocking(&argon2, password.as_bytes())).await?
}

pub async fn verify_password(argon2: &Argon2<'static>, password: String, hash: String) -> Result<bool, SystemError> {
    let argon2 = argon2.clone();
    tokio::task::spawn_blocking(move || verify_password_blocking(&argon2, password.as_bytes(), &hash)).await?
}

#[cfg(test)]
mod tests {
    use argon2::{Algorithm, Argon2, Params, Version};

    use super::*;

    const TEST_PASSWORD: &str = "test_password";

    fn test_argon2() -> Argon2<'static> {
        let params =
            Params::new(Params::DEFAULT_M_COST, Params::DEFAULT_T_COST, Params::DEFAULT_P_COST, Some(32)).unwrap();
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
    }

    #[tokio::test]
    async fn test_hash_success() {
        let argon2 = test_argon2();
        let hash = hash_password(&argon2, TEST_PASSWORD.to_string()).await.unwrap();
        assert!(!hash.is_empty());
    }

    #[tokio::test]
    async fn test_verify_success() {
        let argon2 = test_argon2();
        let hash = hash_password(&argon2, TEST_PASSWORD.to_string()).await.unwrap();
        let is_valid = verify_password(&argon2, TEST_PASSWORD.to_string(), hash).await.unwrap();
        assert!(is_valid);
    }

    #[tokio::test]
    async fn test_verify_wrong_password_fails() {
        let argon2 = test_argon2();
        let hash = hash_password(&argon2, TEST_PASSWORD.to_string()).await.unwrap();
        let is_valid = verify_password(&argon2, "wrong_password".to_string(), hash).await.unwrap();
        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_hash_produces_distinct_results() {
        let argon2 = test_argon2();
        let hash1 = hash_password(&argon2, TEST_PASSWORD.to_string()).await.unwrap();
        let hash2 = hash_password(&argon2, TEST_PASSWORD.to_string()).await.unwrap();
        assert_ne!(hash1, hash2, "same password should produce different hashes due to random salt");
    }

    #[tokio::test]
    async fn test_hash_format_looks_like_argon2id() {
        let argon2 = test_argon2();
        let hash = hash_password(&argon2, TEST_PASSWORD.to_string()).await.unwrap();
        assert!(hash.starts_with("$argon2id$"), "hash should use argon2id algorithm, got: {hash}");
    }

    #[tokio::test]
    async fn test_verify_with_malformed_hash_fails() {
        let argon2 = test_argon2();
        let result = verify_password(&argon2, TEST_PASSWORD.to_string(), "not-a-valid-hash".to_string()).await;
        // Should return an error, not panic
        assert!(result.is_err());
    }
}
