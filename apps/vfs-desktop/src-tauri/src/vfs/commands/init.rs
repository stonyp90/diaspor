//! Initialization Commands
//!
//! Commands for initializing the VFS service and mounting local storage

use std::sync::Arc;
use tauri::State;
use tracing::{info, warn};
use crate::vfs::application::VfsService;
use super::state::VfsStateWrapper;
use super::clipboard::init_global_clipboard;

/// Initialize the VFS service and auto-mount default system folders
#[tauri::command]
pub async fn vfs_init(
    state: State<'_, VfsStateWrapper>,
) -> Result<String, String> {
    // Check if already initialized
    if state.get_service().is_some() {
        info!("VFS already initialized, skipping");
        return Ok("VFS already initialized".to_string());
    }
    
    let mut service = VfsService::new()
        .await
        .map_err(|e| format!("Failed to initialize VFS: {}", e))?;
    
    // Initialize settings manager for storing provider credentials
    if let Err(e) = service.init_settings_manager() {
        warn!("Failed to initialize settings manager: {}", e);
        // Continue anyway - settings are optional
    }
    
    // Auto-mount default system folders
    let home = dirs::home_dir();
    
    if let Some(home_path) = home {
        // Mount home directory
        if let Err(e) = service.add_local_source("Home".to_string(), home_path.clone()).await {
            warn!("Failed to mount Home: {}", e);
        }
        
        // Mount common folders if they exist
        let common_folders = [
            ("Desktop", home_path.join("Desktop")),
            ("Documents", home_path.join("Documents")),
            ("Downloads", home_path.join("Downloads")),
            ("Pictures", home_path.join("Pictures")),
            ("Music", home_path.join("Music")),
            ("Videos", home_path.join("Videos")),
        ];
        
        for (name, path) in common_folders {
            if path.exists() && path.is_dir() {
                if let Err(e) = service.add_local_source(name.to_string(), path).await {
                    warn!("Failed to mount {}: {}", name, e);
                }
            }
        }
        
        // Platform-specific mounts - enumerate external volumes
        #[cfg(target_os = "macos")]
        {
            // Enumerate each volume in /Volumes separately (like Windows drive letters)
            let volumes_dir = std::path::PathBuf::from("/Volumes");
            if volumes_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&volumes_dir) {
                    for entry in entries.filter_map(Result::ok) {
                        let vol_path = entry.path();
                        let vol_name = entry.file_name().to_string_lossy().to_string();
                        
                        // Skip the main Macintosh HD symlink (already have Home folder)
                        // Only include actual mounted external volumes
                        if vol_path.is_dir() && !vol_name.starts_with('.') {
                            // Check if it's a symlink to root (main HD)
                            if let Ok(target) = std::fs::read_link(&vol_path) {
                                if target == std::path::Path::new("/") {
                                    continue; // Skip symlink to root
                                }
                            }
                            
                            let display_name = vol_name.to_string();
                            if let Err(e) = service.add_local_source(display_name.clone(), vol_path).await {
                                warn!("Failed to mount volume {}: {}", display_name, e);
                            }
                        }
                    }
                }
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            // Enumerate user's media mounts (USB drives, etc.)
            if let Some(username) = std::env::var("USER").ok() {
                let media_dir = std::path::PathBuf::from(format!("/media/{}", username));
                if media_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&media_dir) {
                        for entry in entries.filter_map(Result::ok) {
                            let mount_path = entry.path();
                            let mount_name = entry.file_name().to_string_lossy().to_string();
                            
                            if mount_path.is_dir() && !mount_name.starts_with('.') {
                                if let Err(e) = service.add_local_source(mount_name.clone(), mount_path).await {
                                    warn!("Failed to mount media {}: {}", mount_name, e);
                                }
                            }
                        }
                    }
                }
            }
            
            // Also check /mnt for manually mounted drives
            let mnt_dir = std::path::PathBuf::from("/mnt");
            if mnt_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&mnt_dir) {
                    for entry in entries.filter_map(Result::ok) {
                        let mount_path = entry.path();
                        let mount_name = entry.file_name().to_string_lossy().to_string();
                        
                        if mount_path.is_dir() && !mount_name.starts_with('.') {
                            if let Err(e) = service.add_local_source(mount_name.clone(), mount_path).await {
                                warn!("Failed to mount {}: {}", mount_name, e);
                            }
                        }
                    }
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            // Enumerate all available drive letters (A-Z)
            for drive in 'A'..='Z' {
                let drive_path = std::path::PathBuf::from(format!("{}:\\", drive));
                if drive_path.exists() {
                    // Get volume label if available, otherwise use drive letter
                    let name = format!("Drive ({}:)", drive);
                    if let Err(e) = service.add_local_source(name.clone(), drive_path).await {
                        // Only warn for drives that should be accessible
                        if drive >= 'C' {
                            warn!("Failed to mount {}: {}", name, e);
                        }
                    }
                }
            }
        }
    }
    
    // Load persisted storage sources (S3, GCS, etc.)
    // Note: Cloud drives (iCloud, Google Drive, Dropbox, etc.) are already accessible
    // via their local mount points as regular folders - no special detection needed
    if let Err(e) = service.load_sources().await {
        warn!("Failed to load persisted storage sources: {}", e);
        // Continue anyway - don't block initialization
    }
    
    let service_arc = Arc::new(service);
    
    // Initialize global clipboard with VFS service
    init_global_clipboard(service_arc.clone());
    
    state.set_service(service_arc);
    
    info!("VFS service initialized with default folders");
    Ok("VFS initialized successfully".to_string())
}
