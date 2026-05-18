//! Mount Storage Use Case
//!
//! Use case for mounting storage sources

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

use crate::vfs::domain::StorageSource;
use crate::vfs::application::VfsService;

/// Input DTO for mounting local storage
#[derive(Debug, Clone)]
pub struct MountLocalStorageInput {
    pub name: String,
    pub path: PathBuf,
}

/// Output DTO for mounting storage
#[derive(Debug, Clone)]
pub struct MountStorageOutput {
    pub source: StorageSource,
}

/// Use case: Mount a storage source
pub struct MountStorageUseCase {
    vfs_service: Arc<VfsService>,
}

impl MountStorageUseCase {
    pub fn new(vfs_service: Arc<VfsService>) -> Self {
        Self { vfs_service }
    }
    
    /// Execute the mount local storage use case
    pub async fn execute_local(&self, input: MountLocalStorageInput) -> Result<MountStorageOutput> {
        // Validation
        if input.name.trim().is_empty() {
            return Err(anyhow::anyhow!("Storage name cannot be empty"));
        }
        if !input.path.exists() {
            return Err(anyhow::anyhow!("Storage path does not exist: {:?}", input.path));
        }
        if !input.path.is_dir() {
            return Err(anyhow::anyhow!("Storage path must be a directory: {:?}", input.path));
        }
        
        // Business logic
        let source = self.vfs_service.add_local_source(input.name, input.path)
            .await
            .context("Failed to mount local storage")?;
        
        Ok(MountStorageOutput { source })
    }
}
