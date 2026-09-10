use std::time::Duration;

use uuid::Uuid;

use dsa::utils::{
    email::{EmailConfig, Mailer},
    email_template::OtpEmail,
    i18n::Lang,
};

fn mailpit_config() -> EmailConfig {
    EmailConfig {
        host: std::env::var("MAILPIT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
        port: 1025,
        username: String::new(),
        password: String::new(),
        from: "test@example.com".to_string(),
    }
}

fn mailpit_api() -> String {
    std::env::var("MAILPIT_API").unwrap_or_else(|_| "http://127.0.0.1:8025".to_string())
}

#[tokio::test]
#[ignore = "requires Mailpit (podman run -d -p 1025:1025 -p 8025:8025 axllent/mailpit)"]
async fn test_smtp_delivery_via_mailpit() {
    let config = mailpit_config();
    config.validate().unwrap();

    let recipient = format!("user-{}@example.com", Uuid::now_v7());
    Mailer::new(8, &config).send(&recipient, &OtpEmail::new("123456".to_string()), Lang::Vi).await;

    // Poll Mailpit API cho tới khi mail tới (worker chạy nền nên cần chờ).
    let client = reqwest::Client::new();
    let mut landed_id: Option<String> = None;
    for _ in 0..25 {
        let messages: serde_json::Value =
            client.get(format!("{}/api/v1/messages", mailpit_api())).send().await.unwrap().json().await.unwrap();
        landed_id = messages["messages"].as_array().and_then(|list| {
            list.iter()
                .filter(|m| {
                    m["To"].as_array().is_some_and(|to| to.iter().any(|t| t["Address"] == recipient))
                        && m["Subject"].as_str().unwrap_or_default().contains("Mã xác thực")
                })
                .find_map(|m| m["ID"].as_str().map(str::to_string))
        });
        if landed_id.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let id = landed_id.unwrap_or_else(|| panic!("OTP email to {recipient} never arrived in Mailpit"));

    // Bản HTML đầy đủ phải chứa mã OTP trong layout.
    let full: serde_json::Value =
        client.get(format!("{}/api/v1/message/{}", mailpit_api(), id)).send().await.unwrap().json().await.unwrap();
    assert!(full["HTML"].as_str().unwrap_or_default().contains("123456"));
    assert!(full["HTML"].as_str().unwrap_or_default().contains("<!doctype html>"));
}
