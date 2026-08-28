use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SuccessResponse<T: Serialize = ()> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T: Serialize> SuccessResponse<T> {
    pub fn ok(data: T) -> Self {
        Self { success: true, message: "OK".to_string(), data: Some(data) }
    }

    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self { success: true, message: message.into(), data: Some(data) }
    }

    pub fn created(data: T, message: impl Into<String>) -> (StatusCode, Self) {
        (StatusCode::CREATED, Self { success: true, message: message.into(), data: Some(data) })
    }
}

impl SuccessResponse<()> {
    pub fn message_only(message: impl Into<String>) -> Self {
        Self { success: true, message: message.into(), data: None }
    }
}

impl<T: Serialize> IntoResponse for SuccessResponse<T> {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}
