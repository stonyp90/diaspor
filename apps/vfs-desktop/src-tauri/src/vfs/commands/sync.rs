//! Sync and Tier Management Commands
//!
//! Commands for syncing files and managing storage tiers

use tauri::State;
use super::state::VfsStateWrapper;

#[tauri::command]
pub async fn vfs_sync(
    _source_id: String,
    _path: String,
    _target_source_id: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    // Stub implementation - sync not yet fully implemented
    Ok(())
}

#[tauri::command]
pub async fn vfs_get_sync_targets(
    _state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    // Stub implementation - return empty list
    Ok(vec![])
}

#[tauri::command]
pub async fn vfs_change_tier(
    source_id: String,
    path: String,
    tier: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    // Delegate to vfs_sync_to_tier for consistency
    let result = vfs_sync_to_tier(
        source_id.clone(),
        vec![path],
        tier,
        None, // No target source for same-source tier changes
        state,
    ).await?;
    
    // Check if operation succeeded
    let files_failed = result.get("files_failed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    
    if files_failed > 0 {
        let errors = result.get("errors")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_else(|| "Unknown error".to_string());
        
        return Err(format!("Failed to change tier: {}", errors));
    }
    
    Ok(())
}

#[tauri::command]
pub async fn vfs_sync_to_tier(
    source_id: String,
    paths: Vec<String>,
    target_tier: String,
    target_source_id: Option<String>,
    state: State<'_, VfsStateWrapper>,
) -> Result<serde_json::Value, String> {
    use std::path::Path;
    use tracing::{info, warn, error};
    use serde_json::json;
    use crate::vfs::domain::StorageTier;
    use crate::vfs::operation_tracking::OperationTrackingHelper;
    use crate::vfs::operation_tracker::OperationType;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Parse target tier string to StorageTier enum
    let tier = match target_tier.as_str() {
        "hot" => StorageTier::Hot,
        "warm" => StorageTier::Warm,
        "cold" => StorageTier::Cold,
        "nearline" => StorageTier::Nearline,
        "archive" => StorageTier::Archive,
        "instant-retrieval" => StorageTier::InstantRetrieval,
        _ => return Err(format!("Invalid tier: {}", target_tier)),
    };
    
    info!("[vfs_sync_to_tier] Moving {} files from {} to tier {:?} (target_source: {:?})", 
        paths.len(), source_id, tier, target_source_id);
    
    let mut files_synced = 0;
    let mut files_failed = 0;
    let mut errors = Vec::new();
    let mut operation_ids = Vec::new(); // Collect operation IDs for frontend tracking
    
    // Check if this is a cross-source move (local to cloud) or same-source tier change
    if let Some(ref target_source_id) = target_source_id {
        // Cross-source move: copy files from source to target source
        let _target_source = service.get_source(target_source_id)
            .ok_or_else(|| format!("Target storage source not found: {}", target_source_id))?;
        
        info!("[vfs_sync_to_tier] Cross-source move: {} -> {}", source_id, target_source_id);
        
        for path_str in &paths {
            let path = Path::new(path_str);
            
            // Get file size for tracking
            let file_size = service.stat(&source_id, path).await.ok().map(|stat| stat.size);
            
            // For cross-source tier moves, destination should be root path (/)
            // This ensures directories are copied correctly without creating nested paths
            // The copy_to_source method will append the directory/file name to the destination
            let dest_path = Path::new("/");
            
            let operation_id = OperationTrackingHelper::track_operation_start(
                OperationType::Copy,
                source_id.clone(),
                path_str.clone(),
                Some(format!("{}:{}", target_source_id, dest_path.display())),
                file_size,
            );
            
            operation_ids.push(operation_id.clone());
            
            match service.copy_to_source(&source_id, path, target_source_id, dest_path).await {
                Ok(_bytes_copied) => {
                    files_synced += 1;
                    OperationTrackingHelper::complete_operation(&operation_id)
                        .unwrap_or_else(|e| error!("Failed to complete operation: {}", e));
                }
                Err(e) => {
                    files_failed += 1;
                    let error_msg = format!("{}: {}", path_str, e);
                    errors.push(error_msg.clone());
                    OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                        .unwrap_or_else(|err| error!("Failed to fail operation: {}", err));
                    error!("[vfs_sync_to_tier] Failed to copy {}: {}", path_str, e);
                }
            }
        }
    } else {
        // Same-source tier change: change storage class for object storage
        match source.source_type.category() {
            crate::vfs::domain::StorageCategory::Cloud => {
                // For object storage tier changes, we need to recreate the adapter
                // to get access to the operator (this is a limitation of the current architecture)
                
                // Try to get operator from S3StorageAdapter
                use crate::vfs::adapters::S3StorageAdapter;
                use crate::vfs::adapters::object_storage_tiering::change_object_storage_tier;
                
                // We need to downcast to get the operator - for now, use a helper method
                // Check if it's S3 storage
                if matches!(source.source_type, crate::vfs::domain::StorageSourceType::S3 | 
                    crate::vfs::domain::StorageSourceType::S3Compatible) {
                    // For S3, we need to recreate the adapter to get the operator
                    // This is a limitation - ideally we'd store the operator separately
                    warn!("[vfs_sync_to_tier] S3 tier change requires recreating adapter - this is a limitation");
                    
                    // Get S3 config from source
                    let bucket = source.config.path_or_bucket.clone();
                    let region = source.config.region.clone()
                        .unwrap_or_else(|| "us-east-1".to_string());
                    let endpoint = source.config.endpoint.clone();
                    
                    // Read credentials from environment (never from config)
                    let access_key = std::env::var("AWS_ACCESS_KEY_ID").ok();
                    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
                    let session_token = std::env::var("AWS_SESSION_TOKEN").ok();
                    
                    // Create temporary S3 adapter to get operator
                    let s3_adapter = S3StorageAdapter::new(
                        bucket.clone(),
                        region.clone(),
                        access_key,
                        secret_key,
                        session_token,
                        endpoint,
                        source.name.clone(),
                    ).await.map_err(|e| format!("Failed to create S3 adapter: {}", e))?;
                    
                    let operator = s3_adapter.operator();
                    
                    // Change tier for each file
                    for path_str in &paths {
                        let key = path_str.trim_start_matches('/');
                        
                        // Get file size for tracking
                        let file_size = service.stat(&source_id, Path::new(path_str)).await.ok().map(|stat| stat.size);
                        
                        let operation_id = OperationTrackingHelper::track_operation_start(
                            OperationType::Copy, // Using Copy as tier change involves copy
                            source_id.clone(),
                            path_str.clone(),
                            Some(path_str.clone()),
                            file_size,
                        );
                        
                        operation_ids.push(operation_id.clone());
                        
                        match change_object_storage_tier(operator, &source.source_type, key, tier).await {
                            Ok(()) => {
                                files_synced += 1;
                                OperationTrackingHelper::complete_operation(&operation_id)
                                    .unwrap_or_else(|e| error!("Failed to complete operation: {}", e));
                            }
                            Err(e) => {
                                files_failed += 1;
                                let error_msg = format!("{}: {}", path_str, e);
                                errors.push(error_msg.clone());
                                OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                                    .unwrap_or_else(|err| error!("Failed to fail operation: {}", err));
                                error!("[vfs_sync_to_tier] Failed to change tier for {}: {}", path_str, e);
                            }
                        }
                    }
                } else {
                      return Err(format!("Tier changes for {:?} are not yet implemented. Only S3 is supported.", 
                        source.source_type));
                }
            }
            crate::vfs::domain::StorageCategory::Local => {
                return Err("Tier changes for local storage require a target cloud storage source. Please select a target source.".to_string());
            }
            _ => {
                return Err(format!("Tier changes are not supported for storage type: {:?}", source.source_type));
            }
        }
    }
    
    info!("[vfs_sync_to_tier] Completed: {} synced, {} failed", files_synced, files_failed);
    
    Ok(json!({
        "files_synced": files_synced,
        "files_failed": files_failed,
        "errors": errors,
        "operation_ids": operation_ids
    }))
}

#[tauri::command]
pub async fn vfs_get_tier_targets(
    source_id: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    use tracing::{info, warn};
    use serde_json::json;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    info!("[vfs_get_tier_targets] Getting tier targets for source: {} ({:?})", source_id, source.source_type);
    
    let mut targets = Vec::new();
    
    // Determine available tiers based on source type
    match source.source_type.category() {
        crate::vfs::domain::StorageCategory::Cloud => {
            // Object storage (S3, GCS, Azure Blob) supports tier transitions
            match source.source_type {
                crate::vfs::domain::StorageSourceType::S3 | 
                crate::vfs::domain::StorageSourceType::S3Compatible => {
                    // S3 supports: Nearline (STANDARD), Cold (GLACIER_IR)
                    // Note: Archive/Infrequent Access tier removed - not supported
                    // Note: Hot tier removed - it's reserved for ONTAP (FSx) which uses DataSync
                    targets.push(json!({
                        "tier": "nearline",
                        "tier_name": "Nearline",
                        "description": "Standard storage class - immediate access, standard cost",
                        "target_source_id": null,
                        "requires_target_source": false,
                        "provider_name": "AWS S3",
                        "storage_class": "STANDARD"
                    }));
                    targets.push(json!({
                        "tier": "cold",
                        "tier_name": "Cold",
                        "description": "Glacier Instant Retrieval - millisecond access, lower cost",
                        "target_source_id": null,
                        "requires_target_source": false,
                        "provider_name": "AWS S3",
                        "storage_class": "GLACIER_IR"
                    }));
                }
                crate::vfs::domain::StorageSourceType::Gcs => {
                    // GCS supports: Nearline (STANDARD), Cold (COLDLINE)
                    // Note: Archive/Infrequent Access tier removed - not supported
                    // Note: Hot tier removed - it's reserved for ONTAP (FSx) which uses DataSync
                    targets.push(json!({
                        "tier": "nearline",
                        "tier_name": "Nearline",
                        "description": "Standard storage class - immediate access, standard cost",
                        "target_source_id": null,
                        "requires_target_source": false,
                        "provider_name": "Google Cloud Storage",
                        "storage_class": "STANDARD"
                    }));
                    targets.push(json!({
                        "tier": "cold",
                        "tier_name": "Coldline",
                        "description": "Coldline storage - lower cost, immediate access",
                        "target_source_id": null,
                        "requires_target_source": false,
                        "provider_name": "Google Cloud Storage",
                        "storage_class": "COLDLINE"
                    }));
                }
                crate::vfs::domain::StorageSourceType::AzureBlob => {
                    // Azure Blob supports: Nearline (Hot), Cold (Archive)
                    // Note: Archive/Infrequent Access (Cool) tier removed - not supported
                    // Note: Hot tier removed - it's reserved for ONTAP (FSx) which uses DataSync
                    targets.push(json!({
                        "tier": "nearline",
                        "tier_name": "Nearline",
                        "description": "Hot tier - immediate access, standard cost",
                        "target_source_id": null,
                        "requires_target_source": false,
                        "provider_name": "Azure Blob Storage",
                        "storage_class": "Hot"
                    }));
                    targets.push(json!({
                        "tier": "cold",
                        "tier_name": "Cold",
                        "description": "Archive tier - lowest cost, requires restore",
                        "target_source_id": null,
                        "requires_target_source": false,
                        "provider_name": "Azure Blob Storage",
                        "storage_class": "Archive"
                    }));
                }
                _ => {
                    // Check if this is Oracle Object Storage (S3-compatible with oraclecloud.com endpoint)
                    // Oracle uses S3Compatible type but we can detect it by endpoint
                    if let Some(ref endpoint) = source.config.endpoint {
                        if endpoint.contains("oraclecloud.com") {
                            // Oracle Object Storage - same tiers as S3
                            // Note: Archive/Infrequent Access tier removed - not supported
                            targets.push(json!({
                                "tier": "nearline",
                                "tier_name": "Nearline",
                                "description": "Standard storage class - immediate access, standard cost",
                                "target_source_id": null,
                                "requires_target_source": false,
                                "provider_name": "Oracle Object Storage",
                                "storage_class": "STANDARD"
                            }));
                            targets.push(json!({
                                "tier": "cold",
                                "tier_name": "Cold",
                                "description": "Archive storage - lowest cost, requires restore",
                                "target_source_id": null,
                                "requires_target_source": false,
                                "provider_name": "Oracle Object Storage",
                                "storage_class": "ARCHIVE"
                            }));
                        } else {
                            warn!("[vfs_get_tier_targets] Unknown cloud storage type: {:?}", source.source_type);
                        }
                    } else {
                        warn!("[vfs_get_tier_targets] Unknown cloud storage type: {:?}", source.source_type);
                    }
                }
            }
        }
        crate::vfs::domain::StorageCategory::Local => {
            // Local storage - can move to cloud storage sources
            // List all cloud storage sources as potential targets
            let all_sources = service.list_sources();
            for target_source in all_sources {
                if target_source.source_type.category() == crate::vfs::domain::StorageCategory::Cloud {
                    let tier_name = match target_source.source_type {
                        crate::vfs::domain::StorageSourceType::S3 | 
                        crate::vfs::domain::StorageSourceType::S3Compatible => "S3",
                        crate::vfs::domain::StorageSourceType::Gcs => "GCS",
                        crate::vfs::domain::StorageSourceType::AzureBlob => "Azure Blob",
                        _ => "Cloud Storage",
                    };
                    
                    targets.push(json!({
                        "tier": "cold",
                        "tier_name": format!("Move to {}", target_source.name),
                        "description": format!("Move files to {} cloud storage", tier_name),
                        "target_source_id": target_source.id,
                        "requires_target_source": true,
                        "provider_name": tier_name,
                        "storage_class": null
                    }));
                }
            }
            
            // If no cloud sources available, add a message
            if targets.is_empty() {
                targets.push(json!({
                    "tier": "local",
                    "tier_name": "Local Storage Only",
                    "description": "No cloud storage sources available. Add a cloud storage source to enable tier transitions.",
                    "target_source_id": null,
                    "requires_target_source": false,
                    "provider_name": null,
                    "storage_class": null
                }));
            }
        }
        _ => {
            // Network/Block storage - tier transitions not supported
            warn!("[vfs_get_tier_targets] Tier transitions not supported for source type: {:?}", source.source_type);
        }
    }
    
    info!("[vfs_get_tier_targets] Returning {} tier targets for source {}", targets.len(), source_id);
    Ok(targets)
}

#[tauri::command]
pub async fn vfs_check_nvme_cache(
    _state: State<'_, VfsStateWrapper>,
) -> Result<serde_json::Value, String> {
    // Stub implementation - return empty cache info
    Ok(serde_json::json!({
        "available": false,
        "total_bytes": 0,
        "used_bytes": 0,
        "free_bytes": 0
    }))
}

#[tauri::command]
pub async fn vfs_set_tags(
    _source_id: String,
    _path: String,
    _tags: Vec<String>,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    // Stub implementation - tags not yet fully implemented
    Ok(())
}

#[tauri::command]
pub async fn vfs_reveal_in_finder(
    source_id: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use std::path::Path;
    use std::process::Command;
    use tracing::info;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    // Get the source to resolve the native path
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Only support local storage for reveal
    if source.source_type.category() != crate::vfs::domain::StorageCategory::Local {
        return Err("Reveal in Finder is only supported for local storage".to_string());
    }
    
    // Resolve VFS path to native path
    let native_path = {
        let vfs_path = Path::new(&path);
        let path_str = vfs_path.to_string_lossy();
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
    
    // Canonicalize the path to get absolute path
    let absolute_path = native_path.canonicalize()
        .map_err(|e| format!("Failed to resolve path: {}", e))?;
    
    info!("[vfs_reveal_in_finder] Revealing path: {:?} (source: {})", absolute_path, source_id);
    
    // Platform-specific reveal commands
    #[cfg(target_os = "macos")]
    {
        // macOS: open -R reveals the file in Finder
        let status = Command::new("open")
            .arg("-R")
            .arg(&absolute_path)
            .status()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
        
        if !status.success() {
            return Err(format!("Failed to reveal in Finder (exit code: {:?})", status.code()));
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        use crate::vfs::platform::CommandBuilder;
        // Windows: explorer /select,<path> reveals the file in Explorer
        let path_str = absolute_path.to_string_lossy().replace('/', "\\");
        let status = CommandBuilder::new("explorer")
            .arg(format!("/select,{}", path_str))
            .status()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
        
        if !status.success() {
            return Err(format!("Failed to reveal in Explorer (exit code: {:?})", status.code()));
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // Linux: xdg-open opens the parent directory in the default file manager
        let parent = absolute_path.parent()
            .ok_or_else(|| "Path has no parent directory".to_string())?;
        
        let status = Command::new("xdg-open")
            .arg(parent)
            .status()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
        
        if !status.success() {
            return Err(format!("Failed to reveal in file manager (exit code: {:?})", status.code()));
        }
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        return Err("Reveal in Finder is not supported on this platform".to_string());
    }
    
    Ok(())
}
