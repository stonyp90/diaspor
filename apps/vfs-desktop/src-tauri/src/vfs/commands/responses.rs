//! Response Types for Frontend
//!
//! All response DTOs used by Tauri commands

use serde::{Deserialize, Serialize};
use crate::vfs::adapters::transcription::TranscriptionSegment;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsStorageSourceResponse {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub mounted: bool,
    pub status: String,
    pub path: Option<String>,
    pub bucket: Option<String>,
    pub region: Option<String>,
    /// Storage category (local, cloud, network, etc.)
    pub category: String,
    /// Provider ID (e.g., "s3", "gcs", "local")
    pub provider_id: Option<String>,
    /// Whether this is a mounted volume that can be ejected (DMG, external drive, etc.)
    pub is_ejectable: bool,
    /// Whether this is a system location (Home, Documents, etc.)
    pub is_system_location: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsFileMetadataResponse {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: u64,
    #[serde(rename = "size_human")]
    pub size_human: String,
    #[serde(rename = "lastModified")]
    pub last_modified: String,
    #[serde(rename = "isDirectory")]
    pub is_directory: bool,
    #[serde(rename = "isHidden")]
    pub is_hidden: bool,
    #[serde(rename = "tierStatus")]
    pub tier_status: String,
    #[serde(rename = "isCached")]
    pub is_cached: bool,
    #[serde(rename = "canWarm")]
    pub can_warm: bool,
    #[serde(rename = "canTranscode")]
    pub can_transcode: bool,
    #[serde(rename = "transcodeStatus")]
    pub transcode_status: Option<String>,
    #[serde(rename = "transcodeProgress")]
    pub transcode_progress: Option<u8>,
    pub thumbnail: Option<String>,  // Base64 data URL or API URL
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    /// Custom tags with optional colors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<TagResponse>>,
    /// User comments/notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
}

/// Paginated response for file listings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsListFilesResponse {
    pub files: Vec<VfsFileMetadataResponse>,
    /// Continuation token for pagination (None if no more items)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
    /// Total count of items (if known, None if unknown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagResponse {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsCacheStatsResponse {
    pub total_size: u64,
    pub max_size: u64,
    pub entry_count: u64,
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
    pub usage_percent: f64,
}

/// Response for transcription operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    pub operation_id: String,
    pub segments: Vec<TranscriptionSegment>,
}
