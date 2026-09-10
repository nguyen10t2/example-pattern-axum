use std::{collections::HashMap, hash::BuildHasher};

/// Ngôn ngữ response. Mặc định `Vi` khi client không gửi `Accept-Language`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    /// Tiếng Anh (`en`).
    En,
    /// Tiếng Việt (mặc định).
    #[default]
    Vi,
}

impl Lang {
    /// Mã ngôn ngữ cho thuộc tính HTML (`vi`/`en`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Vi => "vi",
        }
    }

    /// Đoán ngôn ngữ từ header `Accept-Language` (ưu tiên q cao nhất, fallback `Vi`).
    #[must_use]
    pub fn from_accept_language(accept_language: Option<&str>) -> Self {
        let Some(header) = accept_language else {
            return Self::Vi;
        };
        if header.trim().is_empty() {
            return Self::Vi;
        }

        let mut langs: Vec<(&str, f32)> = header
            .split(',')
            .filter_map(|lang| {
                let mut parts = lang.split(';');
                let code = parts.next()?.trim();
                if code.is_empty() {
                    return None;
                }
                let q = parts
                    .next()
                    .and_then(|q_part| q_part.trim().strip_prefix("q="))
                    .and_then(|q_val| q_val.parse::<f32>().ok())
                    .unwrap_or(1.0);
                let code_prefix = if code.len() >= 2 { &code[..2] } else { code };
                Some((code_prefix, q))
            })
            .collect();

        langs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (code, _) in langs {
            let code_lower = code.to_lowercase();
            if code_lower == "en" {
                return Self::En;
            }
            if code_lower == "vi" {
                return Self::Vi;
            }
        }

        Self::Vi
    }
}

/// Dịch message không placeholder (tiện cho success messages).
#[must_use]
pub fn t_simple(key: &str, lang: Lang) -> String {
    t(key, lang, None::<&HashMap<&str, &str>>)
}

/// Dịch message theo key + ngôn ngữ, thay `{placeholder}` từ params.
#[must_use]
pub fn t<S: BuildHasher>(key: &str, lang: Lang, params: Option<&HashMap<&str, &str, S>>) -> String {
    let template = match lang {
        Lang::En => translate_en(key),
        Lang::Vi => translate_vi(key),
    };

    let Some(params_map) = params else {
        return template.to_string();
    };

    let mut text = template.to_string();
    for (k, v) in params_map {
        let placeholder = format!("{{{k}}}");
        text = text.replace(&placeholder, v);
    }
    text
}

fn translate_en(key: &str) -> &'static str {
    match key {
        "USER_NOT_FOUND" => "User not found",
        "USER_ALREADY_EXISTS" => "Email already in use",
        "INVALID_OTP" => "Invalid or expired OTP",
        "INVALID_CREDENTIALS" => "Invalid email or password",
        "INVALID_INVITE_CODE" => "Invalid invite code or group deleted",
        "GROUP_NOT_FOUND" => "Group not found or deleted",
        "EXPENSE_NOT_FOUND" => "Expense not found",
        "SETTLEMENT_NOT_FOUND" => "Settlement not found",
        "PAYER_NOT_IN_GROUP" => "Payer is not in this group",
        "USER_NOT_IN_GROUP" => "User {userId} is not in this group",
        "INVALID_SESSION" => "Invalid session",
        "INVALID_OLD_PASSWORD" => "Old password is incorrect",
        "TOO_MANY_REQUESTS" => "Too many requests, please try again later.",
        "INTERNAL_SERVER_ERROR" => "Internal Server Error",
        "NOT_FOUND" => "Not Found",
        "VALIDATION_ERROR" => "Validation Error",
        "SUCCESS" => "Success",
        "SENDER_RECEIVER_NOT_IN_GROUP" => "Sender or receiver is not in this group",
        "UNAUTHORIZED" => "Unauthorized",
        "FORBIDDEN" => "Forbidden",
        "ADMIN_REQUIRED" => "Admin required",
        "DELETE_PERMISSION_DENIED" => "Delete permission denied",
        "USER_ALREADY_IN_GROUP" => "User is already a member of this group",
        "BAD_REQUEST" => "Bad request",
        "CONFLICT" => "Conflict",
        "EMAIL_NOT_VERIFIED" => "Email not verified",
        "NOT_GROUP_MEMBER" => "You are not in this group",
        "OTP_SENT" => "OTP sent",
        "USER_CREATED" => "User created successfully",
        "PASSWORD_RESET" => "Password reset successfully",
        "SIGNED_IN" => "Signed in successfully",
        "TOKEN_REFRESHED" => "Token refreshed successfully",
        "SIGNED_OUT" => "Signed out successfully",
        "USER_FOUND" => "User found",
        "USER_UPDATED" => "User updated successfully",
        "PASSWORD_CHANGED" => "Password changed successfully",
        "GROUPS_RETRIEVED" => "Groups retrieved successfully",
        "GROUP_CREATED" => "Group created successfully",
        "GROUP_JOINED" => "Joined group successfully",
        "GROUP_FOUND" => "Group found",
        "GROUP_SUMMARY_RETRIEVED" => "Group summary retrieved successfully",
        "MEMBER_ADDED" => "Member added successfully",
        "MEMBERS_RETRIEVED" => "Members retrieved successfully",
        "GROUP_DELETED" => "Group deleted successfully",
        "EXPENSE_CREATED" => "Expense created successfully",
        "EXPENSE_FOUND" => "Expense found",
        "EXPENSES_RETRIEVED" => "Expenses for group retrieved successfully",
        "EXPENSE_DELETED" => "Expense deleted successfully",
        "SETTLEMENT_RECORDED" => "Settlement recorded successfully",
        "SETTLEMENT_FOUND" => "Settlement found",
        "SETTLEMENTS_RETRIEVED" => "Settlements for group retrieved successfully",
        "SETTLEMENT_CANCELLED" => "Settlement cancelled successfully",
        "EMAIL_OTP_SUBJECT" => "Your SplitDebt verification code",
        "EMAIL_OTP_HELLO" => "Hello,",
        "EMAIL_OTP_INTRO" => "Use the code below to continue:",
        "EMAIL_OTP_EXPIRES" => "This code expires in 2 minutes.",
        "EMAIL_OTP_IGNORE" => "If you didn't request this, please ignore this email.",
        "EMAIL_FOOTER_TEAM" => "The SplitDebt Team",
        _ => "INTERNAL_SERVER_ERROR",
    }
}

fn translate_vi(key: &str) -> &'static str {
    match key {
        "USER_NOT_FOUND" => "Người dùng không tồn tại",
        "USER_ALREADY_EXISTS" => "Email đã được sử dụng",
        "INVALID_OTP" => "Mã xác thực không chính xác hoặc đã hết hạn",
        "INVALID_CREDENTIALS" => "Email hoặc mật khẩu không chính xác",
        "INVALID_INVITE_CODE" => "Mã mời không hợp lệ hoặc nhóm đã bị xóa",
        "GROUP_NOT_FOUND" => "Không tìm thấy nhóm",
        "EXPENSE_NOT_FOUND" => "Hóa đơn không tồn tại",
        "SETTLEMENT_NOT_FOUND" => "Khoản thanh toán không tồn tại",
        "PAYER_NOT_IN_GROUP" => "Người trả tiền không thuộc nhóm này",
        "USER_NOT_IN_GROUP" => "Người dùng {userId} không thuộc nhóm này",
        "INVALID_SESSION" => "Phiên đăng nhập không hợp lệ",
        "INVALID_OLD_PASSWORD" => "Mật khẩu cũ không chính xác",
        "TOO_MANY_REQUESTS" => "Quá nhiều yêu cầu, vui lòng thử lại sau.",
        "INTERNAL_SERVER_ERROR" => "Lỗi máy chủ nội bộ",
        "NOT_FOUND" => "Không tìm thấy",
        "VALIDATION_ERROR" => "Lỗi xác thực dữ liệu",
        "SUCCESS" => "Thành công",
        "SENDER_RECEIVER_NOT_IN_GROUP" => "Người gửi hoặc người nhận không thuộc nhóm này",
        "UNAUTHORIZED" => "Không có quyền truy cập",
        "FORBIDDEN" => "Bị cấm truy cập",
        "ADMIN_REQUIRED" => "Yêu cầu quyền quản trị viên",
        "DELETE_PERMISSION_DENIED" => "Không có quyền xóa",
        "USER_ALREADY_IN_GROUP" => "Người dùng đã thuộc nhóm này",
        "BAD_REQUEST" => "Yêu cầu không hợp lệ",
        "CONFLICT" => "Xung đột dữ liệu",
        "EMAIL_NOT_VERIFIED" => "Email chưa được xác thực",
        "NOT_GROUP_MEMBER" => "Bạn không thuộc nhóm này",
        "OTP_SENT" => "Đã gửi mã OTP",
        "USER_CREATED" => "Tạo tài khoản thành công",
        "PASSWORD_RESET" => "Đặt lại mật khẩu thành công",
        "SIGNED_IN" => "Đăng nhập thành công",
        "TOKEN_REFRESHED" => "Đã cấp token mới",
        "SIGNED_OUT" => "Đăng xuất thành công",
        "USER_FOUND" => "Tìm thấy người dùng",
        "USER_UPDATED" => "Cập nhật thành công",
        "PASSWORD_CHANGED" => "Đổi mật khẩu thành công",
        "GROUPS_RETRIEVED" => "Lấy danh sách nhóm thành công",
        "GROUP_CREATED" => "Tạo nhóm thành công",
        "GROUP_JOINED" => "Tham gia nhóm thành công",
        "GROUP_FOUND" => "Tìm thấy nhóm",
        "GROUP_SUMMARY_RETRIEVED" => "Lấy tổng hợp nhóm thành công",
        "MEMBER_ADDED" => "Thêm thành viên thành công",
        "MEMBERS_RETRIEVED" => "Lấy danh sách thành viên thành công",
        "GROUP_DELETED" => "Xóa nhóm thành công",
        "EXPENSE_CREATED" => "Tạo chi tiêu thành công",
        "EXPENSE_FOUND" => "Tìm thấy chi tiêu",
        "EXPENSES_RETRIEVED" => "Lấy danh sách chi tiêu thành công",
        "EXPENSE_DELETED" => "Xóa chi tiêu thành công",
        "SETTLEMENT_RECORDED" => "Ghi nhận trả nợ thành công",
        "SETTLEMENT_FOUND" => "Tìm thấy khoản trả nợ",
        "SETTLEMENTS_RETRIEVED" => "Lấy danh sách trả nợ thành công",
        "SETTLEMENT_CANCELLED" => "Hủy trả nợ thành công",
        "EMAIL_OTP_SUBJECT" => "Mã xác thực SplitDebt",
        "EMAIL_OTP_HELLO" => "Xin chào,",
        "EMAIL_OTP_INTRO" => "Dùng mã dưới đây để tiếp tục:",
        "EMAIL_OTP_EXPIRES" => "Mã hết hạn sau 2 phút.",
        "EMAIL_OTP_IGNORE" => "Nếu bạn không yêu cầu, hãy bỏ qua email này.",
        "EMAIL_FOOTER_TEAM" => "Đội ngũ SplitDebt",
        _ => "INTERNAL_SERVER_ERROR",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lang_from_header_default() {
        assert_eq!(Lang::from_accept_language(None), Lang::Vi);
        assert_eq!(Lang::from_accept_language(Some("")), Lang::Vi);
    }

    #[test]
    fn test_lang_from_header_matched() {
        assert_eq!(Lang::from_accept_language(Some("en-US,en;q=0.9,vi;q=0.8")), Lang::En);
        assert_eq!(Lang::from_accept_language(Some("vi-VN,vi;q=0.9,fr-FR;q=0.8,en;q=0.7")), Lang::Vi);
    }

    #[test]
    fn test_lang_from_header_unsupported_defaults_to_vi() {
        assert_eq!(Lang::from_accept_language(Some("fr-FR,fr;q=0.9")), Lang::Vi);
    }

    #[test]
    fn test_translation_en() {
        assert_eq!(t("SUCCESS", Lang::En, None::<&HashMap<&str, &str>>), "Success");
    }

    #[test]
    fn test_translation_vi() {
        assert_eq!(t("SUCCESS", Lang::Vi, None::<&HashMap<&str, &str>>), "Thành công");
    }

    #[test]
    fn test_translation_missing_key() {
        assert_eq!(t("MISSING_KEY", Lang::En, None::<&HashMap<&str, &str>>), "INTERNAL_SERVER_ERROR");
    }

    #[test]
    fn test_translation_with_params() {
        let mut params = HashMap::new();
        params.insert("userId", "123");
        assert_eq!(t("USER_NOT_IN_GROUP", Lang::En, Some(&params)), "User 123 is not in this group");

        let mut params_vi = HashMap::new();
        params_vi.insert("userId", "456");
        assert_eq!(t("USER_NOT_IN_GROUP", Lang::Vi, Some(&params_vi)), "Người dùng 456 không thuộc nhóm này");
    }

    #[test]
    fn test_all_error_codes_resolve_both_languages() {
        use crate::errors::error_codes;
        let codes = [
            error_codes::UNAUTHORIZED,
            error_codes::FORBIDDEN,
            error_codes::INVALID_CREDENTIALS,
            error_codes::USER_ALREADY_EXISTS,
            error_codes::USER_NOT_FOUND,
            error_codes::INVALID_OTP,
            error_codes::INVALID_SESSION,
            error_codes::EMAIL_NOT_VERIFIED,
            error_codes::INVALID_OLD_PASSWORD,
            error_codes::GROUP_NOT_FOUND,
            error_codes::NOT_GROUP_MEMBER,
            error_codes::ADMIN_REQUIRED,
            error_codes::INVALID_INVITE_CODE,
            error_codes::PAYER_NOT_IN_GROUP,
            error_codes::USER_NOT_IN_GROUP,
            error_codes::USER_ALREADY_IN_GROUP,
            error_codes::EXPENSE_NOT_FOUND,
            error_codes::SETTLEMENT_NOT_FOUND,
            error_codes::DELETE_PERMISSION_DENIED,
            error_codes::BAD_REQUEST,
            error_codes::NOT_FOUND,
            error_codes::VALIDATION_ERROR,
            error_codes::TOO_MANY_REQUESTS,
            error_codes::SUCCESS,
            error_codes::CONFLICT,
        ];
        for code in codes {
            assert_ne!(t(code, Lang::En, None::<&HashMap<&str, &str>>), "INTERNAL_SERVER_ERROR", "{code}");
            assert_ne!(t(code, Lang::Vi, None::<&HashMap<&str, &str>>), "Lỗi máy chủ nội bộ", "{code}");
        }
    }

    #[test]
    fn test_all_success_keys_resolve_both_languages() {
        let keys = [
            "OTP_SENT",
            "USER_CREATED",
            "PASSWORD_RESET",
            "SIGNED_IN",
            "TOKEN_REFRESHED",
            "SIGNED_OUT",
            "USER_FOUND",
            "USER_UPDATED",
            "PASSWORD_CHANGED",
            "GROUPS_RETRIEVED",
            "GROUP_CREATED",
            "GROUP_JOINED",
            "GROUP_FOUND",
            "GROUP_SUMMARY_RETRIEVED",
            "MEMBER_ADDED",
            "MEMBERS_RETRIEVED",
            "GROUP_DELETED",
            "EXPENSE_CREATED",
            "EXPENSE_FOUND",
            "EXPENSES_RETRIEVED",
            "EXPENSE_DELETED",
            "SETTLEMENT_RECORDED",
            "SETTLEMENT_FOUND",
            "SETTLEMENTS_RETRIEVED",
            "SETTLEMENT_CANCELLED",
            "EMAIL_OTP_SUBJECT",
            "EMAIL_OTP_HELLO",
            "EMAIL_OTP_INTRO",
            "EMAIL_OTP_EXPIRES",
            "EMAIL_OTP_IGNORE",
            "EMAIL_FOOTER_TEAM",
        ];
        for key in keys {
            assert_ne!(t(key, Lang::En, None::<&HashMap<&str, &str>>), "INTERNAL_SERVER_ERROR", "{key}");
            assert_ne!(t(key, Lang::Vi, None::<&HashMap<&str, &str>>), "Lỗi máy chủ nội bộ", "{key}");
        }
    }
}
