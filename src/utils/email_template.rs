//! Email templates: layout dùng chung + nội dung từng loại, toàn hàm thuần.
//!
//! Quy ước theo best practice transactional email (Mailgun/Postmark/Resend):
//! table tối đa 600px, CSS inline toàn bộ (Gmail strip `<style>`), font hệ thống,
//! một cột, OTP monospace 32px căn giữa, preheader ẩn, luôn kèm bản text.
//! Không ảnh ngoài, không JS, không font lạ — render được cả khi chặn ảnh.

use crate::utils::i18n::{Lang, t_simple};

/// Một loại email gửi được: tiêu đề + bản text + bản HTML.
///
/// Thêm loại mới = thêm struct + impl trait này (code gửi không đổi).
/// `Send + Sync` để dùng được từ worker nền.
pub trait EmailTemplate: Send + Sync {
    /// Tiêu đề mail theo ngôn ngữ.
    fn subject(&self, lang: Lang) -> String;
    /// Bản text thuần (fallback + chống spam).
    fn body_text(&self, lang: Lang) -> String;
    /// Bản HTML (CSS đã inline).
    fn body_html(&self, lang: Lang) -> String;
}

/// Khung chung: preheader ẩn, header brand, nội dung, footer.
fn email_layout(lang: Lang, title: &str, preheader: &str, content_html: &str) -> String {
    let footer_team = t_simple("EMAIL_FOOTER_TEAM", lang);
    format!(
        "<!doctype html>\
        <html lang=\"{lang}\" xmlns=\"http://www.w3.org/1999/xhtml\">\
        <head>\
        <meta http-equiv=\"Content-Type\" content=\"text/html; charset=UTF-8\" />\
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\
        <meta name=\"color-scheme\" content=\"light\" />\
        <meta name=\"supported-color-schemes\" content=\"light\" />\
        <title>{title}</title>\
        </head>\
        <body style=\"margin:0;padding:0;background-color:#f4f5f7;\">\
        <div style=\"display:none;max-height:0;overflow:hidden;opacity:0;\">{preheader}</div>\
        <table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" style=\"background-color:#f4f5f7;margin:0;padding:24px 0;\">\
        <tr><td align=\"center\">\
        <table role=\"presentation\" width=\"600\" cellpadding=\"0\" cellspacing=\"0\" style=\"width:600px;max-width:600px;background-color:#ffffff;border-radius:8px;\">\
        <tr><td style=\"padding:24px 32px 0 32px;font-family:Arial,Helvetica,sans-serif;font-size:20px;font-weight:bold;color:#4f46e5;\">SplitDebt</td></tr>\
        <tr><td style=\"padding:16px 32px 32px 32px;font-family:Arial,Helvetica,sans-serif;font-size:16px;line-height:24px;color:#1f2937;\">{content_html}</td></tr>\
        <tr><td style=\"padding:0 32px 24px 32px;font-family:Arial,Helvetica,sans-serif;font-size:12px;line-height:18px;color:#9ca3af;\">{footer_team}</td></tr>\
        </table>\
        </td></tr></table>\
        </body></html>",
        lang = lang.code(),
    )
}

/// Email mã OTP (đăng ký / quên mật khẩu). `code` do server sinh, chỉ chữ số.
pub struct OtpEmail {
    code: String,
}

impl OtpEmail {
    /// Tạo template OTP từ mã đã sinh.
    #[must_use]
    pub const fn new(code: String) -> Self {
        Self { code }
    }
}

impl EmailTemplate for OtpEmail {
    fn subject(&self, lang: Lang) -> String {
        t_simple("EMAIL_OTP_SUBJECT", lang)
    }

    fn body_text(&self, lang: Lang) -> String {
        format!(
            "{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n--\n{}",
            t_simple("EMAIL_OTP_HELLO", lang),
            t_simple("EMAIL_OTP_INTRO", lang),
            self.code,
            t_simple("EMAIL_OTP_EXPIRES", lang),
            t_simple("EMAIL_OTP_IGNORE", lang),
            t_simple("EMAIL_FOOTER_TEAM", lang),
        )
    }

    fn body_html(&self, lang: Lang) -> String {
        let content = format!(
            "<p style=\"margin:0 0 16px 0;\">{}</p>\
            <p style=\"margin:0 0 16px 0;\">{}</p>\
            <table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"><tr><td align=\"center\" style=\"padding:8px 0 24px 0;\">\
            <span style=\"display:inline-block;font-family:'Courier New',Courier,monospace;font-size:32px;font-weight:bold;letter-spacing:8px;color:#1f2937;background-color:#f8fafc;border:1px solid #e2e8f0;border-radius:8px;padding:12px 24px;\">{}</span>\
            </td></tr></table>\
            <p style=\"margin:0 0 8px 0;font-size:14px;color:#6b7280;\">{}</p>\
            <p style=\"margin:0;font-size:14px;color:#6b7280;\">{}</p>",
            t_simple("EMAIL_OTP_HELLO", lang),
            t_simple("EMAIL_OTP_INTRO", lang),
            self.code,
            t_simple("EMAIL_OTP_EXPIRES", lang),
            t_simple("EMAIL_OTP_IGNORE", lang),
        );
        email_layout(lang, &self.subject(lang), &t_simple("EMAIL_OTP_INTRO", lang), &content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Template mẫu chứng minh thêm loại mail mới không đụng code gửi.
    struct NoticeEmail {
        headline: String,
    }

    impl EmailTemplate for NoticeEmail {
        fn subject(&self, _lang: Lang) -> String {
            self.headline.clone()
        }

        fn body_text(&self, _lang: Lang) -> String {
            self.headline.clone()
        }

        fn body_html(&self, _lang: Lang) -> String {
            format!("<p>{}</p>", self.headline)
        }
    }

    #[test]
    fn test_otp_subject_both_languages() {
        let mail = OtpEmail::new("123456".to_string());
        assert_eq!(mail.subject(Lang::Vi), "Mã xác thực SplitDebt");
        assert_eq!(mail.subject(Lang::En), "Your SplitDebt verification code");
    }

    #[test]
    fn test_otp_bodies_contain_code_without_placeholders() {
        let mail = OtpEmail::new("123456".to_string());
        for lang in [Lang::Vi, Lang::En] {
            let text = mail.body_text(lang);
            let html = mail.body_html(lang);
            assert!(text.contains("123456"), "{lang:?}");
            assert!(html.contains("123456"), "{lang:?}");
            assert!(!text.contains('{'), "{lang:?}");
            assert!(!html.contains("{code}"), "{lang:?}");
        }
    }

    #[test]
    fn test_otp_html_layout_conventions() {
        let html = OtpEmail::new("123456".to_string()).body_html(Lang::Vi);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("width=\"600\""));
        assert!(html.contains("monospace"));
        assert!(html.contains("display:none"));
        assert!(!html.contains("<style"), "CSS phải inline, không thẻ style");
    }

    #[test]
    fn test_custom_template_plugs_into_trait() {
        let notice = NoticeEmail { headline: "Bảo trì".to_string() };
        assert_eq!(notice.subject(Lang::Vi), "Bảo trì");
        assert!(notice.body_html(Lang::Vi).contains("Bảo trì"));
    }
}
