//! Error types and the crate-wide [`Result`] alias.

use thiserror::Error;

/// The crate-wide result alias.
pub type Result<T> = std::result::Result<T, VfsError>;

/// Errors returned by any [`crate::VfsBackend`] implementation.
///
/// Each variant maps cleanly to a `std::io::ErrorKind` for callers that need to bridge to
/// platform IO APIs, but carries richer context (the offending [`crate::VfsPath`], for
/// example) than `std::io::Error` would.
#[derive(Debug, Error)]
pub enum VfsError {
    /// The requested node does not exist.
    #[error("not found: {path}")]
    NotFound {
        /// Path that was requested.
        path: String,
    },

    /// A node with the same path already exists.
    #[error("already exists: {path}")]
    AlreadyExists {
        /// Path that conflicts.
        path: String,
    },

    /// The caller does not have permission to perform the operation.
    #[error("permission denied: {path}")]
    PermissionDenied {
        /// Path that was denied.
        path: String,
    },

    /// The path is syntactically invalid (empty, contains a NUL byte, etc.).
    #[error("invalid path: {path}")]
    InvalidPath {
        /// Offending path string.
        path: String,
    },

    /// Attempted to perform a file operation on a directory or vice-versa.
    #[error("kind mismatch at {path}: expected {expected}, found {found}")]
    KindMismatch {
        /// Path that was operated on.
        path: String,
        /// What the caller expected.
        expected: &'static str,
        /// What was actually found.
        found: &'static str,
    },

    /// The backend does not support this operation.
    #[error("unsupported by backend: {operation}")]
    Unsupported {
        /// Human-readable name of the unsupported operation.
        operation: &'static str,
    },

    /// Wraps a `std::io::Error` from an underlying backend (e.g. local filesystem).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Any other backend-specific error.
    #[error("backend error: {0}")]
    Backend(String),
}

impl VfsError {
    /// Convenience constructor for [`VfsError::NotFound`].
    #[must_use]
    pub fn not_found(path: impl Into<String>) -> Self {
        Self::NotFound { path: path.into() }
    }

    /// Convenience constructor for [`VfsError::AlreadyExists`].
    #[must_use]
    pub fn already_exists(path: impl Into<String>) -> Self {
        Self::AlreadyExists { path: path.into() }
    }

    /// Convenience constructor for [`VfsError::PermissionDenied`].
    #[must_use]
    pub fn permission_denied(path: impl Into<String>) -> Self {
        Self::PermissionDenied { path: path.into() }
    }

    /// Convenience constructor for [`VfsError::InvalidPath`].
    #[must_use]
    pub fn invalid_path(path: impl Into<String>) -> Self {
        Self::InvalidPath { path: path.into() }
    }

    /// Convenience constructor for [`VfsError::Backend`].
    #[must_use]
    pub fn backend(msg: impl Into<String>) -> Self {
        Self::Backend(msg.into())
    }
}
