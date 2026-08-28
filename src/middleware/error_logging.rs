use std::sync::Arc;

use axum::{extract::Request, middleware::Next, response::Response};

use crate::errors::AppError;

pub async fn log_errors(request: Request, next: Next) -> Response {
    let response = next.run(request).await;

    if let Some(err) = response.extensions().get::<Arc<AppError>>() {
        tracing::error!(?err, "handler error");
    }

    response
}
