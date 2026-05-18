//! Storage Source Persistence
//!
//! Persists storage sources to disk so they survive application restarts.
//! Storage sources are saved as JSON in the app data directory.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use tokio::fs as async_fs;
use tracing::{debug, info, warn, error};

use crate::vfs::domain::{StorageSource, StorageSourceType, ConnectionStatus, StorageConfig};

/// Persisted storage source (without runtime-only fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedStorageSource {
    id: String,
    name: String,
    source_type: String, // Serialized StorageSourceType
    config: StorageConfig,
    mount_point: Option<String>, // Path as string
}

impl From<&StorageSource> for PersistedStorageSource {
    fn from(source: &StorageSource) -> Self {
        Self {
            id: source.id.clone(),
            name: source.name.clone(),
            source_type: format!("{:?}", source.source_type),
            config: source.config.clone(),
            mount_point: source.mount_point.as_ref()
                .map(|p| p.to_string_lossy().to_string()),
        }
    }
}

/// Storage persistence manager
pub struct StoragePersistence {
    storage_file: PathBuf,
}

impl StoragePersistence {
    /// Create a new storage persistence manager
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ursly")
            .join("vfs");
        
        fs::create_dir_all(&data_dir)
            .context("Failed to create storage persistence directory")?;
        
        let storage_file = data_dir.join("storage_sources.json");
        
        Ok(Self { storage_file })
    }
    
    /// Get the default storage file path
    pub fn default_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ursly")
            .join("vfs")
            .join("storage_sources.json")
    }
    
    /// Load persisted storage sources from disk
    pub async fn load(&self) -> Result<Vec<StorageSource>> {
        if !self.storage_file.exists() {
            debug!("Storage sources file not found, starting fresh");
            return Ok(Vec::new());
        }
        
        let content = async_fs::read_to_string(&self.storage_file).await
            .context("Failed to read storage sources file")?;
        
        if content.trim().is_empty() {
            return Ok(Vec::new());
        }
        
        let persisted: Vec<PersistedStorageSource> = serde_json::from_str(&content)
            .context("Failed to parse storage sources file")?;
        
        let mut sources = Vec::new();
        for p in persisted {
            // Parse source type from string
            let source_type = match p.source_type.as_str() {
                "Local" => StorageSourceType::Local,
                "S3" => StorageSourceType::S3,
                "Gcs" => StorageSourceType::Gcs,
                "AzureBlob" => StorageSourceType::AzureBlob,
                "S3Compatible" => StorageSourceType::S3Compatible,
                "FsxOntap" => StorageSourceType::FsxOntap,
                "Nfs" => StorageSourceType::Nfs,
                "Smb" | "Nas" => StorageSourceType::Nas,
                "Sftp" => StorageSourceType::Sftp,
                "WebDav" => StorageSourceType::WebDav,
                _ => {
                    // Try to parse as Custom(type)
                    if p.source_type.starts_with("Custom(") {
                        let custom_type = p.source_type
                            .strip_prefix("Custom(")
                            .and_then(|s| s.strip_suffix(")"))
                            .unwrap_or("custom")
                            .to_string();
                        StorageSourceType::Custom(custom_type)
                    } else {
                        warn!("Unknown storage source type: {}, defaulting to Custom", p.source_type);
                        StorageSourceType::Custom(p.source_type)
                    }
                }
            };
            
            let mount_point = p.mount_point.map(PathBuf::from);
            
            let source = StorageSource {
                id: p.id,
                name: p.name,
                source_type,
                status: ConnectionStatus::Disconnected, // Start disconnected, will connect on mount
                mounted: false, // Start unmounted, will mount on init
                mount_point,
                config: p.config,
            };
            
            sources.push(source);
        }
        
        info!("Loaded {} persisted storage sources", sources.len());
        Ok(sources)
    }
    
    /// Save storage sources to disk
    pub async fn save(&self, sources: &[StorageSource]) -> Result<()> {
        // Filter out system locations (they're auto-mounted on init)
        let system_names = ["Home", "Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos"];
        let to_persist: Vec<PersistedStorageSource> = sources
            .iter()
            .filter(|s| !system_names.contains(&s.name.as_str()))
            .map(PersistedStorageSource::from)
            .collect();
        
        let content = serde_json::to_string_pretty(&to_persist)
            .context("Failed to serialize storage sources")?;
        
        // Ensure parent directory exists
        if let Some(parent) = self.storage_file.parent() {
            async_fs::create_dir_all(parent).await?;
        }
        
        async_fs::write(&self.storage_file, content).await
            .context("Failed to write storage sources file")?;
        
        debug!("Saved {} storage sources to disk", to_persist.len());
        Ok(())
    }
    
    /// Remove a storage source from persistence
    pub async fn remove(&self, source_id: &str) -> Result<()> {
        let sources = self.load().await?;
        let filtered: Vec<StorageSource> = sources
            .into_iter()
            .filter(|s| s.id != source_id)
            .collect();
        
        self.save(&filtered).await?;
        info!("Removed storage source {} from persistence", source_id);
        Ok(())
    }
}

impl Default for StoragePersistence {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            error!("Failed to create storage persistence: {}", e);
            // Fallback to a temporary path
            Self {
                storage_file: PathBuf::from("/tmp/ursly_storage_sources.json"),
            }
        })
    }
}
