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
    /// Đọc string thô theo key, `None` khi miss/hết hạn/lỗi.
    fn get_raw(&self, key: &str) -> impl Future<Output = Option<String>> + Send;
    /// Ghi string thô kèm TTL (giây).
    fn set_raw(&self, key: &str, value: &str, expiration_secs: u64) -> impl Future<Output = ()> + Send;
    /// Xóa key (idempotent).
    fn delete(&self, key: &str) -> impl Future<Output = ()> + Send;
    /// Xóa nhiều key trong 1 batch (Redis pipeline; backend khác loop).
    fn delete_many(&self, keys: &[String]) -> impl Future<Output = ()> + Send {
        async move {
            for key in keys {
                self.delete(key).await;
            }
        }
    }
}

pub trait CacheStoreExt: CacheStore {
    /// Đọc + deserialize JSON theo key.
    fn get<T: DeserializeOwned>(&self, key: &str) -> impl Future<Output = Option<T>> + Send {
        async move {
            let raw = self.get_raw(key).await?;
            serde_json::from_str(&raw).ok()
        }
    }

    /// Serialize + ghi JSON theo key kèm TTL (giây).
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
    /// Bọc Redis connection manager dùng chung thành cache.
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

    async fn delete_many(&self, keys: &[String]) {
        if keys.is_empty() {
            return;
        }
        let mut conn = self.manager.clone();
        let mut pipe = redis::pipe();
        for key in keys {
            pipe.cmd("DEL").arg(key);
        }
        let _: Result<(), redis::RedisError> = pipe.exec_async(&mut conn).await;
    }
}

/// In-memory cache store backed by `DashMap` for concurrent testing
#[derive(Clone, Default)]
pub struct MemoryCache {
    store: Arc<DashMap<String, (String, Instant)>>,
}

impl MemoryCache {
    /// Cache in-memory cho test (hết hạn theo `Instant`).
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

/// Tập backend cache của app, dispatch tĩnh thay vì `dyn`.
///
/// Chỉ có `Redis` (production) và `Memory` (test) nên trait object chỉ thêm vtable
/// mà không có lợi gì — thêm backend mới thì thêm variant ở đây.
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
