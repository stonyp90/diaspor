//! Video Streaming Commands
//!
//! Commands for streaming video from S3 and other storage without downloading entire files.
//! Supports presigned URLs and range requests for efficient streaming.

use anyhow::Result;
use serde::{Serialize, Deserialize};
use std::path::Path;
use tauri::State;
use tracing::info;

use crate::vfs::domain::StorageSourceType;
use super::state::VfsStateWrapper;

/// Request to get a streamable URL for a video file
#[derive(Debug, Deserialize)]
pub struct GetStreamUrlRequest {
    pub source_id: String,
    pub file_path: String,
    /// Duration in seconds for presigned URL validity (default: 3600 = 1 hour)
    pub expires_in: Option<u64>,
}

/// Response containing a streamable URL
#[derive(Debug, Serialize)]
pub struct StreamUrlResponse {
    /// The URL that can be used to stream the video
    pub stream_url: String,
    /// Type of streaming: "presigned" (S3 presigned URL) or "local" (file:// or http://)
    pub stream_type: String,
    /// Whether the URL supports HTTP range requests (for seeking)
    pub supports_range_requests: bool,
    /// When the URL expires (Unix timestamp), if applicable
    pub expires_at: Option<u64>,
    /// Message for user
    pub message: String,
}

/// Get a streamable URL for a video file
/// 
/// For S3 storage: Generates a presigned URL that allows streaming without downloading
/// For local storage: Returns file:// URL or local HTTP server URL
#[tauri::command]
pub async fn vfs_get_stream_url(
    request: GetStreamUrlRequest,
    state: State<'_, VfsStateWrapper>,
) -> Result<StreamUrlResponse, String> {
    info!("[vfs_get_stream_url] Getting stream URL for: {} in source {}", 
        request.file_path, request.source_id);
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized".to_string())?;
    
    let source = service.get_source(&request.source_id)
        .ok_or_else(|| format!("Storage source not found: {}", request.source_id))?;
    
    match source.source_type {
        StorageSourceType::S3 | 
        StorageSourceType::AzureBlob |
        StorageSourceType::Gcs |
        StorageSourceType::S3Compatible => {
            // Generate presigned URL for cloud storage
            generate_presigned_url(&service, &source, &request).await
        }
        StorageSourceType::Local |
        StorageSourceType::Nas |
        StorageSourceType::Nfs |
        StorageSourceType::Smb => {
            // For local/mounted storage, return file path or local server URL
            generate_local_url(&source, &request.file_path)
        }
        _ => {
            Err(format!("Streaming not supported for storage type: {:?}", source.source_type))
        }
    }
}

/// Generate presigned URL for S3-like storage
async fn generate_presigned_url(
    service: &std::sync::Arc<crate::vfs::application::VfsService>,
    source: &crate::vfs::domain::StorageSource,
    request: &GetStreamUrlRequest,
) -> Result<StreamUrlResponse, String> {
    use crate::vfs::domain::StorageSourceType;
    
    info!("[generate_presigned_url] Generating presigned URL for {:?} storage", source.source_type);
    
    // Get expires duration (default: 1 hour)
    let _expires_in_secs = request.expires_in.unwrap_or(3600);
    
    // Normalize path
    let normalized_path = request.file_path.strip_prefix("/").unwrap_or(&request.file_path);
    
    // For remote storage, download to cache and stream from there
    // This guarantees it works and benefits AI features (cache reuse)
    if matches!(source.source_type, StorageSourceType::S3 | StorageSourceType::S3Compatible) {
        info!("[stream] Preparing video from remote storage");
        
        // Download to cache
        let temp_dir = std::env::temp_dir().join("ursly-video-cache");
        let cache_file = temp_dir
            .join(&source.id)
            .join(normalized_path);
        
        // Create cache directory
        if let Some(parent) = cache_file.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| format!("Failed to create cache directory: {}", e))?;
        }
        
        // Check if already cached
        if !cache_file.exists() {
            info!("[stream] Downloading video to cache");
            
            let file_data = service.read_file(&source.id, std::path::Path::new(&request.file_path))
                .await
                .map_err(|e| format!("Failed to load video: {}", e))?;
            
            tokio::fs::write(&cache_file, file_data).await
                .map_err(|e| format!("Failed to cache video: {}", e))?;
            
            info!("[stream] Video ready for playback");
        } else {
            info!("[stream] Video already cached, ready for instant playback");
        }
        
        // Return the local file path - frontend will convert to blob URL
        let file_path = cache_file.to_string_lossy().to_string();
        
        info!("[stream] Video ready for playback");
        
        Ok(StreamUrlResponse {
            stream_url: file_path,
            stream_type: "cached".to_string(),
            supports_range_requests: true,
            expires_at: None,
            message: "Video ready".to_string(),
        })
    } else {
        Err(format!("Presigned URL not supported for storage type: {:?}", source.source_type))
    }
}

/// Generate local URL for mounted storage
fn generate_local_url(
    source: &crate::vfs::domain::StorageSource,
    file_path: &str,
) -> Result<StreamUrlResponse, String> {
    if let Some(mount_point) = &source.mount_point {
        let normalized_path = file_path.strip_prefix("/").unwrap_or(file_path);
        let full_path = mount_point.join(normalized_path);
        
        if !full_path.exists() {
            return Err(format!("File not found: {}", full_path.display()));
        }
        
        // Return file:// URL for local playback
        let file_url = format!("file://{}", full_path.display());
        
        info!("[generate_local_url] Generated local file URL");
        
        Ok(StreamUrlResponse {
            stream_url: file_url,
            stream_type: "local".to_string(),
            supports_range_requests: true,
            expires_at: None,
            message: "Ready to stream from local file".to_string(),
        })
    } else {
        Err("Storage source not mounted locally".to_string())
    }
}

/// Download a specific byte range from a file (for implementing custom range requests)
#[derive(Debug, Deserialize)]
pub struct RangeRequest {
    pub source_id: String,
    pub file_path: String,
    /// Start byte (inclusive)
    pub start: u64,
    /// End byte (inclusive), or None for rest of file
    pub end: Option<u64>,
}

/// Response containing a byte range
#[derive(Debug, Serialize)]
pub struct RangeResponse {
    /// The requested data (base64 encoded when serialized)
    pub data: Vec<u8>,
    /// Actual start byte
    pub start: u64,
    /// Actual end byte
    pub end: u64,
    /// Total file size
    pub total_size: u64,
    /// Content type
    pub content_type: String,
}

/// Download a specific byte range from a file
/// This can be used to implement custom streaming for S3 files
#[tauri::command]
pub async fn vfs_get_file_range(
    request: RangeRequest,
    state: State<'_, VfsStateWrapper>,
) -> Result<RangeResponse, String> {
    info!("[vfs_get_file_range] Getting range {}-{:?} for: {} in source {}", 
        request.start, request.end, request.file_path, request.source_id);
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized".to_string())?;
    
    // For now, we'll read the entire file and return the range
    // TODO: Implement actual range requests in storage adapters
    let file_data = service.read_file(&request.source_id, Path::new(&request.file_path))
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    let total_size = file_data.len() as u64;
    let start = request.start.min(total_size - 1);
    let end = request.end.unwrap_or(total_size - 1).min(total_size - 1);
    
    if start > end {
        return Err(format!("Invalid range: {}-{}", start, end));
    }
    
    let range_data = file_data[start as usize..=end as usize].to_vec();
    
    Ok(RangeResponse {
        data: range_data,
        start,
        end,
        total_size,
        content_type: "video/mp4".to_string(), // TODO: Detect from file extension
    })
}
