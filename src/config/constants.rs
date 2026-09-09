//! Giá trị mặc định dùng chung toàn app.
//!
//! Single source of truth thay cho magic values rải rác ở handlers, `main.rs` và utils
//! (timeout, quota rate-limit, TTL, URL fallback, ...). Biến môi trường vẫn override
//! được lúc runtime — const ở đây chỉ là fallback.

use std::time::Duration;

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Host bind mặc định (`HOST`).
pub const DEFAULT_HOST: &str = "0.0.0.0";
/// Port bind mặc định (`PORT`).
pub const DEFAULT_PORT: &str = "3000";
/// Body request tối đa, bytes (1 MiB).
pub const MAX_BODY_BYTES: usize = 1_048_576;

// ---------------------------------------------------------------------------
// Frontend
// ---------------------------------------------------------------------------

/// Base URL frontend mặc định (`FRONTEND_URL`).
pub const DEFAULT_FRONTEND_URL: &str = "http://localhost:5173";
/// Path callback OAuth Google, nối sau base URL frontend.
pub const GOOGLE_CALLBACK_PATH: &str = "/api/users/auth/google/callback";

// ---------------------------------------------------------------------------
// Cache TTLs (giây)
// ---------------------------------------------------------------------------

/// Cache ngắn hạn chung (summary nhóm, profile user, ...).
pub const CACHE_EXPIRATION: u64 = 60; // 1 minute
/// Cache refresh-token / session.
pub const REFRESH_TOKEN_EXPIRATION: u64 = 7 * 24 * 60 * 60; // 7 days
/// Cache mã OTP.
pub const OTP_EXPIRATION: u64 = 2 * 60; // 2 minutes

// ---------------------------------------------------------------------------
// JWT
// ---------------------------------------------------------------------------

/// Tuổi thọ access-token, giây (15 phút).
pub const JWT_ACCESS_TOKEN_EXPIRATION_SECS: usize = 15 * 60;
/// Tuổi thọ refresh-token, giây (7 ngày, khớp [`REFRESH_TOKEN_EXPIRATION`]).
pub const JWT_REFRESH_TOKEN_EXPIRATION_SECS: usize = 7 * 24 * 60 * 60;
/// Secret fallback khi thiếu `JWT_SECRET` (chỉ dev).
pub const DEFAULT_JWT_SECRET: &str = "default_splitdebt_jwt_secret_key_12345";
/// Issuer fallback khi thiếu `JWT_ISSUER`.
pub const DEFAULT_JWT_ISSUER: &str = "default-issuer";
/// Audience fallback khi thiếu `JWT_AUDIENCE`.
pub const DEFAULT_JWT_AUDIENCE: &str = "default-audience";

// ---------------------------------------------------------------------------
// Rate limiting
// ---------------------------------------------------------------------------

/// Cửa sổ rate-limit dùng chung mọi action.
pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// Số lần `request-otp` tối đa mỗi IP mỗi cửa sổ.
pub const RATE_LIMIT_REQUEST_OTP_IP_MAX: u32 = 3;
/// Số lần `forgot-password-otp` tối đa mỗi IP mỗi cửa sổ.
pub const RATE_LIMIT_FORGOT_PASSWORD_OTP_IP_MAX: u32 = 3;
/// Số lần `signin` tối đa mỗi IP mỗi cửa sổ.
pub const RATE_LIMIT_SIGNIN_IP_MAX: u32 = 5;
/// Số lần `change-password` tối đa mỗi user mỗi cửa sổ.
pub const RATE_LIMIT_CHANGE_PASSWORD_USER_MAX: u32 = 3;
/// Số lần `change-password` tối đa mỗi IP mỗi cửa sổ.
pub const RATE_LIMIT_CHANGE_PASSWORD_IP_MAX: u32 = 10;
/// Số lần `create-group` tối đa mỗi user mỗi cửa sổ.
pub const RATE_LIMIT_CREATE_GROUP_USER_MAX: u32 = 5;
/// Số lần `create-group` tối đa mỗi IP mỗi cửa sổ.
pub const RATE_LIMIT_CREATE_GROUP_IP_MAX: u32 = 20;
/// Số lần `join-group` tối đa mỗi user mỗi cửa sổ.
pub const RATE_LIMIT_JOIN_GROUP_USER_MAX: u32 = 10;
/// Số lần `join-group` tối đa mỗi IP mỗi cửa sổ.
pub const RATE_LIMIT_JOIN_GROUP_IP_MAX: u32 = 30;

// ---------------------------------------------------------------------------
// Cookies
// ---------------------------------------------------------------------------

/// `max-age` cookie state/verifier OAuth Google, giây (10 phút).
pub const OAUTH_COOKIE_MAX_AGE_SECS: i64 = 600;

// ---------------------------------------------------------------------------
// Mailer
// ---------------------------------------------------------------------------

/// Sức chứa queue của worker gửi mail nền.
pub const MAILER_BUFFER_SIZE: usize = 128;

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

/// Số session refresh-token tối đa mỗi user (đuổi session cũ nhất trước).
pub const MAX_SESSIONS_PER_USER: usize = 5;
