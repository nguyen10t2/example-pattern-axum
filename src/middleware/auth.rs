use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header, request::Parts},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{
    errors::{AppError, BusinessError},
    state::AppState,
};

#[derive(Debug, Clone, Copy)]
pub struct AuthUser(pub Uuid);

/// Axum modern middleware using `from_fn` / `from_fn_with_state`.
/// Verifies Bearer token, extracts user ID from JWT claims, and inserts `AuthUser` into request extensions.
pub async fn require_auth(State(state): State<AppState>, mut req: Request, next: Next) -> Result<Response, AppError> {
    let auth_header = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => return Err(AppError::Business(BusinessError::Unauthorized)),
    };

    let claims =
        state.jwt_config.verify_access_token(token).map_err(|_| AppError::Business(BusinessError::Unauthorized))?;

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Business(BusinessError::Unauthorized))?;

    req.extensions_mut().insert(AuthUser(user_id));
    Ok(next.run(req).await)
}

/// Extractor implementation for `AuthUser` that reads from request extensions populated by `require_auth`.
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Self>().copied().ok_or(AppError::Business(BusinessError::Unauthorized))
    }
}
