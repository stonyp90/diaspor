//! Local Storage Integration Tests
//!
//! End-to-end integration tests for local filesystem storage adapter.
//! These tests use temporary directories and don't require containers.

mod tests {
    use std::path::Path;
    use tempfile::TempDir;
    use diaspor_vfs_lib::vfs::adapters::LocalStorageAdapter;
    use diaspor_vfs_lib::vfs::ports::StorageAdapter;

    fn create_local_adapter() -> (LocalStorageAdapter, TempDir) {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let adapter = LocalStorageAdapter::new(
            temp_dir.path().to_path_buf(),
            "Local Test".to_string(),
        );
        (adapter, temp_dir)
    }

    #[tokio::test]
    async fn test_local_storage_adapter_contract() {
        let (adapter, _temp_dir) = create_local_adapter();

        // Run contract tests
        test_storage_type(&adapter).await;
        test_name(&adapter).await;
        test_test_connection(&adapter).await;
        test_list_files_root(&adapter).await;
        test_write_read_cycle(&adapter).await;
        test_create_dir(&adapter).await;
        test_delete_file(&adapter).await;
        test_read_file_range(&adapter).await;
    }

    async fn test_storage_type(adapter: &LocalStorageAdapter) {
        use diaspor_vfs_lib::vfs::domain::StorageSourceType;
        assert_eq!(adapter.storage_type(), StorageSourceType::Local);
    }

    async fn test_name(adapter: &LocalStorageAdapter) {
        assert!(!adapter.name().is_empty());
    }

    async fn test_test_connection(adapter: &LocalStorageAdapter) {
        let result = adapter.test_connection().await;
        assert!(result.is_ok());
    }

    async fn test_list_files_root(adapter: &LocalStorageAdapter) {
        let result = adapter.list_files(Path::new("/")).await;
        assert!(result.is_ok());
    }

    async fn test_write_read_cycle(adapter: &LocalStorageAdapter) {
        let test_path = Path::new("/test-file.txt");
        let test_data = b"Hello, Local Storage!";

        adapter.write_file(test_path, test_data).await.unwrap();
        assert!(adapter.exists(test_path).await.unwrap());

        let read_data = adapter.read_file(test_path).await.unwrap();
        assert_eq!(read_data, test_data);

        adapter.delete(test_path).await.unwrap();
    }

    async fn test_create_dir(adapter: &LocalStorageAdapter) {
        let test_dir = Path::new("/test-dir");
        adapter.create_dir(test_dir).await.unwrap();
        let _ = adapter.list_files(test_dir).await.unwrap();
    }

    async fn test_delete_file(adapter: &LocalStorageAdapter) {
        let test_path = Path::new("/test-delete.txt");
        adapter.write_file(test_path, b"delete me").await.unwrap();
        adapter.delete(test_path).await.unwrap();
        assert!(!adapter.exists(test_path).await.unwrap());
    }

    async fn test_read_file_range(adapter: &LocalStorageAdapter) {
        let test_path = Path::new("/test-range.txt");
        let full_data = b"0123456789";
        adapter.write_file(test_path, full_data).await.unwrap();

        let range_data = adapter.read_file_range(test_path, 3, 4).await.unwrap();
        assert_eq!(range_data, b"3456");

        adapter.delete(test_path).await.unwrap();
    }

    #[tokio::test]
    async fn test_local_storage_nested_directories() {
        let (adapter, _temp_dir) = create_local_adapter();

        // Create nested directory structure
        adapter.create_dir(Path::new("/level1")).await.unwrap();
        adapter.create_dir(Path::new("/level1/level2")).await.unwrap();
        adapter.create_dir(Path::new("/level1/level2/level3")).await.unwrap();

        // Create files at different levels
        adapter.write_file(Path::new("/level1/file1.txt"), b"level1").await.unwrap();
        adapter.write_file(Path::new("/level1/level2/file2.txt"), b"level2").await.unwrap();
        adapter.write_file(Path::new("/level1/level2/level3/file3.txt"), b"level3").await.unwrap();

        // List files at each level
        let level1_files = adapter.list_files(Path::new("/level1")).await.unwrap();
        assert!(level1_files.iter().any(|f| f.name == "file1.txt"));
        assert!(level1_files.iter().any(|f| f.name == "level2"));

        let level2_files = adapter.list_files(Path::new("/level1/level2")).await.unwrap();
        assert!(level2_files.iter().any(|f| f.name == "file2.txt"));
        assert!(level2_files.iter().any(|f| f.name == "level3"));
    }

    #[tokio::test]
    async fn test_local_storage_large_file() {
        let (adapter, _temp_dir) = create_local_adapter();
        let test_path = Path::new("/large-file.bin");

        // Create a 1MB file
        let large_data = vec![0x42u8; 1024 * 1024];
        adapter.write_file(test_path, &large_data).await.unwrap();

        let file_size = adapter.file_size(test_path).await.unwrap();
        assert_eq!(file_size, large_data.len() as u64);

        let read_data = adapter.read_file(test_path).await.unwrap();
        assert_eq!(read_data.len(), large_data.len());
        assert_eq!(read_data[0], 0x42);

        adapter.delete(test_path).await.unwrap();
    }
}
