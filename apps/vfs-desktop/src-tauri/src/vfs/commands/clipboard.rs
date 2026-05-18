//! Clipboard Commands - Copy/Paste between Native FS and VFS
//!
//! Handles clipboard operations for copying and pasting files
//! between the native filesystem and VFS, and within VFS.

use std::sync::Arc;
use std::path::PathBuf;
use tauri::State;
use tracing::{info, error, warn};
use anyhow::Result;
use once_cell::sync::Lazy;
use parking_lot::RwLock as SyncRwLock;

use crate::vfs::application::VfsService;
use crate::vfs::adapters::ClipboardAdapter;
use crate::vfs::ports::{IClipboardService, ClipboardSource};
use super::state::VfsStateWrapper;

/// Global clipboard adapter with VfsService
pub static CLIPBOARD: Lazy<SyncRwLock<Option<Arc<ClipboardAdapter>>>> = Lazy::new(|| SyncRwLock::new(None));

/// Initialize the global clipboard with VfsService
pub fn init_global_clipboard(vfs_service: Arc<VfsService>) {
    let mut clipboard_lock = CLIPBOARD.write();
    *clipboard_lock = Some(Arc::new(ClipboardAdapter::with_file_ops_provider(vfs_service)));
    info!("Global clipboard initialized with VFS service");
}

/// Get the global clipboard, initializing if needed
fn get_clipboard_with_vfs(state: &VfsStateWrapper) -> Result<Arc<ClipboardAdapter>, String> {
    // Try to get existing clipboard
    {
        let clipboard_lock = CLIPBOARD.read();
        if let Some(clipboard) = clipboard_lock.as_ref() {
            info!("get_clipboard_with_vfs: returning existing clipboard adapter");
            return Ok(clipboard.clone());
        }
    }
    
    // Initialize with VFS service if not yet initialized
    if let Some(vfs) = state.get_service() {
        let mut clipboard_lock = CLIPBOARD.write();
        if clipboard_lock.is_none() {
            *clipboard_lock = Some(Arc::new(ClipboardAdapter::with_file_ops_provider(vfs)));
            info!("get_clipboard_with_vfs: initialized clipboard with VFS service on demand");
        } else {
            info!("get_clipboard_with_vfs: clipboard already initialized by another thread");
        }
        Ok(clipboard_lock.as_ref().unwrap().clone())
    } else {
        Err("VFS not initialized".to_string())
    }
}

fn get_clipboard_readonly() -> Arc<ClipboardAdapter> {
    let clipboard_lock = CLIPBOARD.read();
    clipboard_lock.as_ref().cloned().unwrap_or_else(|| {
        // Create a clipboard adapter without VFS service (read-only)
        Arc::new(ClipboardAdapter::new())
    })
}

#[allow(dead_code)]
fn generate_copy_name(original_name: &str) -> String {
    // Check if there's an extension
    if let Some(dot_pos) = original_name.rfind('.') {
        let (name_part, ext_part) = original_name.split_at(dot_pos);
        let ext = &ext_part[1..]; // Skip the dot
        
        // Try to find a number suffix
        if let Some(open_paren) = name_part.rfind('(') {
            if let Some(close_paren) = name_part.rfind(')') {
                if close_paren > open_paren {
                    let suffix = &name_part[open_paren + 1..close_paren];
                    if let Ok(num) = suffix.parse::<u32>() {
                        // Increment the number
                        let base_name = &name_part[..open_paren].trim_end();
                        return format!("{} ({})", base_name, num + 1);
                    }
                }
            }
        }
        
        // No existing number, add (1)
        format!("{} (1).{}", name_part, ext)
    } else {
        // No extension
        if let Some(open_paren) = original_name.rfind('(') {
            if let Some(close_paren) = original_name.rfind(')') {
                if close_paren > open_paren {
                    let suffix = &original_name[open_paren + 1..close_paren];
                    if let Ok(num) = suffix.parse::<u32>() {
                        let base_name = &original_name[..open_paren].trim_end();
                        return format!("{} ({})", base_name, num + 1);
                    }
                }
            }
        }
        
        format!("{} (1)", original_name)
    }
}

// Stub implementations - will be moved from commands.rs
// For now, these are placeholders to allow compilation

#[tauri::command]
pub async fn vfs_clipboard_copy(
    source_id: String,
    paths: Vec<String>,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use tracing::{info, debug};
    
    let paths_count = paths.len();
    info!("[vfs_clipboard_copy] Starting copy operation for {} files", paths_count);
    
    // Validate paths are not empty
    if paths.is_empty() {
        return Err("No paths provided for copy operation".to_string());
    }
    
    // Filter out empty paths
    let valid_paths: Vec<String> = paths.into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect();
    
    let valid_count = valid_paths.len();
    if valid_count == 0 {
        return Err("All provided paths are empty".to_string());
    }
    
    if valid_count < paths_count {
        info!("[vfs_clipboard_copy] Filtered {} empty paths, copying {} valid paths", paths_count - valid_count, valid_count);
    }
    
    let clipboard = get_clipboard_with_vfs(&state)?;
    debug!("[vfs_clipboard_copy] Got clipboard adapter");
    
    // Don't track operation here - operation will be created when paste happens
    // This ensures the operation modal shows the actual copy/move operation with progress
    let path_bufs: Vec<PathBuf> = valid_paths.into_iter().map(PathBuf::from).collect();
    debug!("[vfs_clipboard_copy] Calling clipboard.copy_files with {} paths", path_bufs.len());
    
    // Add timeout to prevent hanging - clipboard operations should be instant
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        clipboard.copy_files(ClipboardSource::Vfs { source_id }, path_bufs)
    ).await {
        Ok(Ok(())) => {
            debug!("[vfs_clipboard_copy] copy_files completed successfully");
            info!("[vfs_clipboard_copy] Copy to clipboard completed - operation will be tracked when paste happens");
            Ok(String::new())
        }
        Ok(Err(e)) => {
            error!("[vfs_clipboard_copy] copy_files failed: {}", e);
            Err(format!("Failed to copy files: {}", e))
        }
        Err(_) => {
            error!("[vfs_clipboard_copy] copy_files timed out after 5 seconds");
            Err("Copy operation timed out - clipboard may not be set".to_string())
        }
    }
}

#[tauri::command]
pub async fn vfs_clipboard_cut(
    source_id: String,
    paths: Vec<String>,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use tracing::{info, debug};
    
    let paths_count = paths.len();
    info!("[vfs_clipboard_cut] Starting cut operation for {} files", paths_count);
    
    // Validate paths are not empty
    if paths.is_empty() {
        return Err("No paths provided for cut operation".to_string());
    }
    
    // Filter out empty paths
    let valid_paths: Vec<String> = paths.into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect();
    
    let valid_count = valid_paths.len();
    if valid_count == 0 {
        return Err("All provided paths are empty".to_string());
    }
    
    if valid_count < paths_count {
        info!("[vfs_clipboard_cut] Filtered {} empty paths, cutting {} valid paths", paths_count - valid_count, valid_count);
    }
    
    let clipboard = get_clipboard_with_vfs(&state)?;
    debug!("[vfs_clipboard_cut] Got clipboard adapter");
    
    // Don't track operation here - operation will be created when paste happens
    // This ensures the operation modal shows the actual move operation with progress
    let path_bufs: Vec<PathBuf> = valid_paths.into_iter().map(PathBuf::from).collect();
    debug!("[vfs_clipboard_cut] Calling clipboard.cut_files with {} paths", path_bufs.len());
    
    // Add timeout to prevent hanging - clipboard operations should be instant
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        clipboard.cut_files(ClipboardSource::Vfs { source_id }, path_bufs)
    ).await {
        Ok(Ok(())) => {
            debug!("[vfs_clipboard_cut] cut_files completed successfully");
            info!("Cut to clipboard completed - operation will be tracked when paste happens");
            Ok(String::new())
        }
        Ok(Err(e)) => {
            error!("[vfs_clipboard_cut] cut_files failed: {}", e);
            Err(format!("Failed to cut files: {}", e))
        }
        Err(_) => {
            error!("[vfs_clipboard_cut] cut_files timed out after 5 seconds");
            Err("Cut operation timed out - clipboard may not be set".to_string())
        }
    }
}

#[tauri::command]
pub async fn vfs_clipboard_copy_native(
    paths: Vec<String>,
) -> Result<(), String> {
    let clipboard = get_clipboard_readonly();
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    clipboard.copy_files(ClipboardSource::Native, path_bufs)
        .await
        .map_err(|e| format!("Failed to copy native files: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn vfs_clipboard_copy_for_native(
    source_id: String,
    paths: Vec<String>,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    // Copy VFS files to clipboard so they can be pasted to native filesystem
    let clipboard = get_clipboard_with_vfs(&state)?;
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    clipboard.copy_files(ClipboardSource::Vfs { source_id }, path_bufs)
        .await
        .map_err(|e| format!("Failed to copy VFS files for native paste: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn vfs_clipboard_get(
    state: State<'_, VfsStateWrapper>,
) -> Result<Option<Vec<String>>, String> {
    let clipboard = get_clipboard_with_vfs(&state)?;
    match clipboard.get_clipboard().await {
        Ok(Some(content)) => {
            let paths: Vec<String> = content.paths.into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            Ok(Some(paths))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(format!("Failed to get clipboard: {}", e)),
    }
}

#[tauri::command]
pub async fn vfs_clipboard_get_content(
    state: State<'_, VfsStateWrapper>,
) -> Result<Option<serde_json::Value>, String> {
    use crate::vfs::ports::clipboard::ClipboardOperation;
    
    let clipboard = get_clipboard_with_vfs(&state)?;
    match clipboard.get_clipboard().await {
        Ok(Some(content)) => {
            let operation_str = match content.operation {
                ClipboardOperation::Copy => "copy",
                ClipboardOperation::Cut => "cut",
            };
            
            let source_str = match &content.source {
                crate::vfs::ports::clipboard::ClipboardSource::Native => "native",
                crate::vfs::ports::clipboard::ClipboardSource::Vfs { source_id } => source_id,
            };
            
            let paths: Vec<String> = content.paths.into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            
            Ok(Some(serde_json::json!({
                "operation": operation_str,
                "source": source_str,
                "paths": paths,
                "file_count": paths.len(),
            })))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(format!("Failed to get clipboard content: {}", e)),
    }
}

#[tauri::command]
pub async fn vfs_clipboard_has_files(
    state: State<'_, VfsStateWrapper>,
) -> Result<bool, String> {
    let clipboard = get_clipboard_with_vfs(&state)?;
    clipboard.has_files()
        .await
        .map_err(|e| format!("Failed to check clipboard: {}", e))
}

#[tauri::command]
pub async fn vfs_clipboard_clear(
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let clipboard = get_clipboard_with_vfs(&state)?;
    clipboard.clear_clipboard()
        .await
        .map_err(|e| format!("Failed to clear clipboard: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn vfs_clipboard_paste_to_vfs(
    dest_source_id: String,
    dest_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<serde_json::Value, String> {
    let clipboard = get_clipboard_with_vfs(&state)?;
    
    // Normalize path: trim whitespace, handle empty strings, normalize slashes
    let normalize_path = |p: &str| -> PathBuf {
        let trimmed = p.trim();
        if trimmed.is_empty() || trimmed == "/" {
            return PathBuf::from("/");
        }
        // Remove leading/trailing slashes, collapse multiple slashes
        let normalized: String = trimmed
            .trim_start_matches('/')
            .trim_end_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        
        if normalized.is_empty() {
            PathBuf::from("/")
        } else {
            PathBuf::from("/").join(normalized)
        }
    };
    
    let dest_path_buf = normalize_path(&dest_path);
    
    // Validate source_id
    let dest_source_id = dest_source_id.trim();
    if dest_source_id.is_empty() {
        return Err("Destination source ID cannot be empty".to_string());
    }
    
    info!("Pasting to VFS: source_id={}, path={:?} (normalized from {:?})", dest_source_id, dest_path_buf, dest_path);
    
    // Check if clipboard has files before attempting paste
    let has_files = clipboard.has_files().await
        .map_err(|e| format!("Failed to check clipboard: {}", e))?;
    
    if !has_files {
        return Err("Clipboard is empty. Copy or cut files first.".to_string());
    }
    
    info!("[vfs_clipboard_paste_to_vfs] Starting paste operation to {} at {:?}", dest_source_id, dest_path_buf);
    
    let result = clipboard.paste_to_vfs(dest_source_id, &dest_path_buf)
        .await
        .map_err(|e| {
            error!("[vfs_clipboard_paste_to_vfs] Paste to VFS failed: {}", e);
            let error_msg = format!("{}", e);
            
            // Try to extract operation_id from error if present (some operations might fail after creation)
            // This ensures frontend can still track failed operations
            let operation_id_in_error = if error_msg.contains("OPERATION_ID:") {
                error_msg.split("OPERATION_ID:").nth(1).map(|s| s.trim().to_string())
            } else {
                None
            };
            
            // Provide user-friendly error messages
            let user_friendly_error = if error_msg.contains("Clipboard is empty") {
                "Clipboard is empty. Copy or cut files first.".to_string()
            } else if error_msg.contains("Cannot copy") && error_msg.contains("into itself") {
                // Preserve the user-friendly recursive copy error message
                error_msg.clone()
            } else if error_msg.contains("Permission Denied") || error_msg.contains("Operation not permitted") {
                format!("Permission denied: {}", error_msg)
            } else if error_msg.contains("No such file") || error_msg.contains("does not exist") {
                format!("Destination path does not exist: {}", error_msg)
            } else if error_msg.contains("File operations provider not initialized") {
                "File operations not available. Please restart the application.".to_string()
            } else {
                format!("Failed to paste files: {}", error_msg)
            };
            
            // If we have an operation_id, append it to the error so frontend can track it
            if let Some(op_id) = operation_id_in_error {
                format!("{}|OPERATION_ID:{}", user_friendly_error, op_id)
            } else {
                user_friendly_error
            }
        })?;
    
    // Convert PasteResult to JSON
    let json_result = serde_json::json!({
        "files_pasted": result.files_pasted,
        "files_failed": result.files_failed,
        "pasted_paths": result.pasted_paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>(),
        "errors": result.errors,
        "operation_id": result.operation_id,
    });
    
    info!("[vfs_clipboard_paste_to_vfs] Paste completed: {} files pasted, {} failed, operation_id: {:?}", 
          result.files_pasted, result.files_failed, result.operation_id);
    
    // Log operation_id separately for easier debugging
    if let Some(op_id) = &result.operation_id {
        info!("[vfs_clipboard_paste_to_vfs] Operation ID for frontend tracking: {}", op_id);
    } else {
        warn!("[vfs_clipboard_paste_to_vfs] WARNING: No operation_id in result!");
    }
    
    Ok(json_result)
}

#[tauri::command]
pub async fn vfs_clipboard_paste_to_native(
    dest_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<serde_json::Value, String> {
    let clipboard = get_clipboard_with_vfs(&state)?;
    let dest_path_buf = PathBuf::from(dest_path);
    
    info!("Pasting to native: path={:?}", dest_path_buf);
    
    let result = clipboard.paste_to_native(&dest_path_buf)
        .await
        .map_err(|e| {
            error!("Paste to native failed: {}", e);
            format!("Failed to paste files: {}", e)
        })?;
    
    // Convert PasteResult to JSON
    let json_result = serde_json::json!({
        "files_pasted": result.files_pasted,
        "files_failed": result.files_failed,
        "pasted_paths": result.pasted_paths.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>(),
        "errors": result.errors,
    });
    
    info!("Paste to native completed: {} files pasted, {} failed", result.files_pasted, result.files_failed);
    
    Ok(json_result)
}

#[tauri::command]
pub async fn vfs_clipboard_read_native() -> Result<Vec<String>, String> {
    let clipboard = get_clipboard_readonly();
    match clipboard.read_native_clipboard().await {
        Ok(Some(paths)) => {
            let paths_str: Vec<String> = paths.into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            Ok(paths_str)
        }
        Ok(None) => Ok(Vec::new()),
        Err(e) => Err(format!("Failed to read native clipboard: {}", e)),
    }
}

#[tauri::command]
pub async fn vfs_clipboard_write_native(
    paths: Vec<String>,
) -> Result<(), String> {
    let clipboard = get_clipboard_readonly();
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    clipboard.write_native_clipboard(&path_bufs)
        .await
        .map_err(|e| format!("Failed to write to native clipboard: {}", e))?;
    Ok(())
}
