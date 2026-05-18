//! Local Storage Adapter - Implements StorageAdapter, IFileOperations, and MountedStorage
//!
//! Provides direct access to the local filesystem using native OS APIs.
//! This adapter reads files exactly as they appear in Finder/Explorer.
//!
//! **Cloud Drive Support**: Cloud drives (iCloud Drive, Google Drive, OneDrive, etc.)
//! that are mounted locally use this adapter, ensuring they appear exactly as they
//! do in the native file system. All metadata (dates, sizes, permissions) matches
//! exactly what users see in Finder/Explorer - no abstraction layer.

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tracing::{debug, info, warn};
use filetime::{FileTime, set_file_times};

use crate::vfs::domain::{VirtualFile, StorageSourceType};
use crate::vfs::ports::StorageAdapter;
use crate::vfs::ports::{IFileOperations, FileEntry, FileStat, CopyOptions, MoveOptions};
use crate::vfs::ports::mounted_storage::{MountedStorage, FilesystemStats};

/// Local filesystem storage adapter
pub struct LocalStorageAdapter {
    /// Base path for this adapter
    base_path: PathBuf,
    
    /// Display name
    name: String,
}

impl LocalStorageAdapter {
    pub fn new(base_path: PathBuf, name: String) -> Self {
        Self { base_path, name }
    }
    
    /// Resolve a VFS path to an actual filesystem path
    /// Handles cross-platform path separators (Unix / and Windows \)
    fn resolve_path(&self, path: &Path) -> PathBuf {
        // If already an absolute path starting with our base, use it directly
        if path.is_absolute() && path.starts_with(&self.base_path) {
            return path.to_path_buf();
        }
        
        // Convert path to string for normalization
        let path_str = path.to_string_lossy();
        
        // Normalize: strip leading slashes (Unix) or backslashes (Windows)
        let normalized = path_str
            .trim_start_matches('/')
            .trim_start_matches('\\');
        
        // Handle empty path (root of source)
        if normalized.is_empty() {
            return self.base_path.clone();
        }
        
        // Join with base path - PathBuf::join handles platform separators
        self.base_path.join(normalized)
    }
    
    /// Calculate the total size of a directory recursively
    /// Returns 0 if calculation fails (e.g., permission denied, too large)
    fn calculate_dir_size(&self, dir_path: &PathBuf) -> u64 {
        use walkdir::WalkDir;
        
        let mut total_size = 0u64;
        
        // Walk through directory recursively
        let walker = WalkDir::new(dir_path)
            .follow_links(false) // Don't follow symlinks to avoid infinite loops
            .max_depth(100); // Limit depth to prevent stack overflow on very deep directories
        
        for entry in walker {
            match entry {
                Ok(entry) => {
                    // Only count files, not directories
                    if entry.file_type().is_file() {
                        if let Ok(metadata) = entry.metadata() {
                            total_size += metadata.len();
                        }
                    }
                }
                Err(e) => {
                    // Log but continue - some files might be inaccessible
                    debug!("Failed to access entry in {:?}: {}", dir_path, e);
                    // Don't fail completely, just skip inaccessible files
                }
            }
        }
        
        total_size
    }
    
    /// Recursively copy a directory
    async fn copy_dir_recursive(&self, from: &PathBuf, to: &PathBuf, options: &CopyOptions) -> Result<()> {
        use walkdir::WalkDir;
        
        // Check if destination exists
        if to.exists() {
            if !options.overwrite {
                return Err(anyhow::anyhow!("Destination directory already exists: {:?}", to));
            }
            // If overwrite is enabled, we'll merge into the existing directory
        } else {
            // Create destination directory
            fs::create_dir_all(to).await
                .with_context(|| format!("Failed to create directory: {:?}", to))?;
        }
        
        // Walk through source directory - use sync WalkDir since we're doing async I/O per entry
        let entries: Vec<_> = WalkDir::new(from)
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();
        
        for entry in entries {
            let source_entry = entry.path();
            
            // Calculate relative path from source root
            let relative_path = source_entry.strip_prefix(from)
                .with_context(|| format!("Failed to strip prefix from {:?}", source_entry))?;
            
            // Skip the root directory itself
            if relative_path.as_os_str().is_empty() {
                continue;
            }
            
            let dest_entry = to.join(relative_path);
            
            if source_entry.is_dir() {
                // Create directory at destination
                if !dest_entry.exists() {
                    fs::create_dir_all(&dest_entry).await
                        .with_context(|| format!("Failed to create directory: {:?}", dest_entry))?;
                }
            } else {
                // Copy file
                // Ensure parent directory exists
                if let Some(parent) = dest_entry.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent).await?;
                    }
                }
                
                // Check if destination file exists
                if dest_entry.exists() && !options.overwrite {
                    continue; // Skip existing files if overwrite is disabled
                }
                
                fs::copy(&source_entry, &dest_entry).await
                    .with_context(|| format!("Failed to copy file {:?} to {:?}", source_entry, dest_entry))?;
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl StorageAdapter for LocalStorageAdapter {
    fn storage_type(&self) -> StorageSourceType {
        StorageSourceType::Local
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn test_connection(&self) -> Result<bool> {
        Ok(self.base_path.exists() && self.base_path.is_dir())
    }
    
    async fn list_files(&self, path: &Path) -> Result<Vec<VirtualFile>> {
        let full_path = self.resolve_path(path);
        debug!("Listing files at: {:?}", full_path);
        
        // Check if path exists and is a directory
        let metadata = match fs::metadata(&full_path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Directory doesn't exist yet (might be newly created) - return empty list
                warn!("Directory does not exist yet: {:?}, returning empty list", full_path);
                return Ok(Vec::new());
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to get metadata for {:?}: {}", full_path, e))
                    .with_context(|| format!("Failed to read directory: {:?}", full_path));
            }
        };
        
        if !metadata.is_dir() {
            return Err(anyhow::anyhow!("Path is not a directory: {:?}", full_path));
        }
        
        let mut files = Vec::new();
        let mut entries = match fs::read_dir(&full_path).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Directory was deleted between metadata check and read_dir - return empty list
                warn!("Directory no longer exists: {:?}, returning empty list", full_path);
                return Ok(Vec::new());
            }
            Err(_e) => {
                return Err(anyhow::anyhow!("Failed to read directory: {:?}", full_path))
                    .with_context(|| format!("Failed to read directory: {:?}", full_path));
            }
        };
        
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let file_name = entry_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            let metadata = match entry.metadata().await {
                Ok(m) => m,
                Err(e) => {
                    warn!("Failed to get metadata for {:?}: {}", entry_path, e);
                    continue;
                }
            };
            
            let is_dir = metadata.is_dir();
            let size = if is_dir { 0 } else { metadata.len() };
            
            // Get all native filesystem metadata exactly as it appears in Finder/Explorer
            let last_modified = metadata.modified()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let last_accessed = metadata.accessed().ok();
            let _created = metadata.created().ok();
            
            // Construct VFS path correctly - handle root path ("/") properly
            let vfs_path = if path == Path::new("/") || path.as_os_str().is_empty() {
                // At root, just use the file name as the path
                PathBuf::from("/").join(&file_name)
            } else {
                // In subdirectory, join path with file name
                path.join(&file_name)
            };
            
            let mut virtual_file = VirtualFile::new(
                file_name.clone(),
                vfs_path,
                size,
                is_dir,
            );
            
            // Set all native filesystem timestamps exactly as they appear in Finder
            virtual_file.last_modified = last_modified;
            virtual_file.last_accessed = last_accessed;
            
            // Note: VirtualFile doesn't have a created field yet, but we preserve
            // all metadata that's available to ensure exact match with native FS
            
            files.push(virtual_file);
        }
        
        // Sort: directories first, then files, both alphabetically
        files.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });
        
        debug!("Listed {} files from {:?} (resolved from {:?})", files.len(), full_path, path);
        
        Ok(files)
    }
    
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        let full_path = self.resolve_path(path);
        debug!("Reading file: {:?}", full_path);
        
        fs::read(&full_path).await
            .with_context(|| format!("Failed to read file: {:?}", full_path))
    }
    
    async fn read_file_range(&self, path: &Path, offset: u64, length: u64) -> Result<Vec<u8>> {
        let full_path = self.resolve_path(path);
        debug!("Reading file range: {:?} (offset: {}, length: {})", full_path, offset, length);
        
        let mut file = fs::File::open(&full_path).await
            .with_context(|| format!("Failed to open file: {:?}", full_path))?;
        
        file.seek(std::io::SeekFrom::Start(offset)).await
            .with_context(|| format!("Failed to seek to offset {} in file: {:?}", offset, full_path))?;
        
        let mut buffer = vec![0u8; length as usize];
        let bytes_read = file.read(&mut buffer).await
            .with_context(|| format!("Failed to read from file: {:?}", full_path))?;
        
        buffer.truncate(bytes_read);
        Ok(buffer)
    }
    
    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<()> {
        let full_path = self.resolve_path(path);
        debug!("Writing file: {:?} ({} bytes)", full_path, data.len());
        
        // Create parent directories if they don't exist
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await
                .with_context(|| format!("Failed to create parent directories for: {:?}", full_path))?;
        }
        
        fs::write(&full_path, data).await
            .with_context(|| format!("Failed to write file: {:?}", full_path))
    }
    
    async fn get_metadata(&self, path: &Path) -> Result<VirtualFile> {
        let full_path = self.resolve_path(path);
        let metadata = fs::metadata(&full_path).await
            .with_context(|| format!("Failed to get metadata for: {:?}", full_path))?;
        
        let file_name = full_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let is_dir = metadata.is_dir();
        // Calculate folder size recursively for directories
        let size = if is_dir {
            self.calculate_dir_size(&full_path)
        } else {
            metadata.len()
        };
        
        // Get all native filesystem metadata exactly as it appears in Finder/Explorer
        let last_modified = metadata.modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let last_accessed = metadata.accessed().ok();
        
        let mut virtual_file = VirtualFile::new(
            file_name,
            path.to_path_buf(),
            size,
            is_dir,
        );
        
        // Set all native filesystem timestamps exactly as they appear in Finder
        virtual_file.last_modified = last_modified;
        virtual_file.last_accessed = last_accessed;
        
        Ok(virtual_file)
    }
    
    async fn exists(&self, path: &Path) -> Result<bool> {
        let full_path = self.resolve_path(path);
        Ok(full_path.exists())
    }
    
    async fn delete(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path);
        debug!("Deleting: {:?}", full_path);
        
        let metadata = fs::metadata(&full_path).await
            .with_context(|| format!("Failed to get metadata for: {:?}", full_path))?;
        
        if metadata.is_dir() {
            fs::remove_dir(&full_path).await
                .with_context(|| format!("Failed to remove directory: {:?}", full_path))
        } else {
            fs::remove_file(&full_path).await
                .with_context(|| format!("Failed to remove file: {:?}", full_path))
        }
    }
    
    async fn create_dir(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path);
        debug!("Creating directory: {:?}", full_path);
        
        fs::create_dir_all(&full_path).await
            .with_context(|| format!("Failed to create directory: {:?}", full_path))
    }
    
    async fn file_size(&self, path: &Path) -> Result<u64> {
        let full_path = self.resolve_path(path);
        let metadata = fs::metadata(&full_path).await
            .with_context(|| format!("Failed to get metadata for: {:?}", full_path))?;
        
        Ok(metadata.len())
    }
}

#[async_trait]
impl IFileOperations for LocalStorageAdapter {
    async fn list(&self, path: &Path) -> Result<Vec<FileEntry>> {
        let full_path = self.resolve_path(path);
        let mut entries = Vec::new();
        
        let mut dir = fs::read_dir(&full_path).await
            .with_context(|| format!("Failed to read directory: {:?}", full_path))?;
        
        while let Some(entry) = dir.next_entry().await? {
            let entry_path = entry.path();
            let metadata = entry.metadata().await?;
            
            let name = entry_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            let path_str = entry_path.to_string_lossy().to_string();
            
            entries.push(FileEntry {
                name,
                path: path_str,
                size: if metadata.is_dir() { 0 } else { metadata.len() },
                is_dir: metadata.is_dir(),
                is_file: metadata.is_file(),
                is_symlink: entry_path.is_symlink(),
                modified: metadata.modified().ok(),
                created: metadata.created().ok(),
                accessed: metadata.accessed().ok(),
                mode: self.get_mode(&metadata),
                mime_type: None,
            });
        }
        
        Ok(entries)
    }
    
    async fn stat(&self, path: &Path) -> Result<FileStat> {
        let full_path = self.resolve_path(path);
        let metadata = fs::metadata(&full_path).await
            .with_context(|| format!("Failed to stat: {:?}", full_path))?;
        
        Ok(FileStat {
            size: metadata.len(),
            is_dir: metadata.is_dir(),
            is_file: metadata.is_file(),
            is_symlink: full_path.is_symlink(),
            mtime: metadata.modified().ok(),
            atime: metadata.accessed().ok(),
            ctime: metadata.created().ok(),
            mode: self.get_mode(&metadata).unwrap_or(0o644),
            nlink: 1, // TODO: Get actual link count
            uid: 0,   // TODO: Get actual UID
            gid: 0,   // TODO: Get actual GID
            blksize: 4096, // Default block size
            blocks: (metadata.len() + 511) / 512, // Approximate blocks
        })
    }
    
    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        self.read_file(path).await
    }
    
    async fn read_range(&self, path: &Path, offset: u64, len: u64) -> Result<Vec<u8>> {
        self.read_file_range(path, offset, len).await
    }
    
    async fn write(&self, path: &Path, data: &[u8]) -> Result<()> {
        self.write_file(path, data).await
    }
    
    async fn append(&self, path: &Path, data: &[u8]) -> Result<()> {
        let full_path = self.resolve_path(path);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&full_path)
            .await
            .with_context(|| format!("Failed to open file for append: {:?}", full_path))?;
        
        file.write_all(data).await
            .with_context(|| format!("Failed to append to file: {:?}", full_path))?;
        
        Ok(())
    }
    
    async fn write_at(&self, path: &Path, offset: u64, data: &[u8]) -> Result<()> {
        let full_path = self.resolve_path(path);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&full_path)
            .await
            .with_context(|| format!("Failed to open file for write: {:?}", full_path))?;
        
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        file.write_all(data).await
            .with_context(|| format!("Failed to write at offset {}: {:?}", offset, full_path))?;
        
        Ok(())
    }
    
    async fn truncate(&self, path: &Path, len: u64) -> Result<()> {
        let full_path = self.resolve_path(path);
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&full_path)
            .await
            .with_context(|| format!("Failed to open file for truncate: {:?}", full_path))?;
        
        file.set_len(len).await
            .with_context(|| format!("Failed to truncate file: {:?}", full_path))?;
        
        Ok(())
    }
    
    async fn mkdir(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path);
        fs::create_dir(&full_path).await
            .with_context(|| format!("Failed to create directory: {:?}", full_path))
    }
    
    async fn mkdir_p(&self, path: &Path) -> Result<()> {
        self.create_dir(path).await
    }
    
    async fn rmdir(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path);
        fs::remove_dir(&full_path).await
            .with_context(|| format!("Failed to remove directory: {:?}", full_path))
    }
    
    async fn copy(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
        let from_path = self.resolve_path(from);
        let mut to_path = self.resolve_path(to);
        
        // Handle self-copy FIRST: if source and destination are the same, generate unique name
        // This must come before the "into itself" check because starts_with returns true for equal paths
        if from_path == to_path {
            warn!("Self-copy detected at native level: {:?}, generating unique name", from_path);
            if let Some(parent) = to_path.parent() {
                let file_name = to_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed");
                
                // Parse name and extension (handle .app bundles correctly)
                let (name_part, ext_part) = if let Some(dot_idx) = file_name.rfind('.') {
                    (&file_name[..dot_idx], &file_name[dot_idx..])
                } else {
                    (file_name, "")
                };
                
                // Generate unique name
                let mut counter = 0;
                loop {
                    counter += 1;
                    let new_name = if counter == 1 {
                        format!("{} copy{}", name_part, ext_part)
                    } else {
                        format!("{} copy {}{}", name_part, counter, ext_part)
                    };
                    let candidate = parent.join(&new_name);
                    if !candidate.exists() {
                        to_path = candidate;
                        info!("Generated unique destination: {:?}", to_path);
                        break;
                    }
                    if counter > 100 {
                        return Err(anyhow::anyhow!("Cannot generate unique name for copy"));
                    }
                }
            } else {
                return Err(anyhow::anyhow!("Cannot copy file or folder to itself"));
            }
        }
        
        // Check if we're trying to copy a directory into itself (infinite recursion)
        // This comes AFTER self-copy check, so from_path != to_path at this point
        if to_path.starts_with(&from_path) && from_path.is_dir() {
            return Err(anyhow::anyhow!(
                "Cannot copy folder \"{}\" into itself",
                from_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("folder")
            ));
        }
        
        // Create parent directory if needed
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Check if source is a directory
        if from_path.is_dir() {
            // Recursive directory copy
            self.copy_dir_recursive(&from_path, &to_path, &options).await
                .with_context(|| format!("Failed to copy directory {:?} to {:?}", from_path, to_path))?;
        } else {
            // Simple file copy
            // Check if destination exists and overwrite is disabled
            if to_path.exists() && !options.overwrite {
                return Err(anyhow::anyhow!("Destination file already exists: {:?}", to_path));
            }
            
            fs::copy(&from_path, &to_path).await
                .with_context(|| format!("Failed to copy {:?} to {:?}", from_path, to_path))?;
        }
        
        Ok(())
    }
    
    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        let from_path = self.resolve_path(from);
        let to_path = self.resolve_path(to);
        
        // Create parent directory if needed
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        fs::rename(&from_path, &to_path).await
            .with_context(|| format!("Failed to rename {:?} to {:?}", from_path, to_path))?;
        
        Ok(())
    }
    
    async fn mv(&self, from: &Path, to: &Path, _options: MoveOptions) -> Result<()> {
        self.rename(from, to).await
    }
    
    async fn rm(&self, path: &Path) -> Result<()> {
        self.delete(path).await
    }
    
    async fn rm_rf(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path);
        
        if full_path.is_dir() {
            fs::remove_dir_all(&full_path).await
                .with_context(|| format!("Failed to remove directory recursively: {:?}", full_path))
        } else {
            fs::remove_file(&full_path).await
                .with_context(|| format!("Failed to remove file: {:?}", full_path))
        }
    }
    
    async fn symlink(&self, target: &Path, link: &Path) -> Result<()> {
        let link_path = self.resolve_path(link);
        
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, &link_path)
                .with_context(|| format!("Failed to create symlink: {:?} -> {:?}", link_path, target))?;
        }
        
        #[cfg(windows)]
        {
            // On Windows, symlinks require admin privileges or developer mode
            std::os::windows::fs::symlink_file(target, &link_path)
                .or_else(|_| std::os::windows::fs::symlink_dir(target, &link_path))
                .with_context(|| format!("Failed to create symlink: {:?} -> {:?}", link_path, target))?;
        }
        
        #[cfg(not(any(unix, windows)))]
        {
            return Err(anyhow::anyhow!("Symlinks not supported on this platform"));
        }
        
        Ok(())
    }
    
    async fn readlink(&self, link: &Path) -> Result<String> {
        let link_path = self.resolve_path(link);
        let target = fs::read_link(&link_path).await
            .with_context(|| format!("Failed to read symlink: {:?}", link_path))?;
        Ok(target.to_string_lossy().to_string())
    }
    
    async fn touch(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path);
        
        // Create empty file if it doesn't exist, or update mtime if it does
        if full_path.exists() {
            let now = SystemTime::now();
            let file_time = FileTime::from_system_time(now);
            set_file_times(&full_path, file_time, file_time)?;
        } else {
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::File::create(&full_path).await?;
        }
        
        Ok(())
    }
    
    async fn exists(&self, path: &Path) -> Result<bool> {
        StorageAdapter::exists(self, path).await
    }
    
    async fn is_dir(&self, path: &Path) -> Result<bool> {
        let full_path = self.resolve_path(path);
        let metadata = fs::metadata(&full_path).await?;
        Ok(metadata.is_dir())
    }
    
    async fn is_file(&self, path: &Path) -> Result<bool> {
        let full_path = self.resolve_path(path);
        let metadata = fs::metadata(&full_path).await?;
        Ok(metadata.is_file())
    }
    
    async fn is_symlink(&self, path: &Path) -> Result<bool> {
        let full_path = self.resolve_path(path);
        Ok(full_path.is_symlink())
    }
    
    async fn chmod(&self, path: &Path, mode: u32) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let full_path = self.resolve_path(path);
            let perms = std::fs::Permissions::from_mode(mode);
            fs::set_permissions(&full_path, perms).await?;
            Ok(())
        }
        
        #[cfg(not(unix))]
        {
            let _ = (path, mode);
            Ok(())
        }
    }
    
    async fn chown(&self, path: &Path, uid: u32, gid: u32) -> Result<()> {
        #[cfg(unix)]
        {
            let full_path = self.resolve_path(path);
            // Note: chown requires appropriate permissions
            // In production, you'd use chown syscall
            let _ = (full_path, uid, gid);
            Ok(())
        }
        
        #[cfg(not(unix))]
        {
            let _ = (path, uid, gid);
            Ok(())
        }
    }
    
    async fn set_times(&self, path: &Path, atime: Option<SystemTime>, mtime: Option<SystemTime>) -> Result<()> {
        let full_path = self.resolve_path(path);
        
        // Get current times if not provided
        let metadata = std::fs::metadata(&full_path)?;
        let current_atime = metadata.accessed().ok();
        let current_mtime = metadata.modified().ok();
        
        let atime_ft = atime.map(FileTime::from_system_time)
            .or_else(|| current_atime.map(FileTime::from_system_time))
            .unwrap_or(FileTime::now());
        
        let mtime_ft = mtime.map(FileTime::from_system_time)
            .or_else(|| current_mtime.map(FileTime::from_system_time))
            .unwrap_or(FileTime::now());
        
        set_file_times(&full_path, atime_ft, mtime_ft)?;
        
        Ok(())
    }
    
    async fn file_size(&self, path: &Path) -> Result<u64> {
        StorageAdapter::file_size(self, path).await
    }
    
    async fn available_space(&self) -> Result<u64> {
        // TODO: Implement actual filesystem space check
        Ok(u64::MAX)
    }
    
    async fn total_space(&self) -> Result<u64> {
        // TODO: Implement actual filesystem space check
        Ok(u64::MAX)
    }
    
    fn is_read_only(&self) -> bool {
        false // Local storage is writable
    }
    
    fn root_path(&self) -> &Path {
        &self.base_path
    }
}

#[async_trait]
impl MountedStorage for LocalStorageAdapter {
    fn mount_point(&self) -> &Path {
        &self.base_path
    }
    
    async fn is_mounted(&self) -> Result<bool> {
        Ok(self.base_path.exists() && self.base_path.is_dir())
    }
    
    async fn mount(&self) -> Result<()> {
        // Local storage is always "mounted" - just ensure directory exists
        fs::create_dir_all(&self.base_path).await
            .with_context(|| format!("Failed to create mount point: {:?}", self.base_path))
    }
    
    async fn unmount(&self) -> Result<()> {
        // Local storage cannot be unmounted - this is a no-op
        Ok(())
    }
    
    async fn filesystem_stats(&self) -> Result<FilesystemStats> {
        use std::fs;
        
        let _metadata = fs::metadata(&self.base_path)
            .with_context(|| format!("Failed to get filesystem metadata: {:?}", self.base_path))?;
        
        // Get filesystem stats (platform-specific)
        #[cfg(unix)]
        {
            let _stat = _metadata;
            
            // Try to get actual filesystem stats
            let total_space = 0; // TODO: Use statvfs
            let available_space = 0;
            let used_space = 0;
            
            Ok(FilesystemStats {
                total_space,
                available_space,
                used_space,
                filesystem_type: None,
                block_size: 4096,
                total_inodes: None,
                available_inodes: None,
            })
        }
        
        #[cfg(windows)]
        {
            // Windows filesystem stats
            Ok(FilesystemStats {
                total_space: 0,
                available_space: 0,
                used_space: 0,
                filesystem_type: Some("NTFS".to_string()),
                block_size: 4096,
                total_inodes: None,
                available_inodes: None,
            })
        }
        
        #[cfg(not(any(unix, windows)))]
        {
            Ok(FilesystemStats {
                total_space: 0,
                available_space: 0,
                used_space: 0,
                filesystem_type: None,
                block_size: 4096,
                total_inodes: None,
                available_inodes: None,
            })
        }
    }
    
    async fn is_accessible(&self, path: &Path) -> Result<bool> {
        let full_path = self.resolve_path(path);
        // Check if we can read metadata (basic accessibility check)
        Ok(fs::metadata(&full_path).await.is_ok())
    }
    
    async fn get_permissions(&self, path: &Path) -> Result<u32> {
        let full_path = self.resolve_path(path);
        let metadata = fs::metadata(&full_path).await?;
        Ok(self.get_mode(&metadata).unwrap_or(0o644))
    }
    
    async fn set_permissions(&self, path: &Path, mode: u32) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let full_path = self.resolve_path(path);
            let perms = std::fs::Permissions::from_mode(mode);
            fs::set_permissions(&full_path, perms).await?;
            Ok(())
        }
        
        #[cfg(not(unix))]
        {
            // Permissions not supported on this platform
            let _ = (path, mode);
            Ok(())
        }
    }
    
    async fn get_owner(&self, path: &Path) -> Result<u32> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let full_path = self.resolve_path(path);
            let metadata = fs::metadata(&full_path).await?;
            Ok(metadata.uid())
        }
        
        #[cfg(not(unix))]
        {
            let _ = path;
            Ok(0)
        }
    }
    
    async fn set_owner(&self, path: &Path, uid: u32) -> Result<()> {
        #[cfg(unix)]
        {
            let full_path = self.resolve_path(path);
            // Note: This requires appropriate permissions
            // In production, you'd use chown syscall
            let _ = (full_path, uid);
            Ok(())
        }
        
        #[cfg(not(unix))]
        {
            let _ = (path, uid);
            Ok(())
        }
    }
    
    async fn get_group(&self, path: &Path) -> Result<u32> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let full_path = self.resolve_path(path);
            let metadata = fs::metadata(&full_path).await?;
            Ok(metadata.gid())
        }
        
        #[cfg(not(unix))]
        {
            let _ = path;
            Ok(0)
        }
    }
    
    async fn set_group(&self, path: &Path, gid: u32) -> Result<()> {
        #[cfg(unix)]
        {
            let full_path = self.resolve_path(path);
            // Note: This requires appropriate permissions
            let _ = (full_path, gid);
            Ok(())
        }
        
        #[cfg(not(unix))]
        {
            let _ = (path, gid);
            Ok(())
        }
    }
    
    async fn create_symlink(&self, target: &Path, link_path: &Path) -> Result<()> {
        self.symlink(target, link_path).await
    }
    
    async fn read_symlink(&self, link_path: &Path) -> Result<PathBuf> {
        let link_path_resolved = self.resolve_path(link_path);
        let target = fs::read_link(&link_path_resolved).await
            .with_context(|| format!("Failed to read symlink: {:?}", link_path_resolved))?;
        Ok(target)
    }
    
    async fn is_symlink(&self, path: &Path) -> Result<bool> {
        let full_path = self.resolve_path(path);
        Ok(full_path.is_symlink())
    }
    
    async fn get_xattr(&self, path: &Path, name: &str) -> Result<Option<Vec<u8>>> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;
            
            let full_path = self.resolve_path(path);
            let c_path = CString::new(full_path.as_os_str().as_bytes())?;
            let c_name = CString::new(name)?;
            
            // Get xattr size first
            let size = unsafe {
                libc::getxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                )
            };
            
            if size < 0 {
                return Ok(None);
            }
            
            // Read xattr value
            let mut buffer = vec![0u8; size as usize];
            let result = unsafe {
                libc::getxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    size as usize,
                    0,
                    0,
                )
            };
            
            if result < 0 {
                Ok(None)
            } else {
                Ok(Some(buffer))
            }
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (path, name);
            Ok(None)
        }
    }
    
    async fn set_xattr(&self, path: &Path, name: &str, value: &[u8]) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;
            
            let full_path = self.resolve_path(path);
            let c_path = CString::new(full_path.as_os_str().as_bytes())?;
            let c_name = CString::new(name)?;
            
            let result = unsafe {
                libc::setxattr(
                    c_path.as_ptr(),
                    c_name.as_ptr(),
                    value.as_ptr() as *const libc::c_void,
                    value.len(),
                    0,
                    0,
                )
            };
            
            if result < 0 {
                Err(anyhow::anyhow!("Failed to set xattr"))
            } else {
                Ok(())
            }
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (path, name, value);
            Ok(())
        }
    }
    
    async fn list_xattrs(&self, path: &Path) -> Result<Vec<String>> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;
            
            let full_path = self.resolve_path(path);
            let c_path = CString::new(full_path.as_os_str().as_bytes())?;
            
            // Get list size first
            let size = unsafe {
                libc::listxattr(c_path.as_ptr(), std::ptr::null_mut(), 0, 0)
            };
            
            if size <= 0 {
                return Ok(Vec::new());
            }
            
            // Read xattr names
            let mut buffer = vec![0u8; size as usize];
            let result = unsafe {
                libc::listxattr(
                    c_path.as_ptr(),
                    buffer.as_mut_ptr() as *mut i8,
                    size as usize,
                    0,
                )
            };
            
            if result < 0 {
                return Ok(Vec::new());
            }
            
            // Parse null-separated string list
            let names: Vec<String> = buffer
                .split(|&b| b == 0)
                .filter_map(|bytes| {
                    if bytes.is_empty() {
                        None
                    } else {
                        String::from_utf8(bytes.to_vec()).ok()
                    }
                })
                .collect();
            
            Ok(names)
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Ok(Vec::new())
        }
    }
    
    async fn get_change_time(&self, path: &Path) -> Result<Option<SystemTime>> {
        let full_path = self.resolve_path(path);
        let metadata = fs::metadata(&full_path).await?;
        Ok(metadata.created().ok().or_else(|| metadata.modified().ok()))
    }
    
    async fn get_access_time(&self, path: &Path) -> Result<Option<SystemTime>> {
        let full_path = self.resolve_path(path);
        let metadata = fs::metadata(&full_path).await?;
        Ok(metadata.accessed().ok())
    }
    
    async fn set_access_time(&self, path: &Path, atime: SystemTime) -> Result<()> {
        let full_path = self.resolve_path(path);
        let metadata = std::fs::metadata(&full_path)?;
        let current_mtime = metadata.modified().ok();
        
        let atime_ft = FileTime::from_system_time(atime);
        let mtime_ft = current_mtime.map(FileTime::from_system_time)
            .unwrap_or(FileTime::now());
        
        set_file_times(&full_path, atime_ft, mtime_ft)?;
        Ok(())
    }
    
    async fn set_modification_time(&self, path: &Path, mtime: SystemTime) -> Result<()> {
        let full_path = self.resolve_path(path);
        let metadata = std::fs::metadata(&full_path)?;
        let current_atime = metadata.accessed().ok();
        
        let mtime_ft = FileTime::from_system_time(mtime);
        let atime_ft = current_atime.map(FileTime::from_system_time)
            .unwrap_or(FileTime::now());
        
        set_file_times(&full_path, atime_ft, mtime_ft)?;
        Ok(())
    }
}

impl LocalStorageAdapter {
    /// Get file mode (Unix permissions) from metadata
    fn get_mode(&self, metadata: &std::fs::Metadata) -> Option<u32> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Some(metadata.mode())
        }
        
        #[cfg(not(unix))]
        {
            let _ = metadata;
            None
        }
    }
}
