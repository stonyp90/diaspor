//! Integration Tests
//!
//! End-to-end integration tests for storage adapters using testcontainers.
//!
//! These tests require Docker to be running and use testcontainers to spin up
//! real service instances for testing.
//!
//! Run with: cargo test --test integration --features integration-tests

#[cfg(all(test, feature = "integration-tests"))]
mod s3_storage_test;

#[cfg(test)]
mod local_storage_test;
