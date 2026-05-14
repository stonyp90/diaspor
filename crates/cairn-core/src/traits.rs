//! The async traits every backend implements.

use async_trait::async_trait;
use bytes::Bytes;

use crate::{OpenFlags, Result, VfsMetadata, VfsPath};

/// A backend that stores a virtual filesystem.
///
/// Implementations exist in sibling crates: `cairn-backend-memory`,
/// `cairn-backend-local`, and (eventually) cloud-storage backends.
///
/// Backends are expected to be cheap to clone (typically wrapping an `Arc`) so they can be
/// shared across mount points and async tasks.
#[async_trait]
pub trait VfsBackend: Send + Sync + 'static {
    /// A human-readable name for the backend ("memory", "local", …). Used in logs.
    fn name(&self) -> &'static str;

    /// Returns metadata for the node at `path`.
    async fn metadata(&self, path: &VfsPath) -> Result<VfsMetadata>;

    /// Lists the children of a directory at `path`.
    async fn list(&self, path: &VfsPath) -> Result<Vec<VfsPath>>;

    /// Opens a handle on the node at `path` with the given [`OpenFlags`].
    async fn open(&self, path: &VfsPath, flags: OpenFlags) -> Result<Box<dyn VfsHandle>>;

    /// Creates a directory at `path`. Parents must already exist (no `mkdir -p`).
    async fn create_dir(&self, path: &VfsPath) -> Result<()>;

    /// Removes the node at `path`. For directories, behaviour matches POSIX `rmdir`
    /// (must be empty).
    async fn remove(&self, path: &VfsPath) -> Result<()>;
}

/// An open node — typically a file, occasionally a directory listing handle.
#[async_trait]
pub trait VfsHandle: Send + Sync {
    /// Read up to `len` bytes starting at byte offset `offset`.
    async fn read(&mut self, offset: u64, len: usize) -> Result<Bytes>;

    /// Write `data` at byte offset `offset`. Returns the number of bytes written.
    async fn write(&mut self, offset: u64, data: &[u8]) -> Result<usize>;

    /// Flush any buffered data to the backend.
    async fn flush(&mut self) -> Result<()>;
}

/// A snapshot view of a node, used by tools that don't need a live handle.
///
/// Reserved for future use (content-addressable storage, dedup, etc.).
pub trait VfsNode: Send + Sync {
    /// Path to this node inside the backend.
    fn path(&self) -> &VfsPath;
    /// Cached metadata for this node.
    fn metadata(&self) -> &VfsMetadata;
}
