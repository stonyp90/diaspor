//! Storage Port - Interface for storage adapters

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use crate::vfs::domain::{VirtualFile, StorageSourceType};

/// Storage adapter trait - Port for all storage backends
///
/// This trait defines the contract that all storage adapters must implement.
/// Following the Ports & Adapters pattern, the application core depends on
/// this trait, not on concrete implementations.
#[async_trait]
pub trait StorageAdapter: Send + Sync {
    /// Get the storage type
    fn storage_type(&self) -> StorageSourceType;
    
    /// Get adapter name for display
    fn name(&self) -> &str;
    
    /// Test connection to the storage backend
    async fn test_connection(&self) -> Result<bool>;
    
    /// List files in a directory
    async fn list_files(&self, path: &Path) -> Result<Vec<VirtualFile>>;
    




    /// List files in a directory with pagination support
    /// Returns (files, continuation_token) where continuation_token is Some(token) if there are more items
    async fn list_files_paginated(
        &self,
        path: &Path,
        limit: Option<u64>,
        continuation_token: Option<&str>,
    ) -> Result<(Vec<VirtualFile>, Option<String>)> {
        // Default implementation: call list_files and apply limit/token manually
        // Object storage adapters should override this for efficient pagination
        let mut files = self.list_files(path).await?;
        
        // Apply continuation token (simple offset-based for default impl)
        let start_idx = if let Some(token) = continuation_token {
            token.parse::<usize>().unwrap_or(0)
        } else {
            0
        };
        
        // Apply limit
        let end_idx = if let Some(limit_val) = limit {
            std::cmp::min(start_idx + limit_val as usize, files.len())
        } else {
            files.len()
        };
        
        let result_files = if start_idx < files.len() {
            files.drain(start_idx..end_idx).collect()
        } else {
            Vec::new()
        };
        
        let next_token = if end_idx < files.len() {
            Some(end_idx.to_string())
        } else {
            None
        };
        
        Ok((result_files, next_token))
    }
    
    /// Read file contents
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>>;
    
    /// Read file contents with range (for partial reads)
    async fn read_file_range(&self, path: &Path, offset: u64, length: u64) -> Result<Vec<u8>>;
    
    /// Write file contents
    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<()>;
    
    /// Get file metadata
    async fn get_metadata(&self, path: &Path) -> Result<VirtualFile>;
    
    /// Check if file exists
    async fn exists(&self, path: &Path) -> Result<bool>;
    
    /// Delete file
    async fn delete(&self, path: &Path) -> Result<()>;
    
    /// Create directory
    async fn create_dir(&self, path: &Path) -> Result<()>;
    
    /// Get file size without downloading
    async fn file_size(&self, path: &Path) -> Result<u64>;
}

/// Factory for creating storage adapters
///
/// This factory trait allows the application layer to create storage adapters
/// without depending on concrete implementations, following the Dependency
/// Inversion Principle.
#[async_trait]
pub trait StorageAdapterFactory: Send + Sync {
    /// Create a storage adapter from configuration
    ///
    /// Returns an `Arc<dyn StorageAdapter>` to allow sharing across threads
    /// and avoiding lifetime issues.
    async fn create_adapter(&self, config: StorageAdapterConfig) -> Result<Arc<dyn StorageAdapter>>;
}

/// Configuration for storage adapters
#[derive(Debug, Clone)]
pub struct StorageAdapterConfig {
    pub adapter_type: StorageSourceType,
    pub path_or_bucket: String,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

impl Default for StorageAdapterConfig {
    fn default() -> Self {
        Self {
            adapter_type: StorageSourceType::Local,
            path_or_bucket: String::new(),
            region: None,
            endpoint: None,
            access_key: None,
            secret_key: None,
        }
    }
}



