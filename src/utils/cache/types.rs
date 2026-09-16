//! Kiểu kết quả + tham số cho các op nguyên tử của cache (session/OTP).
//!
//! Dùng struct field đặt tên thay vì tuple/`&str` rời rạc để không tráo thứ tự
//! key ở call site (type-safe hơn positional args, vẫn zero-cost borrowing).

/// Kết quả một lần xoay refresh token nguyên tử.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationOutcome {
    /// Key cũ hợp lệ và đã xoay xong (key cũ xóa, key mới + sessions đã ghi).
    Rotated,
    /// Key cũ thiếu hoặc khác subject — có thể là replay, caller phải revoke cả family.
    Stale,
}

/// Kết quả đối chiếu OTP nguyên tử (lần sai KHÔNG gia hạn TTL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtpCheckOutcome {
    /// Mã đúng (entry không bị mutate, không tiêu thụ).
    Match,
    /// Mã sai nhưng còn lượt thử.
    Mismatch,
    /// Entry thiếu, hết hạn hoặc corrupt (corrupt thì đã xóa).
    Missing,
    /// Vừa vượt quá số lần cho phép — entry đã bị xóa, phải xin mã mới.
    Locked,
}

/// Tham số cho một lần xoay refresh token nguyên tử (`rotate_refresh_token`).
#[derive(Debug)]
pub struct RefreshRotation<'a> {
    /// Key refresh cũ (`refreshToken:{old_jti}`).
    pub old_key: &'a str,
    /// Key refresh mới (`refreshToken:{new_jti}`).
    pub new_key: &'a str,
    /// Key danh sách session (`sessions:{subject}`).
    pub session_key: &'a str,
    /// Subject phải khớp giá trị lưu ở key cũ.
    pub subject: &'a str,
    /// JTI cũ (để loại khỏi danh sách sessions).
    pub old_jti: &'a str,
    /// JTI mới (để thêm vào danh sách sessions).
    pub new_jti: &'a str,
    /// TTL (giây) cho key mới và sessions.
    pub ttl_secs: u64,
}

/// Tham số tạo session refresh mới (`create_refresh_session`, đuổi cũ nhất khi đầy).
#[derive(Debug)]
pub struct NewRefreshSession<'a> {
    /// Key refresh mới (`refreshToken:{jti}`).
    pub key: &'a str,
    /// Key danh sách session (`sessions:{subject}`).
    pub session_key: &'a str,
    /// Subject lưu vào key mới.
    pub subject: &'a str,
    /// JTI mới (để thêm vào danh sách sessions).
    pub jti: &'a str,
    /// TTL (giây) cho key mới và sessions.
    pub ttl_secs: u64,
    /// Số session tối đa mỗi subject (đuổi cũ nhất trước).
    pub max_sessions: usize,
}

/// Tham số thu hồi một session refresh (`revoke_refresh_session`, dùng khi sign-out).
#[derive(Debug)]
pub struct RevokeRefreshSession<'a> {
    /// Key refresh cần thu hồi (`refreshToken:{jti}`).
    pub key: &'a str,
    /// Key danh sách session (`sessions:{subject}`).
    pub session_key: &'a str,
    /// JTI cần loại khỏi danh sách sessions (giữ nguyên TTL còn lại).
    pub jti: &'a str,
    /// TTL (giây) dự phòng khi sessions thiếu TTL gốc.
    pub ttl_secs: u64,
}
