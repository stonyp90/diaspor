//! Setup Commands
//!
//! Commands for setting up default storage sources and configurations

use tauri::State;
use tracing::info;
use super::state::VfsStateWrapper;
use super::responses::VfsStorageSourceResponse;

/// Setup S3 Testing storage source with credentials from environment
/// This is a convenience command for development/testing
#[tauri::command]
pub async fn vfs_setup_s3_testing(
    state: State<'_, VfsStateWrapper>,
) -> Result<VfsStorageSourceResponse, String> {
    use super::storage::vfs_register_s3_source;
    
    // Read credentials from environment variables
    let access_key = std::env::var("AWS_ACCESS_KEY_ID")
        .or_else(|_| std::env::var("aws_access_key_id"))
        .ok();
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
        .or_else(|_| std::env::var("aws_secret_access_key"))
        .ok();
    let session_token = std::env::var("AWS_SESSION_TOKEN")
        .or_else(|_| std::env::var("aws_session_token"))
        .ok();
    let region = std::env::var("AWS_REGION")
        .or_else(|_| std::env::var("aws_region"))
        .unwrap_or_else(|_| "us-east-2".to_string());
    let bucket = std::env::var("AWS_BUCKET")
        .or_else(|_| std::env::var("aws_bucket"))
        .unwrap_or_else(|_| "diaspor-vfs-test".to_string());
    
    if access_key.is_none() || secret_key.is_none() {
        return Err("AWS credentials not found in environment variables. Please set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY.".to_string());
    }
    
    info!("[vfs_setup_s3_testing] Setting up S3 Testing storage source");
    
    // Use the existing register command
    vfs_register_s3_source(
        "S3 Testing".to_string(),
        bucket,
        region,
        access_key,
        secret_key,
        session_token,
        None, // endpoint
        state,
    ).await
}
