use std::sync::Arc;
use uuid::Uuid;

use crate::common::{MockUserRepository, test_argon2, test_cache, test_jwt_config, test_pool};
use dsa::{
    config::constants::MAX_OTP_ATTEMPTS,
    domain::users::{
        entity::NewUserEntity,
        repository::UserRepository,
        request::{ChangePasswordRequest, OtpPurpose, ResetPasswordRequest, SignInRequest, SignUpRequest},
        service::UserService,
    },
    utils::{
        cache::CacheStoreExt,
        email::{EmailConfig, Mailer},
        hash::hash_password,
        i18n::Lang,
        oauth::GoogleUserInfo,
        otp::OtpEntry,
    },
};

fn create_test_user_service() -> (UserService<MockUserRepository>, Arc<dsa::utils::cache::Cache>, MockUserRepository) {
    let repo = MockUserRepository::default();
    let cache = test_cache();
    let argon2 = test_argon2();
    let jwt_config = test_jwt_config();

    let service = UserService::new(
        repo.clone(),
        cache.clone(),
        argon2,
        jwt_config,
        Mailer::new(8, &EmailConfig::disabled()),
        test_pool(),
    );
    (service, cache, repo)
}

#[tokio::test]
async fn test_user_signup_and_signin_flow() {
    let (service, cache, _) = create_test_user_service();

    // 1. Request OTP
    service.request_otp("alice@example.com", Lang::Vi).await.unwrap();

    // Check OTP in cache
    let entry: OtpEntry = cache.get("otp:alice@example.com").await.unwrap();
    let otp = entry.code.clone();

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
    service.request_forgot_password_otp("bob@example.com", Lang::Vi).await.unwrap();
    let forgot_entry: OtpEntry = cache.get("forgot_otp:bob@example.com").await.unwrap();
    let forgot_otp = forgot_entry.code;

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

#[tokio::test]
async fn test_verify_otp_success_failure_and_purpose_isolation() {
    let (service, cache, _) = create_test_user_service();
    service.request_otp("verify@example.com", Lang::Vi).await.unwrap();
    let entry: OtpEntry = cache.get("otp:verify@example.com").await.unwrap();

    // Sai mã thì lỗi nhưng chưa hủy.
    assert!(service.verify_otp("verify@example.com", "000000", OtpPurpose::Signup).await.is_err());
    // Mã signup không dùng được cho flow reset (key riêng).
    assert!(service.verify_otp("verify@example.com", &entry.code, OtpPurpose::Reset).await.is_err());
    // Mã đúng, đúng flow thì pass và KHÔNG tiêu thụ (signup sau vẫn được).
    assert!(service.verify_otp("verify@example.com", &entry.code, OtpPurpose::Signup).await.is_ok());
    assert!(
        service
            .sign_up(SignUpRequest {
                email: "verify@example.com".to_string(),
                full_name: "Verify User".to_string(),
                password: "password123".to_string(),
                otp: entry.code,
            })
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_otp_invalidated_after_max_attempts() {
    let (service, cache, _) = create_test_user_service();
    service.request_otp("cap@example.com", Lang::Vi).await.unwrap();
    let entry: OtpEntry = cache.get("otp:cap@example.com").await.unwrap();

    for _ in 0..MAX_OTP_ATTEMPTS {
        assert!(service.verify_otp("cap@example.com", "000000", OtpPurpose::Signup).await.is_err());
    }

    // Mã đúng cũng rớt vì entry đã bị xóa sau đủ số lần sai.
    assert!(service.verify_otp("cap@example.com", &entry.code, OtpPurpose::Signup).await.is_err());
    assert!(cache.get::<OtpEntry>("otp:cap@example.com").await.is_none());
}

#[tokio::test]
async fn test_signup_attempts_share_counter_with_verify() {
    let (service, cache, _) = create_test_user_service();
    service.request_otp("shared@example.com", Lang::Vi).await.unwrap();
    let entry: OtpEntry = cache.get("otp:shared@example.com").await.unwrap();

    // Sai 2 lần qua signup, đúng qua verify vẫn pass (chung 1 counter, chưa tới hạn).
    let bad = SignUpRequest {
        email: "shared@example.com".to_string(),
        full_name: "Shared User".to_string(),
        password: "password123".to_string(),
        otp: "000000".to_string(),
    };
    assert!(service.sign_up(bad.clone()).await.is_err());
    assert!(service.sign_up(bad).await.is_err());
    assert!(service.verify_otp("shared@example.com", &entry.code, OtpPurpose::Signup).await.is_ok());
}

async fn seed_user_for_reset(repo: &MockUserRepository, email: &str) {
    let hash = hash_password(&test_argon2(), "oldpass123".to_string()).await.unwrap();
    repo.create(
        &test_pool(),
        &NewUserEntity {
            id: Uuid::now_v7(),
            full_name: "Reset User".to_string(),
            email: email.to_string(),
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
}

#[tokio::test]
async fn test_verify_otp_reset_purpose_success_and_isolation() {
    let (service, cache, repo) = create_test_user_service();
    seed_user_for_reset(&repo, "reset-verify@example.com").await;

    service.request_forgot_password_otp("reset-verify@example.com", Lang::Vi).await.unwrap();
    let entry: OtpEntry = cache.get("forgot_otp:reset-verify@example.com").await.unwrap();

    // Sai mã thì lỗi nhưng chưa hủy.
    assert!(service.verify_otp("reset-verify@example.com", "000000", OtpPurpose::Reset).await.is_err());
    // Mã reset không dùng được cho flow signup (key riêng).
    assert!(service.verify_otp("reset-verify@example.com", &entry.code, OtpPurpose::Signup).await.is_err());
    // Đúng mã đúng flow thì pass và KHÔNG tiêu thụ (reset sau vẫn được).
    assert!(service.verify_otp("reset-verify@example.com", &entry.code, OtpPurpose::Reset).await.is_ok());
    assert!(
        service
            .reset_password(ResetPasswordRequest {
                email: "reset-verify@example.com".to_string(),
                otp: entry.code,
                new_password: "newpassword123".to_string(),
            })
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_reset_otp_invalidated_after_max_attempts() {
    let (service, cache, repo) = create_test_user_service();
    seed_user_for_reset(&repo, "reset-cap@example.com").await;

    service.request_forgot_password_otp("reset-cap@example.com", Lang::Vi).await.unwrap();
    let entry: OtpEntry = cache.get("forgot_otp:reset-cap@example.com").await.unwrap();

    for _ in 0..MAX_OTP_ATTEMPTS {
        assert!(service.verify_otp("reset-cap@example.com", "000000", OtpPurpose::Reset).await.is_err());
    }

    // Mã đúng cũng rớt vì entry đã bị xóa sau đủ số lần sai.
    assert!(service.verify_otp("reset-cap@example.com", &entry.code, OtpPurpose::Reset).await.is_err());
    assert!(cache.get::<OtpEntry>("forgot_otp:reset-cap@example.com").await.is_none());
}
