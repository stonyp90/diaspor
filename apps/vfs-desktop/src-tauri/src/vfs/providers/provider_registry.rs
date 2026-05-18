//! Provider Registry - Manages tier sync providers and their configurations

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, info, warn};

use crate::vfs::domain::{StorageSourceType, StorageTier};

/// Configuration for a tier sync provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider ID (e.g., "aws-datasync", "direct-copy")
    pub provider_id: String,
    
    /// Display name
    pub name: String,
    
    /// Provider type
    pub provider_type: ProviderType,
    
    /// Source storage type this provider handles
    pub source_storage_type: StorageSourceType,
    
    /// Target storage type this provider handles
    pub target_storage_type: StorageSourceType,
    
    /// Source tier
    pub source_tier: StorageTier,
    
    /// Target tier
    pub target_tier: StorageTier,
    
    /// Provider-specific configuration (JSON)
    pub config: HashMap<String, serde_json::Value>,
    
    /// Is this provider enabled
    pub enabled: bool,
    
    /// Priority (lower = higher priority)
    pub priority: u32,
}

/// Provider type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderType {
    /// AWS DataSync
    AwsDataSync,
    /// Direct copy (default fallback)
    DirectCopy,
    /// Custom provider
    Custom(String),
}

/// Result of a tier sync operation
#[derive(Debug, Clone)]
pub struct TierSyncResult {
    /// Number of files synced
    pub files_synced: usize,
    
    /// Number of files failed
    pub files_failed: usize,
    
    /// Total bytes transferred
    pub bytes_transferred: u64,
    
    /// Errors encountered
    pub errors: Vec<String>,
    
    /// Task/Job ID if provider supports async operations
    pub task_id: Option<String>,
}

/// Request for tier sync
#[derive(Debug, Clone)]
pub struct TierSyncRequest {
    /// Source storage ID
    pub source_id: String,
    
    /// Source storage type
    pub source_storage_type: StorageSourceType,
    
    /// Target storage ID (if cross-storage)
    pub target_storage_id: Option<String>,
    
    /// Target storage type
    pub target_storage_type: StorageSourceType,
    
    /// Source tier
    pub source_tier: StorageTier,
    
    /// Target tier
    pub target_tier: StorageTier,
    
    /// Paths to sync
    pub paths: Vec<PathBuf>,
    
    /// Source paths (for FSx ONTAP)
    pub source_paths: Option<Vec<String>>,
    
    /// Target paths (for S3)
    pub target_paths: Option<Vec<String>>,
}

/// Trait for tier sync providers
#[async_trait]
pub trait TierSyncProvider: Send + Sync {
    /// Provider ID
    fn provider_id(&self) -> &str;
    
    /// Provider name
    fn name(&self) -> &str;
    
    /// Check if this provider can handle the given request
    fn can_handle(&self, request: &TierSyncRequest) -> bool;
    
    /// Perform the tier sync operation
    async fn sync(&self, request: TierSyncRequest) -> Result<TierSyncResult>;
    
    /// Get provider configuration schema
    fn config_schema(&self) -> Vec<ConfigField>;
    
    /// Validate provider configuration
    fn validate_config(&self, config: &HashMap<String, serde_json::Value>) -> Result<()>;
}

/// Configuration field for provider setup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub field_type: ConfigFieldType,
    pub required: bool,
    pub description: Option<String>,
    pub default_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigFieldType {
    Text,
    Password,
    Number,
    Boolean,
    Select { options: Vec<(String, String)> },
}

/// Global provider registry
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<String, Arc<dyn TierSyncProvider>>>>,
    configs: Arc<RwLock<Vec<ProviderConfig>>>,
    config_path: PathBuf,
}

impl ProviderRegistry {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            configs: Arc::new(RwLock::new(Vec::new())),
            config_path,
        }
    }
    
    /// Register a provider
    pub fn register(&self, provider: Arc<dyn TierSyncProvider>) {
        let provider_id = provider.provider_id().to_string();
        info!("Registering tier sync provider: {}", provider_id);
        self.providers.write().insert(provider_id, provider);
    }
    
    /// Get a provider by ID
    pub fn get_provider(&self, provider_id: &str) -> Option<Arc<dyn TierSyncProvider>> {
        self.providers.read().get(provider_id).cloned()
    }
    
    /// Find the best provider for a given request
    pub fn find_provider(&self, request: &TierSyncRequest) -> Option<Arc<dyn TierSyncProvider>> {
        let configs = self.configs.read();
        
        // Find matching configs
        let mut matching_configs: Vec<_> = configs
            .iter()
            .filter(|config| {
                config.enabled
                    && config.source_storage_type == request.source_storage_type
                    && config.target_storage_type == request.target_storage_type
                    && config.source_tier == request.source_tier
                    && config.target_tier == request.target_tier
            })
            .collect();
        
        // Sort by priority (lower = higher priority)
        matching_configs.sort_by_key(|c| c.priority);
        
        // Try each matching provider
        for config in matching_configs {
            if let Some(provider) = self.get_provider(&config.provider_id) {
                if provider.can_handle(request) {
                    debug!("Selected provider {} for tier sync", config.provider_id);
                    return Some(provider);
                }
            }
        }
        
        None
    }
    
    /// Add a provider configuration
    pub fn add_config(&self, config: ProviderConfig) -> Result<()> {
        // Validate config
        if let Some(provider) = self.get_provider(&config.provider_id) {
            provider.validate_config(&config.config)?;
        }
        
        let mut configs = self.configs.write();
        configs.push(config);
        self.save_configs()?;
        Ok(())
    }
    
    /// Remove a provider configuration
    pub fn remove_config(&self, provider_id: &str, source_type: StorageSourceType, target_type: StorageSourceType) {
        let mut configs = self.configs.write();
        configs.retain(|c| {
            !(c.provider_id == provider_id
                && c.source_storage_type == source_type
                && c.target_storage_type == target_type)
        });
        let _ = self.save_configs();
    }
    
    /// Get all configurations
    pub fn get_configs(&self) -> Vec<ProviderConfig> {
        self.configs.read().clone()
    }
    
    /// Load configurations from disk
    pub fn load_configs(&self) -> Result<()> {
        if !self.config_path.exists() {
            return Ok(());
        }
        
        let data = std::fs::read_to_string(&self.config_path)?;
        let configs: Vec<ProviderConfig> = serde_json::from_str(&data)?;
        let config_count = configs.len();
        
        *self.configs.write() = configs;
        info!("Loaded {} provider configurations", config_count);
        Ok(())
    }
    
    /// Save configurations to disk
    fn save_configs(&self) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let configs = self.configs.read();
        let data = serde_json::to_string_pretty(&*configs)?;
        std::fs::write(&self.config_path, data)?;
        Ok(())
    }
}

// Global provider registry instance
use once_cell::sync::Lazy;

pub fn get_provider_registry() -> &'static ProviderRegistry {
    static REGISTRY: Lazy<ProviderRegistry> = Lazy::new(|| {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("diaspor");
        let config_path = config_dir.join("tier_sync_providers.json");
        
        let registry = ProviderRegistry::new(config_path);
        
        // Load existing configs
        if let Err(e) = registry.load_configs() {
            warn!("Failed to load provider configs: {}", e);
        }
        
        registry
    });
    
    &REGISTRY
}
