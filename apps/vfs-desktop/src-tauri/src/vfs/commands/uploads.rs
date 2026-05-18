//! Upload/Download Operations Commands
//!
//! Commands for managing file uploads and downloads

use tauri::State;
use super::state::VfsStateWrapper;
use super::helpers::get_operation_tracker;
use crate::vfs::operation_tracker::{OperationType, OperationFile};
use std::path::Path;

/// Start a new upload - stub implementation
#[tauri::command]
pub async fn vfs_start_upload(
    _source_id: String,
    _dest_path: String,
    _local_path: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    // Return a placeholder upload ID
    Ok(uuid::Uuid::new_v4().to_string())
}

/// List all active uploads
#[tauri::command]
pub async fn vfs_list_uploads() -> Result<Vec<serde_json::Value>, String> {
    use super::helpers::get_upload_manager;
    use serde_json::json;
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"location":"uploads.rs:25","message":"vfs_list_uploads entry","data":{{}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"H"}}"#,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    let upload_manager = get_upload_manager();
    
    // Ensure persisted states are loaded (idempotent - safe to call multiple times)
    // This is needed because get_upload_manager() is sync but load_states() is async
    let _ = upload_manager.load_states().await;
    
    let uploads = upload_manager.list_uploads().await;
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let upload_ids: Vec<String> = uploads.iter().map(|u| u.upload_id.clone()).collect();
        let _ = writeln!(file, r#"{{"location":"uploads.rs:35","message":"vfs_list_uploads got uploads from manager","data":{{"uploadCount":{},"uploadIds":{:?},"statuses":{:?}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"H"}}"#,
            uploads.len(), upload_ids, uploads.iter().map(|u| format!("{:?}", u.status)).collect::<Vec<_>>(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    // Convert uploads to JSON format expected by frontend
    let json_uploads: Vec<serde_json::Value> = uploads.into_iter().map(|state| {
        json!({
            "upload_id": state.upload_id,
            "operation_id": state.operation_id,
            "source_id": state.source_id,
            "key": state.key,
            "local_path": state.local_path.to_string_lossy(),
            "total_size": state.total_size,
            "bytes_uploaded": state.bytes_uploaded,
            "current_part": state.current_part,
            "total_parts": state.total_parts,
            "status": match state.status {
                crate::vfs::multipart_upload::UploadStatus::Pending => "Pending",
                crate::vfs::multipart_upload::UploadStatus::InProgress => "InProgress",
                crate::vfs::multipart_upload::UploadStatus::Completed => "Completed",
                crate::vfs::multipart_upload::UploadStatus::Failed => "Failed",
                crate::vfs::multipart_upload::UploadStatus::Paused => "Paused",
            },
            "error": state.error,
            "created_at": state.created_at.map(|dt| dt.timestamp()),
            "completed_at": state.completed_at.map(|dt| dt.timestamp()),
            "last_updated_at": state.last_updated_at.map(|dt| dt.timestamp()),
        })
    }).collect();
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"location":"uploads.rs:58","message":"vfs_list_uploads returning","data":{{"jsonUploadCount":{}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"H"}}"#,
            json_uploads.len(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    Ok(json_uploads)
}

/// Resume a paused upload
#[tauri::command]
pub async fn vfs_resume_upload(
    upload_id: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use super::helpers::get_upload_manager;
    use crate::vfs::adapters::S3StorageAdapter;
    use tracing::info;
    
    let upload_manager = get_upload_manager();
    
    // Get upload state to find source_id
    let upload_state = upload_manager.list_uploads().await
        .into_iter()
        .find(|u| u.upload_id == upload_id)
        .ok_or_else(|| format!("Upload not found: {}", upload_id))?;
    
    let source_id = upload_state.source_id.clone();
    
    // Get service to access source information
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Get source to get config
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Check if this is S3 storage
    use crate::vfs::domain::StorageSourceType;
    if !matches!(source.source_type, StorageSourceType::S3 | StorageSourceType::S3Compatible) {
        return Err(format!("Resume upload is only supported for S3 storage. Source {} is {:?}", source_id, source.source_type));
    }
    
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
    
    info!("[vfs_resume_upload] Resuming upload: {}", upload_id);
    
    upload_manager.resume_upload(&operator, &upload_id).await
        .map_err(|e| format!("Failed to resume upload: {}", e))?;
    
    Ok(())
}

/// Pause an active upload
#[tauri::command]
pub async fn vfs_pause_upload(
    upload_id: String,
) -> Result<(), String> {
    use super::helpers::get_upload_manager;
    use tracing::info;
    
    let upload_manager = get_upload_manager();
    
    info!("[vfs_pause_upload] Pausing upload: {}", upload_id);
    
    upload_manager.pause_upload(&upload_id).await
        .map_err(|e| format!("Failed to pause upload: {}", e))?;
    
    Ok(())
}

/// Cancel an upload
#[tauri::command]
pub async fn vfs_cancel_upload(
    upload_id: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use super::helpers::get_upload_manager;
    use crate::vfs::adapters::S3StorageAdapter;
    use tracing::info;
    
    let upload_manager = get_upload_manager();
    
    // Get upload state to find source_id
    let upload_state = upload_manager.list_uploads().await
        .into_iter()
        .find(|u| u.upload_id == upload_id)
        .ok_or_else(|| format!("Upload not found: {}", upload_id))?;
    
    let source_id = upload_state.source_id.clone();
    
    // Get service to access source information
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Get source to get config
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Check if this is S3 storage
    use crate::vfs::domain::StorageSourceType;
    if !matches!(source.source_type, StorageSourceType::S3 | StorageSourceType::S3Compatible) {
        return Err(format!("Cancel upload is only supported for S3 storage. Source {} is {:?}", source_id, source.source_type));
    }
    
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
    
    info!("[vfs_cancel_upload] Canceling upload: {}", upload_id);
    
    upload_manager.cancel_upload(&operator, &upload_id).await
        .map_err(|e| format!("Failed to cancel upload: {}", e))?;
    
    Ok(())
}

/// List all operations
#[tauri::command]
pub async fn vfs_list_operations() -> Result<Vec<serde_json::Value>, String> {
    use super::helpers::get_operation_tracker;
    use serde_json::json;

    let tracker = get_operation_tracker();
    let operations = tracker.get_all_operations();

    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let ops_summary: Vec<_> = operations.iter().map(|op| {
            format!("{{\"id\":\"{}\",\"type\":\"{:?}\",\"status\":\"{:?}\"}}", op.operation_id, op.operation_type, op.status)
        }).collect();
        let _ = writeln!(file, r#"{{"location":"uploads.rs:258","message":"vfs_list_operations result","data":{{"totalOps":{},"opsTypes":[{}]}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"D"}}"#, 
            operations.len(),
            ops_summary.join(","),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion

    // Convert operations to JSON values
    let json_operations: Vec<serde_json::Value> = operations.into_iter().map(|op| {
        json!({
            "operation_id": op.operation_id,
            "operation_type": format!("{:?}", op.operation_type),
            "source_id": op.source_id,
            "source_path": op.source_path,
            "destination_path": op.destination_path,
            "file_size": op.file_size,
            "bytes_processed": op.bytes_processed,
            "status": format!("{:?}", op.status),
            "error": op.error,
            "files": op.files.map(|files| files.into_iter().map(|f| json!({
                "local_path": f.local_path,
                "remote_path": f.remote_path,
                "file_size": f.file_size,
                "bytes_processed": f.bytes_processed,
                "status": f.status.map(|s| format!("{:?}", s)),
                "error": f.error,
            })).collect::<Vec<_>>()),
            "file_count": op.file_count,
            "created_at": op.created_at.map(|dt| dt.timestamp()),
            "completed_at": op.completed_at.map(|dt| dt.timestamp()),
            "last_updated_at": op.last_updated_at.map(|dt| dt.timestamp()),
        })
    }).collect();
    
    Ok(json_operations)
}

/// Start a multipart upload
#[tauri::command]
pub async fn vfs_start_multipart_upload(
    _source_id: String,
    _dest_path: String,
    _local_path: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    Ok(uuid::Uuid::new_v4().to_string())
}

/// Upload a folder
#[tauri::command]
pub async fn vfs_upload_folder(
    _source_id: String,
    _dest_path: String,
    _local_path: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    Ok(uuid::Uuid::new_v4().to_string())
}

/// Batch upload item (file or folder)
#[derive(Debug, serde::Deserialize)]
pub struct BatchUploadItem {
    #[serde(rename = "type")]
    pub item_type: String,
    pub path: String,
}

/// Batch upload multiple files and folders
#[tauri::command]
pub async fn vfs_batch_upload(
    source_id: String,
    items: Vec<BatchUploadItem>,
    s3_base_path: Option<String>,
    part_size: Option<u64>,
    _state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    use tracing::{info, warn, error};
    
    if source_id.is_empty() {
        return Err("sourceId is required".to_string());
    }
    
    if items.is_empty() {
        return Err("items array cannot be empty".to_string());
    }
    
    let s3_base_path = s3_base_path.as_deref().unwrap_or("");
    let _part_size = part_size;
    
    info!(
        "[vfs_batch_upload] Starting batch upload: source_id={}, items={}, s3_base_path={}",
        source_id,
        items.len(),
        s3_base_path
    );
    
    // Collect all files and folders for the operation
    let mut operation_files = Vec::new();
    let mut source_path_summary = String::new();
    
    for item in items.iter() {
        match item.item_type.as_str() {
            "file" => {
                let file_name = Path::new(&item.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                
                let remote_path = if s3_base_path.is_empty() {
                    file_name.to_string()
                } else {
                    format!("{}/{}", s3_base_path.trim_end_matches('/'), file_name)
                };
                
                // Get file size
                let file_size = Path::new(&item.path)
                    .metadata()
                    .map(|m| m.len())
                    .unwrap_or(0);
                
                operation_files.push(OperationFile {
                    local_path: item.path.clone(),
                    remote_path: remote_path.clone(),
                    file_size,
                    bytes_processed: 0,
                    status: Some(crate::vfs::operation_tracker::OperationStatus::Pending),
                    error: None,
                });
                
                if source_path_summary.is_empty() {
                    source_path_summary = format!("{} file(s)", items.len());
                }
                
                info!("[vfs_batch_upload] Added file to operation: {} -> {} ({} bytes)", item.path, remote_path, file_size);
            }
            "folder" => {
                // For folders, we'll need to calculate total size recursively
                // For now, mark as folder with 0 size (will be updated as files are processed)
                let folder_name = Path::new(&item.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                
                let remote_path = if s3_base_path.is_empty() {
                    format!("{}/", folder_name)
                } else {
                    format!("{}/{}/", s3_base_path.trim_end_matches('/'), folder_name)
                };
                
                operation_files.push(OperationFile {
                    local_path: item.path.clone(),
                    remote_path: remote_path.clone(),
                    file_size: 0, // Will be calculated as files are uploaded
                    bytes_processed: 0,
                    status: Some(crate::vfs::operation_tracker::OperationStatus::Pending),
                    error: None,
                });
                
                if source_path_summary.is_empty() {
                    source_path_summary = format!("{} folder(s)", items.len());
                }
                
                info!("[vfs_batch_upload] Added folder to operation: {} -> {}", item.path, remote_path);
            }
            _ => {
                warn!("[vfs_batch_upload] Unknown item type: {} for path: {}", item.item_type, item.path);
            }
        }
    }
    
    // Create a single operation for all files/folders
    let operation_tracker = get_operation_tracker();
    let operation_id = operation_tracker.create_multi_file_operation(
        OperationType::Upload,
        source_id.clone(),
        source_path_summary.clone(),
        Some(s3_base_path.to_string()),
        Some(operation_files.clone()),
    );
    
    info!(
        "[vfs_batch_upload] Created operation {} with {} file(s)/folder(s)",
        operation_id,
        operation_files.len()
    );
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"location":"uploads.rs:213","message":"vfs_batch_upload entry","data":{{"sourceId":"{}","itemCount":{},"s3BasePath":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
            source_id, items.len(), s3_base_path,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    // Get the service to access source information
    let service = _state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Get source to check if it's S3 and get config
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"location":"uploads.rs:225","message":"source retrieved","data":{{"sourceId":"{}","sourceType":"{:?}","bucket":"{}","region":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
            source_id, source.source_type, source.config.path_or_bucket, source.config.region.as_ref().unwrap_or(&"N/A".to_string()),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    // Check if this is S3 storage
    use crate::vfs::domain::StorageSourceType;
    if !matches!(source.source_type, StorageSourceType::S3 | StorageSourceType::S3Compatible) {
        return Err(format!("Batch upload is only supported for S3 storage. Source {} is {:?}", source_id, source.source_type));
    }
    
    // Get S3 config
    let bucket = source.config.path_or_bucket.clone();
    let region = source.config.region.clone()
        .unwrap_or_else(|| "us-east-1".to_string());
    let endpoint = source.config.endpoint.clone();
    let name = source.name.clone();
    
    // Read credentials from environment (never from config)
    let access_key = std::env::var("AWS_ACCESS_KEY_ID").ok()
        .or_else(|| std::env::var("aws_access_key_id").ok());
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok()
        .or_else(|| std::env::var("aws_secret_access_key").ok());
    let session_token = std::env::var("AWS_SESSION_TOKEN").ok()
        .or_else(|| std::env::var("aws_session_token").ok());
    
    // Warn if credentials are missing (but don't fail - OpenDAL will try other methods)
    if access_key.is_none() && secret_key.is_none() {
        warn!("[vfs_batch_upload] AWS credentials not found in environment variables. Set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY, or configure credentials in storage settings.");
    }
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let has_access_key = access_key.is_some();
        let has_secret_key = secret_key.is_some();
        let has_session_token = session_token.is_some();
        let _ = writeln!(file, r#"{{"location":"uploads.rs:240","message":"credentials check","data":{{"hasAccessKey":{},"hasSecretKey":{},"hasSessionToken":{}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
            has_access_key, has_secret_key, has_session_token,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    // Create temporary S3 adapter to get operator
    use crate::vfs::adapters::S3StorageAdapter;
    let s3_adapter = S3StorageAdapter::new(
        bucket.clone(),
        region.clone(),
        access_key,
        secret_key,
        session_token,
        endpoint,
        name,
    ).await.map_err(|e| {
        // #region agent log
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
            use std::io::Write;
            let _ = writeln!(file, r#"{{"location":"uploads.rs:255","message":"S3 adapter creation failed","data":{{"error":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
                e,
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        }
        // #endregion
        format!("Failed to create S3 adapter: {}", e)
    })?;
    
    let operator = s3_adapter.operator().clone();
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"location":"uploads.rs:265","message":"S3 adapter created","data":{{"bucket":"{}","region":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"B"}}"#, 
            bucket, region,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    // Get upload manager
    use super::helpers::get_upload_manager;
    let upload_manager = get_upload_manager();
    
    // Collect all files to upload (including files from folders)
    let mut files_to_upload: Vec<(String, String)> = Vec::new(); // (local_path, remote_path)
    
    for item in items.iter() {
        match item.item_type.as_str() {
            "file" => {
                let file_name = Path::new(&item.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                
                let remote_path = if s3_base_path.is_empty() {
                    file_name.to_string()
                } else {
                    format!("{}/{}", s3_base_path.trim_end_matches('/'), file_name)
                };
                
                files_to_upload.push((item.path.clone(), remote_path));
            }
            "folder" => {
                // Recursively walk folder and collect all files
                let folder_name = Path::new(&item.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                
                let base_remote_path = if s3_base_path.is_empty() {
                    format!("{}/", folder_name)
                } else {
                    format!("{}/{}/", s3_base_path.trim_end_matches('/'), folder_name)
                };
                
                // Walk directory recursively
                let mut stack = vec![item.path.clone()];
                while let Some(current_path) = stack.pop() {
                    let path = Path::new(&current_path);
                    if !path.exists() {
                        warn!("[vfs_batch_upload] Path does not exist: {}", current_path);
                        continue;
                    }
                    
                    if path.is_file() {
                        // Calculate relative path from folder root
                        let relative = match path.strip_prefix(&item.path) {
                            Ok(rel) => rel,
                            Err(e) => {
                                warn!("[vfs_batch_upload] Failed to calculate relative path for {}: {}", current_path, e);
                                continue;
                            }
                        };
                        
                        let remote_path = if relative == Path::new("") {
                            // File is at root of folder
                            format!("{}{}", base_remote_path, path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown"))
                        } else {
                            // File is in subdirectory
                            format!("{}{}", base_remote_path, relative.to_string_lossy().replace('\\', "/"))
                        };
                        
                        files_to_upload.push((current_path, remote_path));
                    } else if path.is_dir() {
                        // Add subdirectories to stack
                        match std::fs::read_dir(path) {
                            Ok(entries) => {
                                for entry in entries.flatten() {
                                    let entry_path = entry.path();
                                    stack.push(entry_path.to_string_lossy().to_string());
                                }
                            }
                            Err(e) => {
                                warn!("[vfs_batch_upload] Failed to read directory {}: {}", current_path, e);
                            }
                        }
                    }
                }
            }
            _ => {
                warn!("[vfs_batch_upload] Unknown item type: {} for path: {}", item.item_type, item.path);
            }
        }
    }
    
    info!("[vfs_batch_upload] Collected {} files to upload", files_to_upload.len());
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let file_names: Vec<String> = files_to_upload.iter().take(5).map(|(_, r)| r.clone()).collect();
        let _ = writeln!(file, r#"{{"location":"uploads.rs:336","message":"files collected","data":{{"fileCount":{},"firstFiles":{:?}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"B"}}"#, 
            files_to_upload.len(), file_names,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    // Start uploads for each file
    for (local_path, remote_path) in files_to_upload {
        let local_path_buf = Path::new(&local_path);
        
        // Normalize S3 key (remove leading slash if present)
        let s3_key = remote_path.trim_start_matches('/');
        
        // Clone operator for each upload task
        let operator_clone = operator.clone();
        let source_id_clone = source_id.clone();
        let operation_id_clone = operation_id.clone();
        let local_path_clone = local_path.clone();
        
        // #region agent log
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
            use std::io::Write;
            let _ = writeln!(file, r#"{{"location":"uploads.rs:355","message":"starting upload","data":{{"localPath":"{}","s3Key":"{}","operationId":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"B"}}"#, 
                local_path, s3_key, operation_id_clone,
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        }
        // #endregion
        
        match upload_manager.start_upload_with_operation_id(
            &operator_clone,
            &source_id_clone,
            local_path_buf,
            s3_key,
            part_size,
            Some(operation_id_clone.clone()),
        ).await {
            Ok(upload_id) => {
                info!("[vfs_batch_upload] Started upload: {} -> {} (upload_id: {})", local_path, s3_key, upload_id);
                
                // #region agent log
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
                    use std::io::Write;
                    let _ = writeln!(file, r#"{{"location":"uploads.rs:368","message":"upload started","data":{{"uploadId":"{}","localPath":"{}","s3Key":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"B"}}"#, 
                        upload_id, local_path, s3_key,
                        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                }
                // #endregion
                
                // Start the actual upload in background
                let operator_upload = operator.clone();
                let upload_id_clone = upload_id.clone();
                let local_path_error = local_path_clone.clone();
                tokio::spawn(async move {
                    // #region agent log
                    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
                        use std::io::Write;
                        let _ = writeln!(file, r#"{{"location":"uploads.rs:378","message":"upload_chunks starting","data":{{"uploadId":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"B"}}"#, 
                            upload_id_clone,
                            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                    }
                    // #endregion
                    
                    let upload_manager_ref = get_upload_manager();
                    if let Err(e) = upload_manager_ref.upload_chunks(&operator_upload, &upload_id_clone).await {
                        error!("[vfs_batch_upload] Upload failed for {}: {}", upload_id_clone, e);
                        
                        // #region agent log
                        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
                            use std::io::Write;
                            let _ = writeln!(file, r#"{{"location":"uploads.rs:388","message":"upload_chunks failed","data":{{"uploadId":"{}","error":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
                                upload_id_clone, e,
                                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                        }
                        // #endregion
                        
                        // Extract user-friendly error message
                        let error_msg = format!("{}", e);
                        let user_friendly_error = if error_msg.contains("InvalidAccessKeyId") || error_msg.contains("does not exist in our records") {
                            "AWS credentials are invalid or missing. Please check your Access Key ID and Secret Access Key in storage settings."
                        } else if error_msg.contains("PermissionDenied") || error_msg.contains("403") {
                            "Permission denied. Check AWS credentials and IAM permissions (s3:PutObject)."
                        } else if error_msg.contains("AccessDenied") {
                            "Access denied. Your AWS credentials don't have permission to upload to this bucket."
                        } else {
                            &error_msg
                        };
                        
                        // Update operation tracker to mark this file as failed with error message
                        let tracker = get_operation_tracker();
                        let _ = tracker.update_file_progress_with_error(
                            &operation_id_clone,
                            &local_path_error,
                            user_friendly_error.to_string(),
                        );
                        
                        // Check if all files have failed - if so, mark entire operation as failed
                        // Otherwise, the operation stays InProgress until all files complete or fail
                        if let Ok(op) = tracker.get_operation(&operation_id_clone) {
                            if let Some(files) = &op.files {
                                let all_failed = files.iter().all(|f| {
                                    matches!(f.status, Some(crate::vfs::operation_tracker::OperationStatus::Failed))
                                });
                                if all_failed && !files.is_empty() {
                                    let _ = tracker.fail_operation(&operation_id_clone, 
                                        format!("All {} file(s) failed: {}", files.len(), user_friendly_error));
                                }
                            } else {
                                // Single file operation - mark as failed
                                let _ = tracker.fail_operation(&operation_id_clone, user_friendly_error.to_string());
                            }
                        }
                    } else {
                        // #region agent log
                        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
                            use std::io::Write;
                            let _ = writeln!(file, r#"{{"location":"uploads.rs:402","message":"upload_chunks completed","data":{{"uploadId":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"B"}}"#, 
                                upload_id_clone,
                                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                        }
                        // #endregion
                    }
                });
            }
            Err(e) => {
                error!("[vfs_batch_upload] Failed to start upload for {}: {}", local_path, e);
                
                // #region agent log
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
                    use std::io::Write;
                    let _ = writeln!(file, r#"{{"location":"uploads.rs:415","message":"start_upload failed","data":{{"localPath":"{}","error":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"A"}}"#, 
                        local_path, e,
                        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
                }
                // #endregion
                
                // Update operation tracker to mark this file as failed
                let tracker = get_operation_tracker();
                let _ = tracker.update_file_progress(
                    &operation_id,
                    &local_path,
                    0,
                    Some(crate::vfs::operation_tracker::OperationStatus::Failed),
                );
            }
        }
    }
    
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/tony/ursly/.cursor/debug.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"location":"uploads.rs:432","message":"vfs_batch_upload completed","data":{{"operationId":"{}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"B"}}"#, 
            operation_id,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
    // #endregion
    
    Ok(operation_id)
}

/// Check if a path is a directory
#[tauri::command]
pub async fn vfs_is_directory(
    path: String,
) -> Result<bool, String> {
    use std::path::Path;
    Ok(Path::new(&path).is_dir())
}

/// Get upload progress
#[tauri::command]
pub async fn vfs_get_upload_progress(
    _upload_id: String,
) -> Result<Option<serde_json::Value>, String> {
    // Return None - no progress data
    Ok(None)
}

/// Remove a completed upload from tracking
#[tauri::command]
pub async fn vfs_remove_upload(
    _upload_id: String,
) -> Result<(), String> {
    Ok(())
}

/// List all active downloads
#[tauri::command]
pub async fn vfs_list_downloads() -> Result<Vec<serde_json::Value>, String> {
    use super::helpers::get_download_manager;
    use serde_json::json;
    
    let download_manager = get_download_manager();
    download_manager.load_states().await.ok(); // Load persisted states
    
    let downloads = download_manager.list_downloads().await;
    
    // Convert downloads to JSON format expected by frontend
    let json_downloads: Vec<serde_json::Value> = downloads.into_iter().map(|state| {
        json!({
            "download_id": state.download_id,
            "operation_id": state.operation_id,
            "source_id": state.source_id,
            "remote_path": state.remote_path,
            "local_path": state.local_path.to_string_lossy(),
            "total_size": state.total_size,
            "bytes_downloaded": state.bytes_downloaded,
            "current_chunk": state.current_chunk,
            "total_chunks": state.total_chunks,
            "status": match state.status {
                crate::vfs::download_manager::DownloadStatus::Pending => "Pending",
                crate::vfs::download_manager::DownloadStatus::InProgress => "InProgress",
                crate::vfs::download_manager::DownloadStatus::Completed => "Completed",
                crate::vfs::download_manager::DownloadStatus::Failed => "Failed",
                crate::vfs::download_manager::DownloadStatus::Paused => "Paused",
            },
            "error": state.error,
            "created_at": state.created_at.map(|dt| dt.timestamp()),
            "completed_at": state.completed_at.map(|dt| dt.timestamp()),
            "last_updated_at": state.last_updated_at.map(|dt| dt.timestamp()),
        })
    }).collect();
    
    Ok(json_downloads)
}

/// Get download progress
#[tauri::command]
pub async fn vfs_get_download_progress(
    operation_id: String,
) -> Result<Option<serde_json::Value>, String> {
    use super::helpers::get_operation_tracker;
    use serde_json::json;
    use crate::vfs::operation_tracker::{OperationType, OperationStatus};
    
    let tracker = get_operation_tracker();
    
    // Get the operation
    let operation = match tracker.get_operation(&operation_id) {
        Ok(op) => op,
        Err(_) => return Ok(None), // Operation not found
    };
    
    // Only return progress for Download operations
    if operation.operation_type != OperationType::Download {
        return Ok(None);
    }
    
    // Calculate percentage
    let percentage = if let Some(file_size) = operation.file_size {
        if file_size > 0 {
            ((operation.bytes_processed as f64 / file_size as f64) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    } else {
        0.0
    };
    
    // Map OperationStatus to string format expected by frontend
    let status_str = match operation.status {
        OperationStatus::Pending => "Pending",
        OperationStatus::InProgress => "InProgress",
        OperationStatus::Completed => "Completed",
        OperationStatus::Failed => "Failed",
        OperationStatus::Canceled => "Canceled",
    };
    
    // Calculate speed and ETA if we have timing information
    let (speed_bytes_per_sec, estimated_time_remaining_sec) = if let (Some(created_at), Some(last_updated)) = (operation.created_at, operation.last_updated_at) {
        let elapsed_secs = (last_updated - created_at).num_seconds().max(1) as f64;
        if elapsed_secs > 0.0 && operation.bytes_processed > 0 {
            let speed = operation.bytes_processed as f64 / elapsed_secs;
            if let Some(file_size) = operation.file_size {
                let remaining_bytes = file_size.saturating_sub(operation.bytes_processed);
                let eta = if speed > 0.0 {
                    (remaining_bytes as f64 / speed).max(0.0)
                } else {
                    0.0
                };
                (Some(speed as u64), Some(eta as u32))
            } else {
                (Some(speed as u64), None)
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    
    // Build the response
    let progress_data = json!({
        "operation_id": operation.operation_id,
        "source_path": operation.source_path,
        "destination_path": operation.destination_path,
        "bytes_processed": operation.bytes_processed,
        "file_size": operation.file_size,
        "percentage": percentage,
        "status": status_str,
        "speed_bytes_per_sec": speed_bytes_per_sec,
        "estimated_time_remaining_sec": estimated_time_remaining_sec,
        "error": operation.error,
    });
    
    Ok(Some(progress_data))
}

/// Cleanup old uploads
#[tauri::command]
pub async fn vfs_cleanup_old_uploads() -> Result<(), String> {
    Ok(())
}

/// Get user audit history
#[tauri::command]
pub async fn vfs_get_audit_history(
    limit: Option<u32>,
) -> Result<Vec<serde_json::Value>, String> {
    use super::helpers::get_operation_tracker;
    use serde_json::json;

    let tracker = get_operation_tracker();
    let operations = tracker.get_audit_history(limit.map(|l| l as usize))
        .map_err(|e| format!("Failed to get audit history: {}", e))?;

    // Convert operations to JSON values (same format as vfs_list_operations)
    let json_operations: Vec<serde_json::Value> = operations.into_iter().map(|op| {
        json!({
            "operation_id": op.operation_id,
            "operation_type": format!("{:?}", op.operation_type),
            "source_id": op.source_id,
            "source_path": op.source_path,
            "destination_path": op.destination_path,
            "file_size": op.file_size,
            "bytes_processed": op.bytes_processed,
            "status": format!("{:?}", op.status),
            "error": op.error,
            "files": op.files.map(|files| files.into_iter().map(|f| json!({
                "local_path": f.local_path,
                "remote_path": f.remote_path,
                "file_size": f.file_size,
                "bytes_processed": f.bytes_processed,
                "status": f.status.map(|s| format!("{:?}", s)),
                "error": f.error,
            })).collect::<Vec<_>>()),
            "file_count": op.file_count,
            "created_at": op.created_at.map(|dt| dt.timestamp()),
            "completed_at": op.completed_at.map(|dt| dt.timestamp()),
            "last_updated_at": op.last_updated_at.map(|dt| dt.timestamp()),
        })
    }).collect();

    Ok(json_operations)
}

/// Get organization audit history
#[tauri::command]
pub async fn vfs_get_organization_audit(
    _limit: Option<u32>,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![])
}

/// Get operation status
#[tauri::command]
pub async fn vfs_get_operation_status(
    _operation_id: String,
) -> Result<Option<serde_json::Value>, String> {
    Ok(None)
}

/// Resume a paused download
#[tauri::command]
pub async fn vfs_resume_download(
    download_id: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use super::helpers::get_download_manager;
    use crate::vfs::adapters::S3StorageAdapter;
    use tracing::{info, warn};
    
    let download_manager = get_download_manager();
    
    // Get download state to find source_id
    let download_state = download_manager.list_downloads().await
        .into_iter()
        .find(|d| d.download_id == download_id)
        .ok_or_else(|| format!("Download not found: {}", download_id))?;
    
    let source_id = download_state.source_id.clone();
    
    // Get service to access source information
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Get source to get config
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Check if this is S3 storage
    use crate::vfs::domain::StorageSourceType;
    if !matches!(source.source_type, StorageSourceType::S3 | StorageSourceType::S3Compatible) {
        return Err(format!("Resume download is only supported for S3 storage. Source {} is {:?}", source_id, source.source_type));
    }
    
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
    
    // Warn if credentials are missing (but don't fail - OpenDAL will try other methods)
    if access_key.is_none() && secret_key.is_none() {
        warn!("[vfs_resume_download] AWS credentials not found in environment variables. Set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY, or configure credentials in storage settings.");
    }
    
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
    
    info!("[vfs_resume_download] Resuming download: {}", download_id);
    
    download_manager.resume_download(&operator, &download_id).await
        .map_err(|e| format!("Failed to resume download: {}", e))?;
    
    Ok(())
}

/// Pause an active download
#[tauri::command]
pub async fn vfs_pause_download(
    download_id: String,
) -> Result<(), String> {
    use super::helpers::get_download_manager;
    use tracing::info;
    
    let download_manager = get_download_manager();
    
    info!("[vfs_pause_download] Pausing download: {}", download_id);
    
    download_manager.pause_download(&download_id).await
        .map_err(|e| format!("Failed to pause download: {}", e))?;
    
    Ok(())
}

/// Cancel a download
#[tauri::command]
pub async fn vfs_cancel_download(
    download_id: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use super::helpers::get_download_manager;
    use tracing::info;
    
    let download_manager = get_download_manager();
    
    info!("[vfs_cancel_download] Canceling download: {}", download_id);
    
    download_manager.cancel_download(&download_id).await
        .map_err(|e| format!("Failed to cancel download: {}", e))?;
    
    Ok(())
}

/// Remove a completed download from tracking
#[tauri::command]
pub async fn vfs_remove_download(
    _operation_id: String,
) -> Result<(), String> {
    Ok(())
}

/// Clear all audit history
#[tauri::command]
pub async fn vfs_clear_audit_history() -> Result<(), String> {
    Ok(())
}

/// Cancel/stop an active operation
#[tauri::command]
pub async fn vfs_cancel_operation(
    operation_id: String,
) -> Result<(), String> {
    use super::helpers::get_operation_tracker;
    
    let tracker = get_operation_tracker();
    
    // Get operation to check type
    let operation = tracker.get_operation(&operation_id)
        .map_err(|e| format!("Operation not found: {}", e))?;
    
    // Cancel uploads if this is an upload operation
    if matches!(operation.operation_type, crate::vfs::operation_tracker::OperationType::Upload) {
        use crate::vfs::commands::helpers::get_upload_manager;
        
        let upload_manager = get_upload_manager();
        let uploads = upload_manager.list_uploads().await;
        
        // Remove all uploads for this operation from tracking
        for upload in uploads {
            if upload.operation_id.as_ref() == Some(&operation_id) {
                if let Err(e) = upload_manager.remove_upload(&upload.upload_id).await {
                    tracing::warn!("Failed to remove upload {}: {}", upload.upload_id, e);
                }
            }
        }
    }
    
    // Mark operation as canceled
    tracker.cancel_operation(&operation_id)
        .map_err(|e| format!("Failed to cancel operation: {}", e))?;
    
    Ok(())
}

/// Restart a failed or canceled operation
/// Returns the original operation details so frontend can re-execute it
#[tauri::command]
pub async fn vfs_restart_operation(
    operation_id: String,
) -> Result<serde_json::Value, String> {
    use super::helpers::get_operation_tracker;
    use serde_json::json;
    
    let tracker = get_operation_tracker();
    
    // Get the original operation
    let original_op = tracker.get_operation(&operation_id)
        .map_err(|e| format!("Operation not found: {}", e))?;
    
    // Only allow restarting failed or canceled operations
    if !matches!(original_op.status, crate::vfs::operation_tracker::OperationStatus::Failed | crate::vfs::operation_tracker::OperationStatus::Canceled) {
        return Err("Can only restart failed or canceled operations".to_string());
    }
    
    // For uploads, try to resume if possible
    if matches!(original_op.operation_type, crate::vfs::operation_tracker::OperationType::Upload) {
        use crate::vfs::commands::helpers::get_upload_manager;
        use crate::vfs::multipart_upload::MultipartUploadState;
        
        let upload_manager = get_upload_manager();
        let uploads: Vec<MultipartUploadState> = upload_manager.list_uploads().await;
        
        // Find upload for this operation
        if let Some(upload_state) = uploads.iter().find(|u| u.operation_id.as_ref() == Some(&operation_id)) {
            // Return upload details for resume
            return Ok(json!({
                "operation_id": operation_id,
                "operation_type": format!("{:?}", original_op.operation_type),
                "can_resume": true,
                "upload_id": upload_state.upload_id,
                "source_id": original_op.source_id,
                "source_path": original_op.source_path,
                "destination_path": original_op.destination_path,
                "file_size": original_op.file_size,
                "files": original_op.files,
            }));
        }
    }
    
    // Return operation details for frontend to re-execute
    Ok(json!({
        "operation_id": operation_id,
        "operation_type": format!("{:?}", original_op.operation_type),
        "can_resume": false,
        "source_id": original_op.source_id,
        "source_path": original_op.source_path,
        "destination_path": original_op.destination_path,
        "file_size": original_op.file_size,
        "files": original_op.files,
    }))
}

/// Delete an operation from tracking
#[tauri::command]
pub async fn vfs_delete_operation(
    operation_id: String,
) -> Result<(), String> {
    use super::helpers::get_operation_tracker;
    
    let tracker = get_operation_tracker();
    tracker.delete_operation(&operation_id)
        .map_err(|e| format!("Failed to delete operation: {}", e))?;
    
    Ok(())
}

/// Clear all operations (for reset/restart)
#[tauri::command]
pub async fn vfs_clear_all_operations() -> Result<(), String> {
    use super::helpers::get_operation_tracker;
    use crate::vfs::commands::helpers::get_upload_manager;
    
    // Clear operations tracker
    let tracker = get_operation_tracker();
    tracker.clear_all()
        .map_err(|e| format!("Failed to clear operations: {}", e))?;
    
    // Clear multipart uploads
    let upload_manager = get_upload_manager();
    let uploads = upload_manager.list_uploads().await;
    for upload in uploads {
        if let Err(e) = upload_manager.remove_upload(&upload.upload_id).await {
            tracing::warn!("Failed to remove upload {}: {}", upload.upload_id, e);
        }
    }
    
    Ok(())
}

/// Get user audit log
#[tauri::command]
pub async fn vfs_get_user_audit_log(
    _limit: Option<u32>,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![])
}

/// Get organization audit log
#[tauri::command]
pub async fn vfs_get_organization_audit_log(
    _limit: Option<u32>,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![])
}

/// Get all audit log entries
#[tauri::command]
pub async fn vfs_get_all_audit_log(
    _limit: Option<u32>,
) -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![])
}
