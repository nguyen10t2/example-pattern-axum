use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, MatchedPath},
    http::{HeaderValue, Method, Request, header},
    middleware::from_fn,
    routing::get,
};
use serde_json::{Value, json};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use dsa::{
    domain::{expenses::expense_router, groups::group_router, settlements::settlement_router, users::user_router},
    middleware::{localize, log_errors, security_headers},
    state::AppState,
};

use dsa::config::constants::{DEFAULT_FRONTEND_URL, DEFAULT_HOST, DEFAULT_PORT, MAX_BODY_BYTES};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dsa=debug,tower_http=debug,axum::rejection=trace,lettre=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let addr = format!("{host}:{port}");

    // Fail-fast khi combo cookie sai (SameSite=None mà thiếu Secure): browser từ
    // chối cookie thì auth gãy hoàn toàn — phải chết lúc boot với log rõ ràng.
    if let Err(e) = dsa::domain::users::handle::validate_cookie_env() {
        tracing::error!("Invalid cookie configuration: {e}");
        std::process::exit(1);
    }

    let state = match AppState::from_env().await {
        Ok(state) => state,
        Err(e) => {
            tracing::error!("Failed to initialize application state: {e}");
            std::process::exit(1);
        }
    };

    // Run migrations — fatal if they fail, server must not start on an un-migrated DB
    if let Err(e) = state.migrate().await {
        tracing::error!("Database migration failed: {e}");
        std::process::exit(1);
    }

    let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_else(|_| DEFAULT_FRONTEND_URL.to_string());

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(parse_cors_origins(&frontend_url)))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::ACCEPT_LANGUAGE,
            header::COOKIE,
        ])
        .allow_credentials(true);

    let app = Router::new()
        .nest("/api/users", user_router(state.clone()))
        .nest("/api/groups", group_router(state.clone()))
        .nest("/api/expenses", expense_router(state.clone()))
        .nest("/api/settlements", settlement_router(state.clone()))
        .route("/", get(root_info))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(cors)
        .layer(from_fn(security_headers))
        .layer(from_fn(log_errors))
        .layer(from_fn(localize))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let method = request.method();
                    // Never attach query strings to spans: OAuth callbacks carry
                    // short-lived authorization codes and state in the query.
                    let path = request.uri().path();
                    let matched_path = request.extensions().get::<MatchedPath>().map(MatchedPath::as_str);

                    tracing::info_span!("request", %method, %path, matched_path)
                })
                .on_failure(()),
        )
        .with_state(state);

    info!("Server running on http://{addr}");

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("Failed to bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!("Server error: {e}");
        std::process::exit(1);
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("Failed to install Ctrl+C handler: {e}");
            std::process::exit(1);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(e) => {
                tracing::error!("Failed to install SIGTERM handler: {e}");
                std::process::exit(1);
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => (),
        () = terminate => (),
    }

    info!("Shutdown signal received, draining in-flight connections...");
}

async fn root_info() -> Json<Value> {
    Json(json!({
        "message": "SplitDebt API is running",
        "version": "1.0.0"
    }))
}

/// Parse danh sách origin CORS từ `FRONTEND_URL`, phân tách dấu phẩy để vừa cho
/// FE prod (Cloudflare) vừa giữ localhost dev. Entry rỗng/parse lỗi bị bỏ qua;
/// tất cả lỗi hết thì fallback về [`DEFAULT_FRONTEND_URL`] (fail-closed).
/// Sync vì chỉ split + parse trong RAM, không có I/O.
fn parse_cors_origins(raw: &str) -> Vec<HeaderValue> {
    let origins: Vec<HeaderValue> =
        raw.split(',').map(str::trim).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
    if origins.is_empty() { vec![HeaderValue::from_static(DEFAULT_FRONTEND_URL)] } else { origins }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cors_origins_single() {
        let origins = parse_cors_origins("https://app.example.com");
        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0], HeaderValue::from_static("https://app.example.com"));
    }

    #[test]
    fn test_parse_cors_origins_multiple_with_spaces() {
        let origins = parse_cors_origins("https://app.example.com, http://localhost:5173");
        assert_eq!(origins.len(), 2);
    }

    #[test]
    fn test_parse_cors_origins_ignores_empties_and_falls_back() {
        assert_eq!(parse_cors_origins("").len(), 1);
        assert_eq!(parse_cors_origins(" , ,").len(), 1);
        assert_eq!(parse_cors_origins("")[0], HeaderValue::from_static(DEFAULT_FRONTEND_URL));
    }
}
