//! Value Objects - Immutable objects defined by their attributes

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// File size value object with human-readable formatting
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileSize(u64);

impl FileSize {
    pub fn from_bytes(bytes: u64) -> Self {
        Self(bytes)
    }
    
    pub fn bytes(&self) -> u64 {
        self.0
    }
    
    pub fn as_human_readable(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;
        
        if self.0 >= TB {
            format!("{:.2} TB", self.0 as f64 / TB as f64)
        } else if self.0 >= GB {
            format!("{:.2} GB", self.0 as f64 / GB as f64)
        } else if self.0 >= MB {
            format!("{:.2} MB", self.0 as f64 / MB as f64)
        } else if self.0 >= KB {
            format!("{:.2} KB", self.0 as f64 / KB as f64)
        } else {
            format!("{} bytes", self.0)
        }
    }
}

/// Storage tier representing data temperature
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[derive(Default)]
pub enum StorageTier {
    /// Data on local NVMe - fastest access
    Hot,
    /// Data on network storage - moderate access
    Warm,
    /// Data in cloud object storage - slower access
    #[default]
    Cold,
    /// Data in nearline storage (e.g., S3 Infrequent Access) - moderate latency
    Nearline,
    /// Data in archival storage (e.g., Glacier) - slowest
    Archive,
    /// Data in instant retrieval tier (e.g., S3 Glacier Instant Retrieval) - millisecond access with lower cost
    InstantRetrieval,
}


impl StorageTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageTier::Hot => "hot",
            StorageTier::Warm => "warm",
            StorageTier::Cold => "cold",
            StorageTier::Nearline => "nearline",
            StorageTier::Archive => "archive",
            StorageTier::InstantRetrieval => "instant-retrieval",
        }
    }
    
    pub fn icon(&self) -> &'static str {
        match self {
            StorageTier::Hot => "flame",
            StorageTier::Warm => "thermometer",
            StorageTier::Cold => "snowflake",
            StorageTier::Nearline => "clock",
            StorageTier::Archive => "archive",
            StorageTier::InstantRetrieval => "bolt",
        }
    }
}

/// Model category for AI models
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModelCategory {
    /// Transcription models (e.g., Whisper)
    Transcription,
    /// Video tagging models (automatic video content tagging)
    VideoTagging,
    /// Image tagging models
    ImageTagging,
    /// Text generation models
    TextGeneration,
    /// Code generation models
    CodeGeneration,
    /// Embedding models (for semantic search)
    Embedding,
    /// Other/uncategorized models
    Other,
}

impl ModelCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelCategory::Transcription => "transcription",
            ModelCategory::VideoTagging => "video_tagging",
            ModelCategory::ImageTagging => "image_tagging",
            ModelCategory::TextGeneration => "text_generation",
            ModelCategory::CodeGeneration => "code_generation",
            ModelCategory::Embedding => "embedding",
            ModelCategory::Other => "other",
        }
    }
    
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "transcription" => ModelCategory::Transcription,
            "video_tagging" | "video-tagging" | "video tagging" => ModelCategory::VideoTagging,
            "image_tagging" | "image-tagging" | "image tagging" => ModelCategory::ImageTagging,
            "text_generation" | "text-generation" | "text generation" => ModelCategory::TextGeneration,
            "code_generation" | "code-generation" | "code generation" => ModelCategory::CodeGeneration,
            "embedding" => ModelCategory::Embedding,
            _ => ModelCategory::Other,
        }
    }
    
    pub fn icon(&self) -> &'static str {
        match self {
            ModelCategory::Transcription => "microphone",
            ModelCategory::VideoTagging => "video",
            ModelCategory::ImageTagging => "image",
            ModelCategory::TextGeneration => "text",
            ModelCategory::CodeGeneration => "code",
            ModelCategory::Embedding => "search",
            ModelCategory::Other => "box",
        }
    }
    
    pub fn display_name(&self) -> &'static str {
        match self {
            ModelCategory::Transcription => "Transcription",
            ModelCategory::VideoTagging => "Video Tagging",
            ModelCategory::ImageTagging => "Image Tagging",
            ModelCategory::TextGeneration => "Text Generation",
            ModelCategory::CodeGeneration => "Code Generation",
            ModelCategory::Embedding => "Embeddings",
            ModelCategory::Other => "Other",
        }
    }
}

/// Tier status for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierStatus {
    /// Current storage tier
    pub current_tier: StorageTier,
    
    /// Is file cached locally
    pub is_cached: bool,
    
    /// Can be warmed (moved to hotter tier)
    pub can_warm: bool,
    
    /// Estimated retrieval time in seconds
    pub retrieval_time_estimate: Option<u32>,
}

impl Default for TierStatus {
    fn default() -> Self {
        Self {
            current_tier: StorageTier::Cold,
            is_cached: false,
            can_warm: true,
            retrieval_time_estimate: None,
        }
    }
}

/// Mount configuration value object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPoint {
    pub path: PathBuf,
    pub label: String,
    pub read_only: bool,
}

impl MountPoint {
    pub fn new(path: PathBuf, label: String) -> Self {
        Self {
            path,
            label,
            read_only: false,
        }
    }
    
    /// Get default mount point for the current OS
    pub fn default_for_os() -> Self {
        #[cfg(target_os = "macos")]
        let path = PathBuf::from("/Volumes/Ursly");
        
        #[cfg(target_os = "linux")]
        let path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("ursly-vfs");
        
        #[cfg(target_os = "windows")]
        let path = PathBuf::from("U:\\");
        
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        let path = PathBuf::from("/tmp/ursly-vfs");
        
        Self::new(path, "Cairn".to_string())
    }
}

/// Cache configuration value object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Path to cache directory
    pub path: PathBuf,
    
    /// Maximum cache size in bytes (0 = unlimited)
    pub max_size: u64,
    
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
    
    /// Enable NVMe optimizations
    pub nvme_optimized: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        let cache_path = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("ursly")
            .join("vfs-cache");
        
        Self {
            path: cache_path,
            max_size: 10 * 1024 * 1024 * 1024, // 10 GB
            eviction_policy: EvictionPolicy::LRU,
            nvme_optimized: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EvictionPolicy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// First In First Out
    FIFO,
}

/// Transcode format options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TranscodeFormat {
    HLS,
    DASH,
    MP4,
    WebM,
}

impl TranscodeFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            TranscodeFormat::HLS => "m3u8",
            TranscodeFormat::DASH => "mpd",
            TranscodeFormat::MP4 => "mp4",
            TranscodeFormat::WebM => "webm",
        }
    }
}

