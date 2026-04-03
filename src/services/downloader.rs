use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use futures::StreamExt;
use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use tracing::{error, info, warn};

use crate::models::{download::DownloadStatus, Download};

pub struct Downloader {
    client: Client,
    cancel_flags: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<uuid::Uuid, Arc<AtomicBool>>>>,
    request_timeout_secs: u64,
}

impl Downloader {
    pub fn new(request_timeout_secs: u64) -> Self {
        let timeout = std::time::Duration::from_secs(request_timeout_secs);
        Self {
            client: Client::builder()
                .timeout(timeout)
                .pool_max_idle_per_host(10)
                .tcp_keepalive(std::time::Duration::from_secs(60))
                .tcp_nodelay(true)
                .build()
                .unwrap_or_else(|_| {
                    warn!("Failed to build client with custom config, using default");
                    Client::new()
                }),
            cancel_flags: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            request_timeout_secs,
        }
    }

    pub fn register_download(&self, id: uuid::Uuid) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flags.lock().unwrap().insert(id, flag.clone());
        flag
    }

    pub fn unregister_download(&self, id: uuid::Uuid) {
        self.cancel_flags.lock().unwrap().remove(&id);
    }

    pub fn cancel_download(&self, id: uuid::Uuid) -> bool {
        if let Some(flag) = self.cancel_flags.lock().unwrap().get(&id) {
            flag.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub async fn start_download(
        &self,
        mut download: Download,
        output_dir: PathBuf,
        progress_callback: impl Fn(Download) + Send + 'static,
        semaphore: Arc<tokio::sync::Semaphore>,
    ) -> Result<Download, String> {
        let _permit = semaphore.acquire().await.map_err(|e| e.to_string())?;
        let cancel_flag = self.register_download(download.id);

        download.status = DownloadStatus::Downloading;
        progress_callback(download.clone());

        info!("Starting download: {} -> {} (timeout: {}s)", download.url, download.filename, self.request_timeout_secs);

        let response = match self.client.get(&download.url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to send request: {}", e);
                let mut download = download;
                download.status = DownloadStatus::Failed;
                download.error_message = Some(e.to_string());
                progress_callback(download);
                return Err(e.to_string());
            }
        };

        if !response.status().is_success() {
            let mut download = download;
            download.status = DownloadStatus::Failed;
            download.error_message = Some(format!("HTTP error: {}", response.status()));
            progress_callback(download);
            return Err(format!("HTTP error: {}", response.status()));
        }

        let total_size = response.content_length().unwrap_or(0);
        download.total_bytes = total_size;
        progress_callback(download.clone());

        let file_path = output_dir.join(&download.filename);
        let mut file = match File::create(&file_path).await {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to create file: {}", e);
                let mut download = download;
                download.status = DownloadStatus::Failed;
                download.error_message = Some(e.to_string());
                progress_callback(download);
                return Err(e.to_string());
            }
        };

        let mut downloaded: u64 = 0;
        let mut last_update = Instant::now();
        let mut last_bytes = 0u64;
        let mut speed_bps: u64 = 0;
        let mut last_progress_update = Instant::now();

        let mut stream = response.bytes_stream();
        let mut download = download;

        while let Some(chunk_result) = stream.next().await {
            if cancel_flag.load(Ordering::SeqCst) {
                warn!("Download cancelled: {}", download.id);
                let _ = tokio::fs::remove_file(&file_path).await;
                let download_id = download.id;
                download.status = DownloadStatus::Cancelled;
                download.error_message = Some("Download cancelled by user".to_string());
                let result = download.clone();
                progress_callback(download);
                self.unregister_download(download_id);
                return Ok(result);
            }

            match chunk_result {
                Ok(chunk) => {
                    if let Err(e) = file.write_all(&chunk).await {
                        error!("Write error: {}", e);
                        download.status = DownloadStatus::Failed;
                        download.error_message = Some(e.to_string());
                        progress_callback(download);
                        return Err(e.to_string());
                    }

                    downloaded += chunk.len() as u64;
                    download.downloaded_bytes = downloaded;

                    let now = Instant::now();
                    let elapsed = now.duration_since(last_update).as_secs_f64();
                    if elapsed >= 1.0 {
                        speed_bps = ((downloaded - last_bytes) as f64 / elapsed) as u64;
                        last_update = now;
                        last_bytes = downloaded;
                        download.speed_bps = speed_bps;
                    }

                    let time_since_progress = now.duration_since(last_progress_update).as_secs_f64();
                    if time_since_progress >= 2.0 {
                        last_progress_update = now;
                        progress_callback(download.clone());
                    }
                }
                Err(e) => {
                    error!("Stream error: {}", e);
                    download.status = DownloadStatus::Failed;
                    download.error_message = Some(e.to_string());
                    progress_callback(download);
                    return Err(e.to_string());
                }
            }
        }

        download.status = DownloadStatus::Completed;
        download.downloaded_bytes = downloaded;
        download.file_path = Some(file_path);
        download.speed_bps = 0;
        progress_callback(download.clone());

        info!("Download completed: {} ({} bytes)", download.filename, downloaded);
        self.unregister_download(download.id);

        Ok(download)
    }
}
