//! Model Port - Abstraction layer for AI model management
//!
//! Provides a unified interface for managing AI models across different providers
//! (Ollama, local models, etc.) with categorization support.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::vfs::domain::value_objects::ModelCategory;

/// Model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Model identifier (e.g., "whisper", "llama3.2")
    pub id: String,
    
    /// Display name
    pub name: String,
    
    /// Model category
    pub category: ModelCategory,
    
    /// Model size in bytes
    pub size_bytes: u64,
    
    /// Model description
    pub description: Option<String>,
    
    /// Provider (e.g., "ollama", "local", "huggingface")
    pub provider: String,
    
    /// Model version/tag
    pub version: Option<String>,
    
    /// Whether the model is installed locally
    pub is_installed: bool,
    
    /// Whether the model is currently running/serving
    pub is_running: bool,
    
    /// Model file path (if local)
    pub local_path: Option<PathBuf>,
    
    /// Required dependencies or prerequisites
    pub dependencies: Vec<String>,
    
    /// Supported languages (for transcription/tagging models)
    pub supported_languages: Vec<String>,
    
    /// Model capabilities (what it can do)
    pub capabilities: Vec<String>,
}

/// Model download/install progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProgress {
    /// Current status message
    pub status: String,
    
    /// Total bytes to download
    pub total_bytes: Option<u64>,
    
    /// Bytes downloaded so far
    pub downloaded_bytes: Option<u64>,
    
    /// Progress percentage (0.0 to 1.0)
    pub progress: f32,
    
    /// Current operation (e.g., "downloading", "installing", "verifying")
    pub operation: String,
}

/// Model operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOperationResult {
    /// Whether the operation succeeded
    pub success: bool,
    
    /// Error message if failed
    pub error: Option<String>,
    
    /// Model metadata after operation
    pub model: Option<ModelMetadata>,
}

/// Model provider trait - abstraction for different model providers
#[async_trait]
pub trait IModelProvider: Send + Sync {
    /// Provider name (e.g., "ollama", "local")
    fn provider_name(&self) -> &str;
    
    /// Check if the provider is available
    async fn is_available(&self) -> bool;
    
    /// List all available models (installed and available to install)
    async fn list_models(&self) -> anyhow::Result<Vec<ModelMetadata>>;
    
    /// List models by category
    async fn list_models_by_category(&self, category: ModelCategory) -> anyhow::Result<Vec<ModelMetadata>> {
        let models = self.list_models().await?;
        Ok(models.into_iter()
            .filter(|m| m.category == category)
            .collect())
    }
    
    /// Get model metadata by ID
    async fn get_model(&self, model_id: &str) -> anyhow::Result<Option<ModelMetadata>>;
    
    /// Download and install a model
    /// Returns a stream of progress updates
    async fn install_model(
        &self,
        model_id: &str,
        on_progress: Option<Box<dyn Fn(ModelProgress) + Send>>,
    ) -> anyhow::Result<ModelOperationResult>;
    
    /// Uninstall a model
    async fn uninstall_model(&self, model_id: &str) -> anyhow::Result<ModelOperationResult>;
    
    /// Start serving a model (make it available for inference)
    async fn start_model(&self, model_id: &str) -> anyhow::Result<ModelOperationResult>;
    
    /// Stop serving a model
    async fn stop_model(&self, model_id: &str) -> anyhow::Result<ModelOperationResult>;
    
    /// Check if a model is running
    async fn is_model_running(&self, model_id: &str) -> anyhow::Result<bool>;
    
    /// Get running models
    async fn get_running_models(&self) -> anyhow::Result<Vec<ModelMetadata>>;
}

/// Model registry - manages model metadata and categorization
pub trait IModelRegistry: Send + Sync {
    /// Register a model with metadata
    fn register_model(&self, metadata: ModelMetadata) -> anyhow::Result<()>;
    
    /// Get model metadata by ID
    fn get_model(&self, model_id: &str) -> Option<ModelMetadata>;
    
    /// List all registered models
    fn list_models(&self) -> Vec<ModelMetadata>;
    
    /// List models by category
    fn list_models_by_category(&self, category: ModelCategory) -> Vec<ModelMetadata> {
        self.list_models()
            .into_iter()
            .filter(|m| m.category == category)
            .collect()
    }
    
    /// Update model metadata
    fn update_model(&self, metadata: ModelMetadata) -> anyhow::Result<()>;
    
    /// Remove model from registry
    fn unregister_model(&self, model_id: &str) -> anyhow::Result<()>;
}
