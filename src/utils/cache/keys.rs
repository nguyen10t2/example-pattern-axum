//! Dựng key cache — single source of truth cho mọi key Redis.
//!
//! Toàn bộ là free function pure (sync, không I/O) thay vì method trên struct:
//! key là dữ liệu, không phải hành vi của object nào.

use uuid::Uuid;

use crate::config::constants::{
    FORGOT_OTP_KEY_PREFIX, GROUP_MEMBERS_KEY_PREFIX, GROUP_SUMMARY_KEY_PREFIX, OTP_KEY_PREFIX,
    REFRESH_TOKEN_KEY_PREFIX, SESSION_KEY_PREFIX, USER_CACHE_KEY_PREFIX,
};

/// Key refresh-token (`refreshToken:{jti} -> subject`).
///
/// Pure constructor nên sync + `#[must_use]`, không `async`.
#[must_use]
pub fn refresh_token_key(jti: &str) -> String {
    format!("{REFRESH_TOKEN_KEY_PREFIX}{jti}")
}

/// Key danh sách session (`sessions:{subject} -> JSON Vec<jti>`).
///
/// Pure constructor nên sync + `#[must_use]`, không `async`.
#[must_use]
pub fn session_list_key(subject: &str) -> String {
    format!("{SESSION_KEY_PREFIX}{subject}")
}

/// Key cache profile user (`user:{id} -> UserResponse`).
///
/// Pure constructor nên sync + `#[must_use]`, không `async`.
#[must_use]
pub fn user_profile_key(id: Uuid) -> String {
    format!("{USER_CACHE_KEY_PREFIX}{id}")
}

/// Key cache tổng hợp nhóm (`group_summary:{id} -> GroupSummaryResponse`).
///
/// Pure constructor nên sync + `#[must_use]`, không `async`.
#[must_use]
pub fn group_summary_key(group_id: Uuid) -> String {
    format!("{GROUP_SUMMARY_KEY_PREFIX}{group_id}")
}

/// Key cache membership nhóm (`group:members:{id} -> Vec<MemberEntry>`).
///
/// Pure constructor nên sync + `#[must_use]`, không `async`.
#[must_use]
pub fn group_members_key(group_id: Uuid) -> String {
    format!("{GROUP_MEMBERS_KEY_PREFIX}{group_id}")
}

/// Key cache OTP đăng ký (`otp:{email} -> OtpEntry`).
///
/// Pure constructor nên sync + `#[must_use]`, không `async`.
#[must_use]
pub fn signup_otp_key(email: &str) -> String {
    format!("{OTP_KEY_PREFIX}{}", email.to_lowercase())
}

/// Key cache OTP quên mật khẩu (`forgot_otp:{email} -> OtpEntry`).
///
/// Pure constructor nên sync + `#[must_use]`, không `async`.
#[must_use]
pub fn reset_otp_key(email: &str) -> String {
    format!("{FORGOT_OTP_KEY_PREFIX}{}", email.to_lowercase())
}
