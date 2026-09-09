use dsa::{
    errors::error_codes,
    utils::i18n::{get_language_from_header, t},
};
use std::collections::HashMap;

#[test]
fn test_i18n_english_translation() {
    let msg = t(error_codes::UNAUTHORIZED, "en", None::<&HashMap<&str, &str>>);
    assert_eq!(msg, "Unauthorized");

    let msg2 = t(error_codes::GROUP_NOT_FOUND, "en", None::<&HashMap<&str, &str>>);
    assert_eq!(msg2, "Group not found or deleted");
}

#[test]
fn test_i18n_vietnamese_translation() {
    let msg = t(error_codes::UNAUTHORIZED, "vi", None::<&HashMap<&str, &str>>);
    assert_eq!(msg, "Không có quyền truy cập");

    let msg2 = t(error_codes::GROUP_NOT_FOUND, "vi", None::<&HashMap<&str, &str>>);
    assert_eq!(msg2, "Không tìm thấy nhóm");
}

#[test]
fn test_i18n_interpolation() {
    let mut params = HashMap::new();
    params.insert("userId", "user_123");

    let msg_en = t(error_codes::USER_NOT_IN_GROUP, "en", Some(&params));
    assert_eq!(msg_en, "User user_123 is not in this group");

    let msg_vi = t(error_codes::USER_NOT_IN_GROUP, "vi", Some(&params));
    assert_eq!(msg_vi, "Người dùng user_123 không thuộc nhóm này");
}

#[test]
fn test_extract_language_from_headers() {
    assert_eq!(get_language_from_header(Some("vi-VN,vi;q=0.9,en-US;q=0.8")), "vi");
    assert_eq!(get_language_from_header(Some("en-US,en;q=0.9")), "en");
    assert_eq!(get_language_from_header(None), "vi");
}
