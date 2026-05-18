//! Operation Tracker - Tracks uploads, downloads, deletes, and other file operations
//!
//! Provides a unified system for tracking file operations with:
//! - Progress tracking
//! - Operation history
//! - State persistence

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{error, info};
use uuid::Uuid;
use chrono::Utc;

/// Operation type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationType {
    Upload,
    Download,
    Delete,
    Move,
    Copy,
    Paste,
    Rename,
    CreateDir,
    RemoveDir,
    AddTag,
    RemoveTag,
    SetFavorite,
    SetRating,
    SetComment,
    SetColorLabel,
    TierChange,
    Transcribe,
    Transcode,
    AutoTag,
}

/// Operation status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Canceled,
}

/// File information for multi-file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationFile {
    /// Local path (source)
    pub local_path: String,
    /// Remote path (destination)
    pub remote_path: String,
    /// File size in bytes
    pub file_size: u64,
    /// Bytes processed for this file
    pub bytes_processed: u64,
    /// Status of this file (if different from operation status)
    pub status: Option<OperationStatus>,
    /// Error message if this file failed
    pub error: Option<String>,
}

/// Operation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// Unique operation ID
    pub operation_id: String,
    /// Operation type
    pub operation_type: OperationType,
    /// Source ID
    pub source_id: String,
    /// Source path (for downloads/deletes) - primary path or summary
    pub source_path: String,
    /// Destination path (for uploads/downloads) - primary path or summary
    pub destination_path: Option<String>,
    /// File size (if applicable) - total size for multi-file operations
    pub file_size: Option<u64>,
    /// Bytes processed - total bytes processed across all files
    pub bytes_processed: u64,
    /// Operation status
    pub status: OperationStatus,
    /// Error message if failed
    pub error: Option<String>,
    /// Files involved in this operation (for multi-file operations)
    pub files: Option<Vec<OperationFile>>,
    /// Total number of files in this operation
    pub file_count: Option<usize>,
    /// Metrics (CPU, memory, network, etc.)
    pub cpu_usage_percent: Option<f32>,
    pub memory_usage_mb: Option<u64>,
    pub network_tx_bytes_sec: Option<u64>,
    pub network_rx_bytes_sec: Option<u64>,
    /// User ID who performed the operation (for audit)
    pub user_id: Option<String>,
    /// Organization ID (for organization audit)
    pub organization_id: Option<String>,
    /// Timestamp when operation was created
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp when operation was completed
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp of last update
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Tags associated with this operation (for tag operations)
    pub tags: Option<Vec<String>>,
    /// Asset information (metadata, tags, etc.)
    pub asset_info: Option<serde_json::Value>,
    /// Additional metadata (key-value pairs)
    pub metadata: Option<serde_json::Value>,
}

/// Operation tracker manager
pub struct OperationTracker {
    /// Active and completed operations
    operations: Arc<RwLock<HashMap<String, Operation>>>,
    /// State file path
    state_file: PathBuf,
    /// Audit log file path (persists all operations)
    audit_file: PathBuf,
    /// Maximum number of completed operations to keep in memory
    max_history: usize,
}

impl OperationTracker {
    pub fn new(state_dir: &Path, max_history: usize) -> Result<Self> {
        std::fs::create_dir_all(state_dir)
            .context("Failed to create operation tracker state directory")?;
        
        let state_file = state_dir.join("operations.json");
        let audit_file = state_dir.join("audit_log.jsonl"); // JSON Lines format for append-only log
        
        let tracker = Self {
            operations: Arc::new(RwLock::new(HashMap::new())),
            state_file,
            audit_file,
            max_history,
        };
        
        // Load existing operations
        tracker.load_state()?;
        
        Ok(tracker)
    }

    /// Load operations from disk
    fn load_state(&self) -> Result<()> {
        if !self.state_file.exists() {
            return Ok(());
        }

        let data = std::fs::read_to_string(&self.state_file)
            .context("Failed to read operations state file")?;
        
        let operations: HashMap<String, Operation> = serde_json::from_str(&data)
            .context("Failed to parse operations state file")?;
        
        let mut ops = self.operations.write();
        *ops = operations;
        
        info!("Loaded {} operations from state file", ops.len());
        Ok(())
    }

    /// Save operations to disk
    pub fn save_state(&self) -> Result<()> {
        let ops = self.operations.read();
        let data = serde_json::to_string_pretty(&*ops)
            .context("Failed to serialize operations")?;
        
        std::fs::write(&self.state_file, data)
            .context("Failed to write operations state file")?;
        
        Ok(())
    }

    /// Clear all operations (for reset/restart)
    pub fn clear_all(&self) -> Result<()> {
        {
            let mut ops = self.operations.write();
            ops.clear();
        }
        
        // Delete the state file if it exists
        if self.state_file.exists() {
            std::fs::remove_file(&self.state_file)
                .context("Failed to delete operations state file")?;
        }
        
        info!("Cleared all operations");
        Ok(())
    }

    /// Create a new operation
    pub fn create_operation(
        &self,
        operation_type: OperationType,
        source_id: String,
        source_path: String,
        destination_path: Option<String>,
        file_size: Option<u64>,
    ) -> String {
        self.create_operation_with_context(
            operation_type,
            source_id,
            source_path,
            destination_path,
            file_size,
            None, // user_id
            None, // organization_id
        )
    }

    /// Create a new operation with user and organization context
    #[allow(clippy::too_many_arguments)]
    pub fn create_operation_with_context(
        &self,
        operation_type: OperationType,
        source_id: String,
        source_path: String,
        destination_path: Option<String>,
        file_size: Option<u64>,
        user_id: Option<String>,
        organization_id: Option<String>,
    ) -> String {
        self.create_multi_file_operation_with_context(
            operation_type,
            source_id,
            source_path,
            destination_path,
            file_size,
            None, // files
            user_id,
            organization_id,
        )
    }

    /// Create a new multi-file operation
    pub fn create_multi_file_operation(
        &self,
        operation_type: OperationType,
        source_id: String,
        source_path: String,
        destination_path: Option<String>,
        files: Option<Vec<OperationFile>>,
    ) -> String {
        self.create_multi_file_operation_with_context(
            operation_type,
            source_id,
            source_path,
            destination_path,
            files.as_ref().map(|f| f.iter().map(|file| file.file_size).sum()),
            files,
            None, // user_id
            None, // organization_id
        )
    }

    /// Create a new multi-file operation with user and organization context
    #[allow(clippy::too_many_arguments)]
    pub fn create_multi_file_operation_with_context(
        &self,
        operation_type: OperationType,
        source_id: String,
        source_path: String,
        destination_path: Option<String>,
        file_size: Option<u64>,
        files: Option<Vec<OperationFile>>,
        user_id: Option<String>,
        organization_id: Option<String>,
    ) -> String {
        let operation_id = Uuid::new_v4().to_string();
        let now = Some(Utc::now());
        
        let file_count = files.as_ref().map(|f| f.len());
        let total_size = file_size.or_else(|| {
            files.as_ref().map(|f| f.iter().map(|file| file.file_size).sum())
        });
        
        let operation = Operation {
            operation_id: operation_id.clone(),
            operation_type,
            source_id,
            source_path,
            destination_path,
            file_size: total_size,
            bytes_processed: 0,
            status: OperationStatus::Pending,
            error: None,
            files,
            file_count,
            cpu_usage_percent: None,
            memory_usage_mb: None,
            network_tx_bytes_sec: None,
            network_rx_bytes_sec: None,
            user_id,
            organization_id,
            created_at: now,
            completed_at: None,
            last_updated_at: now,
            tags: None,
            asset_info: None,
            metadata: None,
        };
        
        {
            let mut ops = self.operations.write();
            ops.insert(operation_id.clone(), operation.clone());
        }
        
        // Append to audit log (append-only for complete history)
        self.append_to_audit_log(&operation);
        
        // Defer save_state to avoid blocking - spawn in background
        // This prevents blocking the async runtime with synchronous file I/O
        // Serialize data while holding the lock, then drop lock before spawning
        let state_file = self.state_file.clone();
        let data = {
            let ops = self.operations.read();
            serde_json::to_string_pretty(&*ops).ok()
        };
        
        if let Some(data) = data {
            tokio::spawn(async move {
                if let Err(e) = tokio::fs::write(&state_file, data).await {
                    error!("Failed to save operation state: {}", e);
                }
            });
        }
        
        tracing::debug!(target: "agent_log", r#"{{"location":"operation_tracker.rs:336","message":"Created operation","data":{{"operation_id":"{}","operation_type":"{:?}","source_id":"{}","source_path":"{}","status":"{:?}"}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"D"}}"#, 
                operation_id, operation.operation_type, operation.source_id, operation.source_path, operation.status,
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        info!("Created operation: {}", operation_id);
        operation_id
    }

    /// Append operation to audit log (append-only, preserves all history)
    fn append_to_audit_log(&self, operation: &Operation) {
        if let Ok(json) = serde_json::to_string(operation) {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.audit_file)
            {
                use std::io::Write;
                if let Err(e) = writeln!(file, "{}", json) {
                    error!("Failed to write to audit log: {}", e);
                }
            }
        }
    }

    /// Update operation progress
    pub fn update_progress(
        &self,
        operation_id: &str,
        bytes_processed: u64,
    ) -> Result<()> {
        self.update_operation_progress(operation_id, bytes_processed, None)
    }

    /// Update operation progress with optional total size
    /// For multi-file operations, this accumulates progress from all files
    pub fn update_operation_progress(
        &self,
        operation_id: &str,
        bytes_processed: u64,
        total_size: Option<u64>,
    ) -> Result<()> {
        {
            let mut ops = self.operations.write();
            if let Some(op) = ops.get_mut(operation_id) {
                // For multi-file operations, accumulate bytes_processed from all files
                if let Some(files) = &op.files {
                    let total_processed: u64 = files.iter().map(|f| f.bytes_processed).sum();
                    op.bytes_processed = total_processed.max(bytes_processed);
                } else {
                    op.bytes_processed = bytes_processed;
                }
                
                if op.status == OperationStatus::Pending {
                    op.status = OperationStatus::InProgress;
                }
                if let Some(total) = total_size {
                    op.file_size = Some(total);
                }
                op.last_updated_at = Some(Utc::now());
            }
        }
        
        self.save_state()?;
        Ok(())
    }

    /// Update individual file progress in a multi-file operation
    /// This automatically recalculates the total operation progress
    pub fn update_file_progress(
        &self,
        operation_id: &str,
        file_local_path: &str,
        bytes_processed: u64,
        status: Option<OperationStatus>,
    ) -> Result<()> {
        {
            let mut ops = self.operations.write();
            if let Some(op) = ops.get_mut(operation_id) {
                if let Some(files) = &mut op.files {
                    // Update the specific file
                    for file in files.iter_mut() {
                        if file.local_path == file_local_path {
                            file.bytes_processed = bytes_processed;
                            if let Some(new_status) = status {
                                file.status = Some(new_status);
                            }
                            break;
                        }
                    }
                    // Recalculate total bytes_processed from all files
                    op.bytes_processed = files.iter().map(|f| f.bytes_processed).sum();
                    
                    // Update operation status based on file statuses
                    // IMPORTANT: Only mark parent as completed when ALL children are completed AND actually at 100%
                    // Check that all files are marked as Completed AND their bytes_processed equals their file_size
                    let all_completed = files.iter().all(|f| {
                        let status_complete = matches!(f.status, Some(OperationStatus::Completed));
                        // File is truly complete only if status is Completed AND bytes_processed >= file_size
                        // Allow small tolerance (1 byte) for rounding issues
                        let bytes_complete = f.bytes_processed >= f.file_size.saturating_sub(1);
                        status_complete && bytes_complete
                    });
                    let any_failed = files.iter().any(|f| {
                        matches!(f.status, Some(OperationStatus::Failed))
                    });
                    let any_in_progress = files.iter().any(|f| {
                        matches!(f.status, Some(OperationStatus::InProgress | OperationStatus::Pending))
                    });
                    
                    // Only mark as completed if ALL files are completed AND actually at 100% AND operation is not already completed
                    if all_completed && !files.is_empty() && op.status != OperationStatus::Completed {
                        // Calculate actual total bytes_processed from files (don't force to file_size)
                        let actual_bytes_processed: u64 = files.iter().map(|f| f.bytes_processed).sum();
                        let total_file_size: u64 = files.iter().map(|f| f.file_size).sum();
                        
                        // Only mark as completed if actual progress equals total size
                        if actual_bytes_processed >= total_file_size.saturating_sub(1) {
                            op.status = OperationStatus::Completed;
                            op.completed_at = Some(Utc::now());
                            // Use actual bytes_processed, not forced file_size
                            op.bytes_processed = actual_bytes_processed;
                        } else {
                            // Files are marked Completed but not at 100% yet - keep as InProgress
                            if op.status == OperationStatus::Pending {
                                op.status = OperationStatus::InProgress;
                            }
                        }
                    } else if any_failed && op.status != OperationStatus::Failed {
                        op.status = OperationStatus::Failed;
                    } else if any_in_progress && op.status == OperationStatus::Pending {
                        op.status = OperationStatus::InProgress;
                    } else if !all_completed && op.status == OperationStatus::Completed {
                        // If not all children are complete but parent is marked complete, revert to InProgress
                        // This prevents premature completion
                        op.status = OperationStatus::InProgress;
                        op.completed_at = None;
                    }
                } else {
                    // Single file operation - update directly
                    op.bytes_processed = bytes_processed;
                    if let Some(new_status) = status {
                        let is_completed = matches!(new_status, OperationStatus::Completed);
                        op.status = new_status;
                        if is_completed {
                            op.completed_at = Some(Utc::now());
                        }
                    }
                }
                op.last_updated_at = Some(Utc::now());
            }
        }
        
        self.save_state()?;
        Ok(())
    }

    /// Update file progress with error status
    pub fn update_file_progress_with_error(
        &self,
        operation_id: &str,
        file_local_path: &str,
        error: String,
    ) -> Result<()> {
        {
            let mut ops = self.operations.write();
            if let Some(op) = ops.get_mut(operation_id) {
                if let Some(files) = &mut op.files {
                    // Update the specific file with error
                    for file in files.iter_mut() {
                        if file.local_path == file_local_path {
                            file.status = Some(OperationStatus::Failed);
                            file.error = Some(error.clone());
                            break;
                        }
                    }
                    // Recalculate total bytes_processed from all files
                    op.bytes_processed = files.iter().map(|f| f.bytes_processed).sum();
                    
                    // Update operation status based on file statuses
                    let _all_completed = files.iter().all(|f| {
                        matches!(f.status, Some(OperationStatus::Completed))
                    });
                    let any_failed = files.iter().any(|f| {
                        matches!(f.status, Some(OperationStatus::Failed))
                    });
                    let any_in_progress = files.iter().any(|f| {
                        matches!(f.status, Some(OperationStatus::InProgress | OperationStatus::Pending))
                    });
                    
                    if any_failed && op.status != OperationStatus::Failed {
                        op.status = OperationStatus::Failed;
                        op.error = Some(format!("One or more files failed: {}", error));
                    } else if any_in_progress && op.status == OperationStatus::Pending {
                        op.status = OperationStatus::InProgress;
                    }
                } else {
                    // Single file operation - mark as failed
                    op.status = OperationStatus::Failed;
                    op.error = Some(error);
                }
                op.last_updated_at = Some(Utc::now());
            }
        }
        
        self.save_state()?;
        Ok(())
    }

    /// Update operation with current system metrics
    pub fn update_with_metrics(&self, operation_id: &str) -> Result<()> {
        // Try to collect metrics - ignore errors if system module is not available
        let metrics = crate::system::get_system_metrics();
        
        let mut ops = self.operations.write();
        if let Some(op) = ops.get_mut(operation_id) {
            op.cpu_usage_percent = Some(metrics.cpu_usage);
            op.memory_usage_mb = Some(metrics.memory_used_mb);
            op.network_tx_bytes_sec = Some(metrics.network_tx_bytes_sec);
            op.network_rx_bytes_sec = Some(metrics.network_rx_bytes_sec);
            op.last_updated_at = Some(Utc::now());
        }
        Ok(())
    }

    /// Update operation metadata (tags, asset_info, metadata)
    pub fn update_operation_metadata(
        &self,
        operation_id: &str,
        tags: Option<Vec<String>>,
        asset_info: Option<serde_json::Value>,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut ops = self.operations.write();
        if let Some(op) = ops.get_mut(operation_id) {
            if let Some(tags) = tags {
                op.tags = Some(tags);
            }
            if let Some(asset_info) = asset_info {
                op.asset_info = Some(asset_info);
            }
            if let Some(metadata) = metadata {
                op.metadata = Some(metadata);
            }
            op.last_updated_at = Some(Utc::now());
        }
        self.save_state()?;
        Ok(())
    }

    /// Mark operation as completed
    /// For multi-file operations, ensures all children are completed before marking parent as complete
    pub fn complete_operation(
        &self,
        operation_id: &str,
    ) -> Result<()> {
        // Update with final metrics before completing
        let _ = self.update_with_metrics(operation_id);
        
        let operation = {
            let mut ops = self.operations.write();
            if let Some(op) = ops.get_mut(operation_id) {
                // For multi-file operations, check that all children are completed AND actually at 100%
                if let Some(ref files) = op.files {
                    let all_completed = files.iter().all(|f| {
                        let status_complete = matches!(f.status, Some(OperationStatus::Completed));
                        // File is truly complete only if status is Completed AND bytes_processed >= file_size
                        // Allow small tolerance (1 byte) for rounding issues
                        let bytes_complete = f.bytes_processed >= f.file_size.saturating_sub(1);
                        status_complete && bytes_complete
                    });
                    
                    if !all_completed {
                        // Not all children are complete - don't complete parent yet
                        // The update_file_progress method will handle completion when all children finish
                        return Ok(());
                    }
                    
                    // All children complete - use actual bytes_processed from files, not forced file_size
                    let actual_bytes_processed: u64 = files.iter().map(|f| f.bytes_processed).sum();
                    let total_file_size: u64 = files.iter().map(|f| f.file_size).sum();
                    
                    // Only complete if actual progress equals total size
                    if actual_bytes_processed >= total_file_size.saturating_sub(1) {
                        op.bytes_processed = actual_bytes_processed;
                    } else {
                        // Files are marked Completed but not at 100% yet - don't complete operation
                        return Ok(());
                    }
                }
                
                op.status = OperationStatus::Completed;
                op.completed_at = Some(Utc::now());
                op.last_updated_at = Some(Utc::now());
                
                // If file_size was not set, set it to bytes_processed
                if op.file_size.is_none() {
                    op.file_size = Some(op.bytes_processed);
                }
                op.clone()
            } else {
                return Ok(());
            }
        };
        
        // Append updated operation to audit log (both old and new systems for compatibility)
        self.append_to_audit_log(&operation);
        
        self.cleanup_old_operations();
        self.save_state()?;
        Ok(())
    }

    /// Mark operation as failed
    pub fn fail_operation(
        &self,
        operation_id: &str,
        error: String,
    ) -> Result<()> {
        let operation = {
            let mut ops = self.operations.write();
            if let Some(op) = ops.get_mut(operation_id) {
                op.status = OperationStatus::Failed;
                op.error = Some(error.clone());
                op.completed_at = Some(Utc::now());
                op.last_updated_at = Some(Utc::now());
                op.clone()
            } else {
                return Ok(());
            }
        };
        
        // Append updated operation to audit log (both old and new systems for compatibility)
        self.append_to_audit_log(&operation);
        
        self.cleanup_old_operations();
        self.save_state()?;
        Ok(())
    }

    /// Cancel operation
    pub fn cancel_operation(
        &self,
        operation_id: &str,
    ) -> Result<()> {
        let operation = {
            let mut ops = self.operations.write();
            if let Some(op) = ops.get_mut(operation_id) {
                op.status = OperationStatus::Canceled;
                op.completed_at = Some(Utc::now());
                op.last_updated_at = Some(Utc::now());
                op.clone()
            } else {
                return Ok(());
            }
        };
        
        // Append updated operation to audit log (both old and new systems for compatibility)
        self.append_to_audit_log(&operation);
        
        self.cleanup_old_operations();
        self.save_state()?;
        Ok(())
    }

    /// Get operation by ID
    pub fn get_operation(&self, operation_id: &str) -> Result<Operation> {
        let ops = self.operations.read();
        ops.get(operation_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Operation not found: {}", operation_id))
    }

    /// Get all operations
    pub fn get_all_operations(&self) -> Vec<Operation> {
        let ops = self.operations.read();
        let operations: Vec<Operation> = ops.values().cloned().collect();
        
        // #region agent log
        let copy_move_rename: Vec<_> = operations.iter()
            .filter(|op| matches!(op.operation_type, OperationType::Copy | OperationType::Move | OperationType::Rename | OperationType::CreateDir))
            .map(|op| format!(r#"{{"id":"{}","type":"{:?}","status":"{:?}"}}"#, op.operation_id, op.operation_type, op.status))
            .collect();
        tracing::debug!(target: "agent_log", r#"{{"location":"operation_tracker.rs:718","message":"get_all_operations returning","data":{{"total_ops":{},"copy_move_rename_count":{},"copy_move_rename_ops":[{}]}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"D"}}"#, 
                operations.len(), copy_move_rename.len(), copy_move_rename.join(","),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
        // #endregion
        
        operations
    }

    /// Get operations by type
    pub fn get_operations_by_type(&self, operation_type: &OperationType) -> Vec<Operation> {
        let ops = self.operations.read();
        ops.values()
            .filter(|op| op.operation_type == *operation_type)
            .cloned()
            .collect()
    }

    /// Get active operations
    pub fn get_active_operations(&self) -> Vec<Operation> {
        let ops = self.operations.read();
        ops.values()
            .filter(|op| {
                op.status == OperationStatus::Pending || op.status == OperationStatus::InProgress
            })
            .cloned()
            .collect()
    }

    /// Get completed operations (limited by max_history)
    pub fn get_completed_operations(&self) -> Vec<Operation> {
        let ops = self.operations.read();
        let mut completed: Vec<Operation> = ops.values()
            .filter(|op| {
                op.status == OperationStatus::Completed || op.status == OperationStatus::Failed
            })
            .cloned()
            .collect();
        
        // Sort by completed_at (most recent first)
        completed.sort_by(|a, b| {
            let a_time = a.completed_at.or(a.last_updated_at).or(a.created_at);
            let b_time = b.completed_at.or(b.last_updated_at).or(b.created_at);
            b_time.cmp(&a_time)
        });
        
        // Limit to max_history
        completed.truncate(self.max_history);
        completed
    }

    /// Get operation history from audit log (all operations, not limited)
    pub fn get_audit_history(&self, limit: Option<usize>) -> Result<Vec<Operation>> {
        if !self.audit_file.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.audit_file)
            .context("Failed to read audit log file")?;
        
        let mut operations: Vec<Operation> = Vec::new();
        
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<Operation>(line) {
                Ok(op) => operations.push(op),
                Err(e) => {
                    error!("Failed to parse audit log line: {} - {}", line, e);
                }
            }
        }
        
        // Sort by created_at (most recent first)
        operations.sort_by(|a, b| {
            let a_time = a.created_at.or(a.last_updated_at);
            let b_time = b.created_at.or(b.last_updated_at);
            b_time.cmp(&a_time)
        });
        
        // Apply limit if specified
        if let Some(limit) = limit {
            operations.truncate(limit);
        }
        
        Ok(operations)
    }

    /// Get organization audit history (filtered by organization_id)
    pub fn get_organization_audit(&self, organization_id: &str, limit: Option<usize>) -> Result<Vec<Operation>> {
        let mut operations = self.get_audit_history(limit)?;
        
        // Filter by organization_id
        operations.retain(|op| {
            op.organization_id.as_ref().map(|id| id == organization_id).unwrap_or(false)
        });
        
        Ok(operations)
    }

    /// Get operations by user ID (for user audit)
    pub fn get_operations_by_user(&self, user_id: &str) -> Vec<Operation> {
        let ops = self.operations.read();
        let mut user_ops: Vec<Operation> = ops.values()
            .filter(|op| op.user_id.as_ref().map(|id| id == user_id).unwrap_or(false))
            .cloned()
            .collect();
        
        // Sort by created_at (most recent first)
        user_ops.sort_by(|a, b| {
            let a_time = a.created_at.or(a.last_updated_at);
            let b_time = b.created_at.or(b.last_updated_at);
            b_time.cmp(&a_time)
        });
        
        user_ops
    }

    /// Get operations by organization ID (for organization audit)
    pub fn get_operations_by_organization(&self, organization_id: &str) -> Vec<Operation> {
        let ops = self.operations.read();
        let mut org_ops: Vec<Operation> = ops.values()
            .filter(|op| op.organization_id.as_ref().map(|id| id == organization_id).unwrap_or(false))
            .cloned()
            .collect();
        
        // Sort by created_at (most recent first)
        org_ops.sort_by(|a, b| {
            let a_time = a.created_at.or(a.last_updated_at);
            let b_time = b.created_at.or(b.last_updated_at);
            b_time.cmp(&a_time)
        });
        
        org_ops
    }

    /// Delete an operation from history (removes from in-memory cache only, audit log remains)
    pub fn delete_operation(&self, operation_id: &str) -> Result<()> {
        let mut ops = self.operations.write();
        ops.remove(operation_id);
        self.save_state()
            .context("Failed to save state after deleting operation")?;
        info!("Deleted operation: {}", operation_id);
        Ok(())
    }

    /// Cleanup old completed operations beyond max_history
    fn cleanup_old_operations(&self) {
        let mut ops = self.operations.write();
        let ops_before = ops.len();

        let mut completed: Vec<(String, chrono::DateTime<chrono::Utc>)> = ops.iter()
            .filter_map(|(id, op)| {
                if op.status == OperationStatus::Completed || op.status == OperationStatus::Failed {
                    op.completed_at.or(op.last_updated_at).or(op.created_at)
                        .map(|time| (id.clone(), time))
                } else {
                    None
                }
            })
            .collect();

        completed.sort_by(|a, b| b.1.cmp(&a.1));

        // Remove operations beyond max_history
        let removed_count = if completed.len() > self.max_history {
            let to_remove = completed.len() - self.max_history;
            let removed_ids: Vec<String> = completed.iter().skip(self.max_history).map(|(id, _)| id.clone()).collect();
            for (id, _) in completed.iter().skip(self.max_history) {
                ops.remove(id);
            }
            tracing::debug!(target: "agent_log", r#"{{"location":"operation_tracker.rs:870","message":"Removed operations during cleanup","data":{{"removed_ids":{:?},"removed_count":{}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"D"}}"#, 
                    removed_ids, to_remove,
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
            to_remove
        } else {
            0
        };
        
        tracing::debug!(target: "agent_log", r#"{{"location":"operation_tracker.rs:878","message":"cleanup_old_operations","data":{{"ops_before":{},"ops_after":{},"completed_count":{},"removed_count":{},"max_history":{}}},"timestamp":{},"sessionId":"debug-session","runId":"run1","hypothesisId":"D"}}"#, 
                ops_before, ops.len(), completed.len(), removed_count, self.max_history,
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    }
}
