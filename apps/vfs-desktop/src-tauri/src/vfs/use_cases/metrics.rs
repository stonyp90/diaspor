//! Metrics and Alerting Use Cases
//!
//! Use cases for collecting system/GPU metrics and managing alert thresholds.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::commands::AllMetrics;

// ============================================================================
// Get Metrics Use Case
// ============================================================================

/// Input DTO for getting all metrics
#[derive(Debug, Clone)]
pub struct GetAllMetricsInput;

/// Output DTO for all metrics
#[derive(Debug)]
pub struct GetAllMetricsOutput {
    pub metrics: AllMetrics,
}

/// Use case: Get all system and GPU metrics
pub struct GetAllMetricsUseCase;

impl GetAllMetricsUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the get all metrics use case
    pub fn execute(&self, _input: GetAllMetricsInput) -> Result<GetAllMetricsOutput> {
        // Note: This use case wraps the command layer
        // In a clean architecture, this would call a service/port instead
        use crate::commands::get_all_metrics;
        
        let metrics = get_all_metrics();
        
        Ok(GetAllMetricsOutput { metrics })
    }
}

impl Default for GetAllMetricsUseCase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Metrics Alerting Use Case
// ============================================================================

/// Alert threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    pub cpu: f32,
    pub memory: f32,
    pub gpu: f32,
    pub temperature: f32,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            cpu: 80.0,
            memory: 85.0,
            gpu: 90.0,
            temperature: 85.0,
        }
    }
}

/// Input DTO for checking alerts
#[derive(Debug)]
pub struct CheckAlertsInput {
    pub metrics: AllMetrics,
    pub thresholds: AlertThresholds,
}

/// Alert result
#[derive(Debug, Clone)]
pub struct Alert {
    pub metric_type: String,
    pub current_value: f32,
    pub threshold: f32,
    pub severity: AlertSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    Warning,
    Critical,
}

/// Output DTO for alert checking
#[derive(Debug, Clone)]
pub struct CheckAlertsOutput {
    pub alerts: Vec<Alert>,
}

/// Use case: Check metrics against alert thresholds
pub struct CheckAlertsUseCase;

impl CheckAlertsUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the check alerts use case
    pub fn execute(&self, input: CheckAlertsInput) -> Result<CheckAlertsOutput> {
        let mut alerts = Vec::new();
        
        // Check CPU
        if input.metrics.system.cpu_usage >= input.thresholds.cpu {
            alerts.push(Alert {
                metric_type: "cpu".to_string(),
                current_value: input.metrics.system.cpu_usage,
                threshold: input.thresholds.cpu,
                severity: AlertSeverity::Warning,
            });
        }
        
        // Check Memory
        if input.metrics.system.memory_usage_percent >= input.thresholds.memory {
            alerts.push(Alert {
                metric_type: "memory".to_string(),
                current_value: input.metrics.system.memory_usage_percent,
                threshold: input.thresholds.memory,
                severity: AlertSeverity::Warning,
            });
        }
        
        // Check GPU utilization and temperature
        for (i, gpu) in input.metrics.gpus.iter().enumerate() {
            if gpu.current.gpu_utilization >= input.thresholds.gpu {
                alerts.push(Alert {
                    metric_type: format!("gpu_{}", i),
                    current_value: gpu.current.gpu_utilization,
                    threshold: input.thresholds.gpu,
                    severity: AlertSeverity::Warning,
                });
            }
            
            if let Some(temp) = gpu.current.temperature_celsius {
                if temp >= input.thresholds.temperature {
                    alerts.push(Alert {
                        metric_type: format!("gpu_{}_temperature", i),
                        current_value: temp,
                        threshold: input.thresholds.temperature,
                        severity: AlertSeverity::Critical,
                    });
                }
            }
        }
        
        Ok(CheckAlertsOutput { alerts })
    }
}

impl Default for CheckAlertsUseCase {
    fn default() -> Self {
        Self::new()
    }
}

/// Input DTO for updating alert thresholds
#[derive(Debug, Clone)]
pub struct UpdateAlertThresholdsInput {
    pub thresholds: AlertThresholds,
}

/// Output DTO for updating thresholds
#[derive(Debug, Clone)]
pub struct UpdateAlertThresholdsOutput {
    pub success: bool,
    pub message: String,
}

/// Use case: Update alert thresholds
pub struct UpdateAlertThresholdsUseCase;

impl UpdateAlertThresholdsUseCase {
    pub fn new() -> Self {
        Self
    }

    /// Execute the update alert thresholds use case
    pub fn execute(&self, input: UpdateAlertThresholdsInput) -> Result<UpdateAlertThresholdsOutput> {
        // Validation
        if input.thresholds.cpu < 0.0 || input.thresholds.cpu > 100.0 {
            return Ok(UpdateAlertThresholdsOutput {
                success: false,
                message: "CPU threshold must be between 0 and 100".to_string(),
            });
        }
        
        if input.thresholds.memory < 0.0 || input.thresholds.memory > 100.0 {
            return Ok(UpdateAlertThresholdsOutput {
                success: false,
                message: "Memory threshold must be between 0 and 100".to_string(),
            });
        }
        
        if input.thresholds.gpu < 0.0 || input.thresholds.gpu > 100.0 {
            return Ok(UpdateAlertThresholdsOutput {
                success: false,
                message: "GPU threshold must be between 0 and 100".to_string(),
            });
        }
        
        // In a real implementation, this would persist to settings
        // For now, we'll just validate and return success
        Ok(UpdateAlertThresholdsOutput {
            success: true,
            message: "Alert thresholds updated successfully".to_string(),
        })
    }
}

impl Default for UpdateAlertThresholdsUseCase {
    fn default() -> Self {
        Self::new()
    }
}
