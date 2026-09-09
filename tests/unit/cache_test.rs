use dsa::utils::cache::{Cache, CacheStore, MemoryCache};

#[tokio::test]
async fn test_delete_many_removes_all_keys() {
    let cache = Cache::Memory(MemoryCache::new());
    cache.set_raw("k1", "v1", 60).await;
    cache.set_raw("k2", "v2", 60).await;
    cache.set_raw("keep", "v", 60).await;

    cache.delete_many(&["k1".to_string(), "k2".to_string()]).await;

    assert!(cache.get_raw("k1").await.is_none());
    assert!(cache.get_raw("k2").await.is_none());
    assert_eq!(cache.get_raw("keep").await.as_deref(), Some("v"));

    // Rỗng và key lạ là no-op, không lỗi.
    cache.delete_many(&[]).await;
    cache.delete_many(&["nope".to_string()]).await;
}
