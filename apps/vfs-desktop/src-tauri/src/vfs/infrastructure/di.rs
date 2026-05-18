//! Dependency Injection Container
//!
//! Provides a service container for managing dependencies and resolving services.
//! This allows the application to follow the Dependency Inversion Principle by
//! depending on abstractions rather than concrete implementations.

use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use crate::vfs::ports::{
    StorageAdapterFactory, CacheAdapterFactory, ClipboardAdapterFactory,
};
use crate::vfs::domain::CacheConfig;
use crate::vfs::adapters::factories::{
    StorageAdapterFactoryImpl,
    CacheAdapterFactoryImpl,
    ClipboardAdapterFactoryImpl,
};
use crate::vfs::application::VfsService;

/// Dependency injection container
/// 
/// Simplified container that stores Arc<dyn Trait> directly for trait objects.
/// This avoids the complexity of downcasting from Any.
pub struct ServiceContainer {
    /// Storage adapter factory (singleton)
    storage_factory: Arc<dyn StorageAdapterFactory>,
    
    /// Cache adapter factory (singleton)
    cache_factory: Arc<dyn CacheAdapterFactory>,
    
    /// Clipboard adapter factory (singleton)
    clipboard_factory: Arc<dyn ClipboardAdapterFactory>,
}

impl ServiceContainer {
    /// Create a new service container with default factories
    pub fn new() -> Self {
        info!("Initializing DI container with default factories");
        
        Self {
            storage_factory: Arc::new(StorageAdapterFactoryImpl::new()),
            cache_factory: Arc::new(CacheAdapterFactoryImpl::new()),
            clipboard_factory: Arc::new(ClipboardAdapterFactoryImpl::new()),
        }
    }
    
    /// Get the storage adapter factory
    pub fn storage_factory(&self) -> Arc<dyn StorageAdapterFactory> {
        self.storage_factory.clone()
    }
    
    /// Get the cache adapter factory
    pub fn cache_factory(&self) -> Arc<dyn CacheAdapterFactory> {
        self.cache_factory.clone()
    }
    
    /// Get the clipboard adapter factory
    pub fn clipboard_factory(&self) -> Arc<dyn ClipboardAdapterFactory> {
        self.clipboard_factory.clone()
    }
    
    /// Create VFS service with dependencies from container
    pub async fn create_vfs_service(&self, cache_config: CacheConfig) -> Result<Arc<VfsService>> {
        let cache = self.cache_factory.create(cache_config).await?;
        let service = VfsService::with_cache(cache).await?;
        Ok(Arc::new(service))
    }
}

impl Default for ServiceContainer {
    fn default() -> Self {
        Self::new()
    }
}
