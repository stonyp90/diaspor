//! Shared types: node kinds, open flags, metadata.

use bitflags::bitflags;
use time::OffsetDateTime;

/// What kind of filesystem node this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// A symbolic link.
    Symlink,
}

impl NodeKind {
    /// Returns a static string label used in error messages.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::Symlink => "symlink",
        }
    }
}

bitflags! {
    /// Flags passed to [`crate::VfsBackend::open`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct OpenFlags: u32 {
        /// Open for reading.
        const READ   = 0b0000_0001;
        /// Open for writing.
        const WRITE  = 0b0000_0010;
        /// Create the file if it does not exist.
        const CREATE = 0b0000_0100;
        /// Fail if the file already exists when used with [`Self::CREATE`].
        const EXCL   = 0b0000_1000;
        /// Truncate the file to zero length on open.
        const TRUNC  = 0b0001_0000;
        /// Append-only mode.
        const APPEND = 0b0010_0000;
    }
}

/// Metadata returned by [`crate::VfsBackend::metadata`].
#[derive(Debug, Clone)]
pub struct VfsMetadata {
    /// What kind of node this is.
    pub kind: NodeKind,
    /// Size in bytes (0 for directories).
    pub size: u64,
    /// Time the node was last modified.
    pub modified: Option<OffsetDateTime>,
    /// Time the node was created.
    pub created: Option<OffsetDateTime>,
    /// Whether the node is read-only.
    pub read_only: bool,
}

impl VfsMetadata {
    /// Convenience constructor for a regular file with the given size.
    #[must_use]
    pub const fn file(size: u64) -> Self {
        Self {
            kind: NodeKind::File,
            size,
            modified: None,
            created: None,
            read_only: false,
        }
    }

    /// Convenience constructor for a directory.
    #[must_use]
    pub const fn directory() -> Self {
        Self {
            kind: NodeKind::Directory,
            size: 0,
            modified: None,
            created: None,
            read_only: false,
        }
    }
}
