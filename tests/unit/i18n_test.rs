use dsa::{
    errors::error_codes,
    utils::i18n::{Lang, t},
};
use std::collections::HashMap;

#[test]
fn test_i18n_english_translation() {
    let msg = t(error_codes::UNAUTHORIZED, Lang::En, None::<&HashMap<&str, &str>>);
    assert_eq!(msg, "Unauthorized");

    let msg2 = t(error_codes::GROUP_NOT_FOUND, Lang::En, None::<&HashMap<&str, &str>>);
    assert_eq!(msg2, "Group not found or deleted");
}

#[test]
fn test_i18n_vietnamese_translation() {
    let msg = t(error_codes::UNAUTHORIZED, Lang::Vi, None::<&HashMap<&str, &str>>);
    assert_eq!(msg, "Không có quyền truy cập");

    let msg2 = t(error_codes::GROUP_NOT_FOUND, Lang::Vi, None::<&HashMap<&str, &str>>);
    assert_eq!(msg2, "Không tìm thấy nhóm");
}

#[test]
fn test_i18n_interpolation() {
    let mut params = HashMap::new();
    params.insert("userId", "user_123");

    let msg_en = t(error_codes::USER_NOT_IN_GROUP, Lang::En, Some(&params));
    assert_eq!(msg_en, "User user_123 is not in this group");

    let msg_vi = t(error_codes::USER_NOT_IN_GROUP, Lang::Vi, Some(&params));
    assert_eq!(msg_vi, "Người dùng user_123 không thuộc nhóm này");
}

#[test]
fn test_extract_language_from_headers() {
    assert_eq!(Lang::from_accept_language(Some("vi-VN,vi;q=0.9,en-US;q=0.8")), Lang::Vi);
    assert_eq!(Lang::from_accept_language(Some("en-US,en;q=0.9")), Lang::En);
    assert_eq!(Lang::from_accept_language(None), Lang::Vi);
}
