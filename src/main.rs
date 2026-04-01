mod api;
mod models;
mod services;
mod state;

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::signal;
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
    let request_timeout = std::env::var("REQUEST_TIMEOUT")
        .unwrap_or_else(|_| "3600".to_string())
        .parse()
        .unwrap_or(3600);
    let config_dir = std::env::var("CONFIG_DIR").unwrap_or_else(|_| "/app/config".to_string());

    if let Err(e) = std::fs::create_dir_all(&download_dir) {
        eprintln!("Warning: Could not create download directory: {}", e);
    }
    let _ = std::fs::create_dir_all(&config_dir);
    let _ = std::fs::set_permissions(&download_dir, std::os::unix::fs::PermissionsExt::from_mode(0o755));

    let state_file = std::path::PathBuf::from(&config_dir).join("downloads.json");
    let config_file = std::path::PathBuf::from(&config_dir).join("config.json");

    let state = Arc::new(RwLock::new(AppState::new(
        download_dir,
        max_concurrent,
        request_timeout,
        state_file,
        config_file,
    )));

    state.write().await.load_config().await;
    {
        let state = state.read().await;
        tracing::info!("Configuration: download_dir={}, max_concurrent={}, request_timeout={}s", 
            state.download_dir.display(), state.max_concurrent, 3600);
    }
    state.write().await.load_from_file().await;

    let state_clone = state.clone();

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
        .with_state(state.clone());

    let port = std::env::var("PORT").unwrap_or_else(|_| "5900".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    tracing::info!("Server starting on {}", addr);

    let graceful = axum::serve(listener, app);
    let graceful = graceful.with_graceful_shutdown(async {
        match signal::ctrl_c().await {
            Ok(()) => {
                tracing::info!("Received shutdown signal (Ctrl+C)");
            }
            Err(err) => {
                eprintln!("Error listening for shutdown signal: {}", err);
            }
        }
    });

    if let Err(e) = graceful.await {
        eprintln!("Server error: {}", e);
    }

    {
        let state = state_clone.read().await;
        state.save_to_file().await;
    }

    tracing::info!("Server shutdown complete");
}
