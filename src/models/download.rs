use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub id: Uuid,
    pub url: String,
    pub filename: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub status: DownloadStatus,
    pub created_at: DateTime<Utc>,
    pub file_path: Option<PathBuf>,
    pub error_message: Option<String>,
    pub speed_bps: u64,
}

impl Download {
    pub fn new(url: String) -> Self {
        let filename = url
            .split('/')
            .last()
            .unwrap_or("download")
            .split('?')
            .next()
            .unwrap_or("download");

        let filename = match percent_encoding::percent_decode_str(filename).decode_utf8() {
            Ok(decoded) => decoded.into_owned(),
            Err(_) => filename.to_string(),
        };

        let filename = if filename.is_empty() {
            "download".to_string()
        } else {
            filename
        };

        Self {
            id: Uuid::new_v4(),
            url,
            filename,
            total_bytes: 0,
            downloaded_bytes: 0,
            status: DownloadStatus::Queued,
            created_at: Utc::now(),
            file_path: None,
            error_message: None,
            speed_bps: 0,
        }
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.downloaded_bytes as f64 / self.total_bytes as f64) * 100.0
        }
    }

    pub fn formatted_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    pub fn formatted_speed(bps: u64) -> String {
        format!("{}/s", Self::formatted_size(bps))
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateDownloadRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct DownloadProgress {
    pub id: Uuid,
    pub filename: String,
    pub status: DownloadStatus,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub progress_percent: f64,
    pub speed_bps: u64,
    pub file_path: Option<String>,
    pub error_message: Option<String>,
}

impl From<&Download> for DownloadProgress {
    fn from(d: &Download) -> Self {
        Self {
            id: d.id,
            filename: d.filename.clone(),
            status: d.status.clone(),
            total_bytes: d.total_bytes,
            downloaded_bytes: d.downloaded_bytes,
            progress_percent: d.progress_percent(),
            speed_bps: d.speed_bps,
            file_path: d.file_path.as_ref().map(|p| p.to_string_lossy().to_string()),
            error_message: d.error_message.clone(),
        }
    }
}
