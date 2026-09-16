use dsa::utils::{
    cache::{
        Cache, CacheStore, MemoryCache, NewRefreshSession, OtpCheckOutcome, RefreshRotation, RevokeRefreshSession,
        RotationOutcome,
    },
    otp::{consume, reset_key, signup_key, store},
};

#[tokio::test]
async fn test_delete_many_removes_all_keys() {
    let cache = Cache::Memory(MemoryCache::new());
    cache.set_raw("k1", "v1", 60).await.unwrap();
    cache.set_raw("k2", "v2", 60).await.unwrap();
    cache.set_raw("keep", "v", 60).await.unwrap();

    cache.delete_many(&["k1".to_string(), "k2".to_string()]).await.unwrap();

    assert!(cache.get_raw("k1").await.unwrap().is_none());
    assert!(cache.get_raw("k2").await.unwrap().is_none());
    assert_eq!(cache.get_raw("keep").await.unwrap().as_deref(), Some("v"));

    // Rỗng và key lạ là no-op, không lỗi.
    cache.delete_many(&[]).await.unwrap();
    cache.delete_many(&["nope".to_string()]).await.unwrap();
}

#[tokio::test]
async fn test_rotate_success_swaps_keys_and_updates_sessions() {
    let cache = Cache::Memory(MemoryCache::new());
    cache.set_raw("refreshToken:old", "user-1", 60).await.unwrap();
    cache.set_raw("sessions:user-1", r#"["old","other"]"#, 60).await.unwrap();

    let rotation = RefreshRotation {
        old_key: "refreshToken:old",
        new_key: "refreshToken:new",
        session_key: "sessions:user-1",
        subject: "user-1",
        old_jti: "old",
        new_jti: "new",
        ttl_secs: 60,
    };
    assert_eq!(cache.rotate_refresh_token(&rotation).await.unwrap(), RotationOutcome::Rotated);

    assert!(cache.get_raw("refreshToken:old").await.unwrap().is_none());
    assert_eq!(cache.get_raw("refreshToken:new").await.unwrap().as_deref(), Some("user-1"));
    assert_eq!(cache.get_raw("sessions:user-1").await.unwrap().as_deref(), Some(r#"["other","new"]"#));
}

#[tokio::test]
async fn test_rotate_stale_when_key_missing_or_subject_mismatch() {
    let cache = Cache::Memory(MemoryCache::new());

    // Key thiếu hẳn → Stale (caller sẽ revoke family + báo InvalidSession).
    let missing = RefreshRotation {
        old_key: "refreshToken:ghost",
        new_key: "refreshToken:new",
        session_key: "sessions:user-1",
        subject: "user-1",
        old_jti: "ghost",
        new_jti: "new",
        ttl_secs: 60,
    };
    assert_eq!(cache.rotate_refresh_token(&missing).await.unwrap(), RotationOutcome::Stale);
    // Không được tạo key mới khi xoay thất bại.
    assert!(cache.get_raw("refreshToken:new").await.unwrap().is_none());

    // Subject khác → Stale (chống tráo token giữa các user).
    cache.set_raw("refreshToken:old", "user-2", 60).await.unwrap();
    let mismatch = RefreshRotation {
        old_key: "refreshToken:old",
        new_key: "refreshToken:new2",
        session_key: "sessions:user-1",
        subject: "user-1",
        old_jti: "old",
        new_jti: "new2",
        ttl_secs: 60,
    };
    assert_eq!(cache.rotate_refresh_token(&mismatch).await.unwrap(), RotationOutcome::Stale);
    assert_eq!(cache.get_raw("refreshToken:old").await.unwrap().as_deref(), Some("user-2"));
}

#[tokio::test]
async fn test_create_session_evicts_oldest_when_full() {
    let cache = Cache::Memory(MemoryCache::new());
    let key_a = "refreshToken:a".to_string();
    let key_b = "refreshToken:b".to_string();
    for (key, jti) in [(&key_a, "a"), (&key_b, "b")] {
        let session = NewRefreshSession {
            key,
            session_key: "sessions:user-1",
            subject: "user-1",
            jti,
            ttl_secs: 60,
            max_sessions: 2,
        };
        cache.create_refresh_session(&session).await.unwrap();
    }

    let third = NewRefreshSession {
        key: "refreshToken:c",
        session_key: "sessions:user-1",
        subject: "user-1",
        jti: "c",
        ttl_secs: 60,
        max_sessions: 2,
    };
    cache.create_refresh_session(&third).await.unwrap();

    assert!(cache.get_raw("refreshToken:a").await.unwrap().is_none());
    assert_eq!(cache.get_raw("refreshToken:b").await.unwrap().as_deref(), Some("user-1"));
    assert_eq!(cache.get_raw("refreshToken:c").await.unwrap().as_deref(), Some("user-1"));
    assert_eq!(cache.get_raw("sessions:user-1").await.unwrap().as_deref(), Some(r#"["b","c"]"#));
}

#[tokio::test]
async fn test_revoke_single_session_keeps_siblings() {
    let cache = Cache::Memory(MemoryCache::new());
    cache.set_raw("refreshToken:old", "user-1", 60).await.unwrap();
    cache.set_raw("refreshToken:other", "user-1", 60).await.unwrap();
    cache.set_raw("sessions:user-1", r#"["old","other"]"#, 60).await.unwrap();

    let revoke =
        RevokeRefreshSession { key: "refreshToken:old", session_key: "sessions:user-1", jti: "old", ttl_secs: 60 };
    cache.revoke_refresh_session(&revoke).await.unwrap();

    assert!(cache.get_raw("refreshToken:old").await.unwrap().is_none());
    assert_eq!(cache.get_raw("refreshToken:other").await.unwrap().as_deref(), Some("user-1"));
    assert_eq!(cache.get_raw("sessions:user-1").await.unwrap().as_deref(), Some(r#"["other"]"#));

    // Thu hồi session cuối thì dọn luôn sessions key.
    let last =
        RevokeRefreshSession { key: "refreshToken:other", session_key: "sessions:user-1", jti: "other", ttl_secs: 60 };
    cache.revoke_refresh_session(&last).await.unwrap();
    assert!(cache.get_raw("sessions:user-1").await.unwrap().is_none());
}

#[tokio::test]
async fn test_otp_check_match_mismatch_locked_missing() {
    let cache = Cache::Memory(MemoryCache::new());
    store(&cache, &signup_key("otp@example.com"), "123456".to_string()).await.unwrap();

    assert_eq!(
        cache.check_otp_code(&signup_key("otp@example.com"), "123456", 5).await.unwrap(),
        OtpCheckOutcome::Match
    );
    // Đúng không tiêu thụ: check lại vẫn Match.
    assert_eq!(
        cache.check_otp_code(&signup_key("otp@example.com"), "123456", 5).await.unwrap(),
        OtpCheckOutcome::Match
    );

    assert_eq!(
        cache.check_otp_code(&signup_key("otp@example.com"), "000000", 2).await.unwrap(),
        OtpCheckOutcome::Mismatch
    );
    // Quá số lần (max=2) → Locked và entry bị xóa.
    assert_eq!(
        cache.check_otp_code(&signup_key("otp@example.com"), "000000", 2).await.unwrap(),
        OtpCheckOutcome::Locked
    );
    assert_eq!(
        cache.check_otp_code(&signup_key("otp@example.com"), "123456", 2).await.unwrap(),
        OtpCheckOutcome::Missing
    );

    // Key chưa từng tồn tại → Missing.
    assert_eq!(
        cache.check_otp_code(&reset_key("ghost@example.com"), "123456", 5).await.unwrap(),
        OtpCheckOutcome::Missing
    );
}

#[tokio::test]
async fn test_otp_entry_expires_and_consume_removes() {
    let cache = Cache::Memory(MemoryCache::new());
    cache.set_raw("otp:short@example.com", r#"{"code":"999999","attempts":0}"#, 0).await.unwrap();
    // TTL 0 hết hạn ngay (Instant::now() < expires_at sai).
    assert_eq!(cache.check_otp_code("otp:short@example.com", "999999", 5).await.unwrap(), OtpCheckOutcome::Missing);

    store(&cache, &signup_key("gone@example.com"), "111111".to_string()).await.unwrap();
    consume(&cache, &signup_key("gone@example.com")).await.unwrap();
    assert_eq!(
        cache.check_otp_code(&signup_key("gone@example.com"), "111111", 5).await.unwrap(),
        OtpCheckOutcome::Missing
    );
}

#[tokio::test]
async fn test_otp_failed_check_keeps_original_ttl() {
    use std::time::Duration;

    // TTL 2s: sai ở t=1.2s. Nếu TTL trượt (reset về 2s) thì entry sống tới t=3.2s;
    // giữ nguyên TTL gốc thì entry chết ở t=2s → check đúng ở t=2.4s phải Missing.
    let cache = Cache::Memory(MemoryCache::new());
    let key = signup_key("ttl@example.com");
    cache.set_raw(&key, r#"{"code":"123456","attempts":0}"#, 2).await.unwrap();

    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(cache.check_otp_code(&key, "000000", 5).await.unwrap(), OtpCheckOutcome::Mismatch);

    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(cache.check_otp_code(&key, "123456", 5).await.unwrap(), OtpCheckOutcome::Missing);
}
