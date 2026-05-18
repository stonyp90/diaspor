//! Mounted Storage Port - Interface for filesystem-like storage
//!
//! This trait defines the contract for mounted storage providers that behave
//! like traditional filesystems (local FS, NAS, NFS, SMB, etc.).
//!
//! All mounted storage can be treated the same way:
//! - Direct file access (no hydration needed)
//! - POSIX-like operations
//! - Directory traversal
//! - Symlink support
//! - File permissions

use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::SystemTime;


/// Mounted Storage trait - Base abstraction for filesystem-like storage
///
/// Implemented by:
/// - LocalStorageAdapter
/// - NasStorageAdapter (NFS, SMB, SFTP, WebDAV)
/// - FsxOntapAdapter (when accessed via NFS/SMB)
/// - Any mounted cloud storage (iCloud Drive, Google Drive, Dropbox, etc.)
#[async_trait]
pub trait MountedStorage: Send + Sync {
    /// Get the mount point path
    fn mount_point(&self) -> &Path;
    
    /// Check if storage is currently mounted/accessible
    async fn is_mounted(&self) -> Result<bool>;
    
    /// Mount the storage (if applicable)
    async fn mount(&self) -> Result<()>;
    
    /// Unmount the storage (if applicable)
    async fn unmount(&self) -> Result<()>;
    
    /// Get filesystem statistics (total space, free space, etc.)
    async fn filesystem_stats(&self) -> Result<FilesystemStats>;
    
    /// Check if path is accessible (permissions check)
    async fn is_accessible(&self, path: &Path) -> Result<bool>;
    
    /// Get file permissions (Unix mode bits)
    async fn get_permissions(&self, path: &Path) -> Result<u32>;
    
    /// Set file permissions (Unix mode bits)
    async fn set_permissions(&self, path: &Path, mode: u32) -> Result<()>;
    
    /// Get file owner (user ID)
    async fn get_owner(&self, path: &Path) -> Result<u32>;
    
    /// Set file owner (user ID)
    async fn set_owner(&self, path: &Path, uid: u32) -> Result<()>;
    
    /// Get file group (group ID)
    async fn get_group(&self, path: &Path) -> Result<u32>;
    
    /// Set file group (group ID)
    async fn set_group(&self, path: &Path, gid: u32) -> Result<()>;
    
    /// Create symbolic link
    async fn create_symlink(&self, target: &Path, link_path: &Path) -> Result<()>;
    
    /// Read symbolic link target
    async fn read_symlink(&self, link_path: &Path) -> Result<PathBuf>;
    
    /// Check if path is a symbolic link
    async fn is_symlink(&self, path: &Path) -> Result<bool>;
    
    /// Get extended attributes (xattrs) - platform specific
    async fn get_xattr(&self, path: &Path, name: &str) -> Result<Option<Vec<u8>>>;
    
    /// Set extended attributes (xattrs) - platform specific
    async fn set_xattr(&self, path: &Path, name: &str, value: &[u8]) -> Result<()>;
    
    /// List extended attributes
    async fn list_xattrs(&self, path: &Path) -> Result<Vec<String>>;
    
    /// Get file change time (ctime) - when metadata last changed
    async fn get_change_time(&self, path: &Path) -> Result<Option<SystemTime>>;
    
    /// Get file access time (atime) - when file was last accessed
    async fn get_access_time(&self, path: &Path) -> Result<Option<SystemTime>>;
    
    /// Set file access time (atime)
    async fn set_access_time(&self, path: &Path, atime: SystemTime) -> Result<()>;
    
    /// Set file modification time (mtime)
    async fn set_modification_time(&self, path: &Path, mtime: SystemTime) -> Result<()>;
}

/// Filesystem statistics
#[derive(Debug, Clone)]
pub struct FilesystemStats {
    /// Total space in bytes
    pub total_space: u64,
    
    /// Available space in bytes
    pub available_space: u64,
    
    /// Used space in bytes
    pub used_space: u64,
    
    /// Filesystem type (e.g., "ext4", "apfs", "ntfs")
    pub filesystem_type: Option<String>,
    
    /// Block size in bytes
    pub block_size: u64,
    
    /// Total number of inodes (if applicable)
    pub total_inodes: Option<u64>,
    
    /// Available inodes (if applicable)
    pub available_inodes: Option<u64>,
}
