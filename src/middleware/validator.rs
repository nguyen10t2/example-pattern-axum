use std::ops::{Deref, DerefMut};

use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Path, Query, Request},
    http::request::Parts,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::errors::{AppError, BusinessError};

/// Axum custom extractor that automatically deserializes JSON and validates using `validator::Validate`.
///
/// - Deserializes JSON payload (`FromRequest`).
/// - Runs `validator::Validate`.
/// - Rejects malformed JSON with `400 Bad Request`.
/// - Rejects invalid rules with `422 Unprocessable Entity` mapped through `AppError`.
/// - Implements `Deref` and `DerefMut` for zero-overhead ergonomic access.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedJson<T>(pub T);

impl<T> Deref for ValidatedJson<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ValidatedJson<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|e| AppError::Business(BusinessError::BadRequest(e.to_string())))?;

        value.validate().map_err(|e| AppError::Business(BusinessError::ValidationError(e.to_string())))?;

        Ok(Self(value))
    }
}

/// Custom Path extractor that converts URL parameter parse errors (e.g. invalid UUIDs) into JSON `AppError`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedPath<T>(pub T);

impl<T> Deref for ValidatedPath<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ValidatedPath<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T, S> FromRequestParts<S> for ValidatedPath<T>
where
    T: DeserializeOwned + Send + Sync,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(|e| AppError::Business(BusinessError::BadRequest(format!("Invalid path parameter: {e}"))))?;

        Ok(Self(value))
    }
}

/// Custom Query extractor that converts URL query parse errors into JSON `AppError`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidatedQuery<T>(pub T);

impl<T> Deref for ValidatedQuery<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ValidatedQuery<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T, S> FromRequestParts<S> for ValidatedQuery<T>
where
    T: DeserializeOwned + Send + Sync,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|e| AppError::Business(BusinessError::BadRequest(format!("Invalid query parameter: {e}"))))?;

        Ok(Self(value))
    }
}
