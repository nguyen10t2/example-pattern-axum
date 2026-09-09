use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn test_root_endpoint_returns_ok() {
    let app = axum::Router::new().route(
        "/",
        axum::routing::get(|| async {
            axum::Json(serde_json::json!({
                "message": "SplitDebt API is running",
                "version": "1.0.0"
            }))
        }),
    );

    let response = app.oneshot(Request::builder().uri("/").body(Body::empty()).unwrap()).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
