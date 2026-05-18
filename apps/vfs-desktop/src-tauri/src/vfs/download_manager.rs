//! Download Manager with Resume Support
//!
//! Implements chunked downloads for large files from object storage with:
//! - Progress tracking
//! - Resume from failure
//! - Chunked downloads (5MB parts)
//! - State persistence
//! - No visible chunk files

use anyhow::{Context, Result};
use opendal::Operator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::sync::RwLock;
use tracing::{error, info};
use uuid::Uuid;

/// Download state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadState {
    /// Unique download ID
    pub download_id: String,
    /// Operation ID (links to operation tracker)
    pub operation_id: Option<String>,
    /// Source ID
    pub source_id: String,
    /// Remote path (source)
    pub remote_path: String,
    /// Local destination path
    pub local_path: PathBuf,
    /// Total file size
    pub total_size: u64,
    /// Chunk size (default 5MB)
    pub chunk_size: u64,
    /// Number of chunks
    pub total_chunks: u64,
    /// Bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Current chunk being downloaded
    pub current_chunk: u64,
    /// Download status
    pub status: DownloadStatus,
    /// Error message if failed
    pub error: Option<String>,
    /// Timestamp when download was created/started
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp when download was completed (or failed/canceled)
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp of last update
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DownloadStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Paused,
}

/// Download manager
pub struct DownloadManager {
    /// Active downloads
    downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
    /// State file path
    state_file: PathBuf,
}

impl DownloadManager {
    pub fn new(state_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(state_dir)
            .context("Failed to create download manager state directory")?;
        
        let state_file = state_dir.join("downloads.json");
        
        Ok(Self {
            downloads: Arc::new(RwLock::new(HashMap::new())),
            state_file,
        })
    }
    
    /// Load persisted download states
    pub async fn load_states(&self) -> Result<()> {
        if !self.state_file.exists() {
            return Ok(());
        }
        
        let data = tokio::fs::read_to_string(&self.state_file).await?;
        let states: HashMap<String, DownloadState> = serde_json::from_str(&data)?;
        
        let mut downloads = self.downloads.write().await;
        *downloads = states;
        
        info!("Loaded {} persisted download states", downloads.len());
        Ok(())
    }
    
    /// Save download states to disk
    pub async fn save_states(&self) -> Result<()> {
        let downloads = self.downloads.read().await;
        let data = serde_json::to_string_pretty(&*downloads)?;
        tokio::fs::write(&self.state_file, data).await?;
        Ok(())
    }
    
    /// Start a new download
    pub async fn start_download(
        &self,
        operator: &Operator,
        source_id: &str,
        remote_path: &str,
        local_path: &Path,
        operation_id: Option<String>,
    ) -> Result<String> {
        // Get file size from remote
        let metadata = operator.stat(remote_path).await
            .with_context(|| format!("Failed to get file metadata: {}", remote_path))?;
        let total_size = metadata.content_length();
        
        let chunk_size = 5 * 1024 * 1024; // 5MB chunks
        let total_chunks = if total_size == 0 {
            1
        } else {
            (total_size + chunk_size - 1) / chunk_size
        };
        
        let download_id = Uuid::new_v4().to_string();
        
        let state = DownloadState {
            download_id: download_id.clone(),
            operation_id,
            source_id: source_id.to_string(),
            remote_path: remote_path.to_string(),
            local_path: local_path.to_path_buf(),
            total_size,
            chunk_size,
            total_chunks,
            bytes_downloaded: 0,
            current_chunk: 0,
            status: DownloadStatus::Pending,
            error: None,
            created_at: Some(chrono::Utc::now()),
            completed_at: None,
            last_updated_at: Some(chrono::Utc::now()),
        };
        
        {
            let mut downloads = self.downloads.write().await;
            downloads.insert(download_id.clone(), state);
        }
        
        self.save_states().await?;
        
        // Start download in background
        let operator_clone = operator.clone();
        let download_id_clone = download_id.clone();
        let downloads_clone = Arc::clone(&self.downloads);
        let state_file_clone = self.state_file.clone();
        
        tokio::spawn(async move {
            let manager = DownloadManager {
                downloads: downloads_clone,
                state_file: state_file_clone,
            };
            
            if let Err(e) = manager.download_chunks(&operator_clone, &download_id_clone).await {
                error!("Download failed: {}", e);
            }
        });
        
        Ok(download_id)
    }
    
    /// Download file in chunks with progress tracking
    pub async fn download_chunks(
        &self,
        operator: &Operator,
        download_id: &str,
    ) -> Result<()> {
        let (remote_path, local_path, total_size, chunk_size, resume_from, _operation_id) = {
            let downloads = self.downloads.read().await;
            let state = downloads.get(download_id)
                .ok_or_else(|| anyhow::anyhow!("Download not found"))?;
            
            let local_path_clone = state.local_path.clone();
            
            // Check if file already exists (for resume)
            let resume_from = if local_path_clone.exists() {
                tokio::fs::metadata(&local_path_clone).await
                    .map(|m| m.len())
                    .unwrap_or(0)
            } else {
                0
            };
            
            (
                state.remote_path.clone(),
                local_path_clone,
                state.total_size,
                state.chunk_size,
                resume_from,
                state.operation_id.clone(),
            )
        };
        
        info!("Starting download: {} -> {} (resume from byte {})", remote_path, local_path.display(), resume_from);
        
        // Update status to InProgress
        {
            let mut downloads = self.downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                state.status = DownloadStatus::InProgress;
                state.bytes_downloaded = resume_from;
                state.last_updated_at = Some(chrono::Utc::now());
            }
        }
        self.save_states().await?;
        
        // Open or create destination file
        let mut dest_file = if resume_from > 0 {
            // Resume: open in append mode
            OpenOptions::new()
                .create(true)
                .append(true)
                .write(true)
                .open(&local_path).await?
        } else {
            // New download: create new file
            if let Some(parent) = local_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            File::create(&local_path).await?
        };
        
        // Seek to resume position
        if resume_from > 0 {
            dest_file.seek(SeekFrom::Start(resume_from)).await?;
            info!("Resuming download from byte {}", resume_from);
        }
        
        // Download in chunks
        let mut offset = resume_from;
        let mut chunk_number = (resume_from / chunk_size) + 1;
        
        while offset < total_size {
            // Check if download was paused or canceled
            {
                let downloads = self.downloads.read().await;
                if let Some(state) = downloads.get(download_id) {
                    if matches!(state.status, DownloadStatus::Paused | DownloadStatus::Failed) {
                        info!("Download {} paused or failed, stopping", download_id);
                        return Ok(());
                    }
                }
            }
            
            let chunk_len = (total_size - offset).min(chunk_size);
            
            // Read chunk from remote using range request
            let chunk_data = operator.read_with(&remote_path)
                .range(offset..offset + chunk_len)
                .await
                .with_context(|| format!("Failed to read chunk at offset {}", offset))?;
            
            // Write chunk to local file
            dest_file.write_all(&chunk_data).await
                .with_context(|| format!("Failed to write chunk to {}", local_path.display()))?;
            
            offset += chunk_data.len() as u64;
            chunk_number += 1;
            
            // Update progress
            {
                let mut downloads = self.downloads.write().await;
                if let Some(state) = downloads.get_mut(download_id) {
                    state.bytes_downloaded = offset;
                    state.current_chunk = chunk_number;
                    state.last_updated_at = Some(chrono::Utc::now());
                    
                    // Sync progress to operation tracker
                    if let Some(op_id) = &state.operation_id {
                        use crate::vfs::commands::get_operation_tracker;
                        let tracker = get_operation_tracker();
                        
                        // Use update_operation_progress for single-file operations
                        let _ = tracker.update_operation_progress(
                            op_id,
                            offset,
                            Some(state.total_size),
                        );
                    }
                }
            }
            
            // Save state periodically (every 5 chunks or every 25MB)
            if chunk_number % 5 == 0 {
                self.save_states().await?;
            }
            
            let percentage = (offset as f64 / total_size as f64) * 100.0;
            info!("[Chunked Download] {}: {} / {} bytes ({:.1}%)", 
                download_id, offset, total_size, percentage);
        }
        
        // Mark as completed
        {
            let mut downloads = self.downloads.write().await;
            if let Some(state) = downloads.get_mut(download_id) {
                let now = chrono::Utc::now();
                state.bytes_downloaded = total_size;
                state.status = DownloadStatus::Completed;
                state.current_chunk = state.total_chunks;
                state.completed_at = Some(now);
                state.last_updated_at = Some(now);
                
                // Mark operation as completed
                if let Some(op_id) = &state.operation_id {
                    use crate::vfs::commands::get_operation_tracker;
                    let tracker = get_operation_tracker();
                    let _ = tracker.complete_operation(op_id);
                }
            }
        }
        
        self.save_states().await?;
        info!("Download completed: {} -> {}", remote_path, local_path.display());
        
        Ok(())
    }
    
    /// Resume a paused download
    pub async fn resume_download(
        &self,
        operator: &Operator,
        download_id: &str,
    ) -> Result<()> {
        let mut downloads = self.downloads.write().await;
        let state = downloads.get_mut(download_id)
            .ok_or_else(|| anyhow::anyhow!("Download not found: {}", download_id))?;
        
        if state.status == DownloadStatus::Completed {
            return Err(anyhow::anyhow!("Download already completed"));
        }
        
        state.status = DownloadStatus::InProgress;
        state.error = None;
        state.last_updated_at = Some(chrono::Utc::now());
        drop(downloads);
        
        self.save_states().await?;
        self.download_chunks(operator, download_id).await
    }
    
    /// Pause a download
    pub async fn pause_download(&self, download_id: &str) -> Result<()> {
        let mut downloads = self.downloads.write().await;
        let state = downloads.get_mut(download_id)
            .ok_or_else(|| anyhow::anyhow!("Download not found"))?;
        
        state.status = DownloadStatus::Paused;
        state.last_updated_at = Some(chrono::Utc::now());
        drop(downloads);
        
        self.save_states().await?;
        Ok(())
    }
    
    /// Cancel a download
    pub async fn cancel_download(&self, download_id: &str) -> Result<()> {
        {
            let mut downloads = self.downloads.write().await;
            downloads.remove(download_id);
        }
        
        self.save_states().await?;
        Ok(())
    }
    
    /// List all active downloads
    pub async fn list_downloads(&self) -> Vec<DownloadState> {
        let downloads = self.downloads.read().await;
        downloads.values().cloned().collect()
    }
    
    /// Get download progress
    pub async fn get_progress(&self, download_id: &str) -> Option<DownloadProgress> {
        let downloads = self.downloads.read().await;
        let state = downloads.get(download_id)?;
        
        let percentage = if state.total_size > 0 {
            (state.bytes_downloaded as f64 / state.total_size as f64) * 100.0
        } else {
            0.0
        };
        
        Some(DownloadProgress {
            download_id: state.download_id.clone(),
            remote_path: state.remote_path.clone(),
            bytes_downloaded: state.bytes_downloaded,
            total_size: state.total_size,
            percentage,
            current_chunk: state.current_chunk,
            total_chunks: state.total_chunks,
            status: state.status.clone(),
            error: state.error.clone(),
        })
    }
    
    /// Remove/delete a download (for completed/failed downloads that user wants to dismiss)
    pub async fn remove_download(&self, download_id: &str) -> Result<()> {
        let mut downloads = self.downloads.write().await;
        downloads.remove(download_id);
        drop(downloads);
        self.save_states().await?;
        Ok(())
    }
}

/// Progress update for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub download_id: String,
    pub remote_path: String,
    pub bytes_downloaded: u64,
    pub total_size: u64,
    pub percentage: f64,
    pub current_chunk: u64,
    pub total_chunks: u64,
    pub status: DownloadStatus,
    pub error: Option<String>,
}
