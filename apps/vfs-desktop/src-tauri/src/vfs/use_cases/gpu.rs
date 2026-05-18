//! GPU Use Cases
//!
//! Use cases for GPU detection, metrics collection, and AI model management.

use anyhow::Result;

use crate::gpu::{GpuInfo, GpuMetrics};

// ============================================================================
// Detect GPUs Use Case
// ============================================================================

/// Input DTO for detecting GPUs
#[derive(Debug, Clone)]
pub struct DetectGpusInput;

/// Output DTO for GPU detection
#[derive(Debug, Clone)]
pub struct DetectGpusOutput {
    pub gpus: Vec<GpuInfo>,
}

/// Use case: Detect all available GPUs
pub struct DetectGpusUseCase;

impl DetectGpusUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the detect GPUs use case
    pub fn execute(&self, _input: DetectGpusInput) -> Result<DetectGpusOutput> {
        use crate::gpu::detect_gpus;
        
        let gpus = detect_gpus();
        
        Ok(DetectGpusOutput { gpus })
    }
}

impl Default for DetectGpusUseCase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Get GPU Metrics Use Case
// ============================================================================

/// Input DTO for getting GPU metrics
#[derive(Debug, Clone)]
pub struct GetGpuMetricsInput {
    pub gpu_id: u32,
}

/// Output DTO for GPU metrics
#[derive(Debug, Clone)]
pub struct GetGpuMetricsOutput {
    pub metrics: GpuMetrics,
}

/// Use case: Get current GPU metrics
pub struct GetGpuMetricsUseCase;

impl GetGpuMetricsUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the get GPU metrics use case
    pub fn execute(&self, input: GetGpuMetricsInput) -> Result<GetGpuMetricsOutput> {
        use crate::gpu::get_current_metrics;
        
        let metrics = get_current_metrics(input.gpu_id);
        
        Ok(GetGpuMetricsOutput { metrics })
    }
}

impl Default for GetGpuMetricsUseCase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Manage AI Model Use Case
// ============================================================================

/// Input DTO for starting an AI model
#[derive(Debug, Clone)]
pub struct StartModelInput {
    pub model_name: String,
    pub ollama_url: Option<String>,
}

/// Input DTO for stopping an AI model
#[derive(Debug, Clone)]
pub struct StopModelInput;

/// Output DTO for model operations
#[derive(Debug, Clone)]
pub struct ModelOperationOutput {
    pub success: bool,
    pub message: String,
}

/// Use case: Start an AI model
pub struct StartModelUseCase;

impl StartModelUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the start model use case
    pub async fn execute(&self, input: StartModelInput) -> Result<ModelOperationOutput> {
        // Note: This use case wraps the command layer
        // In a clean architecture, this would call a service/port instead
        use crate::commands::{start_model, ModelConfig};
        
        let config = ModelConfig {
            name: input.model_name,
            ollama_url: input.ollama_url,
        };
        
        match start_model(config).await {
            Ok(_status) => Ok(ModelOperationOutput {
                success: true,
                message: "Model started successfully".to_string(),
            }),
            Err(e) => Ok(ModelOperationOutput {
                success: false,
                message: format!("Failed to start model: {}", e),
            }),
        }
    }
}

impl Default for StartModelUseCase {
    fn default() -> Self {
        Self::new()
    }
}

/// Use case: Stop an AI model
pub struct StopModelUseCase;

impl StopModelUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the stop model use case
    pub async fn execute(&self, _input: StopModelInput) -> Result<ModelOperationOutput> {
        // Note: This use case wraps the command layer
        // In a clean architecture, this would call a service/port instead
        use crate::commands::stop_model;
        
        match stop_model().await {
            Ok(_) => Ok(ModelOperationOutput {
                success: true,
                message: "Model stopped successfully".to_string(),
            }),
            Err(e) => Ok(ModelOperationOutput {
                success: false,
                message: format!("Failed to stop model: {}", e),
            }),
        }
    }
}

impl Default for StopModelUseCase {
    fn default() -> Self {
        Self::new()
    }
}
