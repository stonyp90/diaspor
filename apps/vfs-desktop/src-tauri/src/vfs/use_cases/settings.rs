//! Settings Management Use Cases
//!
//! Use cases for managing application settings by type.

use anyhow::Result;

use crate::settings::{SettingType, AppSettings};

// ============================================================================
// Get Settings Use Case
// ============================================================================

/// Input DTO for getting settings
#[derive(Debug, Clone)]
pub struct GetSettingsInput {
    pub setting_type: Option<SettingType>,
}

/// Output DTO for getting settings
#[derive(Debug, Clone)]
pub struct GetSettingsOutput {
    pub settings: AppSettings,
}

/// Use case: Get application settings
pub struct GetSettingsUseCase;

impl GetSettingsUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the get settings use case
    pub fn execute(&self, _input: GetSettingsInput) -> Result<GetSettingsOutput> {
        use crate::settings::get_settings;
        
        let settings = get_settings().get_all();
        
        Ok(GetSettingsOutput { settings })
    }
}

impl Default for GetSettingsUseCase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Update Settings Use Case
// ============================================================================

/// Input DTO for updating logging settings
#[derive(Debug, Clone)]
pub struct UpdateLoggingSettingsInput {
    pub log_path: Option<String>,
    pub log_level: Option<String>,
    pub max_file_size: Option<u64>,
    pub max_rotated_files: Option<usize>,
    pub enable_file_logging: Option<bool>,
}

/// Input DTO for updating UI settings
#[derive(Debug, Clone)]
pub struct UpdateUiSettingsInput {
    pub theme: Option<String>,
    pub default_view: Option<String>,
    pub show_hidden_files: Option<bool>,
}

/// Input DTO for updating AI settings
#[derive(Debug, Clone)]
pub struct UpdateAiSettingsInput {
    pub default_model: Option<String>,
    pub ollama_url: Option<String>,
    pub enable_ai: Option<bool>,
}

/// Output DTO for updating settings
#[derive(Debug, Clone)]
pub struct UpdateSettingsOutput {
    pub success: bool,
    pub message: String,
}

/// Use case: Update logging settings
pub struct UpdateLoggingSettingsUseCase;

impl UpdateLoggingSettingsUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the update logging settings use case
    pub fn execute(&self, input: UpdateLoggingSettingsInput) -> Result<UpdateSettingsOutput> {
        use crate::settings::get_settings;
        
        let settings_mgr = get_settings();
        
        settings_mgr.update_logging(|logging| {
            if let Some(path) = input.log_path {
                logging.log_path = Some(path);
            }
            if let Some(level) = input.log_level {
                logging.log_level = Some(level);
            }
            if let Some(size) = input.max_file_size {
                logging.max_file_size = Some(size);
            }
            if let Some(files) = input.max_rotated_files {
                logging.max_rotated_files = Some(files);
            }
            if let Some(enabled) = input.enable_file_logging {
                logging.enable_file_logging = Some(enabled);
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to update logging settings: {}", e))?;
        
        Ok(UpdateSettingsOutput {
            success: true,
            message: "Logging settings updated successfully".to_string(),
        })
    }
}

impl Default for UpdateLoggingSettingsUseCase {
    fn default() -> Self {
        Self::new()
    }
}

/// Use case: Update UI settings
pub struct UpdateUiSettingsUseCase;

impl UpdateUiSettingsUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the update UI settings use case
    pub fn execute(&self, input: UpdateUiSettingsInput) -> Result<UpdateSettingsOutput> {
        use crate::settings::get_settings;
        
        let settings_mgr = get_settings();
        
        settings_mgr.update_ui(|ui| {
            if let Some(theme) = input.theme {
                ui.theme = Some(theme);
            }
            if let Some(view) = input.default_view {
                ui.default_view = Some(view);
            }
            if let Some(show) = input.show_hidden_files {
                ui.show_hidden_files = Some(show);
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to update UI settings: {}", e))?;
        
        Ok(UpdateSettingsOutput {
            success: true,
            message: "UI settings updated successfully".to_string(),
        })
    }
}

impl Default for UpdateUiSettingsUseCase {
    fn default() -> Self {
        Self::new()
    }
}

/// Use case: Update AI settings
pub struct UpdateAiSettingsUseCase;

impl UpdateAiSettingsUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the update AI settings use case
    pub fn execute(&self, input: UpdateAiSettingsInput) -> Result<UpdateSettingsOutput> {
        use crate::settings::get_settings;
        
        let settings_mgr = get_settings();
        
        settings_mgr.update_ai(|ai| {
            if let Some(model) = input.default_model {
                ai.default_model = Some(model);
            }
            if let Some(url) = input.ollama_url {
                ai.ollama_url = Some(url);
            }
            if let Some(enabled) = input.enable_ai {
                ai.enable_ai = Some(enabled);
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to update AI settings: {}", e))?;
        
        Ok(UpdateSettingsOutput {
            success: true,
            message: "AI settings updated successfully".to_string(),
        })
    }
}

impl Default for UpdateAiSettingsUseCase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Reset Settings Use Case
// ============================================================================

/// Input DTO for resetting settings
#[derive(Debug, Clone)]
pub struct ResetSettingsInput {
    pub setting_type: Option<SettingType>,
}

/// Output DTO for resetting settings
#[derive(Debug, Clone)]
pub struct ResetSettingsOutput {
    pub success: bool,
    pub message: String,
}

/// Use case: Reset settings to defaults
pub struct ResetSettingsUseCase;

impl ResetSettingsUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the reset settings use case
    pub fn execute(&self, _input: ResetSettingsInput) -> Result<ResetSettingsOutput> {
        use crate::settings::get_settings;
        
        get_settings()
            .reset()
            .map_err(|e| anyhow::anyhow!("Failed to reset settings: {}", e))?;
        
        Ok(ResetSettingsOutput {
            success: true,
            message: "Settings reset to defaults".to_string(),
        })
    }
}

impl Default for ResetSettingsUseCase {
    fn default() -> Self {
        Self::new()
    }
}
