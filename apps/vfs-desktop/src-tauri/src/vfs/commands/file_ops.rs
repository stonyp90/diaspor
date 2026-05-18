//! File Opening and Thumbnail Commands
//!
//! Commands for opening files and getting thumbnails

use tauri::State;
use std::path::Path;
use tracing::{debug, info, warn};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use super::state::VfsStateWrapper;
use super::helpers;

#[tauri::command]
pub async fn vfs_open_file(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use std::path::Path;
    use std::process::Command;
    use tracing::info;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Get the source to determine storage type
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    let path_buf = Path::new(&path);
    
    // Reduced logging - use debug level to prevent excessive logs during navigation
    debug!("[vfs_open_file] Opening file - source_id: {}, path: {:?}", source_id, path_buf);
    
    // Check if it's a directory - directories should be navigated, not opened
    let stat = service.stat(&source_id, path_buf).await
        .map_err(|e| format!("Failed to get file stats: {}", e))?;
    
    if stat.is_dir {
        return Err("Cannot open directory. Use navigation instead.".to_string());
    }
    
    // For local storage, open directly
    if source.source_type.category() == crate::vfs::domain::StorageCategory::Local {
        // Resolve VFS path to native path
        let native_path = {
            let path_str = path_buf.to_string_lossy();
            let normalized = path_str.trim_start_matches('/').trim_start_matches('\\');
            
            let base_path = source.mount_point
                .or_else(|| {
                    if !source.config.path_or_bucket.is_empty() {
                        Some(std::path::PathBuf::from(&source.config.path_or_bucket))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| std::path::PathBuf::from("/"));
            
            if normalized.is_empty() {
                base_path
            } else {
                base_path.join(normalized)
            }
        };
        
        let absolute_path = native_path.canonicalize()
            .map_err(|e| format!("Failed to resolve path: {}", e))?;
        
        if !absolute_path.exists() {
            return Err(format!("File does not exist: {:?}", absolute_path));
        }
        
        // Platform-specific open command
        #[cfg(target_os = "macos")]
        {
            let status = Command::new("open")
                .arg(&absolute_path)
                .status()
                .map_err(|e| format!("Failed to open file: {}", e))?;
            
            if !status.success() {
                return Err(format!("Failed to open file (exit code: {:?})", status.code()));
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            use crate::vfs::platform::CommandBuilder;
            let path_str = absolute_path.to_string_lossy().replace('/', "\\");
            let status = CommandBuilder::new("cmd")
                .args(["/C", "start", "", &path_str])
                .status()
                .map_err(|e| format!("Failed to open file: {}", e))?;
            
            if !status.success() {
                return Err(format!("Failed to open file (exit code: {:?})", status.code()));
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            let status = Command::new("xdg-open")
                .arg(&absolute_path)
                .status()
                .map_err(|e| format!("Failed to open file: {}", e))?;
            
            if !status.success() {
                return Err(format!("Failed to open file (exit code: {:?})", status.code()));
            }
        }
        
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            return Err("Opening files is not supported on this platform".to_string());
        }
        
        return Ok(());
    }
    
    // For cloud/network storage, download to temp location first, then open
    // This is a simplified implementation - in production, you might want to:
    // 1. Check if file is already cached locally
    // 2. Stream download while opening
    // 3. Handle cleanup of temp files
    
    use std::env;
    let temp_dir = env::temp_dir();
    let file_name = path_buf.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    
    let temp_path = temp_dir.join(format!("ursly_open_{}_{}", 
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_secs(),
        file_name));
    
    // Download file to temp location
    info!("[vfs_open_file] Downloading cloud file to temp location: {:?}", temp_path);
    
    match service.read_file(&source_id, path_buf).await {
        Ok(data) => {
            tokio::fs::write(&temp_path, data).await
                .map_err(|e| format!("Failed to write temp file: {}", e))?;
        }
        Err(e) => {
            return Err(format!("Failed to download file: {}", e));
        }
    }
    
    // Open the temp file
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg(&temp_path)
            .status()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        
        if !status.success() {
            return Err(format!("Failed to open file (exit code: {:?})", status.code()));
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        use crate::vfs::platform::CommandBuilder;
        let path_str = temp_path.to_string_lossy().replace('/', "\\");
        let status = CommandBuilder::new("cmd")
            .args(["/C", "start", "", &path_str])
            .status()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        
        if !status.success() {
            return Err(format!("Failed to open file (exit code: {:?})", status.code()));
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        let status = Command::new("xdg-open")
            .arg(&temp_path)
            .status()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        
        if !status.success() {
            return Err(format!("Failed to open file (exit code: {:?})", status.code()));
        }
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        return Err("Opening files is not supported on this platform".to_string());
    }
    
    // Note: Temp file cleanup could be handled by a background task
    // For now, we leave it to the OS temp file cleanup
    
    Ok(())
}

#[tauri::command]
pub async fn vfs_open_file_with(
    _source_id: String,
    path: String,
    app_path: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    // Open file with specified application
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", &app_path, &path])
            .spawn()
            .map_err(|e| format!("Failed to open file with app: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        use crate::vfs::platform::CommandBuilder;
        CommandBuilder::new("cmd")
            .args(["/C", "start", "", &app_path, &path])
            .spawn()
            .map_err(|e| format!("Failed to open file with app: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new(&app_path)
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file with app: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn vfs_get_apps_for_file(
    _source_id: String,
    _path: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    // Stub implementation - return empty list
    Ok(vec![])
}

#[tauri::command]
pub async fn vfs_get_os_preferences() -> Result<serde_json::Value, String> {
    // Return OS preferences and platform info
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    let path_separator = if cfg!(target_os = "windows") {
        "\\"
    } else {
        "/"
    };

    // Get show_hidden_files setting from UI settings (defaults to false - show hidden files by default)
    let ui_settings = crate::settings::get_settings().get_ui();
    let show_hidden_files = ui_settings.show_hidden_files.unwrap_or(false);

    Ok(serde_json::json!({
        "platform": platform,
        "isMac": cfg!(target_os = "macos"),
        "isWindows": cfg!(target_os = "windows"),
        "isLinux": cfg!(target_os = "linux"),
        "pathSeparator": path_separator,
        "modifierKey": if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" },
        "altKey": if cfg!(target_os = "macos") { "⌥" } else { "Alt" },
        "shiftKey": if cfg!(target_os = "macos") { "⇧" } else { "Shift" },
        "deleteKey": if cfg!(target_os = "macos") { "⌫" } else { "Backspace" },
        "theme": "system",
        "show_hidden_files": show_hidden_files
    }))
}

/// Get thumbnail for a single file
/// Returns base64-encoded data URL (data:image/png;base64,...) or None if unavailable
#[tauri::command]
pub async fn vfs_get_thumbnail(
    source_id: String,
    path: String,
    size: Option<u32>,
    state: State<'_, VfsStateWrapper>,
) -> Result<Option<String>, String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let size = size.unwrap_or(128);
    let vfs_path = Path::new(&path);
    
    // Get storage source to resolve VFS path to native path
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // For local storage, resolve VFS path to native path
    let native_path = if source.source_type.category() == crate::vfs::domain::StorageCategory::Local {
        // Get base path from mount_point or config (same logic as LocalStorageAdapter)
        let base_path = source.mount_point
            .or_else(|| {
                if !source.config.path_or_bucket.is_empty() {
                    Some(std::path::PathBuf::from(&source.config.path_or_bucket))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| std::path::PathBuf::from("/"));
        
        // Resolve VFS path to native path (same logic as LocalStorageAdapter::resolve_path)
        // If already an absolute path starting with base, use it directly
        if vfs_path.is_absolute() && vfs_path.starts_with(&base_path) {
            vfs_path.to_path_buf()
        } else {
            // Normalize: strip leading slashes
            let path_str = vfs_path.to_string_lossy();
            let normalized = path_str.trim_start_matches('/').trim_start_matches('\\');
            
            // Handle empty path (root of source)
            if normalized.is_empty() {
                base_path
            } else {
                base_path.join(normalized)
            }
        }
    } else {
        // For cloud/network storage, we can't generate native thumbnails directly
        // Return None - thumbnails would need to be downloaded first
        debug!("Thumbnail generation not supported for cloud/network storage: {}", source_id);
        return Ok(None);
    };
    
    // Check if file exists
    if !native_path.exists() {
        debug!("File does not exist for thumbnail: {:?}", native_path);
        return Ok(None);
    }
    
    // Get thumbnail queue
    let queue = helpers::get_thumbnail_queue().await
        .map_err(|e| format!("Failed to get thumbnail queue: {}", e))?;
    
    // Generate thumbnail
    match queue.get_thumbnail(&native_path, size).await {
        Ok(Some(thumb_data)) => {
            // Convert to base64 data URL
            let base64_data = STANDARD.encode(&thumb_data.data);
            let data_url = format!("data:image/png;base64,{}", base64_data);
            Ok(Some(data_url))
        }
        Ok(None) => {
            debug!("No thumbnail available for: {:?}", native_path);
            Ok(None)
        }
        Err(e) => {
            warn!("Failed to generate thumbnail for {:?}: {}", native_path, e);
            Ok(None) // Return None instead of error to allow UI to continue
        }
    }
}

/// Get thumbnails for multiple files in batch
/// Returns array of [path, thumbnailDataUrl | null] tuples
/// Note: Tauri automatically converts camelCase (filePaths) to snake_case (file_paths)
#[tauri::command]
pub async fn vfs_get_thumbnails_batch(
    source_id: String,
    file_paths: Vec<String>,
    size: Option<u32>,
    state: State<'_, VfsStateWrapper>,
) -> Result<Vec<(String, Option<String>)>, String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let size = size.unwrap_or(128);
    
    // Get storage source
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Only support local storage for now
    if source.source_type.category() != crate::vfs::domain::StorageCategory::Local {
        debug!("Batch thumbnail generation not supported for cloud/network storage: {}", source_id);
        // Return empty thumbnails for all files
        return Ok(file_paths.into_iter().map(|p| (p, None)).collect());
    }
    
    // Resolve base path
    let base_path = source.mount_point
        .or_else(|| {
            if !source.config.path_or_bucket.is_empty() {
                Some(std::path::PathBuf::from(&source.config.path_or_bucket))
            } else {
                None
            }
        })
        .unwrap_or_else(|| std::path::PathBuf::from("/"));
    
    // Get thumbnail queue
    let queue = helpers::get_thumbnail_queue().await
        .map_err(|e| format!("Failed to get thumbnail queue: {}", e))?;
    
    // Prepare thumbnail requests
    use crate::vfs::adapters::thumbnail_queue::ThumbnailRequest;
    use crate::vfs::adapters::native_thumbnail::ThumbnailType;
    
    let requests: Vec<ThumbnailRequest> = file_paths.iter()
        .filter_map(|vfs_path_str| {
            let vfs_path = Path::new(vfs_path_str);
            
            // Resolve VFS path to native path (same logic as LocalStorageAdapter::resolve_path)
            let native_path = if vfs_path.is_absolute() && vfs_path.starts_with(&base_path) {
                vfs_path.to_path_buf()
            } else {
                // Normalize: strip leading slashes
                let path_str = vfs_path.to_string_lossy();
                let normalized = path_str.trim_start_matches('/').trim_start_matches('\\');
                
                // Handle empty path (root of source)
                if normalized.is_empty() {
                    base_path.clone()
                } else {
                    base_path.join(normalized)
                }
            };
            
            // Check if file exists and can have thumbnail
            if !native_path.exists() {
                return None;
            }
            
            // Check file extension
            let ext = native_path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let thumb_type = ThumbnailType::from_extension(ext);
            
            if !thumb_type.is_supported() {
                return None;
            }
            
            Some(ThumbnailRequest {
                path: native_path,
                size,
                thumb_type,
            })
        })
        .collect();
    
    if requests.is_empty() {
        return Ok(file_paths.into_iter().map(|p| (p, None)).collect());
    }
    
    // Generate thumbnails in batch
    info!("[vfs_get_thumbnails_batch] Generating {} thumbnails for source {}", requests.len(), source_id);
    let results = queue.get_thumbnails_batch(requests).await;
    
    // Build result map: native_path -> thumbnail data URL
    let mut result_map: std::collections::HashMap<std::path::PathBuf, Option<String>> = std::collections::HashMap::new();
    
    let mut success_count = 0;
    for result in results {
        let thumbnail_data_url = result.thumbnail.map(|thumb_data| {
            success_count += 1;
            let base64_data = STANDARD.encode(&thumb_data.data);
            format!("data:image/png;base64,{}", base64_data)
        });
        result_map.insert(result.path, thumbnail_data_url);
    }
    
    info!("[vfs_get_thumbnails_batch] Generated {} thumbnails successfully out of {} requests", success_count, result_map.len());
    
    // Map back to VFS paths
    let mut final_results = Vec::new();
    for vfs_path_str in file_paths {
        let vfs_path = Path::new(&vfs_path_str);
        
        // Resolve VFS path to native path (same logic as above)
        let native_path = if vfs_path.is_absolute() && vfs_path.starts_with(&base_path) {
            vfs_path.to_path_buf()
        } else {
            let path_str = vfs_path.to_string_lossy();
            let normalized = path_str.trim_start_matches('/').trim_start_matches('\\');
            if normalized.is_empty() {
                base_path.clone()
            } else {
                base_path.join(normalized)
            }
        };
        
        let thumbnail = result_map.get(&native_path).cloned().flatten();
        final_results.push((vfs_path_str, thumbnail));
    }
    
    Ok(final_results)
}
