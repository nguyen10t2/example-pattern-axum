use std::sync::Arc;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::{BusinessError, SystemError, error_codes};
use crate::utils::i18n::{Lang, t};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    System(#[from] SystemError),

    #[error(transparent)]
    Business(#[from] BusinessError),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self::System(SystemError::Database(err))
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(_err: jsonwebtoken::errors::Error) -> Self {
        Self::Business(BusinessError::Unauthorized)
    }
}

impl From<lettre::address::AddressError> for AppError {
    fn from(err: lettre::address::AddressError) -> Self {
        Self::System(SystemError::EmailAddress(err))
    }
}

impl From<lettre::error::Error> for AppError {
    fn from(err: lettre::error::Error) -> Self {
        Self::System(SystemError::EmailBuild(err))
    }
}

impl From<lettre::transport::smtp::Error> for AppError {
    fn from(err: lettre::transport::smtp::Error) -> Self {
        Self::System(SystemError::EmailDelivery(err))
    }
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub code: String,
    pub message: String,
}

/// Mã lỗi ổn định cho client (`error_codes`), không đổi theo ngôn ngữ.
#[must_use]
pub const fn error_code(err: &AppError) -> &'static str {
    match err {
        AppError::Business(err) => err.error_code(),
        AppError::System(_) => error_codes::INTERNAL_SERVER_ERROR,
    }
}

/// Message hiển thị theo ngôn ngữ. Message tự do (validator...) giữ nguyên không dịch.
#[must_use]
pub fn error_message(err: &AppError, lang: Lang) -> String {
    match err {
        AppError::Business(err) => {
            let code = err.error_code();
            match err {
                BusinessError::UserNotInGroup(user_id) => {
                    let mut params = std::collections::HashMap::new();
                    params.insert("userId", user_id.as_str());
                    t(code, lang, Some(&params))
                }
                BusinessError::BadRequest(msg)
                | BusinessError::NotFound(msg)
                | BusinessError::Conflict(msg)
                | BusinessError::ValidationError(msg) => msg.clone(),
                _ => t(code, lang, None::<&std::collections::HashMap<&str, &str>>),
            }
        }
        AppError::System(_) => {
            t(error_codes::INTERNAL_SERVER_ERROR, lang, None::<&std::collections::HashMap<&str, &str>>)
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Ngôn ngữ mặc định (vi); middleware `localize` dựng lại body theo
        // `Accept-Language` khi client yêu cầu ngôn ngữ khác.
        let lang = Lang::Vi;
        let status = match &self {
            Self::Business(err) => err.status_code(),
            Self::System(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let code = error_code(&self).to_string();
        let message = error_message(&self, lang);

        let mut response = (status, Json(ErrorResponse { success: false, code, message })).into_response();
        // Luôn gắn error để middleware `log_errors` (log) và `localize` (dịch lại
        // khi khác ngôn ngữ mặc định) dùng — kể cả lỗi 4xx.
        response.extensions_mut().insert(Arc::new(self));
        response
    }
}
