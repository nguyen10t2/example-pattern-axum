use axum::{Router, body::Body, http::Request, middleware::from_fn, routing::get};
use tower::ServiceExt;

use dsa::{
    errors::{AppError, BusinessError},
    middleware::localize,
};

async fn boom() -> Result<String, AppError> {
    Err(AppError::Business(BusinessError::UserNotFound("x".to_string())))
}

async fn boom_body(app: Router, accept_language: Option<&str>) -> (u16, serde_json::Value) {
    let mut req = Request::get("/boom").body(Body::empty()).unwrap();
    if let Some(lang) = accept_language {
        req.headers_mut().insert("accept-language", lang.parse().unwrap());
    }
    let res = app.oneshot(req).await.unwrap();
    let status = res.status().as_u16();
    let bytes = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn test_error_defaults_to_vietnamese() {
    let app = Router::new().route("/boom", get(boom)).layer(from_fn(localize));
    let (status, body) = boom_body(app, None).await;
    assert_eq!(status, 404);
    assert_eq!(body["success"], false);
    assert_eq!(body["code"], "USER_NOT_FOUND");
    assert_eq!(body["message"], "Người dùng không tồn tại");
}

#[tokio::test]
async fn test_error_localized_to_english() {
    let app = Router::new().route("/boom", get(boom)).layer(from_fn(localize));
    let (status, body) = boom_body(app, Some("en-US,en;q=0.9")).await;
    assert_eq!(status, 404);
    assert_eq!(body["code"], "USER_NOT_FOUND");
    assert_eq!(body["message"], "User not found");
}
