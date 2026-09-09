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

pub trait CacheStore: Send + Sync {
    fn get_raw(&self, key: &str) -> impl Future<Output = Option<String>> + Send;
    fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) -> impl Future<Output = ()> + Send;
    fn delete(&self, key: &str) -> impl Future<Output = ()> + Send;
}

pub trait CacheStoreExt: CacheStore {
    fn get<T: DeserializeOwned>(&self, key: &str) -> impl Future<Output = Option<T>> + Send {
        async move {
            let raw = self.get_raw(key).await?;
            serde_json::from_str(&raw).ok()
        }
    }

    fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        expiration_secs: u64,
    ) -> impl Future<Output = ()> + Send {
        async move {
            if let Ok(s) = serde_json::to_string(value) {
                self.set_raw(key, &s, expiration_secs).await;
            }
        }
    }
}

impl<T: CacheStore + ?Sized> CacheStoreExt for T {}

#[derive(Clone)]
pub struct RedisCache {
    manager: redis::aio::ConnectionManager,
}

impl RedisCache {
    #[must_use]
    pub const fn new(manager: redis::aio::ConnectionManager) -> Self {
        Self { manager }
    }
}

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
    #[must_use]
    pub fn new() -> Self {
        Self { store: Arc::new(DashMap::new()) }
    }
}

impl CacheStore for MemoryCache {
    fn get_raw(&self, key: &str) -> impl Future<Output = Option<String>> + Send {
        std::future::ready(match self.store.entry(key.to_string()) {
            dashmap::mapref::entry::Entry::Occupied(entry) => {
                if Instant::now() < entry.get().1 {
                    Some(entry.get().0.clone())
                } else {
                    entry.remove();
                    None
                }
            }
            dashmap::mapref::entry::Entry::Vacant(_) => None,
        })
    }

    fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) -> impl Future<Output = ()> + Send {
        let expires_at = Instant::now() + Duration::from_secs(expiration_secs);
        self.store.insert(key.to_string(), (value.to_string(), expires_at));
        std::future::ready(())
    }

    fn delete(&self, key: &str) -> impl Future<Output = ()> + Send {
        self.store.remove(key);
        std::future::ready(())
    }
}

/// Closed set of cache backends used by the application.
///
/// Static (enum) dispatch instead of `dyn CacheStore`: the set of backends is
/// fixed (`Redis` in production, `Memory` in tests), so a trait object buys
/// nothing but costs a vtable, `Send`/`Sync` plumbing and — with native
/// `async fn` in traits (edition 2024) — object safety itself. New backends
/// are added as variants here.
#[derive(Clone)]
pub enum Cache {
    Redis(RedisCache),
    Memory(MemoryCache),
}

impl CacheStore for Cache {
    async fn get_raw(&self, key: &str) -> Option<String> {
        match self {
            Self::Redis(inner) => inner.get_raw(key).await,
            Self::Memory(inner) => inner.get_raw(key).await,
        }
    }

    async fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) {
        match self {
            Self::Redis(inner) => inner.set_raw(key, value, expiration_secs).await,
            Self::Memory(inner) => inner.set_raw(key, value, expiration_secs).await,
        }
    }

    async fn delete(&self, key: &str) {
        match self {
            Self::Redis(inner) => inner.delete(key).await,
            Self::Memory(inner) => inner.delete(key).await,
        }
    }
}
