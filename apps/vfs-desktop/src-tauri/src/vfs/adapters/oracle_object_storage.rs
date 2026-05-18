//! Oracle Cloud Infrastructure (OCI) Object Storage Adapter
//!
//! Implements storage adapter for Oracle Object Storage using OpenDAL.
//! Oracle Object Storage is S3-compatible, so we use the S3 service with
//! Oracle-specific endpoint configuration.

use anyhow::{Context, Result};
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

/// Oracle Object Storage adapter using OpenDAL (S3-compatible)
pub struct OracleObjectStorageAdapter {
    /// OpenDAL operator
    operator: Operator,
    
    /// Bucket name
    #[allow(dead_code)]
    bucket: String,
    
    /// Display name
    name: String,
    
    /// Namespace (OCI tenancy namespace)
    #[allow(dead_code)]
    namespace: String,
    
    /// Region
    #[allow(dead_code)]
    region: String,
}

impl OracleObjectStorageAdapter {
    /// Create a new Oracle Object Storage adapter
    pub async fn new(
        bucket: String,
        namespace: String,
        region: String,
        access_key: Option<String>,
        secret_key: Option<String>,
        endpoint: Option<String>,
        name: String,
    ) -> Result<Self> {
        let bucket_trimmed = bucket.trim();
        if bucket_trimmed.is_empty() {
            return Err(anyhow::anyhow!("Bucket name cannot be empty"));
        }
        
        // Normalize region
        let normalized_region = region.to_lowercase();
        
        // Oracle Object Storage uses S3-compatible API
        // Default endpoint format: https://{namespace}.compat.objectstorage.{region}.oraclecloud.com
        let default_endpoint = if let Some(custom_endpoint) = endpoint {
            custom_endpoint
        } else {
            format!("https://{}.compat.objectstorage.{}.oraclecloud.com", namespace, normalized_region)
        };
        
        let mut builder = S3::default();
        builder.bucket(bucket_trimmed);
        builder.region(&normalized_region);
        builder.endpoint(&default_endpoint);
        
        // Force path style for Oracle Object Storage (required)
        // Note: OpenDAL S3 service uses path style by default for custom endpoints
        
        // Always read credentials from environment variables if not provided
        let access_key = access_key
            .or_else(|| {
                match std::env::var("OCI_ACCESS_KEY_ID") {
                    Ok(val) => Some(val),
                    Err(_) => {
                        match std::env::var("OCI_ACCESS_KEY") {
                            Ok(val) => Some(val),
                            Err(_) => {
                                debug!("OCI_ACCESS_KEY_ID not found in environment");
                                None
                            }
                        }
                    }
                }
            });
        
        let secret_key = secret_key
            .or_else(|| {
                match std::env::var("OCI_SECRET_ACCESS_KEY") {
                    Ok(val) => Some(val),
                    Err(_) => {
                        match std::env::var("OCI_SECRET_KEY") {
                            Ok(val) => Some(val),
                            Err(_) => {
                                debug!("OCI_SECRET_ACCESS_KEY not found in environment");
                                None
                            }
                        }
                    }
                }
            });
        
        if let Some(key) = access_key {
            builder.access_key_id(&key);
        } else {
            warn!("No Oracle Object Storage access key provided. Connection may fail.");
        }
        
        if let Some(key) = secret_key {
            builder.secret_access_key(&key);
        } else {
            warn!("No Oracle Object Storage secret key provided. Connection may fail.");
        }
        
        let operator = Operator::new(builder)?
            .finish();
        
        info!("Oracle Object Storage adapter initialized for bucket: {} (namespace: {}, region: {})", 
            bucket_trimmed, namespace, normalized_region);
        
        Ok(Self {
            operator,
            bucket: bucket_trimmed.to_string(),
            name,
            namespace,
            region: normalized_region,
        })
    }
    
    /// Get the OpenDAL operator (for multipart uploads)
    pub fn operator(&self) -> &Operator {
        &self.operator
    }
    
    /// Convert path to Oracle object name
    fn to_object_name(&self, path: &Path) -> String {
        path.strip_prefix("/")
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    }
}

#[async_trait]
impl StorageAdapter for OracleObjectStorageAdapter {
    fn storage_type(&self) -> StorageSourceType {
        StorageSourceType::S3Compatible // Oracle uses S3-compatible API
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn test_connection(&self) -> Result<bool> {
        match self.operator.list("/").await {
            Ok(_) => Ok(true),
            Err(e) => {
                error!("Oracle Object Storage connection test failed: {}", e);
                Ok(false)
            }
        }
    }
    
    async fn list_files(&self, path: &Path) -> Result<Vec<VirtualFile>> {
        let object_name = self.to_object_name(path);
        let prefix = if object_name.is_empty() { String::new() } else { format!("{}/", object_name) };
        
        info!("[OCI] Listing files - path: {:?}, object_name: '{}', prefix: '{}'", path, object_name, prefix);
        
        let entries = self.operator.list(&prefix).await
            .with_context(|| format!("Failed to list Oracle objects with prefix: {}", prefix))?;
        
        info!("[OCI] Received {} entries from OpenDAL", entries.len());
        
        let mut files: Vec<VirtualFile> = Vec::new();
        let mut seen_dirs: HashSet<String> = HashSet::new();
        
        for entry in entries {
            let entry_name = entry.name().to_string();
            
            // Remove prefix from path
            let relative_path = if prefix.is_empty() {
                entry_name.clone()
            } else {
                entry_name.strip_prefix(&prefix)
                    .unwrap_or(&entry_name)
                    .to_string()
            };
            
            // Skip empty paths
            if relative_path.is_empty() {
                continue;
            }
            
            // Skip temporary/chunk files from multipart uploads
            if relative_path.ends_with(".part") 
                || relative_path.contains(".part.")
                || relative_path.contains(".chunk.")
                || relative_path.contains(".tmp.")
                || relative_path.ends_with(".tmp")
                || entry_name.contains(".part")
                || entry_name.contains(".chunk.")
                || entry_name.contains(".tmp.") {
                debug!("[OCI] Skipping temporary/chunk file: '{}' (entry: '{}')", relative_path, entry_name);
                continue;
            }
            
            // Handle directory-like paths
            if relative_path.contains('/') {
                let dir_name = relative_path.split('/').next().unwrap();
                let dir_path = if object_name.is_empty() {
                    format!("/{}", dir_name)
                } else {
                    format!("/{}/{}", object_name, dir_name)
                };
                
                if !seen_dirs.contains(&dir_path) {
                    seen_dirs.insert(dir_path.clone());
                    let mut vfile = VirtualFile::new(
                        dir_name.to_string(),
                        PathBuf::from(&dir_path),
                        0,
                        true,
                    );
                    vfile.tier_status = TierStatus {
                        current_tier: StorageTier::Hot,
                        is_cached: false,
                        can_warm: false,
                        retrieval_time_estimate: None,
                    };
                    files.push(vfile);
                }
                continue;
            }
            
            // Get metadata for file
            let meta = entry.metadata();
            let file_path = if object_name.is_empty() {
                PathBuf::from("/").join(&relative_path)
            } else {
                PathBuf::from("/").join(&object_name).join(&relative_path)
            };
            
            let mut vfile = VirtualFile::new(
                relative_path.clone(),
                file_path.clone(),
                meta.content_length(),
                false,
            );
            
            // Set last modified time
            if let Some(last_modified) = meta.last_modified() {
                vfile.last_modified = SystemTime::UNIX_EPOCH + 
                    std::time::Duration::from_secs(last_modified.timestamp() as u64);
            }
            
            vfile.tier_status = TierStatus {
                current_tier: StorageTier::Hot,
                is_cached: false,
                can_warm: true,
                retrieval_time_estimate: None,
            };
            
            files.push(vfile);
        }
        
        info!("[OCI] Returning {} files", files.len());
        Ok(files)
    }
    
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        let object_name = self.to_object_name(path);
        info!("[OCI] Reading file: {}", object_name);
        
        let data = self.operator.read(&object_name).await
            .with_context(|| format!("Failed to read Oracle object: {}", object_name))?;
        
        Ok(data)
    }
    
    async fn read_file_range(&self, path: &Path, offset: u64, length: u64) -> Result<Vec<u8>> {
        let object_name = self.to_object_name(path);
        info!("[OCI] Reading file range: {} (offset: {}, length: {})", object_name, offset, length);
        
        let data = self.operator.read_with(&object_name)
            .range(offset..offset + length)
            .await
            .with_context(|| format!("Failed to read Oracle object range: {}", object_name))?;
        
        Ok(data)
    }
    
    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<()> {
        let object_name = self.to_object_name(path);
        info!("[OCI] Writing file: {} ({} bytes)", object_name, data.len());
        
        self.operator.write(&object_name, data.to_vec()).await
            .with_context(|| format!("Failed to write Oracle object: {}", object_name))?;
        
        Ok(())
    }
    
    async fn get_metadata(&self, path: &Path) -> Result<VirtualFile> {
        let object_name = self.to_object_name(path);
        
        let meta = self.operator.stat(&object_name).await
            .with_context(|| format!("Failed to get Oracle object metadata: {}", object_name))?;
        
        let mut vfile = VirtualFile::new(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            path.to_path_buf(),
            meta.content_length(),
            meta.is_dir(),
        );
        
        // Set last modified time
        if let Some(last_modified) = meta.last_modified() {
            vfile.last_modified = SystemTime::UNIX_EPOCH + 
                std::time::Duration::from_secs(last_modified.timestamp() as u64);
        }
        
        vfile.tier_status = TierStatus {
            current_tier: StorageTier::Hot,
            is_cached: false,
            can_warm: true,
            retrieval_time_estimate: None,
        };
        
        Ok(vfile)
    }
    
    async fn exists(&self, path: &Path) -> Result<bool> {
        let object_name = self.to_object_name(path);
        
        match self.operator.stat(&object_name).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.kind() == opendal::ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(e.into())
                }
            }
        }
    }
    
    async fn delete(&self, path: &Path) -> Result<()> {
        let object_name = self.to_object_name(path);
        info!("[OCI] Deleting file: {}", object_name);
        
        self.operator.delete(&object_name).await
            .with_context(|| format!("Failed to delete Oracle object: {}", object_name))?;
        
        Ok(())
    }
    
    async fn create_dir(&self, path: &Path) -> Result<()> {
        // Oracle Object Storage doesn't have true directories
        // Create a placeholder object to represent a directory
        let object_name = self.to_object_name(path);
        let dir_marker = format!("{}/", object_name);
        
        info!("[OCI] Creating directory marker: {}", dir_marker);
        
        // Create an empty object with trailing slash to represent directory
        self.operator.write(&dir_marker, vec![]).await
            .with_context(|| format!("Failed to create Oracle directory marker: {}", dir_marker))?;
        
        Ok(())
    }
    
    async fn file_size(&self, path: &Path) -> Result<u64> {
        let object_name = self.to_object_name(path);
        
        let meta = self.operator.stat(&object_name).await
            .with_context(|| format!("Failed to get Oracle object size: {}", object_name))?;
        
        Ok(meta.content_length())
    }
}

#[async_trait]
impl IFileOperations for OracleObjectStorageAdapter {
    async fn list(&self, path: &Path) -> Result<Vec<FileEntry>> {
        let object_name = self.to_object_name(path);
        let prefix = if object_name.is_empty() { String::new() } else { format!("{}/", object_name) };
        
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
            
            files.push(FileEntry {
                name: name.trim_end_matches('/').to_string(),
                path: file_path.to_string_lossy().to_string(),
                size,
                is_dir,
                is_file: !is_dir,
                is_symlink: false,
                modified: metadata.last_modified().map(|t| {
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(t.timestamp() as u64)
                }),
                created: None,
                accessed: None,
                mode: Some(0o644),
                mime_type: metadata.content_type().map(String::from),
            });
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
        let object_name = self.to_object_name(path);
        let metadata = self.operator.stat(&object_name).await?;
        
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
        self.read_file(path).await
    }
    
    async fn read_range(&self, path: &Path, offset: u64, len: u64) -> Result<Vec<u8>> {
        self.read_file_range(path, offset, len).await
    }
    
    async fn write(&self, path: &Path, data: &[u8]) -> Result<()> {
        self.write_file(path, data).await
    }
    
    async fn append(&self, path: &Path, data: &[u8]) -> Result<()> {
        // Oracle Object Storage doesn't support append natively
        // Read existing data, append new data, write back
        let existing = if IFileOperations::exists(self, path).await? {
            self.read_file(path).await.unwrap_or_default()
        } else {
            Vec::new()
        };
        
        let mut combined = existing;
        combined.extend_from_slice(data);
        self.write_file(path, &combined).await
    }
    
    async fn write_at(&self, path: &Path, offset: u64, data: &[u8]) -> Result<()> {
        // Oracle Object Storage doesn't support write_at natively
        // Read existing data, modify at offset, write back
        let existing = self.read_file(path).await?;
        let mut modified = existing;
        
        let end = (offset as usize + data.len()).min(modified.len());
        if end > modified.len() {
            modified.resize(end, 0);
        }
        
        modified[offset as usize..end].copy_from_slice(data);
        self.write_file(path, &modified).await
    }
    
    async fn truncate(&self, path: &Path, len: u64) -> Result<()> {
        let existing = self.read_file(path).await?;
        let truncated: Vec<u8> = existing.into_iter().take(len as usize).collect();
        self.write_file(path, &truncated).await
    }
    
    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        self.copy(from, to, CopyOptions::default()).await?;
        self.rm(from).await
    }
    
    async fn copy(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
        if !options.overwrite && IFileOperations::exists(self, to).await? {
            return Err(anyhow::anyhow!("Destination already exists"));
        }
        let data = self.read_file(from).await?;
        self.write_file(to, &data).await
    }
    
    async fn mv(&self, from: &Path, to: &Path, options: MoveOptions) -> Result<()> {
        let copy_opts = CopyOptions {
            overwrite: options.overwrite,
            recursive: true,
            preserve_attributes: false,
            follow_symlinks: false,
        };
        self.copy(from, to, copy_opts).await?;
        self.rm_rf(from).await
    }
    
    async fn rm(&self, path: &Path) -> Result<()> {
        self.delete(path).await
    }
    
    async fn rm_rf(&self, path: &Path) -> Result<()> {
        use futures::stream::{self, StreamExt};
        
        // Recursive delete for Oracle Object Storage - optimized with parallel deletion
        let object_name = self.to_object_name(path);
        let prefix = format!("{}/", object_name);
        
        // List all objects with this prefix
        let entries = self.operator.list(&prefix).await.unwrap_or_default();
        
        if !entries.is_empty() {
            info!("rm_rf: Deleting {} Oracle objects under prefix '{}'", entries.len(), prefix);
            
            // Delete in parallel (up to 16 concurrent deletes)
            let operator = self.operator.clone();
            let _: Vec<_> = stream::iter(entries)
                .map(|entry| {
                    let op = operator.clone();
                    let entry_name = entry.name().to_string();
                    async move {
                        op.delete(&entry_name).await.ok();
                    }
                })
                .buffer_unordered(16)
                .collect()
                .await;
        }
        
        // Delete the object itself and directory marker
        self.operator.delete(&object_name).await.ok();
        self.operator.delete(&prefix).await.ok();
        
        Ok(())
    }
    
    async fn mkdir(&self, path: &Path) -> Result<()> {
        self.create_dir(path).await
    }
    
    async fn mkdir_p(&self, path: &Path) -> Result<()> {
        self.mkdir(path).await
    }
    
    async fn rmdir(&self, path: &Path) -> Result<()> {
        let object_name = self.to_object_name(path);
        let dir_marker = format!("{}/", object_name);
        self.operator.delete(&dir_marker).await?;
        Ok(())
    }
    
    async fn symlink(&self, _target: &Path, _link: &Path) -> Result<()> {
        Err(anyhow::anyhow!("Oracle Object Storage does not support symbolic links"))
    }
    
    async fn readlink(&self, _path: &Path) -> Result<String> {
        Err(anyhow::anyhow!("Oracle Object Storage does not support symbolic links"))
    }
    
    async fn exists(&self, path: &Path) -> Result<bool> {
        let object_name = self.to_object_name(path);
        Ok(self.operator.is_exist(&object_name).await?)
    }
    
    async fn is_dir(&self, path: &Path) -> Result<bool> {
        let object_name = self.to_object_name(path);
        match self.operator.stat(&object_name).await {
            Ok(m) => Ok(m.is_dir()),
            Err(_) => Ok(false),
        }
    }
    
    async fn is_file(&self, path: &Path) -> Result<bool> {
        let object_name = self.to_object_name(path);
        match self.operator.stat(&object_name).await {
            Ok(m) => Ok(!m.is_dir()),
            Err(_) => Ok(false),
        }
    }
    
    async fn is_symlink(&self, _path: &Path) -> Result<bool> {
        Ok(false)
    }
    
    async fn chmod(&self, _path: &Path, _mode: u32) -> Result<()> {
        warn!("chmod not supported on Oracle Object Storage");
        Ok(())
    }
    
    async fn chown(&self, _path: &Path, _uid: u32, _gid: u32) -> Result<()> {
        warn!("chown not supported on Oracle Object Storage");
        Ok(())
    }
    
    async fn touch(&self, path: &Path) -> Result<()> {
        let object_name = self.to_object_name(path);
        if !IFileOperations::exists(self, path).await? {
            self.operator.write(&object_name, vec![]).await?;
        }
        Ok(())
    }
    
    async fn set_times(&self, _path: &Path, _atime: Option<SystemTime>, _mtime: Option<SystemTime>) -> Result<()> {
        warn!("set_times not supported on Oracle Object Storage");
        Ok(())
    }
    
    async fn file_size(&self, path: &Path) -> Result<u64> {
        let object_name = self.to_object_name(path);
        let metadata = self.operator.stat(&object_name).await?;
        Ok(metadata.content_length())
    }
    
    async fn available_space(&self) -> Result<u64> {
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
