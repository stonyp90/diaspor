//! VFS Service - Main service orchestrating VFS operations

use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, error, info, warn};

use crate::vfs::domain::{
    StorageSource, StorageSourceType, ConnectionStatus, StorageConfig,
    VirtualFile, CacheConfig, StorageTier,
};
use crate::vfs::domain::events::*;
use crate::vfs::ports::{
    StorageAdapter, StorageAdapterFactory, StorageAdapterConfig,
    CacheAdapter, CacheAdapterFactory, EventBus, CacheStats,
    IFileOperations, FileStat, CopyOptions, MoveOptions,
    IFileOperationsProvider, ISettingsManager,
};
use crate::vfs::domain::settings::ProviderCredentials;
use async_trait::async_trait;

/// VFS Service - Orchestrates storage, caching, and hydration
pub struct VfsService {
    /// Registered storage sources
    sources: Arc<RwLock<HashMap<String, StorageSourceState>>>,
    
    /// Cache adapter
    cache: Arc<dyn CacheAdapter>,
    
    /// Storage adapter factory (for creating new storage adapters)
    storage_factory: Arc<dyn StorageAdapterFactory>,
    
    /// Event bus (optional, for Tauri integration)
    event_bus: Option<Arc<dyn EventBus>>,
    
    /// Settings manager (optional, for storing provider credentials)
    settings_manager: Option<Arc<dyn ISettingsManager>>,
}

struct StorageSourceState {
    source: StorageSource,
    adapter: Arc<dyn StorageAdapter>,
    /// Optional reference to file operations (same adapter, different trait)
    file_ops: Option<Arc<dyn IFileOperations>>,
}

impl VfsService {
    /// Create a new VFS service with default cache configuration
    pub async fn new() -> Result<Self> {
        use crate::vfs::adapters::factories::CacheAdapterFactoryImpl;
        use crate::vfs::adapters::factories::StorageAdapterFactoryImpl;
        
        let cache_factory = Arc::new(CacheAdapterFactoryImpl::new());
        let cache_config = CacheConfig::default();
        let cache = cache_factory.create(cache_config).await?;
        
        let storage_factory = Arc::new(StorageAdapterFactoryImpl::new());
        
        Ok(Self {
            sources: Arc::new(RwLock::new(HashMap::new())),
            cache,
            storage_factory,
            event_bus: None,
            settings_manager: None,
        })
    }
    
    /// Create with custom cache configuration
    pub async fn with_cache_config(cache_config: CacheConfig) -> Result<Self> {
        use crate::vfs::adapters::factories::CacheAdapterFactoryImpl;
        use crate::vfs::adapters::factories::StorageAdapterFactoryImpl;
        
        let cache_factory = Arc::new(CacheAdapterFactoryImpl::new());
        let cache = cache_factory.create(cache_config).await?;
        
        let storage_factory = Arc::new(StorageAdapterFactoryImpl::new());
        
        Ok(Self {
            sources: Arc::new(RwLock::new(HashMap::new())),
            cache,
            storage_factory,
            event_bus: None,
            settings_manager: None,
        })
    }
    
    /// Create with existing cache adapter and storage factory (for dependency injection)
    pub async fn with_cache_and_factory(
        cache: Arc<dyn CacheAdapter>,
        storage_factory: Arc<dyn StorageAdapterFactory>,
    ) -> Result<Self> {
        Ok(Self {
            sources: Arc::new(RwLock::new(HashMap::new())),
            cache,
            storage_factory,
            event_bus: None,
            settings_manager: None,
        })
    }
    
    /// Create with existing cache adapter (for dependency injection)
    /// Uses default storage factory
    pub async fn with_cache(cache: Arc<dyn CacheAdapter>) -> Result<Self> {
        use crate::vfs::adapters::factories::StorageAdapterFactoryImpl;
        
        let storage_factory = Arc::new(StorageAdapterFactoryImpl::new());
        
        Ok(Self {
            sources: Arc::new(RwLock::new(HashMap::new())),
            cache,
            storage_factory,
            event_bus: None,
            settings_manager: None,
        })
    }
    
    /// Set the event bus for publishing domain events
    pub fn set_event_bus(&mut self, event_bus: Arc<dyn EventBus>) {
        self.event_bus = Some(event_bus);
    }
    
    /// Set the settings manager for storing provider credentials
    pub fn set_settings_manager(&mut self, settings_manager: Arc<dyn ISettingsManager>) {
        self.settings_manager = Some(settings_manager);
    }
    
    /// Initialize settings manager with default file-based store
    pub fn init_settings_manager(&mut self) -> Result<()> {
        use crate::vfs::adapters::settings::{FileSettingsStore, SettingsManager};
        let store = FileSettingsStore::new()?;
        let manager = SettingsManager::new(Box::new(store));
        self.settings_manager = Some(Arc::new(manager));
        Ok(())
    }
    
    /// Register a local storage source
    pub async fn add_local_source(&self, name: String, path: PathBuf) -> Result<StorageSource> {
        // Use factory to create adapter
        let config = StorageAdapterConfig {
            adapter_type: StorageSourceType::Local,
            path_or_bucket: path.to_string_lossy().to_string(),
            region: None,
            endpoint: None,
            access_key: None,
            secret_key: None,
        };
        let adapter = self.storage_factory.create_adapter(config).await?;
        
        // LocalStorageAdapter implements both StorageAdapter and IFileOperations
        // Since we can't downcast from Arc<dyn StorageAdapter>, we need to create it separately
        // TODO: Refactor to use a composite adapter pattern or helper trait
        // For now, we'll create the adapter directly to get IFileOperations
        // This is a temporary workaround until we refactor the adapter pattern
        use crate::vfs::adapters::LocalStorageAdapter;
        let local_adapter = LocalStorageAdapter::new(path.clone(), name.clone());
        let file_ops: Arc<dyn IFileOperations> = Arc::new(local_adapter);
        
        let source = StorageSource {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.clone(),
            source_type: StorageSourceType::Local,
            status: ConnectionStatus::Connected,
            mounted: true,
            mount_point: Some(path.clone()),
            config: StorageConfig {
                path_or_bucket: path.to_string_lossy().to_string(),
                ..Default::default()
            },
        };
        
        self.sources.write().insert(source.id.clone(), StorageSourceState {
            source: source.clone(),
            adapter,
            file_ops: Some(file_ops),
        });
        
        info!("Added local storage source: {} at {:?}", name, path);
        
        Ok(source)
    }
    
    /// Register an S3 storage source
    #[allow(clippy::too_many_arguments)]
    pub async fn add_s3_source(
        &self,
        name: String,
        bucket: String,
        region: String,
        access_key: Option<String>,
        secret_key: Option<String>,
        session_token: Option<String>,
        endpoint: Option<String>,
    ) -> Result<StorageSource> {
        info!("[add_s3_source] Creating S3 source - name: {}, bucket: {}, region: {}, has_access_key: {}, has_secret_key: {}, has_session_token: {}", 
            name, bucket, region, access_key.is_some(), secret_key.is_some(), session_token.is_some());
        
        // Clone values before moving them into config
        let bucket_clone = bucket.clone();
        let region_clone = region.clone();
        let endpoint_clone = endpoint.clone();
        let name_clone = name.clone();
        
        // Use factory to create adapter
        let config = StorageAdapterConfig {
            adapter_type: StorageSourceType::S3,
            path_or_bucket: bucket_clone.clone(),
            region: Some(region_clone.clone()),
            endpoint: endpoint_clone.clone(),
            access_key: access_key.clone(),
            secret_key: secret_key.clone(),
        };
        let adapter = self.storage_factory.create_adapter(config).await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create S3 adapter for bucket '{}' in region '{}': {}. \
                    Verify bucket name, region, and credentials are correct.",
                    bucket_clone, region_clone, e
                )
            })?;
        
        // Test connection to catch credential/permission issues early
        info!("[add_s3_source] Testing S3 connection to bucket '{}' in region '{}'...", bucket_clone, region_clone);
        let connection_status = match adapter.test_connection().await {
            Ok(true) => {
                info!("[add_s3_source] ✅ S3 connection test successful - bucket '{}' is accessible", bucket_clone);
                ConnectionStatus::Connected
            }
            Ok(false) => {
                error!("[add_s3_source] ❌ S3 connection test failed - bucket '{}' is not accessible. Check credentials and permissions.", bucket_clone);
                ConnectionStatus::Error(format!(
                    "Connection test failed: Bucket '{}' is not accessible. Verify credentials and bucket permissions.",
                    bucket_clone
                ))
            }
            Err(e) => {
                error!("[add_s3_source] ❌ S3 connection test error: {} - Bucket: '{}', Region: '{}'", e, bucket_clone, region_clone);
                ConnectionStatus::Error(format!(
                    "Connection test error: {}. Verify bucket name '{}', region '{}', and credentials are correct.",
                    e, bucket_clone, region_clone
                ))
            }
        };
        
        let is_connected = matches!(connection_status, ConnectionStatus::Connected);
        let source = StorageSource {
            id: uuid::Uuid::new_v4().to_string(),
            name: name_clone.clone(),
            source_type: StorageSourceType::S3,
            status: connection_status,
            mounted: is_connected,
            mount_point: None,
            config: StorageConfig {
                path_or_bucket: bucket_clone.clone(),
                region: Some(region_clone.clone()),
                endpoint: endpoint_clone.clone(),
                // Never persist credentials - always read from environment variables
                access_key: None,
                secret_key: None,
                session_token: None,
            },
        };
        
        // S3StorageAdapter implements IFileOperations
        // Since we can't downcast from Arc<dyn StorageAdapter>, we need to create it separately
        // TODO: Refactor to use a composite adapter pattern or helper trait
        use crate::vfs::adapters::S3StorageAdapter;
        let s3_adapter = S3StorageAdapter::new(
            bucket_clone,
            region_clone,
            access_key.clone(),
            secret_key.clone(),
            session_token.clone(),
            endpoint_clone,
            name_clone,
        ).await?;
        let file_ops: Arc<dyn IFileOperations> = Arc::new(s3_adapter);
        
        self.sources.write().insert(source.id.clone(), StorageSourceState {
            source: source.clone(),
            adapter,
            file_ops: Some(file_ops),
        });
        
        // Save credentials to settings store if provided
        if let Some(ref settings_manager) = self.settings_manager {
            if access_key.is_some() || secret_key.is_some() {
                use crate::vfs::domain::settings::EncryptedString;
                let source_id_for_creds = source.id.clone();
                let credentials = ProviderCredentials::AwsS3 {
                    access_key_id: access_key.clone().unwrap_or_default(),
                    secret_access_key: EncryptedString::new_plaintext(
                        secret_key.clone().unwrap_or_default()
                    ),
                    session_token: session_token.clone().map(EncryptedString::new_plaintext),
                    region: region.clone(),
                };
                
                if let Err(e) = settings_manager.save_credentials(
                    &source_id_for_creds,
                    &name,
                    credentials,
                ).await {
                    warn!("Failed to save S3 credentials to settings store: {}", e);
                    // Continue anyway - credentials are still in environment variables
                } else {
                    info!("Saved S3 credentials to settings store for source: {}", source.id);
                }
            }
        }
        
        info!("Added S3 storage source: {}", name);
        
        Ok(source)
    }
    
    /// Load credentials from settings store for a storage source
    pub async fn load_credentials(&self, source_id: &str) -> Result<Option<ProviderCredentials>> {
        if let Some(ref settings_manager) = self.settings_manager {
            settings_manager.get_credentials(source_id).await
                .map_err(|e| anyhow::anyhow!("Failed to load credentials: {}", e))
        } else {
            Ok(None)
        }
    }
    
    /// Add a storage source with a custom adapter (internal method)
    pub(crate) fn add_source(
        &self,
        source: StorageSource,
        adapter: Arc<dyn StorageAdapter>,
        file_ops: Option<Arc<dyn IFileOperations>>,
    ) {
        self.sources.write().insert(source.id.clone(), StorageSourceState {
            source: source.clone(),
            adapter,
            file_ops,
        });
        info!("Added storage source: {} ({:?})", source.name, source.source_type);
    }
    
    /// List all registered storage sources
    pub fn list_sources(&self) -> Vec<StorageSource> {
        self.sources.read()
            .values()
            .map(|s| s.source.clone())
            .collect()
    }
    
    /// Get a storage source by ID
    pub fn get_source(&self, source_id: &str) -> Option<StorageSource> {
        self.sources.read()
            .get(source_id)
            .map(|s| s.source.clone())
    }
    
    /// Get storage adapter for a source (for testing connections)
    pub fn get_adapter(&self, source_id: &str) -> Option<Arc<dyn StorageAdapter>> {
        self.sources.read()
            .get(source_id)
            .map(|s| s.adapter.clone())
    }
    
    /// Update connection status for a storage source
    pub async fn update_source_status(
        &self,
        source_id: &str,
        status: ConnectionStatus,
    ) -> Result<()> {
        let mut sources = self.sources.write();
        if let Some(source_state) = sources.get_mut(source_id) {
            source_state.source.status = status.clone();
            // Update mounted status based on connection status
            source_state.source.mounted = matches!(status, ConnectionStatus::Connected);
            info!("Updated source '{}' status to {:?}", source_id, status);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Source '{}' not found", source_id))
        }
    }
    
    /// Update S3 credentials for an existing storage source
    pub async fn update_s3_credentials(
        &self,
        source_id: &str,
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
    ) -> Result<()> {
        // Get the source to verify it exists and get its details
        let source = self.get_source(source_id)
            .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
        
        // Check if it's an S3 source
        if !matches!(source.source_type, StorageSourceType::S3) {
            return Err(anyhow::anyhow!("Source {} is not an S3 storage source", source_id));
        }
        
        // Get region and bucket from source config
        let region = source.config.region.clone()
            .ok_or_else(|| anyhow::anyhow!("S3 source {} does not have a region configured", source_id))?;
        let bucket = source.config.path_or_bucket.clone();
        
        // Save credentials to settings store
        if let Some(ref settings_manager) = self.settings_manager {
            use crate::vfs::domain::settings::{EncryptedString, ProviderCredentials};
            
            let credentials = ProviderCredentials::AwsS3 {
                access_key_id: access_key_id.clone(),
                secret_access_key: EncryptedString::new_plaintext(secret_access_key.clone()),
                session_token: session_token.clone().map(EncryptedString::new_plaintext),
                region: region.clone(),
            };
            
            settings_manager.save_credentials(
                source_id,
                &source.name,
                credentials,
            ).await
            .map_err(|e| anyhow::anyhow!("Failed to save S3 credentials: {}", e))?;
            
            info!("Successfully saved S3 credentials to settings for source: {}", source_id);
        } else {
            return Err(anyhow::anyhow!("Settings manager not initialized. Cannot save credentials."));
        }
        
        // Recreate the adapter with new credentials and update the sources map
        // Use factory to create StorageAdapter (same pattern as add_s3_source)
        let config = StorageAdapterConfig {
            adapter_type: StorageSourceType::S3,
            path_or_bucket: bucket.clone(),
            region: Some(region.clone()),
            endpoint: source.config.endpoint.clone(),
            access_key: Some(access_key_id.clone()),
            secret_key: Some(secret_access_key.clone()),
        };
        let adapter = self.storage_factory.create_adapter(config).await
            .map_err(|e| anyhow::anyhow!("Failed to recreate S3 adapter with new credentials: {}", e))?;
        
        // Also create IFileOperations adapter (same pattern as add_s3_source)
        use crate::vfs::adapters::S3StorageAdapter;
        use crate::vfs::ports::IFileOperations;
        let s3_adapter = S3StorageAdapter::new(
            bucket,
            region,
            Some(access_key_id),
            Some(secret_access_key),
            session_token,
            source.config.endpoint.clone(),
            source.name.clone(),
        ).await
        .map_err(|e| anyhow::anyhow!("Failed to recreate S3 file operations adapter with new credentials: {}", e))?;
        let file_ops: Arc<dyn IFileOperations> = Arc::new(s3_adapter);
        
        // Update the adapter in the sources map
        let mut sources = self.sources.write();
        if let Some(source_state) = sources.get_mut(source_id) {
            source_state.file_ops = Some(file_ops);
            source_state.adapter = adapter;
            info!("Successfully updated S3 adapter with new credentials for source: {}", source_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Storage source {} not found in sources map", source_id))
        }
    }
    
    /// List files in a storage source
    pub async fn list_files(&self, source_id: &str, path: &Path) -> Result<Vec<VirtualFile>> {
        // Clone the adapter Arc before releasing the lock to avoid holding it across await
        let adapter = {
            let sources = self.sources.read();
            let state = sources.get(source_id)
                .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
            state.adapter.clone()
        };

        let mut files = adapter.list_files(path).await?;

        // Update tier status for cached files
        for file in &mut files {
            if !file.is_directory {
                let file_path = file.path.clone();
                if self.cache.is_cached(&file_path).await {
                    file.tier_status.current_tier = StorageTier::Hot;
                    file.tier_status.is_cached = true;
                    file.tier_status.can_warm = false;
                }
            }
        }

        Ok(files)
    }
    
    /// List files with pagination support
    pub async fn list_files_paginated(
        &self,
        source_id: &str,
        path: &Path,
        limit: Option<u64>,
        continuation_token: Option<&str>,
    ) -> Result<(Vec<VirtualFile>, Option<String>)> {
        // Clone the adapter Arc before releasing the lock to avoid holding it across await
        let adapter = {
            let sources = self.sources.read();
            let state = sources.get(source_id)
                .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
            state.adapter.clone()
        };

        let (mut files, next_token) = adapter.list_files_paginated(path, limit, continuation_token).await?;

        // Update tier status for cached files
        for file in &mut files {
            if !file.is_directory {
                let file_path = file.path.clone();
                if self.cache.is_cached(&file_path).await {
                    file.tier_status.current_tier = StorageTier::Hot;
                    file.tier_status.is_cached = true;
                    file.tier_status.can_warm = false;
                }
            }
        }

        Ok((files, next_token))
    }
    
    /// Hydrate (warm) a file from cold storage to cache
    pub async fn hydrate_file(&self, source_id: &str, path: &Path) -> Result<PathBuf> {
        let start_time = std::time::Instant::now();
        
        let (adapter, source_tier) = {
            let sources = self.sources.read();
            let state = sources.get(source_id)
                .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
            
            // Get current tier based on storage category
            let tier = match state.source.source_type.category() {
                crate::vfs::domain::StorageCategory::Local => StorageTier::Hot,
                crate::vfs::domain::StorageCategory::Block => StorageTier::Hot,
                crate::vfs::domain::StorageCategory::Cloud => StorageTier::Cold,
                crate::vfs::domain::StorageCategory::Network => StorageTier::Warm,
                crate::vfs::domain::StorageCategory::Hybrid => StorageTier::Cold,
                crate::vfs::domain::StorageCategory::Custom => StorageTier::Cold,
            };
            
            (state.adapter.clone(), tier)
        };
        
        // Publish hydration started event
        if let Some(event_bus) = &self.event_bus {
            let file_size = adapter.file_size(path).await.unwrap_or(0);
            event_bus.publish_hydration_started(FileHydrationStarted {
                file_path: path.to_path_buf(),
                source_tier,
                file_size,
                timestamp: SystemTime::now(),
            }).await?;
        }
        
        // Read file from source
        let data = adapter.read_file(path).await?;
        let bytes_transferred = data.len() as u64;
        
        // Cache the file
        let entry = self.cache.cache_file(path, &data).await?;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        // Publish hydration completed event
        if let Some(event_bus) = &self.event_bus {
            event_bus.publish_hydration_completed(FileHydrationCompleted {
                file_path: path.to_path_buf(),
                source_tier,
                target_tier: StorageTier::Hot,
                bytes_transferred,
                duration_ms,
                timestamp: SystemTime::now(),
            }).await?;
        }
        
        info!("Hydrated file: {:?} ({} bytes in {}ms)", path, bytes_transferred, duration_ms);
        
        Ok(entry.cache_path)
    }
    
    /// Read a file (from cache if available, otherwise from source)
    pub async fn read_file(&self, source_id: &str, path: &Path) -> Result<Vec<u8>> {
        // Check cache first
        if self.cache.is_cached(path).await {
            debug!("Cache hit: {:?}", path);
            return self.cache.read_from_cache(path).await;
        }
        
        debug!("Cache miss: {:?}", path);
        
        // Read from source - clone adapter before dropping guard
        let adapter = {
            let sources = self.sources.read();
            let state = sources.get(source_id)
                .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
            state.adapter.clone()
        };
        
        let data = adapter.read_file(path).await?;
        
        // Cache the file for future reads
        self.cache.cache_file(path, &data).await?;
        
        Ok(data)
    }
    
    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        self.cache.stats().await
    }
    
    /// Clear the cache
    pub async fn clear_cache(&self) -> Result<()> {
        self.cache.clear().await
    }
    
    /// Remove a storage source
    pub fn remove_source(&self, source_id: &str) -> Option<StorageSource> {
        let removed = self.sources.write()
            .remove(source_id)
            .map(|s| s.source.clone());
        
        // Note: Credential removal is handled asynchronously in the command layer
        // to avoid blocking here
        
        removed
    }
    
    /// Remove a storage source and its credentials (async version)
    pub async fn remove_source_async(&self, source_id: &str) -> Option<StorageSource> {
        let removed = self.remove_source(source_id);
        
        // Remove credentials from settings store
        if removed.is_some() {
            if let Some(ref settings_manager) = self.settings_manager {
                if let Err(e) = settings_manager.remove_credentials(source_id).await {
                    warn!("Failed to remove credentials from settings store: {}", e);
                }
            }
        }
        
        removed
    }
    
    /// Save storage sources to disk (for persistence)
    pub async fn save_sources(&self) -> anyhow::Result<()> {
        use crate::vfs::infrastructure::StoragePersistence;
        let persistence = StoragePersistence::new()?;
        let sources = self.list_sources();
        persistence.save(&sources).await?;
        Ok(())
    }
    
    /// Load storage sources from disk (for persistence)
    pub async fn load_sources(&self) -> anyhow::Result<()> {
        use crate::vfs::infrastructure::StoragePersistence;
        let persistence = StoragePersistence::new()?;
        let persisted_sources = persistence.load().await?;
        
        for source in persisted_sources {
            // Only restore non-system sources (S3, GCS, etc.)
            // System sources (Home, Desktop, etc.) are auto-mounted in vfs_init
            if matches!(source.source_type.category(), crate::vfs::domain::StorageCategory::Cloud) {
                // Recreate the adapter for cloud storage
                let config = StorageAdapterConfig {
                    adapter_type: source.source_type.clone(),
                    path_or_bucket: source.config.path_or_bucket.clone(),
                    region: source.config.region.clone(),
                    endpoint: source.config.endpoint.clone(),
                    access_key: None, // Never persist credentials
                    secret_key: None,
                };
                
                match self.storage_factory.create_adapter(config).await {
                    Ok(adapter) => {
                        // For S3, also create IFileOperations adapter
                        if matches!(source.source_type, StorageSourceType::S3) {
                            use crate::vfs::adapters::S3StorageAdapter;
                            
                            // Try to load credentials from settings store
                            let (access_key, secret_key, session_token) = if let Some(ref settings_manager) = self.settings_manager {
                                if let Ok(Some(ProviderCredentials::AwsS3 { access_key_id, secret_access_key, session_token, .. })) = settings_manager.get_credentials(&source.id).await {
                                    (
                                        Some(access_key_id),
                                        secret_access_key.plaintext().map(|s| s.to_string()),
                                        session_token.and_then(|t| t.plaintext().map(|s| s.to_string())),
                                    )
                                } else {
                                    (None, None, None)
                                }
                            } else {
                                (None, None, None)
                            };
                            
                            if let Ok(s3_adapter) = S3StorageAdapter::new(
                                source.config.path_or_bucket.clone(),
                                source.config.region.clone().unwrap_or_default(),
                                access_key,
                                secret_key,
                                session_token,
                                source.config.endpoint.clone(),
                                source.name.clone(),
                            ).await {
                                let file_ops: Arc<dyn IFileOperations> = Arc::new(s3_adapter);
                                self.add_source(source.clone(), adapter, Some(file_ops));
                                info!("Restored persisted storage source with credentials: {}", source.name);
                            } else {
                                warn!("Failed to recreate S3 adapter for persisted source: {}", source.name);
                            }
                        } else {
                            self.add_source(source.clone(), adapter, None);
                            info!("Restored persisted storage source: {}", source.name);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to restore persisted storage source {}: {}", source.name, e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Get the real filesystem path for a file in a storage source
    /// This resolves VFS paths to actual filesystem paths for opening with native apps
    pub async fn get_real_path(&self, source_id: &str, path: &Path) -> Result<PathBuf> {
        let sources = self.sources.read();
        let state = sources.get(source_id)
            .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
        
        // Get mount point from the source
        if let Some(mount_point) = &state.source.mount_point {
            // For local sources, combine mount point with relative path
            let real_path = if path.is_absolute() {
                // If path already starts with mount point, use as-is
                if path.starts_with(mount_point) {
                    path.to_path_buf()
                } else {
                    // Strip leading slash and append to mount point
                    let relative = path.strip_prefix("/").unwrap_or(path);
                    mount_point.join(relative)
                }
            } else {
                mount_point.join(path)
            };
            return Ok(real_path);
        }
        
        // For non-local sources (S3, etc.), we may need to download first
        // For now, return an error - future: use cache path
        Err(anyhow::anyhow!("Cannot get real path for non-local storage source"))
    }
    
    // =========================================================================
    // POSIX File Operations
    // =========================================================================
    
    /// Get file operations adapter for a source
    fn get_file_ops(&self, source_id: &str) -> Result<Arc<dyn IFileOperations>> {
        let sources = self.sources.read();
        let state = sources.get(source_id)
            .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
        
        let file_ops = state.file_ops.clone()
            .ok_or_else(|| {
                let source_type = format!("{:?}", state.source.source_type);
                anyhow::anyhow!("Source {} ({}) does not support file operations", source_id, source_type)
            })?;
        
        // Reduced logging - only log on first call per source or if debug enabled
        // This prevents excessive logging during navigation
        debug!("[VfsService] get_file_ops: source_id={}, source_name={}, source_type={:?}, has_file_ops=true", 
              source_id, state.source.name, state.source.source_type);
        
        Ok(file_ops)
    }
    
    /// Create a directory
    pub async fn mkdir(&self, source_id: &str, path: &Path) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.mkdir(path).await
    }
    
    /// Create directory and all parents
    pub async fn mkdir_p(&self, source_id: &str, path: &Path) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.mkdir_p(path).await
    }
    
    /// Remove empty directory
    pub async fn rmdir(&self, source_id: &str, path: &Path) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.rmdir(path).await
    }
    
    /// Rename file or directory
    pub async fn rename(&self, source_id: &str, from: &Path, to: &Path) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.rename(from, to).await
    }
    
    /// Copy file or directory
    pub async fn copy(&self, source_id: &str, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.copy(from, to, options).await
    }
    
    /// Move file or directory
    pub async fn mv(&self, source_id: &str, from: &Path, to: &Path, options: MoveOptions) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.mv(from, to, options).await
    }
    
    /// Remove file
    pub async fn rm(&self, source_id: &str, path: &Path) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.rm(path).await
    }
    
    /// Remove file or directory recursively
    pub async fn rm_rf(&self, source_id: &str, path: &Path) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.rm_rf(path).await
    }
    
    /// Change file permissions
    pub async fn chmod(&self, source_id: &str, path: &Path, mode: u32) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.chmod(path, mode).await
    }
    
    /// Get file statistics
    pub async fn stat(&self, source_id: &str, path: &Path) -> Result<FileStat> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.stat(path).await
    }
    
    /// Get file/folder metadata (includes calculated folder sizes for mounted storage)
    pub async fn get_metadata(&self, source_id: &str, path: &Path) -> Result<VirtualFile> {
        // Clone the adapter Arc before releasing the lock to avoid holding it across await
        let adapter = {
            let sources = self.sources.read();
            let state = sources.get(source_id)
                .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
            state.adapter.clone()
        };
        
        adapter.get_metadata(path).await
    }
    
    /// Refresh S3 credentials for a storage source
    /// This re-reads environment variables and recreates the S3 operator
    pub async fn refresh_s3_credentials(
        &self,
        source_id: &str,
        access_key: Option<String>,
        secret_key: Option<String>,
        session_token: Option<String>,
    ) -> Result<()> {
        // Get source config first, then drop the lock before async operations
        let (bucket, region, endpoint, name) = {
            let sources = self.sources.read();
            let state = sources.get(source_id)
                .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
            
            // Check if this is an S3 source
            if state.source.source_type != crate::vfs::domain::StorageSourceType::S3 {
                return Err(anyhow::anyhow!("Source {} is not an S3 source", source_id));
            }
            
            (
                state.source.config.path_or_bucket.clone(),
                state.source.config.region.clone()
                    .unwrap_or_else(|| "us-east-1".to_string()),
                state.source.config.endpoint.clone(),
                state.source.name.clone(),
            )
        };
        
        // Use factory to create new adapter with updated credentials
        let config = StorageAdapterConfig {
            adapter_type: StorageSourceType::S3,
            path_or_bucket: bucket.clone(),
            region: Some(region.clone()),
            endpoint: endpoint.clone(),
            access_key: access_key.clone(),
            secret_key: secret_key.clone(),
        };
        let new_adapter = self.storage_factory.create_adapter(config).await?;
        
        // S3StorageAdapter implements IFileOperations
        // Since we can't downcast from Arc<dyn StorageAdapter>, we need to create it separately
        // TODO: Refactor to use a composite adapter pattern or helper trait
        use crate::vfs::adapters::S3StorageAdapter;
        let s3_adapter = S3StorageAdapter::new(
            bucket,
            region,
            access_key,
            secret_key,
            session_token,
            endpoint,
            name,
        ).await?;
        let file_ops: Arc<dyn IFileOperations> = Arc::new(s3_adapter);
        
        // Update the adapter and file_ops (acquire write lock again)
        {
            let mut sources = self.sources.write();
            let state = sources.get_mut(source_id)
                .ok_or_else(|| anyhow::anyhow!("Storage source not found: {}", source_id))?;
            state.adapter = new_adapter;
            state.file_ops = Some(file_ops);
        }
        
        info!("Refreshed S3 credentials for source: {}", source_id);
        Ok(())
    }
    
    /// Touch file (create or update timestamp)
    pub async fn touch(&self, source_id: &str, path: &Path) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.touch(path).await
    }
    
    /// Check if path exists
    pub async fn exists(&self, source_id: &str, path: &Path) -> Result<bool> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.exists(path).await
    }
    
    /// Read file contents
    pub async fn read(&self, source_id: &str, path: &Path) -> Result<Vec<u8>> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.read(path).await
    }
    
    /// Write file contents
    pub async fn write(&self, source_id: &str, path: &Path, data: &[u8]) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.write(path, data).await
    }
    
    /// Append to file
    pub async fn append(&self, source_id: &str, path: &Path, data: &[u8]) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.append(path, data).await
    }
    
    // =========================================================================
    // Cross-Storage Operations
    // =========================================================================
    
    /// Copy files from one storage source to another
    pub async fn copy_to_source(
        &self,
        from_source_id: &str,
        from_path: &Path,
        to_source_id: &str,
        to_path: &Path,
    ) -> Result<u64> {
        let from_file_ops = self.get_file_ops(from_source_id)?;
        let to_file_ops = self.get_file_ops(to_source_id)?;
        
        // Get source file info
        let stat = from_file_ops.stat(from_path).await?;
        
        if stat.is_dir {
            // Recursive directory copy
            self.copy_dir_to_source(from_source_id, from_path, to_source_id, to_path).await
        } else {
            // Single file copy
            let data = from_file_ops.read(from_path).await?;
            let file_name = from_path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string());
            let dest_path = to_path.join(&file_name);
            
            to_file_ops.write(&dest_path, &data).await?;
            
            info!("Copied {} to {} ({}:{:?})", 
                from_path.display(), 
                to_source_id, 
                dest_path.display(),
                stat.size
            );
            
            Ok(stat.size)
        }
    }
    
    /// Copy directory recursively between sources
    async fn copy_dir_to_source(
        &self,
        from_source_id: &str,
        from_path: &Path,
        to_source_id: &str,
        to_path: &Path,
    ) -> Result<u64> {
        let from_file_ops = self.get_file_ops(from_source_id)?;
        let to_file_ops = self.get_file_ops(to_source_id)?;
        
        let dir_name = from_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "dir".to_string());
        let dest_dir = to_path.join(&dir_name);
        
        // Create destination directory
        to_file_ops.mkdir_p(&dest_dir).await?;
        
        let mut total_bytes = 0u64;
        
        // List source directory
        let entries = from_file_ops.list(from_path).await?;
        
        for entry in entries {
            let entry_path = from_path.join(&entry.name);
            
            if entry.is_dir {
                total_bytes += Box::pin(self.copy_dir_to_source(
                    from_source_id,
                    &entry_path,
                    to_source_id,
                    &dest_dir,
                )).await?;
            } else {
                let data = from_file_ops.read(&entry_path).await?;
                let dest_file = dest_dir.join(&entry.name);
                to_file_ops.write(&dest_file, &data).await?;
                total_bytes += entry.size;
            }
        }
        
        Ok(total_bytes)
    }
    
    /// Move files from one storage source to another (copy + delete)
    pub async fn move_to_source(
        &self,
        from_source_id: &str,
        from_path: &Path,
        to_source_id: &str,
        to_path: &Path,
    ) -> Result<u64> {
        // Copy first
        let bytes = self.copy_to_source(from_source_id, from_path, to_source_id, to_path).await?;
        
        // Delete source
        let from_file_ops = self.get_file_ops(from_source_id)?;
        from_file_ops.rm_rf(from_path).await?;
        
        info!("Moved {} from {} to {} ({} bytes)", 
            from_path.display(), 
            from_source_id, 
            to_source_id,
            bytes
        );
        
        Ok(bytes)
    }
    
    /// Get list of available storage sources for transfer
    pub fn get_transfer_targets(&self, exclude_source_id: Option<&str>) -> Vec<StorageSource> {
        let sources = self.sources.read();
        sources
            .values()
            .filter(|state| {
                state.source.status == ConnectionStatus::Connected
                    && exclude_source_id.map(|id| state.source.id != id).unwrap_or(true)
            })
            .map(|state| state.source.clone())
            .collect()
    }
}


// ============================================================================
// IFileOperationsProvider Implementation
// ============================================================================

/// Implement IFileOperationsProvider for VfsService
/// This allows ClipboardAdapter to use VfsService without circular dependency
#[async_trait]
impl IFileOperationsProvider for VfsService {
    async fn get_file_ops(&self, source_id: &str) -> Result<Arc<dyn IFileOperations>> {
        self.get_file_ops(source_id)
    }
    
    async fn mkdir_p(&self, source_id: &str, path: &std::path::Path) -> Result<()> {
        self.mkdir_p(source_id, path).await
    }
    
    async fn write(&self, source_id: &str, path: &std::path::Path, data: &[u8]) -> Result<()> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.write(path, data).await
    }
    
    async fn read(&self, source_id: &str, path: &std::path::Path) -> Result<Vec<u8>> {
        self.read_file(source_id, path).await
    }
    
    async fn stat(&self, source_id: &str, path: &std::path::Path) -> Result<FileStat> {
        self.stat(source_id, path).await
    }
    
    async fn list_files(&self, source_id: &str, path: &std::path::Path) -> Result<Vec<crate::vfs::ports::FileEntry>> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.list(path).await
    }
    
    async fn copy(
        &self,
        source_id: &str,
        from: &std::path::Path,
        to: &std::path::Path,
        options: CopyOptions,
    ) -> Result<()> {
        self.copy(source_id, from, to, options).await
    }
    
    async fn copy_to_source(
        &self,
        src_source_id: &str,
        from: &std::path::Path,
        dest_source_id: &str,
        to: &std::path::Path,
    ) -> Result<()> {
        self.copy_to_source(src_source_id, from, dest_source_id, to).await?;
        Ok(())
    }
    
    async fn rm(&self, source_id: &str, path: &std::path::Path) -> Result<()> {
        self.rm(source_id, path).await
    }
    
    async fn rm_rf(&self, source_id: &str, path: &std::path::Path) -> Result<()> {
        self.rm_rf(source_id, path).await
    }
    
    async fn exists(&self, source_id: &str, path: &std::path::Path) -> Result<bool> {
        let file_ops = self.get_file_ops(source_id)?;
        file_ops.exists(path).await
    }
}