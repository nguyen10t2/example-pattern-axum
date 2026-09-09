//! Shared application-wide default constants.
//!
//! Single source of truth for magic values that were previously scattered as
//! literals across handlers, `main.rs` and utils (timeouts, rate-limit
//! budgets, TTLs, fallback URLs, ...). Environment variables still override
//! these at runtime where supported — the constants only define the fallbacks.

use std::time::Duration;

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Default bind host (`HOST`).
pub const DEFAULT_HOST: &str = "0.0.0.0";
/// Default bind port (`PORT`).
pub const DEFAULT_PORT: &str = "3000";
/// Max request body size in bytes (1 MiB).
pub const MAX_BODY_BYTES: usize = 1_048_576;

// ---------------------------------------------------------------------------
// Frontend
// ---------------------------------------------------------------------------

/// Default frontend base URL (`FRONTEND_URL`, `REDIS_URL`-style fallback).
pub const DEFAULT_FRONTEND_URL: &str = "http://localhost:5173";
/// Google OAuth callback path, appended to the frontend base URL.
pub const GOOGLE_CALLBACK_PATH: &str = "/api/users/auth/google/callback";

// ---------------------------------------------------------------------------
// Cache TTLs (seconds)
// ---------------------------------------------------------------------------

/// Generic short-lived cache entries (e.g. group summaries, user profiles).
pub const CACHE_EXPIRATION: u64 = 60; // 1 minute
/// Refresh-token / session entries.
pub const REFRESH_TOKEN_EXPIRATION: u64 = 7 * 24 * 60 * 60; // 7 days
/// One-time-password entries.
pub const OTP_EXPIRATION: u64 = 2 * 60; // 2 minutes

// ---------------------------------------------------------------------------
// JWT
// ---------------------------------------------------------------------------

/// Access-token lifetime in seconds (15 minutes).
pub const JWT_ACCESS_TOKEN_EXPIRATION_SECS: usize = 15 * 60;
/// Refresh-token lifetime in seconds (7 days, mirrors [`REFRESH_TOKEN_EXPIRATION`]).
pub const JWT_REFRESH_TOKEN_EXPIRATION_SECS: usize = 7 * 24 * 60 * 60;
/// Fallback secret when `JWT_SECRET` is unset (dev only).
pub const DEFAULT_JWT_SECRET: &str = "default_splitdebt_jwt_secret_key_12345";
/// Fallback issuer when `JWT_ISSUER` is unset.
pub const DEFAULT_JWT_ISSUER: &str = "default-issuer";
/// Fallback audience when `JWT_AUDIENCE` is unset.
pub const DEFAULT_JWT_AUDIENCE: &str = "default-audience";

// ---------------------------------------------------------------------------
// Rate limiting
// ---------------------------------------------------------------------------

/// Shared rate-limit window for all actions.
pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// Max `request-otp` calls per IP per window.
pub const RATE_LIMIT_REQUEST_OTP_IP_MAX: u32 = 3;
/// Max `forgot-password-otp` calls per IP per window.
pub const RATE_LIMIT_FORGOT_PASSWORD_OTP_IP_MAX: u32 = 3;
/// Max `signin` calls per IP per window.
pub const RATE_LIMIT_SIGNIN_IP_MAX: u32 = 5;
/// Max `change-password` calls per user per window.
pub const RATE_LIMIT_CHANGE_PASSWORD_USER_MAX: u32 = 3;
/// Max `change-password` calls per IP per window.
pub const RATE_LIMIT_CHANGE_PASSWORD_IP_MAX: u32 = 10;
/// Max `create-group` calls per user per window.
pub const RATE_LIMIT_CREATE_GROUP_USER_MAX: u32 = 5;
/// Max `create-group` calls per IP per window.
pub const RATE_LIMIT_CREATE_GROUP_IP_MAX: u32 = 20;
/// Max `join-group` calls per user per window.
pub const RATE_LIMIT_JOIN_GROUP_USER_MAX: u32 = 10;
/// Max `join-group` calls per IP per window.
pub const RATE_LIMIT_JOIN_GROUP_IP_MAX: u32 = 30;

// ---------------------------------------------------------------------------
// Cookies
// ---------------------------------------------------------------------------

/// `max-age` for the Google OAuth state/verifier cookies in seconds (10 minutes).
pub const OAUTH_COOKIE_MAX_AGE_SECS: i64 = 600;

// ---------------------------------------------------------------------------
// Mailer
// ---------------------------------------------------------------------------

/// Bounded queue capacity for the background email worker.
pub const MAILER_BUFFER_SIZE: usize = 128;
