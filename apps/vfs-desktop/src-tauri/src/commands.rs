//! Tauri Commands for GPU Metrics
//! 
//! IPC commands that can be called from the frontend.

use crate::gpu::{self, GpuInfo, GpuMetrics, GPU_METRICS};
use crate::system::{self, SystemInfo, SystemMetrics, ProcessInfo};
use crate::vfs::platform::CommandBuilder;
use serde::{Deserialize, Serialize};
use std::process::Child;
use std::sync::Mutex;
use futures::StreamExt;
use tauri::Emitter;

/// Running model state
static RUNNING_MODEL: once_cell::sync::Lazy<Mutex<Option<RunningModel>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

#[derive(Debug, Serialize)]
pub struct RunningModel {
    pub name: String,
    pub pid: u32,
    pub started_at: u64,
    #[serde(skip)]
    pub process: Option<Child>,
}

/// All metrics combined for dashboard
#[derive(Debug, Serialize)]
pub struct AllMetrics {
    pub gpus: Vec<GpuWithMetrics>,
    pub system: SystemMetrics,
    pub model_processes: Vec<ProcessInfo>,
    pub running_model: Option<ModelStatus>,
}

#[derive(Debug, Serialize)]
pub struct GpuWithMetrics {
    pub info: GpuInfo,
    pub current: GpuMetrics,
    pub history: Vec<GpuMetrics>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelStatus {
    pub name: String,
    pub running: bool,
    pub started_at: u64,
    pub duration_seconds: u64,
}

/// Get information about all detected GPUs
#[tauri::command]
pub fn get_gpu_info() -> Vec<GpuInfo> {
    gpu::detect_gpus()
}

/// Get current metrics for a specific GPU
#[tauri::command]
pub fn get_gpu_metrics(gpu_id: u32) -> GpuMetrics {
    gpu::get_current_metrics(gpu_id)
}

/// Start GPU metrics polling (call when metrics page is opened)
#[tauri::command]
pub fn start_gpu_polling(app: tauri::AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        gpu::start_metrics_polling(handle);
    });
}

/// Stop GPU metrics polling (call when metrics page is closed)
#[tauri::command]
pub fn stop_gpu_polling() {
    gpu::stop_metrics_polling();
}

/// Get system information
#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    system::get_system_info()
}

/// Get all metrics for the dashboard
#[tauri::command]
pub fn get_all_metrics() -> AllMetrics {
    let gpu_infos = gpu::detect_gpus();
    let histories = GPU_METRICS.lock().unwrap();
    
    let gpus: Vec<GpuWithMetrics> = gpu_infos
        .into_iter()
        .map(|info| {
            let current = gpu::get_current_metrics(info.id);
            let history = histories
                .iter()
                .find(|h| h.gpu_id == info.id)
                .map(|h| h.samples.clone())
                .unwrap_or_default();
            
            GpuWithMetrics {
                info,
                current,
                history,
            }
        })
        .collect();

    let system = system::get_system_metrics();
    let model_processes = system::find_model_processes();
    
    let running_model = {
        let model = RUNNING_MODEL.lock().unwrap();
        model.as_ref().map(|m| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            ModelStatus {
                name: m.name.clone(),
                running: true,
                started_at: m.started_at,
                duration_seconds: now - m.started_at,
            }
        })
    };

    AllMetrics {
        gpus,
        system,
        model_processes,
        running_model,
    }
}

/// Model configuration for starting
#[derive(Debug, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    pub ollama_url: Option<String>,
}

/// Start a model using Ollama
#[tauri::command]
pub async fn start_model(config: ModelConfig) -> Result<ModelStatus, String> {
    let mut running = RUNNING_MODEL.lock().map_err(|e| e.to_string())?;
    
    if running.is_some() {
        return Err("A model is already running".to_string());
    }

    let ollama_url = config.ollama_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    
    // Start ollama run command (Windows-safe, no terminal window)
    let process = CommandBuilder::new("ollama")
        .args(["run", &config.name])
        .env("OLLAMA_HOST", &ollama_url)
        .spawn()
        .map_err(|e| format!("Failed to start model: {}", e))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let pid = process.id();
    
    let model = RunningModel {
        name: config.name.clone(),
        pid,
        started_at: now,
        process: Some(process),
    };

    let status = ModelStatus {
        name: model.name.clone(),
        running: true,
        started_at: model.started_at,
        duration_seconds: 0,
    };

    *running = Some(model);
    
    Ok(status)
}

/// Stop the currently running model
#[tauri::command]
pub async fn stop_model() -> Result<(), String> {
    let mut running = RUNNING_MODEL.lock().map_err(|e| e.to_string())?;
    
    if let Some(mut model) = running.take() {
        if let Some(ref mut process) = model.process {
            let _ = process.kill();
        }
        Ok(())
    } else {
        Err("No model is currently running".to_string())
    }
}

/// Get current model status
#[tauri::command]
pub fn get_model_status() -> Option<ModelStatus> {
    let running = RUNNING_MODEL.lock().ok()?;
    
    running.as_ref().map(|m| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        ModelStatus {
            name: m.name.clone(),
            running: true,
            started_at: m.started_at,
            duration_seconds: now - m.started_at,
        }
    })
}


// ============================================================================
// AI Dependencies Installation Commands
// ============================================================================

/// Detect the current platform
#[tauri::command]
pub fn detect_platform() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    return Ok("macos".to_string());
    
    #[cfg(target_os = "windows")]
    return Ok("windows".to_string());
    
    #[cfg(target_os = "linux")]
    return Ok("linux".to_string());
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    return Ok("unknown".to_string());
}

/// Check if Docker is installed
#[tauri::command]
pub async fn check_docker_installed() -> Result<bool, String> {
    let result = CommandBuilder::new("docker")
        .arg("--version")
        .output();
    
    match result {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

/// Check if Docker is running
#[tauri::command]
pub async fn check_docker_running() -> Result<bool, String> {
    let result = CommandBuilder::new("docker")
        .arg("info")
        .output();
    
    match result {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

/// Check if Ollama is installed
#[tauri::command]
pub async fn check_ollama_installed() -> Result<bool, String> {
    let result = CommandBuilder::new("ollama")
        .arg("--version")
        .output();
    
    match result {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

/// Check if Ollama is running by checking HTTP API
#[tauri::command]
pub async fn check_ollama_running() -> Result<bool, String> {
    // Try HTTP API first (more reliable)
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    match client.get("http://localhost:11434/api/tags").send().await {
        Ok(response) => Ok(response.status().is_success()),
        Err(_) => {
            // Fallback to command check (Windows-safe)
            let result = CommandBuilder::new("ollama")
                .arg("list")
                .output();
            
            match result {
                Ok(output) => Ok(output.status.success()),
                Err(_) => Ok(false),
            }
        }
    }
}

/// Ollama model information
#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
    pub digest: String,
}

/// Ollama models list response
#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaModelsResponse {
    pub models: Vec<OllamaModel>,
}

/// Running model information
#[derive(Debug, Serialize, Deserialize)]
pub struct RunningModelInfo {
    pub name: String,
    pub model: String,
    pub size: u64,
    pub expires_at: Option<String>,
}

/// Running models response
#[derive(Debug, Serialize, Deserialize)]
pub struct RunningModelsResponse {
    pub models: Vec<RunningModelInfo>,
}

/// List Ollama models
#[tauri::command]
pub async fn ollama_list() -> Result<OllamaModelsResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Ollama API error: {}", response.status()));
    }
    
    let models_response: OllamaModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    Ok(models_response)
}

/// Pull an Ollama model
#[tauri::command]
pub async fn ollama_pull(model: String, app: tauri::AppHandle) -> Result<String, String> {
    // First check if Ollama is running
    let is_running = check_ollama_running().await.map_err(|e| format!("Failed to check Ollama status: {}", e))?;
    if !is_running {
        return Err("Ollama is not running. Please start Ollama first.".to_string());
    }
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600)) // 10 minutes for large models
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let request_body = serde_json::json!({
        "name": model
    });
    
    let response = client
        .post("http://localhost:11434/api/pull")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to pull model: {}", e))?;
    
    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama API error: {} - {}", status, error_text));
    }
    
    // Parse streaming JSON response and emit progress events
    // Ollama returns newline-delimited JSON (NDJSON) where each line is a JSON object
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut last_progress = 0.0;
    let mut last_chunk_time = std::time::Instant::now();
    let mut is_complete = false;
    const CHUNK_TIMEOUT_SECS: u64 = 120; // 2 minutes timeout per chunk
    
    loop {
        match tokio::time::timeout(
            std::time::Duration::from_secs(CHUNK_TIMEOUT_SECS),
            stream.next()
        ).await {
            Ok(Some(chunk_result)) => {
                match chunk_result {
                    Ok(bytes) => {
                        last_chunk_time = std::time::Instant::now();
                        let new_data = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&new_data);
                        
                        // Process complete lines (lines ending with \n)
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].trim().to_string();
                            let remaining = buffer[newline_pos + 1..].to_string();
                            buffer = remaining;
                            
                            if !line.is_empty() {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                                    // Extract status
                                    let status_str = json.get("status").and_then(|s| s.as_str()).unwrap_or("");
                                    
                                    // Check if pull is complete
                                    if status_str == "success" || status_str.contains("complete") {
                                        is_complete = true;
                                    }
                                    
                                    // Check for error status
                                    if let Some(error_msg) = json.get("error").and_then(|e| e.as_str()) {
                                        return Err(format!("Ollama pull error: {}", error_msg));
                                    }
                                    
                                    // Try to extract progress percentage
                                    let mut progress_opt = None;
                                    if let Some(completed) = json.get("completed").and_then(|c| c.as_u64()) {
                                        if let Some(total) = json.get("total").and_then(|t| t.as_u64()) {
                                            if total > 0 {
                                                let progress = (completed as f64 / total as f64) * 100.0;
                                                // Only update if progress changed significantly (>0.5%)
                                                if (progress - last_progress).abs() > 0.5 {
                                                    progress_opt = Some(progress);
                                                    last_progress = progress;
                                                }
                                            }
                                        }
                                    }
                                    
                                    // Emit progress event
                                    let mut event_data = serde_json::json!({
                                        "model": model,
                                        "status": status_str,
                                    });
                                    
                                    if let Some(progress) = progress_opt {
                                        event_data["progress"] = serde_json::json!(progress);
                                        if let Some(completed) = json.get("completed") {
                                            event_data["completed"] = completed.clone();
                                        }
                                        if let Some(total) = json.get("total") {
                                            event_data["total"] = total.clone();
                                        }
                                    }
                                    
                                    // Emit to all windows
                                    let _ = app.emit("ollama-pull-progress", event_data);
                                    
                                    // If complete, break early
                                    if is_complete {
                                        break;
                                    }
                                }
                            }
                        }
                        
                        // If we detected completion, break out of the loop
                        if is_complete {
                            break;
                        }
                    }
                    Err(e) => {
                        return Err(format!("Stream error: {}", e));
                    }
                }
            }
            Ok(None) => {
                // Stream ended naturally
                break;
            }
            Err(_) => {
                // Timeout waiting for chunk
                let elapsed = last_chunk_time.elapsed();
                return Err(format!(
                    "Timeout waiting for Ollama response (no data received for {} seconds). The download may have stalled. Please check your network connection and Ollama status.",
                    elapsed.as_secs()
                ));
            }
        }
    }
    
    // Process any remaining data in buffer (last line without newline)
    if !buffer.trim().is_empty() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(buffer.trim()) {
            if let Some(status) = json.get("status").and_then(|s| s.as_str()) {
                let _ = app.emit("ollama-pull-progress", serde_json::json!({
                    "model": model,
                    "status": status,
                }));
                
                // Check for error in final message
                if let Some(error_msg) = json.get("error").and_then(|e| e.as_str()) {
                    return Err(format!("Ollama pull error: {}", error_msg));
                }
            }
        }
    }
    
    // Verify completion by checking if model exists
    if !is_complete {
        // Give it a moment, then verify the model was actually pulled
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        match ollama_list().await {
            Ok(models_response) => {
                let model_names: Vec<String> = models_response
                    .models
                    .into_iter()
                    .map(|m| m.name.to_lowercase())
                    .collect();
                let model_lower = model.to_lowercase();
                let found = model_names.iter().any(|n: &String| {
                    n == &model_lower || 
                    n == &format!("{}:latest", model_lower) ||
                    n.contains(&model_lower)
                });
                if !found {
                    return Err(format!(
                        "Model download may have failed. Model '{}' not found after pull completed.",
                        model
                    ));
                }
            }
            Err(e) => {
                // If we can't verify, assume it worked but log a warning
                tracing::warn!("Could not verify model installation: {}", e);
            }
        }
    }
    
    // Emit completion event
    let _ = app.emit("ollama-pull-complete", serde_json::json!({
        "model": model,
    }));
    
    Ok(format!("Model {} pulled successfully", model))
}

/// List running Ollama models
#[tauri::command]
pub async fn ollama_ps() -> Result<RunningModelsResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client
        .get("http://localhost:11434/api/ps")
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Ollama API error: {}", response.status()));
    }
    
    let running_response: RunningModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;
    
    Ok(running_response)
}

/// Run an Ollama model
#[tauri::command]
pub async fn ollama_run(model: String, resource_limits: Option<serde_json::Value>) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let mut request_body = serde_json::json!({
        "model": model,
        "prompt": "",
        "stream": false
    });
    
    if let Some(limits) = resource_limits {
        request_body["options"] = limits;
    }
    
    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to run model: {}", e))?;
    
    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama API error: {} - {}", status, error_text));
    }
    
    Ok(format!("Model {} started successfully", model))
}

/// Stop an Ollama model
#[tauri::command]
pub async fn ollama_stop(model: String) -> Result<String, String> {
    // Ollama doesn't have a direct stop API endpoint
    // We need to use the command line to stop the model (Windows-safe)
    let result = CommandBuilder::new("ollama")
        .args(["stop", &model])
        .output();
    
    match result {
        Ok(output) => {
            if output.status.success() {
                Ok(format!("Model {} stopped successfully", model))
            } else {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                Err(format!("Failed to stop model: {}", error_msg))
            }
        }
        Err(e) => Err(format!("Failed to execute ollama stop: {}", e)),
    }
}

/// Delete an Ollama model
#[tauri::command]
pub async fn ollama_delete(model: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let request_body = serde_json::json!({
        "name": model
    });
    
    let response = client
        .delete("http://localhost:11434/api/delete")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Failed to delete model: {}", e))?;
    
    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama API error: {} - {}", status, error_text));
    }
    
    Ok(format!("Model {} deleted successfully", model))
}

/// Start Ollama service
#[tauri::command]
pub async fn ollama_serve() -> Result<String, String> {
    // On macOS and Linux, try to start Ollama service (Windows-safe)
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let result = CommandBuilder::new("ollama")
            .arg("serve")
            .spawn();
        
        match result {
            Ok(_) => Ok("Ollama service started".to_string()),
            Err(e) => Err(format!("Failed to start Ollama: {}", e)),
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        // On Windows, Ollama should start automatically after installation
        // Try to verify it's running
        match check_ollama_running().await {
            Ok(true) => Ok("Ollama is running".to_string()),
            Ok(false) => Err("Ollama is not running. Please start it manually from the Start menu.".to_string()),
            Err(e) => Err(format!("Failed to check Ollama status: {}", e)),
        }
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err("Unsupported platform".to_string())
    }
}

/// Check if FFmpeg is installed by checking common locations
#[tauri::command]
pub async fn check_ffmpeg_installed() -> Result<bool, String> {
    // Common FFmpeg installation paths (especially on macOS)
    let candidates = if cfg!(target_os = "macos") {
        vec![
            "ffmpeg", // In PATH
            "/opt/homebrew/bin/ffmpeg",
            "/usr/local/bin/ffmpeg",
            "/usr/bin/ffmpeg",
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            "ffmpeg", // In PATH
            "C:\\ffmpeg\\bin\\ffmpeg.exe",
            "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
        ]
    } else {
        vec![
            "ffmpeg", // In PATH
            "/usr/bin/ffmpeg",
            "/usr/local/bin/ffmpeg",
        ]
    };
    
    // Check each candidate location
    for candidate in candidates {
        let result = CommandBuilder::new(candidate)
            .arg("-version")
            .stdout_null()
            .stderr_null()
            .status();
        
        if let Ok(status) = result {
            if status.success() {
                return Ok(true);
            }
        }
    }
    
    Ok(false)
}

/// Get installation instructions for Ollama based on platform
#[tauri::command]
pub fn get_ollama_install_instructions() -> Result<InstallInstructions, String> {
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("Unsupported platform".to_string());
    };

    let instructions = match platform {
        "macos" => InstallInstructions {
            platform: "macOS".to_string(),
            method: "Homebrew".to_string(),
            command: "brew install ollama".to_string(),
            alternative: Some("Download from https://ollama.ai/download".to_string()),
            url: Some("https://ollama.ai/download/Ollama-darwin.zip".to_string()),
        },
        "windows" => InstallInstructions {
            platform: "Windows".to_string(),
            method: "Download Installer".to_string(),
            command: "winget install Ollama.Ollama".to_string(),
            alternative: Some("Download from https://ollama.ai/download".to_string()),
            url: Some("https://ollama.ai/download/OllamaSetup.exe".to_string()),
        },
        "linux" => InstallInstructions {
            platform: "Linux".to_string(),
            method: "curl script".to_string(),
            command: "curl -fsSL https://ollama.ai/install.sh | sh".to_string(),
            alternative: Some("Or use your package manager".to_string()),
            url: None,
        },
        _ => return Err("Unsupported platform".to_string()),
    };

    Ok(instructions)
}

/// Get installation instructions for FFmpeg based on platform
#[tauri::command]
pub fn get_ffmpeg_install_instructions() -> Result<InstallInstructions, String> {
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("Unsupported platform".to_string());
    };

    let instructions = match platform {
        "macos" => InstallInstructions {
            platform: "macOS".to_string(),
            method: "Homebrew".to_string(),
            command: "brew install ffmpeg".to_string(),
            alternative: None,
            url: None,
        },
        "windows" => InstallInstructions {
            platform: "Windows".to_string(),
            method: "winget".to_string(),
            command: "winget install ffmpeg".to_string(),
            alternative: Some("Or download from https://ffmpeg.org/download.html".to_string()),
            url: Some("https://ffmpeg.org/download.html".to_string()),
        },
        "linux" => InstallInstructions {
            platform: "Linux".to_string(),
            method: "Package Manager".to_string(),
            command: "sudo apt-get install ffmpeg".to_string(),
            alternative: Some("Or: sudo yum install ffmpeg / sudo pacman -S ffmpeg".to_string()),
            url: None,
        },
        _ => return Err("Unsupported platform".to_string()),
    };

    Ok(instructions)
}

/// Attempt to install Docker automatically (if possible)
#[tauri::command]
pub async fn install_docker() -> Result<InstallResult, String> {
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("Unsupported platform".to_string());
    };

    match platform {
        "macos" => {
            let brew = match crate::vfs::platform::resolve_brew_path() {
                Some(path) => path,
                None => {
                    return Ok(InstallResult {
                        success: false,
                        message: "Homebrew not found. Please install Docker manually:\n\n1. Install Homebrew: https://brew.sh\n2. Then run: brew install --cask docker\n\nOr download from: https://docker.com".to_string(),
                        requires_restart: false,
                    });
                }
            };

            let brew_result = CommandBuilder::new(&brew)
                .args(["install", "--cask", "docker"])
                .output();
            
            if let Ok(output) = brew_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "Docker installed successfully via Homebrew. Please launch Docker Desktop from Applications.".to_string(),
                        requires_restart: true,
                    });
                }
            }
            
            Ok(InstallResult {
                success: false,
                message: "Please install Docker manually. Run: brew install --cask docker or download from docker.com".to_string(),
                requires_restart: false,
            })
        },
        "windows" => {
            // Try winget (Windows-safe, no terminal window)
            let winget_result = CommandBuilder::new("winget")
                .args(["install", "Docker.DockerDesktop"])
                .output();
            
            if let Ok(output) = winget_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "Docker installed successfully via winget. Please launch Docker Desktop.".to_string(),
                        requires_restart: true,
                    });
                }
            }
            
            // If winget fails, provide manual instructions
            Ok(InstallResult {
                success: false,
                message: "Please install Docker manually. Download from docker.com/products/docker-desktop".to_string(),
                requires_restart: false,
            })
        },
        "linux" => {
            // Try curl script
            let curl_result = CommandBuilder::new("sh")
                .args(["-c", "curl -fsSL https://get.docker.com -o get-docker.sh && sh get-docker.sh"])
                .output();
            
            if let Ok(output) = curl_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "Docker installed successfully. You may need to add your user to the docker group: sudo usermod -aG docker $USER".to_string(),
                        requires_restart: true,
                    });
                }
            }
            
            // If curl script fails, provide manual instructions
            Ok(InstallResult {
                success: false,
                message: "Please install Docker manually. Run: curl -fsSL https://get.docker.com -o get-docker.sh && sh get-docker.sh".to_string(),
                requires_restart: false,
            })
        },
        _ => Err("Unsupported platform".to_string()),
    }
}

/// Attempt to install Ollama automatically (if possible)
#[tauri::command]
pub async fn install_ollama() -> Result<InstallResult, String> {
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("Unsupported platform".to_string());
    };

    match platform {
        "macos" => {
            let brew = match crate::vfs::platform::resolve_brew_path() {
                Some(path) => path,
                None => {
                    return Ok(InstallResult {
                        success: false,
                        message: "Homebrew not found. Please install Ollama manually:\n\n1. Install Homebrew: https://brew.sh\n2. Then run: brew install ollama\n\nOr download from: https://ollama.com".to_string(),
                        requires_restart: false,
                    });
                }
            };

            let brew_result = CommandBuilder::new(&brew)
                .args(["install", "ollama"])
                .output();
            
            if let Ok(output) = brew_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "Ollama installed successfully via Homebrew".to_string(),
                        requires_restart: true,
                    });
                }
            }
            
            Ok(InstallResult {
                success: false,
                message: "Please install Ollama manually. Run: brew install ollama".to_string(),
                requires_restart: false,
            })
        },
        "windows" => {
            // Try winget (Windows-safe, no terminal window)
            let winget_result = CommandBuilder::new("winget")
                .args(["install", "Ollama.Ollama"])
                .output();
            
            if let Ok(output) = winget_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "Ollama installed successfully via winget".to_string(),
                        requires_restart: true,
                    });
                }
            }
            
            // If winget fails, provide manual instructions
            Ok(InstallResult {
                success: false,
                message: "Please install Ollama manually. Download from https://ollama.ai/download".to_string(),
                requires_restart: false,
            })
        },
        "linux" => {
            // Try curl script
            let curl_result = CommandBuilder::new("sh")
                .args(["-c", "curl -fsSL https://ollama.ai/install.sh | sh"])
                .output();
            
            if let Ok(output) = curl_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "Ollama installed successfully".to_string(),
                        requires_restart: true,
                    });
                }
            }
            
            // If curl script fails, provide manual instructions
            Ok(InstallResult {
                success: false,
                message: "Please install Ollama manually. Run: curl -fsSL https://ollama.ai/install.sh | sh".to_string(),
                requires_restart: false,
            })
        },
        _ => Err("Unsupported platform".to_string()),
    }
}

/// Unified one-click install for FFmpeg + Ollama + Required Models
/// Installs everything needed for transcoding and auto-tagging based on OS
#[tauri::command]
pub async fn install_all_ai_dependencies(app: tauri::AppHandle) -> Result<InstallResult, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    
    // Step 1: Install FFmpeg
    let ffmpeg_result = install_ffmpeg().await;
    match &ffmpeg_result {
        Ok(r) if r.success => {
            results.push("FFmpeg installed successfully".to_string());
        }
        Ok(r) => {
            errors.push(format!("FFmpeg: {}", r.message));
        }
        Err(e) => {
            errors.push(format!("FFmpeg: {}", e));
        }
    }

    // Step 1.5: Install whisper.cpp for transcription
    let whisper_installed = check_whisper_cpp_installed().await.unwrap_or(false);
    if !whisper_installed {
        let whisper_result = install_whisper_cpp().await;
        match &whisper_result {
            Ok(r) if r.success => {
                results.push("whisper.cpp installed successfully".to_string());
            }
            Ok(r) => {
                // Not a critical error - transcription can still work with fallback
                tracing::warn!("whisper.cpp: {}", r.message);
            }
            Err(e) => {
                tracing::warn!("whisper.cpp: {}", e);
            }
        }
    } else {
        results.push("whisper.cpp already installed".to_string());
    }

    // Step 2: Install Ollama
    let ollama_result = install_ollama().await;
    match &ollama_result {
        Ok(r) if r.success => {
            results.push("Ollama installed successfully".to_string());
            
            // Wait a bit for Ollama to start
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            
            // Step 3: Start Ollama service if needed (Windows-safe command execution)
            let is_running = check_ollama_running().await.unwrap_or(false);
            if !is_running {
                // Try to start Ollama
                #[cfg(target_os = "macos")]
                {
                    let _ = CommandBuilder::new("open")
                        .args(["-a", "Ollama"])
                        .output();
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = CommandBuilder::new("ollama")
                        .arg("serve")
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = CommandBuilder::new("systemctl")
                        .args(["--user", "start", "ollama"])
                        .output();
                }
                
                // Wait for Ollama to be ready
                for _ in 0..10 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    if check_ollama_running().await.unwrap_or(false) {
                        break;
                    }
                }
            }
            
            // Step 4: Pull required models
            // Note: Whisper is not available in Ollama - transcription uses FFmpeg's built-in Whisper
            // Only pull tagging models from Ollama
            
            // Verify Ollama is still running before pulling models
            let is_still_running = check_ollama_running().await.unwrap_or(false);
            if !is_still_running {
                errors.push("Ollama stopped running before model download could start. Please ensure Ollama is running and try again.".to_string());
            } else {
                let required_models = vec!["llava"]; // llava for video/image tagging (transcription uses FFmpeg Whisper)
                
                for model in required_models {
                    // Double-check Ollama is running before each pull
                    let is_running_before_pull = check_ollama_running().await.unwrap_or(false);
                    if !is_running_before_pull {
                        errors.push(format!("Ollama stopped running before pulling model '{}'. Please ensure Ollama is running and try again.", model));
                        continue;
                    }
                    
                    let pull_result = ollama_pull(model.to_string(), app.clone()).await;
                    match pull_result {
                        Ok(_) => {
                            results.push(format!("Model '{}' installed successfully", model));
                        }
                        Err(e) => {
                            errors.push(format!("Model '{}': {}", model, e));
                        }
                    }
                }
            }
        }
        Ok(r) => {
            errors.push(format!("Ollama: {}", r.message));
        }
        Err(e) => {
            errors.push(format!("Ollama: {}", e));
        }
    }
    
    if errors.is_empty() {
        Ok(InstallResult {
            success: true,
            message: format!("All dependencies installed successfully:\n{}", results.join("\n")),
            requires_restart: false,
        })
    } else {
        Ok(InstallResult {
            success: false,
            message: format!("Some installations failed:\n{}\n\nSuccessful:\n{}", 
                errors.join("\n"), 
                results.join("\n")),
            requires_restart: false,
        })
    }
}

/// Attempt to install FFmpeg automatically (if possible)
#[tauri::command]
pub async fn install_ffmpeg() -> Result<InstallResult, String> {
    // First check if FFmpeg is already installed
    let already_installed = check_ffmpeg_installed().await.unwrap_or(false);
    if already_installed {
        return Ok(InstallResult {
            success: true,
            message: "FFmpeg is already installed".to_string(),
            requires_restart: false,
        });
    }

    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("Unsupported platform".to_string());
    };

    match platform {
        "macos" => {
            // Resolve the full brew path (GUI apps don't inherit shell PATH)
            let brew = match crate::vfs::platform::resolve_brew_path() {
                Some(path) => path,
                None => {
                    return Ok(InstallResult {
                        success: false,
                        message: "FFmpeg not found. To install:\n\n1. Install Homebrew: https://brew.sh\n2. Then run: brew install ffmpeg\n\nOr download FFmpeg directly from: https://ffmpeg.org/download.html".to_string(),
                        requires_restart: false,
                    });
                }
            };
            
            // Try to install FFmpeg via Homebrew
            let brew_result = CommandBuilder::new(&brew)
                .args(["install", "ffmpeg"])
                .output();
            
            if let Ok(output) = brew_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "FFmpeg installed successfully via Homebrew".to_string(),
                        requires_restart: false,
                    });
                }
            }
            
            Ok(InstallResult {
                success: false,
                message: "FFmpeg installation failed. Please install manually:\n\nRun: brew install ffmpeg\n\nOr download from: https://ffmpeg.org/download.html".to_string(),
                requires_restart: false,
            })
        },
        "windows" => {
            // Windows-safe, no terminal window
            // First check if winget is available
            let winget_check = CommandBuilder::new("winget")
                .args(["--version"])
                .output();
            
            if winget_check.is_err() {
                return Ok(InstallResult {
                    success: false,
                    message: "winget not found. Please install FFmpeg manually. Download from https://ffmpeg.org/download.html".to_string(),
                    requires_restart: false,
                });
            }
            
            // Try to install FFmpeg via winget
            let winget_result = CommandBuilder::new("winget")
                .args(["install", "ffmpeg", "--accept-source-agreements", "--accept-package-agreements"])
                .output();
            
            if let Ok(output) = winget_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "FFmpeg installed successfully via winget".to_string(),
                        requires_restart: false,
                    });
                }
            }
            
            Ok(InstallResult {
                success: false,
                message: "FFmpeg installation failed. Please install manually. Download from https://ffmpeg.org/download.html or run: winget install ffmpeg".to_string(),
                requires_restart: false,
            })
        },
        "linux" => {
            // Try multiple package managers
            // First check which package manager is available
            let apt_check = CommandBuilder::new("which")
                .args(["apt-get"])
                .output();
            
            if apt_check.is_ok() {
                // Try apt-get (Debian/Ubuntu)
                let apt_result = CommandBuilder::new("sh")
                    .args(["-c", "sudo apt-get update && sudo apt-get install -y ffmpeg"])
                    .output();
                
                if let Ok(output) = apt_result {
                    if output.status.success() {
                        return Ok(InstallResult {
                            success: true,
                            message: "FFmpeg installed successfully via apt-get".to_string(),
                            requires_restart: false,
                        });
                    }
                }
            }
            
            // Try yum (RHEL/CentOS)
            let yum_check = CommandBuilder::new("which")
                .args(["yum"])
                .output();
            
            if yum_check.is_ok() {
                let yum_result = CommandBuilder::new("sh")
                    .args(["-c", "sudo yum install -y ffmpeg"])
                    .output();
                
                if let Ok(output) = yum_result {
                    if output.status.success() {
                        return Ok(InstallResult {
                            success: true,
                            message: "FFmpeg installed successfully via yum".to_string(),
                            requires_restart: false,
                        });
                    }
                }
            }
            
            // Try pacman (Arch)
            let pacman_check = CommandBuilder::new("which")
                .args(["pacman"])
                .output();
            
            if pacman_check.is_ok() {
                let pacman_result = CommandBuilder::new("sh")
                    .args(["-c", "sudo pacman -S --noconfirm ffmpeg"])
                    .output();
                
                if let Ok(output) = pacman_result {
                    if output.status.success() {
                        return Ok(InstallResult {
                            success: true,
                            message: "FFmpeg installed successfully via pacman".to_string(),
                            requires_restart: false,
                        });
                    }
                }
            }
            
            Ok(InstallResult {
                success: false,
                message: "FFmpeg installation failed. Please install manually. Run: sudo apt-get install ffmpeg (Debian/Ubuntu), sudo yum install ffmpeg (RHEL/CentOS), or sudo pacman -S ffmpeg (Arch)".to_string(),
                requires_restart: false,
            })
        },
        _ => Err("Unsupported platform".to_string()),
    }
}

/// Attempt to install whisper.cpp automatically (for speech-to-text transcription)
#[tauri::command]
pub async fn install_whisper_cpp() -> Result<InstallResult, String> {
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("Unsupported platform".to_string());
    };

    match platform {
        "macos" => {
            let brew = match crate::vfs::platform::resolve_brew_path() {
                Some(path) => path,
                None => {
                    return Ok(InstallResult {
                        success: false,
                        message: "Homebrew not found. Please install whisper.cpp manually:\n\n1. Install Homebrew: https://brew.sh\n2. Then run: brew install whisper-cpp".to_string(),
                        requires_restart: false,
                    });
                }
            };

            let brew_result = CommandBuilder::new(&brew)
                .args(["install", "whisper-cpp"])
                .output();

            if let Ok(output) = brew_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "whisper.cpp installed successfully via Homebrew".to_string(),
                        requires_restart: false,
                    });
                }
            }

            Ok(InstallResult {
                success: false,
                message: "Please install whisper.cpp manually. Run: brew install whisper-cpp".to_string(),
                requires_restart: false,
            })
        },
        "windows" => {
            // whisper.cpp needs to be built from source on Windows
            // or use pre-built binaries
            Ok(InstallResult {
                success: false,
                message: "Please install whisper.cpp manually. Download from https://github.com/ggerganov/whisper.cpp/releases".to_string(),
                requires_restart: false,
            })
        },
        "linux" => {
            // Try to build from source or use package manager
            let apt_result = CommandBuilder::new("sh")
                .args(["-c", "which whisper-cpp || (git clone https://github.com/ggerganov/whisper.cpp.git /tmp/whisper-cpp && cd /tmp/whisper-cpp && make && sudo cp main /usr/local/bin/whisper-cpp)"])
                .output();

            if let Ok(output) = apt_result {
                if output.status.success() {
                    return Ok(InstallResult {
                        success: true,
                        message: "whisper.cpp installed successfully".to_string(),
                        requires_restart: false,
                    });
                }
            }

            Ok(InstallResult {
                success: false,
                message: "Please install whisper.cpp manually. See: https://github.com/ggerganov/whisper.cpp".to_string(),
                requires_restart: false,
            })
        },
        _ => Err("Unsupported platform".to_string()),
    }
}

/// Check if whisper.cpp is installed (v1.8+ renamed binary to whisper-cli)
#[tauri::command]
pub async fn check_whisper_cpp_installed() -> Result<bool, String> {
    let candidates = vec![
        "/opt/homebrew/bin/whisper-cli",
        "/usr/local/bin/whisper-cli",
        "/usr/bin/whisper-cli",
        "/opt/homebrew/bin/whisper-cpp",
        "/usr/local/bin/whisper-cpp",
        "/usr/bin/whisper-cpp",
        "/opt/homebrew/bin/main",
        "/usr/local/bin/whisper",
    ];

    for path in candidates {
        let path_buf = std::path::PathBuf::from(path);
        if path_buf.exists() {
            return Ok(true);
        }
    }

    for name in &["whisper-cli", "whisper-cpp"] {
        if let Ok(output) = CommandBuilder::new(*name).arg("--help").output() {
            if output.status.success() || !output.stderr.is_empty() {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

#[derive(Debug, Serialize)]
pub struct InstallInstructions {
    pub platform: String,
    pub method: String,
    pub command: String,
    pub alternative: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InstallResult {
    pub success: bool,
    pub message: String,
    pub requires_restart: bool,
}

// ============================================================================
// AI Resource Management Commands
// ============================================================================

/// Resource limits for transcoding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodingResourceLimits {
    pub threads: i32, // 0 = auto
    pub use_gpu: bool,
    pub gpu_device: i32, // -1 = auto
    pub memory_limit_mb: u32, // 0 = unlimited
    pub preset: String, // ffmpeg preset
    pub max_concurrent_jobs: u32,
}

/// Resource limits for auto-tagging (Ollama)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTaggingResourceLimits {
    pub gpu_memory_limit_gb: f32, // 0 = unlimited
    pub cpu_cores_limit: u32, // 0 = unlimited
    pub system_memory_limit_gb: f32, // 0 = unlimited
    pub gpu_utilization_percent: u32, // 0-100, 0 = unlimited
    pub num_gpu: u32, // 0 = all available
}

// In-memory storage for resource limits (in production, use settings module)
static TRANSCODING_LIMITS: Lazy<Mutex<Option<TranscodingResourceLimits>>> = Lazy::new(|| Mutex::new(None));
static AUTO_TAGGING_LIMITS: Lazy<Mutex<Option<AutoTaggingResourceLimits>>> = Lazy::new(|| Mutex::new(None));

/// Save transcoding resource limits
#[tauri::command]
pub async fn save_transcoding_resource_limits(
    limits: TranscodingResourceLimits,
) -> Result<String, String> {
    let mut stored = TRANSCODING_LIMITS.lock().map_err(|e| e.to_string())?;
    *stored = Some(limits);
    Ok("Transcoding resource limits saved".to_string())
}

/// Load transcoding resource limits
#[tauri::command]
pub async fn load_transcoding_resource_limits() -> Result<TranscodingResourceLimits, String> {
    let stored = TRANSCODING_LIMITS.lock().map_err(|e| e.to_string())?;
    Ok(stored.clone().unwrap_or(TranscodingResourceLimits {
        // Conservative defaults for basic transcoding without taking too much resources
        threads: 2, // Use 2 threads (good balance for basic transcoding)
        use_gpu: true, // Use GPU if available
        gpu_device: -1, // Auto-select GPU
        memory_limit_mb: 2048, // Limit to 2GB memory
        preset: "fast".to_string(), // Fast preset for lower resource usage
        max_concurrent_jobs: 1, // One job at a time
    }))
}

/// Save auto-tagging resource limits
#[tauri::command]
pub async fn save_auto_tagging_resource_limits(
    limits: AutoTaggingResourceLimits,
) -> Result<String, String> {
    let mut stored = AUTO_TAGGING_LIMITS.lock().map_err(|e| e.to_string())?;
    *stored = Some(limits);
    Ok("Auto-tagging resource limits saved".to_string())
}

/// Load auto-tagging resource limits
#[tauri::command]
pub async fn load_auto_tagging_resource_limits() -> Result<AutoTaggingResourceLimits, String> {
    let stored = AUTO_TAGGING_LIMITS.lock().map_err(|e| e.to_string())?;
    Ok(stored.clone().unwrap_or(AutoTaggingResourceLimits {
        gpu_memory_limit_gb: 0.0,
        cpu_cores_limit: 0,
        system_memory_limit_gb: 0.0,
        gpu_utilization_percent: 0,
        num_gpu: 0,
    }))
}

// ============================================================================
// Token Management Commands
// ============================================================================

use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Utc, Duration};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub total: i64,
    pub used: i64,
    pub remaining: i64,
    pub reset_date: String, // ISO 8601 date string
    pub is_paid: bool,
    pub plan_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPlan {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub tokens_per_month: i64, // -1 for unlimited
    pub features: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenStorage {
    account_id: String,
    balance: TokenBalance,
}

// In-memory token storage with persistent backing
static TOKEN_STORAGE: Lazy<RwLock<HashMap<String, TokenBalance>>> = Lazy::new(|| {
    RwLock::new(HashMap::new())
});

// Account ID storage (device-based, persistent)
static ACCOUNT_ID: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

const FREE_TIER_TOKENS: i64 = 1000;

/// Get or create account ID (device-based)
async fn get_account_id() -> Result<String, String> {
    {
        let account_id = ACCOUNT_ID.read();
        if let Some(id) = account_id.as_ref() {
            return Ok(id.clone());
        }
    }
    
    // Try to load from disk
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ursly")
        .join("vfs");
    
    fs::create_dir_all(&data_dir).await
        .map_err(|e| format!("Failed to create data directory: {}", e))?;
    
    let account_file = data_dir.join("account_id.json");
    
    if account_file.exists() {
        if let Ok(content) = fs::read_to_string(&account_file).await {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(id) = data.get("account_id").and_then(|v| v.as_str()) {
                    let mut account_id = ACCOUNT_ID.write();
                    *account_id = Some(id.to_string());
                    return Ok(id.to_string());
                }
            }
        }
    }
    
    // Generate new account ID
    let new_id = Uuid::new_v4().to_string();
    
    // Save to disk
    let account_data = serde_json::json!({
        "account_id": new_id,
        "created_at": Utc::now().to_rfc3339(),
    });
    
    fs::write(&account_file, serde_json::to_string_pretty(&account_data).unwrap())
        .await
        .map_err(|e| format!("Failed to save account ID: {}", e))?;
    
    let mut account_id = ACCOUNT_ID.write();
    *account_id = Some(new_id.clone());
    
    Ok(new_id)
}

/// Get token storage file path
fn get_token_storage_path() -> Result<PathBuf, String> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ursly")
        .join("vfs");
    
    Ok(data_dir.join("tokens.json"))
}

/// Load token balance from disk
async fn load_token_balance(account_id: &str) -> Result<Option<TokenBalance>, String> {
    let storage_path = get_token_storage_path()?;
    
    if !storage_path.exists() {
        return Ok(None);
    }
    
    let content = fs::read_to_string(&storage_path).await
        .map_err(|e| format!("Failed to read token storage: {}", e))?;
    
    let storage: TokenStorage = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse token storage: {}", e))?;
    
    if storage.account_id == account_id {
        Ok(Some(storage.balance))
    } else {
        Ok(None)
    }
}

/// Save token balance to disk
async fn save_token_balance(account_id: &str, balance: &TokenBalance) -> Result<(), String> {
    let storage_path = get_token_storage_path()?;
    
    // Ensure parent directory exists
    if let Some(parent) = storage_path.parent() {
        fs::create_dir_all(parent).await
            .map_err(|e| format!("Failed to create data directory: {}", e))?;
    }
    
    let storage = TokenStorage {
        account_id: account_id.to_string(),
        balance: balance.clone(),
    };
    
    let content = serde_json::to_string_pretty(&storage)
        .map_err(|e| format!("Failed to serialize token storage: {}", e))?;
    
    fs::write(&storage_path, content).await
        .map_err(|e| format!("Failed to write token storage: {}", e))?;
    
    Ok(())
}

/// Initialize token balance for new account (free tier)
fn initialize_free_tier_balance() -> TokenBalance {
    let now = Utc::now();
    let next_month = now + Duration::days(30);
    
    TokenBalance {
        total: FREE_TIER_TOKENS,
        used: 0,
        remaining: FREE_TIER_TOKENS,
        reset_date: next_month.to_rfc3339(),
        is_paid: false,
        plan_id: "free".to_string(),
    }
}

/// Check if token balance needs reset (monthly reset)
fn check_and_reset_balance(balance: &mut TokenBalance) -> Result<bool, String> {
    let reset_date = DateTime::parse_from_rfc3339(&balance.reset_date)
        .map_err(|e| format!("Invalid reset date: {}", e))?
        .with_timezone(&Utc);
    
    let now = Utc::now();
    
    if now >= reset_date {
        // Reset monthly tokens
        let plan_id = balance.plan_id.clone();
        let tokens_per_month = if plan_id == "free" {
            FREE_TIER_TOKENS
        } else if plan_id == "pro" {
            10000
        } else if plan_id == "unlimited" {
            -1 // Unlimited
        } else {
            FREE_TIER_TOKENS // Default to free
        };
        
        let next_month = now + Duration::days(30);
        
        balance.total = if tokens_per_month == -1 {
            -1 // Unlimited
        } else {
            tokens_per_month
        };
        balance.used = 0;
        balance.remaining = balance.total;
        balance.reset_date = next_month.to_rfc3339();
        
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Get or initialize token balance
async fn get_or_init_token_balance() -> Result<TokenBalance, String> {
    let account_id = get_account_id().await?;
    
    // Check in-memory cache first
    {
        let storage = TOKEN_STORAGE.read();
        if let Some(balance) = storage.get(&account_id) {
            let mut balance = balance.clone();
            // Check if reset is needed
            let _ = check_and_reset_balance(&mut balance);
            return Ok(balance);
        }
    }
    
    // Load from disk
    if let Some(mut balance) = load_token_balance(&account_id).await? {
        // Check if reset is needed
        check_and_reset_balance(&mut balance)?;
        
        // Update cache
        {
            let mut storage = TOKEN_STORAGE.write();
            storage.insert(account_id.clone(), balance.clone());
        }
        
        // Save if reset occurred
        save_token_balance(&account_id, &balance).await?;
        
        return Ok(balance);
    }
    
    // Initialize new account with free tier
    let balance = initialize_free_tier_balance();
    
    // Save to disk and cache
    save_token_balance(&account_id, &balance).await?;
    {
        let mut storage = TOKEN_STORAGE.write();
        storage.insert(account_id, balance.clone());
    }
    
    Ok(balance)
}

/// Get current token balance
#[tauri::command]
pub async fn get_token_balance() -> Result<TokenBalance, String> {
    get_or_init_token_balance().await
}

/// Consume tokens for an operation
#[tauri::command]
pub async fn consume_tokens(
    tokens: i64,
    operation: String,
    description: Option<String>,
) -> Result<bool, String> {
    if tokens <= 0 {
        return Err("Token amount must be positive".to_string());
    }
    
    let account_id = get_account_id().await?;
    let mut balance = get_or_init_token_balance().await?;
    
    // Check if unlimited
    if balance.total == -1 {
        // Unlimited plan - no need to consume
        return Ok(true);
    }
    
    // Check if enough tokens available
    if balance.remaining < tokens {
        return Err(format!(
            "Insufficient tokens. Required: {}, Available: {}",
            tokens, balance.remaining
        ));
    }
    
    // Consume tokens
    balance.used += tokens;
    balance.remaining -= tokens;
    
    // Update cache and save to disk
    {
        let mut storage = TOKEN_STORAGE.write();
        storage.insert(account_id.clone(), balance.clone());
    }
    
    save_token_balance(&account_id, &balance).await?;
    
    tracing::info!(
        "Consumed {} tokens for operation: {} (description: {:?})",
        tokens,
        operation,
        description
    );
    
    Ok(true)
}

/// Get available token plans
#[tauri::command]
pub async fn get_token_plans() -> Result<Vec<TokenPlan>, String> {
    Ok(vec![
        TokenPlan {
            id: "free".to_string(),
            name: "Free".to_string(),
            price: 0.0,
            tokens_per_month: 1000,
            features: vec![
                "1,000 tokens/month".to_string(),
                "Basic AI features".to_string(),
                "Community support".to_string(),
            ],
        },
        TokenPlan {
            id: "pro".to_string(),
            name: "Pro".to_string(),
            price: 9.99,
            tokens_per_month: 10000,
            features: vec![
                "10,000 tokens/month".to_string(),
                "All AI features".to_string(),
                "Priority support".to_string(),
                "Advanced transcription".to_string(),
            ],
        },
        TokenPlan {
            id: "unlimited".to_string(),
            name: "Unlimited".to_string(),
            price: 29.99,
            tokens_per_month: -1, // Unlimited
            features: vec![
                "Unlimited tokens".to_string(),
                "All AI features".to_string(),
                "Priority support".to_string(),
                "Advanced transcription".to_string(),
                "Custom models".to_string(),
            ],
        },
    ])
}

/// Purchase a token plan (upgrade subscription)
#[tauri::command]
pub async fn purchase_token_plan(plan_id: String) -> Result<bool, String> {
    // Get available plans
    let plans = get_token_plans().await?;
    let plan = plans.iter()
        .find(|p| p.id == plan_id)
        .ok_or_else(|| format!("Plan not found: {}", plan_id))?;
    
    // In production, integrate with payment provider (Stripe, etc.)
    // For now, simulate successful purchase
    
    let account_id = get_account_id().await?;
    let mut balance = get_or_init_token_balance().await?;
    
    // Update plan
    balance.plan_id = plan_id.clone();
    balance.is_paid = plan.price > 0.0;
    
    // Reset tokens for new plan
    let tokens_per_month = plan.tokens_per_month;
    let now = Utc::now();
    let next_month = now + Duration::days(30);
    
    balance.total = if tokens_per_month == -1 {
        -1 // Unlimited
    } else {
        tokens_per_month
    };
    
    // Keep used tokens, but reset remaining based on new total
    balance.remaining = if balance.total == -1 {
        -1 // Unlimited
    } else {
        balance.total - balance.used
    };
    
    balance.reset_date = next_month.to_rfc3339();
    
    // Update cache and save to disk
    {
        let mut storage = TOKEN_STORAGE.write();
        storage.insert(account_id.clone(), balance.clone());
    }
    
    save_token_balance(&account_id, &balance).await?;
    
    tracing::info!("Upgraded account {} to plan: {}", account_id, plan_id);
    
    Ok(true)
}

// ============================================================================
// Logging Commands
// ============================================================================

use crate::logging;

/// Get application logs
#[tauri::command]
pub async fn get_logs(
    limit: Option<usize>,
    level_filter: Option<String>,
) -> Result<Vec<logging::LogEntry>, String> {
    let settings = crate::settings::get_settings();
    let log_dir = settings.get_log_directory();
    
    let level_filter_str = level_filter.as_deref();
    logging::read_logs(&log_dir, limit, level_filter_str)
        .map_err(|e| format!("Failed to read logs: {}", e))
}

/// Clear application logs
#[tauri::command]
pub async fn clear_logs() -> Result<String, String> {
    let settings = crate::settings::get_settings();
    let log_dir = settings.get_log_directory();
    
    logging::clear_logs(&log_dir)
        .map_err(|e| format!("Failed to clear logs: {}", e))?;
    
    Ok("Logs cleared".to_string())
}

/// Get log file path
#[tauri::command]
pub async fn get_log_file_path() -> Result<String, String> {
    let settings = crate::settings::get_settings();
    let log_dir = settings.get_log_directory();
    let log_file = log_dir.join("ursly.log");
    Ok(log_file.to_string_lossy().to_string())
}

// ============================================================================
// Settings Commands
// ============================================================================

use crate::settings;

/// Get all settings
#[tauri::command]
pub async fn get_settings() -> Result<settings::AppSettings, String> {
    Ok(settings::get_settings().get_all())
}

/// Get logging settings
#[tauri::command]
pub async fn get_logging_settings() -> Result<settings::LoggingSettings, String> {
    Ok(settings::get_settings().get_logging())
}

/// Update logging settings
#[tauri::command]
pub async fn update_logging_settings(
    log_path: Option<String>,
    log_level: Option<String>,
    max_file_size: Option<u64>,
    max_rotated_files: Option<usize>,
    enable_file_logging: Option<bool>,
) -> Result<String, String> {
    let settings_mgr = settings::get_settings();
    settings_mgr.update_logging(|logging| {
        if let Some(path) = log_path {
            logging.log_path = Some(path);
        }
        if let Some(level) = log_level {
            logging.log_level = Some(level);
        }
        if let Some(size) = max_file_size {
            logging.max_file_size = Some(size);
        }
        if let Some(files) = max_rotated_files {
            logging.max_rotated_files = Some(files);
        }
        if let Some(enabled) = enable_file_logging {
            logging.enable_file_logging = Some(enabled);
        }
    })
    .map_err(|e| format!("Failed to update logging settings: {}", e))?;
    
    Ok("Logging settings updated".to_string())
}

/// Get UI settings
#[tauri::command]
pub async fn get_ui_settings() -> Result<settings::UiSettings, String> {
    Ok(settings::get_settings().get_ui())
}

/// Update UI settings
#[tauri::command]
pub async fn update_ui_settings(
    theme: Option<String>,
    default_view: Option<String>,
    show_hidden_files: Option<bool>,
) -> Result<String, String> {
    let settings_mgr = settings::get_settings();
    settings_mgr.update_ui(|ui| {
        if let Some(t) = theme {
            ui.theme = Some(t);
        }
        if let Some(view) = default_view {
            ui.default_view = Some(view);
        }
        if let Some(show) = show_hidden_files {
            ui.show_hidden_files = Some(show);
        }
    })
    .map_err(|e| format!("Failed to update UI settings: {}", e))?;
    
    Ok("UI settings updated".to_string())
}

/// Reset settings to defaults
#[tauri::command]
pub async fn reset_settings() -> Result<String, String> {
    settings::get_settings()
        .reset()
        .map_err(|e| format!("Failed to reset settings: {}", e))?;
    Ok("Settings reset to defaults".to_string())
}
