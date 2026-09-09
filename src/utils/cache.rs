use async_trait::async_trait;
use dashmap::DashMap;
use redis::AsyncCommands;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

// Cache TTLs live in [`crate::config::constants`]; re-exported here so
// existing `utils::cache::{...}` imports keep working.
pub use crate::config::constants::{CACHE_EXPIRATION, OTP_EXPIRATION, REFRESH_TOKEN_EXPIRATION};

#[async_trait]
pub trait CacheStore: Send + Sync {
    async fn get_raw(&self, key: &str) -> Option<String>;
    async fn set_raw(&self, key: &str, value: &str, expiration_secs: u64);
    async fn delete(&self, key: &str);
}

#[async_trait]
pub trait CacheStoreExt: CacheStore {
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let raw = self.get_raw(key).await?;
        serde_json::from_str(&raw).ok()
    }

    async fn set<T: Serialize + Send + Sync>(&self, key: &str, value: &T, expiration_secs: u64) {
        if let Ok(s) = serde_json::to_string(value) {
            self.set_raw(key, &s, expiration_secs).await;
        }
    }
}

impl<T: CacheStore + ?Sized> CacheStoreExt for T {}

#[derive(Clone)]
pub struct RedisCache {
    manager: redis::aio::ConnectionManager,
}

impl RedisCache {
    pub fn new(manager: redis::aio::ConnectionManager) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl CacheStore for RedisCache {
    async fn get_raw(&self, key: &str) -> Option<String> {
        let mut conn = self.manager.clone();
        let res: Result<Option<String>, redis::RedisError> = conn.get(key).await;
        res.ok().flatten()
    }

    async fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) {
        let mut conn = self.manager.clone();
        let _: Result<(), redis::RedisError> = conn.set_ex(key, value, expiration_secs).await;
    }

    async fn delete(&self, key: &str) {
        let mut conn = self.manager.clone();
        let _: Result<(), redis::RedisError> = conn.del(key).await;
    }
}

/// In-memory cache store backed by `DashMap` for concurrent testing
#[derive(Clone, Default)]
pub struct MemoryCache {
    store: Arc<DashMap<String, (String, Instant)>>,
}

impl MemoryCache {
    pub fn new() -> Self {
        Self { store: Arc::new(DashMap::new()) }
    }
}

#[async_trait]
impl CacheStore for MemoryCache {
    async fn get_raw(&self, key: &str) -> Option<String> {
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

    async fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) {
        let expires_at = Instant::now() + Duration::from_secs(expiration_secs);
        self.store.insert(key.to_string(), (value.to_string(), expires_at));
    }

    async fn delete(&self, key: &str) {
        self.store.remove(key);
    }
}
