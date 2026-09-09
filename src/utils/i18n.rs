use std::{collections::HashMap, hash::BuildHasher};

#[must_use]
pub fn get_language_from_header(accept_language: Option<&str>) -> String {
    let Some(header) = accept_language else {
        return "vi".to_string();
    };
    if header.trim().is_empty() {
        return "vi".to_string();
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
        if code_lower == "en" || code_lower == "vi" {
            return code_lower;
        }
    }

    "vi".to_string()
}

#[must_use]
pub fn t<S: BuildHasher>(key: &str, lang: &str, params: Option<&HashMap<&str, &str, S>>) -> String {
    let template = match lang {
        "en" => translate_en(key),
        _ => translate_vi(key),
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
        _ => "INTERNAL_SERVER_ERROR",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_language_from_header_default() {
        assert_eq!(get_language_from_header(None), "vi");
        assert_eq!(get_language_from_header(Some("")), "vi");
    }

    #[test]
    fn test_get_language_from_header_matched() {
        assert_eq!(get_language_from_header(Some("en-US,en;q=0.9,vi;q=0.8")), "en");
        assert_eq!(get_language_from_header(Some("vi-VN,vi;q=0.9,fr-FR;q=0.8,en;q=0.7")), "vi");
    }

    #[test]
    fn test_get_language_from_header_unsupported_defaults_to_vi() {
        assert_eq!(get_language_from_header(Some("fr-FR,fr;q=0.9")), "vi");
    }

    #[test]
    fn test_translation_en() {
        assert_eq!(t("SUCCESS", "en", None::<&HashMap<&str, &str>>), "Success");
    }

    #[test]
    fn test_translation_vi() {
        assert_eq!(t("SUCCESS", "vi", None::<&HashMap<&str, &str>>), "Thành công");
    }

    #[test]
    fn test_translation_missing_key() {
        assert_eq!(t("MISSING_KEY", "en", None::<&HashMap<&str, &str>>), "INTERNAL_SERVER_ERROR");
    }

    #[test]
    fn test_translation_with_params() {
        let mut params = HashMap::new();
        params.insert("userId", "123");
        assert_eq!(t("USER_NOT_IN_GROUP", "en", Some(&params)), "User 123 is not in this group");

        let mut params_vi = HashMap::new();
        params_vi.insert("userId", "456");
        assert_eq!(t("USER_NOT_IN_GROUP", "vi", Some(&params_vi)), "Người dùng 456 không thuộc nhóm này");
    }
}
