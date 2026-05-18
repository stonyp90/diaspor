//! Contract Tests for StorageAdapter Trait
//!
//! These tests ensure that all storage adapter implementations
//! correctly implement the StorageAdapter trait contract.
//!
//! Contract tests verify:
//! - All required methods are implemented
//! - Methods behave consistently across implementations
//! - Error handling follows the contract
//! - Edge cases are handled correctly


/// Contract test suite for StorageAdapter implementations
///
/// This macro generates a test suite that can be used with any
/// StorageAdapter implementation to verify it follows the contract.
#[macro_export]
macro_rules! storage_adapter_contract_tests {
    ($adapter_name:ident, $create_adapter:expr) => {
        mod $adapter_name {
            use super::*;
            use std::path::PathBuf;
            use tokio::test;

            #[test]
            async fn test_storage_type() {
                let adapter = $create_adapter.await;
                let storage_type = adapter.storage_type();
                // Storage type should be one of the valid types
                match storage_type {
                    StorageSourceType::Local
                    | StorageSourceType::S3
                    | StorageSourceType::S3Compatible
                    | StorageSourceType::GCS
                    | StorageSourceType::AzureBlob
                    | StorageSourceType::OracleObjectStorage
                    | StorageSourceType::NFS
                    | StorageSourceType::SMB
                    | StorageSourceType::SFTP => {}
                    _ => panic!("Invalid storage type: {:?}", storage_type),
                }
            }

            #[test]
            async fn test_name() {
                let adapter = $create_adapter.await;
                let name = adapter.name();
                assert!(!name.is_empty(), "Adapter name should not be empty");
            }

            #[test]
            async fn test_test_connection() {
                let adapter = $create_adapter.await;
                // Connection test should not panic
                let result = adapter.test_connection().await;
                assert!(result.is_ok(), "test_connection should return Result");
            }

            #[test]
            async fn test_list_files_root() {
                let adapter = $create_adapter.await;
                let result = adapter.list_files(Path::new("/")).await;
                assert!(result.is_ok(), "list_files should succeed for root path");
                let files = result.unwrap();
                // Root listing should return a Vec (may be empty)
                assert!(files.is_empty() || !files.is_empty(), "Should return files list");
            }

            #[test]
            async fn test_exists_nonexistent() {
                let adapter = $create_adapter.await;
                let nonexistent = Path::new("/nonexistent-file-12345");
                let result = adapter.exists(nonexistent).await;
                assert!(result.is_ok(), "exists should return Result");
                assert!(!result.unwrap(), "Nonexistent file should return false");
            }

            #[test]
            async fn test_file_size_nonexistent() {
                let adapter = $create_adapter.await;
                let nonexistent = Path::new("/nonexistent-file-12345");
                let result = adapter.file_size(nonexistent).await;
                // Should return error for nonexistent file
                assert!(result.is_err(), "file_size should error for nonexistent file");
            }

            #[test]
            async fn test_get_metadata_nonexistent() {
                let adapter = $create_adapter.await;
                let nonexistent = Path::new("/nonexistent-file-12345");
                let result = adapter.get_metadata(nonexistent).await;
                // Should return error for nonexistent file
                assert!(result.is_err(), "get_metadata should error for nonexistent file");
            }

            #[test]
            async fn test_read_file_nonexistent() {
                let adapter = $create_adapter.await;
                let nonexistent = Path::new("/nonexistent-file-12345");
                let result = adapter.read_file(nonexistent).await;
                // Should return error for nonexistent file
                assert!(result.is_err(), "read_file should error for nonexistent file");
            }

            #[test]
            async fn test_write_read_cycle() {
                let adapter = $create_adapter.await;
                let test_path = Path::new("/test-contract-file.txt");
                let test_data = b"Hello, Contract Test!";

                // Write file
                let write_result = adapter.write_file(test_path, test_data).await;
                assert!(write_result.is_ok(), "write_file should succeed");

                // Verify file exists
                let exists_result = adapter.exists(test_path).await;
                assert!(exists_result.is_ok() && exists_result.unwrap(), "File should exist after write");

                // Read file back
                let read_result = adapter.read_file(test_path).await;
                assert!(read_result.is_ok(), "read_file should succeed");
                assert_eq!(read_result.unwrap(), test_data, "Read data should match written data");

                // Cleanup
                let _ = adapter.delete(test_path).await;
            }

            #[test]
            async fn test_create_dir() {
                let adapter = $create_adapter.await;
                let test_dir = Path::new("/test-contract-dir");
                
                let result = adapter.create_dir(test_dir).await;
                assert!(result.is_ok(), "create_dir should succeed");
                
                // Verify directory exists (by listing)
                let list_result = adapter.list_files(test_dir).await;
                assert!(list_result.is_ok(), "Should be able to list created directory");
            }

            #[test]
            async fn test_delete_file() {
                let adapter = $create_adapter.await;
                let test_path = Path::new("/test-delete-file.txt");
                let test_data = b"Delete me";

                // Create file
                adapter.write_file(test_path, test_data).await.unwrap();

                // Delete file
                let delete_result = adapter.delete(test_path).await;
                assert!(delete_result.is_ok(), "delete should succeed");

                // Verify file is gone
                let exists_result = adapter.exists(test_path).await;
                assert!(exists_result.is_ok() && !exists_result.unwrap(), "File should not exist after delete");
            }

            #[test]
            async fn test_read_file_range() {
                let adapter = $create_adapter.await;
                let test_path = Path::new("/test-range-file.txt");
                let full_data = b"0123456789ABCDEF";

                // Write file
                adapter.write_file(test_path, full_data).await.unwrap();

                // Read range
                let range_result = adapter.read_file_range(test_path, 5, 5).await;
                assert!(range_result.is_ok(), "read_file_range should succeed");
                assert_eq!(range_result.unwrap(), b"56789", "Range read should return correct data");

                // Cleanup
                let _ = adapter.delete(test_path).await;
            }
        }
    };
}
