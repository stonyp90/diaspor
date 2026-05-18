//! Settings Management System
//!
//! Provides a decoupled settings system organized by setting type.
//! Settings are persisted to disk and can be accessed/modified via Tauri commands.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, info, warn};

/// Setting categories/types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingType {
    /// Logging settings
    Logging,
    /// UI/UX settings
    Ui,
    /// Storage settings
    Storage,
    /// AI/ML settings
    Ai,
    /// Performance settings
    Performance,
    /// Security settings
    Security,
}

/// Logging settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
    /// Log directory path
    pub log_path: Option<String>,
    /// Log level (DEBUG, INFO, WARN, ERROR, TRACE)
    pub log_level: Option<String>,
    /// Maximum log file size in bytes (default: 10MB)
    pub max_file_size: Option<u64>,
    /// Number of rotated log files to keep (default: 5)
    pub max_rotated_files: Option<usize>,
    /// Enable file logging (default: true)
    pub enable_file_logging: Option<bool>,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            log_path: None,
            log_level: Some("INFO".to_string()),
            max_file_size: Some(10 * 1024 * 1024), // 10MB
            max_rotated_files: Some(5),
            enable_file_logging: Some(true),
        }
    }
}

/// UI settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    /// Theme (light, dark, auto)
    pub theme: Option<String>,
    /// Default view mode (grid, list)
    pub default_view: Option<String>,
    /// Show hidden files
    pub show_hidden_files: Option<bool>,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: Some("auto".to_string()),
            default_view: Some("grid".to_string()),
            show_hidden_files: Some(false),
        }
    }
}

/// Storage settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    /// Default storage source ID
    pub default_source_id: Option<String>,
    /// Cache directory path
    pub cache_path: Option<String>,
    /// Maximum cache size in bytes
    pub max_cache_size: Option<u64>,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            default_source_id: None,
            cache_path: None,
            max_cache_size: Some(1024 * 1024 * 1024), // 1GB
        }
    }
}

/// AI settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    /// Default Ollama model
    pub default_model: Option<String>,
    /// Ollama API URL
    pub ollama_url: Option<String>,
    /// Enable AI features
    pub enable_ai: Option<bool>,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            default_model: None,
            ollama_url: Some("http://localhost:11434".to_string()),
            enable_ai: Some(true),
        }
    }
}

/// Performance settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    /// Number of worker threads
    pub worker_threads: Option<usize>,
    /// Enable GPU acceleration
    pub enable_gpu: Option<bool>,
    /// Thumbnail cache size
    pub thumbnail_cache_size: Option<usize>,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            worker_threads: None,
            enable_gpu: Some(true),
            thumbnail_cache_size: Some(1000),
        }
    }
}

/// Security settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    /// Require authentication
    pub require_auth: Option<bool>,
    /// Session timeout in seconds
    pub session_timeout: Option<u64>,
    /// Enable audit logging
    pub enable_audit_log: Option<bool>,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            require_auth: Some(false),
            session_timeout: Some(3600), // 1 hour
            enable_audit_log: Some(true),
        }
    }
}

/// All settings grouped by type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct AppSettings {
    pub logging: LoggingSettings,
    pub ui: UiSettings,
    pub storage: StorageSettings,
    pub ai: AiSettings,
    pub performance: PerformanceSettings,
    pub security: SecuritySettings,
}


/// Settings manager
pub struct SettingsManager {
    settings: Arc<RwLock<AppSettings>>,
    settings_file: PathBuf,
}

impl SettingsManager {
    /// Create a new settings manager
    pub fn new(settings_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(settings_dir)
            .context("Failed to create settings directory")?;

        let settings_file = settings_dir.join("settings.json");
        
        let settings = if settings_file.exists() {
            let content = std::fs::read_to_string(&settings_file)
                .context("Failed to read settings file")?;
            serde_json::from_str::<AppSettings>(&content)
                .unwrap_or_else(|e| {
                    warn!("Failed to parse settings file: {}, using defaults", e);
                    AppSettings::default()
                })
        } else {
            AppSettings::default()
        };

        let manager = Self {
            settings: Arc::new(RwLock::new(settings)),
            settings_file,
        };

        // Save defaults if file didn't exist
        manager.save()?;

        Ok(manager)
    }

    /// Get default settings directory
    pub fn default_settings_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("diaspor")
            .join("settings")
    }

    /// Get all settings
    pub fn get_all(&self) -> AppSettings {
        self.settings.read().clone()
    }

    /// Get logging settings
    pub fn get_logging(&self) -> LoggingSettings {
        self.settings.read().logging.clone()
    }

    /// Get UI settings
    pub fn get_ui(&self) -> UiSettings {
        self.settings.read().ui.clone()
    }

    /// Get storage settings
    pub fn get_storage(&self) -> StorageSettings {
        self.settings.read().storage.clone()
    }

    /// Get AI settings
    pub fn get_ai(&self) -> AiSettings {
        self.settings.read().ai.clone()
    }

    /// Get performance settings
    pub fn get_performance(&self) -> PerformanceSettings {
        self.settings.read().performance.clone()
    }

    /// Get security settings
    pub fn get_security(&self) -> SecuritySettings {
        self.settings.read().security.clone()
    }

    /// Update logging settings
    pub fn update_logging<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut LoggingSettings),
    {
        let mut settings = self.settings.write();
        updater(&mut settings.logging);
        self.save()?;
        Ok(())
    }

    /// Update UI settings
    pub fn update_ui<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut UiSettings),
    {
        let mut settings = self.settings.write();
        updater(&mut settings.ui);
        self.save()?;
        Ok(())
    }

    /// Update storage settings
    pub fn update_storage<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut StorageSettings),
    {
        let mut settings = self.settings.write();
        updater(&mut settings.storage);
        self.save()?;
        Ok(())
    }

    /// Update AI settings
    pub fn update_ai<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut AiSettings),
    {
        let mut settings = self.settings.write();
        updater(&mut settings.ai);
        self.save()?;
        Ok(())
    }

    /// Update performance settings
    pub fn update_performance<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut PerformanceSettings),
    {
        let mut settings = self.settings.write();
        updater(&mut settings.performance);
        self.save()?;
        Ok(())
    }

    /// Update security settings
    pub fn update_security<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut SecuritySettings),
    {
        let mut settings = self.settings.write();
        updater(&mut settings.security);
        self.save()?;
        Ok(())
    }

    /// Get log directory path (with fallback to default)
    pub fn get_log_directory(&self) -> PathBuf {
        let logging = self.get_logging();
        if let Some(ref log_path) = logging.log_path {
            PathBuf::from(log_path)
        } else {
            // Default log directory - use platform-specific app data directory
            #[cfg(windows)]
            {
                // On Windows, prefer LocalAppData over Roaming for logs
                dirs::data_local_dir()
                    .or_else(|| dirs::data_dir())
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("diaspor")
                    .join("logs")
            }
            #[cfg(not(windows))]
            {
                // On Unix-like systems, use standard data directory
                dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("diaspor")
                    .join("logs")
            }
        }
    }

    /// Save settings to disk
    fn save(&self) -> Result<()> {
        let settings = self.settings.read();
        let content = serde_json::to_string_pretty(&*settings)
            .context("Failed to serialize settings")?;
        std::fs::write(&self.settings_file, content)
            .context("Failed to write settings file")?;
        debug!("Settings saved to {:?}", self.settings_file);
        Ok(())
    }

    /// Reset settings to defaults
    pub fn reset(&self) -> Result<()> {
        let mut settings = self.settings.write();
        *settings = AppSettings::default();
        self.save()?;
        info!("Settings reset to defaults");
        Ok(())
    }
}

/// Global settings manager instance
static SETTINGS_MANAGER: once_cell::sync::Lazy<Arc<SettingsManager>> =
    once_cell::sync::Lazy::new(|| {
        let settings_dir = SettingsManager::default_settings_dir();
        match SettingsManager::new(&settings_dir) {
            Ok(manager) => Arc::new(manager),
            Err(e) => {
                warn!("Failed to initialize settings manager: {}, using in-memory defaults", e);
                // Try fallback to current directory
                let fallback_dir = std::path::PathBuf::from(".").join(".diaspor-settings");
                match SettingsManager::new(&fallback_dir) {
                    Ok(manager) => Arc::new(manager),
                    Err(e2) => {
                        warn!("Fallback settings directory also failed: {}, using in-memory only", e2);
                        // Create a minimal in-memory settings manager
                        Arc::new(SettingsManager {
                            settings: Arc::new(RwLock::new(AppSettings::default())),
                            settings_file: PathBuf::from("/tmp/diaspor-settings-fallback.json"),
                        })
                    }
                }
            }
        }
    });

/// Get the global settings manager
pub fn get_settings() -> Arc<SettingsManager> {
    Arc::clone(&SETTINGS_MANAGER)
}
