//! Backend in-memory cho test — mirror semantics của Redis (TTL, sessions JSON,
//! `KEEPTTL` khi check OTP sai) nhưng sync toàn phần: không có `.await` giữa các
//! bước nên bất biến trong runtime single-thread (`#[tokio::test]` mặc định).
//! Redis Lua mới là atomic thật đa tiến trình.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use dashmap::DashMap;

use super::{
    error::CacheError,
    store::CacheStore,
    types::{NewRefreshSession, OtpCheckOutcome, RefreshRotation, RevokeRefreshSession, RotationOutcome},
};
use crate::{config::constants::REFRESH_TOKEN_KEY_PREFIX, utils::otp::OtpEntry};

/// Cache in-memory backed by `DashMap` cho test.
#[derive(Clone, Default)]
pub struct MemoryCache {
    store: Arc<DashMap<String, (String, Instant)>>,
}

impl MemoryCache {
    /// Cache in-memory cho test (hết hạn theo `Instant`).
    #[must_use]
    pub fn new() -> Self {
        Self { store: Arc::new(DashMap::new()) }
    }

    /// Đọc entry còn hạn (dọn entry hết hạn luôn). Sync vì chỉ chạm `DashMap` trong RAM.
    fn read_live(&self, key: &str) -> Option<String> {
        match self.store.entry(key.to_string()) {
            dashmap::mapref::entry::Entry::Occupied(entry) => {
                if Instant::now() < entry.get().1 {
                    Some(entry.get().0.clone())
                } else {
                    entry.remove();
                    None
                }
            }
            dashmap::mapref::entry::Entry::Vacant(_) => None,
        }
    }

    /// Parse danh sách sessions, corrupt thì coi như rỗng (giống Lua `pcall`).
    fn parse_sessions(raw: Option<String>) -> Vec<String> {
        raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    /// Ghi đè value nhưng giữ nguyên TTL còn lại (tương đương Redis `KEEPTTL`).
    ///
    /// Caller phải đảm bảo key còn sống; sync vì chỉ chạm `DashMap` trong RAM.
    fn overwrite_keep_ttl(&self, key: &str, value: String) {
        if let dashmap::mapref::entry::Entry::Occupied(entry) = self.store.entry(key.to_string()) {
            let expires_at = entry.get().1;
            drop(entry); // Nhả shard guard trước khi insert (cùng shard → deadlock nếu giữ).
            self.store.insert(key.to_string(), (value, expires_at));
        }
    }
}

#[async_trait]
impl CacheStore for MemoryCache {
    async fn get_raw(&self, key: &str) -> Result<Option<String>, CacheError> {
        Ok(self.read_live(key))
    }

    async fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) -> Result<(), CacheError> {
        let expires_at = Instant::now() + Duration::from_secs(expiration_secs);
        self.store.insert(key.to_string(), (value.to_string(), expires_at));
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.store.remove(key);
        Ok(())
    }

    async fn rotate_refresh_token(&self, rotation: &RefreshRotation<'_>) -> Result<RotationOutcome, CacheError> {
        let outcome = match self.read_live(rotation.old_key) {
            Some(stored) if stored == rotation.subject => {
                self.store.remove(rotation.old_key);
                let expires_at = Instant::now() + Duration::from_secs(rotation.ttl_secs);
                self.store.insert(rotation.new_key.to_string(), (rotation.subject.to_string(), expires_at));
                let mut sessions = Self::parse_sessions(self.read_live(rotation.session_key));
                sessions.retain(|jti| jti != rotation.old_jti);
                sessions.push(rotation.new_jti.to_string());
                let sessions_expires_at = Instant::now() + Duration::from_secs(rotation.ttl_secs);
                if let Ok(serialized) = serde_json::to_string(&sessions) {
                    self.store.insert(rotation.session_key.to_string(), (serialized, sessions_expires_at));
                }
                RotationOutcome::Rotated
            }
            _ => RotationOutcome::Stale,
        };
        Ok(outcome)
    }

    async fn create_refresh_session(&self, session: &NewRefreshSession<'_>) -> Result<(), CacheError> {
        let mut sessions = Self::parse_sessions(self.read_live(session.session_key));
        sessions.push(session.jti.to_string());
        if sessions.len() > session.max_sessions {
            let drain_count = sessions.len() - session.max_sessions;
            for evicted in sessions.drain(0..drain_count) {
                self.store.remove(format!("{REFRESH_TOKEN_KEY_PREFIX}{evicted}").as_str());
            }
        }
        let expires_at = Instant::now() + Duration::from_secs(session.ttl_secs);
        self.store.insert(session.key.to_string(), (session.subject.to_string(), expires_at));
        if let Ok(serialized) = serde_json::to_string(&sessions) {
            self.store.insert(session.session_key.to_string(), (serialized, expires_at));
        }
        Ok(())
    }

    async fn revoke_refresh_session(&self, revoke: &RevokeRefreshSession<'_>) -> Result<(), CacheError> {
        // Giữ nguyên TTL còn lại của sessions (sửa value tại chỗ, không chạm `expires_at`).
        self.store.remove(revoke.key);
        if let dashmap::mapref::entry::Entry::Occupied(mut entry) = self.store.entry(revoke.session_key.to_string()) {
            if Instant::now() >= entry.get().1 {
                entry.remove();
            } else {
                let mut sessions: Vec<String> = serde_json::from_str(&entry.get().0).unwrap_or_default();
                sessions.retain(|jti| jti != revoke.jti);
                if sessions.is_empty() {
                    entry.remove();
                } else if let Ok(serialized) = serde_json::to_string(&sessions) {
                    entry.get_mut().0 = serialized;
                }
            }
        }
        Ok(())
    }

    async fn check_otp_code(
        &self,
        key: &str,
        candidate: &str,
        max_attempts: u32,
    ) -> Result<OtpCheckOutcome, CacheError> {
        // Giữ nguyên `expires_at` khi sai (không chạm TTL) — tương đương `KEEPTTL`.
        if let dashmap::mapref::entry::Entry::Occupied(entry) = self.store.entry(key.to_string()) {
            if Instant::now() >= entry.get().1 {
                entry.remove();
                return Ok(OtpCheckOutcome::Missing);
            }
            let parsed: Result<OtpEntry, _> = serde_json::from_str(&entry.get().0);
            match parsed {
                Err(_) => {
                    entry.remove();
                    return Ok(OtpCheckOutcome::Missing);
                }
                Ok(otp) if otp.code == candidate => return Ok(OtpCheckOutcome::Match),
                Ok(mut otp) => {
                    otp.attempts += 1;
                    if otp.attempts >= max_attempts {
                        entry.remove();
                        return Ok(OtpCheckOutcome::Locked);
                    }
                    let serialized = serde_json::to_string(&otp).ok();
                    drop(entry);
                    if let Some(serialized) = serialized {
                        self.overwrite_keep_ttl(key, serialized);
                    }
                    return Ok(OtpCheckOutcome::Mismatch);
                }
            }
        }
        Ok(OtpCheckOutcome::Missing)
    }
}
