//! Trait truy xuất cache — `#[async_trait]` đồng bộ với repository traits.
//!
//! Mọi method đều `async` vì backend Redis là I/O mạng thật; backend `Memory`
//! giữ cùng chữ ký để dispatch tĩnh qua enum [`super::Cache`] mà không cần `dyn`.

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};

use super::{
    error::CacheError,
    types::{NewRefreshSession, OtpCheckOutcome, RefreshRotation, RevokeRefreshSession, RotationOutcome},
};

#[async_trait]
pub trait CacheStore: Send + Sync {
    /// Đọc string thô theo key.
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller critical phải fail-closed (503).
    async fn get_raw(&self, key: &str) -> Result<Option<String>, CacheError>;
    /// Ghi string thô kèm TTL (giây).
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller critical phải fail-closed (503).
    async fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) -> Result<(), CacheError>;
    /// Xóa key (idempotent — key lạ vẫn `Ok`).
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller critical phải fail-closed (503).
    async fn delete(&self, key: &str) -> Result<(), CacheError>;
    /// Xóa nhiều key trong 1 batch (Redis pipeline; backend khác loop).
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller critical phải fail-closed (503).
    async fn delete_many(&self, keys: &[String]) -> Result<(), CacheError> {
        for key in keys {
            self.delete(key).await?;
        }
        Ok(())
    }

    /// Xoay refresh token nguyên tử: check subject + xóa key cũ + ghi key mới +
    /// cập nhật sessions trong đúng 1 round trip (Redis Lua; Memory sync toàn phần).
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller phải fail-closed (503),
    /// KHÔNG được coi như [`RotationOutcome::Stale`] (nhầm lẫn hai trạng thái này
    /// biến sự cố hạ tầng thành logout hàng loạt).
    async fn rotate_refresh_token(&self, rotation: &RefreshRotation<'_>) -> Result<RotationOutcome, CacheError>;

    /// Tạo session refresh mới, đuổi session cũ nhất khi vượt `max_sessions`.
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller phải fail-closed (503):
    /// đã ký token nhưng chưa lưu session thì token đó vô dụng, không được trả về client.
    async fn create_refresh_session(&self, session: &NewRefreshSession<'_>) -> Result<(), CacheError>;

    /// Thu hồi một session (sign-out đơn session), giữ nguyên TTL còn lại của sessions.
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller phải fail-closed (503),
    /// không được báo "đã đăng xuất" giả.
    async fn revoke_refresh_session(&self, revoke: &RevokeRefreshSession<'_>) -> Result<(), CacheError>;

    /// Đối chiếu OTP nguyên tử: lần sai tăng counter NHƯNG giữ nguyên TTL
    /// (Redis `KEEPTTL`, Memory không chạm `expires_at`) — chống kéo dài cửa sổ
    /// brute-force bằng cách spam sai.
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller phải fail-closed (503),
    /// không được coi như sai OTP (sẽ khóa user oan khi hạ tầng chập chờn).
    async fn check_otp_code(
        &self,
        key: &str,
        candidate: &str,
        max_attempts: u32,
    ) -> Result<OtpCheckOutcome, CacheError>;
}

#[async_trait]
pub trait CacheStoreExt: CacheStore {
    /// Đọc + deserialize JSON theo key. Entry corrupt được coi như miss (đã xóa).
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller critical phải fail-closed (503).
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>, CacheError> {
        let raw = self.get_raw(key).await?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        match serde_json::from_str(&raw) {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                tracing::warn!("Dropping corrupt cache entry (treated as miss): {err}");
                self.delete(key).await?;
                Ok(None)
            }
        }
    }

    /// Serialize + ghi JSON theo key kèm TTL (giây).
    ///
    /// # Errors
    ///
    /// Trả [`CacheError::Redis`] khi Redis lỗi — caller critical phải fail-closed (503).
    async fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        expiration_secs: u64,
    ) -> Result<(), CacheError> {
        match serde_json::to_string(value) {
            Ok(serialized) => self.set_raw(key, &serialized, expiration_secs).await,
            Err(err) => {
                tracing::warn!("Skipping cache write (unserializable value): {err}");
                Ok(())
            }
        }
    }

    /// Đọc best-effort cho cache không critical (profile, summary): Redis lỗi thì
    /// miss và warn, caller tự fallback DB — KHÔNG bao giờ fail request.
    async fn get_best_effort<T: DeserializeOwned + Send>(&self, key: &str) -> Option<T> {
        match self.get::<T>(key).await {
            Ok(hit) => hit,
            Err(err) => {
                tracing::warn!("Best-effort cache read missed: {err}");
                None
            }
        }
    }

    /// Ghi best-effort cho cache không critical: Redis lỗi thì bỏ qua và warn.
    async fn set_best_effort<T: Serialize + Send + Sync>(&self, key: &str, value: &T, expiration_secs: u64) {
        if let Err(err) = self.set(key, value, expiration_secs).await {
            tracing::warn!("Best-effort cache write skipped: {err}");
        }
    }

    /// Xóa best-effort cho invalidate không critical: Redis lỗi thì bỏ qua và warn.
    async fn delete_best_effort(&self, key: &str) {
        if let Err(err) = self.delete(key).await {
            tracing::warn!("Best-effort cache delete skipped: {err}");
        }
    }

    /// Xóa nhiều key best-effort cho invalidate không critical.
    async fn delete_many_best_effort(&self, keys: &[String]) {
        if let Err(err) = self.delete_many(keys).await {
            tracing::warn!("Best-effort cache batch delete skipped: {err}");
        }
    }
}

impl<T: CacheStore + ?Sized> CacheStoreExt for T {}
