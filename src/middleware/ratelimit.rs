use std::{collections::HashSet, net::IpAddr, sync::LazyLock, time::Duration};
use uuid::Uuid;

use crate::utils::cache::CacheError;

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

/// Only peers explicitly listed here may supply a forwarded client chain.
/// With an empty/default value every forwarding header is ignored.
static TRUSTED_PROXY_IPS: LazyLock<HashSet<IpAddr>> = LazyLock::new(|| {
    std::env::var("TRUSTED_PROXY_IPS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|value| value.trim().parse().ok())
        .collect()
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
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] when Redis cannot enforce the limit.
    #[tracing::instrument(skip(self))]
    pub async fn check_rate_limit(&self, key: &str, max_requests: u32, duration: Duration) -> Result<bool, CacheError> {
        let mut conn = self.manager.clone();
        let duration_secs = duration.as_secs().max(1).cast_signed();
        let count_res: Result<i64, redis::RedisError> =
            INCR_EXPIRE_SCRIPT.key(key).arg(duration_secs).invoke_async(&mut conn).await;

        count_res.map(|current| current <= i64::from(max_requests)).map_err(CacheError::from)
    }

    /// Rate limits specifically by Client IP (used for public endpoints like OTP, login).
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] when Redis cannot enforce the limit.
    pub async fn check_ip_limit(
        &self,
        action: &str,
        ip: &str,
        max_requests: u32,
        duration: Duration,
    ) -> Result<bool, CacheError> {
        let key = format!("ratelimit:{action}:ip:{ip}");
        self.check_rate_limit(&key, max_requests, duration).await
    }

    /// Rate limits by email address (anti inbox-bombing cho endpoint xin OTP).
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] when Redis cannot enforce the limit.
    pub async fn check_email_limit(
        &self,
        action: &str,
        email: &str,
        max_requests: u32,
        duration: Duration,
    ) -> Result<bool, CacheError> {
        let key = format!("ratelimit:{action}:email:{}", email.to_lowercase());
        self.check_rate_limit(&key, max_requests, duration).await
    }

    /// Rate limits specifically by Authenticated User ID (prevents rotating IPs with proxy/VPN).
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] when Redis cannot enforce the limit.
    pub async fn check_user_limit(
        &self,
        action: &str,
        user_id: Uuid,
        max_requests: u32,
        duration: Duration,
    ) -> Result<bool, CacheError> {
        let key = format!("ratelimit:{action}:user:{user_id}");
        self.check_rate_limit(&key, max_requests, duration).await
    }

    /// Mixed / Hybrid Rate Limiting:
    /// Checks BOTH User ID limit (per-account budget) AND IP limit (per-network budget)
    /// in a single Redis round trip. Returns `true` only if both checks pass.
    ///
    /// # Errors
    ///
    /// Returns [`CacheError`] when Redis cannot enforce either limit.
    pub async fn check_mixed_limit(
        &self,
        action: &str,
        user_id: Uuid,
        ip: &str,
        user_max: u32,
        ip_max: u32,
        duration: Duration,
    ) -> Result<bool, CacheError> {
        let mut conn = self.manager.clone();
        let user_key = format!("ratelimit:{action}:user:{user_id}");
        let ip_key = format!("ratelimit:{action}:ip:{ip}");
        let duration_secs = duration.as_secs().max(1).cast_signed();
        let counts: Result<Vec<i64>, redis::RedisError> =
            INCR_EXPIRE_TWO_SCRIPT.key(user_key).key(ip_key).arg(duration_secs).invoke_async(&mut conn).await;

        match counts {
            Ok(counts) if counts.len() == 2 => Ok(counts[0] <= i64::from(user_max) && counts[1] <= i64::from(ip_max)),
            Ok(_) => Err(CacheError::from(redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "rate-limit script returned an invalid response",
            )))),
            Err(error) => Err(CacheError::from(error)),
        }
    }
}

/// Resolve the client IP without trusting attacker-supplied forwarding headers.
///
/// Starting at the TCP peer, walk `X-Forwarded-For` from right to left only
/// while the current hop is explicitly trusted. This also handles multiple
/// trusted proxy hops without accepting a spoofed left-most value.
#[must_use]
pub fn extract_client_ip(headers: &axum::http::HeaderMap, peer_ip: IpAddr) -> IpAddr {
    let Some(forwarded_for) = headers.get("x-forwarded-for").and_then(|value| value.to_str().ok()) else {
        return peer_ip;
    };

    let mut client_ip = peer_ip;
    for hop in forwarded_for.rsplit(',') {
        if !TRUSTED_PROXY_IPS.contains(&client_ip) {
            break;
        }
        let Ok(parsed) = hop.trim().parse() else {
            break;
        };
        client_ip = parsed;
    }
    client_ip
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn untrusted_peer_cannot_spoof_forwarded_client_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.7"));
        let peer_ip = "203.0.113.10".parse().unwrap();

        assert_eq!(extract_client_ip(&headers, peer_ip), peer_ip);
    }
}
