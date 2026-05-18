//! Settings Port - Interface for settings storage
//!
//! This trait defines the contract for storing and retrieving application settings,
//! particularly provider credentials. Following the Ports & Adapters pattern,
//! the application core depends on this trait, not on concrete implementations.

use anyhow::Result;
use async_trait::async_trait;

use crate::vfs::domain::settings::{
    AppSettings, StorageSourceSettings, ProviderCredentials,
};

/// Settings storage port
///
/// This trait defines how settings are stored and retrieved. Implementations
/// may use file-based storage, database, secure keychain, etc.
#[async_trait]
pub trait ISettingsStore: Send + Sync {
    /// Load all application settings
    async fn load_settings(&self) -> Result<AppSettings>;
    
    /// Save all application settings
    async fn save_settings(&self, settings: &AppSettings) -> Result<()>;
    
    /// Get settings for a specific storage source
    async fn get_storage_source_settings(&self, source_id: &str) -> Result<Option<StorageSourceSettings>>;
    
    /// Save settings for a specific storage source
    async fn save_storage_source_settings(&self, settings: StorageSourceSettings) -> Result<()>;
    
    /// Remove settings for a specific storage source
    async fn remove_storage_source_settings(&self, source_id: &str) -> Result<()>;
    
    /// List all storage source IDs that have settings
    async fn list_storage_source_ids(&self) -> Result<Vec<String>>;
    
    /// Encrypt a plaintext string
    ///
    /// This method encrypts sensitive data before storing it.
    /// The encryption key should be derived from a master key stored securely.
    async fn encrypt(&self, plaintext: &str) -> Result<String>;
    
    /// Decrypt an encrypted string
    ///
    /// This method decrypts sensitive data after loading it.
    async fn decrypt(&self, encrypted: &str) -> Result<String>;
    
    /// Check if encryption is enabled
    fn is_encryption_enabled(&self) -> bool;
}

/// Settings manager trait
///
/// Higher-level interface for managing settings, with convenience methods
/// for common operations.
#[async_trait]
pub trait ISettingsManager: Send + Sync {
    /// Get the underlying settings store
    fn store(&self) -> &dyn ISettingsStore;
    
    /// Get credentials for a storage source
    async fn get_credentials(&self, source_id: &str) -> Result<Option<ProviderCredentials>> {
        let store = self.store();
        let settings = store.get_storage_source_settings(source_id).await?;
        Ok(settings.map(|s| s.credentials))
    }
    
    /// Save credentials for a storage source
    async fn save_credentials(
        &self,
        source_id: &str,
        source_name: &str,
        credentials: ProviderCredentials,
    ) -> Result<()> {
        let store = self.store();
        let settings = StorageSourceSettings {
            source_id: source_id.to_string(),
            source_name: source_name.to_string(),
            credentials,
            updated_at: Some(chrono::Utc::now()),
        };
        store.save_storage_source_settings(settings).await?;
        Ok(())
    }
    
    /// Remove credentials for a storage source
    async fn remove_credentials(&self, source_id: &str) -> Result<()> {
        let store = self.store();
        store.remove_storage_source_settings(source_id).await?;
        Ok(())
    }
}
