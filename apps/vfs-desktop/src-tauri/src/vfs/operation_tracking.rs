//! Operation Tracking Helper
//!
//! Provides utilities for tracking file operations with full metadata.
//! This ensures all operations are logged to the audit log with complete information.

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error};

use crate::vfs::operation_tracker::{OperationType, OperationStatus};
use crate::vfs::ports::file_operations::IFileOperations;
use crate::vfs::audit_log::AuditLog;
use std::sync::OnceLock;
use std::path::PathBuf;

// Use the shared operation tracker from commands::helpers to avoid separate instances
use crate::vfs::commands::helpers::get_operation_tracker;

fn get_audit_log() -> &'static AuditLog {
    static AUDIT_LOG: OnceLock<AuditLog> = OnceLock::new();
    AUDIT_LOG.get_or_init(|| {
        let audit_dir = dirs::data_dir()
            .unwrap_or_else(|| dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp")))
            .join("ursly")
            .join("vfs")
            .join("audit");
        AuditLog::new(&audit_dir, 0)
            .expect("Failed to initialize audit log")
    })
}

fn get_current_user_id() -> Option<String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
}

fn get_current_organization_id() -> Option<String> {
    None
}

/// Track a file operation with full metadata
pub struct OperationTrackingHelper;

impl OperationTrackingHelper {
    /// Track a file operation before execution
    pub fn track_operation_start(
        operation_type: OperationType,
        source_id: String,
        source_path: String,
        destination_path: Option<String>,
        file_size: Option<u64>,
    ) -> String {
        let tracker = get_operation_tracker();
        let user_id = get_current_user_id();
        let org_id = get_current_organization_id();
        
        // Clone operation_type for debug logging
        let operation_type_debug = format!("{:?}", operation_type);
        
        let operation_id = tracker.create_multi_file_operation_with_context(
            operation_type,
            source_id,
            source_path,
            destination_path,
            file_size,
            None, // files - will be populated if multi-file
            user_id,
            org_id,
        );
        
        debug!("Started tracking operation: {} ({})", operation_id, operation_type_debug);
        operation_id
    }
    
    /// Track a file operation with file metadata
    pub async fn track_operation_with_metadata<F, T>(
        operation_type: OperationType,
        source_id: String,
        source_path: String,
        destination_path: Option<String>,
        file_ops: Option<Arc<dyn IFileOperations>>,
        operation: F,
    ) -> Result<(String, T)>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        // Get file metadata before operation
        let file_size = if let Some(ref ops) = file_ops {
            ops.file_size(Path::new(&source_path)).await.ok()
        } else {
            None
        };
        
        // Start tracking
        let operation_id = Self::track_operation_start(
            operation_type.clone(),
            source_id.clone(),
            source_path.clone(),
            destination_path.clone(),
            file_size,
        );
        
        let tracker = get_operation_tracker();
        
        // Execute operation
        let result = operation.await;
        
        // Update operation status based on result
        match result {
            Ok(output) => {
                // Get final file size if destination exists
                let final_size = if let Some(ref ops) = file_ops {
                    if let Some(ref dest) = destination_path {
                        ops.file_size(Path::new(dest)).await.ok()
                    } else {
                        file_size
                    }
                } else {
                    file_size
                };
                
                // Update with final metadata
                if let Some(size) = final_size {
                    let _ = tracker.update_operation_progress(&operation_id, size, Some(size));
                }
                
                // Mark as completed
                if let Err(e) = tracker.complete_operation(&operation_id) {
                    error!("Failed to complete operation {}: {}", operation_id, e);
                }
                
                // Also log to audit log
                if let Ok(op) = tracker.get_operation(&operation_id) {
                    let audit_log = get_audit_log();
                    if let Err(e) = audit_log.log_operation(op) {
                        error!("Failed to log operation to audit log: {}", e);
                    }
                }
                
                Ok((operation_id, output))
            }
            Err(e) => {
                let error_msg = format!("{}", e);
                if let Err(track_err) = tracker.fail_operation(&operation_id, error_msg.clone()) {
                    error!("Failed to fail operation {}: {}", operation_id, track_err);
                }
                
                // Also log failed operation to audit log
                if let Ok(op) = tracker.get_operation(&operation_id) {
                    let audit_log = get_audit_log();
                    if let Err(e) = audit_log.log_operation(op) {
                        error!("Failed to log failed operation to audit log: {}", e);
                    }
                }
                
                Err(e)
            }
        }
    }
    
    /// Track a multi-file operation (like paste with multiple files)
    pub fn track_multi_file_operation_start(
        operation_type: OperationType,
        source_id: String,
        source_path: String,
        destination_path: Option<String>,
        files: Vec<(String, u64)>, // (path, size)
    ) -> String {
        let tracker = get_operation_tracker();
        let user_id = get_current_user_id();
        let org_id = get_current_organization_id();
        
        use crate::vfs::operation_tracker::OperationFile;
        
        let operation_files: Vec<OperationFile> = files.iter().map(|(path, size)| {
            OperationFile {
                local_path: path.clone(),
                remote_path: destination_path.clone().unwrap_or_default(),
                file_size: *size,
                bytes_processed: 0,
                status: Some(OperationStatus::Pending),
                error: None,
            }
        }).collect();
        
        let total_size: u64 = files.iter().map(|(_, size)| size).sum();
        
        let operation_id = tracker.create_multi_file_operation_with_context(
            operation_type,
            source_id,
            source_path,
            destination_path,
            Some(total_size),
            Some(operation_files),
            user_id,
            org_id,
        );
        
        debug!("Started tracking multi-file operation: {} ({} files)", operation_id, files.len());
        operation_id
    }
    
    /// Track a directory operation (mkdir, rmdir)
    pub async fn track_directory_operation<F, T>(
        operation_type: OperationType,
        source_id: String,
        path: String,
        file_ops: Option<Arc<dyn IFileOperations>>,
        operation: F,
    ) -> Result<(String, T)>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        Self::track_operation_with_metadata(
            operation_type,
            source_id,
            path.clone(),
            None,
            file_ops,
            operation,
        ).await
    }
    
    /// Track a rename/move operation
    pub async fn track_rename_operation<F, T>(
        source_id: String,
        old_path: String,
        new_path: String,
        file_ops: Option<Arc<dyn IFileOperations>>,
        operation: F,
    ) -> Result<(String, T)>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        Self::track_operation_with_metadata(
            OperationType::Rename,
            source_id,
            old_path,
            Some(new_path),
            file_ops,
            operation,
        ).await
    }
    
    /// Track a copy operation
    pub async fn track_copy_operation<F, T>(
        source_id: String,
        from_path: String,
        to_path: String,
        file_ops: Option<Arc<dyn IFileOperations>>,
        operation: F,
    ) -> Result<(String, T)>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        Self::track_operation_with_metadata(
            OperationType::Copy,
            source_id,
            from_path,
            Some(to_path),
            file_ops,
            operation,
        ).await
    }
    
    /// Track a move operation
    pub async fn track_move_operation<F, T>(
        source_id: String,
        from_path: String,
        to_path: String,
        file_ops: Option<Arc<dyn IFileOperations>>,
        operation: F,
    ) -> Result<(String, T)>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        Self::track_operation_with_metadata(
            OperationType::Move,
            source_id,
            from_path,
            Some(to_path),
            file_ops,
            operation,
        ).await
    }
    
    /// Track a delete operation
    pub async fn track_delete_operation<F, T>(
        source_id: String,
        path: String,
        file_ops: Option<Arc<dyn IFileOperations>>,
        operation: F,
    ) -> Result<(String, T)>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        Self::track_operation_with_metadata(
            OperationType::Delete,
            source_id,
            path,
            None,
            file_ops,
            operation,
        ).await
    }
    
    /// Track a paste operation with multiple files
    pub fn track_paste_operation_start(
        _source_id: String,
        dest_source_id: String,
        source_path: String,
        dest_path: String,
        files: Vec<(String, u64)>,
    ) -> String {
        Self::track_multi_file_operation_start(
            OperationType::Paste,
            dest_source_id,
            dest_path,
            Some(source_path),
            files,
        )
    }
    
    /// Update operation progress
    pub fn update_progress(operation_id: &str, bytes_processed: u64) -> Result<()> {
        let tracker = get_operation_tracker();
        tracker.update_progress(operation_id, bytes_processed)
    }
    
    /// Update operation with system metrics
    pub fn update_with_metrics(operation_id: &str) -> Result<()> {
        let tracker = get_operation_tracker();
        tracker.update_with_metrics(operation_id)
    }
    
    /// Complete an operation manually
    pub fn complete_operation(operation_id: &str) -> Result<()> {
        let tracker = get_operation_tracker();
        tracker.complete_operation(operation_id)?;
        
        // Also log to audit log
        if let Ok(op) = tracker.get_operation(operation_id) {
            let audit_log = get_audit_log();
            audit_log.log_operation(op)?;
        }
        
        Ok(())
    }
    
    /// Fail an operation manually
    pub fn fail_operation(operation_id: &str, error: String) -> Result<()> {
        let tracker = get_operation_tracker();
        tracker.fail_operation(operation_id, error.clone())?;
        
        // Also log to audit log
        if let Ok(op) = tracker.get_operation(operation_id) {
            let audit_log = get_audit_log();
            audit_log.log_operation(op)?;
        }
        
        Ok(())
    }
}
