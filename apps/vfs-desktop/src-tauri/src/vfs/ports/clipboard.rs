//! Clipboard Port - Interface for file clipboard operations
//!
//! This module defines the contract for clipboard operations that enable
//! seamless copy/paste between native filesystem and VFS.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

/// Clipboard operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardOperation {
    /// Copy files (preserve source)
    Copy,
    /// Cut files (delete source after paste)
    Cut,
}

/// Source of clipboard content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardSource {
    /// Files from native OS filesystem (Finder, Explorer, etc.)
    Native,
    /// Files from VFS with source ID
    Vfs { source_id: String },
}

/// Clipboard content for file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardContent {
    /// Type of operation (copy or cut)
    pub operation: ClipboardOperation,
    
    /// Source of the files
    pub source: ClipboardSource,
    
    /// List of file/folder paths
    pub paths: Vec<PathBuf>,
    
    /// Timestamp when copied
    pub timestamp: u64,
}

impl ClipboardContent {
    /// Create new clipboard content for copy operation
    pub fn copy(source: ClipboardSource, paths: Vec<PathBuf>) -> Self {
        Self {
            operation: ClipboardOperation::Copy,
            source,
            paths,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    /// Create new clipboard content for cut operation
    pub fn cut(source: ClipboardSource, paths: Vec<PathBuf>) -> Self {
        Self {
            operation: ClipboardOperation::Cut,
            source,
            paths,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
    
    /// Check if clipboard content is from VFS
    pub fn is_vfs(&self) -> bool {
        matches!(self.source, ClipboardSource::Vfs { .. })
    }
    
    /// Check if clipboard content is from native filesystem
    pub fn is_native(&self) -> bool {
        matches!(self.source, ClipboardSource::Native)
    }
    
    /// Check if this is a cut operation
    pub fn is_cut(&self) -> bool {
        self.operation == ClipboardOperation::Cut
    }
}

/// Result of a paste operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasteResult {
    /// Number of files successfully pasted
    pub files_pasted: usize,
    
    /// Number of files that failed
    pub files_failed: usize,
    
    /// Paths of successfully pasted files
    pub pasted_paths: Vec<PathBuf>,
    
    /// Errors encountered
    pub errors: Vec<String>,
    
    /// Operation ID for tracking progress
    pub operation_id: Option<String>,
}

impl PasteResult {
    pub fn success(pasted_paths: Vec<PathBuf>) -> Self {
        Self {
            files_pasted: pasted_paths.len(),
            files_failed: 0,
            pasted_paths,
            errors: Vec::new(),
            operation_id: None,
        }
    }
    
    pub fn partial(pasted_paths: Vec<PathBuf>, errors: Vec<String>) -> Self {
        Self {
            files_pasted: pasted_paths.len(),
            files_failed: errors.len(),
            pasted_paths,
            errors,
            operation_id: None,
        }
    }
}

/// Clipboard service interface for file operations
#[async_trait]
pub trait IClipboardService: Send + Sync {
    /// Copy files to clipboard (from VFS or native)
    async fn copy_files(&self, source: ClipboardSource, paths: Vec<PathBuf>) -> Result<()>;
    
    /// Cut files to clipboard (from VFS or native)
    async fn cut_files(&self, source: ClipboardSource, paths: Vec<PathBuf>) -> Result<()>;
    
    /// Get current clipboard content
    async fn get_clipboard(&self) -> Result<Option<ClipboardContent>>;
    
    /// Clear clipboard
    async fn clear_clipboard(&self) -> Result<()>;
    
    /// Check if clipboard has file content
    async fn has_files(&self) -> Result<bool>;
    
    /// Paste files to a destination
    /// - If destination is VFS, provide source_id
    /// - If destination is native, provide absolute path
    async fn paste_to_vfs(
        &self,
        dest_source_id: &str,
        dest_path: &std::path::Path,
    ) -> Result<PasteResult>;
    
    /// Paste files to native filesystem
    async fn paste_to_native(&self, dest_path: &std::path::Path) -> Result<PasteResult>;
    
    /// Read files from OS clipboard (Finder/Explorer copy)
    async fn read_native_clipboard(&self) -> Result<Option<Vec<PathBuf>>>;
    
    /// Write files to OS clipboard (so Finder/Explorer can paste)
    async fn write_native_clipboard(&self, paths: &[PathBuf]) -> Result<()>;
}

/// Configuration for clipboard adapters
#[derive(Clone)]
pub struct ClipboardAdapterConfig {
    /// Optional VFS service reference for file operations
    /// This is optional because clipboard can work without VFS for native-only operations
    pub vfs_service: Option<Arc<dyn IFileOperationsProvider>>,
}

/// Provider for file operations (used by clipboard adapter)
///
/// This trait allows clipboard adapter to perform file operations without
/// directly depending on VfsService, breaking circular dependencies.
#[async_trait]
pub trait IFileOperationsProvider: Send + Sync {
    /// Get file operations for a source
    async fn get_file_ops(&self, source_id: &str) -> Result<Arc<dyn super::IFileOperations>>;
    
    /// Create directory recursively
    async fn mkdir_p(&self, source_id: &str, path: &std::path::Path) -> Result<()>;
    
    /// Write file contents
    async fn write(&self, source_id: &str, path: &std::path::Path, data: &[u8]) -> Result<()>;
    
    /// Read file contents
    async fn read(&self, source_id: &str, path: &std::path::Path) -> Result<Vec<u8>>;
    
    /// Get file statistics
    async fn stat(&self, source_id: &str, path: &std::path::Path) -> Result<super::FileStat>;
    
    /// List files in directory
    async fn list_files(&self, source_id: &str, path: &std::path::Path) -> Result<Vec<super::FileEntry>>;
    
    /// Copy file or directory
    async fn copy(
        &self,
        source_id: &str,
        from: &std::path::Path,
        to: &std::path::Path,
        options: super::CopyOptions,
    ) -> Result<()>;
    
    /// Copy between sources
    async fn copy_to_source(
        &self,
        src_source_id: &str,
        from: &std::path::Path,
        dest_source_id: &str,
        to: &std::path::Path,
    ) -> Result<()>;
    
    /// Remove file
    async fn rm(&self, source_id: &str, path: &std::path::Path) -> Result<()>;
    
    /// Remove file or directory recursively
    async fn rm_rf(&self, source_id: &str, path: &std::path::Path) -> Result<()>;
    
    /// Check if file exists
    async fn exists(&self, source_id: &str, path: &std::path::Path) -> Result<bool>;
}

/// Factory for creating clipboard adapters
///
/// This factory trait allows the application layer to create clipboard adapters
/// without depending on concrete implementations, following the Dependency
/// Inversion Principle.
#[async_trait]
pub trait ClipboardAdapterFactory: Send + Sync {
    /// Create a clipboard adapter from configuration
    ///
    /// Returns an `Arc<dyn IClipboardService>` to allow sharing across threads.
    async fn create(&self, config: ClipboardAdapterConfig) -> Result<Arc<dyn IClipboardService>>;
}




