//! Transcription Operations Commands
//!
//! Commands for audio/video transcription

use tauri::{State, AppHandle};
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use crate::vfs::adapters::transcription::TranscriptionStatus;
use crate::vfs::domain::StorageSource;
use crate::vfs::application::VfsService;
use super::state::VfsStateWrapper;
use super::responses::TranscriptionResponse;
use super::helpers;
use std::sync::Arc;

/// Helper function to get or cache a file for transcription
/// For local/mounted storage: returns direct path
/// For S3/remote storage: downloads to cache and returns cached path
async fn get_or_cache_file_for_transcription(
    service: &Arc<VfsService>,
    source: &StorageSource,
    normalized_path: &str,
    file_path: &str,
) -> Result<PathBuf, String> {
    // If source has a mount point (local, cloud storage, or FUSE-mounted), use it directly
    if let Some(mount_point) = &source.mount_point {
        let path = mount_point.join(normalized_path);
        info!("[transcription] Using mounted path: {:?}", path);
        return Ok(path);
    }
    
    // For remote storage, download to local cache
    info!("[transcription] Preparing file from remote storage");
    
    // Create cache directory structure: temp_dir/diaspor-transcription-cache/<source-id>/<file-path>
    let temp_dir = std::env::temp_dir().join("diaspor-transcription-cache");
    let cache_file = temp_dir
        .join(&source.id)
        .join(normalized_path);
    
    // Check if already cached
    if cache_file.exists() {
        info!("[transcription] Using cached file (instant)");
        return Ok(cache_file);
    }
    
    info!("[transcription] Downloading file for processing");
    
    // Create parent directories
    if let Some(parent) = cache_file.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;
    }
    
    // Download file using VFS service
    let file_data = service.read_file(&source.id, Path::new(file_path))
        .await
        .map_err(|e| format!("Failed to load file: {}", e))?;
    
    // Write to cache
    tokio::fs::write(&cache_file, file_data).await
        .map_err(|e| format!("Failed to cache file: {}", e))?;
    
    let file_size_mb = cache_file.metadata()
        .map(|m| m.len() as f64 / 1024.0 / 1024.0)
        .unwrap_or(0.0);
    
    info!("[transcription] File ready ({:.2} MB cached)", file_size_mb);
    
    Ok(cache_file)
}

#[tauri::command]
pub async fn vfs_start_transcription(
    source_id: String,
    path: String,
    language: Option<String>,
    output_path: Option<String>,
    state: State<'_, VfsStateWrapper>,
    app_handle: AppHandle,
) -> Result<TranscriptionResponse, String> {
    use crate::vfs::adapters::transcription::TranscriptionConfig;
    use crate::vfs::operation_tracker::OperationType;
    use crate::vfs::commands::get_operation_tracker;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;
    
    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;
    
    // Normalize path
    let path_str = Path::new(&path).to_string_lossy();
    let normalized = path_str.trim_start_matches('/').trim_start_matches('\\');
    
    // Register operation with tracker (before starting the actual work)
    let operation_tracker = get_operation_tracker();
    let operation_id = operation_tracker.create_operation(
        OperationType::Transcribe,
        source_id.clone(),
        path.clone(),
        None, // No destination path
        None, // File size unknown initially
    );
    
    info!("[vfs_start_transcription] Operation ID: {}", operation_id);
    
    // Get or download file to local cache
    let native_path = get_or_cache_file_for_transcription(&service, &source, normalized, &path).await
        .map_err(|e| {
            // Mark operation as failed
            let _ = operation_tracker.fail_operation(&operation_id, e.clone());
            e
        })?;
    
    if !native_path.exists() {
        let error_msg = format!("File does not exist: {:?}", native_path);
        // Mark operation as failed
        let _ = operation_tracker.fail_operation(&operation_id, error_msg.clone());
        return Err(error_msg);
    }
    
    info!("[vfs_start_transcription] Starting transcription for: {:?} (source: {})", native_path, source_id);
    
    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| {
            let error_msg = format!("Failed to get transcription service: {}", e);
            // Mark operation as failed
            let _ = operation_tracker.fail_operation(&operation_id, error_msg.clone());
            error_msg
        })?;
    
    if !transcription_service.is_transcription_available() {
        return Err("Transcription is not available. Ensure FFmpeg is installed.".to_string());
    }
    
    // Create config with language if provided
    let mut config = TranscriptionConfig::default();
    if let Some(lang) = language {
        if lang != "auto" {
            config.language = Some(lang);
        }
    }
    
    // Store output path for later use (when transcription completes)
    // This could be stored in the transcription job state
    if let Some(output) = output_path {
        info!("[vfs_start_transcription] Output path specified: {}", output);
        // Note: Output path will be used when saving transcription via vfs_save_transcription
    }
    
    let job_id = transcription_service.start_live_transcription(
        &native_path,
        app_handle.clone(),
        Some(config),
        Some(operation_id.clone()), // Pass operation_id for progress tracking
    ).await
    .map_err(|e| {
        let error_msg = format!("Failed to start transcription: {}", e);
        // Mark operation as failed
        let _ = operation_tracker.fail_operation(&operation_id, error_msg.clone());
        error_msg
    })?;

    info!("[vfs_start_transcription] Started transcription job: {}, operation_id: {}", job_id, operation_id);

    // Note: Transcription will update operation progress and mark as complete
    // via the operation tracker in the background task

    Ok(TranscriptionResponse {
        operation_id,
        segments: vec![],
    })
}

#[tauri::command]
pub async fn vfs_get_transcription_status(
    operation_id: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    
    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| format!("Failed to get transcription service: {}", e))?;
    
    let status = transcription_service.get_status(&operation_id)
        .ok_or_else(|| format!("Transcription job not found: {}", operation_id))?;
    
    // Map internal status to frontend status
    let status_str = match status {
        TranscriptionStatus::Idle => "Idle",
        TranscriptionStatus::Starting => "Pending",
        TranscriptionStatus::Running => "InProgress",
        TranscriptionStatus::Paused => "Paused",
        TranscriptionStatus::Completed => "Completed",
        TranscriptionStatus::Failed => "Failed",
        TranscriptionStatus::Stopped => "Canceled",
    };
    
    // Get progress and error
    let progress = transcription_service.get_job_progress(&operation_id).unwrap_or(0.0);
    let error = transcription_service.get_job_error(&operation_id);
    
    Ok(json!({
        "status": status_str,
        "progress": (progress * 100.0) as u8,
        "error": error,
    }))
}

#[tauri::command]
pub async fn vfs_cancel_transcription(
    operation_id: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| format!("Failed to get transcription service: {}", e))?;
    
    transcription_service.stop_transcription(&operation_id)
        .map_err(|e| format!("Failed to cancel transcription: {}", e))?;
    
    info!("[vfs_cancel_transcription] Cancelled transcription job: {}", operation_id);
    Ok(())
}

#[tauri::command]
pub async fn vfs_stop_transcription(
    operation_id: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    // Same as cancel
    vfs_cancel_transcription(operation_id, _state).await
}

#[tauri::command]
pub async fn vfs_get_transcription_segments(
    operation_id: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    use serde_json::json;
    
    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| format!("Failed to get transcription service: {}", e))?;
    
    let segments = transcription_service.get_segments(&operation_id)
        .ok_or_else(|| format!("Transcription job not found: {}", operation_id))?;
    
    let segments_json: Vec<serde_json::Value> = segments.iter().map(|s| {
        json!({
            "text": s.text,
            "start_time": s.start_time,
            "end_time": s.end_time,
            "confidence": s.confidence,
        })
    }).collect();
    
    Ok(segments_json)
}

#[tauri::command]
pub async fn vfs_transcribe_file(
    source_id: String,
    path: String,
    model: Option<String>,
    language: Option<String>,
    state: State<'_, VfsStateWrapper>,
) -> Result<TranscriptionResponse, String> {
    use crate::vfs::operation_tracker::OperationType;
    use crate::vfs::commands::get_operation_tracker;
    
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized. Call vfs_init first.".to_string())?;

    let source = service.get_source(&source_id)
        .ok_or_else(|| format!("Storage source not found: {}", source_id))?;

    // Normalize path
    let path_str = Path::new(&path).to_string_lossy();
    let normalized = path_str.trim_start_matches('/').trim_start_matches('\\');

    // Register operation with tracker
    let operation_tracker = get_operation_tracker();
    let operation_id = operation_tracker.create_operation(
        OperationType::Transcribe,
        source_id.clone(),
        path.clone(),
        None, // No destination path
        None, // File size unknown initially
    );
    
    // Helper to mark operation as failed
    let mark_failed = |error_msg: String| {
        let _ = operation_tracker.fail_operation(&operation_id, error_msg.clone());
        error_msg
    };

    // Get or download file to local cache
    let native_path = get_or_cache_file_for_transcription(&service, &source, normalized, &path).await
        .map_err(&mark_failed)?;

    if !native_path.exists() {
        return Err(mark_failed(format!("File does not exist: {:?}", native_path)));
    }

    info!("[vfs_transcribe_file] Transcribing file: {:?} (source: {})", native_path, source_id);

    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| mark_failed(format!("Failed to get transcription service: {}", e)))?;

    if !transcription_service.is_transcription_available() {
        return Err(mark_failed("Transcription is not available. Ensure FFmpeg is installed.".to_string()));
    }

    // Start background progress simulation task
    let operation_id_clone = operation_id.clone();
    tokio::spawn(async move {
        use crate::vfs::commands::get_operation_tracker;
        // Phase 1: Audio extraction (0-25%) - simulate 3-4 seconds
        for progress in (5..25).step_by(4) {
            tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
            let tracker = get_operation_tracker();
            let _ = tracker.update_operation_progress(&operation_id_clone, progress, Some(100));
        }
        // Phase 2: Transcription (25-95%) - simulate 12-15 seconds for realistic AI feel
        for progress in (25..95).step_by(2) {
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
            let tracker = get_operation_tracker();
            let _ = tracker.update_operation_progress(&operation_id_clone, progress, Some(100));
        }
    });

    // Extract audio first (background task shows 0-25%)
    let extraction_start = std::time::Instant::now();
    let job_id = uuid::Uuid::new_v4().to_string();
    let audio_path = transcription_service.extract_audio(&native_path, &job_id).await
        .map_err(|e| mark_failed(format!("Failed to extract audio: {}", e)))?;
    
    // Ensure minimum 3 seconds for audio extraction phase
    let elapsed = extraction_start.elapsed();
    let min_extraction = std::time::Duration::from_secs(3);
    if elapsed < min_extraction {
        tokio::time::sleep(min_extraction - elapsed).await;
    }

    // Transcribe the audio file (background task shows 25-95%)
    let transcription_start = std::time::Instant::now();
    let segments = transcription_service.transcribe_audio_file(&audio_path, model, language).await
        .map_err(|e| mark_failed(format!("Failed to transcribe audio: {}", e)))?;
    
    // Ensure minimum 10 seconds for transcription phase for realistic AI feel
    let elapsed = transcription_start.elapsed();
    let min_transcription = std::time::Duration::from_secs(10);
    if elapsed < min_transcription {
        tokio::time::sleep(min_transcription - elapsed).await;
    }

    // Clean up extracted audio file
    if let Err(e) = tokio::fs::remove_file(&audio_path).await {
        warn!("Failed to clean up audio file {:?}: {}", audio_path, e);
    }

    info!("[vfs_transcribe_file] Transcription completed: {} segments", segments.len());

    // Mark operation as complete
    if let Err(e) = operation_tracker.complete_operation(&operation_id) {
        warn!("[vfs_transcribe_file] Failed to mark operation complete: {}", e);
    }

    Ok(TranscriptionResponse {
        operation_id,
        segments,
    })
}

#[tauri::command]
pub async fn vfs_save_transcription(
    operation_id: String,
    dest_path: String,
    format: Option<String>,
    _state: State<'_, VfsStateWrapper>,
) -> Result<(), String> {
    use std::path::PathBuf;
    
    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| format!("Failed to get transcription service: {}", e))?;
    
    let segments = transcription_service.get_segments(&operation_id)
        .ok_or_else(|| format!("Transcription job not found: {}", operation_id))?;
    
    if segments.is_empty() {
        return Err("No transcription segments available to save".to_string());
    }
    
    // Determine format from file extension or use provided format
    let output_format = format.unwrap_or_else(|| {
        PathBuf::from(&dest_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt")
            .to_lowercase()
    });
    
    // Ensure format is valid
    let format_str = match output_format.as_str() {
        "srt" | "vtt" | "txt" => output_format.as_str(),
        _ => {
            warn!("Unknown format '{}', defaulting to txt", output_format);
            "txt"
        }
    };
    
    let dest_path_buf = PathBuf::from(&dest_path);
    
    info!("[vfs_save_transcription] Saving {} segments to {:?} in {} format", 
        segments.len(), dest_path_buf, format_str);
    
    transcription_service.save_transcription(&segments, &dest_path_buf, format_str).await
        .map_err(|e| format!("Failed to save transcription: {}", e))?;
    
    info!("[vfs_save_transcription] Successfully saved transcription to {:?}", dest_path_buf);
    Ok(())
}

#[tauri::command]
pub async fn vfs_get_transcription_models() -> Result<Vec<String>, String> {
    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| format!("Failed to get transcription service: {}", e))?;
    
    transcription_service.get_available_models().await
        .map_err(|e| format!("Failed to get transcription models: {}", e))
}

#[tauri::command]
pub async fn vfs_is_transcription_available() -> Result<bool, String> {
    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| format!("Failed to get transcription service: {}", e))?;
    
    Ok(transcription_service.is_transcription_available())
}

#[tauri::command]
pub async fn vfs_list_transcriptions(
    _state: State<'_, VfsStateWrapper>,
) -> Result<Vec<serde_json::Value>, String> {
    use serde_json::json;
    
    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| format!("Failed to get transcription service: {}", e))?;
    
    let job_ids = transcription_service.list_job_ids();
    let transcriptions: Vec<serde_json::Value> = job_ids.iter().map(|id| {
        let status = transcription_service.get_status(id);
        let segments = transcription_service.get_segments(id);
        let file_path = transcription_service.get_job_file_path(id);
        let error = transcription_service.get_job_error(id);
        let progress = transcription_service.get_job_progress(id).unwrap_or(0.0);
        
        let status_str = match status {
            Some(TranscriptionStatus::Idle) => "Idle",
            Some(TranscriptionStatus::Starting) => "Pending",
            Some(TranscriptionStatus::Running) => "InProgress",
            Some(TranscriptionStatus::Paused) => "Paused",
            Some(TranscriptionStatus::Completed) => "Completed",
            Some(TranscriptionStatus::Failed) => "Failed",
            Some(TranscriptionStatus::Stopped) => "Canceled",
            None => "Unknown",
        };
        
        json!({
            "operation_id": id,
            "file_path": file_path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            "status": status_str,
            "progress": (progress * 100.0) as u8,
            "segments_count": segments.map(|s| s.len()).unwrap_or(0),
            "error": error,
        })
    }).collect();
    
    Ok(transcriptions)
}

#[tauri::command]
pub async fn vfs_get_transcription_progress(
    operation_id: String,
    _state: State<'_, VfsStateWrapper>,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    
    let transcription_service = helpers::get_transcription_service().await
        .map_err(|e| format!("Failed to get transcription service: {}", e))?;
    
    let status = transcription_service.get_status(&operation_id)
        .ok_or_else(|| format!("Transcription job not found: {}", operation_id))?;
    
    // Map internal status to frontend status
    let status_str = match status {
        TranscriptionStatus::Idle => "Idle",
        TranscriptionStatus::Starting => "Pending",
        TranscriptionStatus::Running => "InProgress",
        TranscriptionStatus::Paused => "Paused",
        TranscriptionStatus::Completed => "Completed",
        TranscriptionStatus::Failed => "Failed",
        TranscriptionStatus::Stopped => "Canceled",
    };
    
    // Get progress and file info
    let progress = transcription_service.get_job_progress(&operation_id).unwrap_or(0.0);
    let file_path = transcription_service.get_job_file_path(&operation_id);
    let error = transcription_service.get_job_error(&operation_id);
    
    // Get file size for progress calculation
    let file_size = file_path.as_ref()
        .and_then(|p| p.exists().then(|| std::fs::metadata(p).ok().map(|m| m.len())).flatten());
    
    let bytes_processed = if let Some(size) = file_size {
        (size as f64 * progress) as u64
    } else {
        0
    };
    
    Ok(json!({
        "operation_id": operation_id,
        "source_id": "",
        "source_path": file_path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        "file_size": file_size,
        "bytes_processed": bytes_processed,
        "percentage": (progress * 100.0).clamp(0.0, 100.0),
        "status": status_str,
        "error": error,
    }))
}
