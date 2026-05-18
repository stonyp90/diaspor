//! Azure Blob Storage Adapter
//!
//! Implements storage adapter for Azure Blob Storage using OpenDAL.

use anyhow::{Context, Result};
use async_trait::async_trait;
use opendal::services::Azblob;
use opendal::Operator;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{debug, error, info, warn};

use crate::vfs::domain::{VirtualFile, StorageSourceType, TierStatus, StorageTier};
use crate::vfs::ports::{
    StorageAdapter, IFileOperations, FileEntry, FileStat, CopyOptions, MoveOptions
};

/// Azure Blob Storage adapter using OpenDAL
pub struct AzureBlobStorageAdapter {
    /// OpenDAL operator
    operator: Operator,
    
    /// Container name
    #[allow(dead_code)]
    container: String,
    
    /// Display name
    name: String,
    
    /// Account name
    #[allow(dead_code)]
    account_name: String,
}

impl AzureBlobStorageAdapter {
    /// Create a new Azure Blob Storage adapter
    pub async fn new(
        container: String,
        account_name: String,
        account_key: Option<String>,
        endpoint: Option<String>,
        name: String,
    ) -> Result<Self> {
        let container_trimmed = container.trim();
        if container_trimmed.is_empty() {
            return Err(anyhow::anyhow!("Container name cannot be empty"));
        }
        
        let mut builder = Azblob::default();
        builder.container(container_trimmed);
        builder.account_name(&account_name);
        
        // Set account key from parameter or environment
        if let Some(key) = account_key {
            builder.account_key(&key);
        } else {
            // Try environment variables
            if let Ok(key) = std::env::var("AZURE_STORAGE_ACCOUNT_KEY") {
                builder.account_key(&key);
            } else if let Ok(key) = std::env::var("AZURE_STORAGE_KEY") {
                builder.account_key(&key);
            } else {
                warn!("No Azure storage account key provided. Connection may fail.");
            }
        }
        
        // Set endpoint if provided (for custom endpoints or Azure Stack)
        if let Some(endpoint_url) = endpoint {
            builder.endpoint(&endpoint_url);
        }
        
        let operator = Operator::new(builder)?
            .finish();
        
        info!("Azure Blob Storage adapter initialized for container: {}", container_trimmed);
        
        Ok(Self {
            operator,
            container: container_trimmed.to_string(),
            name,
            account_name,
        })
    }
    
    /// Get the OpenDAL operator (for multipart uploads)
    pub fn operator(&self) -> &Operator {
        &self.operator
    }
    
    /// Convert path to Azure blob name
    fn to_blob_name(&self, path: &Path) -> String {
        path.strip_prefix("/")
            .unwrap_or(path)
            .to_string_lossy()
            .to_string()
    }
}

#[async_trait]
impl StorageAdapter for AzureBlobStorageAdapter {
    fn storage_type(&self) -> StorageSourceType {
        StorageSourceType::AzureBlob
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn test_connection(&self) -> Result<bool> {
        match self.operator.list("/").await {
            Ok(_) => Ok(true),
            Err(e) => {
                error!("Azure Blob Storage connection test failed: {}", e);
                Ok(false)
            }
        }
    }
    
    async fn list_files(&self, path: &Path) -> Result<Vec<VirtualFile>> {
        let blob_name = self.to_blob_name(path);
        let prefix = if blob_name.is_empty() { String::new() } else { format!("{}/", blob_name) };
        
        info!("[Azure Blob] Listing files - path: {:?}, blob_name: '{}', prefix: '{}'", path, blob_name, prefix);
        
        let entries = self.operator.list(&prefix).await
            .with_context(|| format!("Failed to list Azure blobs with prefix: {}", prefix))?;
        
        info!("[Azure Blob] Received {} entries from OpenDAL", entries.len());
        
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
                debug!("[Azure Blob] Skipping temporary/chunk file: '{}' (entry: '{}')", relative_path, entry_name);
                continue;
            }
            
            // Handle directory-like paths (Azure Blob Storage doesn't have true directories)
            if relative_path.contains('/') {
                let dir_name = relative_path.split('/').next().unwrap();
                let dir_path = if blob_name.is_empty() {
                    format!("/{}", dir_name)
                } else {
                    format!("/{}/{}", blob_name, dir_name)
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
            let file_path = if blob_name.is_empty() {
                PathBuf::from("/").join(&relative_path)
            } else {
                PathBuf::from("/").join(&blob_name).join(&relative_path)
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
        
        info!("[Azure Blob] Returning {} files", files.len());
        Ok(files)
    }
    
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        let blob_name = self.to_blob_name(path);
        info!("[Azure Blob] Reading file: {}", blob_name);
        
        let data = self.operator.read(&blob_name).await
            .with_context(|| format!("Failed to read Azure blob: {}", blob_name))?;
        
        Ok(data)
    }
    
    async fn read_file_range(&self, path: &Path, offset: u64, length: u64) -> Result<Vec<u8>> {
        let blob_name = self.to_blob_name(path);
        info!("[Azure Blob] Reading file range: {} (offset: {}, length: {})", blob_name, offset, length);
        
        let data = self.operator.read_with(&blob_name)
            .range(offset..offset + length)
            .await
            .with_context(|| format!("Failed to read Azure blob range: {}", blob_name))?;
        
        Ok(data)
    }
    
    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<()> {
        let blob_name = self.to_blob_name(path);
        info!("[Azure Blob] Writing file: {} ({} bytes)", blob_name, data.len());
        
        self.operator.write(&blob_name, data.to_vec()).await
            .with_context(|| format!("Failed to write Azure blob: {}", blob_name))?;
        
        Ok(())
    }
    
    async fn get_metadata(&self, path: &Path) -> Result<VirtualFile> {
        let blob_name = self.to_blob_name(path);
        
        let meta = self.operator.stat(&blob_name).await
            .with_context(|| format!("Failed to get Azure blob metadata: {}", blob_name))?;
        
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
        let blob_name = self.to_blob_name(path);
        
        match self.operator.stat(&blob_name).await {
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
        let blob_name = self.to_blob_name(path);
        info!("[Azure Blob] Deleting file: {}", blob_name);
        
        self.operator.delete(&blob_name).await
            .with_context(|| format!("Failed to delete Azure blob: {}", blob_name))?;
        
        Ok(())
    }
    
    async fn create_dir(&self, path: &Path) -> Result<()> {
        // Azure Blob Storage doesn't have true directories
        // Create a placeholder blob to represent a directory
        let blob_name = self.to_blob_name(path);
        let dir_marker = format!("{}/", blob_name);
        
        info!("[Azure Blob] Creating directory marker: {}", dir_marker);
        
        // Create an empty blob with trailing slash to represent directory
        self.operator.write(&dir_marker, vec![]).await
            .with_context(|| format!("Failed to create Azure blob directory marker: {}", dir_marker))?;
        
        Ok(())
    }
    
    async fn file_size(&self, path: &Path) -> Result<u64> {
        let blob_name = self.to_blob_name(path);
        
        let meta = self.operator.stat(&blob_name).await
            .with_context(|| format!("Failed to get Azure blob size: {}", blob_name))?;
        
        Ok(meta.content_length())
    }
}

#[async_trait]
impl IFileOperations for AzureBlobStorageAdapter {
    async fn list(&self, path: &Path) -> Result<Vec<FileEntry>> {
        let blob_name = self.to_blob_name(path);
        let prefix = if blob_name.is_empty() { String::new() } else { format!("{}/", blob_name) };
        
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
        let blob_name = self.to_blob_name(path);
        let metadata = self.operator.stat(&blob_name).await?;
        
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
        // Azure Blob Storage doesn't support append natively
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
        // Azure Blob Storage doesn't support write_at natively
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
        
        // Recursive delete for Azure Blob Storage - optimized with parallel deletion
        let blob_name = self.to_blob_name(path);
        let prefix = format!("{}/", blob_name);
        
        // List all objects with this prefix
        let entries = self.operator.list(&prefix).await.unwrap_or_default();
        
        if !entries.is_empty() {
            info!("rm_rf: Deleting {} Azure blobs under prefix '{}'", entries.len(), prefix);
            
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
        self.operator.delete(&blob_name).await.ok();
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
        let blob_name = self.to_blob_name(path);
        let dir_marker = format!("{}/", blob_name);
        self.operator.delete(&dir_marker).await?;
        Ok(())
    }
    
    async fn symlink(&self, _target: &Path, _link: &Path) -> Result<()> {
        Err(anyhow::anyhow!("Azure Blob Storage does not support symbolic links"))
    }
    
    async fn readlink(&self, _path: &Path) -> Result<String> {
        Err(anyhow::anyhow!("Azure Blob Storage does not support symbolic links"))
    }
    
    async fn exists(&self, path: &Path) -> Result<bool> {
        let blob_name = self.to_blob_name(path);
        Ok(self.operator.is_exist(&blob_name).await?)
    }
    
    async fn is_dir(&self, path: &Path) -> Result<bool> {
        let blob_name = self.to_blob_name(path);
        match self.operator.stat(&blob_name).await {
            Ok(m) => Ok(m.is_dir()),
            Err(_) => Ok(false),
        }
    }
    
    async fn is_file(&self, path: &Path) -> Result<bool> {
        let blob_name = self.to_blob_name(path);
        match self.operator.stat(&blob_name).await {
            Ok(m) => Ok(!m.is_dir()),
            Err(_) => Ok(false),
        }
    }
    
    async fn is_symlink(&self, _path: &Path) -> Result<bool> {
        Ok(false) // Azure Blob Storage doesn't support symlinks
    }
    
    async fn chmod(&self, _path: &Path, _mode: u32) -> Result<()> {
        warn!("chmod not supported on Azure Blob Storage");
        Ok(())
    }
    
    async fn chown(&self, _path: &Path, _uid: u32, _gid: u32) -> Result<()> {
        warn!("chown not supported on Azure Blob Storage");
        Ok(())
    }
    
    async fn touch(&self, path: &Path) -> Result<()> {
        // Create empty file if it doesn't exist
        if !IFileOperations::exists(self, path).await? {
            self.write_file(path, &[]).await?;
        }
        Ok(())
    }
    
    async fn set_times(&self, _path: &Path, _atime: Option<SystemTime>, _mtime: Option<SystemTime>) -> Result<()> {
        warn!("set_times not supported on Azure Blob Storage");
        Ok(())
    }
    
    async fn file_size(&self, path: &Path) -> Result<u64> {
        StorageAdapter::file_size(self, path).await
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
