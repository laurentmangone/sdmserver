use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use tokio::sync::mpsc;
use crate::models::download::Download;
use crate::models::DownloadStatus;

pub struct AppState {
    pub downloads: HashMap<Uuid, Download>,
    pub download_dir: PathBuf,
    pub max_concurrent: usize,
    pub cancel_tx: mpsc::Sender<Uuid>,
}

impl AppState {
    pub fn new(download_dir: String, max_concurrent: usize) -> Self {
        let (tx, _rx) = mpsc::channel::<Uuid>(100);
        Self {
            downloads: HashMap::new(),
            download_dir: PathBuf::from(download_dir),
            max_concurrent,
            cancel_tx: tx,
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
}
