//! Local Storage Adapter Tests
//!
//! Tests for LocalStorageAdapter implementation.

#[cfg(test)]
mod tests {
    use crate::vfs::adapters::LocalStorageAdapter;
    use crate::vfs::ports::{StorageAdapter, IFileOperations};
    use std::path::Path;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_storage_list_files() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create test structure
        std::fs::create_dir(temp_dir.path().join("Documents")).unwrap();
        std::fs::write(temp_dir.path().join("readme.txt"), "Hello").unwrap();
        
        let adapter = LocalStorageAdapter::new(
            temp_dir.path().to_path_buf(),
            "test".to_string(),
        );
        
        let files = adapter.list_files(Path::new("/")).await.unwrap();
        assert_eq!(files.len(), 2);
    }

    #[tokio::test]
    async fn test_local_storage_read_write() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = LocalStorageAdapter::new(
            temp_dir.path().to_path_buf(),
            "test".to_string(),
        );
        
        let test_data = b"Hello, World!";
        adapter.write(Path::new("/test.txt"), test_data).await.unwrap();
        
        let read_data = adapter.read(Path::new("/test.txt")).await.unwrap();
        assert_eq!(read_data, test_data);
    }
}
