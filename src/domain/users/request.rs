use crate::domain::Currency;
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Deserialize)]
pub struct ParamId {
    pub id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct RequestOtpRequest {
    #[validate(email)]
    pub email: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SignUpRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1, max = 255))]
    #[serde(rename = "fullName")]
    pub full_name: String,
    #[validate(length(min = 6))]
    pub password: String,
    #[validate(length(min = 6, max = 6))]
    pub otp: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct SignInRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6))]
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 6))]
    #[serde(rename = "oldPassword")]
    pub old_password: String,
    #[validate(length(min = 6))]
    #[serde(rename = "newPassword")]
    pub new_password: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ForgotPasswordOtpRequest {
    #[validate(email)]
    pub email: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ResetPasswordRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6, max = 6))]
    pub otp: String,
    #[validate(length(min = 6))]
    #[serde(rename = "newPassword")]
    pub new_password: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateUserRequest {
    #[serde(rename = "fullName")]
    pub full_name: Option<String>,
    #[serde(rename = "isActive")]
    pub is_active: Option<bool>,
    pub phone: Option<String>,
    #[serde(rename = "preferredCurrency")]
    pub preferred_currency: Option<Currency>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_up_request_validation() {
        let req = SignUpRequest {
            email: "test@example.com".to_string(),
            full_name: "Test User".to_string(),
            password: "password123".to_string(),
            otp: "123456".to_string(),
        };
        assert!(req.validate().is_ok());

        let invalid_req = SignUpRequest {
            email: "invalid-email".to_string(),
            full_name: "".to_string(),
            password: "123".to_string(),
            otp: "12".to_string(),
        };
        assert!(invalid_req.validate().is_err());
    }
}
