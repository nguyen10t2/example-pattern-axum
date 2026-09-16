/// Lỗi hạ tầng cache — mọi lỗi Redis đều quy về đây để caller fail-closed (503).
///
/// Lỗi serialize khi *đọc* được coi như miss (entry corrupt thì xóa) thay vì
/// `Err`, nên enum này chỉ mang lỗi transport thật sự.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// Redis unreachable/timeout/lỗi lệnh.
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
}
