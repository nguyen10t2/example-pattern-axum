use std::sync::Arc;

use axum::{extract::Request, middleware::Next, response::Response};

use crate::errors::AppError;

/// Log lỗi handler: lỗi hệ thống ở mức error, lỗi client (4xx) ở mức warn.
pub async fn log_errors(request: Request, next: Next) -> Response {
    let response = next.run(request).await;

    if let Some(err) = response.extensions().get::<Arc<AppError>>() {
        match err.as_ref() {
            AppError::System(_) => tracing::error!(?err, "handler error"),
            AppError::Business(_) => tracing::warn!(?err, "handler client error"),
        }
    }

    response
}
