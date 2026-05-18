//! VfsService Tests
//!
//! Tests for VfsService application service.

#[cfg(test)]
mod tests {
    use crate::vfs::application::VfsService;
    use crate::vfs::domain::{StorageSourceType, ConnectionStatus};
    use tempfile::TempDir;
    use std::path::Path;

    #[tokio::test]
    async fn test_vfs_service_local_source() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create test file
        std::fs::write(temp_dir.path().join("test.txt"), "hello").unwrap();
        
        let service = VfsService::new().await.unwrap();
        
        // Add local source
        let source = service.add_local_source(
            "Test".to_string(),
            temp_dir.path().to_path_buf(),
        ).await.unwrap();
        
        assert_eq!(source.source_type, StorageSourceType::Local);
        assert_eq!(source.status, ConnectionStatus::Connected);
        
        // Verify mounted storage properties
        assert!(source.mounted, "Local source should be marked as mounted");
        assert!(source.mount_point.is_some(), "Local source should have a mount_point");
        assert_eq!(source.mount_point, Some(temp_dir.path().to_path_buf()), "Mount point should match the provided path");
        
        // List files
        let files = service.list_files(&source.id, Path::new("/")).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "test.txt");
        
        // Read file
        let data = service.read_file(&source.id, Path::new("/test.txt")).await.unwrap();
        assert_eq!(data, b"hello");
    }
    
    #[tokio::test]
    async fn test_mounted_storage_persistence() {
        let temp_dir = TempDir::new().unwrap();
        
        let service = VfsService::new().await.unwrap();
        
        // Add local source
        let source = service.add_local_source(
            "Test Mount".to_string(),
            temp_dir.path().to_path_buf(),
        ).await.unwrap();
        
        // Verify source is accessible after adding
        let retrieved_source = service.get_source(&source.id);
        assert!(retrieved_source.is_some(), "Source should be retrievable after adding");
        
        let retrieved = retrieved_source.unwrap();
        assert!(retrieved.mounted, "Retrieved source should still be marked as mounted");
        assert!(retrieved.mount_point.is_some(), "Retrieved source should still have mount_point");
        assert_eq!(retrieved.mount_point, source.mount_point, "Mount point should persist");
    }
}
