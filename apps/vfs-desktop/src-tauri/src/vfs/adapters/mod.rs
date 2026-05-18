//! Adapters Layer - Concrete implementations of ports
//!
//! Adapters implement the port interfaces and handle the actual
//! communication with external systems (S3, local filesystem, etc.)

pub mod local_storage;
pub mod s3_storage;
pub mod s3_tiering;
pub mod object_storage_tiering;
pub mod nvme_cache;
pub mod tauri_event_bus;
pub mod ffmpeg_media;
pub mod fsxn_storage;
pub mod gcs_storage;
pub mod azure_blob_storage;
pub mod oracle_object_storage;
pub mod nas_storage;
pub mod clipboard;
pub mod metadata_store;
pub mod native_thumbnail;
pub mod thumbnail_queue;
pub mod transcription;
pub mod ollama_client;
pub mod model_manager;
pub mod factories;
pub mod settings;

pub use local_storage::LocalStorageAdapter;
pub use s3_storage::S3StorageAdapter;
pub use nvme_cache::NvmeCacheAdapter;
pub use tauri_event_bus::TauriEventBus;
pub use ffmpeg_media::FfmpegMediaAdapter;
pub use fsxn_storage::FsxOntapAdapter;
pub use gcs_storage::GcsStorageAdapter;
pub use azure_blob_storage::AzureBlobStorageAdapter;
pub use oracle_object_storage::OracleObjectStorageAdapter;
pub use nas_storage::{NasStorageAdapter, NasProtocol};
pub use clipboard::ClipboardAdapter;
pub use metadata_store::JsonMetadataStore;
pub use factories::{
    StorageAdapterFactoryImpl,
    CacheAdapterFactoryImpl,
    ClipboardAdapterFactoryImpl,
};
pub use native_thumbnail::{NativeThumbnailAdapter, ThumbnailType};
pub use settings::{FileSettingsStore, SettingsManager};

