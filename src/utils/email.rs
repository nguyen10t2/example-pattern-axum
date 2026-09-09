use tokio::sync::mpsc;
use tracing::{error, info};

pub struct MailMessage {
    pub to: String,
    pub subject: String,
    pub body: String,
}

#[derive(Clone)]
pub struct Mailer {
    sender: mpsc::Sender<MailMessage>,
}

impl Mailer {
    /// Khởi động worker gửi mail nền (queue bounded = backpressure, handler không block I/O).
    #[must_use]
    pub fn new(buffer: usize) -> Self {
        let (sender, mut receiver) = mpsc::channel::<MailMessage>(buffer);

        tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                deliver(&msg);
            }
        });

        Self { sender }
    }

    /// Đẩy OTP vào queue (queue đóng thì rớt mail và chỉ log).
    pub async fn send_otp(&self, to: &str, otp: &str) {
        let msg = MailMessage {
            to: to.to_string(),
            subject: "Your verification code".to_string(),
            body: format!("Your OTP is {otp}. It expires in a few minutes."),
        };

        if self.sender.send(msg).await.is_err() {
            error!(target: "email", to = %to, "mail queue closed; dropped OTP email");
        }
    }
}

/// Giao mail (stub log; thay bằng SMTP/provider khi production).
///
/// Để sync + không `Result` vì hiện chưa có I/O nào có thể lỗi —
/// thêm `async`/`Result` lại khi cắm SMTP thật.
fn deliver(msg: &MailMessage) {
    // In production, SMTP or a transactional email provider is used here.
    // The worker runs on a dedicated task so request handlers never block on I/O.
    info!(target: "email", to = %msg.to, subject = %msg.subject, "email delivered");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_otp_success() {
        let mailer = Mailer::new(8);
        mailer.send_otp("user@example.com", "123456").await;

        // Give the background worker a moment to drain the queue.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
