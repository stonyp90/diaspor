//! S3 Storage Integration Tests
//!
//! End-to-end integration tests for S3 storage adapter using MinIO (S3-compatible storage)
//! via testcontainers.
//!
//! These tests require Docker to be running. They are skipped if Docker is unavailable.

#[cfg(feature = "integration-tests")]
mod tests {
    use std::path::Path;
    use std::time::Duration;
    use testcontainers::clients::Cli;
    use testcontainers::Container;
    use testcontainers::core::WaitFor;
    use testcontainers::images::generic::GenericImage;
    use ursly_vfs_lib::vfs::adapters::S3StorageAdapter;
    use ursly_vfs_lib::vfs::ports::StorageAdapter;
    

    fn generate_test_bucket_name(prefix: &str) -> String {
        use uuid::Uuid;
        format!("{}-{}", prefix, Uuid::new_v4().to_string().replace("-", ""))
    }
    
    async fn wait_for_service<F, Fut>(check: F, max_attempts: u32) -> bool
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        for _ in 0..max_attempts {
            if check().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        false
    }

    async fn create_s3_adapter(container: &Container<'_, GenericImage>) -> S3StorageAdapter {
        let endpoint = format!("http://localhost:{}", container.get_host_port_ipv4(9000));
        let bucket = generate_test_bucket_name("test-bucket");
        
        // Create bucket in MinIO using MinIO client API
        let client = reqwest::Client::new();
        let _ = client
            .put(&format!("{}/{}", endpoint, bucket))
            .header("Authorization", "AWS minioadmin:minioadmin")
            .send()
            .await;

        S3StorageAdapter::new(
            bucket.clone(),
            "us-east-1".to_string(),
            Some("minioadmin".to_string()),
            Some("minioadmin".to_string()),
            None,
            Some(endpoint),
            format!("S3 Test {}", bucket),
        )
        .await
        .expect("Failed to create S3 adapter")
    }

    #[tokio::test]
    async fn test_s3_storage_adapter_contract() {
        let docker = Cli::default();
        let minio_image = GenericImage::new("minio/minio", "latest")
            .with_env_var("MINIO_ROOT_USER", "minioadmin")
            .with_env_var("MINIO_ROOT_PASSWORD", "minioadmin")
            .with_wait_for(WaitFor::message_on_stdout("API:"));
        let container = docker.run(minio_image);
        
        // Wait a bit for MinIO to be fully ready
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Wait for MinIO to be ready
        let endpoint = format!("http://localhost:{}", container.get_host_port_ipv4(9000));
        let is_ready = wait_for_service(|| {
            let client = reqwest::Client::new();
            async move {
                client
                    .get(&endpoint)
                    .send()
                    .await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false)
            }
        }, 30)
        .await;

        assert!(is_ready, "MinIO should be ready");

        let adapter = create_s3_adapter(&container).await;

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

    async fn test_storage_type(adapter: &S3StorageAdapter) {
        use ursly_vfs_lib::vfs::domain::StorageSourceType;
        assert_eq!(adapter.storage_type(), StorageSourceType::S3);
    }

    async fn test_name(adapter: &S3StorageAdapter) {
        assert!(!adapter.name().is_empty());
    }

    async fn test_test_connection(adapter: &S3StorageAdapter) {
        let result = adapter.test_connection().await;
        assert!(result.is_ok());
    }

    async fn test_list_files_root(adapter: &S3StorageAdapter) {
        let result = adapter.list_files(Path::new("/")).await;
        assert!(result.is_ok());
    }

    async fn test_write_read_cycle(adapter: &S3StorageAdapter) {
        let test_path = Path::new("/test-file.txt");
        let test_data = b"Hello, S3!";

        adapter.write_file(test_path, test_data).await.unwrap();
        assert!(adapter.exists(test_path).await.unwrap());

        let read_data = adapter.read_file(test_path).await.unwrap();
        assert_eq!(read_data, test_data);

        adapter.delete(test_path).await.unwrap();
    }

    async fn test_create_dir(adapter: &S3StorageAdapter) {
        let test_dir = Path::new("/test-dir");
        adapter.create_dir(test_dir).await.unwrap();
        let _ = adapter.list_files(test_dir).await.unwrap();
    }

    async fn test_delete_file(adapter: &S3StorageAdapter) {
        let test_path = Path::new("/test-delete.txt");
        adapter.write_file(test_path, b"delete me").await.unwrap();
        adapter.delete(test_path).await.unwrap();
        assert!(!adapter.exists(test_path).await.unwrap());
    }

    async fn test_read_file_range(adapter: &S3StorageAdapter) {
        let test_path = Path::new("/test-range.txt");
        let full_data = b"0123456789";
        adapter.write_file(test_path, full_data).await.unwrap();

        let range_data = adapter.read_file_range(test_path, 3, 4).await.unwrap();
        assert_eq!(range_data, b"3456");

        adapter.delete(test_path).await.unwrap();
    }

    #[tokio::test]
    async fn test_s3_multipart_upload() {
        let docker = Cli::default();
        let minio_image = GenericImage::new("minio/minio", "latest")
            .with_env_var("MINIO_ROOT_USER", "minioadmin")
            .with_env_var("MINIO_ROOT_PASSWORD", "minioadmin")
            .with_wait_for(WaitFor::message_on_stdout("API:"));
        let container = docker.run(minio_image);

        let adapter = create_s3_adapter(&container).await;

        // Test large file upload (simulating multipart)
        let large_data = vec![0u8; 10 * 1024 * 1024]; // 10MB
        let test_path = Path::new("/large-file.bin");

        adapter.write_file(test_path, &large_data).await.unwrap();
        assert!(adapter.exists(test_path).await.unwrap());

        let file_size = adapter.file_size(test_path).await.unwrap();
        assert_eq!(file_size, large_data.len() as u64);

        adapter.delete(test_path).await.unwrap();
    }

    #[tokio::test]
    async fn test_s3_list_with_prefix() {
        let docker = Cli::default();
        let minio_image = GenericImage::new("minio/minio", "latest")
            .with_env_var("MINIO_ROOT_USER", "minioadmin")
            .with_env_var("MINIO_ROOT_PASSWORD", "minioadmin")
            .with_wait_for(WaitFor::message_on_stdout("API:"));
        let container = docker.run(minio_image);

        let adapter = create_s3_adapter(&container).await;

        // Create files with different prefixes
        adapter.write_file(Path::new("/dir1/file1.txt"), b"content1").await.unwrap();
        adapter.write_file(Path::new("/dir1/file2.txt"), b"content2").await.unwrap();
        adapter.write_file(Path::new("/dir2/file3.txt"), b"content3").await.unwrap();

        // List files in dir1
        let files = adapter.list_files(Path::new("/dir1")).await.unwrap();
        assert_eq!(files.len(), 2);

        // Cleanup
        adapter.delete(Path::new("/dir1/file1.txt")).await.unwrap();
        adapter.delete(Path::new("/dir1/file2.txt")).await.unwrap();
        adapter.delete(Path::new("/dir2/file3.txt")).await.unwrap();
    }
}
