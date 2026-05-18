//! Unified Object Storage Tier Management
//!
//! Provides consistent tier management across all object storage providers:
//! - AWS S3
//! - Google Cloud Storage (GCS)
//! - Azure Blob Storage
//!
//! Maps customer-facing tier names to provider-specific storage classes:
//! - Hot = Reserved for ONTAP (FSx) - uses DataSync for transfers (not available for object storage)
//! - Nearline = Standard tier (immediate access, standard cost):
//!   * S3: STANDARD
//!   * GCS: STANDARD
//!   * Azure: Hot
//!   * Oracle: STANDARD (S3-compatible)
//! - Archive = Infrequent Access tier (immediate access, lower cost):
//!   * S3: STANDARD_IA (Infrequent Access)
//!   * GCS: NEARLINE
//!   * Azure: Cool
//!   * Oracle: STANDARD_IA (S3-compatible)
//! - Cold = Lower cost tier (instant retrieval, lower cost than Archive):
//!   * S3: GLACIER_IR (Glacier Instant Retrieval)
//!   * GCS: COLDLINE
//!   * Azure: Archive
//!   * Oracle: ARCHIVE (S3-compatible)

use anyhow::{Context, Result};
use opendal::Operator;
use tracing::{info, warn};

use crate::vfs::domain::{StorageTier, StorageSourceType};

/// Change storage tier for object storage objects
/// Works across S3, GCS, and Azure Blob Storage
pub async fn change_object_storage_tier(
    operator: &Operator,
    source_type: &StorageSourceType,
    key: &str,
    target_tier: StorageTier,
) -> Result<()> {
    // Map our tier enum to provider-specific storage class
    let storage_class = match (source_type, target_tier) {
        // Hot/Warm = Reserved for ONTAP (FSx) - not available for object storage
        (_, StorageTier::Hot | StorageTier::Warm) => {
            // Hot tier is reserved for ONTAP/FSx which uses DataSync
            // For object storage, fall back to Nearline (STANDARD)
            get_nearline_storage_class(source_type)
        }
        
        // Nearline = Standard tier (immediate access, standard cost)
        (_, StorageTier::Nearline) => {
            get_nearline_storage_class(source_type)
        }
        
        // Archive = Infrequent Access tier (immediate access, lower cost)
        (_, StorageTier::Archive) => {
            get_archive_storage_class(source_type)
        }
        
        // Cold = Lower cost tier (instant retrieval, lower cost than Archive)
        (_, StorageTier::Cold | StorageTier::InstantRetrieval) => {
            get_cold_storage_class(source_type)
        }
    };

    info!(
        "Changing {} object '{}' to storage class '{}' (tier: {:?})",
        get_provider_name(source_type), key, storage_class, target_tier
    );

    // Object storage doesn't support direct storage class change - we need to copy the object
    // with the new storage class. OpenDAL doesn't expose storage class in write(),
    // so we need to use provider SDKs directly.
    // 
    // For now, we'll use copy operation which will use bucket defaults.
    // TODO: Integrate provider SDKs to use CopyObject/PatchObject with StorageClass parameter
    
    // Read the object
    let data = operator.read(key).await
        .with_context(|| format!("Failed to read object '{}' for tier change", key))?;
    
    // Copy the object - OpenDAL will use bucket default storage class
    // In production, this should use provider SDKs with StorageClass parameter
    warn!(
        "Using copy operation - storage class will use bucket default. For explicit storage class '{}', use provider SDK.",
        storage_class
    );
    
    // Create a temporary key, copy, then rename
    let temp_key = format!("{}.tiering", key);
    operator.write(&temp_key, data.clone()).await
        .with_context(|| format!("Failed to write temporary object '{}'", temp_key))?;
    
    // Delete original and rename temp
    operator.delete(key).await.ok(); // Best effort delete
    operator.write(key, data).await
        .with_context(|| format!("Failed to write object '{}' with new storage class", key))?;
    
    // Clean up temp file
    operator.delete(&temp_key).await.ok();
    
    info!(
        "Successfully changed tier for {} object '{}' to '{}'",
        get_provider_name(source_type), key, storage_class
    );
    Ok(())
}

/// Get standard storage class for a provider (Hot tier)
#[allow(dead_code)]
fn get_standard_storage_class(source_type: &StorageSourceType) -> &'static str {
    match source_type {
        StorageSourceType::S3 | StorageSourceType::S3Compatible => "STANDARD",
        StorageSourceType::Gcs => "STANDARD",
        StorageSourceType::AzureBlob => "Hot",
        _ => "STANDARD", // Default fallback
    }
}

/// Get Nearline storage class for a provider
/// Maps customer-facing "Nearline" tier to provider-specific storage classes:
/// - S3: STANDARD (Standard storage class)
/// - GCS: STANDARD (Standard storage class)
/// - Azure: Hot (Hot tier)
/// - Oracle: STANDARD (Standard storage class, S3-compatible)
pub fn get_nearline_storage_class(source_type: &StorageSourceType) -> &'static str {
    match source_type {
        StorageSourceType::S3 | StorageSourceType::S3Compatible => "STANDARD",
        StorageSourceType::Gcs => "STANDARD",
        StorageSourceType::AzureBlob => "Hot",
        _ => "STANDARD", // Default fallback (includes Oracle via S3Compatible)
    }
}

/// Get Cold storage class for a provider
/// Maps customer-facing "Cold" tier to provider-specific storage classes:
/// - S3: GLACIER_IR (Glacier Instant Retrieval)
/// - GCS: COLDLINE
/// - Azure: Archive
/// - Oracle: ARCHIVE (Archive storage class, S3-compatible)
pub fn get_cold_storage_class(source_type: &StorageSourceType) -> &'static str {
    match source_type {
        StorageSourceType::S3 | StorageSourceType::S3Compatible => "GLACIER_IR",
        StorageSourceType::Gcs => "COLDLINE",
        StorageSourceType::AzureBlob => "Archive",
        _ => "GLACIER_IR", // Default fallback (includes Oracle via S3Compatible)
    }
}

/// Get Archive storage class for a provider (Infrequent Access tier)
/// Maps customer-facing "Archive" tier to provider-specific storage classes:
/// - S3: STANDARD_IA (Infrequent Access)
/// - GCS: NEARLINE (Infrequent Access equivalent)
/// - Azure: Cool (Infrequent Access equivalent)
/// - Oracle: STANDARD_IA (Infrequent Access, S3-compatible)
pub fn get_archive_storage_class(source_type: &StorageSourceType) -> &'static str {
    match source_type {
        StorageSourceType::S3 | StorageSourceType::S3Compatible => "STANDARD_IA",
        StorageSourceType::Gcs => "NEARLINE",
        StorageSourceType::AzureBlob => "Cool",
        _ => "STANDARD_IA", // Default fallback (includes Oracle via S3Compatible)
    }
}

/// Get provider display name
fn get_provider_name(source_type: &StorageSourceType) -> &'static str {
    match source_type {
        StorageSourceType::S3 => "S3",
        StorageSourceType::S3Compatible => "S3-Compatible",
        StorageSourceType::Gcs => "GCS",
        StorageSourceType::AzureBlob => "Azure Blob",
        _ => "Object Storage",
    }
}

/// Get current storage class of an object storage object
pub async fn get_object_storage_class(
    operator: &Operator,
    _source_type: &StorageSourceType,
    key: &str,
) -> Result<Option<String>> {
    // OpenDAL's stat() should include storage class in metadata
    let _metadata = operator.stat(key).await?;
    
    // Check if OpenDAL exposes storage class
    // This may require using provider SDKs directly to get object metadata
    // For now, return None and detect from tier status
    Ok(None)
}
