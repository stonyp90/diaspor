//! Hydrate File Use Case
//!
//! Use case for hydrating files from cold storage

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

use crate::vfs::application::VfsService;

/// Input DTO for hydrating a file
#[derive(Debug, Clone)]
pub struct HydrateFileInput {
    pub source_id: String,
    pub path: PathBuf,
}

/// Output DTO for hydrating a file
#[derive(Debug, Clone)]
pub struct HydrateFileOutput {
    pub cache_path: PathBuf,
}

/// Use case: Hydrate a file from cold storage
pub struct HydrateFileUseCase {
    vfs_service: Arc<VfsService>,
}

impl HydrateFileUseCase {
    pub fn new(vfs_service: Arc<VfsService>) -> Self {
        Self { vfs_service }
    }
    
    /// Execute the hydrate file use case
    pub async fn execute(&self, input: HydrateFileInput) -> Result<HydrateFileOutput> {
        // Validation
        if input.source_id.is_empty() {
            return Err(anyhow::anyhow!("Source ID cannot be empty"));
        }
        
        // Business logic
        let cache_path = self.vfs_service.hydrate_file(&input.source_id, &input.path)
            .await
            .with_context(|| format!("Failed to hydrate file in source {} at {:?}", input.source_id, input.path))?;
        
        Ok(HydrateFileOutput { cache_path })
    }
}
