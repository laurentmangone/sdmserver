use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;
use tokio::sync::{mpsc, Semaphore};
use tracing::{error, info, warn};
use crate::models::download::Download;
use crate::models::DownloadStatus;
use crate::services::Downloader;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub max_concurrent: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 3,
        }
    }
}

pub struct AppState {
    pub downloads: HashMap<Uuid, Download>,
    pub download_dir: PathBuf,
    pub max_concurrent: usize,
    pub cancel_tx: mpsc::Sender<Uuid>,
    pub downloader: Arc<Downloader>,
    pub download_semaphore: Arc<Semaphore>,
    state_file: PathBuf,
    config_file: PathBuf,
}

impl AppState {
    pub fn new(download_dir: String, max_concurrent: usize, request_timeout_secs: u64, state_file: PathBuf, config_file: PathBuf) -> Self {
        let (tx, _rx) = mpsc::channel::<Uuid>(100);
        Self {
            downloads: HashMap::new(),
            download_dir: PathBuf::from(download_dir),
            max_concurrent,
            cancel_tx: tx,
            downloader: Arc::new(Downloader::new(request_timeout_secs)),
            download_semaphore: Arc::new(Semaphore::new(max_concurrent)),
            state_file,
            config_file,
        }
    }

    pub async fn load_config(&mut self) {
        if !self.config_file.exists() {
            info!("No config file found, using defaults");
            return;
        }

        match tokio::fs::read_to_string(&self.config_file).await {
            Ok(content) => {
                match serde_json::from_str::<AppConfig>(&content) {
                    Ok(config) => {
                        self.max_concurrent = config.max_concurrent;
                        info!("Loaded config: max_concurrent={}", self.max_concurrent);
                    }
                    Err(e) => {
                        warn!("Failed to parse config file: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read config file: {}", e);
            }
        }
    }

    pub async fn save_config(&self) {
        let config = AppConfig {
            max_concurrent: self.max_concurrent,
        };
        match serde_json::to_string_pretty(&config) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write(&self.config_file, json).await {
                    error!("Failed to save config: {}", e);
                } else {
                    info!("Config saved to {:?}", self.config_file);
                }
            }
            Err(e) => {
                error!("Failed to serialize config: {}", e);
            }
        }
    }

    pub async fn load_from_file(&mut self) {
        if !self.state_file.exists() {
            info!("No state file found, starting fresh");
            return;
        }

        match tokio::fs::read_to_string(&self.state_file).await {
            Ok(content) => {
                match serde_json::from_str::<Vec<Download>>(&content) {
                    Ok(loaded) => {
                        let count = loaded.len();
                        for download in loaded {
                            if download.status == DownloadStatus::Queued || download.status == DownloadStatus::Failed {
                                info!("Restoring pending download: {} ({})", download.filename, download.id);
                                self.downloads.insert(download.id, download);
                            }
                        }
                        info!("Loaded {} downloads from state file", count);
                    }
                    Err(e) => {
                        warn!("Failed to parse state file: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read state file: {}", e);
            }
        }
    }

    pub async fn save_to_file(&self) {
        let downloads: Vec<&Download> = self.downloads.values().collect();
        match serde_json::to_string_pretty(&downloads) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write(&self.state_file, json).await {
                    error!("Failed to save state: {}", e);
                } else {
                    info!("State saved to {:?}", self.state_file);
                }
            }
            Err(e) => {
                error!("Failed to serialize state: {}", e);
            }
        }
    }

    pub fn add_download(&mut self, download: Download) {
        self.downloads.insert(download.id, download);
    }

    pub fn remove_download(&mut self, id: Uuid) -> Option<Download> {
        self.downloads.remove(&id)
    }

    pub fn get_download(&self, id: Uuid) -> Option<&Download> {
        self.downloads.get(&id)
    }

    pub fn get_download_mut(&mut self, id: Uuid) -> Option<&mut Download> {
        self.downloads.get_mut(&id)
    }

    pub fn list_downloads(&self) -> Vec<&Download> {
        self.downloads.values().collect()
    }

    pub fn active_count(&self) -> usize {
        self.downloads
            .values()
            .filter(|d| d.status == DownloadStatus::Downloading)
            .count()
    }

    pub fn pending_or_failed_count(&self) -> usize {
        self.downloads
            .values()
            .filter(|d| d.status == DownloadStatus::Queued || d.status == DownloadStatus::Failed)
            .count()
    }

    pub fn get_pending_downloads(&self) -> Vec<Download> {
        self.downloads
            .values()
            .filter(|d| d.status == DownloadStatus::Queued || d.status == DownloadStatus::Failed)
            .cloned()
            .collect()
    }

    pub fn reload_semaphore(&mut self) {
        self.download_semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        info!("Semaphore reloaded with max_concurrent={}", self.max_concurrent);
    }
}
