//! Virtual File System Implementation
//!
//! Clean Architecture structure following Ports & Adapters pattern:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      VFS Module                              │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐        │
//! │  │   Domain    │   │    Ports    │   │  Adapters   │        │
//! │  │  entities   │   │  (traits)   │   │ (concrete)  │        │
//! │  │  values     │   │ IStorage    │   │ S3Adapter   │        │
//! │  │  events     │   │ ICache      │   │ LocalAdapter│        │
//! │  └─────────────┘   └─────────────┘   └─────────────┘        │
//! │           │               ▲                 │                │
//! │           └───────────────┼─────────────────┘                │
//! │                           │                                  │
//! │                  ┌────────┴────────┐                         │
//! │                  │   Application   │                         │
//! │                  │   (use cases)   │                         │
//! │                  └─────────────────┘                         │
//! └─────────────────────────────────────────────────────────────┘
//! ```

// Domain Layer - Core business entities and value objects
pub mod domain;

// Ports - Abstract interfaces (traits) defining contracts
pub mod ports;

// Adapters - Concrete implementations of ports
pub mod adapters;

// Application Layer - Business logic orchestration
pub mod application;

// Use Cases - Business operations (Clean Architecture)
pub mod use_cases;

// Infrastructure - FUSE filesystem, commands
pub mod infrastructure;

// Platform-specific utilities (cross-platform support)
pub mod platform;

// Re-exports for convenience
pub use domain::*;
pub use ports::StorageAdapter;
pub use application::VfsService;

// Commands module - refactored into sub-modules
pub mod commands;
pub mod types;

// Feature tests - one clear test per use case
#[cfg(test)]
mod tests;

// Conditionally include FUSE-dependent modules
#[cfg(all(feature = "vfs", feature = "mount"))]
pub mod filesystem;
#[cfg(all(feature = "vfs", feature = "mount"))]
pub mod hydration;
#[cfg(all(feature = "vfs", feature = "mount"))]
pub mod mount;

// Multipart upload support
pub mod multipart_upload;
pub mod download_manager;

// Operation tracker (uploads, downloads, deletes, etc.)
pub mod operation_tracker;

// Audit log (persistent audit trail for user and organization operations)
pub mod audit_log;

// Operation tracking helper (utilities for tracking operations with full metadata)
pub mod operation_tracking;

// Tier sync providers (AWS DataSync, etc.)
pub mod providers;

#[cfg(all(feature = "vfs", feature = "mount"))]
pub use filesystem::DiasporFS;
#[cfg(all(feature = "vfs", feature = "mount"))]
pub use hydration::HydratedOperator;
#[cfg(all(feature = "vfs", feature = "mount"))]
pub use mount::{mount_virtual_drive, unmount_virtual_drive};
