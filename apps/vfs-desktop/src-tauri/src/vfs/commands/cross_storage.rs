//! Cross-Storage Operations Commands
//!
//! Commands for copying/moving files between different storage sources

use tauri::State;
use std::path::Path;
use tracing::{info, error, warn};
use super::state::VfsStateWrapper;
use crate::vfs::operation_tracking::OperationTrackingHelper;
use crate::vfs::operation_tracker::OperationType;

#[tauri::command]
pub async fn vfs_copy_to_source(
    src_source_id: String,
    from_path: String,
    dest_source_id: String,
    to_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
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
    
    info!("[vfs_copy_to_source] Copying file - from: {}:{:?}, to: {}:{:?}", 
          src_source_id, from_path_buf, dest_source_id, to_path_buf);
    
    // Get file size for tracking (best effort)
    let file_size = service.stat(&src_source_id, from_path_buf).await.ok().map(|stat| stat.size);
    
    // Track copy operation
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::Copy,
        src_source_id.clone(),
        from_path_buf.to_string_lossy().to_string(),
        Some(to_path_buf.to_string_lossy().to_string()),
        file_size,
    );

    // Immediately transition operation to InProgress so it's visible in OperationsPanel
    OperationTrackingHelper::update_progress(&operation_id, 0)
        .unwrap_or_else(|e| warn!("Failed to update cross-storage copy operation progress: {}", e));

    // Perform cross-storage copy
    // First read from source, then write to destination
    // Read file data from source using VfsService public method
    let data = service.read(&src_source_id, from_path_buf).await
        .map_err(|e| format!("Failed to read source file: {}", e))?;
    
    // Write file data to destination using VfsService public method
    match service.write(&dest_source_id, to_path_buf, &data).await {
        Ok(()) => {
            info!("[vfs_copy_to_source] Successfully copied file");
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete copy operation: {}", e));
            Ok(())
        }
        Err(e) => {
            error!("[vfs_copy_to_source] Failed to copy file: {}", e);
            OperationTrackingHelper::fail_operation(&operation_id, format!("{}", e))
                .unwrap_or_else(|err| error!("Failed to fail copy operation: {}", err));
            Err(format!("Failed to copy file: {}", e))
        }
    }
}

#[tauri::command]
pub async fn vfs_move_to_source(
    src_source_id: String,
    from_path: String,
    dest_source_id: String,
    to_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
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
    
    info!("[vfs_move_to_source] Moving file - from: {}:{:?}, to: {}:{:?}", 
          src_source_id, from_path_buf, dest_source_id, to_path_buf);
    
    // Get file size for tracking (best effort)
    let file_size = service.stat(&src_source_id, from_path_buf).await.ok().map(|stat| stat.size);
    
    // Track move operation
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::Move,
        src_source_id.clone(),
        from_path_buf.to_string_lossy().to_string(),
        Some(to_path_buf.to_string_lossy().to_string()),
        file_size,
    );

    // Immediately transition operation to InProgress so it's visible in OperationsPanel
    OperationTrackingHelper::update_progress(&operation_id, 0)
        .unwrap_or_else(|e| warn!("Failed to update cross-storage move operation progress: {}", e));

    // Perform cross-storage move (copy + delete)
    // First copy to destination
    // Read file data from source using VfsService public method
    let data = service.read(&src_source_id, from_path_buf).await
        .map_err(|e| format!("Failed to read source file: {}", e))?;
    
    // Write file data to destination using VfsService public method
    service.write(&dest_source_id, to_path_buf, &data).await
        .map_err(|e| format!("Failed to write destination file: {}", e))?;
    
    // Delete source file after successful copy using VfsService public method
    match service.rm(&src_source_id, from_path_buf).await {
        Ok(()) => {
            info!("[vfs_move_to_source] Successfully moved file");
            OperationTrackingHelper::complete_operation(&operation_id)
                .unwrap_or_else(|e| error!("Failed to complete move operation: {}", e));
            Ok(())
        }
        Err(e) => {
            error!("[vfs_move_to_source] Failed to delete source file after copy: {}", e);
            // Try to clean up destination file if source deletion failed
            let _ = service.rm(&dest_source_id, to_path_buf).await;
            OperationTrackingHelper::fail_operation(&operation_id, format!("Failed to delete source: {}", e))
                .unwrap_or_else(|err| error!("Failed to fail move operation: {}", err));
            Err(format!("Failed to move file: {}", e))
        }
    }
}

#[tauri::command]
pub async fn vfs_get_transfer_targets(
    state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let sources = service.list_sources();
    
    let targets: Vec<serde_json::Value> = sources.into_iter().map(|source| {
        serde_json::json!({
            "id": source.id,
            "name": source.name,
            "category": format!("{:?}", source.source_type.category()),
        })
    }).collect();
    
    Ok(targets)
}

#[tauri::command]
pub async fn vfs_batch_copy_to_source(
    src_source_id: String,
    paths: Vec<String>,
    dest_source_id: String,
    dest_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<serde_json::Value, String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let dest_path_buf = if dest_path.is_empty() || dest_path == "/" {
        Path::new("/")
    } else {
        Path::new(&dest_path)
    };
    
    let mut copied = Vec::new();
    let mut failed = Vec::new();
    
    for path in paths {
        let src_path_buf = if path.is_empty() || path == "/" {
            Path::new("/")
        } else {
            Path::new(&path)
        };
        
        let file_name = src_path_buf.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        let final_dest = if dest_path_buf == Path::new("/") {
            Path::new("/").join(file_name)
        } else {
            dest_path_buf.join(file_name)
        };
        
        // Read from source
        let data = match service.read(&src_source_id, src_path_buf).await {
            Ok(data) => data,
            Err(e) => {
                failed.push(format!("{}: Failed to read: {}", path, e));
                continue;
            }
        };
        
        // Write to destination
        match service.write(&dest_source_id, &final_dest, &data).await {
            Ok(()) => copied.push(path),
            Err(e) => failed.push(format!("{}: Failed to write: {}", path, e)),
        }
    }
    
    Ok(serde_json::json!({
        "copied": copied.len(),
        "failed": failed.len(),
        "copied_paths": copied,
        "errors": failed,
    }))
}

#[tauri::command]
pub async fn vfs_batch_move_to_source(
    src_source_id: String,
    paths: Vec<String>,
    dest_source_id: String,
    dest_path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<serde_json::Value, String> {
    use std::path::Path;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let dest_path_buf = if dest_path.is_empty() || dest_path == "/" {
        Path::new("/")
    } else {
        Path::new(&dest_path)
    };
    
    let mut moved = Vec::new();
    let mut failed = Vec::new();
    
    for path in paths {
        let src_path_buf = if path.is_empty() || path == "/" {
            Path::new("/")
        } else {
            Path::new(&path)
        };
        
        let file_name = src_path_buf.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        let final_dest = if dest_path_buf == Path::new("/") {
            Path::new("/").join(file_name)
        } else {
            dest_path_buf.join(file_name)
        };
        
        // Read from source
        let data = match service.read(&src_source_id, src_path_buf).await {
            Ok(data) => data,
            Err(e) => {
                failed.push(format!("{}: Failed to read: {}", path, e));
                continue;
            }
        };
        
        // Write to destination
        match service.write(&dest_source_id, &final_dest, &data).await {
            Ok(()) => {
                // Delete source after successful copy
                if let Err(e) = service.rm(&src_source_id, src_path_buf).await {
                    failed.push(format!("{}: Failed to delete source: {}", path, e));
                    // Try to clean up destination
                    let _ = service.rm(&dest_source_id, &final_dest).await;
                } else {
                    moved.push(path);
                }
            }
            Err(e) => failed.push(format!("{}: Failed to write: {}", path, e)),
        }
    }
    
    Ok(serde_json::json!({
        "moved": moved.len(),
        "failed": failed.len(),
        "moved_paths": moved,
        "errors": failed,
    }))
}
