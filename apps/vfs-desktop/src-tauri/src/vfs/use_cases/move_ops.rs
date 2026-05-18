//! Move Files Use Cases
//!
//! Use cases for moving files:
//! - Within VFS (same source)
//! - Between VFS sources
//! - From VFS to native filesystem
//! - From native filesystem to VFS

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info};

use crate::vfs::ports::IFileOperationsProvider;

// ============================================================================
// Move Within VFS Use Case
// ============================================================================

/// Input DTO for moving files within the same VFS source
#[derive(Debug, Clone)]
pub struct MoveWithinVfsInput {
    /// Source ID
    pub source_id: String,
    /// Source path
    pub from_path: PathBuf,
    /// Destination path
    pub to_path: PathBuf,
}

/// Output DTO for move operation
#[derive(Debug, Clone)]
pub struct MoveWithinVfsOutput {
    /// Final destination path (may differ if renamed due to conflicts)
    pub final_path: PathBuf,
}

/// Use case: Move files within the same VFS source
pub struct MoveWithinVfsUseCase {
    file_ops_provider: Arc<dyn IFileOperationsProvider>,
}

impl MoveWithinVfsUseCase {
    pub fn new(file_ops_provider: Arc<dyn IFileOperationsProvider>) -> Self {
        Self { file_ops_provider }
    }

    /// Execute the move within VFS use case
    pub async fn execute(&self, input: MoveWithinVfsInput) -> Result<MoveWithinVfsOutput> {
        debug!("MoveWithinVfsUseCase: Moving {:?} to {:?} in source {}", 
            input.from_path, input.to_path, input.source_id);
        
        // Validation
        if input.source_id.is_empty() {
            return Err(anyhow::anyhow!("Source ID cannot be empty"));
        }

        if input.from_path == input.to_path {
            return Err(anyhow::anyhow!("Source and destination paths are the same"));
        }

        // Check if source exists
        let exists = self.file_ops_provider.exists(&input.source_id, &input.from_path).await
            .context("Failed to check if source exists")?;
        
        if !exists {
            return Err(anyhow::anyhow!("Source path does not exist: {:?}", input.from_path));
        }

        // Business logic: Move file within VFS
        // Use mv operation which handles both files and directories
        let file_ops = self.file_ops_provider.get_file_ops(&input.source_id).await
            .context("Failed to get file operations")?;
        
        use crate::vfs::ports::MoveOptions;
        let options = MoveOptions { overwrite: true };
        file_ops.mv(&input.from_path, &input.to_path, options).await
            .with_context(|| format!("Failed to move {:?} to {:?}", input.from_path, input.to_path))?;

        info!("MoveWithinVfsUseCase: Successfully moved {:?} to {:?}", 
            input.from_path, input.to_path);
        
        Ok(MoveWithinVfsOutput {
            final_path: input.to_path,
        })
    }
}

// ============================================================================
// Move Between VFS Sources Use Case
// ============================================================================

/// Input DTO for moving files between different VFS sources
#[derive(Debug, Clone)]
pub struct MoveBetweenVfsInput {
    /// Source VFS ID
    pub src_source_id: String,
    /// Source path
    pub from_path: PathBuf,
    /// Destination VFS ID
    pub dest_source_id: String,
    /// Destination path
    pub to_path: PathBuf,
}

/// Output DTO for move between sources
#[derive(Debug, Clone)]
pub struct MoveBetweenVfsOutput {
    /// Final destination path
    pub final_path: PathBuf,
}

/// Use case: Move files between different VFS sources
pub struct MoveBetweenVfsUseCase {
    file_ops_provider: Arc<dyn IFileOperationsProvider>,
}

impl MoveBetweenVfsUseCase {
    pub fn new(file_ops_provider: Arc<dyn IFileOperationsProvider>) -> Self {
        Self { file_ops_provider }
    }

    /// Execute the move between VFS sources use case
    pub async fn execute(&self, input: MoveBetweenVfsInput) -> Result<MoveBetweenVfsOutput> {
        debug!("MoveBetweenVfsUseCase: Moving {:?} from source {} to {:?} in source {}", 
            input.from_path, input.src_source_id, input.to_path, input.dest_source_id);
        
        // Validation
        if input.src_source_id.is_empty() || input.dest_source_id.is_empty() {
            return Err(anyhow::anyhow!("Source IDs cannot be empty"));
        }

        if input.src_source_id == input.dest_source_id {
            return Err(anyhow::anyhow!("Use MoveWithinVfsUseCase for same-source moves"));
        }

        // Check if source exists
        let exists = self.file_ops_provider.exists(&input.src_source_id, &input.from_path).await
            .context("Failed to check if source exists")?;
        
        if !exists {
            return Err(anyhow::anyhow!("Source path does not exist: {:?}", input.from_path));
        }

        // Business logic: Copy then delete (move between sources)
        // First copy to destination
        self.file_ops_provider.copy_to_source(
            &input.src_source_id,
            &input.from_path,
            &input.dest_source_id,
            &input.to_path,
        ).await
        .with_context(|| format!("Failed to copy {:?} to destination", input.from_path))?;

        // Check if it's a directory using stat
        let stat = self.file_ops_provider.stat(&input.src_source_id, &input.from_path).await
            .context("Failed to stat source")?;
        
        if stat.is_dir {
            self.file_ops_provider.rm_rf(&input.src_source_id, &input.from_path).await
                .with_context(|| format!("Failed to delete source directory {:?}", input.from_path))?;
        } else {
            self.file_ops_provider.rm(&input.src_source_id, &input.from_path).await
                .with_context(|| format!("Failed to delete source file {:?}", input.from_path))?;
        }

        info!("MoveBetweenVfsUseCase: Successfully moved {:?} from {} to {:?} in {}", 
            input.from_path, input.src_source_id, input.to_path, input.dest_source_id);
        
        Ok(MoveBetweenVfsOutput {
            final_path: input.to_path,
        })
    }
}

// ============================================================================
// Move from VFS to Native FS Use Case
// ============================================================================

/// Input DTO for moving files from VFS to native filesystem
#[derive(Debug, Clone)]
pub struct MoveVfsToNativeInput {
    /// Source VFS ID
    pub source_id: String,
    /// Source path in VFS
    pub vfs_path: PathBuf,
    /// Destination path on native filesystem (absolute)
    pub native_path: PathBuf,
}

/// Output DTO for move to native
#[derive(Debug, Clone)]
pub struct MoveVfsToNativeOutput {
    /// Final destination path on native filesystem
    pub final_path: PathBuf,
}

/// Use case: Move files from VFS to native filesystem
pub struct MoveVfsToNativeUseCase {
    file_ops_provider: Arc<dyn IFileOperationsProvider>,
}

impl MoveVfsToNativeUseCase {
    pub fn new(file_ops_provider: Arc<dyn IFileOperationsProvider>) -> Self {
        Self { file_ops_provider }
    }

    /// Execute the move from VFS to native use case
    pub async fn execute(&self, input: MoveVfsToNativeInput) -> Result<MoveVfsToNativeOutput> {
        debug!("MoveVfsToNativeUseCase: Moving {:?} from VFS {} to native {:?}", 
            input.vfs_path, input.source_id, input.native_path);
        
        // Validation
        if input.source_id.is_empty() {
            return Err(anyhow::anyhow!("Source ID cannot be empty"));
        }

        if !input.native_path.is_absolute() {
            return Err(anyhow::anyhow!("Native destination path must be absolute"));
        }

        // Check if source exists
        let exists = self.file_ops_provider.exists(&input.source_id, &input.vfs_path).await
            .context("Failed to check if source exists")?;
        
        if !exists {
            return Err(anyhow::anyhow!("Source path does not exist: {:?}", input.vfs_path));
        }

        // Business logic: Copy to native then delete from VFS
        // Read from VFS
        let data = self.file_ops_provider.read(&input.source_id, &input.vfs_path).await
            .context("Failed to read from VFS")?;

        // Write to native filesystem
        if let Some(parent) = input.native_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .context("Failed to create parent directory")?;
        }
        
        tokio::fs::write(&input.native_path, &data).await
            .with_context(|| format!("Failed to write to native path {:?}", input.native_path))?;

        // Delete from VFS
        self.file_ops_provider.rm(&input.source_id, &input.vfs_path).await
            .with_context(|| format!("Failed to delete source from VFS {:?}", input.vfs_path))?;

        info!("MoveVfsToNativeUseCase: Successfully moved {:?} to native {:?}", 
            input.vfs_path, input.native_path);
        
        Ok(MoveVfsToNativeOutput {
            final_path: input.native_path,
        })
    }
}

// ============================================================================
// Move from Native FS to VFS Use Case
// ============================================================================

/// Input DTO for moving files from native filesystem to VFS
#[derive(Debug, Clone)]
pub struct MoveNativeToVfsInput {
    /// Source path on native filesystem (absolute)
    pub native_path: PathBuf,
    /// Destination VFS ID
    pub dest_source_id: String,
    /// Destination path in VFS
    pub vfs_path: PathBuf,
}

/// Output DTO for move from native to VFS
#[derive(Debug, Clone)]
pub struct MoveNativeToVfsOutput {
    /// Final destination path in VFS
    pub final_path: PathBuf,
}

/// Use case: Move files from native filesystem to VFS
pub struct MoveNativeToVfsUseCase {
    file_ops_provider: Arc<dyn IFileOperationsProvider>,
}

impl MoveNativeToVfsUseCase {
    pub fn new(file_ops_provider: Arc<dyn IFileOperationsProvider>) -> Self {
        Self { file_ops_provider }
    }

    /// Execute the move from native to VFS use case
    pub async fn execute(&self, input: MoveNativeToVfsInput) -> Result<MoveNativeToVfsOutput> {
        debug!("MoveNativeToVfsUseCase: Moving native {:?} to VFS {} at {:?}", 
            input.native_path, input.dest_source_id, input.vfs_path);
        
        // Validation
        if input.dest_source_id.is_empty() {
            return Err(anyhow::anyhow!("Destination source ID cannot be empty"));
        }

        if !input.native_path.is_absolute() {
            return Err(anyhow::anyhow!("Native source path must be absolute"));
        }

        if !input.native_path.exists() {
            return Err(anyhow::anyhow!("Source path does not exist: {:?}", input.native_path));
        }

        // Business logic: Copy to VFS then delete from native
        // Read from native filesystem
        let data = tokio::fs::read(&input.native_path).await
            .with_context(|| format!("Failed to read from native path {:?}", input.native_path))?;

        // Write to VFS
        self.file_ops_provider.write(&input.dest_source_id, &input.vfs_path, &data).await
            .with_context(|| format!("Failed to write to VFS {:?}", input.vfs_path))?;

        // Delete from native filesystem
        tokio::fs::remove_file(&input.native_path).await
            .with_context(|| format!("Failed to delete source from native {:?}", input.native_path))?;

        info!("MoveNativeToVfsUseCase: Successfully moved native {:?} to VFS {:?}", 
            input.native_path, input.vfs_path);
        
        Ok(MoveNativeToVfsOutput {
            final_path: input.vfs_path,
        })
    }
}
