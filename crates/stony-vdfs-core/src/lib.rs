//! # stony-vdfs-core
//!
//! Core traits and types for the Stony VDFS (Virtual Distributed File System) library.
//!
//! This crate defines the backend-agnostic abstractions used by every other crate in the
//! workspace: paths, metadata, errors, and the async traits that backends implement.
//!
//! ## Design goals
//!
//! - **Backend-agnostic**: a single [`VfsBackend`] trait supports memory, local, and future
//!   cloud backends without leaking platform details to consumers.
//! - **Async-first**: every IO operation is async, designed for the `tokio` runtime.
//! - **Privacy-by-default**: no telemetry, no implicit cloud calls; backends are explicit.
//! - **Cross-platform paths**: path manipulation that behaves the same on Linux, macOS and
//!   Windows, while still bridging cleanly to native filesystem APIs at mount points.
//!
//! ## Example
//!
//! ```ignore
//! use stony_vdfs_core::{VfsBackend, VfsPath};
//!
//! async fn list_root(backend: &dyn VfsBackend) -> stony_vdfs_core::Result<Vec<VfsPath>> {
//!     backend.list(&VfsPath::root()).await
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/stony-vdfs-core/0.1.0-alpha.1")]

pub mod error;
pub mod path;
pub mod traits;
pub mod types;

pub use error::{Result, VfsError};
pub use path::VfsPath;
pub use traits::{VfsBackend, VfsHandle, VfsNode};
pub use types::{NodeKind, OpenFlags, VfsMetadata};

#[cfg(test)]
mod tests;
