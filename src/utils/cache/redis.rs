//! Backend Redis production — mọi op session/OTP chạy bằng Lua (1 round trip, atomic).

use std::sync::LazyLock;

use async_trait::async_trait;
use redis::AsyncCommands;

use super::{
    error::CacheError,
    store::CacheStore,
    types::{NewRefreshSession, OtpCheckOutcome, RefreshRotation, RevokeRefreshSession, RotationOutcome},
};
use crate::config::constants::REFRESH_TOKEN_KEY_PREFIX;

/// Gom warn + wrap lỗi Redis (tránh lặp closure ở mọi op).
fn redis_error(op: &'static str, err: redis::RedisError) -> CacheError {
    tracing::warn!("Redis {op} failed: {err}");
    CacheError::Redis(err)
}

/// Xoay refresh + cập nhật sessions JSON trong 1 round trip (`cjson` có sẵn trong Redis Lua).
static ROTATE_REFRESH_SCRIPT: LazyLock<redis::Script> = LazyLock::new(|| {
    redis::Script::new(
        r"
        local cur = redis.call('GET', KEYS[1])
        if not cur or cur ~= ARGV[1] then
            return 0
        end
        redis.call('DEL', KEYS[1])
        redis.call('SET', KEYS[2], ARGV[1], 'EX', ARGV[4])
        local raw = redis.call('GET', KEYS[3])
        local sessions = {}
        if raw then
            local ok, decoded = pcall(cjson.decode, raw)
            if ok and type(decoded) == 'table' then
                sessions = decoded
            end
        end
        local updated = {}
        for _, v in ipairs(sessions) do
            if v ~= ARGV[2] then
                updated[#updated + 1] = v
            end
        end
        updated[#updated + 1] = ARGV[3]
        redis.call('SET', KEYS[3], cjson.encode(updated), 'EX', ARGV[4])
        return 1
        ",
    )
});

/// Tạo session mới + đuổi cũ nhất khi đầy trong 1 round trip.
static CREATE_SESSION_SCRIPT: LazyLock<redis::Script> = LazyLock::new(|| {
    redis::Script::new(
        r"
        local raw = redis.call('GET', KEYS[2])
        local sessions = {}
        if raw then
            local ok, decoded = pcall(cjson.decode, raw)
            if ok and type(decoded) == 'table' then
                sessions = decoded
            end
        end
        sessions[#sessions + 1] = ARGV[2]
        local max_n = tonumber(ARGV[4])
        while #sessions > max_n do
            local evicted = table.remove(sessions, 1)
            redis.call('DEL', ARGV[5] .. evicted)
        end
        redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[3])
        redis.call('SET', KEYS[2], cjson.encode(sessions), 'EX', ARGV[3])
        return #sessions
        ",
    )
});

/// Thu hồi 1 session (sign-out đơn session), giữ TTL còn lại của sessions.
static REVOKE_SESSION_SCRIPT: LazyLock<redis::Script> = LazyLock::new(|| {
    redis::Script::new(
        r"
        redis.call('DEL', KEYS[1])
        local raw = redis.call('GET', KEYS[2])
        if not raw then
            return 0
        end
        local ok, decoded = pcall(cjson.decode, raw)
        if not ok or type(decoded) ~= 'table' then
            redis.call('DEL', KEYS[2])
            return 0
        end
        local updated = {}
        for _, v in ipairs(decoded) do
            if v ~= ARGV[1] then
                updated[#updated + 1] = v
            end
        end
        if #updated == 0 then
            redis.call('DEL', KEYS[2])
        else
            redis.call('SET', KEYS[2], cjson.encode(updated), 'KEEPTTL')
        end
        return 1
        ",
    )
});

/// Đối chiếu OTP: sai tăng counter nhưng `KEEPTTL` (không trượt TTL).
/// Trả `1` khớp, `0` sai còn lượt, `-1` thiếu/corrupt, `-2` vừa quá số lần (đã xóa).
static OTP_CHECK_SCRIPT: LazyLock<redis::Script> = LazyLock::new(|| {
    redis::Script::new(
        r"
        local raw = redis.call('GET', KEYS[1])
        if not raw then
            return -1
        end
        local ok, entry = pcall(cjson.decode, raw)
        if not ok or type(entry) ~= 'table' then
            redis.call('DEL', KEYS[1])
            return -1
        end
        if entry.code == ARGV[1] then
            return 1
        end
        local attempts = tonumber(entry.attempts) or 0
        attempts = attempts + 1
        if attempts >= tonumber(ARGV[2]) then
            redis.call('DEL', KEYS[1])
            return -2
        end
        entry.attempts = attempts
        redis.call('SET', KEYS[1], cjson.encode(entry), 'KEEPTTL')
        return 0
        ",
    )
});

/// Cache production trên Redis dùng chung (connection manager clone nhẹ).
#[derive(Clone)]
pub struct RedisCache {
    manager: redis::aio::ConnectionManager,
}

impl RedisCache {
    /// Bọc Redis connection manager dùng chung thành cache.
    ///
    /// `const` vì chỉ di chuyển ownership, không có I/O.
    #[must_use]
    pub const fn new(manager: redis::aio::ConnectionManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl CacheStore for RedisCache {
    async fn get_raw(&self, key: &str) -> Result<Option<String>, CacheError> {
        let mut conn = self.manager.clone();
        conn.get(key).await.map_err(|err| redis_error("GET", err))
    }

    async fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) -> Result<(), CacheError> {
        let mut conn = self.manager.clone();
        conn.set_ex(key, value, expiration_secs).await.map_err(|err| redis_error("SETEX", err))
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut conn = self.manager.clone();
        conn.del(key).await.map_err(|err| redis_error("DEL", err))
    }

    async fn delete_many(&self, keys: &[String]) -> Result<(), CacheError> {
        if keys.is_empty() {
            return Ok(());
        }
        let mut conn = self.manager.clone();
        let mut pipe = redis::pipe();
        for key in keys {
            pipe.cmd("DEL").arg(key);
        }
        pipe.exec_async(&mut conn).await.map_err(|err| redis_error("pipelined DEL", err))
    }

    async fn rotate_refresh_token(&self, rotation: &RefreshRotation<'_>) -> Result<RotationOutcome, CacheError> {
        let mut conn = self.manager.clone();
        let rotated: i64 = ROTATE_REFRESH_SCRIPT
            .key(rotation.old_key)
            .key(rotation.new_key)
            .key(rotation.session_key)
            .arg(rotation.subject)
            .arg(rotation.old_jti)
            .arg(rotation.new_jti)
            .arg(rotation.ttl_secs)
            .invoke_async(&mut conn)
            .await
            .map_err(|err| redis_error("refresh rotation", err))?;
        Ok(if rotated == 1 { RotationOutcome::Rotated } else { RotationOutcome::Stale })
    }

    async fn create_refresh_session(&self, session: &NewRefreshSession<'_>) -> Result<(), CacheError> {
        let mut conn = self.manager.clone();
        let _: i64 = CREATE_SESSION_SCRIPT
            .key(session.key)
            .key(session.session_key)
            .arg(session.subject)
            .arg(session.jti)
            .arg(session.ttl_secs)
            .arg(session.max_sessions)
            .arg(REFRESH_TOKEN_KEY_PREFIX)
            .invoke_async(&mut conn)
            .await
            .map_err(|err| redis_error("session creation", err))?;
        Ok(())
    }

    async fn revoke_refresh_session(&self, revoke: &RevokeRefreshSession<'_>) -> Result<(), CacheError> {
        let mut conn = self.manager.clone();
        let _: i64 = REVOKE_SESSION_SCRIPT
            .key(revoke.key)
            .key(revoke.session_key)
            .arg(revoke.jti)
            .invoke_async(&mut conn)
            .await
            .map_err(|err| redis_error("session revocation", err))?;
        Ok(())
    }

    async fn check_otp_code(
        &self,
        key: &str,
        candidate: &str,
        max_attempts: u32,
    ) -> Result<OtpCheckOutcome, CacheError> {
        let mut conn = self.manager.clone();
        let verdict: i64 = OTP_CHECK_SCRIPT
            .key(key)
            .arg(candidate)
            .arg(max_attempts)
            .invoke_async(&mut conn)
            .await
            .map_err(|err| redis_error("OTP check", err))?;
        Ok(match verdict {
            1 => OtpCheckOutcome::Match,
            0 => OtpCheckOutcome::Mismatch,
            -2 => OtpCheckOutcome::Locked,
            _ => OtpCheckOutcome::Missing,
        })
    }
}
