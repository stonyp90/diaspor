//! Media Service State Management for Tauri
//!
//! Manages the global media service instance for transcoding and thumbnail generation

use std::sync::Arc;
use parking_lot::RwLock as ParkingLotRwLock;
use anyhow::Result;
use std::path::PathBuf;

use crate::vfs::ports::IMediaService;
use crate::vfs::adapters::FfmpegMediaAdapter;

/// Global media service state wrapped for Tauri
#[derive(Clone)]
pub struct MediaStateWrapper(pub Arc<ParkingLotRwLock<Option<Arc<dyn IMediaService>>>>);

impl MediaStateWrapper {
    pub fn new() -> Self {
        Self(Arc::new(ParkingLotRwLock::new(None)))
    }
    
    /// Initialize the media service with output directory
    pub async fn init(&self, output_dir: PathBuf) -> Result<()> {
        if self.0.read().is_some() {
            return Ok(());
        }
        
        // Ensure output directory exists
        tokio::fs::create_dir_all(&output_dir).await?;
        
        let adapter = FfmpegMediaAdapter::new(output_dir).await?;
        let service: Arc<dyn IMediaService> = Arc::new(adapter);
        
        *self.0.write() = Some(service);
        
        Ok(())
    }
    
    /// Get or initialize the media service
    /// Initializes lazily on first access
    pub async fn get_or_init_service(&self) -> Result<Arc<dyn IMediaService>> {
        // Check if already initialized
        if let Some(service) = self.0.read().clone() {
            return Ok(service);
        }
        
        // Initialize with default output directory
        let output_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("ursly")
            .join("transcodes");
        
        self.init(output_dir).await?;
        
        self.0.read()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Failed to initialize media service"))
    }
    
    /// Get a clone of the service if initialized
    pub fn get_service(&self) -> Option<Arc<dyn IMediaService>> {
        self.0.read().clone()
    }
    
    /// Set the service
    pub fn set_service(&self, service: Arc<dyn IMediaService>) {
        *self.0.write() = Some(service);
    }
    
    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.0.read().is_some()
    }
}

impl Default for MediaStateWrapper {
    fn default() -> Self {
        Self::new()
    }
}
