use std::sync::Arc;
use std::time::Duration;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::{CreateDownloadRequest, Download, DownloadProgress, DownloadStatus};
use crate::services::Downloader;
use crate::state::AppState;

type SharedState = Arc<RwLock<AppState>>;

#[derive(serde::Serialize)]
struct BatchResult {
    added: usize,
    ids: Vec<Uuid>,
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/health", axum::routing::get(health_check))
        .route("/downloads", get(list_downloads))
        .route("/downloads", post(create_download))
        .route("/downloads/batch", post(create_batch_downloads))
        .route("/downloads/:id", get(get_download))
        .route("/downloads/:id", delete(delete_download))
        .route("/downloads/:id/cancel", post(cancel_download))
        .route("/downloads/:id/retry", post(retry_download))
}

async fn health_check() -> &'static str {
    "OK"
}

async fn list_downloads(State(state): State<SharedState>) -> Json<Vec<DownloadProgress>> {
    let state = state.read().await;
    Json(state.list_downloads().iter().map(|d| DownloadProgress::from(*d)).collect())
}

async fn create_download(
    State(state): State<SharedState>,
    Json(payload): Json<CreateDownloadRequest>,
) -> Result<(StatusCode, Json<DownloadProgress>), StatusCode> {
    let download = Download::new(payload.url.clone());
    let download_id = download.id;

    {
        let mut state = state.write().await;
        state.add_download(download);
    }

    let download_dir = {
        let state = state.read().await;
        state.download_dir.clone()
    };

    let max_concurrent = {
        let state = state.read().await;
        state.max_concurrent
    };

    let downloader = std::sync::Arc::new(Downloader::new());
    let state_clone = state.clone();

    tokio::spawn(async move {
        loop {
            let active_count = {
                let state = state_clone.read().await;
                state.active_count()
            };

            if active_count < max_concurrent {
                break;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let progress_callback = |d: Download| {
            let state = state_clone.clone();
            tokio::spawn(async move {
                let mut state = state.write().await;
                if let Some(download) = state.get_download_mut(d.id) {
                    *download = d;
                }
            });
        };

        let downloader = downloader.as_ref();
        let download = {
            let state = state_clone.read().await;
            state.get_download(download_id).cloned().unwrap()
        };

        let result = downloader.start_download(download, download_dir, progress_callback).await;

        let mut state = state_clone.write().await;
        if let Some(d) = state.get_download_mut(download_id) {
            if let Ok(completed) = result {
                *d = completed;
            }
        }
    });

    let state = state.read().await;
    if let Some(d) = state.get_download(download_id) {
        Ok((StatusCode::CREATED, Json(DownloadProgress::from(d))))
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

async fn get_download(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DownloadProgress>, StatusCode> {
    let state = state.read().await;
    state
        .get_download(id)
        .map(|d| Json(DownloadProgress::from(d)))
        .ok_or(StatusCode::NOT_FOUND)
}

async fn delete_download(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut state = state.write().await;

    if let Some(download) = state.remove_download(id) {
        if let Some(path) = download.file_path {
            let _ = tokio::fs::remove_file(path).await;
        }
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn cancel_download(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut state = state.write().await;

    if let Some(download) = state.get_download_mut(id) {
        download.status = DownloadStatus::Cancelled;
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn retry_download(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<DownloadProgress>), StatusCode> {
    let download = {
        let mut state = state.write().await;
        if let Some(download) = state.get_download_mut(id) {
            if download.status != DownloadStatus::Failed {
                return Err(StatusCode::BAD_REQUEST);
            }
            download.status = DownloadStatus::Queued;
            download.downloaded_bytes = 0;
            download.progress_percent();
            download.error_message = None;
            download.clone()
        } else {
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let download_id = download.id;
    let download_dir = {
        let state = state.read().await;
        state.download_dir.clone()
    };

    let max_concurrent = {
        let state = state.read().await;
        state.max_concurrent
    };

    let downloader = std::sync::Arc::new(Downloader::new());
    let state_clone = state.clone();

    tokio::spawn(async move {
        loop {
            let active_count = {
                let state = state_clone.read().await;
                state.active_count()
            };

            if active_count < max_concurrent {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let progress_callback = |d: Download| {
            let state = state_clone.clone();
            tokio::spawn(async move {
                let mut state = state.write().await;
                if let Some(download) = state.get_download_mut(d.id) {
                    *download = d;
                }
            });
        };

        let downloader = downloader.as_ref();
        let download = {
            let state = state_clone.read().await;
            state.get_download(download_id).cloned().unwrap()
        };

        let result = downloader.start_download(download, download_dir, progress_callback).await;

        let mut state = state_clone.write().await;
        if let Some(d) = state.get_download_mut(download_id) {
            if let Ok(completed) = result {
                *d = completed;
            }
        }
    });

    let state = state.read().await;
    if let Some(d) = state.get_download(download_id) {
        Ok((StatusCode::OK, Json(DownloadProgress::from(d))))
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

async fn create_batch_downloads(
    State(state): State<SharedState>,
    content: String,
) -> Result<(StatusCode, Json<BatchResult>), StatusCode> {
    let urls: Vec<String> = content
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && (line.starts_with("http://") || line.starts_with("https://")))
        .collect();

    let mut ids = Vec::new();

    {
        let mut state = state.write().await;
        for url in urls {
            let download = Download::new(url);
            ids.push(download.id);
            state.add_download(download);
        }
    }

    let download_dir = {
        let state = state.read().await;
        state.download_dir.clone()
    };

    let max_concurrent = {
        let state = state.read().await;
        state.max_concurrent
    };

    let state_clone = state.clone();
    let download_ids = ids.clone();

    tokio::spawn(async move {
        for download_id in download_ids {
            loop {
                let active_count = {
                    let state = state_clone.read().await;
                    state.active_count()
                };

                if active_count < max_concurrent {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            let progress_callback = |d: Download| {
                let state = state_clone.clone();
                tokio::spawn(async move {
                    let mut state = state.write().await;
                    if let Some(download) = state.get_download_mut(d.id) {
                        *download = d;
                    }
                });
            };

            let downloader = std::sync::Arc::new(Downloader::new());
            let downloader = downloader.as_ref();

            let download = {
                let state = state_clone.read().await;
                state.get_download(download_id).cloned()
            };

            if let Some(download) = download {
                let result = downloader.start_download(download, download_dir.clone(), progress_callback).await;

                let mut state = state_clone.write().await;
                if let Some(d) = state.get_download_mut(download_id) {
                    if let Ok(completed) = result {
                        *d = completed;
                    }
                }
            }
        }
    });

    Ok((StatusCode::CREATED, Json(BatchResult {
        added: ids.len(),
        ids,
    })))
}
