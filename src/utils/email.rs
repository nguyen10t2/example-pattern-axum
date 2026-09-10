use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::MultiPart,
    transport::smtp::authentication::Credentials,
};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::{
    config::constants::EMAIL_SMTP_TIMEOUT_SECS,
    errors::AppError,
    utils::{email_template::EmailTemplate, i18n::Lang},
};

pub struct MailMessage {
    pub to: String,
    pub subject: String,
    pub text_body: String,
    pub html_body: String,
}

/// Cấu hình SMTP. Thiếu username/password thì mailer chạy log-only (dev/test).
#[derive(Clone)]
pub struct EmailConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
}

/// Lỗi cấu hình email (fail-fast lúc boot, không phải lỗi runtime).
#[derive(Debug, thiserror::Error)]
pub enum EmailConfigError {
    #[error("incomplete SMTP configuration: {0}")]
    Incomplete(String),
}

impl EmailConfig {
    /// Cấu hình tắt gửi mail (log-only), dùng cho dev/test.
    #[must_use]
    pub const fn disabled() -> Self {
        Self { host: String::new(), port: 0, username: String::new(), password: String::new(), from: String::new() }
    }

    /// Đọc từ env: `SMTP_HOST` (mặc định `smtp.gmail.com`), `SMTP_PORT` (587),
    /// `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".to_string()),
            port: std::env::var("SMTP_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(587),
            username: std::env::var("SMTP_USERNAME").unwrap_or_default(),
            password: std::env::var("SMTP_PASSWORD").unwrap_or_default(),
            from: std::env::var("SMTP_FROM").unwrap_or_default(),
        }
    }

    /// Bật gửi mail khi đủ username + password, hoặc có `from` mà không cần auth
    /// (relay dev kiểu Mailpit).
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        (!self.username.is_empty() && !self.password.is_empty()) || !self.from.is_empty()
    }

    /// Kiểm tra cấu hình đủ để bật mail: thiếu `SMTP_FROM` khi đã có credentials,
    /// hoặc chỉ có 1 trong 2 username/password, thì là lỗi config.
    ///
    /// # Errors
    ///
    /// Trả `Incomplete` khi cấu hình dở dang (fail-fast lúc boot).
    pub fn validate(&self) -> Result<(), EmailConfigError> {
        let has_user = !self.username.is_empty();
        let has_pass = !self.password.is_empty();
        if has_user != has_pass {
            return Err(EmailConfigError::Incomplete(
                "set both SMTP_USERNAME and SMTP_PASSWORD, or neither to disable mail".to_string(),
            ));
        }
        if has_user && self.from.is_empty() {
            return Err(EmailConfigError::Incomplete(
                "SMTP_FROM is required when SMTP credentials are set".to_string(),
            ));
        }
        if self.is_enabled() && self.from.parse::<lettre::message::Mailbox>().is_err() {
            return Err(EmailConfigError::Incomplete("SMTP_FROM is not a valid email address".to_string()));
        }
        Ok(())
    }
}

/// Dựng transport SMTP: port 465 → TLS implicit, 587/25 → STARTTLS, port lạ → plaintext
/// (relay dev kiểu Mailpit, có `warn!` để khỏi nhầm production).
fn build_transport(config: &EmailConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>, lettre::transport::smtp::Error> {
    let mut builder: lettre::transport::smtp::AsyncSmtpTransportBuilder = if config.port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)?
    } else if config.port == 587 || config.port == 25 {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)?
    } else {
        warn!(port = config.port, "non-standard SMTP port; using plaintext (dev relay only)");
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
    };
    builder = builder.port(config.port).timeout(Some(Duration::from_secs(EMAIL_SMTP_TIMEOUT_SECS)));
    if !config.username.is_empty() {
        builder = builder.credentials(Credentials::new(config.username.clone(), config.password.clone()));
    }
    Ok(builder.build())
}

#[derive(Clone)]
pub struct Mailer {
    sender: mpsc::Sender<MailMessage>,
}

impl Mailer {
    /// Khởi động worker gửi mail nền (queue bounded = backpressure, handler không block I/O).
    /// Config lỗi thì log và chạy log-only để boot không chết vì mail (fail-open).
    /// Worker sở hữu transport/địa chỉ gửi; `Mailer` chỉ giữ đầu gửi của queue.
    #[must_use]
    pub fn new(buffer: usize, config: &EmailConfig) -> Self {
        let (sender, mut receiver) = mpsc::channel::<MailMessage>(buffer);

        let transport: Option<AsyncSmtpTransport<Tokio1Executor>> = if config.is_enabled() {
            match build_transport(config) {
                Ok(transport) => {
                    info!(host = %config.host, port = config.port, "SMTP mailer enabled");
                    Some(transport)
                }
                Err(e) => {
                    error!(error = %e, "invalid SMTP configuration; mail delivery disabled");
                    None
                }
            }
        } else {
            warn!("SMTP not configured; mail delivery disabled (log-only mode)");
            None
        };
        let from = config.from.clone();

        tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                if let Err(err) = deliver(transport.as_ref(), &from, &msg).await {
                    error!(target: "email", to = %msg.to, error = %err, "failed to deliver email");
                }
            }
        });

        Self { sender }
    }

    /// Đẩy mail vào queue theo template (queue đóng thì rớt mail và chỉ log).
    ///
    /// Để `async` vì gửi vào queue là I/O bất đồng bộ (lý do duy nhất hàm này async).
    pub async fn send<T: EmailTemplate>(&self, to: &str, template: &T, lang: Lang) {
        let msg = MailMessage {
            to: to.to_string(),
            subject: template.subject(lang),
            text_body: template.body_text(lang),
            html_body: template.body_html(lang),
        };

        if self.sender.send(msg).await.is_err() {
            error!(target: "email", to = %to, "mail queue closed; dropped email");
        }
    }
}

/// Giao mail qua SMTP; log-only khi transport `None`.
///
/// Để `async` vì gửi SMTP là I/O mạng (lý do duy nhất hàm này async).
///
/// # Errors
///
/// Trả lỗi địa chỉ, lỗi dựng message hoặc lỗi gửi SMTP.
async fn deliver(
    transport: Option<&AsyncSmtpTransport<Tokio1Executor>>,
    from: &str,
    msg: &MailMessage,
) -> Result<(), AppError> {
    let Some(transport) = transport else {
        info!(target: "email", to = %msg.to, subject = %msg.subject, "email delivered (log-only mode)");
        return Ok(());
    };
    let email = Message::builder()
        .from(from.parse()?)
        .to(msg.to.parse()?)
        .subject(&msg.subject)
        .multipart(MultiPart::alternative_plain_html(msg.text_body.clone(), msg.html_body.clone()))?;
    transport.send(email).await?;
    info!(target: "email", to = %msg.to, subject = %msg.subject, "email delivered");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_config_disabled_by_default() {
        let config = EmailConfig::disabled();
        assert!(!config.is_enabled());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_email_config_validate_partial() {
        let missing_from = EmailConfig {
            host: "smtp.gmail.com".to_string(),
            port: 587,
            username: "user@gmail.com".to_string(),
            password: "app-password".to_string(),
            from: String::new(),
        };
        assert!(missing_from.validate().is_err());

        let half_auth = EmailConfig { username: "user@gmail.com".to_string(), ..EmailConfig::disabled() };
        assert!(half_auth.validate().is_err());
    }

    #[test]
    fn test_email_config_validate_full_and_relay() {
        let config = EmailConfig {
            host: "smtp.gmail.com".to_string(),
            port: 587,
            username: "user@gmail.com".to_string(),
            password: "app-password".to_string(),
            from: "user@gmail.com".to_string(),
        };
        assert!(config.is_enabled());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_email_config_rejects_invalid_from() {
        let config = EmailConfig {
            host: "smtp.gmail.com".to_string(),
            port: 587,
            username: "user@gmail.com".to_string(),
            password: "app-password".to_string(),
            from: "not-an-email".to_string(),
        };
        assert!(config.is_enabled());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_build_transport_modes() {
        let starttls = EmailConfig { host: "smtp.gmail.com".to_string(), port: 587, ..EmailConfig::disabled() };
        assert!(build_transport(&starttls).is_ok());

        let implicit = EmailConfig { port: 465, ..starttls };
        assert!(build_transport(&implicit).is_ok());

        let plain = EmailConfig { host: "127.0.0.1".to_string(), port: 1025, ..EmailConfig::disabled() };
        assert!(build_transport(&plain).is_ok());
    }

    #[tokio::test]
    async fn test_send_otp_success() {
        use crate::utils::{email_template::OtpEmail, i18n::Lang};

        let mailer = Mailer::new(8, &EmailConfig::disabled());
        mailer.send("user@example.com", &OtpEmail::new("123456".to_string()), Lang::Vi).await;

        // Give the background worker a moment to drain the queue.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
