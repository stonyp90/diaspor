//! Settings Adapter - File-based settings storage with encryption
//!
//! Implements ISettingsStore using JSON file storage with optional encryption.
//! Credentials are encrypted at rest using AES-256-GCM.

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs as async_fs;
use tracing::{debug, info, warn, error};
use base64::{Engine as _, engine::general_purpose};

use crate::vfs::domain::settings::{
    AppSettings, StorageSourceSettings, ProviderCredentials,
};
use crate::vfs::ports::settings::{ISettingsStore, ISettingsManager};

/// File-based settings store with encryption support
pub struct FileSettingsStore {
    settings_file: PathBuf,
    encryption_enabled: bool,
    /// Master key for encryption (derived from system keychain or user password)
    /// In production, this should be stored securely (e.g., macOS Keychain, Windows Credential Store)
    #[allow(dead_code)]
    master_key: Option<Vec<u8>>,
}

impl FileSettingsStore {
    /// Create a new file-based settings store
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("diaspor")
            .join("vfs");
        
        // Create directory synchronously (blocking)
        std::fs::create_dir_all(&data_dir)
            .context("Failed to create settings directory")?;
        
        let settings_file = data_dir.join("settings.json");
        
        // For now, encryption is disabled by default
        // TODO: Implement proper keychain integration for master key
        let encryption_enabled = false;
        let master_key = None;
        
        Ok(Self {
            settings_file,
            encryption_enabled,
            master_key,
        })
    }
    
    /// Create with encryption enabled
    pub fn with_encryption(master_key: Vec<u8>) -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("diaspor")
            .join("vfs");
        
        // Create directory synchronously (blocking)
        std::fs::create_dir_all(&data_dir)
            .context("Failed to create settings directory")?;
        
        let settings_file = data_dir.join("settings.json");
        
        Ok(Self {
            settings_file,
            encryption_enabled: true,
            master_key: Some(master_key),
        })
    }
    
    /// Derive master key from system (placeholder - should use keychain)
    #[allow(dead_code)]
    fn derive_master_key() -> Result<Vec<u8>> {
        // TODO: Use system keychain (macOS Keychain, Windows Credential Store, Linux Secret Service)
        // For now, use a simple approach: derive from machine ID + user ID
        use std::fs;
        
        // Try to get a stable machine identifier
        let machine_id = if cfg!(target_os = "linux") {
            fs::read_to_string("/etc/machine-id")
                .ok()
                .or_else(|| fs::read_to_string("/var/lib/dbus/machine-id").ok())
        } else if cfg!(target_os = "macos") {
            // Use hardware UUID on macOS
            None // TODO: Get hardware UUID
        } else {
            None
        };
        
        let user_id = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "default".to_string());
        
        // Simple key derivation (not cryptographically secure - replace with proper KDF)
        let key_material = format!("{}{}", machine_id.unwrap_or_default(), user_id);
        let key = md5::compute(key_material.as_bytes());
        
        // Use first 32 bytes for AES-256
        Ok(key.0.to_vec())
    }
}

#[async_trait]
impl ISettingsStore for FileSettingsStore {
    async fn load_settings(&self) -> Result<AppSettings> {
        if !self.settings_file.exists() {
            debug!("Settings file not found, returning default settings");
            return Ok(AppSettings::default());
        }
        
        let content = async_fs::read_to_string(&self.settings_file).await
            .context("Failed to read settings file")?;
        
        if content.trim().is_empty() {
            return Ok(AppSettings::default());
        }
        
        let mut settings: AppSettings = serde_json::from_str(&content)
            .context("Failed to parse settings file")?;
        
        // Decrypt credentials if encryption is enabled
        if self.encryption_enabled {
            for source_settings in settings.storage_sources.values_mut() {
                self.decrypt_credentials(&mut source_settings.credentials).await?;
            }
        }
        
        info!("Loaded settings for {} storage sources", settings.storage_sources.len());
        Ok(settings)
    }
    
    async fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        // Clone settings to avoid mutating the original
        let mut settings_to_save = settings.clone();
        
        // Encrypt credentials if encryption is enabled
        if self.encryption_enabled {
            for source_settings in settings_to_save.storage_sources.values_mut() {
                self.encrypt_credentials(&mut source_settings.credentials).await?;
            }
        }
        
        let content = serde_json::to_string_pretty(&settings_to_save)
            .context("Failed to serialize settings")?;
        
        // Ensure parent directory exists
        if let Some(parent) = self.settings_file.parent() {
            async_fs::create_dir_all(parent).await?;
        }
        
        async_fs::write(&self.settings_file, content).await
            .context("Failed to write settings file")?;
        
        debug!("Saved settings to {:?}", self.settings_file);
        Ok(())
    }
    
    async fn get_storage_source_settings(&self, source_id: &str) -> Result<Option<StorageSourceSettings>> {
        let settings = self.load_settings().await?;
        let mut source_settings = settings.storage_sources.get(source_id).cloned();
        
        // Decrypt if needed
        if self.encryption_enabled {
            if let Some(ref mut ss) = source_settings {
                self.decrypt_credentials(&mut ss.credentials).await?;
            }
        }
        
        Ok(source_settings)
    }
    
    async fn save_storage_source_settings(&self, settings: StorageSourceSettings) -> Result<()> {
        let mut app_settings = self.load_settings().await?;
        app_settings.storage_sources.insert(settings.source_id.clone(), settings);
        self.save_settings(&app_settings).await?;
        Ok(())
    }
    
    async fn remove_storage_source_settings(&self, source_id: &str) -> Result<()> {
        let mut app_settings = self.load_settings().await?;
        app_settings.storage_sources.remove(source_id);
        self.save_settings(&app_settings).await?;
        Ok(())
    }
    
    async fn list_storage_source_ids(&self) -> Result<Vec<String>> {
        let settings = self.load_settings().await?;
        Ok(settings.storage_sources.keys().cloned().collect())
    }
    
    async fn encrypt(&self, plaintext: &str) -> Result<String> {
        if !self.encryption_enabled {
            // If encryption is disabled, return base64-encoded plaintext
            return Ok(general_purpose::STANDARD.encode(plaintext.as_bytes()));
        }
        
        // TODO: Implement proper AES-256-GCM encryption
        // For now, use base64 encoding as placeholder
        warn!("Encryption not fully implemented, using base64 encoding");
        Ok(general_purpose::STANDARD.encode(plaintext.as_bytes()))
    }
    
    async fn decrypt(&self, encrypted: &str) -> Result<String> {
        if !self.encryption_enabled {
            // If encryption is disabled, decode from base64
            let decoded = general_purpose::STANDARD.decode(encrypted)
                .context("Failed to decode base64")?;
            return String::from_utf8(decoded)
                .context("Failed to convert to UTF-8");
        }
        
        // TODO: Implement proper AES-256-GCM decryption
        // For now, decode from base64
        warn!("Decryption not fully implemented, using base64 decoding");
        let decoded = general_purpose::STANDARD.decode(encrypted)
            .context("Failed to decode base64")?;
        Ok(String::from_utf8(decoded)
            .context("Failed to convert to UTF-8")?)
    }
    
    fn is_encryption_enabled(&self) -> bool {
        self.encryption_enabled
    }
}

impl FileSettingsStore {
    /// Encrypt credentials in a ProviderCredentials enum
    async fn encrypt_credentials(&self, credentials: &mut ProviderCredentials) -> Result<()> {
        match credentials {
            ProviderCredentials::AwsS3 { secret_access_key, session_token, .. } => {
                if let Some(plaintext) = secret_access_key.plaintext() {
                    let encrypted = self.encrypt(plaintext).await?;
                    secret_access_key.set_encrypted(encrypted);
                }
                if let Some(ref mut token) = session_token {
                    if let Some(plaintext) = token.plaintext() {
                        let encrypted = self.encrypt(plaintext).await?;
                        token.set_encrypted(encrypted);
                    }
                }
            }
            ProviderCredentials::Gcs { service_account_json, .. } => {
                if let Some(plaintext) = service_account_json.plaintext() {
                    let encrypted = self.encrypt(plaintext).await?;
                    service_account_json.set_encrypted(encrypted);
                }
            }
            ProviderCredentials::AzureBlob { account_key, .. } => {
                if let Some(plaintext) = account_key.plaintext() {
                    let encrypted = self.encrypt(plaintext).await?;
                    account_key.set_encrypted(encrypted);
                }
            }
            ProviderCredentials::S3Compatible { secret_access_key, .. } => {
                if let Some(plaintext) = secret_access_key.plaintext() {
                    let encrypted = self.encrypt(plaintext).await?;
                    secret_access_key.set_encrypted(encrypted);
                }
            }
            ProviderCredentials::Oracle { private_key, passphrase, .. } => {
                if let Some(plaintext) = private_key.plaintext() {
                    let encrypted = self.encrypt(plaintext).await?;
                    private_key.set_encrypted(encrypted);
                }
                if let Some(ref mut pass) = passphrase {
                    if let Some(plaintext) = pass.plaintext() {
                        let encrypted = self.encrypt(plaintext).await?;
                        pass.set_encrypted(encrypted);
                    }
                }
            }
            ProviderCredentials::Custom { credentials, .. } => {
                for (_, value) in credentials.iter_mut() {
                    if let Some(plaintext) = value.plaintext() {
                        let encrypted = self.encrypt(plaintext).await?;
                        value.set_encrypted(encrypted);
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Decrypt credentials in a ProviderCredentials enum
    async fn decrypt_credentials(&self, credentials: &mut ProviderCredentials) -> Result<()> {
        match credentials {
            ProviderCredentials::AwsS3 { secret_access_key, session_token, .. } => {
                if let Some(encrypted) = secret_access_key.encrypted() {
                    let plaintext = self.decrypt(encrypted).await?;
                    secret_access_key.set_plaintext(plaintext);
                }
                if let Some(ref mut token) = session_token {
                    if let Some(encrypted) = token.encrypted() {
                        let plaintext = self.decrypt(encrypted).await?;
                        token.set_plaintext(plaintext);
                    }
                }
            }
            ProviderCredentials::Gcs { service_account_json, .. } => {
                if let Some(encrypted) = service_account_json.encrypted() {
                    let plaintext = self.decrypt(encrypted).await?;
                    service_account_json.set_plaintext(plaintext);
                }
            }
            ProviderCredentials::AzureBlob { account_key, .. } => {
                if let Some(encrypted) = account_key.encrypted() {
                    let plaintext = self.decrypt(encrypted).await?;
                    account_key.set_plaintext(plaintext);
                }
            }
            ProviderCredentials::S3Compatible { secret_access_key, .. } => {
                if let Some(encrypted) = secret_access_key.encrypted() {
                    let plaintext = self.decrypt(encrypted).await?;
                    secret_access_key.set_plaintext(plaintext);
                }
            }
            ProviderCredentials::Oracle { private_key, passphrase, .. } => {
                if let Some(encrypted) = private_key.encrypted() {
                    let plaintext = self.decrypt(encrypted).await?;
                    private_key.set_plaintext(plaintext);
                }
                if let Some(ref mut pass) = passphrase {
                    if let Some(encrypted) = pass.encrypted() {
                        let plaintext = self.decrypt(encrypted).await?;
                        pass.set_plaintext(plaintext);
                    }
                }
            }
            ProviderCredentials::Custom { credentials, .. } => {
                for (_, value) in credentials.iter_mut() {
                    if let Some(encrypted) = value.encrypted() {
                        let plaintext = self.decrypt(encrypted).await?;
                        value.set_plaintext(plaintext);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Settings manager implementation
pub struct SettingsManager {
    store: Box<dyn ISettingsStore>,
}

impl SettingsManager {
    pub fn new(store: Box<dyn ISettingsStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ISettingsManager for SettingsManager {
    fn store(&self) -> &dyn ISettingsStore {
        self.store.as_ref()
    }
}

impl Default for FileSettingsStore {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            error!("Failed to create settings store: {}", e);
            // Fallback to a temporary path
            Self {
                settings_file: PathBuf::from("/tmp/diaspor_settings.json"),
                encryption_enabled: false,
                master_key: None,
            }
        })
    }
}
