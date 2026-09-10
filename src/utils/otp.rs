//! OTP cache: lưu mã + đếm sai, check tập trung cho mọi flow (signup/reset/verify).
//!
//! Lifecycle chuẩn: [`store`] khi xin mã → [`check`] mỗi lần đối chiếu
//! (sai đủ `MAX_OTP_ATTEMPTS` thì tự hủy) → [`consume`] sau khi dùng xong.

use serde::{Deserialize, Serialize};

use crate::{
    config::constants::{MAX_OTP_ATTEMPTS, OTP_EXPIRATION},
    errors::{AppError, BusinessError},
    utils::cache::{Cache, CacheStore, CacheStoreExt},
};

/// OTP lưu cache kèm bộ đếm sai (chống brute-force).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtpEntry {
    pub code: String,
    pub attempts: u32,
}

/// Key cache OTP đăng ký theo email.
#[must_use]
pub fn signup_key(email: &str) -> String {
    format!("otp:{}", email.to_lowercase())
}

/// Key cache OTP quên mật khẩu theo email.
#[must_use]
pub fn reset_key(email: &str) -> String {
    format!("forgot_otp:{}", email.to_lowercase())
}

/// Lưu mã mới (reset bộ đếm), TTL theo `OTP_EXPIRATION`.
pub async fn store(cache: &Cache, key: &str, code: String) {
    cache.set(key, &OtpEntry { code, attempts: 0 }, OTP_EXPIRATION).await;
}

/// Đối chiếu OTP, sai thì tăng đếm; đủ `MAX_OTP_ATTEMPTS` lần thì xóa mã.
/// Không tiêu thụ mã khi đúng (caller tự [`consume`] sau khi dùng xong).
///
/// # Errors
///
/// Trả `InvalidOtp` khi mã sai, hết hạn hoặc đã bị hủy do sai nhiều lần.
pub async fn check(cache: &Cache, key: &str, otp: &str) -> Result<(), AppError> {
    let Some(mut entry) = cache.get::<OtpEntry>(key).await else {
        return Err(AppError::Business(BusinessError::InvalidOtp));
    };
    if entry.code != otp {
        entry.attempts += 1;
        if entry.attempts >= MAX_OTP_ATTEMPTS {
            cache.delete(key).await;
        } else {
            cache.set(key, &entry, OTP_EXPIRATION).await;
        }
        return Err(AppError::Business(BusinessError::InvalidOtp));
    }
    Ok(())
}

/// Xóa mã sau khi dùng xong (OTP một lần).
pub async fn consume(cache: &Cache, key: &str) {
    cache.delete(key).await;
}
