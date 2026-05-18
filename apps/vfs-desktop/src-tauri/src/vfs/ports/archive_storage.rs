//! Archive Storage Port - Interface for cold/archive storage
//!
//! This trait defines the contract for archive/cold storage providers.
//!
//! All archive/cold storage can be treated the same way:
//! - Requires restoration before access
//! - Different retrieval tiers (expedited, standard, bulk)
//! - Restoration jobs/tasks
//! - Long-term storage with lower costs
//! - Retrieval time varies by tier

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Archive Storage trait - Base abstraction for cold/archive storage
///
/// Implemented by:
/// - S3GlacierAdapter (S3 Glacier, Glacier Deep Archive)
/// - AzureArchiveStorageAdapter (Azure Archive tier)
/// - GcsColdlineAdapter (GCS Coldline, Archive)
/// - Any storage with archive/cold tier
#[async_trait]
pub trait ArchiveStorage: Send + Sync {
    /// Get bucket/container name
    fn bucket_name(&self) -> &str;
    
    /// Get region
    fn region(&self) -> Option<&str>;
    
    // =========================================================================
    // Restoration Operations
    // =========================================================================
    
    /// Initiate restoration of an archived object
    async fn initiate_restore(
        &self,
        key: &Path,
        tier: RestoreTier,
        days: u32,
    ) -> Result<String>; // Returns restoration job ID
    
    /// Check restoration status
    async fn get_restore_status(&self, key: &Path, job_id: &str) -> Result<RestoreStatus>;
    
    /// Check if object is currently being restored
    async fn is_restoring(&self, key: &Path) -> Result<bool>;
    
    /// Check if object is restored and ready for access
    async fn is_restored(&self, key: &Path) -> Result<bool>;
    
    /// Get restoration expiry time (when restored copy expires)
    async fn get_restore_expiry(&self, key: &Path) -> Result<Option<SystemTime>>;
    
    /// Cancel an ongoing restoration
    async fn cancel_restore(&self, key: &Path, job_id: &str) -> Result<()>;
    
    /// List active restoration jobs
    async fn list_restore_jobs(&self, prefix: Option<&str>) -> Result<Vec<RestoreJob>>;
    
    // =========================================================================
    // Archive Tier Operations
    // =========================================================================
    
    /// Get the archive tier of an object
    async fn get_archive_tier(&self, key: &Path) -> Result<ArchiveTier>;
    
    /// Transition object to archive tier
    async fn transition_to_archive(&self, key: &Path, tier: ArchiveTier) -> Result<()>;
    
    /// Transition multiple objects to archive tier
    async fn batch_transition_to_archive(
        &self,
        keys: Vec<&Path>,
        tier: ArchiveTier,
    ) -> Result<Vec<TransitionResult>>;
    
    // =========================================================================
    // Archive Inventory & Queries
    // =========================================================================
    
    /// Get archive inventory (list of archived objects)
    async fn get_archive_inventory(&self, prefix: Option<&str>) -> Result<Vec<ArchiveObject>>;
    
    /// Query archive inventory (SQL-like queries on metadata)
    async fn query_archive_inventory(&self, query: &str) -> Result<Vec<ArchiveObject>>;
    
    /// Get archive statistics
    async fn get_archive_stats(&self) -> Result<ArchiveStats>;
}

/// Restore tier - Speed vs cost tradeoff
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreTier {
    /// Expedited (1-5 minutes) - Highest cost
    Expedited,
    
    /// Standard (3-5 hours) - Standard cost
    Standard,
    
    /// Bulk (5-12 hours) - Lowest cost
    Bulk,
}

impl RestoreTier {
    /// Get estimated retrieval time
    pub fn estimated_time(&self) -> Duration {
        match self {
            RestoreTier::Expedited => Duration::from_secs(5 * 60), // 5 minutes
            RestoreTier::Standard => Duration::from_secs(4 * 3600), // 4 hours
            RestoreTier::Bulk => Duration::from_secs(12 * 3600), // 12 hours
        }
    }
    
    /// Get cost multiplier (relative to standard)
    pub fn cost_multiplier(&self) -> f64 {
        match self {
            RestoreTier::Expedited => 10.0, // 10x more expensive
            RestoreTier::Standard => 1.0,   // Baseline
            RestoreTier::Bulk => 0.5,      // Half the cost
        }
    }
}

/// Archive tier - Storage class
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveTier {
    /// Glacier Instant Retrieval (milliseconds access)
    InstantRetrieval,
    
    /// Glacier Flexible Retrieval (Standard)
    FlexibleRetrieval,
    
    /// Glacier Deep Archive (lowest cost, longest retrieval)
    DeepArchive,
    
    /// Azure Archive
    AzureArchive,
    
    /// GCS Coldline
    Coldline,
    
    /// GCS Archive
    Archive,
}

impl ArchiveTier {
    /// Get minimum storage duration (days)
    pub fn minimum_duration_days(&self) -> u32 {
        match self {
            ArchiveTier::InstantRetrieval => 0,
            ArchiveTier::FlexibleRetrieval => 90,
            ArchiveTier::DeepArchive => 180,
            ArchiveTier::AzureArchive => 180,
            ArchiveTier::Coldline => 90,
            ArchiveTier::Archive => 365,
        }
    }
    
    /// Get retrieval time estimate
    pub fn retrieval_time(&self) -> Duration {
        match self {
            ArchiveTier::InstantRetrieval => Duration::from_millis(100),
            ArchiveTier::FlexibleRetrieval => Duration::from_secs(4 * 3600), // 4 hours
            ArchiveTier::DeepArchive => Duration::from_secs(12 * 3600), // 12 hours
            ArchiveTier::AzureArchive => Duration::from_secs(15 * 3600), // 15 hours
            ArchiveTier::Coldline => Duration::from_secs(4 * 3600), // 4 hours
            ArchiveTier::Archive => Duration::from_secs(12 * 3600), // 12 hours
        }
    }
}

/// Restoration status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreStatus {
    /// Restoration in progress
    InProgress {
        /// Job ID
        job_id: String,
        /// Estimated completion time
        estimated_completion: Option<SystemTime>,
    },
    
    /// Restoration completed, object ready
    Completed {
        /// Job ID
        job_id: String,
        /// Expiry time for restored copy
        expiry: SystemTime,
    },
    
    /// Restoration failed
    Failed {
        /// Job ID
        job_id: String,
        /// Error message
        error: String,
    },
    
    /// No active restoration
    NotRestoring,
}

/// Restoration job information
#[derive(Debug, Clone)]
pub struct RestoreJob {
    /// Job ID
    pub job_id: String,
    
    /// Object key
    pub key: String,
    
    /// Restore tier
    pub tier: RestoreTier,
    
    /// Requested days
    pub days: u32,
    
    /// Status
    pub status: RestoreStatus,
    
    /// Initiated timestamp
    pub initiated: SystemTime,
}

/// Archive object information
#[derive(Debug, Clone)]
pub struct ArchiveObject {
    /// Object key
    pub key: String,
    
    /// Size in bytes
    pub size: u64,
    
    /// Archive tier
    pub tier: ArchiveTier,
    
    /// Archived timestamp
    pub archived_at: SystemTime,
    
    /// Last modified timestamp
    pub last_modified: SystemTime,
    
    /// ETag
    pub etag: String,
    
    /// Storage class
    pub storage_class: String,
    
    /// Is currently restored
    pub is_restored: bool,
    
    /// Restore expiry (if restored)
    pub restore_expiry: Option<SystemTime>,
}

/// Transition result
#[derive(Debug, Clone)]
pub struct TransitionResult {
    /// Object key
    pub key: String,
    
    /// Success or error message
    pub result: Result<(), String>,
}

/// Archive statistics
#[derive(Debug, Clone)]
pub struct ArchiveStats {
    /// Total archived objects
    pub total_objects: u64,
    
    /// Total archived size in bytes
    pub total_size: u64,
    
    /// Objects by tier
    pub objects_by_tier: Vec<(ArchiveTier, u64)>,
    
    /// Size by tier
    pub size_by_tier: Vec<(ArchiveTier, u64)>,
    
    /// Active restoration jobs
    pub active_restore_jobs: u32,
    
    /// Estimated monthly cost (if available)
    pub estimated_monthly_cost: Option<f64>,
}
