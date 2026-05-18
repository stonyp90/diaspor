//! VFS State Management
//!
//! Manages global VFS service state for Tauri commands

use std::sync::Arc;
use parking_lot::RwLock as ParkingLotRwLock;
use crate::vfs::application::VfsService;

/// Global VFS state wrapped for Tauri
pub struct VfsStateWrapper(pub Arc<ParkingLotRwLock<Option<Arc<VfsService>>>>);

impl VfsStateWrapper {
    pub fn new() -> Self {
        Self(Arc::new(ParkingLotRwLock::new(None)))
    }
    
    /// Get a clone of the service if initialized
    pub fn get_service(&self) -> Option<Arc<VfsService>> {
        self.0.read().clone()
    }
    
    /// Set the service
    pub fn set_service(&self, service: Arc<VfsService>) {
        *self.0.write() = Some(service);
    }
}

impl Default for VfsStateWrapper {
    fn default() -> Self {
        Self::new()
    }
}
