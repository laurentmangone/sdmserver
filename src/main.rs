mod api;
mod models;
mod services;
mod state;

use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use axum::response::Html;
use axum::response::IntoResponse;

use state::AppState;

async fn index() -> impl IntoResponse {
    match tokio::fs::read_to_string("/app/static/index.html").await {
        Ok(content) => Html(content).into_response(),
        Err(_) => Html("<h1>Super Download Manager Server</h1><p>Error loading index.html</p>").into_response(),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sdmserver=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let download_dir = std::env::var("DOWNLOAD_DIR").unwrap_or_else(|_| "/app/downloads".to_string());
    let max_concurrent = std::env::var("MAX_CONCURRENT_DOWNLOADS")
        .unwrap_or_else(|_| "3".to_string())
        .parse()
        .unwrap_or(3);

    if let Err(e) = std::fs::create_dir_all(&download_dir) {
        eprintln!("Warning: Could not create download directory: {}", e);
    }
    let _ = std::fs::set_permissions(&download_dir, std::os::unix::fs::PermissionsExt::from_mode(0o755));

    let state = Arc::new(RwLock::new(AppState::new(
        download_dir,
        max_concurrent,
    )));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = api::download::router()
        .route("/", axum::routing::get(index))
        .route("/style.css", axum::routing::get_service(ServeDir::new("/app/static")))
        .route("/app.js", axum::routing::get_service(ServeDir::new("/app/static")))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "5900".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    tracing::info!("Server starting on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
