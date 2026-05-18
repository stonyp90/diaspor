//! Clipboard Use Cases
//!
//! Use cases for copy, cut, and paste operations between VFS and native filesystem.
//! These use cases encapsulate the business logic for clipboard operations.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info};

use crate::vfs::ports::clipboard::{
    ClipboardSource, IClipboardService, PasteResult,
};

// ============================================================================
// Copy Files Use Case
// ============================================================================

/// Input DTO for copying files to clipboard
#[derive(Debug, Clone)]
pub struct CopyFilesInput {
    /// Source of files (VFS or Native)
    pub source: ClipboardSource,
    /// Paths to copy
    pub paths: Vec<PathBuf>,
}

/// Output DTO for copying files
#[derive(Debug, Clone)]
pub struct CopyFilesOutput {
    /// Number of files copied
    pub files_copied: usize,
}

/// Use case: Copy files to clipboard
pub struct CopyFilesUseCase {
    clipboard_service: Arc<dyn IClipboardService>,
}

impl CopyFilesUseCase {
    pub fn new(clipboard_service: Arc<dyn IClipboardService>) -> Self {
        Self { clipboard_service }
    }

    /// Execute the copy files use case
    pub async fn execute(&self, input: CopyFilesInput) -> Result<CopyFilesOutput> {
        debug!("CopyFilesUseCase: Copying {} files from {:?}", input.paths.len(), input.source);
        
        // Validation
        if input.paths.is_empty() {
            return Err(anyhow::anyhow!("No files to copy"));
        }

        // Business logic: Copy files to clipboard
        self.clipboard_service
            .copy_files(input.source.clone(), input.paths.clone())
            .await
            .with_context(|| format!("Failed to copy {} files to clipboard", input.paths.len()))?;

        info!("CopyFilesUseCase: Successfully copied {} files", input.paths.len());
        
        Ok(CopyFilesOutput {
            files_copied: input.paths.len(),
        })
    }
}

// ============================================================================
// Cut Files Use Case
// ============================================================================

/// Input DTO for cutting files to clipboard
#[derive(Debug, Clone)]
pub struct CutFilesInput {
    /// Source of files (VFS or Native)
    pub source: ClipboardSource,
    /// Paths to cut
    pub paths: Vec<PathBuf>,
}

/// Output DTO for cutting files
#[derive(Debug, Clone)]
pub struct CutFilesOutput {
    /// Number of files cut
    pub files_cut: usize,
}

/// Use case: Cut files to clipboard
pub struct CutFilesUseCase {
    clipboard_service: Arc<dyn IClipboardService>,
}

impl CutFilesUseCase {
    pub fn new(clipboard_service: Arc<dyn IClipboardService>) -> Self {
        Self { clipboard_service }
    }

    /// Execute the cut files use case
    pub async fn execute(&self, input: CutFilesInput) -> Result<CutFilesOutput> {
        debug!("CutFilesUseCase: Cutting {} files from {:?}", input.paths.len(), input.source);
        
        // Validation
        if input.paths.is_empty() {
            return Err(anyhow::anyhow!("No files to cut"));
        }

        // Business logic: Cut files to clipboard
        self.clipboard_service
            .cut_files(input.source.clone(), input.paths.clone())
            .await
            .with_context(|| format!("Failed to cut {} files to clipboard", input.paths.len()))?;

        info!("CutFilesUseCase: Successfully cut {} files", input.paths.len());
        
        Ok(CutFilesOutput {
            files_cut: input.paths.len(),
        })
    }
}

// ============================================================================
// Paste to VFS Use Case
// ============================================================================

/// Input DTO for pasting files to VFS
#[derive(Debug, Clone)]
pub struct PasteToVfsInput {
    /// Destination source ID
    pub dest_source_id: String,
    /// Destination path
    pub dest_path: PathBuf,
}

/// Output DTO for pasting files
#[derive(Debug, Clone)]
pub struct PasteToVfsOutput {
    /// Result of paste operation
    pub result: PasteResult,
}

/// Use case: Paste files to VFS
pub struct PasteToVfsUseCase {
    clipboard_service: Arc<dyn IClipboardService>,
}

impl PasteToVfsUseCase {
    pub fn new(clipboard_service: Arc<dyn IClipboardService>) -> Self {
        Self { clipboard_service }
    }

    /// Execute the paste to VFS use case
    pub async fn execute(&self, input: PasteToVfsInput) -> Result<PasteToVfsOutput> {
        debug!("PasteToVfsUseCase: Pasting to VFS source {} at {:?}", 
            input.dest_source_id, input.dest_path);
        
        // Validation
        if input.dest_source_id.is_empty() {
            return Err(anyhow::anyhow!("Destination source ID cannot be empty"));
        }

        // Check if clipboard has files
        let has_files = self.clipboard_service.has_files().await
            .context("Failed to check clipboard")?;
        
        if !has_files {
            return Err(anyhow::anyhow!("Clipboard is empty"));
        }

        // Business logic: Paste files to VFS
        let result = self.clipboard_service
            .paste_to_vfs(&input.dest_source_id, &input.dest_path)
            .await
            .with_context(|| format!("Failed to paste files to VFS source {} at {:?}", 
                input.dest_source_id, input.dest_path))?;

        info!("PasteToVfsUseCase: Pasted {} files, {} failed", 
            result.files_pasted, result.files_failed);
        
        Ok(PasteToVfsOutput { result })
    }
}

// ============================================================================
// Paste to Native FS Use Case
// ============================================================================

/// Input DTO for pasting files to native filesystem
#[derive(Debug, Clone)]
pub struct PasteToNativeInput {
    /// Destination path (absolute path on native filesystem)
    pub dest_path: PathBuf,
}

/// Output DTO for pasting files to native
#[derive(Debug, Clone)]
pub struct PasteToNativeOutput {
    /// Result of paste operation
    pub result: PasteResult,
}

/// Use case: Paste files to native filesystem
pub struct PasteToNativeUseCase {
    clipboard_service: Arc<dyn IClipboardService>,
}

impl PasteToNativeUseCase {
    pub fn new(clipboard_service: Arc<dyn IClipboardService>) -> Self {
        Self { clipboard_service }
    }

    /// Execute the paste to native use case
    pub async fn execute(&self, input: PasteToNativeInput) -> Result<PasteToNativeOutput> {
        debug!("PasteToNativeUseCase: Pasting to native FS at {:?}", input.dest_path);
        
        // Validation
        if !input.dest_path.is_absolute() {
            return Err(anyhow::anyhow!("Destination path must be absolute"));
        }

        // Check if clipboard has files
        let has_files = self.clipboard_service.has_files().await
            .context("Failed to check clipboard")?;
        
        if !has_files {
            return Err(anyhow::anyhow!("Clipboard is empty"));
        }

        // Business logic: Paste files to native filesystem
        let result = self.clipboard_service
            .paste_to_native(&input.dest_path)
            .await
            .with_context(|| format!("Failed to paste files to native FS at {:?}", input.dest_path))?;

        info!("PasteToNativeUseCase: Pasted {} files, {} failed", 
            result.files_pasted, result.files_failed);
        
        Ok(PasteToNativeOutput { result })
    }
}
