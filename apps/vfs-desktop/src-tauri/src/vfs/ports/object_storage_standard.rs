//! Object Storage Standard Port - Interface for object storage with standard access
//!
//! This trait defines the contract for object storage providers with standard
//! access tiers (hot/warm data).
//!
//! All object storage standard tier can be treated the same way:
//! - Direct object access (no restoration needed)
//! - Multipart uploads
//! - Presigned URLs
//! - Versioning support
//! - Lifecycle policies
//! - Metadata tagging

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Object Storage Standard trait - Base abstraction for standard-tier object storage
///
/// Implemented by:
/// - S3StorageAdapter (S3 Standard tier)
/// - GcsStorageAdapter (GCS Standard tier)
/// - AzureBlobStorageAdapter (Hot/Cool tiers)
/// - S3CompatibleStorageAdapter (MinIO, etc.)
#[async_trait]
pub trait ObjectStorageStandard: Send + Sync {
    /// Get bucket/container name
    fn bucket_name(&self) -> &str;
    
    /// Get region
    fn region(&self) -> Option<&str>;
    
    /// Get endpoint URL
    fn endpoint(&self) -> Option<&str>;
    
    // =========================================================================
    // Multipart Upload Operations
    // =========================================================================
    
    /// Start a multipart upload
    async fn start_multipart_upload(&self, key: &Path) -> Result<String>;
    
    /// Upload a part of a multipart upload
    async fn upload_part(
        &self,
        upload_id: &str,
        key: &Path,
        part_number: u32,
        data: &[u8],
    ) -> Result<String>; // Returns ETag
    
    /// Complete a multipart upload
    async fn complete_multipart_upload(
        &self,
        upload_id: &str,
        key: &Path,
        parts: Vec<(u32, String)>, // (part_number, etag)
    ) -> Result<()>;
    
    /// Abort a multipart upload
    async fn abort_multipart_upload(&self, upload_id: &str, key: &Path) -> Result<()>;
    
    /// List multipart uploads in progress
    async fn list_multipart_uploads(&self, prefix: Option<&str>) -> Result<Vec<MultipartUpload>>;
    
    // =========================================================================
    // Presigned URL Operations
    // =========================================================================
    
    /// Generate a presigned URL for GET operation
    async fn presigned_get_url(&self, key: &Path, expires_in: Duration) -> Result<String>;
    
    /// Generate a presigned URL for PUT operation
    async fn presigned_put_url(&self, key: &Path, expires_in: Duration) -> Result<String>;
    
    /// Generate a presigned URL for DELETE operation
    async fn presigned_delete_url(&self, key: &Path, expires_in: Duration) -> Result<String>;
    
    // =========================================================================
    // Versioning Operations
    // =========================================================================
    
    /// Check if versioning is enabled
    async fn is_versioning_enabled(&self) -> Result<bool>;
    
    /// Enable versioning
    async fn enable_versioning(&self) -> Result<()>;
    
    /// Disable versioning
    async fn disable_versioning(&self) -> Result<()>;
    
    /// List object versions
    async fn list_versions(&self, key: &Path) -> Result<Vec<ObjectVersion>>;
    
    /// Get specific version of an object
    async fn get_version(&self, key: &Path, version_id: &str) -> Result<Vec<u8>>;
    
    /// Delete a specific version
    async fn delete_version(&self, key: &Path, version_id: &str) -> Result<()>;
    
    // =========================================================================
    // Metadata & Tagging Operations
    // =========================================================================
    
    /// Get object metadata/tags
    async fn get_object_metadata(&self, key: &Path) -> Result<ObjectMetadata>;
    
    /// Set object metadata/tags
    async fn set_object_metadata(&self, key: &Path, metadata: ObjectMetadata) -> Result<()>;
    
    /// Get object tags (key-value pairs)
    async fn get_object_tags(&self, key: &Path) -> Result<Vec<(String, String)>>;
    
    /// Set object tags (key-value pairs)
    async fn set_object_tags(&self, key: &Path, tags: Vec<(String, String)>) -> Result<()>;
    
    /// Delete object tags
    async fn delete_object_tags(&self, key: &Path) -> Result<()>;
    
    // =========================================================================
    // Lifecycle & Policies
    // =========================================================================
    
    /// Get lifecycle policy
    async fn get_lifecycle_policy(&self) -> Result<Option<LifecyclePolicy>>;
    
    /// Set lifecycle policy
    async fn set_lifecycle_policy(&self, policy: LifecyclePolicy) -> Result<()>;
    
    /// Delete lifecycle policy
    async fn delete_lifecycle_policy(&self) -> Result<()>;
    
    // =========================================================================
    // CORS Configuration
    // =========================================================================
    
    /// Get CORS configuration
    async fn get_cors_config(&self) -> Result<Option<CorsConfig>>;
    
    /// Set CORS configuration
    async fn set_cors_config(&self, config: CorsConfig) -> Result<()>;
    
    /// Delete CORS configuration
    async fn delete_cors_config(&self) -> Result<()>;
}

/// Multipart upload information
#[derive(Debug, Clone)]
pub struct MultipartUpload {
    /// Upload ID
    pub upload_id: String,
    
    /// Object key
    pub key: String,
    
    /// Initiated timestamp
    pub initiated: SystemTime,
    
    /// Storage class (if applicable)
    pub storage_class: Option<String>,
}

/// Object version information
#[derive(Debug, Clone)]
pub struct ObjectVersion {
    /// Version ID
    pub version_id: String,
    
    /// Is this the latest version
    pub is_latest: bool,
    
    /// Last modified timestamp
    pub last_modified: SystemTime,
    
    /// ETag
    pub etag: String,
    
    /// Size in bytes
    pub size: u64,
    
    /// Storage class
    pub storage_class: Option<String>,
    
    /// Is delete marker
    pub is_delete_marker: bool,
}

/// Object metadata
#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    /// Content type
    pub content_type: Option<String>,
    
    /// Content encoding
    pub content_encoding: Option<String>,
    
    /// Content disposition
    pub content_disposition: Option<String>,
    
    /// Cache control
    pub cache_control: Option<String>,
    
    /// Custom metadata (key-value pairs)
    pub custom_metadata: Vec<(String, String)>,
    
    /// Server-side encryption (SSE) algorithm
    pub sse_algorithm: Option<String>,
    
    /// SSE key ID (if applicable)
    pub sse_key_id: Option<String>,
}

/// Lifecycle policy
#[derive(Debug, Clone)]
pub struct LifecyclePolicy {
    /// Rules for lifecycle transitions
    pub rules: Vec<LifecycleRule>,
}

/// Lifecycle rule
#[derive(Debug, Clone)]
pub struct LifecycleRule {
    /// Rule ID
    pub id: String,
    
    /// Status (Enabled/Disabled)
    pub status: LifecycleRuleStatus,
    
    /// Prefix filter
    pub prefix: Option<String>,
    
    /// Tag filter
    pub tag: Option<(String, String)>,
    
    /// Transition to standard-IA after N days
    pub transition_to_standard_ia: Option<u32>,
    
    /// Transition to glacier after N days
    pub transition_to_glacier: Option<u32>,
    
    /// Transition to deep archive after N days
    pub transition_to_deep_archive: Option<u32>,
    
    /// Expire after N days
    pub expire: Option<u32>,
    
    /// Delete expired delete markers
    pub delete_expired_delete_markers: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleRuleStatus {
    Enabled,
    Disabled,
}

/// CORS configuration
#[derive(Debug, Clone)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,
    
    /// Allowed methods
    pub allowed_methods: Vec<String>,
    
    /// Allowed headers
    pub allowed_headers: Vec<String>,
    
    /// Exposed headers
    pub exposed_headers: Vec<String>,
    
    /// Max age in seconds
    pub max_age_seconds: Option<u32>,
}
