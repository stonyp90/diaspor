//! Adapter Factories - Concrete implementations of factory ports
//!
//! These factories create concrete adapter instances based on configuration,
//! allowing the application layer to depend on ports rather than concrete types.

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

use crate::vfs::domain::StorageSourceType;
use crate::vfs::ports::{
    StorageAdapter, StorageAdapterFactory, StorageAdapterConfig,
    CacheAdapter, CacheAdapterFactory,
    IClipboardService, ClipboardAdapterFactory, ClipboardAdapterConfig,
};
use crate::vfs::domain::CacheConfig;

use super::{
    LocalStorageAdapter, S3StorageAdapter, NvmeCacheAdapter,
    GcsStorageAdapter, FsxOntapAdapter,
    AzureBlobStorageAdapter, OracleObjectStorageAdapter,
    ClipboardAdapter,
};

/// Concrete implementation of StorageAdapterFactory
pub struct StorageAdapterFactoryImpl;

impl StorageAdapterFactoryImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StorageAdapterFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageAdapterFactory for StorageAdapterFactoryImpl {
    async fn create_adapter(&self, config: StorageAdapterConfig) -> Result<Arc<dyn StorageAdapter>> {
        info!("Creating storage adapter: {:?}", config.adapter_type);
        
        match config.adapter_type {
            StorageSourceType::Local => {
                let path = std::path::PathBuf::from(&config.path_or_bucket);
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Local Storage")
                    .to_string();
                let adapter = LocalStorageAdapter::new(path, name);
                Ok(Arc::new(adapter))
            }
            StorageSourceType::S3 => {
                let bucket = config.path_or_bucket.clone();
                let region = config.region.clone()
                    .ok_or_else(|| anyhow::anyhow!("Region is required for S3"))?;
                let name = bucket.clone();
                
                // S3StorageAdapter::new reads credentials from environment variables
                // We pass None for credentials to ensure they're read from env
                let adapter = S3StorageAdapter::new(
                    bucket,
                    region,
                    config.access_key.clone(),
                    config.secret_key.clone(),
                    None, // session_token not in StorageAdapterConfig yet
                    config.endpoint.clone(),
                    name,
                )
                .await
                .context("Failed to create S3 adapter")?;
                
                Ok(Arc::new(adapter))
            }
            StorageSourceType::Gcs => {
                let bucket = config.path_or_bucket.clone();
                let name = bucket.clone();
                
                // GCS uses credentials_path, not region
                let credentials_path = config.access_key.clone(); // Reuse access_key field for credentials path
                
                let adapter = GcsStorageAdapter::new(
                    bucket,
                    credentials_path,
                    name,
                )
                .await
                .context("Failed to create GCS adapter")?;
                
                Ok(Arc::new(adapter))
            }
            StorageSourceType::AzureBlob => {
                let container = config.path_or_bucket.clone();
                let account_name = config.region.clone()
                    .ok_or_else(|| anyhow::anyhow!("Account name (region field) is required for Azure Blob Storage"))?;
                let name = container.clone();
                
                // Azure Blob Storage uses account_key (secret_key) and optional endpoint
                let adapter = AzureBlobStorageAdapter::new(
                    container,
                    account_name,
                    config.access_key.clone(), // Account key
                    config.endpoint.clone(),
                    name,
                )
                .await
                .context("Failed to create Azure Blob Storage adapter")?;
                
                Ok(Arc::new(adapter))
            }
            StorageSourceType::S3Compatible => {
                // Check if this is Oracle Object Storage (has namespace in endpoint or special config)
                // For now, we'll use Oracle adapter if endpoint contains "oraclecloud.com"
                // Otherwise, fall back to S3 adapter with custom endpoint
                let bucket = config.path_or_bucket.clone();
                let region = config.region.clone()
                    .unwrap_or_else(|| "us-ashburn-1".to_string()); // Default Oracle region
                let name = bucket.clone();
                
                // Check if endpoint suggests Oracle Object Storage
                let is_oracle = config.endpoint.as_ref()
                    .map(|e| e.contains("oraclecloud.com"))
                    .unwrap_or(false);
                
                if is_oracle {
                    // Extract namespace from endpoint or use a default
                    // Oracle endpoint format: https://{namespace}.compat.objectstorage.{region}.oraclecloud.com
                    let namespace = config.endpoint.as_ref()
                        .and_then(|e| {
                            e.strip_prefix("https://")
                                .and_then(|s| s.split('.').next())
                                .map(|s| s.to_string())
                        })
                        .unwrap_or_else(|| {
                            // Try to get from access_key field (reused for namespace)
                            config.access_key.clone().unwrap_or_else(|| "default".to_string())
                        });
                    
                    let adapter = OracleObjectStorageAdapter::new(
                        bucket,
                        namespace,
                        region,
                        config.access_key.clone(),
                        config.secret_key.clone(),
                        config.endpoint.clone(),
                        name,
                    )
                    .await
                    .context("Failed to create Oracle Object Storage adapter")?;
                    
                    Ok(Arc::new(adapter))
                } else {
                    // Use S3 adapter with custom endpoint for other S3-compatible services
                    let adapter = S3StorageAdapter::new(
                        bucket,
                        region,
                        config.access_key.clone(),
                        config.secret_key.clone(),
                        None, // session_token
                        config.endpoint.clone(),
                        name,
                    )
                    .await
                    .context("Failed to create S3-compatible adapter")?;
                    
                    Ok(Arc::new(adapter))
                }
            }
            StorageSourceType::Nas => {
                let _server = config.path_or_bucket.clone();
                let _name = _server.clone();
                
                // NAS requires additional config that's not in StorageAdapterConfig
                // For now, return an error - this should be extended with proper NAS config
                Err(anyhow::anyhow!("NAS adapter creation requires additional configuration"))
            }
            StorageSourceType::FsxOntap => {
                let mount_point = std::path::PathBuf::from(&config.path_or_bucket);
                let name = mount_point.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("FSx ONTAP")
                    .to_string();
                
                // FSx ONTAP uses endpoint for S3 access point and API endpoint
                let s3_access_point = config.endpoint.clone();
                let api_endpoint = config.region.clone(); // Reuse region field for API endpoint
                
                let adapter = FsxOntapAdapter::new(
                    mount_point,
                    name,
                    s3_access_point,
                    api_endpoint,
                );
                
                Ok(Arc::new(adapter))
            }
            StorageSourceType::Custom(_) => {
                // Custom storage types would need special handling
                Err(anyhow::anyhow!("Custom storage adapter creation not yet implemented"))
            }
            StorageSourceType::FsxN
            | StorageSourceType::Block
            | StorageSourceType::Smb
            | StorageSourceType::Nfs
            | StorageSourceType::Sftp
            | StorageSourceType::WebDav => {
                Err(anyhow::anyhow!("Storage adapter type {:?} not yet implemented", config.adapter_type))
            }
        }
    }
}

/// Concrete implementation of CacheAdapterFactory
pub struct CacheAdapterFactoryImpl;

impl CacheAdapterFactoryImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CacheAdapterFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CacheAdapterFactory for CacheAdapterFactoryImpl {
    async fn create(&self, config: CacheConfig) -> Result<Arc<dyn CacheAdapter>> {
        info!("Creating cache adapter with config: {:?}", config);
        
        let adapter = NvmeCacheAdapter::new(config)
            .await
            .context("Failed to create NVMe cache adapter")?;
        
        Ok(Arc::new(adapter))
    }
}

/// Concrete implementation of ClipboardAdapterFactory
pub struct ClipboardAdapterFactoryImpl;

impl ClipboardAdapterFactoryImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClipboardAdapterFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ClipboardAdapterFactory for ClipboardAdapterFactoryImpl {
    async fn create(&self, config: ClipboardAdapterConfig) -> Result<Arc<dyn IClipboardService>> {
        info!("Creating clipboard adapter");
        
        let mut adapter = ClipboardAdapter::new();
        
        // Set file operations provider if provided
        if let Some(file_ops_provider) = config.vfs_service {
            adapter.set_file_ops_provider(file_ops_provider);
        }
        
        Ok(Arc::new(adapter))
    }
}
