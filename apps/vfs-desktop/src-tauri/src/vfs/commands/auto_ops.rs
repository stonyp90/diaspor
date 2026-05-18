//! Auto Operations Commands
//!
//! Commands for automatic tagging and transcoding when files are clicked.
//! Uses LLaVA for image/video analysis and Whisper for transcription.

use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;
use tracing::{info, warn, debug};
use crate::vfs::platform::AsyncCommandBuilder;

use crate::vfs::adapters::ollama_client::OllamaClient;
use crate::vfs::domain::{FileTag, StorageSource};
use crate::vfs::infrastructure::media_state::MediaStateWrapper;
use crate::vfs::ports::metadata::IMetadataStore;
use crate::vfs::application::VfsService;
use super::state::VfsStateWrapper;
use super::helpers;
use std::sync::Arc;

/// Helper function to get or cache a file for AI processing
/// For local/mounted storage: returns direct path
/// For S3/remote storage: downloads to cache and returns cached path
async fn get_or_cache_file(
    service: &Arc<VfsService>,
    source: &StorageSource,
    normalized_path: &str,
    file_path: &str,
) -> Result<PathBuf, String> {
    // If source has a mount point (local, cloud storage, or FUSE-mounted), use it directly
    if let Some(mount_point) = &source.mount_point {
        let path = mount_point.join(normalized_path);
        info!("[get_or_cache_file] Using mounted path: {:?}", path);
        return Ok(path);
    }
    
    // For remote storage, download to local cache
    info!("[ai-cache] Preparing file from remote storage");
    
    // Create cache directory structure: temp_dir/ursly-ai-cache/<source-id>/<file-path>
    let temp_dir = std::env::temp_dir().join("ursly-ai-cache");
    let cache_file = temp_dir
        .join(&source.id)
        .join(normalized_path);
    
    // Check if already cached
    if cache_file.exists() {
        info!("[ai-cache] Using cached file (instant)");
        return Ok(cache_file);
    }
    
    info!("[ai-cache] Downloading file for processing");
    
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
    
    info!("[ai-cache] File ready ({:.2} MB cached)", file_size_mb);
    
    Ok(cache_file)
}

/// Auto-tag a file using AI models (LLaVA for vision analysis)
#[tauri::command]
pub async fn vfs_auto_tag_file(
    source_id: String,
    file_path: String,
    max_tags: Option<u32>,
    state: State<'_, VfsStateWrapper>,
) -> Result<AutoTagResult, String> {
    use crate::vfs::operation_tracker::OperationType;
    use crate::vfs::commands::get_operation_tracker;
    
    let max_tags = max_tags.unwrap_or(5).min(10) as usize;
    info!("[vfs_auto_tag_file] Auto-tagging file: {} in source {} (max {} tags)", file_path, source_id, max_tags);
    
    // Register operation with tracker
    let operation_tracker = get_operation_tracker();
    let operation_id = operation_tracker.create_operation(
        OperationType::AutoTag,
        source_id.clone(),
        file_path.clone(),
        None, // No destination
        None, // File size unknown initially
    );
    
    // Helper to mark operation as failed
    let mark_failed = |error_msg: String| {
        let _ = operation_tracker.fail_operation(&operation_id, error_msg.clone());
        error_msg
    };
    
    // Check if Ollama is available
    let client = OllamaClient::new(None);
    if !client.is_available().await {
        return Err(mark_failed("Ollama is not available. Please install and start Ollama first.".to_string()));
    }
    
    // Get the file from VFS
    let service = state.get_service()
        .ok_or_else(|| mark_failed("VFS service not initialized".to_string()))?;
    
    let path = Path::new(&file_path);
    let files = service.list_files(&source_id, path.parent().unwrap_or(Path::new("/")))
        .await
        .map_err(|e| mark_failed(format!("Failed to list files: {}", e)))?;
    
    let file = files.iter()
        .find(|f| f.path == path)
        .ok_or_else(|| mark_failed("File not found".to_string()))?;
    
    // Determine file type
    let mime_type = file.content_type.as_deref().unwrap_or("");
    let is_video = mime_type.starts_with("video/") || 
        ["mp4", "mov", "avi", "mkv", "webm", "m4v"].iter()
            .any(|ext| file.name.to_lowercase().ends_with(&format!(".{}", ext)));
    let is_image = mime_type.starts_with("image/") ||
        ["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "heic"].iter()
            .any(|ext| file.name.to_lowercase().ends_with(&format!(".{}", ext)));
    
    if !is_video && !is_image {
        return Err(mark_failed("Auto-tagging is only supported for video and image files".to_string()));
    }
    
    // Find the best available vision model
    let model_name = find_best_vision_model(&client).await
        .map_err(|e| mark_failed(format!("No vision model available: {}", e)))?;
    
    info!("[vfs_auto_tag_file] Using vision model: {}", model_name);
    
    // Get the actual file path
    let sources = service.list_sources();
    let source = sources.iter()
        .find(|s| s.id == source_id)
        .ok_or_else(|| format!("Source {} not found", source_id))?;
    
    let normalized_path = file_path.strip_prefix("/").unwrap_or(&file_path);
    
    // Start background progress simulation task
    let operation_id_clone = operation_id.clone();
    tokio::spawn(async move {
        use crate::vfs::commands::get_operation_tracker;
        // Simulate gradual progress over 10-12 seconds for realistic AI analysis feel
        // Even if actual operation is faster, this ensures visible feedback
        for progress in (5..95).step_by(3) {
            tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
            let tracker = get_operation_tracker();
            let _ = tracker.update_operation_progress(&operation_id_clone, progress, Some(100));
        }
    });

    // Get or download file to local cache (5-10%)
    let actual_path = get_or_cache_file(&service, source, normalized_path, &file_path).await
        .map_err(&mark_failed)?;
    
    if !actual_path.exists() {
        return Err(mark_failed(format!("File not found: {}", actual_path.display())));
    }
    
    // Get image data to analyze (10-20%)
    let image_data = if is_video {
        // Extract a frame from the video using FFmpeg
        extract_video_frame(&actual_path).await
            .map_err(|e| mark_failed(format!("Failed to extract video frame: {}", e)))?
    } else {
        // Read the image directly
        tokio::fs::read(&actual_path).await
            .map_err(|e| mark_failed(format!("Failed to read image: {}", e)))?
    };
    
    // Analyze the image using LLaVA (20-90%)
    // This is the longest part - the background task simulates progress
    // Add minimum processing time to ensure progress is visible
    let analysis_start = std::time::Instant::now();
    let analysis = client.analyze_image(&model_name, &image_data).await
        .map_err(|e| mark_failed(format!("Failed to analyze image: {}", e)))?;
    
    // Ensure minimum 8 seconds elapsed for realistic AI analysis feel
    let elapsed = analysis_start.elapsed();
    let min_duration = std::time::Duration::from_secs(8);
    if elapsed < min_duration {
        tokio::time::sleep(min_duration - elapsed).await;
    }
    
    info!("[vfs_auto_tag_file] Analysis complete: {} tags, description: '{}'", 
        analysis.tags.len(), 
        analysis.description.chars().take(50).collect::<String>());
    
    // Get metadata store
    let metadata_store = helpers::get_metadata_store_instance().await
        .map_err(|e| format!("Metadata store not available: {}", e))?;
    
    // Convert to FileTags and save, limiting to max_tags
    let mut tags: Vec<FileTag> = analysis.tags.iter()
        .take(max_tags.saturating_sub(1)) // Reserve one slot for file type tag
        .map(FileTag::new)
        .collect();
    
    // Add file type tag at the beginning
    if is_video && !tags.iter().any(|t| t.name == "video") {
        tags.insert(0, FileTag::new("video"));
    } else if is_image && !tags.iter().any(|t| t.name == "image" || t.name == "photo") {
        tags.insert(0, FileTag::new("image"));
    }
    
    // Ensure we don't exceed max_tags
    tags.truncate(max_tags);
    
    // Save tags to metadata store
    for tag in &tags {
        if let Err(e) = IMetadataStore::add_tag(&*metadata_store, &source_id, path, tag.clone()).await {
            warn!("[vfs_auto_tag_file] Failed to save tag '{}': {}", tag.name, e);
        }
    }
    
    let tag_names: Vec<String> = tags.iter().map(|t| t.name.clone()).collect();
    let tag_count = tags.len();
    
    info!("[vfs_auto_tag_file] Successfully tagged file with {} AI-generated tags", tag_count);
    
    // Mark operation as complete
    if let Err(e) = operation_tracker.complete_operation(&operation_id) {
        warn!("[vfs_auto_tag_file] Failed to mark operation complete: {}", e);
    }
    
    Ok(AutoTagResult {
        success: true,
        tags: tag_names,
        message: format!("AI analyzed and tagged file with {} tags", tag_count),
        operation_id: Some(operation_id),
    })
}

/// Find the best available vision model (prefer llava:13b, then llava, then others)
async fn find_best_vision_model(client: &OllamaClient) -> Result<String, String> {
    let models = client.list_models().await
        .map_err(|e| format!("Failed to list models: {}", e))?;
    
    // Priority order for vision models
    let preferred_models = [
        "llava:13b",
        "llava:34b",
        "llava:latest",
        "llava",
        "bakllava",
        "moondream",
    ];
    
    // First check for preferred models in order
    for preferred in preferred_models {
        if models.iter().any(|m| m.name.to_lowercase() == preferred.to_lowercase()) {
            return Ok(preferred.to_string());
        }
    }
    
    // Fallback: find any vision-capable model
    for model in &models {
        if OllamaClient::is_vision_model(&model.name) {
            return Ok(model.name.clone());
        }
    }
    
    Err("No vision model found. Please install LLaVA: ollama pull llava".to_string())
}

/// Extract a representative frame from a video file using FFmpeg
async fn extract_video_frame(video_path: &PathBuf) -> Result<Vec<u8>, String> {
    // Find FFmpeg
    let ffmpeg_path = find_ffmpeg().await
        .ok_or_else(|| "FFmpeg not found. Please install FFmpeg.".to_string())?;
    
    // Get video duration to extract a frame from the middle
    let duration = get_video_duration(&ffmpeg_path, video_path).await.unwrap_or(10.0);
    let seek_time = (duration / 3.0).max(1.0); // Extract from 1/3 into the video
    
    debug!("[extract_video_frame] Extracting frame at {:.2}s from {:?}", seek_time, video_path);
    
    // Extract a single frame as JPEG to stdout
    let output = AsyncCommandBuilder::new(ffmpeg_path.to_string_lossy())
        .args([
            "-ss", &format!("{:.2}", seek_time),
            "-i", video_path.to_str().unwrap(),
            "-vframes", "1",
            "-f", "image2pipe",
            "-vcodec", "mjpeg",
            "-q:v", "2", // High quality
            "-",
        ])
        .stdout_piped()
        .stderr_null()
        .output()
        .await
        .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;
    
    if !output.status.success() || output.stdout.is_empty() {
        // Try extracting from the beginning if middle extraction failed
        let output = AsyncCommandBuilder::new(ffmpeg_path.to_string_lossy())
            .args([
                "-i", video_path.to_str().unwrap(),
                "-vframes", "1",
                "-f", "image2pipe",
                "-vcodec", "mjpeg",
                "-q:v", "2",
                "-",
            ])
            .stdout_piped()
            .stderr_null()
            .output()
            .await
            .map_err(|e| format!("Failed to run FFmpeg: {}", e))?;
        
        if output.stdout.is_empty() {
            return Err("Failed to extract frame from video".to_string());
        }
        
        return Ok(output.stdout);
    }
    
    Ok(output.stdout)
}

/// Find FFmpeg binary path
async fn find_ffmpeg() -> Option<PathBuf> {
    let candidates = vec![
        PathBuf::from("/opt/homebrew/bin/ffmpeg"),
        PathBuf::from("/usr/local/bin/ffmpeg"),
        PathBuf::from("/usr/bin/ffmpeg"),
        PathBuf::from("ffmpeg"),
    ];
    
    for path in candidates {
        if test_binary(&path).await {
            return Some(path);
        }
    }
    
    None
}

/// Test if a binary exists and is executable
async fn test_binary(path: &Path) -> bool {
    AsyncCommandBuilder::new(path.to_string_lossy())
        .arg("-version")
        .stdout_null()
        .stderr_null()
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get video duration using FFprobe
async fn get_video_duration(ffmpeg_path: &Path, video_path: &Path) -> Option<f64> {
    // Derive ffprobe path from ffmpeg path
    let ffprobe_path = ffmpeg_path.with_file_name("ffprobe");
    
    let output = AsyncCommandBuilder::new(ffprobe_path.to_string_lossy())
        .args([
            "-v", "quiet",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            video_path.to_str()?,
        ])
        .stdout_piped()
        .stderr_null()
        .output()
        .await
        .ok()?;
    
    let duration_str = String::from_utf8_lossy(&output.stdout);
    duration_str.trim().parse::<f64>().ok()
}

/// Auto-transcode a video file
#[tauri::command]
pub async fn vfs_auto_transcode(
    source_id: String,
    file_path: String,
    state: State<'_, VfsStateWrapper>,
    media_state: State<'_, MediaStateWrapper>,
) -> Result<AutoTranscodeResult, String> {
    info!("[vfs_auto_transcode] Auto-transcoding file: {} in source {}", file_path, source_id);
    
    // Check if FFmpeg is available (Windows-safe)
    let ffmpeg_available = crate::vfs::platform::CommandBuilder::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    
    if !ffmpeg_available {
        return Err("FFmpeg is not available. Please install FFmpeg first.".to_string());
    }
    
    // Get the file from VFS
    let service = state.get_service()
        .ok_or_else(|| "VFS service not initialized".to_string())?;
    
    let path = Path::new(&file_path);
    let files = service.list_files(&source_id, path.parent().unwrap_or(Path::new("/")))
        .await
        .map_err(|e| format!("Failed to list files: {}", e))?;
    
    let file = files.iter()
        .find(|f| f.path == path)
        .ok_or_else(|| "File not found".to_string())?;
    
    // Check if it's a video file
    let mime_type = file.content_type.as_deref().unwrap_or("");
    let is_video = mime_type.starts_with("video/") || 
        ["mp4", "mov", "avi", "mkv", "webm", "m4v"].iter()
            .any(|ext| file.name.to_lowercase().ends_with(&format!(".{}", ext)));
    
    if !is_video {
        return Err("Auto-transcoding is only supported for video files".to_string());
    }
    
    // Get media service and start transcoding
    use crate::vfs::ports::media::{StreamFormat, TranscodeQuality};
    
    let media_service = media_state.get_or_init_service().await
        .map_err(|e| format!("Failed to initialize media service: {}", e))?;
    
    if !media_service.is_available() {
        return Err("FFmpeg is not available. Please install FFmpeg first.".to_string());
    }
    
    // Resolve the actual file path
    let sources = service.list_sources();
    let source = sources.iter()
        .find(|s| s.id == source_id)
        .ok_or_else(|| format!("Source {} not found", source_id))?;
    
    // Normalize path
    let normalized_path = file_path.strip_prefix("/").unwrap_or(&file_path);
    
    // Get or download file to local cache
    let actual_path = get_or_cache_file(&service, source, normalized_path, &file_path).await?;
    
    // Verify file exists
    if !actual_path.exists() {
        return Err(format!("File not found: {}", actual_path.display()));
    }
    
    // Start transcoding job
    let job = media_service.transcode(&actual_path, StreamFormat::HLS, TranscodeQuality::Medium).await
        .map_err(|e| format!("Failed to start transcoding: {}", e))?;
    
    info!("[vfs_auto_transcode] Started transcoding job: {}", job.id);
    
    let job_id = job.id.clone();
    Ok(AutoTranscodeResult {
        success: true,
        job_id,
        message: format!("Transcoding started successfully. Job ID: {}", job.id),
    })
}

/// Ensure required models are running for an operation
#[tauri::command]
pub async fn vfs_ensure_models_running(
    operation_type: String, // "transcription", "tagging", "transcoding"
) -> Result<ModelStatusResult, String> {
    info!("[vfs_ensure_models_running] Ensuring models running for: {}", operation_type);
    
    let client = OllamaClient::new(None);
    
    if !client.is_available().await {
        return Err("Ollama is not available. Please install and start Ollama first.".to_string());
    }
    
    // Determine which model is needed
    let model_name = match operation_type.as_str() {
        "transcription" => "whisper",
        "tagging" => "llava",
        "transcoding" => {
            // Transcoding doesn't need Ollama, just FFmpeg (Windows-safe)
            let ffmpeg_available = crate::vfs::platform::CommandBuilder::new("ffmpeg")
                .arg("-version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            
            if !ffmpeg_available {
                return Err("FFmpeg is not available. Please install FFmpeg first.".to_string());
            }
            
            return Ok(ModelStatusResult {
                success: true,
                message: "FFmpeg is available for transcoding".to_string(),
                model_name: None,
            });
        }
        _ => return Err(format!("Unknown operation type: {}", operation_type)),
    };
    
    // Check if model is installed
    let models = client.list_models().await
        .map_err(|e| format!("Failed to list models: {}", e))?;
    
    let model_installed = models.iter().any(|m| m.name.contains(model_name));
    if !model_installed {
        return Err(format!("Model '{}' is not installed. Please install it first using: ollama pull {}", model_name, model_name));
    }
    
    // Models are loaded on-demand by Ollama, so we just need to verify availability
    Ok(ModelStatusResult {
        success: true,
        message: format!("Model '{}' is available and ready", model_name),
        model_name: Some(model_name.to_string()),
    })
}

#[derive(Debug, Serialize)]
pub struct AutoTagResult {
    pub success: bool,
    pub tags: Vec<String>,
    pub message: String,
    pub operation_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AutoTranscodeResult {
    pub success: bool,
    pub job_id: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ModelStatusResult {
    pub success: bool,
    pub message: String,
    pub model_name: Option<String>,
}
