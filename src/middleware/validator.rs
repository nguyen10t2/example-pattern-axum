use std::ops::{Deref, DerefMut};

use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Path, Query, Request},
    http::request::Parts,
};
use serde::de::DeserializeOwned;
use tracing::warn;
use validator::Validate;

use crate::errors::{AppError, BusinessError};
use crate::middleware::RequestLang;
use crate::utils::i18n::t_simple;

/// Extractor JSON tự deserialize + validate: sai format → `400`, sai rule → `422`.
/// Hỗ trợ `Deref`/`DerefMut` để dùng trực tiếp như `T`.
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
        let lang = req.extensions().get::<RequestLang>().map(|request_lang| request_lang.0).unwrap_or_default();

        let Json(value) = Json::<T>::from_request(req, state).await.map_err(|e| {
            warn!(error = %e, "invalid JSON request body");
            AppError::Business(BusinessError::BadRequest(t_simple("BAD_REQUEST", lang)))
        })?;

        value.validate().map_err(|e| {
            warn!(error = %e, "request body validation failed");
            AppError::Business(BusinessError::ValidationError(t_simple("VALIDATION_ERROR", lang)))
        })?;

        Ok(Self(value))
    }
}

/// Extractor path param: parse lỗi (vd UUID sai) thì trả `AppError` JSON.
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
        let lang = parts.extensions.get::<RequestLang>().map(|request_lang| request_lang.0).unwrap_or_default();

        let Path(value) = Path::<T>::from_request_parts(parts, state).await.map_err(|e| {
            warn!(error = %e, "invalid path parameter");
            AppError::Business(BusinessError::BadRequest(t_simple("BAD_REQUEST", lang)))
        })?;

        Ok(Self(value))
    }
}

/// Extractor query param: parse lỗi thì trả `AppError` JSON.
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
        let lang = parts.extensions.get::<RequestLang>().map(|request_lang| request_lang.0).unwrap_or_default();

        let Query(value) = Query::<T>::from_request_parts(parts, state).await.map_err(|e| {
            warn!(error = %e, "invalid query parameter");
            AppError::Business(BusinessError::BadRequest(t_simple("BAD_REQUEST", lang)))
        })?;

        Ok(Self(value))
    }
}
