use std::sync::Arc;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::{BusinessError, SystemError};

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

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message, err) = match &self {
            Self::Business(err) => {
                let code = err.error_code();
                let msg = match err {
                    BusinessError::UserNotInGroup(user_id) => {
                        let mut params = std::collections::HashMap::new();
                        params.insert("userId", user_id.as_str());
                        crate::utils::i18n::t(code, "vi", Some(&params))
                    }
                    BusinessError::BadRequest(msg)
                    | BusinessError::NotFound(msg)
                    | BusinessError::Conflict(msg)
                    | BusinessError::ValidationError(msg) => msg.clone(),
                    _ => crate::utils::i18n::t(code, "vi", None::<&std::collections::HashMap<&str, &str>>),
                };
                (err.status_code(), msg, None)
            }
            Self::System(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                crate::utils::i18n::t("INTERNAL_SERVER_ERROR", "vi", None::<&std::collections::HashMap<&str, &str>>),
                Some(self),
            ),
        };

        let mut response = (status, Json(ErrorResponse { success: false, message })).into_response();
        if let Some(err) = err {
            response.extensions_mut().insert(Arc::new(err));
        }
        response
    }
}
