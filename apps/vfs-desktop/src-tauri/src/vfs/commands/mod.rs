//! VFS Tauri Commands
//!
//! These commands expose VFS functionality to the frontend.
//! Named with vfs_ prefix for consistent API naming.
//!
//! This module is organized into sub-modules by feature:
//! - `state` - State management
//! - `responses` - Response DTOs
//! - `init` - Initialization commands
//! - `storage` - Storage source management
//! - `files` - File operations
//! - `clipboard` - Clipboard operations
//! - `cache` - Cache operations
//! - `metadata` - Tags, favorites, metadata
//! - `uploads` - Upload/download operations
//! - `transcription` - Transcription operations

pub mod state;
pub mod responses;

// Re-export commonly used types
pub use state::VfsStateWrapper;
pub use responses::*;

// Import all command modules
mod init;
mod storage;
mod files;
mod clipboard;
mod cache;
mod metadata;
mod uploads;
mod transcription;
mod cross_storage;
mod sync;
mod file_ops;
mod models;
pub mod helpers;
mod setup;
mod auto_ops;
mod stream;

// Re-export all commands for Tauri registration
pub use init::*;
pub use storage::*;
pub use files::*;
pub use clipboard::*;
pub use cache::*;
pub use metadata::*;
pub use uploads::*;
pub use transcription::*;
pub use cross_storage::*;
pub use sync::*;
pub use file_ops::*;
pub use models::*;
pub use helpers::*;
pub use setup::*;
pub use auto_ops::*;
pub use stream::*;