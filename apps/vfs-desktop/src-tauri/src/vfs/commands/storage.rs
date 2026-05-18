//! Storage Source Management Commands
//!
//! Commands for managing storage sources (add, remove, list, mount, eject)

use tauri::State;
use tracing::{error, info, warn};
use super::state::VfsStateWrapper;
use super::responses::VfsStorageSourceResponse;

#[tauri::command]
pub async fn vfs_list_sources(
    state: State<'_, VfsStateWrapper>,
) -> Result<Vec<VfsStorageSourceResponse>, String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let sources = service.list_sources();
    
    let responses: Vec<VfsStorageSourceResponse> = sources.into_iter().map(|source| {
        let category = source.source_type.category();
        let provider_id = source.source_type.to_provider_id().as_str().to_string();
        
        // Convert category to lowercase string (frontend expects lowercase)
        let category_str = match category {
            crate::vfs::domain::StorageCategory::Local => "local",
            crate::vfs::domain::StorageCategory::Cloud => "cloud",
            crate::vfs::domain::StorageCategory::Block => "block",
            crate::vfs::domain::StorageCategory::Network => "network",
            crate::vfs::domain::StorageCategory::Hybrid => "hybrid",
            crate::vfs::domain::StorageCategory::Custom => "custom",
        };
        
        // Determine path and bucket based on storage type
        // For cloud storage (S3, GCS, etc.), use bucket; for local/network, use path
        let (path, bucket) = match category {
            crate::vfs::domain::StorageCategory::Cloud => {
                // Cloud storage: bucket is the main identifier
                (None, Some(source.config.path_or_bucket.clone()))
            },
            _ => {
                // Local/network storage: path is the mount point or config path
                let path_value = source.mount_point
                    .map(|p| p.to_string_lossy().to_string())
                    .or_else(|| {
                        if !source.config.path_or_bucket.is_empty() {
                            Some(source.config.path_or_bucket.clone())
                        } else {
                            None
                        }
                    });
                (path_value, None)
            },
        };
        
        // For cloud storage sources (S3, GCS, Azure Blob, etc.), always show as Connected
        // unless there's an explicit error. This prevents false "disconnected" indicators
        // when the source is valid but hasn't been tested recently.
        let status_str = match category {
            crate::vfs::domain::StorageCategory::Cloud => {
                // Cloud storage: default to Connected unless explicitly Error
                match &source.status {
                    crate::vfs::domain::ConnectionStatus::Error(_) => format!("{:?}", source.status),
                    _ => "Connected".to_string(),
                }
            },
            _ => format!("{:?}", source.status),
        };

        VfsStorageSourceResponse {
            id: source.id.clone(),
            name: source.name.clone(),
            source_type: format!("{:?}", source.source_type),
            mounted: source.mounted,
            status: status_str,
            path,
            bucket,
            region: source.config.region.clone(),
            category: category_str.to_string(),
            provider_id: Some(provider_id),
            is_ejectable: false, // TODO: Detect ejectable volumes
            // CRITICAL: Only these exact 7 standard folder names are marked as system locations
            // Everything else (volumes, custom mounts, drive letters, DMGs, etc.) will be volumes
            // This ensures strict separation:
            //   - Locations = ONLY default user folders (Home, Desktop, Documents, Downloads, Pictures, Music, Videos)
            //   - Volumes = ALL other local storage sources (mounted drives, custom paths, etc.)
            // Case-sensitive exact match required - no partial matches or variations
            is_system_location: source.name == "Home" || 
                               source.name == "Desktop" || 
                               source.name == "Documents" ||
                               source.name == "Downloads" ||
                               source.name == "Pictures" ||
                               source.name == "Music" ||
                               source.name == "Videos",
        }
    }).collect();
    
    Ok(responses)
}

#[tauri::command]
pub async fn vfs_add_source(
    source: serde_json::Value,
    _state: State<'_, VfsStateWrapper>,
) -> Result<VfsStorageSourceResponse, String> {
    // Parse source config from JSON for the response
    let name = source.get("name")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'name' field")?
        .to_string();
    
    let provider_id = source.get("providerId")
        .and_then(|v| v.as_str())
        .unwrap_or("local")
        .to_string();
    
    let path_or_bucket = source.get("path")
        .or_else(|| source.get("bucket"))
        .and_then(|v| v.as_str())
        .unwrap_or("/")
        .to_string();
    
    let region = source.get("region")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let source_id = uuid::Uuid::new_v4().to_string();
    
    let category = match provider_id.as_str() {
        "local" => "local",
        "aws-s3" | "gcs" | "azure-blob" | "s3-compatible" => "cloud",
        _ => "network",
    }.to_string();
    
    let source_type = match provider_id.as_str() {
        "aws-s3" => "S3",
        "gcs" => "Gcs",
        "azure-blob" => "AzureBlob",
        "smb" | "nas" => "Nas",
        "nfs" => "Nfs",
        "sftp" => "Sftp",
        _ => "Local",
    }.to_string();
    
    info!("[vfs_add_source] Added source: {} ({})", name, source_id);
    
    // Return the source info - actual storage connection is handled by the UI
    // via the storage-specific dialogs that call register_storage_source
    Ok(VfsStorageSourceResponse {
        id: source_id,
        name,
        source_type,
        mounted: false,
        status: "connected".to_string(),
        path: Some(path_or_bucket),
        bucket: None,
        region,
        category,
        provider_id: Some(provider_id),
        is_ejectable: false,
        is_system_location: false,
    })
}

#[tauri::command]
pub async fn vfs_remove_source(
    source_id: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    info!("[vfs_remove_source] Removing source: {}", source_id);
    
    // Check if source exists before attempting removal
    if service.get_source(&source_id).is_none() {
        warn!("[vfs_remove_source] Source not found: {}", source_id);
        return Err(format!("Storage source not found: {}", source_id));
    }
    
    // Clean up metadata for this source before removing
    if let Ok(metadata_store) = super::helpers::get_metadata_store_instance().await {
        if let Err(e) = metadata_store.remove_source_metadata(&source_id).await {
            warn!("[vfs_remove_source] Failed to clean up metadata for source {}: {}", source_id, e);
            // Continue with removal even if metadata cleanup fails
        }
    }
    
    // Remove source from VfsService (async version also removes credentials)
    service.remove_source_async(&source_id).await
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Persist storage sources to disk
    if let Err(e) = service.save_sources().await {
        warn!("Failed to persist storage sources after removal: {}", e);
        // Continue anyway - don't fail the operation
    }
    
    info!("[vfs_remove_source] Successfully removed source: {}", source_id);
    Ok(())
}

#[tauri::command]
pub async fn vfs_refresh_s3_credentials(
    source_id: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    info!("[vfs_refresh_s3_credentials] Refreshing S3 credentials for source: {}", source_id);
    
    // Get the source to verify it exists
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Check if it's an S3 source
    if !matches!(source.source_type.category(), crate::vfs::domain::StorageCategory::Cloud) {
        return Err(format!("Source {} is not a cloud storage source", source_id));
    }
    
    // Refresh credentials by re-reading from environment variables
    // The S3 adapter reads credentials from environment variables on each operation
    // So we don't need to do anything special here - just log it
    info!("[vfs_refresh_s3_credentials] S3 credentials will be read from environment variables on next operation");
    
    Ok(())
}

#[tauri::command]
pub async fn vfs_update_s3_credentials(
    source_id: String,
    access_key_id: String,
    secret_access_key: String,
    session_token: Option<String>,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    info!("[vfs_update_s3_credentials] Updating S3 credentials for source: {}", source_id);
    
    service.update_s3_credentials(
        &source_id,
        access_key_id,
        secret_access_key,
        session_token,
    ).await
    .map_err(|e| format!("Failed to update S3 credentials: {}", e))?;
    
    info!("[vfs_update_s3_credentials] Successfully updated S3 credentials for source: {}", source_id);
    
    Ok(())
}

#[tauri::command]
pub async fn vfs_mount_local(
    name: String,
    path: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<VfsStorageSourceResponse, String> {
    use std::path::PathBuf;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let path_buf = PathBuf::from(&path);
    
    info!("[vfs_mount_local] Mounting local storage - name: {}, path: {:?}", name, path_buf);
    
    let source = service.add_local_source(name.clone(), path_buf.clone()).await
        .map_err(|e| format!("Failed to mount local storage: {}", e))?;
    
    // Convert to response
    Ok(VfsStorageSourceResponse {
        id: source.id.clone(),
        name: source.name.clone(),
        source_type: format!("{:?}", source.source_type),
        mounted: source.mounted,
        status: format!("{:?}", source.status),
        path: source.mount_point.map(|p| p.to_string_lossy().to_string()),
        bucket: None,
        region: None,
        category: "local".to_string(),
        provider_id: Some("local".to_string()),
        is_ejectable: false,
        is_system_location: false,
    })
}

#[tauri::command]
pub async fn vfs_eject(
    source_id: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    info!("[vfs_eject] Ejecting source: {}", source_id);
    
    // Get the source
    let _source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Remove the source (ejecting it)
    // Note: We don't check is_ejectable here as the frontend should handle that
    service.remove_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    info!("[vfs_eject] Successfully ejected source: {}", source_id);
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn vfs_register_s3_source(
    name: String,
    bucket: String,
    region: String,
    access_key: Option<String>,
    secret_key: Option<String>,
    session_token: Option<String>,
    endpoint: Option<String>,
    state: State<'_, VfsStateWrapper>,
) -> Result<VfsStorageSourceResponse, String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    info!("[vfs_register_s3_source] Registering S3 source - name: {}, bucket: {}, region: {}", name, bucket, region);
    
    // Set environment variables if credentials are provided
    if let Some(ref ak) = access_key {
        std::env::set_var("AWS_ACCESS_KEY_ID", ak);
    }
    if let Some(ref sk) = secret_key {
        std::env::set_var("AWS_SECRET_ACCESS_KEY", sk);
    }
    if let Some(ref st) = session_token {
        std::env::set_var("AWS_SESSION_TOKEN", st);
    }
    
    // Register the S3 source
    let source = service.add_s3_source(
        name.clone(),
        bucket.clone(),
        region.clone(),
        access_key.clone(),
        secret_key.clone(),
        session_token.clone(),
        endpoint.clone(),
    ).await
    .map_err(|e| format!("Failed to register S3 source: {}", e))?;
    
    // Persist storage sources to disk
    if let Err(e) = service.save_sources().await {
        warn!("Failed to persist storage sources: {}", e);
        // Continue anyway - don't fail the operation
    }
    
    // Convert to response
    Ok(VfsStorageSourceResponse {
        id: source.id.clone(),
        name: source.name.clone(),
        source_type: format!("{:?}", source.source_type),
        mounted: source.mounted,
        status: format!("{:?}", source.status),
        path: None,
        bucket: Some(bucket),
        region: Some(region),
        category: "cloud".to_string(),
        provider_id: Some("aws-s3".to_string()),
        is_ejectable: false,
        is_system_location: false,
    })
}

/// Test connection to an S3 storage source
#[tauri::command]
pub async fn vfs_test_s3_connection(
    source_id: String,
    state: State<'_, VfsStateWrapper>,
) -> Result<bool, String> {
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    info!("[vfs_test_s3_connection] Testing connection for source: {}", source_id);
    
    // Get the source
    let sources = service.list_sources();
    let source = sources.iter()
        .find(|s| s.id == source_id)
        .ok_or_else(|| format!("Source '{}' not found", source_id))?;
    
    // Only test S3 sources
    if !matches!(source.source_type, crate::vfs::domain::StorageSourceType::S3) {
        return Err(format!("Source '{}' is not an S3 source", source_id));
    }
    
    // Get the adapter and test connection
    let adapter = service.get_adapter(&source_id)
        .ok_or_else(|| format!("Source state for '{}' not found", source_id))?;
    
    match adapter.test_connection().await {
        Ok(true) => {
            info!("[vfs_test_s3_connection] ✅ Connection test successful for source '{}'", source_id);
            // Update source status to Connected
            if let Err(e) = service.update_source_status(&source_id, crate::vfs::domain::ConnectionStatus::Connected).await {
                warn!("[vfs_test_s3_connection] Failed to update source status: {}", e);
            }
            Ok(true)
        }
        Ok(false) => {
            warn!("[vfs_test_s3_connection] ❌ Connection test returned false for source '{}'", source_id);
            // Update source status to Error
            if let Err(e) = service.update_source_status(&source_id, crate::vfs::domain::ConnectionStatus::Error(
                "Connection test failed: Bucket is not accessible. Verify credentials and permissions.".to_string()
            )).await {
                warn!("[vfs_test_s3_connection] Failed to update source status: {}", e);
            }
            Ok(false)
        }
        Err(e) => {
            let error_msg = format!("Connection test error: {}", e);
            error!("[vfs_test_s3_connection] ❌ {}", error_msg);
            // Update source status to Error
            if let Err(update_err) = service.update_source_status(&source_id, crate::vfs::domain::ConnectionStatus::Error(error_msg.clone())).await {
                warn!("[vfs_test_s3_connection] Failed to update source status: {}", update_err);
            }
            Err(error_msg)
        }
    }
}
