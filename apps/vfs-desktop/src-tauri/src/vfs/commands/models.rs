//! Model Management Commands
//!
//! Tauri commands for managing AI models with categorization support.

use tracing::info;

use crate::vfs::adapters::model_manager::{InMemoryModelRegistry, ModelManager};
use crate::vfs::domain::value_objects::ModelCategory;
use crate::vfs::ports::model::{ModelMetadata, ModelOperationResult};

use std::sync::OnceLock;
use tokio::sync::Mutex as TokioMutex;

// Global model manager instance
static MODEL_MANAGER: OnceLock<TokioMutex<ModelManager>> = OnceLock::new();

async fn get_model_manager() -> tokio::sync::MutexGuard<'static, ModelManager> {
    MODEL_MANAGER.get_or_init(|| {
        let registry = std::sync::Arc::new(InMemoryModelRegistry::with_defaults());
        TokioMutex::new(ModelManager::new(registry))
    }).lock().await
}

/// List all available models
#[tauri::command]
pub async fn vfs_list_models() -> Result<Vec<ModelMetadata>, String> {
    info!("[vfs_list_models] Listing all models");
    
    let manager = get_model_manager().await;
    manager.list_all_models().await
        .map_err(|e| format!("Failed to list models: {}", e))
}

/// List models by category
#[tauri::command]
pub async fn vfs_list_models_by_category(
    category: String,
) -> Result<Vec<ModelMetadata>, String> {
    let model_category = ModelCategory::from_str(&category);
    info!("[vfs_list_models_by_category] Listing models for category: {:?}", model_category);
    
    let manager = get_model_manager().await;
    manager.list_models_by_category(model_category).await
        .map_err(|e| format!("Failed to list models by category: {}", e))
}

/// Get model by ID
#[tauri::command]
pub async fn vfs_get_model(model_id: String) -> Result<Option<ModelMetadata>, String> {
    info!("[vfs_get_model] Getting model: {}", model_id);
    
    let manager = get_model_manager().await;
    manager.get_model(&model_id).await
        .map_err(|e| format!("Failed to get model: {}", e))
}

/// Install a model
#[tauri::command]
pub async fn vfs_install_model(
    model_id: String,
) -> Result<ModelOperationResult, String> {
    info!("[vfs_install_model] Installing model: {}", model_id);
    
    // TODO: Implement actual installation with progress callbacks
    // For now, this is a placeholder that would need to integrate with Ollama API
    Err("Model installation not yet implemented. Use Ollama directly for now.".to_string())
}

/// Uninstall a model
#[tauri::command]
pub async fn vfs_uninstall_model(
    model_id: String,
) -> Result<ModelOperationResult, String> {
    info!("[vfs_uninstall_model] Uninstalling model: {}", model_id);
    
    // TODO: Implement actual uninstallation
    Err("Model uninstallation not yet implemented. Use Ollama directly for now.".to_string())
}

/// Start serving a model
#[tauri::command]
pub async fn vfs_start_model(
    model_id: String,
) -> Result<ModelOperationResult, String> {
    info!("[vfs_start_model] Starting model: {}", model_id);
    
    // TODO: Implement actual model starting
    Err("Model starting not yet implemented. Use Ollama directly for now.".to_string())
}

/// Stop serving a model
#[tauri::command]
pub async fn vfs_stop_model(
    model_id: String,
) -> Result<ModelOperationResult, String> {
    info!("[vfs_stop_model] Stopping model: {}", model_id);
    
    // TODO: Implement actual model stopping
    Err("Model stopping not yet implemented. Use Ollama directly for now.".to_string())
}

/// Get all model categories
#[tauri::command]
pub fn vfs_get_model_categories() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": "transcription",
            "name": "Transcription",
            "icon": "microphone",
            "description": "Speech-to-text transcription models"
        }),
        serde_json::json!({
            "id": "video_tagging",
            "name": "Video Tagging",
            "icon": "video",
            "description": "Automatic video content tagging and analysis"
        }),
        serde_json::json!({
            "id": "image_tagging",
            "name": "Image Tagging",
            "icon": "image",
            "description": "Image content tagging and classification"
        }),
        serde_json::json!({
            "id": "text_generation",
            "name": "Text Generation",
            "icon": "text",
            "description": "Text generation and language models"
        }),
        serde_json::json!({
            "id": "code_generation",
            "name": "Code Generation",
            "icon": "code",
            "description": "Code generation and programming assistance"
        }),
        serde_json::json!({
            "id": "embedding",
            "name": "Embeddings",
            "icon": "search",
            "description": "Semantic search and embedding models"
        }),
        serde_json::json!({
            "id": "other",
            "name": "Other",
            "icon": "box",
            "description": "Other models"
        }),
    ]
}
