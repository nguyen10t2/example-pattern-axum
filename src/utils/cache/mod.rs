//! Cache backend cho session/OTP/read-through.
//!
//! Layout theo trách nhiệm, mỗi file một việc:
//!
//! - [`error`] — [`CacheError`]: lỗi transport duy nhất, caller fail-closed (503).
//! - [`types`] — outcome enum + param struct cho op nguyên tử (tránh tráo arg `&str`).
//! - [`keys`] — free function dựng key (pure, sync), single source of truth.
//! - [`store`] — [`CacheStore`] + [`CacheStoreExt`] (`#[async_trait]`, đồng bộ repo traits).
//! - [`redis`] — backend production (Lua 1 round trip).
//! - [`memory`] — backend in-memory cho test (mirror semantics Redis).
//!
//! Chính sách lỗi: op critical (session/OTP/revoke) trả `Err` để fail-closed;
//! read-through không critical dùng `*_best_effort` để fallback DB.

mod error;
mod keys;
mod memory;
mod redis;
mod store;
mod types;

pub use error::CacheError;
pub use keys::{
    group_members_key, group_summary_key, refresh_token_key, reset_otp_key, session_list_key, signup_otp_key,
    user_profile_key,
};
pub use memory::MemoryCache;
pub use redis::RedisCache;
pub use store::{CacheStore, CacheStoreExt};
pub use types::{NewRefreshSession, OtpCheckOutcome, RefreshRotation, RevokeRefreshSession, RotationOutcome};

// Cache TTLs live in [`crate::config::constants`]; re-exported here so
// existing `utils::cache::{...}` imports keep working.
pub use crate::config::constants::{CACHE_EXPIRATION, OTP_EXPIRATION, REFRESH_TOKEN_EXPIRATION};

use async_trait::async_trait;

/// Tập backend cache của app, dispatch tĩnh thay vì `dyn`.
///
/// Chỉ có `Redis` (production) và `Memory` (test) nên trait object chỉ thêm vtable
/// mà không có lợi gì — thêm backend mới thì thêm variant ở đây.
#[derive(Clone)]
pub enum Cache {
    Redis(RedisCache),
    Memory(MemoryCache),
}

#[async_trait]
impl CacheStore for Cache {
    async fn get_raw(&self, key: &str) -> Result<Option<String>, CacheError> {
        match self {
            Self::Redis(inner) => inner.get_raw(key).await,
            Self::Memory(inner) => inner.get_raw(key).await,
        }
    }

    async fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) -> Result<(), CacheError> {
        match self {
            Self::Redis(inner) => inner.set_raw(key, value, expiration_secs).await,
            Self::Memory(inner) => inner.set_raw(key, value, expiration_secs).await,
        }
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        match self {
            Self::Redis(inner) => inner.delete(key).await,
            Self::Memory(inner) => inner.delete(key).await,
        }
    }

    async fn rotate_refresh_token(&self, rotation: &RefreshRotation<'_>) -> Result<RotationOutcome, CacheError> {
        match self {
            Self::Redis(inner) => inner.rotate_refresh_token(rotation).await,
            Self::Memory(inner) => inner.rotate_refresh_token(rotation).await,
        }
    }

    async fn create_refresh_session(&self, session: &NewRefreshSession<'_>) -> Result<(), CacheError> {
        match self {
            Self::Redis(inner) => inner.create_refresh_session(session).await,
            Self::Memory(inner) => inner.create_refresh_session(session).await,
        }
    }

    async fn revoke_refresh_session(&self, revoke: &RevokeRefreshSession<'_>) -> Result<(), CacheError> {
        match self {
            Self::Redis(inner) => inner.revoke_refresh_session(revoke).await,
            Self::Memory(inner) => inner.revoke_refresh_session(revoke).await,
        }
    }

    async fn check_otp_code(
        &self,
        key: &str,
        candidate: &str,
        max_attempts: u32,
    ) -> Result<OtpCheckOutcome, CacheError> {
        match self {
            Self::Redis(inner) => inner.check_otp_code(key, candidate, max_attempts).await,
            Self::Memory(inner) => inner.check_otp_code(key, candidate, max_attempts).await,
        }
    }
}
