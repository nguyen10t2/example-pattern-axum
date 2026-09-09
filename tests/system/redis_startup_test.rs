use std::time::{Duration, Instant};

use dsa::config::{RedisConfig, RedisConnectError};

fn unreachable_config() -> RedisConfig {
    RedisConfig::builder()
        .url("redis://10.255.255.1:6399")
        .connection_timeout(Duration::from_secs(1))
        .response_timeout(Duration::from_secs(1))
        .build()
}

#[tokio::test]
async fn test_redis_connect_invalid_url_fails_fast() {
    let config = RedisConfig::builder().url("http://[invalid").connection_timeout(Duration::from_secs(1)).build();

    let started = Instant::now();
    let result = config.connect().await;

    assert!(matches!(result, Err(RedisConnectError::InvalidUrl(_))));
    assert!(started.elapsed() < Duration::from_secs(10), "invalid URL must fail without dialing");
}

#[tokio::test]
async fn test_redis_connect_unreachable_times_out() {
    let config = unreachable_config();

    let started = Instant::now();
    let result = config.connect().await;
    let elapsed = started.elapsed();

    assert!(result.is_err(), "unreachable Redis must surface an error, not hang");
    assert!(elapsed < Duration::from_secs(10), "connect took {elapsed:?}, expected fail-fast");
}
