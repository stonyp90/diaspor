//! File Operations Commands
//!
//! POSIX-compliant file operations (list, read, write, copy, move, delete, etc.)

use tauri::State;
use tracing::{info, error, warn};
use std::sync::Arc;
use super::state::VfsStateWrapper;
use super::responses::{VfsFileMetadataResponse, TagResponse, VfsListFilesResponse};

#[tauri::command]
pub async fn vfs_list_files(
    source_id: String,
    path: String,
    limit: Option<u64>,
    continuation_token: Option<String>,
    state: State<'_, VfsStateWrapper>,
) -> Result<VfsListFilesResponse, String> {
    use std::path::Path;
    use std::time::Duration;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Normalize path: empty string or "/" means root
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    // Default limit for object storage is 50, no limit for local storage
    let effective_limit = limit.or_else(|| {
        // Check if this is object storage (S3, GCS, Azure, Oracle)
        if let Some(service) = state.get_service() {
            let sources = service.list_sources();
            if let Some(source) = sources.iter().find(|s| s.id == source_id) {
                match source.source_type {
                    crate::vfs::domain::StorageSourceType::S3 |
                    crate::vfs::domain::StorageSourceType::Custom(_) => Some(50),
                    _ => None, // No limit for local/network storage
                }
            } else {
                None
            }
        } else {
            None
        }
    });
    
    info!("[vfs_list_files] Listing files - source_id: {}, path: {:?} (normalized: {:?}), limit: {:?}, continuation_token: {:?}", 
        source_id, path, path_buf, effective_limit, continuation_token);
    
    // Add timeout to prevent infinite spinner (30 seconds for cloud storage, 10 seconds for local)
    let is_cloud_storage = {
        let sources = service.list_sources();
        sources.iter()
            .find(|s| s.id == source_id)
            .map(|s| matches!(s.source_type.category(), crate::vfs::domain::StorageCategory::Cloud))
            .unwrap_or(false)
    };
    
    let timeout_duration = if is_cloud_storage {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(10)
    };
    
    let list_result = tokio::time::timeout(
        timeout_duration,
        service.list_files_paginated(
            &source_id, 
            path_buf, 
            effective_limit, 
            continuation_token.as_deref()
        )
    ).await;
    
    let (virtual_files, next_token) = match list_result {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            error!("[vfs_list_files] Failed to list files for source {} at path {:?}: {}", source_id, path_buf, e);
            return Err(format!("Failed to list files: {}", e));
        }
        Err(_) => {
            error!("[vfs_list_files] Timeout listing files for source {} at path {:?} (timeout: {:?})", source_id, path_buf, timeout_duration);
            return Err(format!("Operation timed out after {:?}. The storage may be slow or unavailable.", timeout_duration));
        }
    };
    
    info!("[vfs_list_files] Found {} files in source {} at path {:?}, has_more: {}", 
        virtual_files.len(), source_id, path_buf, next_token.is_some());
    
    // Load metadata store for tags
    let metadata_store = super::helpers::get_metadata_store_instance().await.ok();
    
    // Convert VirtualFile to VfsFileMetadataResponse with tags
    let mut responses = Vec::new();
    for vf in virtual_files {
        let size_bytes = vf.size.bytes();
        let file_path = vf.path.clone();
        
        // Load tags and comments from metadata store (if available)
        let (tags, comments) = if let Some(store) = &metadata_store {
            if let Ok(Some(metadata)) = crate::vfs::ports::metadata::IMetadataStore::get(
                store.as_ref(),
                &source_id,
                &file_path,
            ).await {
                let file_tags = if !metadata.tags.is_empty() {
                    Some(metadata.tags.into_iter().map(|tag| {
                        super::responses::TagResponse {
                            name: tag.name,
                            color: tag.color,
                        }
                    }).collect())
                } else {
                    None
                };
                (file_tags, metadata.comment)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        
        responses.push(VfsFileMetadataResponse {
            id: vf.id.clone(),
            name: vf.name.clone(),
            path: vf.path.to_string_lossy().to_string(),
            size: size_bytes,
            size_human: format_bytes(size_bytes),
            last_modified: {
                // Convert SystemTime to ISO 8601 format (frontend expects ISO string)
                let duration = vf.last_modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::from_secs(0));
                let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(
                    duration.as_secs() as i64,
                    duration.subsec_nanos(),
                );
                datetime
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| {
                        // Fallback to current time if conversion fails
                        chrono::Utc::now().to_rfc3339()
                    })
            },
            is_directory: vf.is_directory,
            is_hidden: vf.is_hidden.unwrap_or_else(|| vf.name.starts_with('.')),
            tier_status: format!("{:?}", vf.tier_status.current_tier),
            is_cached: vf.tier_status.is_cached,
            can_warm: vf.tier_status.can_warm,
            can_transcode: vf.transcodable,
            transcode_status: vf.transcode_status.as_ref().map(|ts| format!("{:?}", ts.state)),
            transcode_progress: vf.transcode_status.as_ref().map(|ts| ts.progress),
            thumbnail: None, // TODO: Generate thumbnails
            mime_type: vf.content_type.clone(),
            tags,
            comments,
        });
    }
    
    Ok(VfsListFilesResponse {
        files: responses,
        continuation_token: next_token,
        total_count: None, // We don't track total count for now
    })
}

// Helper function to format bytes
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

#[tauri::command]
pub async fn vfs_mkdir(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    info!("[vfs_mkdir] Creating directory - source_id: {}, path: {:?}", source_id, path_buf);
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/diaspor/.cursor/debug.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"location":"files.rs:150","message":"vfs_mkdir entry","data":{{"sourceId":"{}","path":"{:?}","pathString":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"C"}}"#, 
            source_id, path_buf, path_buf.to_string_lossy(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::CreateDir,
        source_id.clone(),
        path_buf.to_string_lossy().to_string(),
        None,
        None,
    );
    
    match service.mkdir(&source_id, path_buf).await {
        Ok(()) => {
            info!("[vfs_mkdir] Successfully created directory: {:?}", path_buf);
            // #region agent log
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/diaspor/.cursor/debug.log") {
                use std::io::Write;
                let _ = writeln!(file, r#"{{"location":"files.rs:162","message":"vfs_mkdir success","data":{{"sourceId":"{}","path":"{:?}","operationId":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"C"}}"#, 
                    source_id, path_buf, operation_id,
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            }
            // #endregion
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete mkdir operation: {}", e));
            Ok(operation_id)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            error!("[vfs_mkdir] Failed to create directory {:?}: {}", path_buf, error_msg);
            // #region agent log
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/diaspor/.cursor/debug.log") {
                use std::io::Write;
                let _ = writeln!(file, r#"{{"location":"files.rs:172","message":"vfs_mkdir error","data":{{"sourceId":"{}","path":"{:?}","error":"{}","operationId":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"D"}}"#, 
                    source_id, path_buf, error_msg, operation_id,
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            }
            // #endregion
            OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                .unwrap_or_else(|err| error!("Failed to fail mkdir operation: {}", err));
            // Append operation_id to error message for frontend tracking (format: "error|OPERATION_ID:operation_id")
            Err(format!("Failed to create directory: {}|OPERATION_ID:{}", error_msg, operation_id))
        }
    }
}

#[tauri::command]
pub async fn vfs_mkdir_p(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use std::path::Path;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    info!("[vfs_mkdir_p] Creating directory tree - source_id: {}, path: {:?}", source_id, path_buf);
    
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::CreateDir,
        source_id.clone(),
        path_buf.to_string_lossy().to_string(),
        None,
        None,
    );
    
    match service.mkdir_p(&source_id, path_buf).await {
        Ok(()) => {
            info!("[vfs_mkdir_p] Successfully created directory tree: {:?}", path_buf);
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete mkdir_p operation: {}", e));
            Ok(())
        }
        Err(e) => {
            error!("[vfs_mkdir_p] Failed to create directory tree {:?}: {}", path_buf, e);
            OperationTrackingHelper::fail_operation(&operation_id, format!("{}", e))
                .unwrap_or_else(|err| error!("Failed to fail mkdir_p operation: {}", err));
            Err(format!("Failed to create directory tree: {}", e))
        }
    }
}

#[tauri::command]
pub async fn vfs_rmdir(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    info!("[vfs_rmdir] Removing directory - source_id: {}, path: {:?}", source_id, path_buf);
    
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::RemoveDir,
        source_id.clone(),
        path_buf.to_string_lossy().to_string(),
        None,
        None,
    );
    
    match service.rmdir(&source_id, path_buf).await {
        Ok(()) => {
            info!("[vfs_rmdir] Successfully removed directory: {:?}", path_buf);
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete rmdir operation: {}", e));
            Ok(operation_id)
        }
        Err(e) => {
            error!("[vfs_rmdir] Failed to remove directory {:?}: {}", path_buf, e);
            let error_msg = format!("{}", e);
            OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                .unwrap_or_else(|err| error!("Failed to fail rmdir operation: {}", err));
            // Append operation_id to error message for frontend tracking (format: "error|OPERATION_ID:operation_id")
            Err(format!("Failed to remove directory: {}|OPERATION_ID:{}", error_msg, operation_id))
        }
    }
}

#[tauri::command]
pub async fn vfs_rename(
    source_id: String,
    old_path: String,
    new_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    
    // Validate source_id
    if source_id.trim().is_empty() {
        return Err("Source ID cannot be empty".to_string());
    }
    
    // Validate old_path
    if old_path.trim().is_empty() {
        return Err("Old path cannot be empty".to_string());
    }
    
    // Validate new_path
    if new_path.trim().is_empty() {
        return Err("New path cannot be empty".to_string());
    }
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Normalize paths: trim whitespace, ensure leading slash, remove trailing slashes (except root)
    let normalize_path = |p: &str| -> String {
        let trimmed = p.trim();
        if trimmed.is_empty() || trimmed == "/" {
            return "/".to_string();
        }
        let normalized = trimmed.trim_start_matches('/').trim_end_matches('/').to_string();
        if normalized.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", normalized)
        }
    };
    
    let old_path_normalized = normalize_path(&old_path);
    let new_path_normalized = normalize_path(&new_path);
    
    // Additional validation: cannot rename root directory
    if old_path_normalized == "/" {
        return Err("Cannot rename root directory".to_string());
    }
    
    if new_path_normalized == "/" {
        return Err("Cannot rename to root directory".to_string());
    }
    
    // Check for invalid characters in new path (platform-specific)
    #[cfg(unix)]
    {
        if new_path_normalized.contains('\0') {
            return Err("New path contains invalid character (null byte)".to_string());
        }
    }
    
    let old_path_buf = Path::new(&old_path_normalized);
    let new_path_buf = Path::new(&new_path_normalized);
    
    // Check if old path exists
    if !service.exists(&source_id, old_path_buf).await
        .map_err(|e| format!("Failed to check if source file exists: {}", e))? {
        return Err(format!("Source file does not exist: {}", old_path_normalized));
    }
    
    // Check if this is a folder rename on object storage (blocked operation)
    let file_stat = service.stat(&source_id, old_path_buf).await.ok();
    let is_directory = file_stat.as_ref().map(|s| s.is_dir).unwrap_or(false);
    
    if is_directory {
        // Check if source is object storage
        let sources = service.list_sources();
        if let Some(source) = sources.iter().find(|s| s.id == source_id) {
            use crate::vfs::commands::helpers::is_object_storage_type;
            if is_object_storage_type(&source.source_type) {
                return Err(
                    "Folder rename is not supported on object storage (S3, GCS, Azure Blob, Oracle). \
                    This operation would require copying and deleting all objects under the prefix, \
                    which is resource-intensive and can be very slow for large folders. \
                    Please rename individual files instead, or use a different storage type.".to_string()
                );
            }
        }
    }
    
    // Check if new path already exists (unless it's the same file)
    if old_path_normalized != new_path_normalized
        && service.exists(&source_id, new_path_buf).await
            .map_err(|e| format!("Failed to check if destination file exists: {}", e))? {
            return Err(format!("Destination file already exists: {}", new_path_normalized));
        }
    
    info!("[vfs_rename] Renaming - source_id: {}, from: {:?}, to: {:?}", source_id, old_path_buf, new_path_buf);
    
    let file_size = service.stat(&source_id, old_path_buf).await.ok().map(|stat| stat.size);
    
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::Rename,
        source_id.clone(),
        old_path_buf.to_string_lossy().to_string(),
        Some(new_path_buf.to_string_lossy().to_string()),
        file_size,
    );
    
    // Immediately transition operation to InProgress so it's visible in OperationsPanel
    OperationTrackingHelper::update_progress(&operation_id, 0)
        .unwrap_or_else(|e| warn!("Failed to update rename operation progress: {}", e));
    
    match service.rename(&source_id, old_path_buf, new_path_buf).await {
        Ok(()) => {
            info!("[vfs_rename] Successfully renamed: {:?} -> {:?}", old_path_buf, new_path_buf);
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete rename operation: {}", e));
            Ok(operation_id)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            error!("[vfs_rename] Failed to rename {:?} -> {:?}: {}", old_path_buf, new_path_buf, error_msg);
            OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                .unwrap_or_else(|err| error!("Failed to fail rename operation: {}", err));
            
            // Provide user-friendly error messages
            let user_friendly_error = if error_msg.contains("Permission denied") || error_msg.contains("permission") {
                "Permission denied: You don't have permission to rename this file. Please check file permissions.".to_string()
            } else if error_msg.contains("No such file") || error_msg.contains("not found") {
                format!("File not found: The source file '{}' does not exist.", old_path_normalized)
            } else if error_msg.contains("already exists") || error_msg.contains("exists") {
                format!("File already exists: A file named '{}' already exists at that location.", new_path_normalized)
            } else if error_msg.contains("Invalid argument") || error_msg.contains("invalid") {
                format!("Invalid path: The new path '{}' contains invalid characters.", new_path_normalized)
            } else {
                format!("Failed to rename '{}' to '{}': {}", old_path_normalized, new_path_normalized, error_msg)
            };
            
            // Return error with operation_id appended so frontend can track it
            // Format: "error_message|OPERATION_ID:operation_id"
            Err(format!("{}|OPERATION_ID:{}", user_friendly_error, operation_id))
        }
    }
}

#[tauri::command]
pub async fn vfs_copy(
    source_id: String,
    from_path: String,
    to_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    use crate::vfs::ports::file_operations::CopyOptions;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Normalize paths
    let from_path_buf = if from_path.is_empty() || from_path == "/" {
        Path::new("/")
    } else {
        Path::new(&from_path)
    };
    
    let to_path_buf = if to_path.is_empty() || to_path == "/" {
        Path::new("/")
    } else {
        Path::new(&to_path)
    };
    
    info!("[vfs_copy] Copying file - source_id: {}, from: {:?}, to: {:?}", source_id, from_path_buf, to_path_buf);
    
    // Get file size for tracking (best effort)
    let file_size = service.stat(&source_id, from_path_buf).await.ok().map(|stat| stat.size);
    
    // Track copy operation
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::Copy,
        source_id.clone(),
        from_path_buf.to_string_lossy().to_string(),
        Some(to_path_buf.to_string_lossy().to_string()),
        file_size,
    );
    
    // Immediately transition operation to InProgress so it's visible in OperationsPanel
    OperationTrackingHelper::update_progress(&operation_id, 0)
        .unwrap_or_else(|e| warn!("Failed to update copy operation progress: {}", e));
    
    // Perform copy using VfsService public method
    let options = CopyOptions {
        overwrite: false, // Don't overwrite by default for safety
        preserve_attributes: true,
        recursive: true, // Support directory copying
        follow_symlinks: false,
    };
    
    match service.copy(&source_id, from_path_buf, to_path_buf, options).await {
        Ok(()) => {
            info!("[vfs_copy] Successfully copied file: {:?} -> {:?}", from_path_buf, to_path_buf);
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete copy operation: {}", e));
            Ok(operation_id)
        }
        Err(e) => {
            error!("[vfs_copy] Failed to copy file {:?} -> {:?}: {}", from_path_buf, to_path_buf, e);
            OperationTrackingHelper::fail_operation(&operation_id, format!("{}", e))
                .unwrap_or_else(|err| error!("Failed to fail copy operation: {}", err));
            Err(format!("Failed to copy file: {}", e))
        }
    }
}

#[tauri::command]
pub async fn vfs_move(
    source_id: String,
    from_path: String,
    to_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    use crate::vfs::ports::file_operations::MoveOptions;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Normalize paths
    let from_path_buf = if from_path.is_empty() || from_path == "/" {
        Path::new("/")
    } else {
        Path::new(&from_path)
    };
    
    let to_path_buf = if to_path.is_empty() || to_path == "/" {
        Path::new("/")
    } else {
        Path::new(&to_path)
    };
    
    // Check if this is a folder move on object storage (blocked operation)
    let file_stat = service.stat(&source_id, from_path_buf).await.ok();
    let is_directory = file_stat.as_ref().map(|s| s.is_dir).unwrap_or(false);
    
    if is_directory {
        // Check if source is object storage
        let sources = service.list_sources();
        if let Some(source) = sources.iter().find(|s| s.id == source_id) {
            use crate::vfs::commands::helpers::is_object_storage_type;
            if is_object_storage_type(&source.source_type) {
                return Err(
                    "Folder move is not supported on object storage (S3, GCS, Azure Blob, Oracle). \
                    This operation would require copying and deleting all objects under the prefix, \
                    which is resource-intensive and can be very slow for large folders. \
                    Please move individual files instead, or use a different storage type.".to_string()
                );
            }
        }
    }
    
    info!("[vfs_move] Moving file - source_id: {}, from: {:?}, to: {:?}", source_id, from_path_buf, to_path_buf);
    
    // Get file size for tracking (best effort)
    let file_size = service.stat(&source_id, from_path_buf).await.ok().map(|stat| stat.size);
    
    // Track move operation
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::Move,
        source_id.clone(),
        from_path_buf.to_string_lossy().to_string(),
        Some(to_path_buf.to_string_lossy().to_string()),
        file_size,
    );
    
    // Immediately transition operation to InProgress so it's visible in OperationsPanel
    OperationTrackingHelper::update_progress(&operation_id, 0)
        .unwrap_or_else(|e| warn!("Failed to update move operation progress: {}", e));
    
    // Perform move using VfsService public method
    let options = MoveOptions {
        overwrite: false, // Don't overwrite by default for safety
    };
    
    match service.mv(&source_id, from_path_buf, to_path_buf, options).await {
        Ok(()) => {
            info!("[vfs_move] Successfully moved file: {:?} -> {:?}", from_path_buf, to_path_buf);
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete move operation: {}", e));
            Ok(operation_id)
        }
        Err(e) => {
            error!("[vfs_move] Failed to move file {:?} -> {:?}: {}", from_path_buf, to_path_buf, e);
            let error_msg = format!("{}", e);
            OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                .unwrap_or_else(|err| error!("Failed to fail move operation: {}", err));
            // Append operation_id to error message for frontend tracking (format: "error|OPERATION_ID:operation_id")
            Err(format!("Failed to move file: {}|OPERATION_ID:{}", error_msg, operation_id))
        }
    }
}

#[tauri::command]
pub async fn vfs_delete(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    
    // Validate source_id
    if source_id.trim().is_empty() {
        return Err("Source ID cannot be empty".to_string());
    }
    
    // Validate path
    if path.trim().is_empty() {
        return Err("Path cannot be empty".to_string());
    }
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Normalize path: trim whitespace, ensure leading slash, remove trailing slashes (except root)
    let normalize_path = |p: &str| -> String {
        let trimmed = p.trim();
        if trimmed.is_empty() || trimmed == "/" {
            return "/".to_string();
        }
        let normalized = trimmed.trim_start_matches('/').trim_end_matches('/').to_string();
        if normalized.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", normalized)
        }
    };
    
    let path_normalized = normalize_path(&path);
    
    // Additional validation: cannot delete root directory
    if path_normalized == "/" {
        return Err("Cannot delete root directory".to_string());
    }
    
    let path_buf = Path::new(&path_normalized);
    
    // Check if file exists before attempting deletion
    if !service.exists(&source_id, path_buf).await
        .map_err(|e| format!("Failed to check if file exists: {}", e))? {
        return Err(format!("File does not exist: {}", path_normalized));
    }
    
    // Get file metadata for better error messages
    let file_stat = service.stat(&source_id, path_buf).await.ok();
    let file_name = path_buf.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path_normalized)
        .to_string();
    let is_directory = file_stat.as_ref().map(|s| s.is_dir).unwrap_or(false);
    let file_size = file_stat.as_ref().map(|stat| stat.size);
    
    info!("[vfs_delete] Deleting {} - source_id: {}, path: {:?}", 
          if is_directory { "directory" } else { "file" },
          source_id, path_buf);
    
    // Track delete operation
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::Delete,
        source_id.clone(),
        path_buf.to_string_lossy().to_string(),
        None,
        file_size,
    );
    
    // Perform delete using VfsService public method
    match service.rm(&source_id, path_buf).await {
        Ok(()) => {
            info!("[vfs_delete] Successfully deleted {}: {:?}", 
                  if is_directory { "directory" } else { "file" },
                  path_buf);
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete delete operation: {}", e));
            Ok(operation_id)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            error!("[vfs_delete] Failed to delete {} {:?}: {}", 
                   if is_directory { "directory" } else { "file" },
                   path_buf, error_msg);
            OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                .unwrap_or_else(|err| error!("Failed to fail delete operation: {}", err));
            
            // Provide user-friendly error messages
            let user_friendly_error = if error_msg.contains("Permission denied") || error_msg.contains("permission") {
                format!("Permission denied: You don't have permission to delete \"{}\". Please check file permissions.", file_name)
            } else if error_msg.contains("No such file") || error_msg.contains("not found") || error_msg.contains("does not exist") {
                format!("File not found: \"{}\" does not exist.", file_name)
            } else if error_msg.contains("Directory not empty") || error_msg.contains("not empty") {
                format!("Cannot delete \"{}\": Directory is not empty. Please delete files inside first.", file_name)
            } else if error_msg.contains("Invalid argument") || error_msg.contains("invalid") {
                format!("Invalid path: \"{}\" contains invalid characters.", path_normalized)
            } else if is_directory {
                format!("Failed to delete folder \"{}\": {}", file_name, error_msg)
            } else {
                format!("Failed to delete file \"{}\": {}", file_name, error_msg)
            };
            
            // Append operation_id to error message for frontend tracking (format: "error|OPERATION_ID:operation_id")
            Err(format!("{}|OPERATION_ID:{}", user_friendly_error, operation_id))
        }
    }
}

#[tauri::command]
pub async fn vfs_delete_recursive(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Normalize path: empty string or "/" means root
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    info!("[vfs_delete_recursive] Deleting recursively - source_id: {}, path: {:?}", source_id, path_buf);
    
    // Get file size for tracking (best effort)
    let file_size = service.stat(&source_id, path_buf).await.ok().map(|stat| stat.size);
    
    // Track delete operation
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::Delete,
        source_id.clone(),
        path_buf.to_string_lossy().to_string(),
        None,
        file_size,
    );
    
    // Perform recursive delete using VfsService public method
    match service.rm_rf(&source_id, path_buf).await {
        Ok(()) => {
            info!("[vfs_delete_recursive] Successfully deleted recursively: {:?}", path_buf);
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete delete operation: {}", e));
            Ok(operation_id)
        }
        Err(e) => {
            error!("[vfs_delete_recursive] Failed to delete recursively {:?}: {}", path_buf, e);
            let error_msg = format!("{}", e);
            OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                .unwrap_or_else(|err| error!("Failed to fail delete operation: {}", err));
            // Append operation_id to error message for frontend tracking (format: "error|OPERATION_ID:operation_id")
            Err(format!("Failed to delete: {}|OPERATION_ID:{}", error_msg, operation_id))
        }
    }
}

/// Batch delete multiple files/folders as a single operation
#[tauri::command]
pub async fn vfs_batch_delete(
    source_id: String,
    paths: Vec<String>,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracker::{OperationType, OperationStatus, OperationFile};
    use crate::vfs::commands::get_operation_tracker;
    
    if paths.is_empty() {
        return Err("Paths array cannot be empty".to_string());
    }
    
    let service_arc = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    info!("[vfs_batch_delete] Starting batch delete: source_id={}, paths={}", source_id, paths.len());
    
    // Collect file metadata for operation tracking
    let mut operation_files = Vec::new();
    
    for path_str in &paths {
        let path_buf = Path::new(path_str);
        
        // Get file size and metadata
        let file_stat = service_arc.stat(&source_id, path_buf).await.ok();
        let file_size = file_stat.as_ref().map(|s| s.size).unwrap_or(0);
        
        operation_files.push(OperationFile {
            local_path: path_str.clone(),
            remote_path: path_str.clone(), // For delete, source and dest are the same
            file_size,
            bytes_processed: 0,
            status: Some(OperationStatus::Pending),
            error: None,
        });
    }
    
    // Create a single operation for all files
    let operation_tracker = get_operation_tracker();
    let source_path_summary = if paths.len() == 1 {
        paths[0].clone()
    } else {
        format!("{} file(s)", paths.len())
    };
    
    let operation_id = operation_tracker.create_multi_file_operation(
        OperationType::Delete,
        source_id.clone(),
        source_path_summary,
        None,
        Some(operation_files.clone()),
    );
    
    info!("[vfs_batch_delete] Created operation {} with {} file(s)", operation_id, paths.len());
    
    // Immediately transition operation to InProgress so it's visible in OperationsPanel
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    OperationTrackingHelper::update_progress(&operation_id, 0)
        .unwrap_or_else(|e| warn!("Failed to update batch delete operation progress: {}", e));
    
    // Process deletions in parallel using tokio::spawn
    // service_arc is already Arc<VfsService>, so we can clone it directly
    let source_id_clone = source_id.clone();
    let operation_id_clone = operation_id.clone();
    
    // Spawn parallel delete tasks (up to 16 concurrent)
    let mut handles = Vec::new();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(16));
    
    for (idx, path_str) in paths.iter().enumerate() {
        let path_str_clone = path_str.clone();
        let path_buf = Path::new(&path_str_clone).to_path_buf();
        let service = service_arc.clone();
        let source_id = source_id_clone.clone();
        let operation_id = operation_id_clone.clone();
        let file_size = operation_files[idx].file_size;
        let permit = semaphore.clone();
        
        let handle = tokio::spawn(async move {
            use crate::vfs::commands::get_operation_tracker;
            
            let _permit = permit.acquire().await.unwrap();
            let tracker = get_operation_tracker();
            
            // Update file status to InProgress
            let _ = tracker.update_file_progress(
                &operation_id,
                &path_str_clone,
                0,
                Some(OperationStatus::InProgress),
            );
            
            // Perform delete
            let result = service.rm_rf(&source_id, &path_buf).await;
            
            match result {
                Ok(_) => {
                    // Mark file as completed
                    let _ = tracker.update_file_progress(
                        &operation_id,
                        &path_str_clone,
                        file_size,
                        Some(OperationStatus::Completed),
                    );
                    Ok(())
                }
                Err(e) => {
                    let error_msg = format!("{}", e);
                    // Mark file as failed
                    let _ = tracker.update_file_progress_with_error(
                        &operation_id,
                        &path_str_clone,
                        error_msg.clone(),
                    );
                    Err(error_msg)
                }
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all deletions to complete
    let mut failures = Vec::new();
    for (idx, handle) in handles.into_iter().enumerate() {
        match handle.await {
            Ok(Ok(_)) => {
                // Success
            }
            Ok(Err(e)) => {
                failures.push((idx, e));
            }
            Err(e) => {
                failures.push((idx, format!("Task panicked: {}", e)));
            }
        }
    }
    
    // Check results and complete operation
    if failures.is_empty() {
        // All succeeded
        operation_tracker.complete_operation(&operation_id)
            .unwrap_or_else(|e| error!("Failed to complete batch delete operation: {}", e));
        info!("[vfs_batch_delete] Successfully completed batch delete operation {}", operation_id);
    } else {
        // Some failed
        let error_msg = format!("{} out of {} deletions failed", failures.len(), paths.len());
        operation_tracker.fail_operation(&operation_id, error_msg.clone())
            .unwrap_or_else(|e| error!("Failed to fail batch delete operation: {}", e));
        warn!("[vfs_batch_delete] Batch delete operation {} completed with {} failures", operation_id, failures.len());
    }
    
    Ok(operation_id)
}

/// Batch move item (from_path, to_path)
#[derive(Debug, serde::Deserialize)]
pub struct BatchMoveItem {
    pub from_path: String,
    pub to_path: String,
}

/// Batch move multiple files/folders as a single operation
#[tauri::command]
pub async fn vfs_batch_move(
    source_id: String,
    moves: Vec<BatchMoveItem>,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracker::{OperationType, OperationStatus, OperationFile};
    use crate::vfs::commands::get_operation_tracker;
    use crate::vfs::ports::file_operations::MoveOptions;
    
    if moves.is_empty() {
        return Err("Moves array cannot be empty".to_string());
    }
    
    let service_arc = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    info!("[vfs_batch_move] Starting batch move: source_id={}, moves={}", source_id, moves.len());
    
    // Collect file metadata for operation tracking
    let mut operation_files = Vec::new();
    
    for move_item in &moves {
        let from_path_buf = Path::new(&move_item.from_path);
        
        // Get file size and metadata
        let file_stat = service_arc.stat(&source_id, from_path_buf).await.ok();
        let file_size = file_stat.as_ref().map(|s| s.size).unwrap_or(0);
        
        operation_files.push(OperationFile {
            local_path: move_item.from_path.clone(),
            remote_path: move_item.to_path.clone(),
            file_size,
            bytes_processed: 0,
            status: Some(OperationStatus::Pending),
            error: None,
        });
    }
    
    // Create a single operation for all files
    let operation_tracker = get_operation_tracker();
    let source_path_summary = if moves.len() == 1 {
        moves[0].from_path.clone()
    } else {
        format!("{} file(s)", moves.len())
    };
    
    let destination_path_summary = if moves.len() == 1 {
        Some(moves[0].to_path.clone())
    } else {
        Some(format!("{} destination(s)", moves.len()))
    };
    
    let operation_id = operation_tracker.create_multi_file_operation(
        OperationType::Move,
        source_id.clone(),
        source_path_summary,
        destination_path_summary,
        Some(operation_files.clone()),
    );
    
    info!("[vfs_batch_move] Created operation {} with {} file(s)", operation_id, moves.len());
    
    // Process moves in parallel using tokio::spawn
    // service_arc is already Arc<VfsService>, so we can clone it directly
    let source_id_clone = source_id.clone();
    let operation_id_clone = operation_id.clone();
    
    // Spawn parallel move tasks (up to 16 concurrent)
    let mut handles = Vec::new();
    let semaphore = Arc::new(tokio::sync::Semaphore::new(16));
    
    for (idx, move_item) in moves.iter().enumerate() {
        let from_path = move_item.from_path.clone();
        let to_path = move_item.to_path.clone();
        let from_path_buf = Path::new(&from_path).to_path_buf();
        let to_path_buf = Path::new(&to_path).to_path_buf();
        let service = service_arc.clone();
        let source_id = source_id_clone.clone();
        let operation_id = operation_id_clone.clone();
        let file_size = operation_files[idx].file_size;
        let move_options = MoveOptions { overwrite: false };
        let permit = semaphore.clone();
        
        let handle = tokio::spawn(async move {
            use crate::vfs::commands::get_operation_tracker;
            
            let _permit = permit.acquire().await.unwrap();
            let tracker = get_operation_tracker();
            
            // Update file status to InProgress
            let _ = tracker.update_file_progress(
                &operation_id,
                &from_path,
                0,
                Some(OperationStatus::InProgress),
            );
            
            // Perform move
            let result = service.mv(&source_id, &from_path_buf, &to_path_buf, move_options).await;
            
            match result {
                Ok(_) => {
                    // Mark file as completed
                    let _ = tracker.update_file_progress(
                        &operation_id,
                        &from_path,
                        file_size,
                        Some(OperationStatus::Completed),
                    );
                    Ok(())
                }
                Err(e) => {
                    let error_msg = format!("{}", e);
                    // Mark file as failed
                    let _ = tracker.update_file_progress_with_error(
                        &operation_id,
                        &from_path,
                        error_msg.clone(),
                    );
                    Err(error_msg)
                }
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all moves to complete
    let mut failures = Vec::new();
    for (idx, handle) in handles.into_iter().enumerate() {
        match handle.await {
            Ok(Ok(_)) => {
                // Success
            }
            Ok(Err(e)) => {
                failures.push((idx, e));
            }
            Err(e) => {
                failures.push((idx, format!("Task panicked: {}", e)));
            }
        }
    }
    
    // Check results and complete operation
    if failures.is_empty() {
        // All succeeded
        operation_tracker.complete_operation(&operation_id)
            .unwrap_or_else(|e| error!("Failed to complete batch move operation: {}", e));
        info!("[vfs_batch_move] Successfully completed batch move operation {}", operation_id);
    } else {
        // Some failed
        let error_msg = format!("{} out of {} moves failed", failures.len(), moves.len());
        operation_tracker.fail_operation(&operation_id, error_msg.clone())
            .unwrap_or_else(|e| error!("Failed to fail batch move operation: {}", e));
        warn!("[vfs_batch_move] Batch move operation {} completed with {} failures", operation_id, failures.len());
    }
    
    Ok(operation_id)
}

#[tauri::command]
pub async fn vfs_chmod(
    source_id: String,
    path: String,
    mode: u32,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    info!("[vfs_chmod] Changing permissions - source_id: {}, path: {:?}, mode: {:o}", source_id, path_buf, mode);
    
    service.chmod(&source_id, path_buf, mode).await
        .map_err(|e| format!("Failed to change permissions: {}", e))
}

#[tauri::command]
pub async fn vfs_stat(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<VfsFileMetadataResponse, String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    let file_stat = service.stat(&source_id, path_buf).await
        .map_err(|e| format!("Failed to stat file: {}", e))?;
    
    // Load tags and comments from metadata store (if available)
    let metadata_store = super::helpers::get_metadata_store_instance().await.ok();
    let (tags, comments) = if let Some(store) = &metadata_store {
        if let Ok(Some(metadata)) = crate::vfs::ports::metadata::IMetadataStore::get(
            store.as_ref(),
            &source_id,
            path_buf,
        ).await {
            let file_tags = if !metadata.tags.is_empty() {
                Some(metadata.tags.into_iter().map(|tag| {
                    TagResponse {
                        name: tag.name,
                        color: tag.color,
                    }
                }).collect())
            } else {
                None
            };
            (file_tags, metadata.comment)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    
    // Convert FileStat to VfsFileMetadataResponse
    let name = path_buf.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    
    Ok(VfsFileMetadataResponse {
        id: format!("{}_{}", source_id, path),
        name,
        path: path_buf.to_string_lossy().to_string(),
        size: file_stat.size,
        size_human: format_bytes(file_stat.size),
        last_modified: file_stat.mtime.and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH).ok()
        }).and_then(|d| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(
                d.as_secs() as i64,
                d.subsec_nanos(),
            ).map(|dt| dt.to_rfc3339())
        }).unwrap_or_default(),
        is_directory: file_stat.is_dir,
        is_hidden: false, // TODO: Check if file is hidden
        tier_status: "hot".to_string(),
        is_cached: false,
        can_warm: false,
        can_transcode: false,
        transcode_status: None,
        transcode_progress: None,
        thumbnail: None,
        mime_type: None,
        tags,
        comments,
    })
}

#[tauri::command]
pub async fn vfs_touch(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    info!("[vfs_touch] Touching file - source_id: {}, path: {:?}", source_id, path_buf);
    
    // Touch creates an empty file or updates mtime
    // Use write with empty data if file doesn't exist, or stat + chmod to update mtime
    if service.exists(&source_id, path_buf).await.unwrap_or(false) {
        // File exists, just update mtime (touch it)
        // For now, we'll write empty data which effectively touches it
        service.write(&source_id, path_buf, &[]).await
            .map_err(|e| format!("Failed to touch file: {}", e))?;
    } else {
        // File doesn't exist, create it
        service.write(&source_id, path_buf, &[]).await
            .map_err(|e| format!("Failed to create file: {}", e))?;
    }
    
    Ok(())
}

#[tauri::command]
pub async fn vfs_exists(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<bool, String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    service.exists(&source_id, path_buf).await
        .map_err(|e| format!("Failed to check if file exists: {}", e))
}

#[tauri::command]
pub async fn vfs_read_text(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    let data = service.read(&source_id, path_buf).await
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    String::from_utf8(data)
        .map_err(|e| format!("Failed to decode file as UTF-8: {}", e))
}

#[tauri::command]
pub async fn vfs_read_file_bytes(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<Vec<u8>, String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    service.read(&source_id, path_buf).await
        .map_err(|e| format!("Failed to read file: {}", e))
}

/// Get Downloads folder path (cross-platform)
fn get_downloads_folder() -> Result<std::path::PathBuf, String> {
    // Use home_dir/Downloads (works on macOS, Windows, Linux)
    let downloads_path = dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory".to_string())?
        .join("Downloads");
    
    // Ensure Downloads folder exists
    if !downloads_path.exists() {
        std::fs::create_dir_all(&downloads_path)
            .map_err(|e| format!("Failed to create Downloads folder: {}", e))?;
    }
    
    Ok(downloads_path)
}

#[tauri::command]
pub async fn vfs_download_file(
    source_id: String,
    path: String,
    dest_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let src_path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    let dest_path_buf = Path::new(&dest_path);
    
    info!("[vfs_download_file] Downloading - source_id: {}, from: {:?}, to: {:?}", source_id, src_path_buf, dest_path_buf);
    
    let file_size = service.stat(&source_id, src_path_buf).await.ok().map(|stat| stat.size);
    
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::Download,
        source_id.clone(),
        src_path_buf.to_string_lossy().to_string(),
        Some(dest_path_buf.to_string_lossy().to_string()),
        file_size,
    );
    
    // Check if source is object storage - use chunked download manager
    let sources = service.list_sources();
    let is_object_storage = if let Some(source) = sources.iter().find(|s| s.id == source_id) {
        use crate::vfs::commands::helpers::is_object_storage_type;
        is_object_storage_type(&source.source_type)
    } else {
        false
    };
    
    if is_object_storage {
        // Use DownloadManager for chunked downloads with resume support
        use crate::vfs::commands::helpers::get_download_manager;
        use crate::vfs::adapters::S3StorageAdapter;
        
        let source = sources.iter().find(|s| s.id == source_id)
            .ok_or_else(|| "Storage source not found".to_string())?;
        
        // Get S3 config
        let bucket = source.config.path_or_bucket.clone();
        let region = source.config.region.clone()
            .unwrap_or_else(|| "us-east-1".to_string());
        let endpoint = source.config.endpoint.clone();
        let name = source.name.clone();
        
        // Read credentials from environment
        let access_key = std::env::var("AWS_ACCESS_KEY_ID").ok()
            .or_else(|| std::env::var("aws_access_key_id").ok());
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok()
            .or_else(|| std::env::var("aws_secret_access_key").ok());
        let session_token = std::env::var("AWS_SESSION_TOKEN").ok()
            .or_else(|| std::env::var("aws_session_token").ok());
        
        // Create S3 adapter to get operator
        let s3_adapter = S3StorageAdapter::new(
            bucket.clone(),
            region.clone(),
            access_key,
            secret_key,
            session_token,
            endpoint,
            name,
        ).await.map_err(|e| format!("Failed to create S3 adapter: {}", e))?;
        
        let operator = s3_adapter.operator().clone();
        
        // Normalize remote path (remove leading slash)
        let remote_path = src_path_buf.to_string_lossy()
            .trim_start_matches('/')
            .to_string();
        
        let download_manager = get_download_manager();
        download_manager.load_states().await.ok(); // Load persisted states
        
        // Start chunked download
        let download_id = download_manager.start_download(
            &operator,
            &source_id,
            &remote_path,
            dest_path_buf,
            Some(operation_id.clone()),
        ).await.map_err(|e| format!("Failed to start download: {}", e))?;
        
        info!("[vfs_download_file] Started chunked download: {}", download_id);
        
        // Download runs in background - operation tracker will be updated by DownloadManager
        // Return operation_id for frontend to track progress
        Ok(operation_id)
    } else {
        // For local/NAS storage, use simple read/write (no chunking needed)
        // Mark operation as InProgress before starting download
        use crate::vfs::commands::get_operation_tracker;
        let tracker = get_operation_tracker();
        if let Err(e) = tracker.update_operation_progress(&operation_id, 0, file_size) {
            warn!("Failed to update download operation status: {}", e);
        }
        
        // Read from VFS source
        let data = service.read(&source_id, src_path_buf).await
            .map_err(|e| {
                // Mark operation as failed
                let _ = OperationTrackingHelper::fail_operation(&operation_id, format!("Failed to read source file: {}", e));
                format!("Failed to read source file: {}", e)
            })?;
        
        // Update progress: file read (50% of operation)
        let bytes_read = data.len() as u64;
        if let Err(e) = tracker.update_operation_progress(&operation_id, bytes_read, file_size) {
            warn!("Failed to update download progress: {}", e);
        }
        
        // Write to native filesystem
        if let Some(parent) = dest_path_buf.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| {
                    let _ = OperationTrackingHelper::fail_operation(&operation_id, format!("Failed to create destination directory: {}", e));
                    format!("Failed to create destination directory: {}", e)
                })?;
        }
        
        std::fs::write(dest_path_buf, &data)
            .map_err(|e| {
                let _ = OperationTrackingHelper::fail_operation(&operation_id, format!("Failed to write destination file: {}", e));
                format!("Failed to write destination file: {}", e)
            })?;
        
        // Mark operation as completed
        OperationTrackingHelper::complete_operation(&operation_id)
            .unwrap_or_else(|e| error!("Failed to complete download operation: {}", e));
        
        // Return operation_id for frontend to track progress
        Ok(operation_id)
    }
}

/// Download file to Downloads folder automatically
#[tauri::command]
pub async fn vfs_download_to_downloads(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use std::path::Path;
    
    let _service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let src_path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    // Get filename from source path
    let file_name = src_path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    
    // Get Downloads folder path
    let downloads_folder = get_downloads_folder()?;
    
    // Build destination path
    let dest_path = downloads_folder.join(file_name);
    
    // Handle filename conflicts by appending a number
    let mut final_dest_path = dest_path.clone();
    let mut counter = 1;
    while final_dest_path.exists() {
        let stem = dest_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("download");
        let extension = dest_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let new_name = if extension.is_empty() {
            format!("{} ({})", stem, counter)
        } else {
            format!("{} ({}).{}", stem, counter, extension)
        };
        final_dest_path = downloads_folder.join(new_name);
        counter += 1;
    }
    
    // Use existing download_file command
    let operation_id = vfs_download_file(
        source_id,
        path,
        final_dest_path.to_string_lossy().to_string(),
        state,
    ).await?;
    
    // Return operation_id for frontend to track progress
    Ok(operation_id)
}

#[tauri::command]
pub async fn vfs_write_text(
    source_id: String,
    path: String,
    content: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    let data = content.as_bytes().to_vec();
    service.write(&source_id, path_buf, &data).await
        .map_err(|e| format!("Failed to write file: {}", e))
}

#[tauri::command]
pub async fn vfs_append_text(
    source_id: String,
    path: String,
    content: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    // Read existing content
    let existing_data = service.read(&source_id, path_buf).await
        .unwrap_or_default();
    
    // Append new content
    let mut new_data = existing_data;
    new_data.extend_from_slice(content.as_bytes());
    
    service.write(&source_id, path_buf, &new_data).await
        .map_err(|e| format!("Failed to append to file: {}", e))
}

/// Get folder size recursively (for mounted storage: local, NAS, FSx)
/// Returns 0 for object storage or if calculation fails
#[tauri::command]
pub async fn vfs_get_folder_size(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<u64, String> {
    use std::path::Path;
    use crate::vfs::commands::helpers::is_object_storage_type;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Check if this is mounted storage (local, NAS, FSx) - only calculate for these
    let sources = service.list_sources();
    let is_mounted_storage = if let Some(source) = sources.iter().find(|s| s.id == source_id) {
        !is_object_storage_type(&source.source_type)
    } else {
        return Err("Storage source not found".to_string());
    };
    
    if !is_mounted_storage {
        // Object storage doesn't support folder size calculation
        return Ok(0);
    }
    
    let path_buf = if path.is_empty() || path == "/" {
        Path::new("/")
    } else {
        Path::new(&path)
    };
    
    // Use service's get_metadata which already calculates folder sizes for mounted storage
    let metadata = service.get_metadata(&source_id, path_buf).await
        .map_err(|e| format!("Failed to get folder metadata: {}", e))?;
    
    // Return the size in bytes (already calculated recursively for folders)
    Ok(metadata.size.bytes())
}
