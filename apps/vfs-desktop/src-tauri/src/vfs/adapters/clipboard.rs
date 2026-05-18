//! Clipboard Adapter - Platform-specific clipboard implementation
//!
//! Handles copy/paste between native filesystem and VFS across all platforms:
//! - macOS: NSPasteboard with file URLs
//! - Windows: Clipboard with CF_HDROP
//! - Linux: X11/Wayland with text/uri-list

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};
use futures::future;

use crate::vfs::ports::clipboard::{
    ClipboardContent, ClipboardSource, IClipboardService, PasteResult,
    IFileOperationsProvider,
};
use crate::vfs::ports::CopyOptions;
use crate::vfs::operation_tracking::OperationTrackingHelper;

/// Clipboard adapter for cross-platform file operations
#[derive(Clone)]
pub struct ClipboardAdapter {
    /// Internal clipboard storage (for VFS-to-VFS operations)
    internal_clipboard: Arc<RwLock<Option<ClipboardContent>>>,
    
    /// Reference to file operations provider (port, not concrete service)
    /// This breaks the circular dependency with VfsService
    file_ops_provider: Option<Arc<dyn IFileOperationsProvider>>,
}

impl ClipboardAdapter {
    /// Create a new clipboard adapter
    pub fn new() -> Self {
        Self {
            internal_clipboard: Arc::new(RwLock::new(None)),
            file_ops_provider: None,
        }
    }
    
    /// Create with file operations provider (port interface)
    pub fn with_file_ops_provider(file_ops_provider: Arc<dyn IFileOperationsProvider>) -> Self {
        Self {
            internal_clipboard: Arc::new(RwLock::new(None)),
            file_ops_provider: Some(file_ops_provider),
        }
    }
    
    /// Set file operations provider after creation
    pub fn set_file_ops_provider(&mut self, file_ops_provider: Arc<dyn IFileOperationsProvider>) {
        self.file_ops_provider = Some(file_ops_provider);
    }
    
    /// Get file name from path
    fn file_name(path: &Path) -> String {
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string())
    }
    
    /// Generate a unique name for a file/folder when copying to same location
    /// Follows macOS/Finder pattern: "file.txt" -> "file copy.txt" -> "file copy 2.txt"
    async fn generate_unique_name(
        dest_dir: &Path,
        base_name: &str,
        provider: &Arc<dyn IFileOperationsProvider>,
        source_id: &str,
    ) -> Result<PathBuf> {
        // Parse base name into name and extension
        let (name_part, ext_part) = if let Some(dot_idx) = base_name.rfind('.') {
            let name = &base_name[..dot_idx];
            let ext = &base_name[dot_idx..];
            (name.to_string(), ext.to_string())
        } else {
            (base_name.to_string(), String::new())
        };
        
        // Try base name first
        let mut candidate = dest_dir.join(base_name);
        let mut counter = 0;
        
        // Check if file/directory exists
        while provider.exists(source_id, &candidate).await.unwrap_or(false) {
            counter += 1;
            let new_name = if counter == 1 {
                format!("{} copy{}", name_part, ext_part)
            } else {
                format!("{} copy {}{}", name_part, counter, ext_part)
            };
            candidate = dest_dir.join(&new_name);
            
            // Safety: prevent infinite loop
            if counter > 1000 {
                return Err(anyhow::anyhow!("Too many duplicate names, cannot generate unique name"));
            }
        }
        
        Ok(candidate)
    }
    
    /// Copy a single file or directory from native to VFS (recursive)
    async fn copy_native_to_vfs(
        &self,
        source_path: &Path,
        dest_source_id: &str,
        dest_path: &Path,
    ) -> Result<PathBuf> {
        let provider = self.file_ops_provider.as_ref()
            .context("File operations provider not initialized")?;
        
        let file_name = Self::file_name(source_path);
        let mut dest_file_path = dest_path.join(&file_name);
        
        // Check if source and destination are the same (self-copy prevention)
        // For local storage, when copying native to VFS, the VFS path resolves to the same native path
        let source_canonical = source_path.canonicalize().unwrap_or_else(|_| source_path.to_path_buf());
        
        // Try to detect self-copy by comparing source's parent with destination directory
        // For local storage, if dest_path is absolute, it might be a native path we can compare directly
        if let Some(source_parent) = source_path.parent() {
            let source_parent_canonical = source_parent.canonicalize().unwrap_or_else(|_| source_parent.to_path_buf());
            
            // Check if dest_path is absolute (might be a native path)
            let dest_path_canonical = if dest_path.is_absolute() {
                dest_path.canonicalize().unwrap_or_else(|_| dest_path.to_path_buf())
            } else {
                // VFS relative path - try to construct native path for comparison
                // For local storage, VFS paths like "/Icaros_v3.3.3 (3)" map to native paths
                // We can't easily resolve this without knowing base_path, so use a heuristic:
                // If dest_path (when joined with file_name) would equal source_path, it's self-copy
                dest_path.to_path_buf()
            };
            
            // Check if source's parent equals destination directory
            if source_parent_canonical == dest_path_canonical {
                // Same directory - check if destination file exists (would be self-copy)
                if let Ok(exists) = provider.exists(dest_source_id, &dest_file_path).await {
                    if exists {
                        // Destination exists in same directory - likely self-copy
                        // Generate unique name to avoid "cannot copy to itself" error
                        warn!("Self-copy detected: {:?} -> {:?}, generating unique name", source_path, dest_file_path);
                        dest_file_path = Self::generate_unique_name(dest_path, &file_name, provider, dest_source_id).await?;
                    }
                }
            }
            
            // Additional check: try to construct destination native path and compare directly
            // For local storage, if dest_path is absolute and equals source_parent, dest_file_path equals source_path
            if dest_path.is_absolute() {
                if let Ok(dest_file_canonical) = dest_file_path.canonicalize() {
                    if source_canonical == dest_file_canonical {
                        // Direct path match - definitely self-copy
                        warn!("Direct self-copy detected: {:?} == {:?}, generating unique name", source_canonical, dest_file_canonical);
                        dest_file_path = Self::generate_unique_name(dest_path, &file_name, provider, dest_source_id).await?;
                    }
                }
            }
        }
        
        let metadata = tokio::fs::metadata(source_path).await
            .with_context(|| format!("Failed to get metadata for {:?}", source_path))?;
        
        // For directories, also check if destination would be inside source (infinite recursion prevention)
        // Note: canonicalize() only works for native paths, not VFS paths (like S3)
        // So we skip this check for VFS destinations and rely on path string comparison instead
        if metadata.is_dir() {
            // For native-to-VFS copies, we can't use canonicalize on VFS paths
            // Instead, compare path strings directly - if dest_path starts with source_path, it's inside
            let source_str = source_canonical.to_string_lossy().to_string();
            let dest_str = dest_file_path.to_string_lossy().to_string();
            
            // Normalize both paths for comparison (remove leading/trailing slashes)
            let normalize = |s: &str| -> String {
                s.trim_start_matches('/').trim_end_matches('/').to_string()
            };
            let source_normalized = normalize(&source_str);
            let dest_normalized = normalize(&dest_str);
            
            // Check if destination is inside source (would cause infinite recursion)
            // This works for both native and VFS paths
            if !source_normalized.is_empty() && 
               (dest_normalized.starts_with(&format!("{}/", source_normalized)) || 
                dest_normalized == source_normalized) {
                warn!("Directory self-copy detected (destination inside source), generating unique name");
                dest_file_path = Self::generate_unique_name(dest_path, &file_name, provider, dest_source_id).await?;
            }
        }
        
        if metadata.is_dir() {
            // Create directory in VFS
            provider.mkdir_p(dest_source_id, &dest_file_path).await
                .with_context(|| format!("Failed to create directory in VFS: {:?}", dest_file_path))?;
            
            // Copy contents recursively
            let mut entries = tokio::fs::read_dir(source_path).await
                .with_context(|| format!("Failed to read directory: {:?}", source_path))?;
            
            let mut errors = Vec::new();
            while let Some(entry) = entries.next_entry().await? {
                let entry_path = entry.path();
                // Recursively copy each entry (boxed for async recursion)
                match Box::pin(self.copy_native_to_vfs(&entry_path, dest_source_id, &dest_file_path)).await {
                    Ok(_) => {
                        debug!("Successfully copied {:?} to {:?}", entry_path, dest_file_path);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to copy {:?}: {}", entry_path, e);
                        error!("{}", error_msg);
                        errors.push(error_msg);
                        // Continue with other files instead of failing completely
                        // This allows partial success for directory copies
                    }
                }
            }
            
            // If all files failed, return an error
            if !errors.is_empty() && errors.len() == 1 {
                // Single error - return it directly
                return Err(anyhow::anyhow!("{}", errors[0]));
            } else if !errors.is_empty() {
                // Multiple errors - return summary
                return Err(anyhow::anyhow!(
                    "Failed to copy {} files in directory: {}",
                    errors.len(),
                    errors.join("; ")
                ));
            }
        } else {
            // Copy file
            let data = tokio::fs::read(source_path).await
                .with_context(|| format!("Failed to read native file: {:?}", source_path))?;
            
            provider.write(dest_source_id, &dest_file_path, &data).await
                .with_context(|| format!("Failed to write to VFS: {:?}", dest_file_path))?;
        }
        
        debug!("Copied native {:?} to VFS {:?}", source_path, dest_file_path);
        Ok(dest_file_path)
    }
    
    /// Copy a single file or directory from VFS to native (recursive)
    async fn copy_vfs_to_native(
        &self,
        source_id: &str,
        source_path: &Path,
        dest_path: &Path,
    ) -> Result<PathBuf> {
        let provider = self.file_ops_provider.as_ref()
            .context("File operations provider not initialized")?;
        
        let file_name = Self::file_name(source_path);
        let dest_file_path = dest_path.join(&file_name);
        
        // Use stat to check if it's a directory (more reliable than listing)
        let is_dir = match provider.stat(source_id, source_path).await {
            Ok(stat) => stat.is_dir,
            Err(_) => {
                // If stat fails, try listing as fallback
                provider.list_files(source_id, source_path).await.is_ok()
            }
        };
        
        if is_dir {
            // Create directory in native filesystem
            tokio::fs::create_dir_all(&dest_file_path).await
                .with_context(|| format!("Failed to create directory: {:?}", dest_file_path))?;
            
            // List directory contents and copy recursively
            match provider.list_files(source_id, source_path).await {
                Ok(entries) => {
                    for entry in entries {
                        let entry_path = PathBuf::from(&entry.path);
                        // Recursively copy each entry (boxed for async recursion)
                        Box::pin(self.copy_vfs_to_native(source_id, &entry_path, &dest_file_path)).await?;
                    }
                }
                Err(e) => {
                    warn!("Failed to list directory {:?} for recursive copy: {}", source_path, e);
                    // If listing fails, we still created the directory, so return success
                }
            }
        } else {
            // Copy file
            let data = provider.read(source_id, source_path).await
                .with_context(|| format!("Failed to read from VFS: {:?}", source_path))?;
            
            // Ensure parent directory exists
            if let Some(parent) = dest_file_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            
            tokio::fs::write(&dest_file_path, &data).await
                .with_context(|| format!("Failed to write to native: {:?}", dest_file_path))?;
        }
        
        debug!("Copied VFS {:?} to native {:?}", source_path, dest_file_path);
        Ok(dest_file_path)
    }
    
    /// Copy within VFS (same or different sources) - recursive for directories
    pub async fn copy_vfs_to_vfs(
        &self,
        src_source_id: &str,
        source_path: &Path,
        dest_source_id: &str,
        dest_path: &Path,
    ) -> Result<PathBuf> {
        let provider = self.file_ops_provider.as_ref()
            .context("File operations provider not initialized")?;
        
        let file_name = Self::file_name(source_path);
        
        // Ensure file_name is not empty
        if file_name.is_empty() {
            return Err(anyhow::anyhow!("Cannot copy: source path has no file name"));
        }
        
        // Construct destination path - handle root path correctly
        // When dest_path is "/", join("/", "file.txt") creates "/file.txt" correctly
        let mut dest_file_path = dest_path.join(&file_name);
        
        // Normalize the path to ensure it starts with "/" for VFS paths
        let dest_path_str = dest_file_path.to_string_lossy().to_string();
        if !dest_path_str.starts_with('/') {
            dest_file_path = PathBuf::from("/").join(&dest_file_path);
        }
        
        // Ensure destination path is valid (not empty or just "/")
        let final_dest_str = dest_file_path.to_string_lossy();
        if final_dest_str.trim() == "/" || final_dest_str.trim().is_empty() {
            return Err(anyhow::anyhow!("Cannot copy to root path - destination path would be invalid. File name: '{}'", file_name));
        }
        
        // Check if source and destination are the same (self-copy prevention)
        if src_source_id == dest_source_id {
            // Normalize paths for comparison (remove leading slashes, handle empty)
            let normalize_for_comparison = |p: &Path| -> String {
                p.to_string_lossy()
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .to_string()
            };
            
            let source_normalized = normalize_for_comparison(source_path);
            let dest_normalized = normalize_for_comparison(&dest_file_path);
            
            info!("[copy_vfs_to_vfs] Comparing paths - source: {:?} ({}) vs dest: {:?} ({})", 
                  source_path, source_normalized, dest_file_path, dest_normalized);
            
            // Check if paths are exactly the same
            if source_normalized == dest_normalized {
                // Same path - generate unique name to avoid self-copy
                info!("[copy_vfs_to_vfs] Self-copy detected (same path): {:?} -> {:?}, generating unique name", source_path, dest_file_path);
                dest_file_path = Self::generate_unique_name(dest_path, &file_name, provider, dest_source_id).await?;
                info!("[copy_vfs_to_vfs] Generated unique name: {:?}", dest_file_path);
            } else {
                // Check if source is a directory and destination would be inside it (infinite recursion)
                let is_source_dir = match provider.stat(src_source_id, source_path).await {
                    Ok(stat) => stat.is_dir,
                    Err(_) => false, // If we can't determine, assume it's not a directory to be safe
                };
                
                if is_source_dir {
                    // For directories, check if destination is inside source
                    let source_with_slash = format!("{}/", source_normalized);
                    if dest_normalized.starts_with(&source_with_slash) || dest_normalized == source_normalized {
                        // Destination is inside source - would cause infinite recursion
                        return Err(anyhow::anyhow!(
                            "Cannot copy folder \"{}\" into itself. The destination \"{}\" is inside the source folder \"{}\".",
                            file_name,
                            dest_file_path.display(),
                            source_path.display()
                        ));
                    }
                }
                
                // Also check if destination directory is the same as source (for files)
                if !is_source_dir {
                    let dest_dir_normalized = normalize_for_comparison(dest_path);
                    if source_normalized == dest_dir_normalized {
                        // Same directory - check if destination file exists (would be self-copy)
                        if provider.exists(dest_source_id, &dest_file_path).await.unwrap_or(false) {
                            warn!("Self-copy detected (destination exists): {:?} -> {:?}, generating unique name", source_path, dest_file_path);
                            dest_file_path = Self::generate_unique_name(dest_path, &file_name, provider, dest_source_id).await?;
                        }
                    }
                }
            }
        }
        
        // Use stat to check if it's a directory (more reliable than listing)
        let is_dir = match provider.stat(src_source_id, source_path).await {
            Ok(stat) => stat.is_dir,
            Err(_) => {
                // If stat fails, try listing as fallback
                provider.list_files(src_source_id, source_path).await.is_ok()
            }
        };
        
        if is_dir {
            // For directories, use copy_to_source which handles recursive copying properly
            // This works for both same-source and cross-source copies
            if src_source_id == dest_source_id {
                // Same source - use internal copy with recursive option
                // Set overwrite to true to allow pasting folders even if destination exists
                // This matches typical paste behavior where existing folders are overwritten/merged
                let opts = CopyOptions {
                    recursive: true,
                    overwrite: true, // Allow overwriting existing folders when pasting
                    ..Default::default()
                };
                info!("[copy_vfs_to_vfs] Calling provider.copy for directory: source={:?} to dest={:?}", source_path, dest_file_path);
                provider.copy(src_source_id, source_path, &dest_file_path, opts).await?;
            } else {
                // Different sources - use copy_to_source which handles recursive directory copying
                provider.copy_to_source(src_source_id, source_path, dest_source_id, &dest_file_path).await?;
            }
        } else {
            // Copy file
            if src_source_id == dest_source_id {
                // Same source - use internal copy
                // Set overwrite to true for paste operations
                let opts = CopyOptions {
                    recursive: false,
                    overwrite: true, // Allow overwriting existing files when pasting
                    ..Default::default()
                };
                provider.copy(src_source_id, source_path, &dest_file_path, opts).await?;
            } else {
                // Different sources - use copy_to_source
                provider.copy_to_source(src_source_id, source_path, dest_source_id, &dest_file_path).await?;
            }
        }
        
        debug!("Copied VFS {:?} to VFS {:?}", source_path, dest_file_path);
        Ok(dest_file_path)
    }
    
    /// Copy within native filesystem
    async fn copy_native_to_native(
        &self,
        source_path: &Path,
        dest_path: &Path,
    ) -> Result<PathBuf> {
        let file_name = Self::file_name(source_path);
        let dest_file_path = dest_path.join(&file_name);
        
        // Check if source and destination are the same (self-copy prevention)
        let source_canonical = source_path.canonicalize()
            .unwrap_or_else(|_| source_path.to_path_buf());
        let dest_path_canonical = dest_path.canonicalize()
            .unwrap_or_else(|_| dest_path.to_path_buf());
        
        // Check if trying to copy a directory into itself
        let metadata = tokio::fs::metadata(source_path).await
            .with_context(|| format!("Failed to get metadata for {:?}", source_path))?;
        
        if metadata.is_dir() {
            // For directories, check if destination would be inside source (infinite recursion)
            if let Ok(dest_file_canonical) = dest_file_path.canonicalize() {
                let source_str = source_canonical.to_string_lossy();
                let dest_str = dest_file_canonical.to_string_lossy();
                
                // Check if destination is inside source (would cause infinite recursion)
                if dest_str.starts_with(&format!("{}/", source_str)) || dest_str == source_str {
                    return Err(anyhow::anyhow!(
                        "Cannot copy folder \"{}\" into itself. The destination \"{}\" is inside the source folder.",
                        file_name,
                        dest_file_path.display()
                    ));
                }
            }
            
            // Also check if destination directory is the same as source
            if source_canonical == dest_path_canonical {
                return Err(anyhow::anyhow!(
                    "Cannot copy folder \"{}\" to the same location. Please choose a different destination.",
                    file_name
                ));
            }
            
            // Recursively copy directory
            tokio::fs::create_dir_all(&dest_file_path).await
                .with_context(|| format!("Failed to create destination directory: {:?}", dest_file_path))?;
            
            let mut entries = tokio::fs::read_dir(source_path).await
                .with_context(|| format!("Failed to read source directory: {:?}", source_path))?;
            
            while let Some(entry) = entries.next_entry().await? {
                let entry_path = entry.path();
                // Recursively copy each entry
                Box::pin(self.copy_native_to_native(&entry_path, &dest_file_path)).await?;
            }
        } else {
            // For files, check if destination equals source
            if let Ok(dest_file_canonical) = dest_file_path.canonicalize() {
                if source_canonical == dest_file_canonical {
                    return Err(anyhow::anyhow!(
                        "Cannot copy file \"{}\" to itself. The source and destination are the same.",
                        file_name
                    ));
                }
            }
            
            // Ensure parent directory exists
            if let Some(parent) = dest_file_path.parent() {
                tokio::fs::create_dir_all(parent).await
                    .with_context(|| format!("Failed to create parent directory: {:?}", parent))?;
            }
            
            // Copy file
            tokio::fs::copy(source_path, &dest_file_path).await
                .with_context(|| format!("Failed to copy {:?} to {:?}", source_path, dest_file_path))?;
        }
        
        debug!("Copied native {:?} to native {:?}", source_path, dest_file_path);
        Ok(dest_file_path)
    }
    
    /// Export VFS files to a temp directory for native clipboard access
    /// This allows Finder/Explorer to paste VFS files
    async fn export_vfs_to_temp(
        &self,
        source_id: &str,
        paths: &[PathBuf],
    ) -> Result<Vec<PathBuf>> {
        let provider = match &self.file_ops_provider {
            Some(p) => p,
            None => {
                warn!("File operations provider not initialized, cannot export to clipboard");
                return Ok(Vec::new());
            }
        };
        
        // Create temp directory for exported files
        let temp_dir = std::env::temp_dir().join("ursly-clipboard");
        tokio::fs::create_dir_all(&temp_dir).await?;
        
        let mut exported_paths = Vec::new();
        
        for path in paths {
            let file_name = Self::file_name(path);
            let temp_path = temp_dir.join(&file_name);
            
            // Check if it's a directory using stat (more reliable)
            let is_dir = match provider.stat(source_id, path).await {
                Ok(stat) => stat.is_dir,
                Err(_) => {
                    // Fallback: try listing
                    provider.list_files(source_id, path).await.is_ok()
                }
            };
            
            if is_dir {
                // Recursively copy directory to temp
                let temp_dir_clone = temp_dir.clone();
                match Box::pin(self.copy_vfs_to_native(source_id, path, &temp_dir_clone)).await {
                    Ok(dest_path) => {
                        let dest_path_clone = dest_path.clone();
                        exported_paths.push(dest_path);
                        debug!("Exported VFS directory {:?} to temp {:?}", path, dest_path_clone);
                    }
                    Err(e) => {
                        warn!("Failed to export VFS directory {:?} to temp: {}", path, e);
                    }
                }
            } else {
                // Copy file
                let temp_path_clone = temp_path.clone();
                match provider.read(source_id, path).await {
                    Ok(data) => {
                        // Ensure parent directory exists
                        if let Some(parent) = temp_path_clone.parent() {
                            tokio::fs::create_dir_all(parent).await?;
                        }
                        let temp_path_for_write = temp_path_clone.clone();
                        if let Err(e) = tokio::fs::write(&temp_path_for_write, &data).await {
                            warn!("Failed to export {:?} to temp: {}", path, e);
                            continue;
                        }
                        let temp_path_for_debug = temp_path_clone.clone();
                        exported_paths.push(temp_path_clone);
                        debug!("Exported VFS file {:?} to temp {:?}", path, temp_path_for_debug);
                    }
                    Err(e) => {
                        warn!("Failed to read VFS file {:?}: {}", path, e);
                    }
                }
            }
        }
        
        info!("Exported {} VFS items to temp for clipboard", exported_paths.len());
        Ok(exported_paths)
    }
}

impl Default for ClipboardAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IClipboardService for ClipboardAdapter {
    async fn copy_files(&self, source: ClipboardSource, paths: Vec<PathBuf>) -> Result<()> {
        info!("Copying {} files to clipboard", paths.len());
        debug!("copy_files: source={:?}, paths={:?}", source, paths);
        
        let content = ClipboardContent::copy(source.clone(), paths.clone());
        
        // Store internally FIRST - this ensures the operation succeeds even if native clipboard fails
        // Use timeout to prevent hanging on lock acquisition
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            self.internal_clipboard.write()
        ).await {
            Ok(mut clipboard) => {
                *clipboard = Some(content.clone());
                info!("Internal clipboard set successfully with {} paths", content.paths.len());
                // Drop the lock immediately
                drop(clipboard);
            }
            Err(_) => {
                error!("copy_files: Failed to acquire write lock within 2 seconds - possible deadlock");
                return Err(anyhow::anyhow!("Failed to acquire clipboard lock - operation may be in progress"));
            }
        }
        
        // Write to OS clipboard (non-blocking - don't fail if this hangs)
        // Use tokio::spawn to run in background so it doesn't block the response
        let source_clone = source.clone();
        let paths_clone = paths.clone();
        let adapter_clone = self.clone();
        
        tokio::spawn(async move {
            match &source_clone {
                ClipboardSource::Native => {
                    // Native files - write directly
                    if let Err(e) = adapter_clone.write_native_clipboard(&paths_clone).await {
                        warn!("Failed to write to native clipboard (non-critical): {}", e);
                    } else {
                        info!("Successfully wrote to native clipboard");
                    }
                }
                ClipboardSource::Vfs { source_id } => {
                    // VFS files - export to temp and write those paths
                    // Use timeout to prevent hanging
                    match tokio::time::timeout(
                        tokio::time::Duration::from_secs(5),
                        adapter_clone.export_vfs_to_temp(source_id, &paths_clone)
                    ).await {
                        Ok(Ok(temp_paths)) => {
                            if !temp_paths.is_empty() {
                                if let Err(e) = adapter_clone.write_native_clipboard(&temp_paths).await {
                                    warn!("Failed to write VFS files to native clipboard (non-critical): {}", e);
                                } else {
                                    info!("Successfully wrote {} VFS files to native clipboard", temp_paths.len());
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            warn!("Failed to export VFS files to temp (non-critical): {}", e);
                        }
                        Err(_) => {
                            warn!("Export VFS files to temp timed out after 5 seconds (non-critical)");
                        }
                    }
                }
            }
        });
        
        // Return success immediately - internal clipboard is set
        Ok(())
    }
    
    async fn cut_files(&self, source: ClipboardSource, paths: Vec<PathBuf>) -> Result<()> {
        info!("Cutting {} files to clipboard", paths.len());
        debug!("cut_files: source={:?}, paths={:?}", source, paths);
        
        let content = ClipboardContent::cut(source.clone(), paths.clone());
        
        // Store internally FIRST - this ensures the operation succeeds even if native clipboard fails
        // Use timeout to prevent hanging on lock acquisition
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            self.internal_clipboard.write()
        ).await {
            Ok(mut clipboard) => {
                *clipboard = Some(content.clone());
                info!("Internal clipboard set successfully (cut) with {} paths", content.paths.len());
                // Drop the lock immediately
                drop(clipboard);
            }
            Err(_) => {
                error!("cut_files: Failed to acquire write lock within 2 seconds - possible deadlock");
                return Err(anyhow::anyhow!("Failed to acquire clipboard lock - operation may be in progress"));
            }
        }
        
        // Write to OS clipboard (non-blocking - don't fail if this hangs)
        // Use tokio::spawn to run in background so it doesn't block the response
        let source_clone = source.clone();
        let paths_clone = paths.clone();
        let adapter_clone = self.clone();
        
        tokio::spawn(async move {
            match &source_clone {
                ClipboardSource::Native => {
                    // Native files - write directly
                    if let Err(e) = adapter_clone.write_native_clipboard(&paths_clone).await {
                        warn!("Failed to write to native clipboard (non-critical): {}", e);
                    } else {
                        info!("Successfully wrote to native clipboard (cut)");
                    }
                }
                ClipboardSource::Vfs { source_id } => {
                    // VFS files - export to temp and write those paths
                    // Use timeout to prevent hanging
                    match tokio::time::timeout(
                        tokio::time::Duration::from_secs(5),
                        adapter_clone.export_vfs_to_temp(source_id, &paths_clone)
                    ).await {
                        Ok(Ok(temp_paths)) => {
                            if !temp_paths.is_empty() {
                                if let Err(e) = adapter_clone.write_native_clipboard(&temp_paths).await {
                                    warn!("Failed to write VFS files to native clipboard (non-critical): {}", e);
                                } else {
                                    info!("Successfully wrote {} VFS files to native clipboard (cut)", temp_paths.len());
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            warn!("Failed to export VFS files to temp (non-critical): {}", e);
                        }
                        Err(_) => {
                            warn!("Export VFS files to temp timed out after 5 seconds (non-critical)");
                        }
                    }
                }
            }
        });
        
        // Return success immediately - internal clipboard is set
        Ok(())
    }
    
    async fn get_clipboard(&self) -> Result<Option<ClipboardContent>> {
        // #region agent log
        use std::fs::OpenOptions;
        use std::io::Write;
        let _ = OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log").map(|mut f| {
            let _ = writeln!(f, r#"{{"location":"clipboard.rs:469","message":"get_clipboard ENTRY","data":{{}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"H1"}}"#, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            
        });
        // #endregion
        // First check internal clipboard
        let internal = self.internal_clipboard.read().await;
        if let Some(ref content) = *internal {
            debug!("get_clipboard: Found internal clipboard content: source={:?}, paths={:?}", content.source, content.paths);
            // #region agent log
            let _ = OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log").map(|mut f| {
                let _ = writeln!(f, r#"{{"location":"clipboard.rs:473","message":"get_clipboard INTERNAL FOUND","data":{{"paths_count":{},"source":"{:?}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"H1"}}"#, content.paths.len(), content.source, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                
            });
            // #endregion
            return Ok(Some(content.clone()));
        }
        
        // Fall back to OS clipboard
        debug!("get_clipboard: Internal clipboard empty, checking native clipboard");
        // #region agent log
        let _ = OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log").map(|mut f| {
            let _ = writeln!(f, r#"{{"location":"clipboard.rs:479","message":"get_clipboard INTERNAL EMPTY","data":{{}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"H1"}}"#, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            
        });
        // #endregion
        if let Some(paths) = self.read_native_clipboard().await? {
            if !paths.is_empty() {
                debug!("get_clipboard: Found {} paths in native clipboard", paths.len());
                // Filter out paths that don't exist (temp files might have been cleaned up)
                let existing_paths: Vec<PathBuf> = paths.into_iter()
                    .filter(|p| p.exists())
                    .collect();
                
                if !existing_paths.is_empty() {
                    debug!("get_clipboard: Returning {} existing paths from native clipboard", existing_paths.len());
                    // #region agent log
                    let _ = OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log").map(|mut f| {
                        let _ = writeln!(f, r#"{{"location":"clipboard.rs:488","message":"get_clipboard NATIVE FOUND","data":{{"paths_count":{}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"H1"}}"#, existing_paths.len(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                        
                    });
                    // #endregion
                    // Cache the native clipboard content internally to avoid re-reading
                    // This prevents macOS permission dialogs on subsequent reads (e.g., when using keyboard shortcuts)
                    let content = ClipboardContent::copy(ClipboardSource::Native, existing_paths.clone());
                    {
                        let mut internal = self.internal_clipboard.write().await;
                        *internal = Some(content.clone());
                    }
                    return Ok(Some(content));
                } else {
                    debug!("get_clipboard: All native clipboard paths no longer exist");
                    // #region agent log
                    let _ = OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log").map(|mut f| {
                        let _ = writeln!(f, r#"{{"location":"clipboard.rs:499","message":"get_clipboard NATIVE PATHS MISSING","data":{{}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"H1"}}"#, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                        
                    });
                    // #endregion
                }
            }
        }
        
        debug!("get_clipboard: No clipboard content found");
        Ok(None)
    }
    
    async fn clear_clipboard(&self) -> Result<()> {
        let mut clipboard = self.internal_clipboard.write().await;
        *clipboard = None;
        Ok(())
    }
    
    async fn has_files(&self) -> Result<bool> {
        // First check internal clipboard
        {
            let clipboard = self.internal_clipboard.read().await;
            if clipboard.as_ref().map(|c| !c.paths.is_empty()).unwrap_or(false) {
                return Ok(true);
            }
        }
        
        // Also check native OS clipboard for files copied from Finder/Explorer
        // This allows paste to work directly after copying files externally
        if let Ok(Some(paths)) = self.read_native_clipboard().await {
            if !paths.is_empty() {
                // Verify at least one path exists
                let has_existing = paths.iter().any(|p| p.exists());
                if has_existing {
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
    
    async fn paste_to_vfs(
        &self,
        dest_source_id: &str,
        dest_path: &Path,
    ) -> Result<PasteResult> {
        // Validate inputs
        if dest_source_id.trim().is_empty() {
            return Err(anyhow::anyhow!("Destination source ID cannot be empty"));
        }
        
        let content = self.get_clipboard().await?
            .context("Clipboard is empty")?;
        
        if content.paths.is_empty() {
            return Err(anyhow::anyhow!("Clipboard contains no files"));
        }
        
        info!("Pasting {} files to VFS {} at {:?}", content.paths.len(), dest_source_id, dest_path);
        
        // IMPORTANT: Create operation tracking BEFORE validation so operation_id is always available
        // This ensures frontend can track operations even if they fail early
        // Track paste operation with file metadata
        // IMPORTANT: Get operation type BEFORE clipboard might be cleared
        let is_cut_operation = content.is_cut();
        let operation_type = if is_cut_operation {
            crate::vfs::operation_tracker::OperationType::Move // Cut operations show as Move
        } else {
            crate::vfs::operation_tracker::OperationType::Copy // Copy operations show as Copy
        };
        
        let source_id = match &content.source {
            ClipboardSource::Native => "native".to_string(),
            ClipboardSource::Vfs { source_id } => source_id.clone(),
        };
        
        // Collect file metadata for tracking
        let mut files_with_metadata = Vec::new();
        for path in &content.paths {
            match &content.source {
                ClipboardSource::Native => {
                    // Get size from native filesystem
                    if let Ok(metadata) = tokio::fs::metadata(path).await {
                        files_with_metadata.push((path.to_string_lossy().to_string(), metadata.len()));
                    }
                }
                ClipboardSource::Vfs { source_id } => {
                    // Get size from VFS file operations
                    if let Some(provider) = &self.file_ops_provider {
                        if let Ok(file_ops) = provider.get_file_ops(source_id).await {
                            if let Ok(stat) = file_ops.stat(path).await {
                                files_with_metadata.push((path.to_string_lossy().to_string(), stat.size));
                            }
                        }
                    }
                }
            }
        }
        
        // Start tracking paste operation with correct operation type
        // Copy operations show as "Copy", Cut operations show as "Move"
        // Create operation BEFORE validation so operation_id is always available for frontend tracking
        let operation_id = OperationTrackingHelper::track_multi_file_operation_start(
            operation_type,
            source_id.clone(),
            content.paths.first().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            Some(dest_path.to_string_lossy().to_string()),
            files_with_metadata.clone(), // Clone before moving
        );
        
        // Immediately transition operation to InProgress so it's visible in OperationsPanel
        // This ensures the operation modal appears right away, even before files start copying
        OperationTrackingHelper::update_progress(&operation_id, 0)
            .unwrap_or_else(|e| warn!("Failed to update paste operation progress: {}", e));
        
        // Pre-validation: Check for conflicts and validate paths before starting
        // This provides early feedback and prevents wasted work
        // IMPORTANT: If validation fails, we still have operation_id to return to frontend
        if let Some(provider) = &self.file_ops_provider {
            // Validate destination path exists and is a directory
            if !provider.exists(dest_source_id, dest_path).await.unwrap_or(false) {
                let error_msg = format!("Destination path does not exist: {}", dest_path.display());
                OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                    .unwrap_or_else(|e| error!("Failed to fail paste operation: {}", e));
                return Err(anyhow::anyhow!("{}|OPERATION_ID:{}", error_msg, operation_id));
            }
            
            // Check if destination is a directory (for paste operations)
            if let Ok(stat) = provider.stat(dest_source_id, dest_path).await {
                if !stat.is_dir {
                    let error_msg = format!("Destination path is not a directory: {}", dest_path.display());
                    OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                        .unwrap_or_else(|e| error!("Failed to fail paste operation: {}", e));
                    return Err(anyhow::anyhow!("{}|OPERATION_ID:{}", error_msg, operation_id));
                }
            }
            
            let mut conflicts = Vec::new();
            for path in &content.paths {
                let file_name = Self::file_name(path);
                let dest_file_path = dest_path.join(&file_name);
                
                // Check if destination already exists
                if let Ok(exists) = provider.exists(dest_source_id, &dest_file_path).await {
                    if exists {
                        conflicts.push(file_name);
                    }
                }
            }
            
            // Log conflicts but don't fail - we'll overwrite by default (can be enhanced with user dialog later)
            if !conflicts.is_empty() {
                info!("Found {} existing files at destination - will overwrite", conflicts.len());
            }
        }
        
        // Optimize: Copy files in parallel for better performance
        // Use a semaphore to limit concurrent operations (prevent overwhelming the system)
        let semaphore = Arc::new(Semaphore::new(8)); // Max 8 concurrent copies
        
        let mut pasted_paths = Vec::new();
        let mut errors = Vec::new();
        
        // Create futures for all copy operations
        let operation_id_clone = operation_id.clone();
        let copy_futures: Vec<_> = content.paths.iter().map(|path| {
            let semaphore = semaphore.clone();
            let path = path.clone();
            let dest_source_id = dest_source_id.to_string();
            let dest_path = dest_path.to_path_buf();
            let adapter = self.clone();
            let source = content.source.clone();
            let operation_id = operation_id_clone.clone();
            
            async move {
                // Acquire semaphore permit (limits concurrent operations)
                let _permit = semaphore.acquire().await
                    .map_err(|e| anyhow::anyhow!("Failed to acquire semaphore: {}", e))?;
                
                let result = match &source {
                    ClipboardSource::Native => {
                        adapter.copy_native_to_vfs(&path, &dest_source_id, &dest_path).await
                    }
                    ClipboardSource::Vfs { source_id } => {
                        adapter.copy_vfs_to_vfs(source_id, &path, &dest_source_id, &dest_path).await
                    }
                };
                
                // Update progress after each file
                if let Ok(_dest) = &result {
                    // Find file size for progress update
                    let file_size = match &source {
                        ClipboardSource::Native => {
                            tokio::fs::metadata(&path).await.ok().map(|m| m.len()).unwrap_or(0)
                        }
                        ClipboardSource::Vfs { source_id } => {
                            if let Some(provider) = &adapter.file_ops_provider {
                                if let Ok(file_ops) = provider.get_file_ops(source_id).await {
                                    file_ops.stat(&path).await.ok().map(|s| s.size).unwrap_or(0)
                                } else {
                                    0
                                }
                            } else {
                                0
                            }
                        }
                    };
                    
                    // Update progress incrementally using update_file_progress for multi-file operations
                    // This properly tracks individual file progress within the operation
                    if file_size > 0 {
                        use crate::vfs::operation_tracker::OperationStatus;
                        use crate::vfs::commands::get_operation_tracker;
                        let tracker = get_operation_tracker();
                        let _ = tracker.update_file_progress(
                            &operation_id,
                            &path.to_string_lossy(),
                            file_size,
                            Some(OperationStatus::Completed),
                        );
                    }
                }
                
                result
            }
        }).collect();
        
        // Execute all copy operations in parallel
        let results = future::join_all(copy_futures).await;
        
        // Process results
        for (path, result) in content.paths.iter().zip(results) {
            match result {
                Ok(dest) => {
                    pasted_paths.push(dest.clone());
                    debug!("Successfully pasted {:?} to {:?}", path, dest);
                }
                Err(e) => {
                    // Create user-friendly error message
                    let file_name = path.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string());
                    
                    // Construct the attempted destination path
                    let attempted_dest = dest_path.join(&file_name).to_string_lossy().to_string();
                    
                    let error_msg = if e.to_string().contains("Cannot copy") || e.to_string().contains("into itself") {
                        // Use the error message as-is if it's already user-friendly
                        format!("\"{}\": {}", file_name, e)
                    } else if e.to_string().contains("Permission denied") || e.to_string().contains("403") || e.to_string().contains("AccessDenied") {
                        // Permission errors - include IAM guidance
                        format!("\"{}\": Permission denied. Check IAM permissions: s3:GetObject (read), s3:PutObject (write), s3:DeleteObject (delete). Error: {}", file_name, e)
                    } else if e.to_string().contains("cannot be empty") || e.to_string().contains("invalid") || e.to_string().contains("Destination path cannot be empty") {
                        // Path validation errors - provide helpful message
                        format!("\"{}\": Invalid destination path. Error: {}", file_name, e)
                    } else {
                        // Generic error - show attempted destination
                        format!("\"{}\": Failed to copy \"{}\" to \"{}\". Error: {}", 
                            file_name,
                            path.display(),
                            attempted_dest,
                            e
                        )
                    };
                    
                    error!("Failed to paste {:?} to {:?}: {}", path, attempted_dest, e);
                    errors.push(error_msg);
                }
            }
        }
        
        // Progress is updated incrementally during parallel copy operations above
        // No need to recalculate here - each file updates progress as it completes
        
        // Update operation status
        if errors.is_empty() {
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete paste operation: {}", e));
        } else {
            OperationTrackingHelper::fail_operation(&operation_id, format!("{} files failed", errors.len()))
                .unwrap_or_else(|e| error!("Failed to fail paste operation: {}", e));
        }
        
        // If cut operation, delete sources after successful paste
        // Use parallel deletion for better performance
        if content.is_cut() && errors.is_empty() {
            info!("Cut operation: deleting {} source files in parallel", content.paths.len());
            
            // Use semaphore to limit concurrent deletions (prevent overwhelming the system)
            let delete_semaphore = Arc::new(Semaphore::new(16)); // Higher parallelism for deletes (they're fast)
            
            match &content.source {
                ClipboardSource::Native => {
                    let delete_futures: Vec<_> = content.paths.iter().map(|path| {
                        let path = path.clone();
                        let sem = delete_semaphore.clone();
                        async move {
                            let _permit = sem.acquire().await.ok();
                            // Check if it's a directory
                            let is_dir = tokio::fs::metadata(&path).await
                                .map(|m| m.is_dir())
                                .unwrap_or(false);
                            
                            if is_dir {
                                if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                                    warn!("Failed to delete cut native directory {:?}: {}", path, e);
                                } else {
                                    debug!("Deleted cut native directory {:?}", path);
                                }
                            } else if let Err(e) = tokio::fs::remove_file(&path).await {
                                warn!("Failed to delete cut native file {:?}: {}", path, e);
                            } else {
                                debug!("Deleted cut native file {:?}", path);
                            }
                        }
                    }).collect();
                    
                    future::join_all(delete_futures).await;
                    info!("Cut operation: finished deleting {} native source files", content.paths.len());
                }
                ClipboardSource::Vfs { source_id } => {
                    if let Some(provider) = &self.file_ops_provider {
                        let delete_futures: Vec<_> = content.paths.iter().map(|path| {
                            let path = path.clone();
                            let source_id = source_id.clone();
                            let provider = provider.clone();
                            let sem = delete_semaphore.clone();
                            async move {
                                let _permit = sem.acquire().await.ok();
                                // Use rm_rf which handles both files and directories
                                // This avoids the extra stat call for each file
                                if let Err(e) = provider.rm_rf(&source_id, &path).await {
                                    warn!("Failed to delete cut VFS file/directory {:?}: {}", path, e);
                                } else {
                                    debug!("Deleted cut VFS file/directory {:?}", path);
                                }
                            }
                        }).collect();
                        
                        future::join_all(delete_futures).await;
                        info!("Cut operation: finished deleting {} VFS source files", content.paths.len());
                    }
                }
            }
            
            // Clear clipboard after cut
            self.clear_clipboard().await?;
        }
        
        // Return PasteResult with operation_id for frontend to track progress
        if errors.is_empty() {
            Ok(PasteResult {
                files_pasted: pasted_paths.len(),
                files_failed: 0,
                pasted_paths,
                errors: Vec::new(),
                operation_id: Some(operation_id),
            })
        } else {
            Ok(PasteResult {
                files_pasted: pasted_paths.len(),
                files_failed: errors.len(),
                pasted_paths,
                errors,
                operation_id: Some(operation_id),
            })
        }
    }
    
    async fn paste_to_native(&self, dest_path: &Path) -> Result<PasteResult> {
        let content = self.get_clipboard().await?
            .context("Clipboard is empty")?;
        
        info!("Pasting {} files to native {:?}", content.paths.len(), dest_path);
        
        let mut pasted_paths = Vec::new();
        let mut errors = Vec::new();
        
        for path in &content.paths {
            let result = match &content.source {
                ClipboardSource::Native => {
                    self.copy_native_to_native(path, dest_path).await
                }
                ClipboardSource::Vfs { source_id } => {
                    self.copy_vfs_to_native(source_id, path, dest_path).await
                }
            };
            
            match result {
                Ok(dest) => pasted_paths.push(dest),
                Err(e) => errors.push(format!("{:?}: {}", path, e)),
            }
        }
        
        // If cut, delete sources
        if content.is_cut() && errors.is_empty() {
            match &content.source {
                ClipboardSource::Native => {
                    for path in &content.paths {
                        if let Err(e) = tokio::fs::remove_file(path).await {
                            warn!("Failed to delete cut source {:?}: {}", path, e);
                        }
                    }
                }
                ClipboardSource::Vfs { source_id } => {
                    if let Some(provider) = &self.file_ops_provider {
                        for path in &content.paths {
                            // Check if it's a directory
                            let is_dir = match provider.stat(source_id, path).await {
                                Ok(stat) => stat.is_dir,
                                Err(_) => false,
                            };
                            
                            if is_dir {
                                // Use rm_rf for recursive directory deletion
                                if let Err(e) = provider.rm_rf(source_id, path).await {
                                    warn!("Failed to delete cut VFS directory {:?}: {}", path, e);
                                } else {
                                    info!("Successfully deleted cut VFS directory {:?}", path);
                                }
                            } else if let Err(e) = provider.rm(source_id, path).await {
                                warn!("Failed to delete cut VFS file {:?}: {}", path, e);
                            } else {
                                info!("Successfully deleted cut VFS file {:?}", path);
                            }
                        }
                    }
                }
            }
            
            self.clear_clipboard().await?;
        }
        
        // Write to OS clipboard so user can paste in Finder/Explorer
        if !pasted_paths.is_empty() {
            self.write_native_clipboard(&pasted_paths).await?;
        }
        
        if errors.is_empty() {
            Ok(PasteResult::success(pasted_paths))
        } else {
            Ok(PasteResult::partial(pasted_paths, errors))
        }
    }
    
    /// Read files from OS clipboard (Finder/Explorer copy)
    async fn read_native_clipboard(&self) -> Result<Option<Vec<PathBuf>>> {
        #[cfg(target_os = "macos")]
        {
            read_macos_clipboard().await
        }
        
        #[cfg(target_os = "windows")]
        {
            read_windows_clipboard().await
        }
        
        #[cfg(target_os = "linux")]
        {
            read_linux_clipboard().await
        }
        
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            Ok(None)
        }
    }
    
    /// Write files to OS clipboard (so Finder/Explorer can paste)
    async fn write_native_clipboard(&self, paths: &[PathBuf]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        
        #[cfg(target_os = "macos")]
        {
            write_macos_clipboard(paths).await
        }
        
        #[cfg(target_os = "windows")]
        {
            write_windows_clipboard(paths).await
        }
        
        #[cfg(target_os = "linux")]
        {
            write_linux_clipboard(paths).await
        }
        
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            warn!("Native clipboard not supported on this platform");
            Ok(())
        }
    }
}

// =============================================================================
// macOS Clipboard Implementation
// =============================================================================

#[cfg(target_os = "macos")]
async fn read_macos_clipboard() -> Result<Option<Vec<PathBuf>>> {
    use std::process::Command;
    
    // Use osascript to read file paths from clipboard
    // This handles both single and multiple file selections from Finder
    let script = r#"
use framework "AppKit"
use scripting additions

set thePaths to {}
try
    set thePasteboard to current application's NSPasteboard's generalPasteboard()
    set theURLs to thePasteboard's readObjectsForClasses:{current application's NSURL} options:(missing value)
    
    if theURLs is not missing value then
        repeat with theURL in theURLs
            if (theURL's isFileURL()) as boolean then
                set end of thePaths to (theURL's |path|()) as text
            end if
        end repeat
    end if
end try

set AppleScript's text item delimiters to linefeed
return thePaths as text
"#;
    
    // Use spawn_blocking to avoid blocking the async runtime
    // Command::new().output() is a blocking operation
    let script_owned = script.to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new("osascript")
            .arg("-e")
            .arg(&script_owned)
            .output()
    }).await;
    
    match output {
        Ok(Ok(out)) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let paths: Vec<PathBuf> = text
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| PathBuf::from(l.trim()))
                .filter(|p| p.exists())
                .collect();
            
            if paths.is_empty() {
                Ok(None)
            } else {
                debug!("Read {} paths from macOS clipboard", paths.len());
                Ok(Some(paths))
            }
        }
        Ok(Ok(out)) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.is_empty() {
                debug!("macOS clipboard read stderr: {}", stderr);
            }
            Ok(None)
        }
        Ok(Err(e)) => {
            warn!("Failed to read macOS clipboard: {}", e);
            Ok(None)
        }
        Err(e) => {
            warn!("Clipboard read task panicked: {}", e);
            Ok(None)
        }
    }
}

#[cfg(target_os = "macos")]
async fn write_macos_clipboard(paths: &[PathBuf]) -> Result<()> {
    use std::process::Command;
    
    if paths.is_empty() {
        return Ok(());
    }
    
    // Build the list of POSIX files for AppleScript
    let file_list: Vec<String> = paths
        .iter()
        .filter(|p| p.exists())
        .map(|p| format!(r#"(POSIX file "{}")"#, p.display()))
        .collect();
    
    if file_list.is_empty() {
        warn!("No existing files to write to clipboard");
        return Ok(());
    }
    
    // Use AppleScript with NSPasteboard for proper Finder integration
    let script = format!(
        r#"
use framework "AppKit"
use scripting additions

set thePasteboard to current application's NSPasteboard's generalPasteboard()
thePasteboard's clearContents()

set theURLs to current application's NSMutableArray's new()
{}

thePasteboard's writeObjects:theURLs
return "ok"
"#,
        paths
            .iter()
            .filter(|p| p.exists())
            .map(|p| format!(
                r#"theURLs's addObject:(current application's NSURL's fileURLWithPath:"{}")"#,
                p.display()
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
    
    // Use spawn_blocking to avoid blocking the async runtime
    let output = tokio::task::spawn_blocking(move || {
        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
    }).await;
    
    match output {
        Ok(Ok(out)) if out.status.success() => {
            debug!("Wrote {} paths to macOS clipboard", paths.len());
            Ok(())
        }
        Ok(Ok(out)) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!("macOS clipboard write failed: {}", stderr);
            Err(anyhow::anyhow!("Failed to write to clipboard: {}", stderr))
        }
        Ok(Err(e)) => {
            Err(anyhow::anyhow!("Failed to write to macOS clipboard: {}", e))
        }
        Err(e) => {
            Err(anyhow::anyhow!("Clipboard write task panicked: {}", e))
        }
    }
}

// =============================================================================
// Windows Clipboard Implementation
// =============================================================================

#[cfg(target_os = "windows")]
async fn read_windows_clipboard() -> Result<Option<Vec<PathBuf>>> {
    use std::process::{Command, Stdio};
    use std::os::windows::process::CommandExt;
    
    // Use PowerShell to read file paths (hidden window)
    // Run in blocking task to ensure CREATE_NO_WINDOW flag works
    let output = tokio::task::spawn_blocking(move || {
        let mut cmd = Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle", "Hidden",
            "-Command",
            r#"
            Add-Type -AssemblyName System.Windows.Forms
            $files = [System.Windows.Forms.Clipboard]::GetFileDropList()
            $files | ForEach-Object { Write-Output $_ }
            "#,
        ]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        // Set CREATE_NO_WINDOW flag to prevent window creation
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        cmd.output()
    })
    .await
    .context("Failed to spawn blocking task for clipboard read")?;
    
    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let paths: Vec<PathBuf> = text
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| PathBuf::from(l.trim()))
                .filter(|p| p.exists())
                .collect();
            
            if paths.is_empty() {
                Ok(None)
            } else {
                Ok(Some(paths))
            }
        }
        _ => Ok(None),
    }
}

#[cfg(target_os = "windows")]
async fn write_windows_clipboard(paths: &[PathBuf]) -> Result<()> {
    
    let paths_str: Vec<String> = paths
        .iter()
        .map(|p| format!("'{}'", p.display()))
        .collect();
    
    let script = format!(
        r#"
        Add-Type -AssemblyName System.Windows.Forms
        $files = New-Object System.Collections.Specialized.StringCollection
        {}
        [System.Windows.Forms.Clipboard]::SetFileDropList($files)
        "#,
        paths_str
            .iter()
            .map(|p| format!("$files.Add({})", p))
            .collect::<Vec<_>>()
            .join("\n")
    );
    
    // Run in blocking task to ensure CREATE_NO_WINDOW flag works
    tokio::task::spawn_blocking(move || {
        use std::process::{Command, Stdio};
        use std::os::windows::process::CommandExt;
        
        let mut cmd = Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle", "Hidden",
            "-Command", &script
        ]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        // Set CREATE_NO_WINDOW flag to prevent window creation
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
        
        cmd.output()
    })
    .await
    .context("Failed to spawn blocking task for clipboard write")?
    .context("Failed to write to Windows clipboard")?;
    
    debug!("Wrote {} paths to Windows clipboard", paths.len());
    Ok(())
}

// =============================================================================
// Linux Clipboard Implementation
// =============================================================================

#[cfg(target_os = "linux")]
async fn read_linux_clipboard() -> Result<Option<Vec<PathBuf>>> {
    use std::process::Command;
    
    // Use spawn_blocking to avoid blocking the async runtime
    let output = tokio::task::spawn_blocking(move || {
        // Try xclip first, then xsel
        Command::new("xclip")
            .args(["-selection", "clipboard", "-o", "-t", "text/uri-list"])
            .output()
            .or_else(|_| {
                Command::new("xsel")
                    .args(["--clipboard", "--output"])
                    .output()
            })
    }).await;
    
    match output {
        Ok(Ok(out)) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let paths: Vec<PathBuf> = text
                .lines()
                .filter(|l| l.starts_with("file://"))
                .map(|l| {
                    let path = l.strip_prefix("file://").unwrap_or(l);
                    // URL decode
                    let decoded = urlencoding::decode(path).unwrap_or_else(|_| path.into());
                    PathBuf::from(decoded.as_ref())
                })
                .filter(|p| p.exists())
                .collect();
            
            if paths.is_empty() {
                Ok(None)
            } else {
                Ok(Some(paths))
            }
        }
        _ => Ok(None),
    }
}

#[cfg(target_os = "linux")]
async fn write_linux_clipboard(paths: &[PathBuf]) -> Result<()> {
    use std::process::{Command, Stdio};
    use std::io::Write;
    
    // Convert to file:// URIs
    let uris: String = paths
        .iter()
        .map(|p| format!("file://{}", urlencoding::encode(&p.to_string_lossy())))
        .collect::<Vec<_>>()
        .join("\n");
    
    // Use spawn_blocking to avoid blocking the async runtime
    let result = tokio::task::spawn_blocking(move || -> Result<()> {
        // Try xclip first
        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "text/uri-list"])
            .stdin(Stdio::piped())
            .spawn()
            .or_else(|_| {
                Command::new("xsel")
                    .args(["--clipboard", "--input"])
                    .stdin(Stdio::piped())
                    .spawn()
            })
            .context("Failed to open clipboard (install xclip or xsel)")?;
        
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(uris.as_bytes())?;
        }
        
        child.wait()?;
        Ok(())
    }).await;
    
    match result {
        Ok(Ok(())) => {
            debug!("Wrote {} paths to Linux clipboard", paths.len());
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(e) => Err(anyhow::anyhow!("Clipboard write task panicked: {}", e)),
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::ports::ClipboardOperation;
    
    
    #[tokio::test]
    async fn test_clipboard_copy_and_get() {
        let clipboard = ClipboardAdapter::new();
        
        let paths = vec![
            PathBuf::from("/test/file1.txt"),
            PathBuf::from("/test/file2.txt"),
        ];
        
        clipboard.copy_files(ClipboardSource::Native, paths.clone()).await.unwrap();
        
        let content = clipboard.get_clipboard().await.unwrap().unwrap();
        assert_eq!(content.paths, paths);
        assert_eq!(content.operation, ClipboardOperation::Copy);
    }
    
    #[tokio::test]
    async fn test_clipboard_cut() {
        let clipboard = ClipboardAdapter::new();
        
        let paths = vec![PathBuf::from("/test/file.txt")];
        
        clipboard.cut_files(
            ClipboardSource::Vfs { source_id: "local".to_string() },
            paths.clone(),
        ).await.unwrap();
        
        let content = clipboard.get_clipboard().await.unwrap().unwrap();
        assert!(content.is_cut());
        assert!(content.is_vfs());
    }
    
    #[tokio::test]
    async fn test_clipboard_clear() {
        let clipboard = ClipboardAdapter::new();
        
        clipboard.copy_files(
            ClipboardSource::Native,
            vec![PathBuf::from("/test/file.txt")],
        ).await.unwrap();
        
        assert!(clipboard.has_files().await.unwrap());
        
        clipboard.clear_clipboard().await.unwrap();
        
        // Internal clipboard cleared, but OS clipboard may still have content
        let internal = clipboard.internal_clipboard.read().await;
        assert!(internal.is_none());
    }
    
    #[tokio::test]
    async fn test_file_name_extraction() {
        assert_eq!(ClipboardAdapter::file_name(Path::new("/path/to/file.txt")), "file.txt");
        assert_eq!(ClipboardAdapter::file_name(Path::new("file.txt")), "file.txt");
        assert_eq!(ClipboardAdapter::file_name(Path::new("/path/to/folder")), "folder");
    }
}

