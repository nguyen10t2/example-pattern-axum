use std::{convert::Infallible, sync::Arc};

use axum::{
    body::Body,
    extract::{FromRequestParts, Request},
    http::{header, request::Parts},
    middleware::Next,
    response::Response,
};

use crate::{
    errors::{AppError, ErrorResponse, error_code, error_message},
    utils::i18n::Lang,
};

/// Ngôn ngữ của request, middleware [`localize`] nhét vào extensions từ header
/// `Accept-Language` (thiếu thì mặc định `Vi`).
#[derive(Debug, Clone, Copy)]
pub struct RequestLang(pub Lang);

impl<S> FromRequestParts<S> for RequestLang
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    // Giữ `async` vì trait `FromRequestParts` của axum bắt buộc — body không có `.await` nào.
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(parts.extensions.get::<Self>().map(|l| l.0).unwrap_or_default()))
    }
}

/// Đọc `Accept-Language` cho request, dựng lại body lỗi theo đúng ngôn ngữ cho response.
///
/// `IntoResponse` của `AppError` chỉ thấy error (không thấy request) nên luôn render
/// mặc định `Vi`; middleware này sửa lại khi client xin ngôn ngữ khác — status, headers
/// (kể cả cookie) và extensions giữ nguyên.
pub async fn localize(request: Request, next: Next) -> Response {
    let lang = Lang::from_accept_language(request.headers().get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()));
    let mut request = request;
    request.extensions_mut().insert(RequestLang(lang));

    let response = next.run(request).await;
    if lang == Lang::default() {
        return response;
    }
    let Some(err) = response.extensions().get::<Arc<AppError>>() else {
        return response;
    };
    let payload =
        ErrorResponse { success: false, code: error_code(err).to_string(), message: error_message(err, lang) };
    let (mut parts, _old_body) = response.into_parts();
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    let new_body = Body::from(serde_json::to_vec(&payload).unwrap_or_default());
    Response::from_parts(parts, new_body)
}
