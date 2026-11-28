use std::{sync::LazyLock, time::Duration};
use uuid::Uuid;

/// `INCR` + `EXPIRE` trong 1 round trip, atomic — hết race rò rỉ key không TTL
/// khi crash giữa 2 lệnh như bản gọi riêng lẻ trước đây.
static INCR_EXPIRE_SCRIPT: LazyLock<redis::Script> = LazyLock::new(|| {
    redis::Script::new(
        r"
        local current = redis.call('INCR', KEYS[1])
        if current == 1 then
            redis.call('EXPIRE', KEYS[1], ARGV[1])
        end
        return current
        ",
    )
});

/// Bản 2 keys cho mixed limit (user + IP) trong đúng 1 round trip.
static INCR_EXPIRE_TWO_SCRIPT: LazyLock<redis::Script> = LazyLock::new(|| {
    redis::Script::new(
        r"
        local first = redis.call('INCR', KEYS[1])
        if first == 1 then
            redis.call('EXPIRE', KEYS[1], ARGV[1])
        end
        local second = redis.call('INCR', KEYS[2])
        if second == 1 then
            redis.call('EXPIRE', KEYS[2], ARGV[1])
        end
        return {first, second}
        ",
    )
});

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

    /// Increments key in Redis and sets expiration on initial hit (1 round trip, atomic).
    /// Returns `true` if under or equal to `max_requests`, `false` otherwise.
    #[tracing::instrument(skip(self))]
    pub async fn check_rate_limit(&self, key: &str, max_requests: u32, duration: Duration) -> bool {
        let mut conn = self.manager.clone();
        let duration_secs = duration.as_secs().max(1).cast_signed();
        let count_res: Result<i64, redis::RedisError> =
            INCR_EXPIRE_SCRIPT.key(key).arg(duration_secs).invoke_async(&mut conn).await;

        match count_res {
            Ok(current) => current <= i64::from(max_requests),
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

    /// Rate limits by email address (anti inbox-bombing cho endpoint xin OTP).
    pub async fn check_email_limit(&self, action: &str, email: &str, max_requests: u32, duration: Duration) -> bool {
        let key = format!("ratelimit:{action}:email:{}", email.to_lowercase());
        self.check_rate_limit(&key, max_requests, duration).await
    }

    /// Rate limits specifically by Authenticated User ID (prevents rotating IPs with proxy/VPN)
    pub async fn check_user_limit(&self, action: &str, user_id: Uuid, max_requests: u32, duration: Duration) -> bool {
        let key = format!("ratelimit:{action}:user:{user_id}");
        self.check_rate_limit(&key, max_requests, duration).await
    }

    /// Mixed / Hybrid Rate Limiting:
    /// Checks BOTH User ID limit (per-account budget) AND IP limit (per-network budget)
    /// in a single Redis round trip. Returns `true` only if both checks pass.
    pub async fn check_mixed_limit(
        &self,
        action: &str,
        user_id: Uuid,
        ip: &str,
        user_max: u32,
        ip_max: u32,
        duration: Duration,
    ) -> bool {
        let mut conn = self.manager.clone();
        let user_key = format!("ratelimit:{action}:user:{user_id}");
        let ip_key = format!("ratelimit:{action}:ip:{ip}");
        let duration_secs = duration.as_secs().max(1).cast_signed();
        let counts: Result<Vec<i64>, redis::RedisError> =
            INCR_EXPIRE_TWO_SCRIPT.key(user_key).key(ip_key).arg(duration_secs).invoke_async(&mut conn).await;

        match counts {
            Ok(counts) if counts.len() == 2 => counts[0] <= i64::from(user_max) && counts[1] <= i64::from(ip_max),
            Ok(_) => true, // Unreachable shape — fail open.
            Err(e) => {
                tracing::warn!("Redis rate limiter error: {e}");
                true // Fail-open if redis temporary connection error
            }
        }
    }
}

/// Số hop proxy tin cậy, đọc 1 lần lúc boot từ `TRUSTED_PROXY_HOPS`.
static TRUSTED_PROXY_HOPS: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("TRUSTED_PROXY_HOPS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(crate::config::constants::DEFAULT_TRUSTED_PROXY_HOPS)
});

/// Lấy IP client từ `X-Forwarded-For`, đếm `TRUSTED_PROXY_HOPS` hop từ phải.
///
/// Proxy tin cậy append IP client vào cuối header, nên hop cuối là giá trị duy nhất
/// client không tự đặt được. Thiếu header thì fallback loopback (dev/local).
#[must_use]
pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
    extract_client_ip_with_hops(headers, *TRUSTED_PROXY_HOPS)
}

/// Bản thuần (không đọc env) để test được hop count.
fn extract_client_ip_with_hops(headers: &axum::http::HeaderMap, hops: usize) -> String {
    let hops = hops.max(1);
    headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.rsplit(',').nth(hops - 1))
        .map_or_else(|| "127.0.0.1".to_string(), |s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    use super::extract_client_ip_with_hops;

    #[test]
    fn test_extract_client_ip_uses_last_trusted_hop() {
        let mut headers = HeaderMap::new();
        // Client tự đặt "1.2.3.4"; proxy append IP thật → chỉ IP cuối là tin được.
        headers.insert("x-forwarded-for", "1.2.3.4, 5.6.7.8".parse().unwrap());
        assert_eq!(extract_client_ip_with_hops(&headers, 1), "5.6.7.8");
        assert_eq!(extract_client_ip_with_hops(&headers, 2), "1.2.3.4");
    }

    #[test]
    fn test_extract_client_ip_falls_back_to_loopback() {
        assert_eq!(extract_client_ip_with_hops(&HeaderMap::new(), 1), "127.0.0.1");
    }
}
