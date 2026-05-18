//! Storage Port Tests
//!
//! Tests for StorageAdapter trait contract.

#[cfg(test)]
mod tests {
    use crate::vfs::ports::StorageAdapter;
    use crate::vfs::adapters::LocalStorageAdapter;
    
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_storage_adapter_trait() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = LocalStorageAdapter::new(
            temp_dir.path().to_path_buf(),
            "test".to_string(),
        );
        
        assert_eq!(adapter.name(), "test");
        assert!(adapter.test_connection().await.unwrap());
    }
}
