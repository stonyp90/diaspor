//! AI Settings and Configuration Use Cases
//!
//! Use cases for configuring AI settings, checking dependencies, and installing AI tools.

use anyhow::Result;
use crate::vfs::platform::CommandBuilder;

// ============================================================================
// Configure AI Settings Use Case
// ============================================================================

/// Input DTO for configuring AI settings
#[derive(Debug, Clone)]
pub struct ConfigureAiInput {
    pub default_model: Option<String>,
    pub ollama_url: Option<String>,
    pub enable_ai: Option<bool>,
}

/// Output DTO for AI configuration
#[derive(Debug, Clone)]
pub struct ConfigureAiOutput {
    pub success: bool,
    pub message: String,
}

/// Use case: Configure AI settings
pub struct ConfigureAiUseCase;

impl ConfigureAiUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the configure AI use case
    pub fn execute(&self, input: ConfigureAiInput) -> Result<ConfigureAiOutput> {
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
        .map_err(|e| anyhow::anyhow!("Failed to configure AI settings: {}", e))?;
        
        Ok(ConfigureAiOutput {
            success: true,
            message: "AI settings configured successfully".to_string(),
        })
    }
}

impl Default for ConfigureAiUseCase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Check AI Dependencies Use Case
// ============================================================================

/// Input DTO for checking dependencies
#[derive(Debug, Clone)]
pub struct CheckDependenciesInput {
    pub dependency: DependencyType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyType {
    Ollama,
    Ffmpeg,
}

/// Output DTO for dependency check
#[derive(Debug, Clone)]
pub struct CheckDependenciesOutput {
    pub installed: bool,
    pub version: Option<String>,
    pub message: String,
}

/// Use case: Check if AI dependencies are installed
pub struct CheckDependenciesUseCase;

impl CheckDependenciesUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the check dependencies use case
    pub async fn execute(&self, input: CheckDependenciesInput) -> Result<CheckDependenciesOutput> {
        match input.dependency {
            DependencyType::Ollama => {
                let result = CommandBuilder::new("ollama")
                    .arg("--version")
                    .output();
                
                match result {
                    Ok(output) if output.status.success() => {
                        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        Ok(CheckDependenciesOutput {
                            installed: true,
                            version: Some(version),
                            message: "Ollama is installed".to_string(),
                        })
                    }
                    _ => Ok(CheckDependenciesOutput {
                        installed: false,
                        version: None,
                        message: "Ollama is not installed".to_string(),
                    }),
                }
            }
            DependencyType::Ffmpeg => {
                let result = CommandBuilder::new("ffmpeg")
                    .arg("-version")
                    .output();
                
                match result {
                    Ok(output) if output.status.success() => {
                        let version_output = String::from_utf8_lossy(&output.stdout);
                        // Extract version from first line
                        let version = version_output
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(2))
                            .map(|s| s.to_string());
                        
                        Ok(CheckDependenciesOutput {
                            installed: true,
                            version,
                            message: "FFmpeg is installed".to_string(),
                        })
                    }
                    _ => Ok(CheckDependenciesOutput {
                        installed: false,
                        version: None,
                        message: "FFmpeg is not installed".to_string(),
                    }),
                }
            }
        }
    }
}

impl Default for CheckDependenciesUseCase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Install AI Dependencies Use Case
// ============================================================================

/// Input DTO for installing dependencies
#[derive(Debug, Clone)]
pub struct InstallDependencyInput {
    pub dependency: DependencyType,
}

/// Output DTO for installation
#[derive(Debug, Clone)]
pub struct InstallDependencyOutput {
    pub success: bool,
    pub message: String,
    pub requires_restart: bool,
}

/// Use case: Install AI dependencies
pub struct InstallDependencyUseCase;

impl InstallDependencyUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the install dependency use case
    pub async fn execute(&self, input: InstallDependencyInput) -> Result<InstallDependencyOutput> {
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            return Ok(InstallDependencyOutput {
                success: false,
                message: "Unsupported platform".to_string(),
                requires_restart: false,
            });
        };

        match input.dependency {
            DependencyType::Ollama => self.install_ollama(platform).await,
            DependencyType::Ffmpeg => self.install_ffmpeg(platform).await,
        }
    }

    async fn install_ollama(&self, platform: &str) -> Result<InstallDependencyOutput> {
        match platform {
            "macos" => {
                let brew = match crate::vfs::platform::resolve_brew_path() {
                    Some(path) => path,
                    None => {
                        return Ok(InstallDependencyOutput {
                            success: false,
                            message: "Homebrew not found. Please install Ollama manually:\n\n1. Install Homebrew: https://brew.sh\n2. Then run: brew install ollama\n\nOr download from: https://ollama.com".to_string(),
                            requires_restart: false,
                        });
                    }
                };

                let result = CommandBuilder::new(&brew)
                    .args(["install", "ollama"])
                    .output();
                
                match result {
                    Ok(output) if output.status.success() => Ok(InstallDependencyOutput {
                        success: true,
                        message: "Ollama installed successfully via Homebrew".to_string(),
                        requires_restart: true,
                    }),
                    _ => Ok(InstallDependencyOutput {
                        success: false,
                        message: "Please install Ollama manually. Run: brew install ollama".to_string(),
                        requires_restart: false,
                    }),
                }
            }
            "windows" => {
                let result = CommandBuilder::new("winget")
                    .args(["install", "Ollama.Ollama"])
                    .output();
                
                match result {
                    Ok(output) if output.status.success() => Ok(InstallDependencyOutput {
                        success: true,
                        message: "Ollama installed successfully via winget".to_string(),
                        requires_restart: true,
                    }),
                    _ => Ok(InstallDependencyOutput {
                        success: false,
                        message: "Please install Ollama manually. Download from https://ollama.ai/download".to_string(),
                        requires_restart: false,
                    }),
                }
            }
            "linux" => {
                let result = CommandBuilder::new("sh")
                    .arg("-c")
                    .arg("curl -fsSL https://ollama.ai/install.sh | sh")
                    .output();
                
                match result {
                    Ok(output) if output.status.success() => Ok(InstallDependencyOutput {
                        success: true,
                        message: "Ollama installed successfully".to_string(),
                        requires_restart: true,
                    }),
                    _ => Ok(InstallDependencyOutput {
                        success: false,
                        message: "Please install Ollama manually. Run: curl -fsSL https://ollama.ai/install.sh | sh".to_string(),
                        requires_restart: false,
                    }),
                }
            }
            _ => Ok(InstallDependencyOutput {
                success: false,
                message: "Unsupported platform".to_string(),
                requires_restart: false,
            }),
        }
    }

    /// Check if FFmpeg is installed by checking common locations
    fn check_ffmpeg_installed(&self) -> bool {
        let candidates = if cfg!(target_os = "macos") {
            vec![
                "ffmpeg",
                "/opt/homebrew/bin/ffmpeg",
                "/usr/local/bin/ffmpeg",
                "/usr/bin/ffmpeg",
            ]
        } else if cfg!(target_os = "windows") {
            vec![
                "ffmpeg",
                "C:\\ffmpeg\\bin\\ffmpeg.exe",
                "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
            ]
        } else {
            vec![
                "ffmpeg",
                "/usr/bin/ffmpeg",
                "/usr/local/bin/ffmpeg",
            ]
        };
        
        for candidate in candidates {
            let result = CommandBuilder::new(candidate)
                .arg("-version")
                .stdout_null()
                .stderr_null()
                .status();
            
            if let Ok(status) = result {
                if status.success() {
                    return true;
                }
            }
        }
        
        false
    }

    async fn install_ffmpeg(&self, platform: &str) -> Result<InstallDependencyOutput> {
        // First check if FFmpeg is already installed
        if self.check_ffmpeg_installed() {
            return Ok(InstallDependencyOutput {
                success: true,
                message: "FFmpeg is already installed".to_string(),
                requires_restart: false,
            });
        }

        match platform {
            "macos" => {
                // Resolve the full brew path (GUI apps don't inherit shell PATH)
                let brew = match crate::vfs::platform::resolve_brew_path() {
                    Some(path) => path,
                    None => {
                        return Ok(InstallDependencyOutput {
                            success: false,
                            message: "FFmpeg not found. To install:\n\n1. Install Homebrew: https://brew.sh\n2. Then run: brew install ffmpeg\n\nOr download FFmpeg directly from: https://ffmpeg.org/download.html".to_string(),
                            requires_restart: false,
                        });
                    }
                };

                let result = CommandBuilder::new(&brew)
                    .args(["install", "ffmpeg"])
                    .output();
                
                match result {
                    Ok(output) if output.status.success() => Ok(InstallDependencyOutput {
                        success: true,
                        message: "FFmpeg installed successfully via Homebrew".to_string(),
                        requires_restart: false,
                    }),
                    _ => Ok(InstallDependencyOutput {
                        success: false,
                        message: "FFmpeg installation failed. Please install manually:\n\nRun: brew install ffmpeg\n\nOr download from: https://ffmpeg.org/download.html".to_string(),
                        requires_restart: false,
                    }),
                }
            }
            "windows" => {
                let result = CommandBuilder::new("winget")
                    .args(["install", "ffmpeg"])
                    .output();
                
                match result {
                    Ok(output) if output.status.success() => Ok(InstallDependencyOutput {
                        success: true,
                        message: "FFmpeg installed successfully via winget".to_string(),
                        requires_restart: false,
                    }),
                    _ => Ok(InstallDependencyOutput {
                        success: false,
                        message: "Please install FFmpeg manually. Download from https://ffmpeg.org/download.html".to_string(),
                        requires_restart: false,
                    }),
                }
            }
            "linux" => {
                let result = CommandBuilder::new("sh")
                    .arg("-c")
                    .arg("sudo apt-get update && sudo apt-get install -y ffmpeg")
                    .output();
                
                match result {
                    Ok(output) if output.status.success() => Ok(InstallDependencyOutput {
                        success: true,
                        message: "FFmpeg installed successfully".to_string(),
                        requires_restart: false,
                    }),
                    _ => Ok(InstallDependencyOutput {
                        success: false,
                        message: "Please install FFmpeg manually. Run: sudo apt-get install ffmpeg".to_string(),
                        requires_restart: false,
                    }),
                }
            }
            _ => Ok(InstallDependencyOutput {
                success: false,
                message: "Unsupported platform".to_string(),
                requires_restart: false,
            }),
        }
    }
}

impl Default for InstallDependencyUseCase {
    fn default() -> Self {
        Self::new()
    }
}
