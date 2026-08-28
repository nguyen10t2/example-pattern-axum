use axum::{
    extract::Request,
    http::{HeaderValue, header},
    middleware::Next,
    response::Response,
};

pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(header::HeaderName::from_static("x-content-type-options"), HeaderValue::from_static("nosniff"));
    headers.insert(header::HeaderName::from_static("x-frame-options"), HeaderValue::from_static("DENY"));
    headers.insert(header::HeaderName::from_static("x-xss-protection"), HeaderValue::from_static("1; mode=block"));
    headers.insert(header::STRICT_TRANSPORT_SECURITY, HeaderValue::from_static("max-age=31536000; includeSubDomains"));
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'self'"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));

    response
}
