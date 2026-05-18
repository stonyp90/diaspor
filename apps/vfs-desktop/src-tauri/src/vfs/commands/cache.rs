//! Cache Operations Commands
//!
//! Commands for managing cache (stats, clear, warm)

use tauri::{State, Emitter};
use super::state::VfsStateWrapper;
use super::responses::VfsCacheStatsResponse;
use crate::vfs::infrastructure::media_state::MediaStateWrapper;
use crate::vfs::ports::{StreamFormat, TranscodeQuality};
use crate::commands::{load_transcoding_resource_limits, TranscodingResourceLimits};
use crate::vfs::operation_tracking::OperationTrackingHelper;
use crate::vfs::operation_tracker::OperationType;

/// Warm a file from cold storage to hot cache
#[tauri::command]
pub async fn vfs_warm_file(
    _source_id: String,
    _path: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    // Stub implementation - warming not yet fully implemented
    // In a full implementation, this would copy the file to local cache
    Ok(())
}

/// Transcode a video file
#[tauri::command]
pub async fn vfs_transcode_video(
    source_id: String,
    path: String,
    format: String,
    state: State<'_, VfsStateWrapper>,
    media_state: State<'_, MediaStateWrapper>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    // Get VFS service to resolve the file path
    let vfs_service = state.get_service()
        .ok_or_else(|| "VFS service not initialized".to_string())?;
    
    // Get or initialize media service (lazy initialization)
    let media_service = media_state.get_or_init_service().await
        .map_err(|e| format!("Failed to initialize media service: {}. Please ensure FFmpeg is installed.", e))?;
    
    // Check if FFmpeg is available
    if !media_service.is_available() {
        return Err("FFmpeg is not available. Please install FFmpeg to use transcoding.".to_string());
    }
    
    // Resolve the file path from the source
    let sources = vfs_service.list_sources();
    
    let source = sources.iter()
        .find(|s| s.id == source_id)
        .ok_or_else(|| format!("Source {} not found", source_id))?;
    
    // Normalize path
    let normalized_path = path.strip_prefix("/").unwrap_or(&path);
    
    // Build full path - handle both local and remote storage
    let file_path = if let Some(mount_point) = &source.mount_point {
        // Local mounted storage - use mount point directly
        mount_point.join(normalized_path)
    } else {
        // Remote storage - need to download to temp location first
        // TODO: Implement download for remote files
        // For now, return error for remote files
        return Err("Transcoding remote files requires downloading first. This feature is not yet implemented. Please use local storage or download the file first.".to_string());
    };
    
    // Verify file exists
    if !file_path.exists() {
        return Err(format!("File not found: {}", file_path.display()));
    }
    
    // Parse format
    let stream_format = match format.to_lowercase().as_str() {
        "hls" => StreamFormat::HLS,
        "dash" => StreamFormat::DASH,
        "mp4" => StreamFormat::MP4,
        _ => StreamFormat::HLS, // Default to HLS
    };
    
    // Track transcoding operation
    let operation_id = OperationTrackingHelper::track_operation_start(
        OperationType::Transcode,
        source_id.clone(),
        normalized_path.to_string(),
        Some(format!("Transcoding to {}", format)),
        None,
    );
    
    // Load transcoding settings
    let limits = load_transcoding_resource_limits().await
        .unwrap_or_else(|_| TranscodingResourceLimits {
            threads: 0,
            use_gpu: true,
            gpu_device: -1,
            memory_limit_mb: 0,
            preset: "fast".to_string(),
            max_concurrent_jobs: 1,
        });
    
    // Map preset to quality (simplified - in production, use preset directly)
    let quality = match limits.preset.as_str() {
        "ultrafast" | "superfast" | "veryfast" => TranscodeQuality::Low,
        "faster" | "fast" => TranscodeQuality::Medium,
        "medium" => TranscodeQuality::High,
        "slow" | "slower" | "veryslow" => TranscodeQuality::Ultra,
        _ => TranscodeQuality::Medium,
    };
    
    // Start transcoding
    let job = match media_service.transcode(&file_path, stream_format, quality).await {
        Ok(job) => job,
        Err(e) => {
            let error_msg = format!("Failed to start transcoding: {}", e);
            OperationTrackingHelper::fail_operation(&operation_id, error_msg.clone())
                .unwrap_or_else(|err| tracing::error!("Failed to fail transcode operation: {}", err));
            return Err(error_msg);
        }
    };
    
    // Emit transcoding started event (for frontend event listeners)
    let _ = app_handle.emit("transcode-started", &serde_json::json!({
        "job_id": job.id,
        "operation_id": operation_id,
        "source_path": file_path.to_string_lossy(),
        "format": format,
    }));
    
    // Spawn a task to monitor progress
    let app_handle_clone = app_handle.clone();
    let media_service_clone = media_service.clone();
    let job_id = job.id.clone();
    let operation_id_clone = operation_id.clone(); // Clone before moving into closure
    
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            
            match media_service_clone.get_transcode_status(&job_id).await {
                Ok(job) => {
                    let progress_data = serde_json::json!({
                        "job_id": job.id,
                        "progress": job.progress,
                        "status": format!("{:?}", job.status),
                        "output_path": job.output_path.to_string_lossy(),
                    });
                    
                    let _ = app_handle_clone.emit("transcode-progress", &progress_data);
                    
                    // Update operation progress
                    OperationTrackingHelper::update_progress(&operation_id_clone, job.progress as u64)
                        .unwrap_or_else(|e| tracing::warn!("Failed to update transcode operation progress: {}", e));
                    
                    match job.status {
                        crate::vfs::ports::TranscodeStatus::Completed => {
                            // Complete operation tracking
                            OperationTrackingHelper::complete_operation(&operation_id_clone)
                                .unwrap_or_else(|e| tracing::error!("Failed to complete transcode operation: {}", e));
                            
                            let _ = app_handle_clone.emit("transcode-completed", &serde_json::json!({
                                "job_id": job.id,
                                "operation_id": operation_id_clone,
                                "output_path": job.output_path.to_string_lossy(),
                                "stream_url": job.stream_url,
                            }));
                            break;
                        }
                        crate::vfs::ports::TranscodeStatus::Failed => {
                            // Fail operation tracking
                            let error_msg = job.error.unwrap_or_else(|| "Transcoding failed".to_string());
                            OperationTrackingHelper::fail_operation(&operation_id_clone, error_msg.clone())
                                .unwrap_or_else(|e| tracing::error!("Failed to fail transcode operation: {}", e));
                            
                            let _ = app_handle_clone.emit("transcode-failed", &serde_json::json!({
                                "job_id": job.id,
                                "operation_id": operation_id_clone,
                                "error": error_msg,
                            }));
                            break;
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    let error_msg = format!("Failed to get status: {}", e);
                    OperationTrackingHelper::fail_operation(&operation_id_clone, error_msg.clone())
                        .unwrap_or_else(|err| tracing::error!("Failed to fail transcode operation: {}", err));
                    
                    let _ = app_handle_clone.emit("transcode-failed", &serde_json::json!({
                        "job_id": job_id,
                        "operation_id": operation_id_clone,
                        "error": error_msg,
                    }));
                    break;
                }
            }
        }
    });
    
    // Return operation_id instead of job.id so frontend can track it
    Ok(operation_id)
}

/// Get cache statistics
#[tauri::command]
pub async fn vfs_cache_stats(
    _state: State<'_, VfsStateWrapper>,
) -> Result<VfsCacheStatsResponse, String> {
    // Stub implementation - return empty cache stats
    Ok(VfsCacheStatsResponse {
        total_size: 0,
        max_size: 1024 * 1024 * 1024, // 1GB default
        entry_count: 0,
        hit_count: 0,
        miss_count: 0,
        hit_rate: 0.0,
        usage_percent: 0.0,
    })
}

/// Clear all cached files
#[tauri::command]
pub async fn vfs_clear_cache(
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    // Stub implementation - cache clearing not yet fully implemented
    Ok(())
}
