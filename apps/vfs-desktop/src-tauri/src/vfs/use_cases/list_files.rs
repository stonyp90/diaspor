//! List Files Use Case
//!
//! Use case for listing files in a storage source

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::debug;

use crate::vfs::domain::VirtualFile;
use crate::vfs::application::VfsService;

/// Input DTO for listing files
#[derive(Debug, Clone)]
pub struct ListFilesInput {
    pub source_id: String,
    pub path: PathBuf,
}

/// Output DTO for listing files
#[derive(Debug, Clone)]
pub struct ListFilesOutput {
    pub files: Vec<VirtualFile>,
}

/// Use case: List files in a storage source
pub struct ListFilesUseCase {
    vfs_service: Arc<VfsService>,
}

impl ListFilesUseCase {
    pub fn new(vfs_service: Arc<VfsService>) -> Self {
        Self { vfs_service }
    }
    
    /// Execute the list files use case
    pub async fn execute(&self, input: ListFilesInput) -> Result<ListFilesOutput> {
        debug!("ListFilesUseCase: Listing files in source {} at {:?}", input.source_id, input.path);
        
        // Validation
        if input.source_id.is_empty() {
            return Err(anyhow::anyhow!("Source ID cannot be empty"));
        }
        
        // Business logic
        let files = self.vfs_service.list_files(&input.source_id, &input.path)
            .await
            .with_context(|| format!("Failed to list files in source {} at {:?}", input.source_id, input.path))?;
        
        Ok(ListFilesOutput { files })
    }
}
