use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use tokio::sync::RwLock;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::models::{CreateDownloadRequest, Download, DownloadProgress, DownloadStatus};
use crate::state::AppState;

type SharedState = Arc<RwLock<AppState>>;

#[derive(serde::Serialize)]
struct BatchResult {
    added: usize,
    ids: Vec<Uuid>,
}

#[derive(serde::Serialize)]
struct Settings {
    max_concurrent: usize,
}

#[derive(serde::Deserialize)]
struct UpdateSettings {
    max_concurrent: usize,
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/api/health", axum::routing::get(health_check))
        .route("/api/settings", get(get_settings))
        .route("/api/settings", post(update_settings))
        .route("/api/downloads", get(list_downloads))
        .route("/api/downloads", post(create_download))
        .route("/api/downloads/batch", post(create_batch_downloads))
        .route("/api/downloads/:id", get(get_download))
        .route("/api/downloads/:id", delete(delete_download))
        .route("/api/downloads/:id/file", delete(delete_download_with_file))
        .route("/api/downloads/:id/cancel", post(cancel_download))
        .route("/api/downloads/:id/retry", post(retry_download))
}

async fn health_check() -> &'static str {
    "OK"
}

async fn get_settings(State(state): State<SharedState>) -> Json<Settings> {
    let state = state.read().await;
    Json(Settings {
        max_concurrent: state.max_concurrent,
    })
}

async fn update_settings(
    State(state): State<SharedState>,
    Json(payload): Json<UpdateSettings>,
) -> Result<Json<Settings>, StatusCode> {
    if payload.max_concurrent < 1 || payload.max_concurrent > 20 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut state = state.write().await;
    state.max_concurrent = payload.max_concurrent;
    state.reload_semaphore();
    state.save_config().await;

    tracing::info!("Settings updated: max_concurrent={}", state.max_concurrent);

    Ok(Json(Settings {
        max_concurrent: state.max_concurrent,
    }))
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
        state.add_download(download.clone());
        state.save_to_file().await;
    }

    let downloader = {
        let state = state.read().await;
        state.downloader.clone()
    };
    let semaphore = {
        let state = state.read().await;
        state.download_semaphore.clone()
    };
    let state_clone = state.clone();

    tokio::spawn(async move {
        let permit = semaphore.acquire().await.unwrap();

        let download = {
            let state = state_clone.read().await;
            state.get_download(download_id).cloned().unwrap()
        };
        let download_dir = {
            let state = state_clone.read().await;
            state.download_dir.clone()
        };

        let state_for_callback = state_clone.clone();
        let progress_callback = move |d: Download| {
            let state_inner = state_for_callback.clone();
            let downloaded = d.clone();
            tokio::spawn(async move {
                let mut state = state_inner.write().await;
                if let Some(download) = state.get_download_mut(downloaded.id) {
                    *download = downloaded;
                }
                state.save_to_file().await;
            });
        };

        let result = downloader.start_download(download, download_dir, progress_callback, permit).await;

        let mut state = state_clone.write().await;
        if let Some(d) = state.get_download_mut(download_id) {
            if let Ok(completed) = result {
                *d = completed;
            }
        }
        state.save_to_file().await;
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

    if state.remove_download(id).is_some() {
        state.save_to_file().await;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn delete_download_with_file(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut state = state.write().await;

    if let Some(download) = state.remove_download(id) {
        if let Some(path) = download.file_path {
            let _ = tokio::fs::remove_file(path).await;
        }
        state.save_to_file().await;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn cancel_download(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let downloader = {
        let state = state.read().await;
        state.downloader.clone()
    };

    downloader.cancel_download(id);

    let mut state = state.write().await;

    if let Some(download) = state.get_download_mut(id) {
        download.status = DownloadStatus::Cancelled;
        state.save_to_file().await;
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
            let result = download.clone();
            state.save_to_file().await;
            result
        } else {
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let download_id = download.id;
    let downloader = {
        let state = state.read().await;
        state.downloader.clone()
    };
    let semaphore = {
        let state = state.read().await;
        state.download_semaphore.clone()
    };
    let state_clone = state.clone();

    tokio::spawn(async move {
        let permit = semaphore.acquire().await.unwrap();

        let download = {
            let state = state_clone.read().await;
            state.get_download(download_id).cloned().unwrap()
        };
        let download_dir = {
            let state = state_clone.read().await;
            state.download_dir.clone()
        };

        let state_for_callback = state_clone.clone();
        let progress_callback = move |d: Download| {
            let state_inner = state_for_callback.clone();
            let downloaded = d.clone();
            tokio::spawn(async move {
                let mut state = state_inner.write().await;
                if let Some(download) = state.get_download_mut(downloaded.id) {
                    *download = downloaded;
                }
                state.save_to_file().await;
            });
        };

        let result = downloader.start_download(download, download_dir, progress_callback, permit).await;

        let mut state = state_clone.write().await;
        if let Some(d) = state.get_download_mut(download_id) {
            if let Ok(completed) = result {
                *d = completed;
            }
        }
        state.save_to_file().await;
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
        state.save_to_file().await;
    }

    let downloader = {
        let state = state.read().await;
        state.downloader.clone()
    };
    let semaphore = {
        let state = state.read().await;
        state.download_semaphore.clone()
    };

    let state_clone = state.clone();
    let download_ids = ids.clone();

    tokio::spawn(async move {
        let mut handles = Vec::new();
        
        for download_id in download_ids {
            let semaphore = {
                let state = state_clone.read().await;
                state.download_semaphore.clone()
            };
            let downloader = {
                let state = state_clone.read().await;
                state.downloader.clone()
            };
            let download_dir = {
                let state = state_clone.read().await;
                state.download_dir.clone()
            };
            let state_for_callback = state_clone.clone();
            let download_id_for_handler = download_id;

            let handle = tokio::spawn(async move {
                let permit = semaphore.acquire().await.unwrap();

                let download = {
                    let state = state_for_callback.read().await;
                    state.get_download(download_id_for_handler).cloned()
                };

                if download.is_none() {
                    return;
                }

                let download_for_callback = download.unwrap();
                let state_for_callback2 = state_for_callback.clone();

                let progress_callback = move |d: Download| {
                    let state_inner = state_for_callback2.clone();
                    let downloaded = d.clone();
                    tokio::spawn(async move {
                        let mut state = state_inner.write().await;
                        if let Some(download) = state.get_download_mut(downloaded.id) {
                            *download = downloaded;
                        }
                        state.save_to_file().await;
                    });
                };

                let result = downloader.start_download(download_for_callback, download_dir, progress_callback, permit).await;

                let mut state = state_for_callback.write().await;
                if let Some(d) = state.get_download_mut(download_id_for_handler) {
                    if let Ok(completed) = result {
                        *d = completed;
                    }
                }
                state.save_to_file().await;
            });
            
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }
    });

    Ok((StatusCode::CREATED, Json(BatchResult {
        added: ids.len(),
        ids,
    })))
}
