use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, MatchedPath},
    http::{HeaderValue, Method, Request, header},
    middleware::from_fn,
    routing::get,
};
use serde_json::{Value, json};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use dsa::{
    domain::{expenses::expense_router, groups::group_router, settlements::settlement_router, users::user_router},
    middleware::{log_errors, security_headers},
    state::AppState,
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "dsa=debug,tower_http=debug,axum::rejection=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{host}:{port}");

    let state = AppState::from_env().await;

    // Run migrations — fatal if they fail, server must not start on an un-migrated DB
    if let Err(e) = state.migrate().await {
        tracing::error!("Database migration failed: {e}");
        std::process::exit(1);
    }

    let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());

    let cors = CorsLayer::new()
        .allow_origin(
            frontend_url.parse::<HeaderValue>().unwrap_or_else(|_| HeaderValue::from_static("http://localhost:5173")),
        )
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
        .layer(DefaultBodyLimit::max(1_048_576))
        .layer(cors)
        .layer(from_fn(security_headers))
        .layer(from_fn(log_errors))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let method = request.method();
                    let uri = request.uri();
                    let matched_path = request.extensions().get::<MatchedPath>().map(MatchedPath::as_str);

                    tracing::info_span!("request", %method, %uri, matched_path)
                })
                .on_failure(()),
        )
        .with_state(state);

    info!("Server running on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await.unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, draining in-flight connections...");
}

async fn root_info() -> Json<Value> {
    Json(json!({
        "message": "SplitDebt API is running",
        "version": "1.0.0"
    }))
}
