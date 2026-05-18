//! Model Manager - Unified model management with categorization
//!
//! Provides a centralized model management system that abstracts over different
//! model providers (Ollama, local models, etc.) and categorizes models by their purpose.

use anyhow::Result;
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::vfs::adapters::ollama_client::OllamaClient;
use crate::vfs::domain::value_objects::ModelCategory;
use crate::vfs::ports::model::{
    IModelProvider, IModelRegistry, ModelMetadata, ModelOperationResult, ModelProgress,
};

/// In-memory model registry
pub struct InMemoryModelRegistry {
    models: Arc<RwLock<HashMap<String, ModelMetadata>>>,
}

impl Default for InMemoryModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryModelRegistry {
    pub fn new() -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Initialize with default model metadata
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        
        // Register known transcription models
        registry.register_model(ModelMetadata {
            id: "whisper".to_string(),
            name: "Whisper".to_string(),
            category: ModelCategory::Transcription,
            size_bytes: 0, // Will be updated when model is installed
            description: Some("OpenAI Whisper - High-quality speech-to-text transcription".to_string()),
            provider: "ollama".to_string(),
            version: None,
            is_installed: false,
            is_running: false,
            local_path: None,
            dependencies: vec!["ollama".to_string()],
            supported_languages: vec!["en".to_string(), "es".to_string(), "fr".to_string(), "de".to_string(), "it".to_string(), "pt".to_string(), "ru".to_string(), "ja".to_string(), "zh".to_string()],
            capabilities: vec!["transcription".to_string(), "audio-to-text".to_string()],
        }).ok();
        
        registry.register_model(ModelMetadata {
            id: "whisper-large".to_string(),
            name: "Whisper Large".to_string(),
            category: ModelCategory::Transcription,
            size_bytes: 0,
            description: Some("OpenAI Whisper Large - Highest quality transcription with better accuracy".to_string()),
            provider: "ollama".to_string(),
            version: None,
            is_installed: false,
            is_running: false,
            local_path: None,
            dependencies: vec!["ollama".to_string()],
            supported_languages: vec!["en".to_string(), "es".to_string(), "fr".to_string(), "de".to_string(), "it".to_string(), "pt".to_string(), "ru".to_string(), "ja".to_string(), "zh".to_string()],
            capabilities: vec!["transcription".to_string(), "audio-to-text".to_string(), "high-accuracy".to_string()],
        }).ok();
        
        // Register known video tagging models (examples - these would need to be actual models)
        registry.register_model(ModelMetadata {
            id: "video-tagger".to_string(),
            name: "Video Tagger".to_string(),
            category: ModelCategory::VideoTagging,
            size_bytes: 0,
            description: Some("Automatic video content tagging and description".to_string()),
            provider: "ollama".to_string(),
            version: None,
            is_installed: false,
            is_running: false,
            local_path: None,
            dependencies: vec!["ollama".to_string()],
            supported_languages: vec!["en".to_string()],
            capabilities: vec!["video-tagging".to_string(), "content-analysis".to_string()],
        }).ok();
        
        registry
    }
}

impl IModelRegistry for InMemoryModelRegistry {
    fn register_model(&self, metadata: ModelMetadata) -> Result<()> {
        let mut models = self.models.write();
        models.insert(metadata.id.clone(), metadata);
        Ok(())
    }
    
    fn get_model(&self, model_id: &str) -> Option<ModelMetadata> {
        let models = self.models.read();
        models.get(model_id).cloned()
    }
    
    fn list_models(&self) -> Vec<ModelMetadata> {
        let models = self.models.read();
        models.values().cloned().collect()
    }
    
    fn update_model(&self, metadata: ModelMetadata) -> Result<()> {
        let mut models = self.models.write();
        models.insert(metadata.id.clone(), metadata);
        Ok(())
    }
    
    fn unregister_model(&self, model_id: &str) -> Result<()> {
        let mut models = self.models.write();
        models.remove(model_id);
        Ok(())
    }
}

/// Ollama model provider implementation
pub struct OllamaModelProvider {
    client: OllamaClient,
    registry: Arc<dyn IModelRegistry>,
}

impl OllamaModelProvider {
    pub fn new(registry: Arc<dyn IModelRegistry>) -> Self {
        Self {
            client: OllamaClient::new(None),
            registry,
        }
    }
    
    /// Infer model category from model name
    fn infer_category(model_name: &str) -> ModelCategory {
        let name_lower = model_name.to_lowercase();
        
        if name_lower.contains("whisper") || name_lower.contains("transcribe") || name_lower.contains("audio") {
            ModelCategory::Transcription
        } else if name_lower.contains("video") || name_lower.contains("tag") {
            ModelCategory::VideoTagging
        } else if name_lower.contains("image") || name_lower.contains("vision") {
            ModelCategory::ImageTagging
        } else if name_lower.contains("code") || name_lower.contains("coder") {
            ModelCategory::CodeGeneration
        } else if name_lower.contains("embed") || name_lower.contains("vector") {
            ModelCategory::Embedding
        } else if name_lower.contains("llama") || name_lower.contains("mistral") || name_lower.contains("gpt") {
            ModelCategory::TextGeneration
        } else {
            ModelCategory::Other
        }
    }
}

#[async_trait]
impl IModelProvider for OllamaModelProvider {
    fn provider_name(&self) -> &str {
        "ollama"
    }
    
    async fn is_available(&self) -> bool {
        self.client.is_available().await
    }
    
    async fn list_models(&self) -> Result<Vec<ModelMetadata>> {
        let ollama_models = self.client.list_models().await?;
        
        let mut models = Vec::new();
        for ollama_model in ollama_models {
            let category = Self::infer_category(&ollama_model.name);
            
            // Check registry for existing metadata
            let mut metadata = self.registry.get_model(&ollama_model.name)
                .unwrap_or_else(|| ModelMetadata {
                    id: ollama_model.name.clone(),
                    name: ollama_model.name.clone(),
                    category,
                    size_bytes: ollama_model.size,
                    description: None,
                    provider: "ollama".to_string(),
                    version: None,
                    is_installed: true,
                    is_running: false,
                    local_path: None,
                    dependencies: vec!["ollama".to_string()],
                    supported_languages: vec![],
                    capabilities: vec![],
                });
            
            // Update with current status
            metadata.size_bytes = ollama_model.size;
            metadata.is_installed = true;
            
            models.push(metadata);
        }
        
        Ok(models)
    }
    
    async fn get_model(&self, model_id: &str) -> Result<Option<ModelMetadata>> {
        let models = self.list_models().await?;
        Ok(models.into_iter().find(|m| m.id == model_id))
    }
    
    async fn install_model(
        &self,
        model_id: &str,
        on_progress: Option<Box<dyn Fn(ModelProgress) + Send>>,
    ) -> Result<ModelOperationResult> {
        // For Ollama, installation is done via `ollama pull`
        // This would need to be implemented via command execution or Ollama API
        // For now, return a placeholder
        if let Some(callback) = on_progress {
            callback(ModelProgress {
                status: format!("Installing {}...", model_id),
                total_bytes: None,
                downloaded_bytes: None,
                progress: 0.0,
                operation: "installing".to_string(),
            });
        }
        
        // TODO: Implement actual Ollama pull via API or command
        Ok(ModelOperationResult {
            success: true,
            error: None,
            model: self.get_model(model_id).await?.map(|mut m| {
                m.is_installed = true;
                m
            }),
        })
    }
    
    async fn uninstall_model(&self, _model_id: &str) -> Result<ModelOperationResult> {
        // TODO: Implement Ollama delete
        Ok(ModelOperationResult {
            success: true,
            error: None,
            model: None,
        })
    }
    
    async fn start_model(&self, model_id: &str) -> Result<ModelOperationResult> {
        // TODO: Implement Ollama run/start
        Ok(ModelOperationResult {
            success: true,
            error: None,
            model: self.get_model(model_id).await?.map(|mut m| {
                m.is_running = true;
                m
            }),
        })
    }
    
    async fn stop_model(&self, model_id: &str) -> Result<ModelOperationResult> {
        // TODO: Implement Ollama stop
        Ok(ModelOperationResult {
            success: true,
            error: None,
            model: self.get_model(model_id).await?.map(|mut m| {
                m.is_running = false;
                m
            }),
        })
    }
    
    async fn is_model_running(&self, _model_id: &str) -> Result<bool> {
        // TODO: Check Ollama ps
        Ok(false)
    }
    
    async fn get_running_models(&self) -> Result<Vec<ModelMetadata>> {
        // TODO: Get from Ollama ps
        Ok(vec![])
    }
}

/// Unified model manager that coordinates multiple providers
pub struct ModelManager {
    providers: Vec<Arc<dyn IModelProvider>>,
    #[allow(dead_code)]
    registry: Arc<dyn IModelRegistry>,
}

impl ModelManager {
    pub fn new(registry: Arc<dyn IModelRegistry>) -> Self {
        let mut manager = Self {
            providers: Vec::new(),
            registry: registry.clone(),
        };
        
        // Add Ollama provider
        manager.providers.push(Arc::new(OllamaModelProvider::new(registry)));
        
        manager
    }
    
    /// Add a model provider
    pub fn add_provider(&mut self, provider: Arc<dyn IModelProvider>) {
        self.providers.push(provider);
    }
    
    /// List all models from all providers
    pub async fn list_all_models(&self) -> Result<Vec<ModelMetadata>> {
        let mut all_models = Vec::new();
        
        for provider in &self.providers {
            if provider.is_available().await {
                match provider.list_models().await {
                    Ok(models) => all_models.extend(models),
                    Err(e) => {
                        tracing::warn!("Failed to list models from {}: {}", provider.provider_name(), e);
                    }
                }
            }
        }
        
        Ok(all_models)
    }
    
    /// List models by category
    pub async fn list_models_by_category(&self, category: ModelCategory) -> Result<Vec<ModelMetadata>> {
        let models = self.list_all_models().await?;
        Ok(models.into_iter()
            .filter(|m| m.category == category)
            .collect())
    }
    
    /// Get model by ID from any provider
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelMetadata>> {
        for provider in &self.providers {
            if provider.is_available().await {
                if let Ok(Some(model)) = provider.get_model(model_id).await {
                    return Ok(Some(model));
                }
            }
        }
        Ok(None)
    }
}
