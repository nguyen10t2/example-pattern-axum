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

use dsa::config::constants::{DEFAULT_FRONTEND_URL, DEFAULT_HOST, DEFAULT_PORT, MAX_BODY_BYTES};

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

    let host = std::env::var("HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let addr = dbg!(format!("{host}:{port}"));

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
        .allow_origin(
            frontend_url.parse::<HeaderValue>().unwrap_or_else(|_| HeaderValue::from_static(DEFAULT_FRONTEND_URL)),
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
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
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

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("Failed to bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await {
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
