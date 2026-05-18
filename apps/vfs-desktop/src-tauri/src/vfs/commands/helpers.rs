//! Shared Helper Functions
//!
//! Helper functions used across multiple command modules

use std::sync::OnceLock;
use std::sync::Arc;

use crate::vfs::multipart_upload::MultipartUploadManager;
use crate::vfs::download_manager::DownloadManager;
use crate::vfs::operation_tracker::OperationTracker;
use crate::vfs::audit_log::AuditLog;
use crate::vfs::adapters::JsonMetadataStore;
use crate::vfs::adapters::transcription::TranscriptionService;
use crate::vfs::adapters::thumbnail_queue;

// Note: CLIPBOARD is defined in clipboard.rs to avoid circular dependencies

/// Multipart upload manager
static MULTIPART_UPLOAD_MANAGER: OnceLock<MultipartUploadManager> = OnceLock::new();

/// Download manager
static DOWNLOAD_MANAGER: OnceLock<DownloadManager> = OnceLock::new();

/// Operation tracker
static OPERATION_TRACKER: OnceLock<OperationTracker> = OnceLock::new();

/// Audit log
static AUDIT_LOG: OnceLock<AuditLog> = OnceLock::new();

/// Metadata store
pub static METADATA_STORE: OnceLock<tokio::sync::RwLock<Option<Arc<JsonMetadataStore>>>> = OnceLock::new();

/// Thumbnail queue
static THUMBNAIL_QUEUE: OnceLock<Arc<tokio::sync::RwLock<Option<Arc<thumbnail_queue::ThumbnailQueue>>>>> = OnceLock::new();

/// Transcription service
static TRANSCRIPTION_SERVICE: OnceLock<Arc<tokio::sync::RwLock<Option<Arc<TranscriptionService>>>>> = OnceLock::new();

/// Get or initialize the transcription service
pub async fn get_transcription_service() -> Result<Arc<TranscriptionService>, String> {
    let service_lock = TRANSCRIPTION_SERVICE.get_or_init(|| {
        Arc::new(tokio::sync::RwLock::new(None))
    });
    
    let mut service_guard = service_lock.write().await;
    
    if let Some(ref service) = *service_guard {
        return Ok(service.clone());
    }
    
    // Initialize transcription service
    let temp_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("ursly")
        .join("transcription");
    
    let service = Arc::new(
        TranscriptionService::new(temp_dir)
            .await
            .map_err(|e| format!("Failed to initialize transcription service: {}", e))?
    );
    
    *service_guard = Some(service.clone());
    Ok(service)
}

/// Refresh transcription service models (call after downloading new models)
/// This reinitializes the service to pick up newly downloaded models
pub async fn refresh_transcription_models() -> Result<(), String> {
    let service_lock = TRANSCRIPTION_SERVICE.get_or_init(|| {
        Arc::new(tokio::sync::RwLock::new(None))
    });
    
    let mut service_guard = service_lock.write().await;
    
    // Reinitialize the service to pick up new models
    let temp_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("ursly")
        .join("transcription");
    
    let new_service = Arc::new(
        TranscriptionService::new(temp_dir)
            .await
            .map_err(|e| format!("Failed to reinitialize transcription service: {}", e))?
    );
    
    *service_guard = Some(new_service);
    
    Ok(())
}

pub fn get_upload_manager() -> &'static MultipartUploadManager {
    MULTIPART_UPLOAD_MANAGER.get_or_init(|| {
        let state_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("ursly")
            .join("multipart_uploads");
        MultipartUploadManager::new(&state_dir)
            .expect("Failed to initialize multipart upload manager")
    })
}

pub fn get_download_manager() -> &'static DownloadManager {
    DOWNLOAD_MANAGER.get_or_init(|| {
        let state_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("ursly")
            .join("downloads");
        DownloadManager::new(&state_dir)
            .expect("Failed to initialize download manager")
    })
}

pub fn get_operation_tracker() -> &'static OperationTracker {
    OPERATION_TRACKER.get_or_init(|| {
        let state_dir = dirs::data_dir()
            .unwrap_or_else(|| dirs::cache_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp")))
            .join("ursly")
            .join("vfs")
            .join("operations");
        OperationTracker::new(&state_dir, 1000) // Keep last 1000 operations in memory, all in audit log
            .expect("Failed to initialize operation tracker")
    })
}

pub fn get_audit_log() -> &'static AuditLog {
    AUDIT_LOG.get_or_init(|| {
        let audit_dir = dirs::data_dir()
            .unwrap_or_else(|| dirs::cache_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp")))
            .join("ursly")
            .join("vfs")
            .join("audit");
        AuditLog::new(&audit_dir, 0) // 0 = unlimited entries for audit log
            .expect("Failed to initialize audit log")
    })
}

pub async fn get_metadata_store() -> Result<&'static tokio::sync::RwLock<Option<Arc<JsonMetadataStore>>>, String> {
    let store = METADATA_STORE.get_or_init(|| tokio::sync::RwLock::new(None));
    Ok(store)
}

pub fn get_current_user_id() -> Option<String> {
    // For now, use system username as user ID
    // In future, this should come from authentication/session
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
}

pub fn get_current_organization_id() -> Option<String> {
    // For now, return None (organization support not yet implemented)
    // In future, this should come from authentication/session
    None
}

/// Get or initialize the thumbnail queue
pub async fn get_thumbnail_queue() -> Result<Arc<thumbnail_queue::ThumbnailQueue>, String> {
    let queue_lock = THUMBNAIL_QUEUE.get_or_init(|| {
        Arc::new(tokio::sync::RwLock::new(None))
    });
    
    let mut queue_guard = queue_lock.write().await;
    
    if let Some(ref queue) = *queue_guard {
        return Ok(queue.clone());
    }
    
    // Initialize thumbnail queue
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("ursly")
        .join("thumbnails");
    
    let queue = Arc::new(
        thumbnail_queue::ThumbnailQueue::new(cache_dir, 3) // Max 3 concurrent thumbnail generations
            .await
            .map_err(|e| format!("Failed to initialize thumbnail queue: {}", e))?
    );
    
    *queue_guard = Some(queue.clone());
    Ok(queue)
}

/// Get or initialize the metadata store
/// Returns a reference to the store that can be used to call trait methods
pub async fn get_metadata_store_instance() -> Result<Arc<JsonMetadataStore>, String> {
    let store_lock = METADATA_STORE.get_or_init(|| tokio::sync::RwLock::new(None));
    
    // Check if already initialized
    {
        let store_guard = store_lock.read().await;
        if let Some(ref store) = *store_guard {
            return Ok(store.clone());
        }
    }
    
    // Initialize metadata store
    let store = Arc::new(
        JsonMetadataStore::default_store()
            .await
            .map_err(|e| format!("Failed to initialize metadata store: {}", e))?
    );
    
    {
        let mut store_guard = store_lock.write().await;
        *store_guard = Some(store.clone());
    }
    
    Ok(store)
}

/// Check if a storage source type is object storage (S3, GCS, Azure Blob, Oracle, or S3-compatible)
pub fn is_object_storage_type(source_type: &crate::vfs::domain::StorageSourceType) -> bool {
    use crate::vfs::domain::StorageSourceType;
    
    match source_type {
        StorageSourceType::S3 
        | StorageSourceType::Gcs 
        | StorageSourceType::AzureBlob 
        | StorageSourceType::S3Compatible => true,
        StorageSourceType::Custom(id) => {
            // Check if Custom type is Oracle (detected by provider ID or endpoint)
            // Oracle uses S3Compatible but may be registered as Custom("oracle")
            id == "oracle" || id == "oracle-object-storage"
        },
        _ => false,
    }
}

/// Check if a storage source ID is object storage
/// Requires access to VfsService to look up the source type
pub fn is_object_storage_source_id(
    source_id: &str,
    service: &crate::vfs::application::VfsService,
) -> bool {
    let sources = service.list_sources();
    if let Some(source) = sources.iter().find(|s| s.id == source_id) {
        is_object_storage_type(&source.source_type)
    } else {
        false
    }
}
