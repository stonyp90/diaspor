//! Centralized Logging System
//!
//! Provides persistent file-based logging with rotation and different log levels.
//! Logs are written to disk and can be accessed via Tauri commands for troubleshooting.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use parking_lot::Mutex;
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
    Registry,
};

/// Log entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: Option<serde_json::Value>,
}

/// File-based log writer with rotation
pub struct FileLogWriter {
    log_dir: PathBuf,
    current_file: Arc<Mutex<Option<File>>>,
    max_file_size: u64,
    max_files: usize,
}

impl FileLogWriter {
    pub fn new(log_dir: &Path, max_file_size: u64, max_files: usize) -> Result<Self> {
        // Normalize path separators for Windows
        let normalized_dir = log_dir.to_path_buf();
        
        // Create directory with better error handling for Windows
        std::fs::create_dir_all(&normalized_dir)
            .with_context(|| format!(
                "Failed to create log directory: {:?} (platform: {})", 
                normalized_dir,
                std::env::consts::OS
            ))?;

        // Verify directory was created and is writable
        let metadata = std::fs::metadata(&normalized_dir)
            .with_context(|| format!("Failed to access log directory: {:?}", normalized_dir))?;
        
        if !metadata.is_dir() {
            return Err(anyhow::anyhow!("Log path exists but is not a directory: {:?}", normalized_dir));
        }

        Ok(Self {
            log_dir: normalized_dir,
            current_file: Arc::new(Mutex::new(None)),
            max_file_size,
            max_files,
        })
    }

    fn get_log_file_path(&self, index: usize) -> PathBuf {
        if index == 0 {
            self.log_dir.join("diaspor.log")
        } else {
            self.log_dir.join(format!("diaspor.{}.log", index))
        }
    }

    fn rotate_if_needed(&self) -> Result<()> {
        // Check current file size
        let log_file = self.get_log_file_path(0);
        if log_file.exists() {
            let metadata = std::fs::metadata(&log_file)?;
            if metadata.len() >= self.max_file_size {
                // Rotate files
                for i in (0..self.max_files).rev() {
                    let src = self.get_log_file_path(i);
                    let dst = self.get_log_file_path(i + 1);
                    
                    if src.exists() {
                        if dst.exists() {
                            std::fs::remove_file(&dst)?;
                        }
                        std::fs::rename(&src, &dst)?;
                    }
                }
                
                // Close current file handle
                let mut current = self.current_file.lock();
                *current = None;
            }
        }
        Ok(())
    }

    fn get_or_create_file(&self) -> Result<File> {
        self.rotate_if_needed()?;
        
        let mut current = self.current_file.lock();
        if let Some(ref file) = *current {
            // Try to use existing file (check if it's still valid)
            match file.try_clone() {
                Ok(cloned) => return Ok(cloned),
                Err(e) => {
                    // File handle is invalid, clear it and create a new one
                    tracing::warn!("Log file handle invalid, recreating: {}", e);
                    *current = None;
                }
            }
        }
        
        // Ensure log directory still exists (might have been deleted)
        if !self.log_dir.exists() {
            std::fs::create_dir_all(&self.log_dir)
                .with_context(|| format!("Failed to recreate log directory: {:?}", self.log_dir))?;
        }
        
        let log_file = self.get_log_file_path(0);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .with_context(|| format!(
                "Failed to open log file: {:?} (platform: {})", 
                log_file,
                std::env::consts::OS
            ))?;
        
        *current = Some(file.try_clone()?);
        Ok(file)
    }

    pub fn write_log(&self, level: Level, target: &str, message: &str, fields: Option<serde_json::Value>) -> Result<()> {
        let timestamp = Utc::now();
        let level_str = match level {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN",
            Level::INFO => "INFO",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "TRACE",
        };

        let entry = LogEntry {
            timestamp,
            level: level_str.to_string(),
            target: target.to_string(),
            message: message.to_string(),
            fields,
        };

        let json = serde_json::to_string(&entry)?;
        let mut file = self.get_or_create_file()?;
        writeln!(file, "{}", json)?;
        file.flush()?;

        Ok(())
    }

    pub fn current_log_file_path(&self) -> PathBuf {
        self.get_log_file_path(0)
    }
}

/// Custom writer for tracing-subscriber
pub struct LogWriter {
    writer: Arc<FileLogWriter>,
    level: Level,
}

impl LogWriter {
    pub fn new(writer: Arc<FileLogWriter>, level: Level) -> Self {
        Self { writer, level }
    }
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let message = String::from_utf8_lossy(buf);
        let trimmed = message.trim();
        if !trimmed.is_empty() {
            let _ = self.writer.write_log(
                self.level,
                "tracing",
                trimmed,
                None,
            );
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogWriter {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            writer: Arc::clone(&self.writer),
            level: self.level,
        }
    }
}

/// Initialize file-based logging with configurable settings
pub fn init_file_logging_with_settings(
    log_dir: &Path,
    max_file_size: u64,
    max_rotated_files: usize,
) -> Result<Arc<FileLogWriter>> {
    let writer = Arc::new(FileLogWriter::new(
        log_dir,
        max_file_size,
        max_rotated_files,
    )?);

    // Create layers for different log levels
    let file_layer = fmt::layer()
        .with_writer(LogWriter::new(Arc::clone(&writer), Level::DEBUG))
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false);

    // Also log to stdout/stderr for development
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    let subscriber = Registry::default()
        .with(file_layer)
        .with(stdout_layer);

    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set global tracing subscriber")?;

    Ok(writer)
}

/// Initialize file-based logging (convenience function with defaults)
pub fn init_file_logging(log_dir: &Path) -> Result<Arc<FileLogWriter>> {
    init_file_logging_with_settings(
        log_dir,
        10 * 1024 * 1024, // 10MB per file
        5,                 // Keep 5 rotated files
    )
}

/// Read logs from file
pub fn read_logs(log_dir: &Path, limit: Option<usize>, level_filter: Option<&str>) -> Result<Vec<LogEntry>> {
    let mut entries = Vec::new();
    
    // Read from all log files (current + rotated)
    for i in 0..6 {
        let log_file = if i == 0 {
            log_dir.join("diaspor.log")
        } else {
            log_dir.join(format!("diaspor.{}.log", i))
        };
        
        if !log_file.exists() {
            continue;
        }
        
        let content = std::fs::read_to_string(&log_file)?;
        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                // Apply level filter if specified
                if let Some(filter_level) = level_filter {
                    if entry.level != filter_level {
                        continue;
                    }
                }
                entries.push(entry);
            }
        }
    }
    
    // Sort by timestamp (newest first)
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    // Apply limit
    if let Some(limit) = limit {
        entries.truncate(limit);
    }
    
    Ok(entries)
}

/// Clear old logs
pub fn clear_logs(log_dir: &Path) -> Result<()> {
    for i in 0..6 {
        let log_file = if i == 0 {
            log_dir.join("diaspor.log")
        } else {
            log_dir.join(format!("diaspor.{}.log", i))
        };
        
        if log_file.exists() {
            std::fs::remove_file(&log_file)?;
        }
    }
    
    Ok(())
}
