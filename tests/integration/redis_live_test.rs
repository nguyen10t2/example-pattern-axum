//! Live Redis tests cho rate limiter Lua và `delete_many` pipeline.
//!
//! Cần Redis thật (mặc định `127.0.0.1:6379`, hoặc set `REDIS_URL`).
//! Dùng key suffix UUID nên chạy song song và lặp lại an toàn.

use std::time::Duration;

use redis::AsyncCommands;
use uuid::Uuid;

use dsa::{
    domain::{
        Currency, GroupRole,
        groups::{
            entity::{GroupEntity, GroupMemberWithUser},
            service::GroupService,
        },
        users::entity::UserEntity,
    },
    middleware::RedisRateLimiter,
    utils::cache::{Cache, CacheStore, RedisCache},
};
use std::sync::Arc;

fn redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

async fn test_manager() -> redis::aio::ConnectionManager {
    let client = redis::Client::open(redis_url().as_str()).unwrap();
    redis::aio::ConnectionManager::new(client).await.unwrap()
}

async fn test_client() -> redis::aio::MultiplexedConnection {
    let client = redis::Client::open(redis_url().as_str()).unwrap();
    client.get_multiplexed_async_connection().await.unwrap()
}

fn unique_key(prefix: &str) -> String {
    format!("{prefix}:test:{}", Uuid::now_v7())
}

#[tokio::test]
#[ignore = "requires a live Redis (REDIS_URL or 127.0.0.1:6379)"]
async fn test_rate_limit_blocks_over_max() {
    let limiter = RedisRateLimiter::new(test_manager().await);
    let key = unique_key("rl-single");
    let window = Duration::from_secs(60);

    for _ in 0..3 {
        assert!(limiter.check_rate_limit(&key, 3, window).await);
    }
    assert!(!limiter.check_rate_limit(&key, 3, window).await);

    let mut raw = test_client().await;
    let _: () = redis::cmd("DEL").arg(&key).query_async(&mut raw).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a live Redis (REDIS_URL or 127.0.0.1:6379)"]
async fn test_rate_limit_sets_ttl_on_first_hit() {
    let limiter = RedisRateLimiter::new(test_manager().await);
    let key = unique_key("rl-ttl");

    assert!(limiter.check_rate_limit(&key, 100, Duration::from_secs(60)).await);

    // EXPIRE phải được set ngay lần đầu — không thì key rò rỉ không TTL.
    let mut raw = test_client().await;
    let ttl: i64 = raw.ttl(&key).await.unwrap();
    assert!(ttl > 0 && ttl <= 60, "expected TTL in (0, 60], got {ttl}");

    let _: () = redis::cmd("DEL").arg(&key).query_async(&mut raw).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a live Redis (REDIS_URL or 127.0.0.1:6379)"]
async fn test_mixed_limit_enforces_both_budgets() {
    let limiter = RedisRateLimiter::new(test_manager().await);
    let action = format!("mixed-test-{}", Uuid::now_v7());
    let user_id = Uuid::now_v7();
    let window = Duration::from_secs(60);

    // user budget 2, ip budget 5 — request thứ 3 phải rớt vì user budget.
    let ip = "10.9.9.1";
    assert!(limiter.check_mixed_limit(&action, user_id, ip, 2, 5, window).await);
    assert!(limiter.check_mixed_limit(&action, user_id, ip, 2, 5, window).await);
    assert!(!limiter.check_mixed_limit(&action, user_id, ip, 2, 5, window).await);

    // IP khác nhưng cùng user vẫn rớt — chứng tỏ đếm theo user thật.
    assert!(!limiter.check_mixed_limit(&action, user_id, "10.9.9.2", 2, 5, window).await);

    let mut raw = test_client().await;
    let user_key = format!("ratelimit:{action}:user:{user_id}");
    let first_ip_key = format!("ratelimit:{action}:ip:{ip}");
    let second_ip_key = format!("ratelimit:{action}:ip:10.9.9.2");
    let _: () =
        redis::cmd("DEL").arg(&user_key).arg(&first_ip_key).arg(&second_ip_key).query_async(&mut raw).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a live Redis (REDIS_URL or 127.0.0.1:6379)"]
async fn test_membership_cache_roundtrip_over_redis() {
    use crate::common::{
        MockExpenseRepository, MockGroupRepository, MockSettlementRepository, MockUserRepository, test_pool,
    };
    use chrono::Utc;

    let group_repo = MockGroupRepository::default();
    let user_repo = MockUserRepository::default();
    let group_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();

    user_repo.users.lock().await.push(UserEntity {
        id: user_id,
        full_name: "Live User".to_string(),
        email: "live@example.com".to_string(),
        email_verified: true,
        password_hash: None,
        google_id: None,
        avatar_url: None,
        phone: None,
        phone_verified: false,
        preferred_currency: Currency::VND,
        is_active: true,
        deleted_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    });
    group_repo.groups.lock().await.push(GroupEntity {
        id: group_id,
        name: "Live Group".to_string(),
        description: None,
        invite_code: None,
        default_currency: Currency::VND,
        deleted_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    });
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id,
        full_name: "Live User".to_string(),
        role: GroupRole::MEMBER,
        joined_at: Utc::now(),
    });

    let cache = Arc::new(Cache::Redis(RedisCache::new(test_manager().await)));
    let service = GroupService::new(
        group_repo,
        MockExpenseRepository::default(),
        MockSettlementRepository::default(),
        user_repo,
        cache,
        test_pool(),
    );

    // Miss (query + nạp cache) rồi hit (đọc từ Redis) — serde `MemberEntry` lỗi là rớt ngay.
    assert!(service.ensure_membership(group_id, user_id).await.is_ok());
    assert!(service.ensure_membership(group_id, user_id).await.is_ok());
}

#[tokio::test]
#[ignore = "requires a live Redis (REDIS_URL or 127.0.0.1:6379)"]
async fn test_redis_cache_delete_many_pipelined() {
    let cache = RedisCache::new(test_manager().await);
    let suffix = Uuid::now_v7();
    let keys: Vec<String> = (0..3).map(|i| format!("cache-test:{suffix}:{i}")).collect();

    for key in &keys {
        cache.set_raw(key, "v", 60).await;
    }
    cache.delete_many(&keys).await;

    let mut raw = test_client().await;
    for key in &keys {
        let got: Option<String> = raw.get(key).await.unwrap();
        assert!(got.is_none(), "key {key} should be gone after pipelined delete");
    }
}
