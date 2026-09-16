//! OTP cache: lưu mã + đếm sai, check tập trung cho mọi flow (signup/reset/verify).
//!
//! Lifecycle chuẩn: [`store`] khi xin mã → [`check`] mỗi lần đối chiếu
//! (sai đủ `MAX_OTP_ATTEMPTS` lần thì tự hủy) → [`consume`] sau khi dùng xong.
//!
//! Đối chiếu chạy nguyên tử trong cache backend (Redis Lua `KEEPTTL`, Memory sửa
//! tại chỗ): lần sai tăng counter NHƯNG giữ nguyên TTL, chống kéo dài cửa sổ
//! brute-force bằng spam sai.

use serde::{Deserialize, Serialize};

use crate::{
    config::constants::{MAX_OTP_ATTEMPTS, OTP_EXPIRATION},
    errors::{AppError, BusinessError},
    utils::cache::{Cache, CacheStore, CacheStoreExt, OtpCheckOutcome, reset_otp_key, signup_otp_key},
};

/// OTP lưu cache kèm bộ đếm sai (chống brute-force).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtpEntry {
    pub code: String,
    pub attempts: u32,
}

/// Key cache OTP đăng ký theo email (delegate về `cache::keys` — format single source).
///
/// `#[must_use]` vì chỉ dựng `String`, không có I/O.
#[must_use]
pub fn signup_key(email: &str) -> String {
    signup_otp_key(email)
}

/// Key cache OTP quên mật khẩu theo email (delegate về `cache::keys` — format single source).
///
/// `#[must_use]` vì chỉ dựng `String`, không có I/O.
#[must_use]
pub fn reset_key(email: &str) -> String {
    reset_otp_key(email)
}

/// Lưu mã mới (reset bộ đếm), TTL theo `OTP_EXPIRATION`.
///
/// Để `async` vì ghi Redis là I/O mạng thật.
///
/// # Errors
///
/// Trả lỗi hệ thống (cache unavailable → 503) khi Redis lỗi — caller phải
/// fail-closed, không được coi như đã gửi OTP thành công.
pub async fn store(cache: &Cache, key: &str, code: String) -> Result<(), AppError> {
    cache.set(key, &OtpEntry { code, attempts: 0 }, OTP_EXPIRATION).await?;
    Ok(())
}

/// Đối chiếu OTP nguyên tử, sai thì tăng đếm; đủ `MAX_OTP_ATTEMPTS` lần thì xóa mã.
/// Không tiêu thụ mã khi đúng (caller tự [`consume`] sau khi dùng xong).
///
/// Để `async` vì đọc/ghi Redis là I/O mạng thật.
///
/// # Errors
///
/// Trả `InvalidOtp` khi mã sai, hết hạn hoặc đã bị hủy do sai nhiều lần.
/// Trả lỗi hệ thống (cache unavailable → 503) khi Redis lỗi — fail-closed thay vì
/// khóa user oan.
pub async fn check(cache: &Cache, key: &str, otp: &str) -> Result<(), AppError> {
    match cache.check_otp_code(key, otp, MAX_OTP_ATTEMPTS).await? {
        OtpCheckOutcome::Match => Ok(()),
        OtpCheckOutcome::Mismatch | OtpCheckOutcome::Missing | OtpCheckOutcome::Locked => {
            Err(AppError::Business(BusinessError::InvalidOtp))
        }
    }
}

/// Xóa mã sau khi dùng xong (OTP một lần).
///
/// Để `async` vì xóa Redis là I/O mạng thật.
///
/// # Errors
///
/// Trả lỗi hệ thống (cache unavailable → 503) khi Redis lỗi — caller critical
/// (reset password) phải fail-closed để mã không bị tái sử dụng lén.
pub async fn consume(cache: &Cache, key: &str) -> Result<(), AppError> {
    cache.delete(key).await?;
    Ok(())
}
