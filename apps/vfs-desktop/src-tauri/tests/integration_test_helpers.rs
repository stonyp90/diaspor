//! Integration Test Helpers
//!
//! Provides utilities for setting up testcontainers and test fixtures
//! for end-to-end integration testing of storage adapters.

use std::time::Duration;
use tempfile::TempDir;

/// Test configuration for integration tests
#[allow(dead_code)]
pub struct TestConfig {
    pub timeout: Duration,
    pub cleanup_on_drop: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            cleanup_on_drop: true,
        }
    }
}

/// Create a temporary directory for local storage tests
#[allow(dead_code)]
pub fn create_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temporary directory")
}

/// Generate a unique test bucket name
#[allow(dead_code)]
pub fn generate_test_bucket_name(prefix: &str) -> String {
    use uuid::Uuid;
    format!("{}-{}", prefix, Uuid::new_v4().to_string().replace("-", ""))
}

/// Wait for a service to be ready
#[allow(dead_code)]
pub async fn wait_for_service<F, Fut>(check: F, max_attempts: u32) -> bool
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
