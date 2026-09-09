use redis::AsyncCommands;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone)]
pub struct RedisRateLimiter {
    manager: redis::aio::ConnectionManager,
}

impl RedisRateLimiter {
    /// Tạo rate limiter trên Redis connection manager dùng chung.
    #[must_use]
    pub const fn new(manager: redis::aio::ConnectionManager) -> Self {
        Self { manager }
    }

    /// Increments key in Redis and sets expiration on initial hit.
    /// Returns `true` if under or equal to `max_requests`, `false` otherwise.
    pub async fn check_rate_limit(&self, key: &str, max_requests: u32, duration: Duration) -> bool {
        let mut conn = self.manager.clone();
        let count_res: Result<i64, redis::RedisError> = conn.incr(key, 1).await;

        match count_res {
            Ok(current) => {
                if current == 1 {
                    let duration_secs = duration.as_secs().max(1).cast_signed();
                    let _: Result<(), redis::RedisError> = conn.expire(key, duration_secs).await;
                }
                current <= i64::from(max_requests)
            }
            Err(e) => {
                tracing::warn!("Redis rate limiter error: {e}");
                true // Fail-open if redis temporary connection error
            }
        }
    }

    /// Rate limits specifically by Client IP (used for public endpoints like OTP, login)
    pub async fn check_ip_limit(&self, action: &str, ip: &str, max_requests: u32, duration: Duration) -> bool {
        let key = format!("ratelimit:{action}:ip:{ip}");
        self.check_rate_limit(&key, max_requests, duration).await
    }

    /// Rate limits specifically by Authenticated User ID (prevents rotating IPs with proxy/VPN)
    pub async fn check_user_limit(&self, action: &str, user_id: Uuid, max_requests: u32, duration: Duration) -> bool {
        let key = format!("ratelimit:{action}:user:{user_id}");
        self.check_rate_limit(&key, max_requests, duration).await
    }

    /// Mixed / Hybrid Rate Limiting:
    /// Checks BOTH User ID limit (per-account budget) AND IP limit (per-network budget) concurrently.
    /// Returns `true` only if both checks pass.
    pub async fn check_mixed_limit(
        &self,
        action: &str,
        user_id: Uuid,
        ip: &str,
        user_max: u32,
        ip_max: u32,
        duration: Duration,
    ) -> bool {
        let (user_ok, ip_ok) = tokio::join!(
            self.check_user_limit(action, user_id, user_max, duration),
            self.check_ip_limit(action, ip, ip_max, duration)
        );

        user_ok && ip_ok
    }
}

/// Lấy IP client (ưu tiên `X-Forwarded-For`), fallback loopback.
#[must_use]
pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map_or_else(|| "127.0.0.1".to_string(), |s| s.trim().to_string())
}
