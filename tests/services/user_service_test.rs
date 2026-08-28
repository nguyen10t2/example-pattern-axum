use std::sync::Arc;
use uuid::Uuid;

use crate::common::{MockUserRepository, test_argon2, test_cache, test_jwt_config, test_pool};
use dsa::{
    domain::users::{
        entity::NewUserEntity,
        repository::UserRepository,
        request::{ChangePasswordRequest, ResetPasswordRequest, SignInRequest, SignUpRequest},
        service::UserService,
    },
    utils::{cache::CacheStoreExt, email::Mailer, hash::hash_password, oauth::GoogleUserInfo},
};

fn create_test_user_service()
-> (UserService<MockUserRepository>, Arc<dsa::utils::cache::MemoryCache>, MockUserRepository) {
    let repo = MockUserRepository::default();
    let cache = test_cache();
    let argon2 = test_argon2();
    let jwt_config = test_jwt_config();

    let service = UserService::new(repo.clone(), cache.clone(), argon2, jwt_config, Mailer::new(8), test_pool());
    (service, cache, repo)
}

#[tokio::test]
async fn test_user_signup_and_signin_flow() {
    let (service, cache, _) = create_test_user_service();

    // 1. Request OTP
    service.request_otp("alice@example.com").await.unwrap();

    // Check OTP in cache
    let otp: String = cache.get("otp:alice@example.com").await.unwrap();

    // 2. Sign Up with valid OTP
    let user = service
        .sign_up(SignUpRequest {
            email: "alice@example.com".to_string(),
            full_name: "Alice Smith".to_string(),
            password: "password123".to_string(),
            otp: otp.clone(),
        })
        .await
        .unwrap();

    assert_eq!(user.full_name, "Alice Smith");
    assert_eq!(user.email, "alice@example.com");

    // Sign Up with same OTP again should fail
    let duplicate = service
        .sign_up(SignUpRequest {
            email: "alice@example.com".to_string(),
            full_name: "Alice Smith".to_string(),
            password: "password123".to_string(),
            otp,
        })
        .await;
    assert!(duplicate.is_err());

    // 3. Sign In
    let tokens = service
        .sign_in(SignInRequest { email: "alice@example.com".to_string(), password: "password123".to_string() })
        .await
        .unwrap();

    assert!(!tokens.access_token.is_empty());
    assert!(!tokens.refresh_token.is_empty());

    // 4. Refresh token
    let refreshed = service.refresh(Some(&tokens.refresh_token)).await.unwrap();
    assert!(!refreshed.access_token.is_empty());

    // Old refresh token must be invalidated (single-use rotation)
    let reuse_err = service.refresh(Some(&tokens.refresh_token)).await;
    assert!(reuse_err.is_err());

    // 5. Sign Out
    service.sign_out(Some(&refreshed.refresh_token)).await.unwrap();
    let after_signout_refresh = service.refresh(Some(&refreshed.refresh_token)).await;
    assert!(after_signout_refresh.is_err());
}

#[tokio::test]
async fn test_change_and_reset_password() {
    let (service, cache, repo) = create_test_user_service();

    // Setup initial user
    let hash = hash_password(&test_argon2(), "oldpass123".to_string()).await.unwrap();
    let user_id = Uuid::now_v7();
    repo.create(
        &test_pool(),
        &NewUserEntity {
            id: user_id,
            full_name: "Bob Jones".to_string(),
            email: "bob@example.com".to_string(),
            email_verified: true,
            password_hash: Some(hash),
            google_id: None,
            avatar_url: None,
            phone: None,
            phone_verified: false,
            preferred_currency: dsa::domain::Currency::VND,
            is_active: true,
        },
    )
    .await
    .unwrap();

    // Change password with wrong old password
    let wrong_old = service
        .change_password(
            user_id,
            ChangePasswordRequest { old_password: "wrongpass".to_string(), new_password: "newpassword123".to_string() },
        )
        .await;
    assert!(wrong_old.is_err());

    // Change password with correct old password
    service
        .change_password(
            user_id,
            ChangePasswordRequest {
                old_password: "oldpass123".to_string(),
                new_password: "newpassword123".to_string(),
            },
        )
        .await
        .unwrap();

    // Forgot password flow
    service.request_forgot_password_otp("bob@example.com").await.unwrap();
    let forgot_otp: String = cache.get("forgot_otp:bob@example.com").await.unwrap();

    service
        .reset_password(ResetPasswordRequest {
            email: "bob@example.com".to_string(),
            otp: forgot_otp,
            new_password: "finalpassword123".to_string(),
        })
        .await
        .unwrap();

    // Sign in with reset password
    let signin_res = service
        .sign_in(SignInRequest { email: "bob@example.com".to_string(), password: "finalpassword123".to_string() })
        .await;
    assert!(signin_res.is_ok());
}

#[tokio::test]
async fn test_google_oauth_signin() {
    let (service, _, _) = create_test_user_service();

    let google_user = GoogleUserInfo {
        sub: "google_12345".to_string(),
        email: "googleuser@gmail.com".to_string(),
        email_verified: true,
        name: "Google User".to_string(),
        picture: Some("https://example.com/photo.jpg".to_string()),
    };

    let tokens = service.sign_in_with_google(google_user.clone()).await.unwrap();
    assert!(!tokens.access_token.is_empty());

    // Sign in again with same google account should link and succeed
    let tokens2 = service.sign_in_with_google(google_user).await.unwrap();
    assert!(!tokens2.access_token.is_empty());
}
