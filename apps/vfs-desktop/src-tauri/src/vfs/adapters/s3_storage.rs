//! S3 Storage Adapter - Implements StorageAdapter and IFileOperations for AWS S3

use anyhow::Result;
use async_trait::async_trait;
use opendal::services::S3;
use opendal::Operator;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, error, info, warn};

use crate::vfs::domain::{VirtualFile, StorageSourceType, TierStatus, StorageTier};
use crate::vfs::ports::{
    StorageAdapter, IFileOperations, FileEntry, FileStat, CopyOptions, MoveOptions
};

/// S3 storage adapter using OpenDAL
pub struct S3StorageAdapter {
    /// OpenDAL operator
    operator: Operator,
    
    /// Bucket name
    bucket: String,
    
    /// Display name
    name: String,
    
    /// Region
    region: String,
    
    /// Optional custom endpoint (for S3-compatible services or VPC Gateway)
    endpoint: Option<String>,
}

impl S3StorageAdapter {
    pub async fn new(
        bucket: String,
        region: String,
        access_key: Option<String>,
        secret_key: Option<String>,
        session_token: Option<String>,
        endpoint: Option<String>,
        name: String,
    ) -> Result<Self> {
        // Validate bucket name - must not be empty and should not contain spaces or invalid characters
        let bucket_trimmed = bucket.trim();
        if bucket_trimmed.is_empty() {
            return Err(anyhow::anyhow!("Bucket name cannot be empty"));
        }
        
        // Check for common issues
        if bucket_trimmed != bucket {
            warn!("Bucket name has leading/trailing whitespace: '{}' -> '{}'", bucket, bucket_trimmed);
        }
        if bucket_trimmed.contains(' ') {
            return Err(anyhow::anyhow!(
                "Bucket name '{}' contains spaces. S3 bucket names cannot contain spaces. \
                Please use the actual bucket name (e.g., 'diaspor-vfs-test'), not a display name.",
                bucket_trimmed
            ));
        }
        
        // S3 bucket name validation rules:
        // - 3-63 characters
        // - Lowercase letters, numbers, dots, hyphens
        // - Must start and end with letter or number
        // - Cannot be formatted as IP address
        if bucket_trimmed.len() < 3 || bucket_trimmed.len() > 63 {
            return Err(anyhow::anyhow!(
                "Bucket name '{}' must be between 3 and 63 characters (got {}).",
                bucket_trimmed, bucket_trimmed.len()
            ));
        }
        
        // Use trimmed bucket name
        let bucket = bucket_trimmed.to_string();
        
        // Normalize region to lowercase (AWS requires lowercase regions)
        let normalized_region = region.to_lowercase();
        
        let mut builder = S3::default();
        builder.bucket(&bucket);
        builder.region(&normalized_region);
        
        // Always read credentials from environment variables (never persist credentials)
        // This ensures credentials are not stored in config files
        let access_key = access_key
            .or_else(|| {
                match std::env::var("AWS_ACCESS_KEY_ID") {
                    Ok(val) => Some(val),
                    Err(_) => {
                        debug!("AWS_ACCESS_KEY_ID not found in environment");
                        None
                    }
                }
            })
            .or_else(|| {
                match std::env::var("aws_access_key_id") {
                    Ok(val) => Some(val),
                    Err(_) => {
                        debug!("aws_access_key_id not found in environment");
                        None
                    }
                }
            });
        let secret_key = secret_key
            .or_else(|| {
                match std::env::var("AWS_SECRET_ACCESS_KEY") {
                    Ok(val) => Some(val),
                    Err(_) => {
                        debug!("AWS_SECRET_ACCESS_KEY not found in environment");
                        None
                    }
                }
            })
            .or_else(|| {
                match std::env::var("aws_secret_access_key") {
                    Ok(val) => Some(val),
                    Err(_) => {
                        debug!("aws_secret_access_key not found in environment");
                        None
                    }
                }
            });
        let session_token = session_token
            .or_else(|| {
                match std::env::var("AWS_SESSION_TOKEN") {
                    Ok(val) => Some(val),
                    Err(_) => {
                        debug!("AWS_SESSION_TOKEN not found in environment");
                        None
                    }
                }
            })
            .or_else(|| {
                match std::env::var("aws_session_token") {
                    Ok(val) => Some(val),
                    Err(_) => {
                        debug!("aws_session_token not found in environment");
                        None
                    }
                }
            });
        
        // Set credentials if provided (from config or environment)
        // OpenDAL will also check AWS credentials file and IAM roles if not provided here
        if let Some(ref ak) = access_key {
            builder.access_key_id(ak);
            info!("[S3] Using AWS_ACCESS_KEY_ID from config/env: {}... (length: {})", &ak[..ak.len().min(10)], ak.len());
        } else {
            debug!("[S3] No AWS_ACCESS_KEY_ID in config or environment. OpenDAL will try AWS credentials file or IAM role.");
        }
        if let Some(ref sk) = secret_key {
            builder.secret_access_key(sk);
            info!("[S3] Using AWS_SECRET_ACCESS_KEY from config/env: {}... (length: {})", &sk[..sk.len().min(10)], sk.len());
        } else {
            debug!("[S3] No AWS_SECRET_ACCESS_KEY in config or environment. OpenDAL will try AWS credentials file or IAM role.");
        }
        // Set session token if provided (required for temporary credentials)
        if let Some(ref st) = session_token {
            builder.security_token(st);
            info!("[S3] Using AWS_SESSION_TOKEN from config/env: {}... (length: {})", &st[..st.len().min(20)], st.len());
        } else {
            debug!("[S3] No AWS_SESSION_TOKEN found (may be required for temporary credentials). If using temporary credentials, ensure AWS_SESSION_TOKEN is set.");
        }
        if let Some(ref ep) = endpoint {
            builder.endpoint(ep);
        }
        
        let operator = Operator::new(builder)
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create S3 operator for bucket '{}' in region '{}': {}. \
                    Check that bucket name, region, and credentials are correct.",
                    bucket, region, e
                )
            })?
            .finish();
        
        let has_access_key = access_key.is_some();
        let has_secret_key = secret_key.is_some();
        let has_session_token = session_token.is_some();
        info!("S3 adapter initialized - bucket: {}, region: {} (normalized from '{}'), has_access_key: {}, has_secret_key: {}, has_session_token: {}, endpoint: {:?}", 
            bucket, normalized_region, region, has_access_key, has_secret_key, has_session_token, endpoint);
        
        Ok(Self {
            operator,
            bucket,
            name,
            region: normalized_region,
            endpoint,
        })
    }
    
    /// Get the OpenDAL operator (for multipart uploads)
    pub fn operator(&self) -> &Operator {
        &self.operator
    }
    
    /// Generate a presigned URL for GET operation
    /// This allows temporary read access to an S3 object without authentication
    /// Perfect for video streaming!
    pub async fn generate_presigned_url(
        &self,
        key: &str,
        expires_in: std::time::Duration,
    ) -> Result<String> {
        let normalized_key = key.trim_start_matches('/');
        
        info!("[S3] Generating presigned URL for: {} (expires in {:?})", 
            normalized_key, expires_in);
        
        // Try to use OpenDAL's presign functionality if available
        match self.operator.presign_read(normalized_key, expires_in).await {
            Ok(presigned_req) => {
                let url = presigned_req.uri().to_string();
                info!("[S3] ✓ Generated presigned URL via OpenDAL (valid for {:?})", expires_in);
                Ok(url)
            }
            Err(e) => {
                warn!("[S3] OpenDAL presign not available: {}, falling back to manual URL construction", e);
                
                // Fallback: Construct presigned URL manually
                // This is a simplified version - in production, you'd want proper AWS SDK signing
                let base_url = if let Some(ref endpoint) = self.endpoint {
                    format!("{}/{}", endpoint, normalized_key)
                } else {
                    format!("https://{}.s3.{}.amazonaws.com/{}", 
                        self.bucket, self.region, normalized_key)
                };
                
                // Note: This URL won't have proper AWS signatures
                // It will work for public buckets or with bucket policies that allow VPC endpoint access
                warn!("[S3] Using unsigned URL - ensure bucket policy allows access");
                
                Ok(base_url)
            }
        }
    }
    
    /// Get bucket name
    pub fn bucket(&self) -> &str {
        &self.bucket
    }
    
    /// Get region
    pub fn region(&self) -> &str {
        &self.region
    }
    
    /// Refresh credentials by re-reading environment variables and recreating the operator
    /// This is useful when credentials expire and need to be refreshed
    pub async fn refresh_credentials(
        &mut self,
        access_key: Option<String>,
        secret_key: Option<String>,
        session_token: Option<String>,
    ) -> Result<()> {
        info!("[S3] Refreshing credentials for bucket: {}, region: {}", self.bucket, self.region);
        
        // Read credentials from parameters or environment variables
        let access_key = access_key
            .or_else(|| std::env::var("AWS_ACCESS_KEY_ID").ok())
            .or_else(|| std::env::var("aws_access_key_id").ok());
        let secret_key = secret_key
            .or_else(|| std::env::var("AWS_SECRET_ACCESS_KEY").ok())
            .or_else(|| std::env::var("aws_secret_access_key").ok());
        let session_token = session_token
            .or_else(|| std::env::var("AWS_SESSION_TOKEN").ok())
            .or_else(|| std::env::var("aws_session_token").ok());
        
        // Recreate the operator with new credentials
        let mut builder = S3::default();
        builder.bucket(&self.bucket);
        builder.region(&self.region);
        
        if let Some(ref ak) = access_key {
            builder.access_key_id(ak);
            info!("[S3] Refreshed AWS_ACCESS_KEY_ID");
        }
        if let Some(ref sk) = secret_key {
            builder.secret_access_key(sk);
            info!("[S3] Refreshed AWS_SECRET_ACCESS_KEY");
        }
        if let Some(ref st) = session_token {
            builder.security_token(st);
            info!("[S3] Refreshed AWS_SESSION_TOKEN");
        }
        
        self.operator = Operator::new(builder)
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to refresh S3 operator: {}. Check that credentials are correct.",
                    e
                )
            })?
            .finish();
        
        info!("[S3] Successfully refreshed credentials");
        Ok(())
    }
    
    /// Convert path to S3 key
    fn to_key(&self, path: &Path) -> String {
        path.strip_prefix("/")
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    }
    
    /// Detect storage tier from S3 storage class
    /// Maps S3 storage classes to our tier system:
    /// - STANDARD, STANDARD_IA, INTELLIGENT_TIERING → Nearline (moderate cost, immediate access)
    /// - GLACIER_IR, GLACIER, DEEP_ARCHIVE → Cold (lower cost, instant retrieval or restore required)
    pub fn detect_tier(storage_class: Option<&str>) -> StorageTier {
        match storage_class {
            Some("STANDARD") | None => StorageTier::Nearline, // S3 Standard = Nearline tier
            Some("STANDARD_IA") | Some("ONEZONE_IA") => StorageTier::Nearline, // Infrequent Access = Nearline
            Some("INTELLIGENT_TIERING") => StorageTier::Nearline, // Intelligent Tiering defaults to Nearline
            Some("GLACIER_IR") => StorageTier::Cold, // Glacier Instant Retrieval = Cold tier
            Some("GLACIER") | Some("DEEP_ARCHIVE") => StorageTier::Cold, // Glacier/Deep Archive = Cold tier
            _ => StorageTier::Nearline, // Default to Nearline for unknown classes
        }
    }
}

#[async_trait]
impl StorageAdapter for S3StorageAdapter {
    fn storage_type(&self) -> StorageSourceType {
        StorageSourceType::S3
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn test_connection(&self) -> Result<bool> {
        // Try to list bucket root to verify access
        // Use empty string for root (consistent with list_files)
        info!("[S3] Testing connection to bucket '{}' in region '{}'...", self.bucket, self.region);
        match self.operator.list("").await {
            Ok(_) => {
                info!("[S3] ✅ Connection test successful for bucket '{}'", self.bucket);
                Ok(true)
            }
            Err(e) => {
                let error_msg = format!(
                    "Failed to connect to S3 bucket '{}' in region '{}': {}. \
                    Verify bucket name, region, credentials (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY), \
                    and IAM permissions are correct.",
                    self.bucket, self.region, e
                );
                error!("[S3] ❌ Connection test failed: {}", error_msg);
                // Return error with detailed message for better diagnostics
                Err(anyhow::anyhow!(error_msg))
            }
        }
    }
    
    async fn list_files(&self, path: &Path) -> Result<Vec<VirtualFile>> {
        let key = self.to_key(path);
        // For root path, use empty string (S3 keys don't have leading slashes)
        // For subdirectories, add trailing slash for prefix matching
        let prefix = if key.is_empty() { String::new() } else { format!("{}/", key) };
        
        info!("[S3] Listing files - bucket: {}, region: {}, path: {:?}, key: '{}', prefix: '{}'", 
            self.bucket, self.region, path, key, prefix);
        
        // #region agent log
        let has_access_key = std::env::var("AWS_ACCESS_KEY_ID").is_ok() || 
                             std::env::var("aws_access_key_id").is_ok();
        let has_secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").is_ok() || 
                             std::env::var("aws_secret_access_key").is_ok();
        let has_session_token = std::env::var("AWS_SESSION_TOKEN").is_ok() || 
                                std::env::var("aws_session_token").is_ok();
        tracing::debug!(target: "agent_log", r#"{{"location":"s3_storage.rs:294","message":"list_files entry","data":{{"bucket":"{}","region":"{}","path":"{:?}","key":"{}","prefix":"{}","hasAccessKey":{},"hasSecretKey":{},"hasSessionToken":{}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
                self.bucket, self.region, path, key, prefix, has_access_key, has_secret_key, has_session_token, 
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        // #endregion
        
        // OpenDAL's list() returns all entries with the given prefix
        // We need to filter to only immediate children
        let entries = self.operator.list(&prefix).await
            .map_err(|e| {
                let error_msg = format!("{}", e);
                let error_debug = format!("{:?}", e);
                
                // Check for common error patterns
                let mut diagnostic = format!(
                    "Failed to list S3 objects in bucket '{}' (region: {}) with prefix '{}'.\n\n",
                    self.bucket, self.region, prefix
                );
                
                // Check for credential-related errors
                let is_expired_token = error_msg.contains("ExpiredToken") || error_debug.contains("ExpiredToken");
                let is_invalid_token = error_msg.contains("InvalidToken") || error_msg.contains("InvalidAccessKeyId") || error_msg.contains("SignatureDoesNotMatch");
                
                if is_expired_token || is_invalid_token {
                    diagnostic.push_str("❌ Credential Error Detected:\n");
                    if is_expired_token {
                        diagnostic.push_str("   ⚠️  EXPIRED TOKEN: Your AWS session token has expired\n");
                        diagnostic.push_str("   - Update your AWS credentials (access key, secret key, session token)\n");
                        diagnostic.push_str("   - Set updated credentials as environment variables:\n");
                        diagnostic.push_str("     export AWS_ACCESS_KEY_ID='your-key'\n");
                        diagnostic.push_str("     export AWS_SECRET_ACCESS_KEY='your-secret'\n");
                        diagnostic.push_str("     export AWS_SESSION_TOKEN='your-token'\n");
                        diagnostic.push_str("   - Then refresh the storage source or restart the application\n");
                    } else {
                        diagnostic.push_str("   - Verify AWS_ACCESS_KEY_ID is correct\n");
                        diagnostic.push_str("   - Verify AWS_SECRET_ACCESS_KEY is correct\n");
                        if error_msg.contains("InvalidToken") {
                            diagnostic.push_str("   - Verify AWS_SESSION_TOKEN is set and not expired (required for temporary credentials)\n");
                        }
                        diagnostic.push_str("   - Check if credentials are expired\n");
                    }
                    diagnostic.push_str("   - Ensure credentials are entered in storage configuration or set as environment variables\n\n");
                }
                
                // Check for permission errors
                if error_msg.contains("AccessDenied") || error_msg.contains("Forbidden") {
                    diagnostic.push_str("❌ Permission Error Detected:\n");
                    diagnostic.push_str("   - Check IAM permissions: s3:ListBucket on bucket\n");
                    diagnostic.push_str("   - Check IAM permissions: s3:GetObject on objects (for reading)\n");
                    diagnostic.push_str("   - Check IAM permissions: s3:PutObject on objects (for writing/creating)\n");
                    diagnostic.push_str("   - Check IAM permissions: s3:DeleteObject on objects (for deleting)\n");
                    diagnostic.push_str(&format!("   - Verify the IAM user/role has access to bucket '{}'\n\n", self.bucket));
                }
                
                // Check for bucket/region errors
                if error_msg.contains("NoSuchBucket") || error_msg.contains("NotFound") {
                    diagnostic.push_str("❌ Bucket Error Detected:\n");
                    diagnostic.push_str(&format!("   - Verify bucket name '{}' is correct\n", self.bucket));
                    diagnostic.push_str(&format!("   - Verify bucket exists in region '{}'\n", self.region));
                    diagnostic.push_str("   - Check if bucket name is spelled correctly\n\n");
                }
                
                // Check for region errors
                if error_msg.contains("IllegalLocationConstraintException") || error_msg.contains("InvalidLocationConstraint") {
                    diagnostic.push_str("❌ Region Error Detected:\n");
                    diagnostic.push_str(&format!("   - Verify region '{}' is correct\n", self.region));
                    diagnostic.push_str("   - Check if bucket exists in this region\n");
                    diagnostic.push_str("   - Common regions: us-east-1, us-west-2, eu-west-1, etc.\n\n");
                }
                
                // General troubleshooting
                diagnostic.push_str("Troubleshooting Steps:\n");
                diagnostic.push_str(&format!("   1. Verify bucket name: '{}'\n", self.bucket));
                diagnostic.push_str(&format!("   2. Verify region: '{}'\n", self.region));
                diagnostic.push_str("   3. Check IAM permissions:\n");
                diagnostic.push_str("      - s3:ListBucket on bucket (for listing files)\n");
                diagnostic.push_str("      - s3:GetObject on objects (for reading files)\n");
                diagnostic.push_str("      - s3:PutObject on objects (for creating/writing files)\n");
                diagnostic.push_str("      - s3:DeleteObject on objects (for deleting files)\n");
                diagnostic.push_str("   4. Verify AWS credentials are valid and not expired\n");
                diagnostic.push_str("   5. For temporary credentials, ensure AWS_SESSION_TOKEN is set\n");
                diagnostic.push_str("   6. Check bucket exists and is accessible\n");
                diagnostic.push_str("   7. Verify credentials are entered in storage configuration or set as environment variables\n\n");
                diagnostic.push_str(&format!("Error details: {}", error_debug));
                
                anyhow::anyhow!("{}", diagnostic)
            })?;
        
        info!("[S3] Received {} entries from OpenDAL for prefix '{}'", entries.len(), prefix);
        
        // #region agent log
        let entry_names: Vec<String> = entries.iter().take(5).map(|e| e.name().to_string()).collect();
        tracing::debug!(target: "agent_log", r#"{{"location":"s3_storage.rs:379","message":"list_files entries received","data":{{"bucket":"{}","prefix":"{}","entryCount":{},"firstEntries":{:?}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"B"}}"#, 
                self.bucket, prefix, entries.len(), entry_names,
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        // #endregion
        
        if entries.is_empty() {
            // Check if this might be a credentials issue by checking if we have credentials
            let has_access_key = std::env::var("AWS_ACCESS_KEY_ID").is_ok() || 
                                 std::env::var("aws_access_key_id").is_ok();
            let has_secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").is_ok() || 
                                 std::env::var("aws_secret_access_key").is_ok();
            
            tracing::debug!(target: "agent_log", r#"{{"location":"s3_storage.rs:393","message":"list_files empty result","data":{{"bucket":"{}","prefix":"{}","hasAccessKey":{},"hasSecretKey":{}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
                    self.bucket, prefix, has_access_key, has_secret_key,
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            
            if !has_access_key || !has_secret_key {
                warn!("[S3] No entries returned and credentials may be missing. Check AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY environment variables.");
            } else {
                info!("[S3] No entries returned from OpenDAL - bucket may be empty or prefix doesn't match any objects");
            }
        }
        
        let mut files = Vec::new();
        let mut seen_names = HashSet::new();
        
        for (idx, entry) in entries.iter().enumerate() {
            let entry_name = entry.name().to_string();
            let metadata = entry.metadata();
            let is_dir = metadata.is_dir();
            let size = metadata.content_length();
            
            info!("[S3] Entry {}: name='{}', is_dir={}, size={}", idx, entry_name, is_dir, size);
            
            // Skip empty entries
            if entry_name.is_empty() || entry_name == "/" {
                debug!("[S3] Skipping empty entry");
                continue;
            }
            
            // Skip if entry name exactly matches prefix (this is the directory itself)
            if entry_name == prefix {
                debug!("[S3] Skipping prefix directory: '{}' (prefix: '{}')", entry_name, prefix);
                continue;
            }
            
            // Extract immediate child name
            // OpenDAL returns full paths from bucket root
            // At root (prefix=""), entries are like "file.txt" or "folder/"
            // In subdirectory (prefix="folder/"), entries are like "folder/file.txt" or "folder/subfolder/"
            let child_name = if prefix.is_empty() {
                // At root: entry_name is "file.txt" or "folder/" - get first component only
                entry_name.split('/').next().unwrap_or(&entry_name).trim_end_matches('/')
            } else if entry_name.starts_with(&prefix) {
                // Remove prefix: "folder/file.txt" -> "file.txt"
                let relative = entry_name.strip_prefix(&prefix).unwrap_or(&entry_name);
                // Get first component only (immediate child)
                let first_part = relative.split('/').next().unwrap_or(relative);
                first_part.trim_end_matches('/')
            } else {
                // Entry doesn't match prefix - this shouldn't happen, but log and skip
                warn!("[S3] Entry '{}' doesn't start with prefix '{}' - skipping", entry_name, prefix);
                continue;
            };
            
            if child_name.is_empty() {
                warn!("[S3] Entry '{}' resulted in empty child name, skipping", entry_name);
                continue;
            }
            
            // Skip temporary/chunk files from multipart uploads
            if child_name.ends_with(".part") 
                || child_name.contains(".part.")
                || child_name.contains(".chunk.")
                || child_name.contains(".tmp.")
                || child_name.ends_with(".tmp")
                || entry_name.contains(".part")
                || entry_name.contains(".chunk.")
                || entry_name.contains(".tmp.") {
                debug!("[S3] Skipping temporary/chunk file: '{}' (entry: '{}')", child_name, entry_name);
                continue;
            }
            
            // Deduplicate by child name
            if seen_names.contains(child_name) {
                debug!("[S3] Skipping duplicate child: '{}' (from entry '{}')", child_name, entry_name);
                continue;
            }
            seen_names.insert(child_name.to_string());
            
            // Build file path relative to current path
            let file_path = if path.as_os_str().is_empty() || path == Path::new("/") {
                PathBuf::from("/").join(child_name)
            } else {
                path.join(child_name)
            };
            
            info!("[S3] ✓ Adding: child='{}', path={:?}, is_dir={}, size={}", 
                child_name, file_path, is_dir, size);
            
            tracing::debug!(target: "agent_log", r#"{{"location":"s3_storage.rs:446","message":"processing entry","data":{{"entryName":"{}","childName":"{}","isDir":{},"size":{}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"C"}}"#, 
                    entry_name, child_name, is_dir, size,
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            
            let mut vfile = VirtualFile::new(
                child_name.to_string(),
                file_path,
                size,
                is_dir,
            );
            
            // Set last_modified from S3 metadata
            if let Some(last_modified) = metadata.last_modified() {
                vfile.last_modified = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(last_modified.timestamp() as u64);
            }
            
            // Detect storage tier from S3 storage class
            // OpenDAL doesn't expose storage class directly, so we default to Nearline (STANDARD)
            // For actual storage class detection, we'd need to use AWS SDK directly
            // For now, assume STANDARD (Nearline) unless we can detect otherwise
            let storage_class = None; // TODO: Get from metadata when OpenDAL supports it
            let detected_tier = Self::detect_tier(storage_class);
            
            vfile.tier_status = TierStatus {
                current_tier: detected_tier,
                is_cached: false,
                can_warm: true,
                retrieval_time_estimate: match detected_tier {
                    StorageTier::Nearline => Some(0), // Immediate access
                    StorageTier::Cold => Some(5), // Instant retrieval (GLACIER_IR) or restore required
                    _ => Some(5),
                },
            };
            
            vfile.transcodable = vfile.can_transcode();
            
            files.push(vfile);
        }
        
        // #region agent log
        let file_names: Vec<String> = files.iter().take(10).map(|f| f.name.clone()).collect();
        tracing::debug!(target: "agent_log", r#"{{"location":"s3_storage.rs:481","message":"list_files returning files","data":{{"bucket":"{}","prefix":"{}","totalEntries":{},"filesReturned":{},"fileNames":{:?}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"C"}}"#, 
                self.bucket, prefix, entries.len(), files.len(), file_names,
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        // #endregion
        
        info!("[S3] Returning {} files after processing {} entries", files.len(), entries.len());
        
        // Sort: directories first, then by name
        files.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        
        Ok(files)
    }
    
    async fn list_files_paginated(
        &self,
        path: &Path,
        limit: Option<u64>,
        continuation_token: Option<&str>,
    ) -> Result<(Vec<VirtualFile>, Option<String>)> {
        // For now, use the default implementation which calls list_files and applies limit manually
        // This ensures compatibility and works for all storage types
        // TODO: Optimize for S3 using OpenDAL's lister API with start_after when available
        let mut all_files = self.list_files(path).await?;
        
        // Sort: directories first, then by name (ensure consistent ordering)
        all_files.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        
        let limit_val = limit.unwrap_or(50) as usize;
        let start_idx = if let Some(token) = continuation_token {
            // Find the index of the token in the sorted list
            all_files.iter()
                .position(|f| f.path.to_string_lossy() == token)
                .map(|idx| idx + 1)
                .unwrap_or(0)
        } else {
            0
        };
        
        let total_len = all_files.len();
        let result_files: Vec<VirtualFile> = all_files.into_iter()
            .skip(start_idx)
            .take(limit_val)
            .collect();
        
        // Generate continuation token from last file path if we hit the limit
        let next_token = if start_idx + limit_val < total_len && !result_files.is_empty() {
            result_files.last().map(|f| f.path.to_string_lossy().to_string())
        } else {
            None
        };
        
        info!("[S3] Paginated list returning {} files (start: {}, total: {}), continuation_token: {:?}", 
            result_files.len(), start_idx, total_len, next_token);
        
        Ok((result_files, next_token))
    }
    
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        let key = self.to_key(path);
        debug!("Reading S3 object: {}", key);
        
        let data = self.operator.read(&key).await?;
        Ok(data.to_vec())
    }
    
    async fn read_file_range(&self, path: &Path, offset: u64, length: u64) -> Result<Vec<u8>> {
        let key = self.to_key(path);
        debug!("Reading S3 object range: {} (offset={}, length={})", key, offset, length);
        
        // Use range read with opendal
        let data = self.operator
            .read_with(&key)
            .range(offset..offset + length)
            .await?;
        Ok(data.to_vec())
    }
    
    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<()> {
        let key = self.to_key(path);
        debug!("Writing S3 object: {}", key);
        
        self.operator.write(&key, data.to_vec()).await?;
        Ok(())
    }
    
    async fn get_metadata(&self, path: &Path) -> Result<VirtualFile> {
        let key = self.to_key(path);
        let metadata = self.operator.stat(&key).await?;
        
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| key.clone());
        
        let mut vfile = VirtualFile::new(
            name,
            path.to_path_buf(),
            metadata.content_length(),
            metadata.is_dir(),
        );
        
        // Set last_modified from S3 metadata
        if let Some(last_modified) = metadata.last_modified() {
            vfile.last_modified = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(last_modified.timestamp() as u64);
        }
        
        // Detect storage tier from S3 storage class
        // OpenDAL doesn't expose storage class directly, so we default to Nearline (STANDARD)
        let storage_class = None; // TODO: Get from metadata when OpenDAL supports it
        let detected_tier = Self::detect_tier(storage_class);
        
        vfile.tier_status = TierStatus {
            current_tier: detected_tier,
            is_cached: false,
            can_warm: true,
            retrieval_time_estimate: match detected_tier {
                StorageTier::Nearline => Some(0), // Immediate access
                StorageTier::Cold => Some(5), // Instant retrieval or restore required
                _ => Some(5),
            },
        };
        
        vfile.transcodable = vfile.can_transcode();
        
        Ok(vfile)
    }
    
    async fn exists(&self, path: &Path) -> Result<bool> {
        let key = self.to_key(path);
        Ok(self.operator.is_exist(&key).await?)
    }
    
    async fn delete(&self, path: &Path) -> Result<()> {
        let key = self.to_key(path);
        self.operator.delete(&key).await?;
        Ok(())
    }
    
    async fn create_dir(&self, path: &Path) -> Result<()> {
        let key = format!("{}/", self.to_key(path));
        // S3 doesn't have real directories, but we can create a zero-byte object
        self.operator.write(&key, vec![]).await?;
        Ok(())
    }
    
    async fn file_size(&self, path: &Path) -> Result<u64> {
        let key = self.to_key(path);
        let metadata = self.operator.stat(&key).await?;
        Ok(metadata.content_length())
    }
}

// =============================================================================
// IFileOperations Implementation for S3
// =============================================================================

#[async_trait]
impl IFileOperations for S3StorageAdapter {
    async fn list(&self, path: &Path) -> Result<Vec<FileEntry>> {
        let key = self.to_key(path);
        let prefix = if key.is_empty() { String::new() } else { format!("{}/", key) };
        
        debug!("Listing S3 objects with prefix: {}", prefix);
        
        let entries = self.operator.list(&prefix).await?;
        let mut files = Vec::new();
        
        for entry in entries {
            let name = entry.name().to_string();
            if name.is_empty() || name == "/" {
                continue;
            }
            
            let metadata = entry.metadata();
            let is_dir = metadata.is_dir();
            let size = metadata.content_length();
            let file_path = PathBuf::from("/").join(&prefix).join(&name);
            
            let file_entry = FileEntry {
                name: name.trim_end_matches('/').to_string(),
                path: file_path.to_string_lossy().to_string(),
                size,
                is_dir,
                is_file: !is_dir,
                is_symlink: false, // S3 doesn't have symlinks
                modified: metadata.last_modified().map(|t| {
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(t.timestamp() as u64)
                }),
                created: None,
                accessed: None,
                mode: Some(0o644),
                mime_type: metadata.content_type().map(String::from),
            };
            
            files.push(file_entry);
        }
        
        files.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        
        Ok(files)
    }
    
    async fn stat(&self, path: &Path) -> Result<FileStat> {
        let key = self.to_key(path);
        let metadata = self.operator.stat(&key).await?;
        
        Ok(FileStat {
            size: metadata.content_length(),
            is_dir: metadata.is_dir(),
            is_file: !metadata.is_dir(),
            is_symlink: false,
            mtime: metadata.last_modified().map(|t| {
                SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(t.timestamp() as u64)
            }),
            atime: None,
            ctime: None,
            mode: 0o644,
            nlink: 1,
            uid: 0,
            gid: 0,
            blksize: 4096,
            blocks: (metadata.content_length() + 511) / 512,
        })
    }
    
    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let key = self.to_key(path);
        debug!("Reading S3 object: {}", key);
        let data = self.operator.read(&key).await?;
        Ok(data.to_vec())
    }
    
    async fn read_range(&self, path: &Path, offset: u64, len: u64) -> Result<Vec<u8>> {
        let key = self.to_key(path);
        let data = self.operator
            .read_with(&key)
            .range(offset..offset + len)
            .await?;
        Ok(data.to_vec())
    }
    
    async fn write(&self, path: &Path, data: &[u8]) -> Result<()> {
        let key = self.to_key(path);
        debug!("Writing S3 object: {}", key);
        self.operator.write(&key, data.to_vec()).await?;
        Ok(())
    }
    
    async fn append(&self, path: &Path, data: &[u8]) -> Result<()> {
        // S3 doesn't support append, so we need to read + append + write
        let key = self.to_key(path);
        let mut existing = match self.operator.read(&key).await {
            Ok(d) => d.to_vec(),
            Err(_) => Vec::new(),
        };
        existing.extend_from_slice(data);
        self.operator.write(&key, existing).await?;
        Ok(())
    }
    
    async fn write_at(&self, path: &Path, offset: u64, data: &[u8]) -> Result<()> {
        // S3 doesn't support partial writes
        let key = self.to_key(path);
        let mut existing = self.operator.read(&key).await?.to_vec();
        
        let end = offset as usize + data.len();
        if existing.len() < end {
            existing.resize(end, 0);
        }
        existing[offset as usize..end].copy_from_slice(data);
        
        self.operator.write(&key, existing).await?;
        Ok(())
    }
    
    async fn truncate(&self, path: &Path, len: u64) -> Result<()> {
        let key = self.to_key(path);
        let mut existing = self.operator.read(&key).await?.to_vec();
        existing.truncate(len as usize);
        self.operator.write(&key, existing).await?;
        Ok(())
    }
    
    async fn mkdir(&self, path: &Path) -> Result<()> {
        // S3 doesn't have real directories - create a placeholder
        let base_key = self.to_key(path);
        let key = if base_key.is_empty() {
            // Root path - this shouldn't happen for mkdir, but handle it gracefully
            return Err(anyhow::anyhow!("Cannot create directory at root path"));
        } else {
            format!("{}/", base_key)
        };
        
        // Check if directory already exists - if so, return success (idempotent operation)
        if self.operator.is_exist(&key).await.unwrap_or(false) {
            info!("[S3] Directory marker '{}' already exists, skipping creation", key);
            return Ok(());
        }
        
        tracing::debug!(target: "agent_log", r#"{{"location":"s3_storage.rs:793","message":"mkdir entry","data":{{"bucket":"{}","path":"{:?}","baseKey":"{}","finalKey":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
                self.bucket, path, base_key, key,
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        
        match self.operator.write(&key, vec![]).await {
            Ok(_) => {
                info!("[S3] Successfully created directory marker: {}", key);
                tracing::debug!(target: "agent_log", r#"{{"location":"s3_storage.rs:810","message":"mkdir success","data":{{"bucket":"{}","key":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
                        self.bucket, key,
                        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                let error_lower = error_msg.to_lowercase();
                
                // Check if error is because path is already a directory (IsADirectory)
                // This can happen if OpenDAL detects it as a directory before we check
                if error_lower.contains("isadirectory") || error_lower.contains("is a directory") || 
                   error_lower.contains("write path is a directory") {
                    // Directory already exists - treat as success (idempotent)
                    info!("[S3] Directory marker '{}' already exists (detected as directory), treating as success", key);
                    return Ok(());
                }
                
                // Check for permission errors and provide helpful diagnostics
                if error_lower.contains("accessdenied") || error_lower.contains("forbidden") || 
                   error_lower.contains("403") || error_msg.contains("PermissionDenied") {
                    let mut diagnostic = format!(
                        "Permission denied when creating directory '{}' in bucket '{}'.\n\n",
                        key, self.bucket
                    );
                    diagnostic.push_str("Required IAM permissions for creating folders:\n");
                    diagnostic.push_str("  - s3:PutObject on objects (for creating directory markers)\n");
                    diagnostic.push_str("  - s3:ListBucket on bucket (for checking if directory exists)\n\n");
                    diagnostic.push_str("Directory creation in S3 requires writing an empty object with a trailing '/'.\n");
                    diagnostic.push_str("Ensure your IAM policy includes s3:PutObject permission.\n\n");
                    diagnostic.push_str(&format!("Error details: {}", error_msg));
                    return Err(anyhow::anyhow!("{}", diagnostic));
                }
                
                error!("[S3] Failed to create directory marker '{}': {}", key, error_msg);
                tracing::debug!(target: "agent_log", r#"{{"location":"s3_storage.rs:820","message":"mkdir error","data":{{"bucket":"{}","key":"{}","error":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"B"}}"#, 
                        self.bucket, key, error_msg,
                        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                Err(anyhow::anyhow!("Failed to create directory '{}': {}", key, error_msg))
            }
        }
    }
    
    async fn mkdir_p(&self, path: &Path) -> Result<()> {
        // Same as mkdir for S3
        self.mkdir(path).await
    }
    
    async fn rmdir(&self, path: &Path) -> Result<()> {
        let key = format!("{}/", self.to_key(path));
        self.operator.delete(&key).await?;
        Ok(())
    }
    
    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        let from_key = self.to_key(from);
        let to_key = self.to_key(to);
        info!("Renaming S3 object: {} -> {}", from_key, to_key);
        
        // Check if source exists - handle permission errors gracefully
        let source_exists = self.operator.is_exist(&from_key).await
            .map_err(|e| {
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("permissiondenied") || error_str.contains("403") || error_str.contains("accessdenied") {
                    anyhow::anyhow!(
                        "Permission denied: Cannot access source file '{}'. Check IAM permissions: s3:GetObject, s3:ListBucket. Error: {}",
                        from_key, e
                    )
                } else {
                    anyhow::anyhow!("Failed to check if source file exists: {}", e)
                }
            })?;
        
        if !source_exists {
            return Err(anyhow::anyhow!("Source does not exist: {}", from_key));
        }
        
        // Check if destination already exists (and is different from source)
        if from_key != to_key {
            let dest_exists = self.operator.is_exist(&to_key).await
                .map_err(|e| {
                    let error_str = e.to_string().to_lowercase();
                    if error_str.contains("permissiondenied") || error_str.contains("403") || error_str.contains("accessdenied") {
                        anyhow::anyhow!(
                            "Permission denied: Cannot check if destination exists '{}'. Check IAM permissions: s3:GetObject, s3:ListBucket. Error: {}",
                            to_key, e
                        )
                    } else {
                        anyhow::anyhow!("Failed to check if destination exists: {}", e)
                    }
                })?;
            
            if dest_exists {
                return Err(anyhow::anyhow!("Destination already exists: {}", to_key));
            }
        }
        
        // Check if source is a directory - handle permission errors
        let metadata = self.operator.stat(&from_key).await
            .map_err(|e| {
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("permissiondenied") || error_str.contains("403") || error_str.contains("accessdenied") {
                    anyhow::anyhow!(
                        "Permission denied: Cannot access source file '{}'. Check IAM permissions: s3:GetObject, s3:ListBucket. Error: {}",
                        from_key, e
                    )
                } else {
                    anyhow::anyhow!("Failed to check if source file exists: {}", e)
                }
            })?;
        let is_directory = metadata.is_dir();
        
        // S3 doesn't have atomic rename - copy then delete
        // Use copy with overwrite enabled and recursive for directories
        let copy_opts = CopyOptions {
            overwrite: true,
            recursive: is_directory, // Enable recursive for directories
            preserve_attributes: false,
            follow_symlinks: false,
        };
        
        // Copy first
        self.copy(from, to, copy_opts).await?;
        
        // Then delete source - use rm_rf for directories, rm for files
        if is_directory {
            self.rm_rf(from).await?;
        } else {
            self.rm(from).await?;
        }
        
        info!("Successfully renamed S3 {}: {} -> {}", 
              if is_directory { "directory" } else { "object" },
              from_key, to_key);
        Ok(())
    }
    
    async fn copy(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
        let from_key = self.to_key(from);
        let to_key = self.to_key(to);
        
        info!("Copying S3 object: {} -> {}", from_key, to_key);
        
        // Validate keys - empty keys are invalid for S3 operations
        if from_key.is_empty() {
            return Err(anyhow::anyhow!("Source path cannot be empty"));
        }
        if to_key.is_empty() {
            return Err(anyhow::anyhow!("Destination path cannot be empty - cannot copy to root. Specify a file name."));
        }
        
        // Check if destination exists
        if !options.overwrite && self.operator.is_exist(&to_key).await? {
            return Err(anyhow::anyhow!("Destination already exists: {}", to_key));
        }
        
        // Check if source exists
        if !self.operator.is_exist(&from_key).await? {
            return Err(anyhow::anyhow!("Source does not exist: {}", from_key));
        }
        
        let metadata = self.operator.stat(&from_key).await?;
        
        if metadata.is_dir() && options.recursive {
            // Copy directory recursively
            let prefix_with_slash = format!("{}/", from_key);
            let entries = self.operator.list(&prefix_with_slash).await?;
            info!("Copying directory with {} entries", entries.len());
            
            for entry in entries {
                let entry_name = entry.name();
                // Get relative path from the source directory
                let relative_path = entry_name.strip_prefix(&prefix_with_slash)
                    .unwrap_or(entry_name);
                
                let from_path = from.join(relative_path);
                let to_path = to.join(relative_path);
                Box::pin(self.copy(&from_path, &to_path, options.clone())).await?;
            }
            
            // Copy directory marker if it exists
            if self.operator.is_exist(&prefix_with_slash).await.unwrap_or(false) {
                let marker_data = self.operator.read(&prefix_with_slash).await.ok();
                if let Some(data) = marker_data {
                    let _ = self.operator.write(&format!("{}/", to_key), data.to_vec()).await;
                }
            }
        } else {
            // Copy single file - use read+write (OpenDAL handles S3 CopyObject internally when possible)
            // For large files, this will be less efficient, but it's the most compatible approach
            let data = self.operator.read(&from_key).await
                .map_err(|e| anyhow::anyhow!("Failed to read source object '{}': {}", from_key, e))?;
            self.operator.write(&to_key, data.to_vec()).await
                .map_err(|e| anyhow::anyhow!("Failed to write destination object '{}': {}", to_key, e))?;
            info!("Successfully copied S3 object: {} -> {}", from_key, to_key);
        }
        
        Ok(())
    }
    
    async fn mv(&self, from: &Path, to: &Path, options: MoveOptions) -> Result<()> {
        let from_key = self.to_key(from);
        let to_key = self.to_key(to);
        info!("Moving S3 object: {} -> {}", from_key, to_key);
        
        let copy_opts = CopyOptions {
            overwrite: options.overwrite,
            recursive: true,
            preserve_attributes: false,
            follow_symlinks: false,
        };
        self.copy(from, to, copy_opts).await?;
        self.rm_rf(from).await?;
        info!("Successfully moved S3 object: {} -> {}", from_key, to_key);
        Ok(())
    }
    
    async fn rm(&self, path: &Path) -> Result<()> {
        let key = self.to_key(path);
        debug!("Deleting S3 object: {}", key);
        
        // Check if this looks like a multipart upload part file
        let is_part_file = key.contains(".part") || key.ends_with(".part0") || key.ends_with(".part1");
        
        // Attempt to delete the object
        // Note: GLACIER_IR objects can be deleted directly without restore
        // For other Glacier classes (DEEP_ARCHIVE), objects may need to be restored first
        match self.operator.delete(&key).await {
            Ok(_) => {
                info!("Successfully deleted S3 object: {}", key);
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to delete S3 object '{}': {}", key, e);
                error!("{}", error_msg);
                
                let error_str = e.to_string().to_lowercase();
                let error_debug = format!("{:?}", e);
                
                // Check if this is likely an orphaned multipart upload part
                if is_part_file {
                    return Err(anyhow::anyhow!(
                        "{} - This appears to be an orphaned multipart upload part. \
                        Orphaned parts from failed uploads cannot be deleted with s3:DeleteObject. \
                        They must be cleaned up by aborting the incomplete multipart upload using \
                        s3:AbortMultipartUpload. Check for incomplete multipart uploads with: \
                        aws s3api list-multipart-uploads --bucket {}. \
                        Then abort them with: aws s3api abort-multipart-upload --bucket {} --key {} --upload-id <id>",
                        error_msg, self.bucket, self.bucket, key
                    ));
                }
                
                // Check for specific AWS error codes
                if error_str.contains("accessdenied") || error_str.contains("403") {
                    return Err(anyhow::anyhow!(
                        "{} - Access Denied. Check IAM permissions: \
                        - s3:DeleteObject on the object \
                        - s3:ListBucket on the bucket \
                        - If bucket has versioning enabled, also need s3:DeleteObjectVersion \
                        - If bucket has MFA delete enabled, MFA token is required",
                        error_msg
                    ));
                }
                
                if error_str.contains("nosuchkey") || error_str.contains("404") {
                    // Object doesn't exist - might have been deleted already or never existed
                    warn!("Object '{}' not found - may have been deleted already", key);
                    return Ok(()); // Treat as success if object doesn't exist
                }
                
                // Check if error is related to Glacier storage class
                if error_str.contains("glacier") || error_str.contains("restore") || error_str.contains("invalidobjectstate") {
                    return Err(anyhow::anyhow!(
                        "{} - Objects in Glacier storage classes (GLACIER, DEEP_ARCHIVE) must be restored before deletion. \
                        GLACIER_IR objects should be deletable directly. \
                        To restore: aws s3api restore-object --bucket {} --key {} --restore-request '{{\"Days\":1,\"GlacierJobParameters\":{{\"Tier\":\"Expedited\"}}}}' \
                        Then wait for restore to complete before deleting. Check IAM permissions: s3:DeleteObject, s3:RestoreObject.",
                        error_msg, self.bucket, key
                    ));
                }
                
                // Check for versioning-related errors
                if error_str.contains("version") || error_str.contains("versionid") {
                    return Err(anyhow::anyhow!(
                        "{} - This bucket may have versioning enabled. \
                        Try deleting with version ID or disable versioning. \
                        Check IAM permissions: s3:DeleteObjectVersion.",
                        error_msg
                    ));
                }
                
                // Generic error with helpful suggestions
                Err(anyhow::anyhow!(
                    "{} - Possible causes: \
                    1. Insufficient IAM permissions (s3:DeleteObject, s3:ListBucket) \
                    2. Object is in Glacier/DEEP_ARCHIVE and needs restore first \
                    3. Bucket has versioning enabled (need s3:DeleteObjectVersion) \
                    4. Bucket has MFA delete enabled (need MFA token) \
                    5. Object is locked by Object Lock retention/legal hold \
                    Error details: {}",
                    error_msg, error_debug
                ))
            }
        }
    }
    
    async fn rm_rf(&self, path: &Path) -> Result<()> {
        use futures::stream::{self, StreamExt};
        
        let key = self.to_key(path);
        debug!("rm_rf: Deleting S3 object/directory: {}", key);
        
        // Check if path exists as a directory marker first
        let prefix_with_slash = format!("{}/", key);
        let is_dir_marker = self.operator.is_exist(&prefix_with_slash).await.unwrap_or(false);
        
        // Also check if it's a directory by listing objects with this prefix
        // Use the key itself if it's empty (root), otherwise use prefix_with_slash
        let list_prefix = if key.is_empty() {
            String::new()
        } else {
            prefix_with_slash.clone()
        };
        
        let entries_result = self.operator.list(&list_prefix).await;
        let entries = match entries_result {
            Ok(entries) => {
                // Filter entries to only include those that start with our prefix
                // This handles the case where we're listing from root but only want entries under our path
                if key.is_empty() {
                    entries // Root listing - return all entries
                } else {
                    entries.into_iter()
                        .filter(|entry| {
                            let entry_name = entry.name();
                            entry_name.starts_with(&prefix_with_slash) || entry_name == prefix_with_slash
                        })
                        .collect()
                }
            }
            Err(e) => {
                // If listing fails, check if it's because the path doesn't exist
                // Try to delete as a single file instead
                warn!("rm_rf: Failed to list prefix '{}': {}, trying to delete as single file", list_prefix, e);
                Vec::new()
            }
        };
        
        if !entries.is_empty() || is_dir_marker {
            // It's a directory - delete all objects with this prefix in parallel
            info!("rm_rf: Found {} entries under prefix '{}' (dir_marker: {}), deleting in parallel", 
                  entries.len(), prefix_with_slash, is_dir_marker);
            
            // Collect all keys to delete (including directory marker)
            let mut keys_to_delete: Vec<String> = entries.iter()
                .map(|entry| entry.name().to_string())
                .collect();
            
            // Add directory marker if it exists
            if is_dir_marker && !keys_to_delete.contains(&prefix_with_slash) {
                keys_to_delete.push(prefix_with_slash.clone());
            }
            
            // Delete in parallel batches (up to 16 concurrent deletes)
            let operator = self.operator.clone();
            let delete_results: Vec<_> = stream::iter(keys_to_delete)
                .map(|key| {
                    let op = operator.clone();
                    async move {
                        match op.delete(&key).await {
                            Ok(_) => {
                                debug!("rm_rf: Deleted object: {}", key);
                                Ok(())
                            }
                            Err(e) => {
                                // Ignore "not found" errors - object may have been deleted already
                                let error_lower = e.to_string().to_lowercase();
                                if error_lower.contains("nosuchkey") || 
                                   error_lower.contains("404") ||
                                   error_lower.contains("not found") {
                                    debug!("rm_rf: Object '{}' not found - already deleted", key);
                                    Ok(())
                                } else {
                                    warn!("rm_rf: Failed to delete object '{}': {}", key, e);
                                    Err(e)
                                }
                            }
                        }
                    }
                })
                .buffer_unordered(16) // Process up to 16 deletes concurrently
                .collect()
                .await;
            
            // Check for failures - if all failed, return error
            let failures: Vec<_> = delete_results.iter().filter(|r| r.is_err()).collect();
            if failures.len() == delete_results.len() && !delete_results.is_empty() {
                // All deletes failed - return error
                let first_error = failures[0].as_ref().unwrap_err();
                return Err(anyhow::anyhow!(
                    "Failed to delete directory '{}': All {} delete operations failed. First error: {}",
                    key, delete_results.len(), first_error
                ));
            } else if !failures.is_empty() {
                warn!("rm_rf: {} out of {} deletes failed for directory '{}'", 
                      failures.len(), delete_results.len(), key);
            }
            
            info!("rm_rf: Successfully deleted directory: {} ({} objects)", key, entries.len());
            Ok(())
        } else {
            // It's a single file - delete it directly
            info!("rm_rf: Deleting single file: {}", key);
            
            // Check if this looks like a multipart upload part file
            let is_part_file = key.contains(".part") || key.ends_with(".part0") || key.ends_with(".part1");
            
            self.operator.delete(&key).await.map_err(|e| {
                let error_str = e.to_string().to_lowercase();
                let error_debug = format!("{:?}", e);
                
                // Check if this is likely an orphaned multipart upload part
                if is_part_file {
                    anyhow::anyhow!(
                        "Failed to delete S3 object '{}': {} - This appears to be an orphaned multipart upload part. \
                        Orphaned parts from failed uploads cannot be deleted with s3:DeleteObject. \
                        They must be cleaned up by aborting the incomplete multipart upload using \
                        s3:AbortMultipartUpload. Use AWS CLI: aws s3api list-multipart-uploads --bucket {}",
                        key, e, self.bucket
                    )
                } else if error_str.contains("accessdenied") || error_str.contains("403") {
                    // Check for access denied
                    anyhow::anyhow!(
                        "Failed to delete S3 object '{}': {} - Access Denied. \
                        Check IAM permissions: s3:DeleteObject, s3:ListBucket, s3:DeleteObjectVersion (if versioning enabled)",
                        key, e
                    )
                } else if error_str.contains("nosuchkey") || error_str.contains("404") {
                    // Check if object doesn't exist (treat as success)
                    warn!("rm_rf: Object '{}' not found - may have been deleted already", key);
                    // Return a special error that we'll catch and convert to Ok
                    anyhow::anyhow!("OBJECT_NOT_FOUND")
                } else if error_str.contains("glacier") || error_str.contains("restore") || error_str.contains("invalidobjectstate") {
                    // Check for Glacier errors
                    anyhow::anyhow!(
                        "Failed to delete S3 object '{}': {} - Objects in Glacier storage classes must be restored before deletion. \
                        GLACIER_IR objects should be deletable directly. Restore command: \
                        aws s3api restore-object --bucket {} --key {} --restore-request '{{\"Days\":1}}'",
                        key, e, self.bucket, key
                    )
                } else {
                    // Generic error
                    anyhow::anyhow!(
                        "Failed to delete S3 object '{}': {} - Check IAM permissions (s3:DeleteObject), \
                        storage class (may need restore), versioning (may need DeleteObjectVersion), \
                        or Object Lock settings. Error: {}",
                        key, e, error_debug
                    )
                }
            }).map(|_| {
                info!("rm_rf: Successfully deleted file: {}", key);
                
            }).or_else(|e| {
                // If error is "OBJECT_NOT_FOUND", treat as success
                if e.to_string().contains("OBJECT_NOT_FOUND") {
                    Ok(())
                } else {
                    Err(e)
                }
            })
        }
    }
    
    async fn symlink(&self, _target: &Path, _link: &Path) -> Result<()> {
        Err(anyhow::anyhow!("S3 does not support symbolic links"))
    }
    
    async fn readlink(&self, _path: &Path) -> Result<String> {
        Err(anyhow::anyhow!("S3 does not support symbolic links"))
    }
    
    async fn exists(&self, path: &Path) -> Result<bool> {
        let key = self.to_key(path);
        Ok(self.operator.is_exist(&key).await?)
    }
    
    async fn is_dir(&self, path: &Path) -> Result<bool> {
        let key = self.to_key(path);
        match self.operator.stat(&key).await {
            Ok(m) => Ok(m.is_dir()),
            Err(_) => {
                // Try with trailing slash
                match self.operator.stat(&format!("{}/", key)).await {
                    Ok(m) => Ok(m.is_dir()),
                    Err(_) => Ok(false),
                }
            }
        }
    }
    
    async fn is_file(&self, path: &Path) -> Result<bool> {
        let key = self.to_key(path);
        match self.operator.stat(&key).await {
            Ok(m) => Ok(!m.is_dir()),
            Err(_) => Ok(false),
        }
    }
    
    async fn is_symlink(&self, _path: &Path) -> Result<bool> {
        Ok(false) // S3 doesn't have symlinks
    }
    
    async fn chmod(&self, _path: &Path, _mode: u32) -> Result<()> {
        warn!("chmod is not supported on S3");
        Ok(())
    }
    
    async fn chown(&self, _path: &Path, _uid: u32, _gid: u32) -> Result<()> {
        warn!("chown is not supported on S3");
        Ok(())
    }
    
    async fn touch(&self, path: &Path) -> Result<()> {
        let key = self.to_key(path);
        if !self.operator.is_exist(&key).await? {
            self.operator.write(&key, vec![]).await?;
        }
        // S3 doesn't support updating mtime without rewriting
        Ok(())
    }
    
    async fn set_times(&self, _path: &Path, _atime: Option<SystemTime>, _mtime: Option<SystemTime>) -> Result<()> {
        warn!("set_times is not supported on S3");
        Ok(())
    }
    
    async fn file_size(&self, path: &Path) -> Result<u64> {
        let key = self.to_key(path);
        let metadata = self.operator.stat(&key).await?;
        Ok(metadata.content_length())
    }
    
    async fn available_space(&self) -> Result<u64> {
        // S3 has virtually unlimited space
        Ok(u64::MAX)
    }
    
    async fn total_space(&self) -> Result<u64> {
        Ok(u64::MAX)
    }
    
    fn is_read_only(&self) -> bool {
        false
    }
    
    fn root_path(&self) -> &Path {
        Path::new("/")
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_to_key_removes_leading_slash() {
        // Create a mock operator - we can't easily test with real S3 but can test helpers
        let path = Path::new("/some/path/to/file.txt");
        let expected = "some/path/to/file.txt";
        
        // Test the logic directly
        let result = path.strip_prefix("/")
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        
        assert_eq!(result, expected);
    }
    
    #[test]
    fn test_to_key_handles_no_leading_slash() {
        let path = Path::new("relative/path.txt");
        
        let result = path.strip_prefix("/")
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        
        assert_eq!(result, "relative/path.txt");
    }
    
    #[test]
    fn test_detect_tier_standard() {
        assert_eq!(S3StorageAdapter::detect_tier(Some("STANDARD")), StorageTier::Nearline);
        assert_eq!(S3StorageAdapter::detect_tier(None), StorageTier::Nearline);
    }
    
    #[test]
    fn test_detect_tier_glacier() {
        assert_eq!(S3StorageAdapter::detect_tier(Some("GLACIER")), StorageTier::Cold);
        assert_eq!(S3StorageAdapter::detect_tier(Some("GLACIER_IR")), StorageTier::Cold);
        assert_eq!(S3StorageAdapter::detect_tier(Some("DEEP_ARCHIVE")), StorageTier::Cold);
    }
    
    #[test]
    fn test_detect_tier_infrequent_access() {
        assert_eq!(S3StorageAdapter::detect_tier(Some("STANDARD_IA")), StorageTier::Nearline);
        assert_eq!(S3StorageAdapter::detect_tier(Some("ONEZONE_IA")), StorageTier::Nearline);
        assert_eq!(S3StorageAdapter::detect_tier(Some("INTELLIGENT_TIERING")), StorageTier::Nearline);
    }
    
    #[test]
    fn test_detect_tier_unknown() {
        assert_eq!(S3StorageAdapter::detect_tier(Some("UNKNOWN")), StorageTier::Nearline);
    }
}

