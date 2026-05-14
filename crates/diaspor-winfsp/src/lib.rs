//! # diaspor-winfsp
//!
//! `WinFsp` mount adapter for [`diaspor_core::VfsBackend`] implementations on Windows.
//!
//! This crate is a **stub** in the v0.1.0-alpha line. The full `WinFsp` integration arrives
//! in roadmap milestone M4; until then, callers should depend on
//! `diaspor-backend-memory` or `diaspor-backend-local` directly.

#![doc(html_root_url = "https://docs.rs/diaspor-winfsp/0.1.0-alpha.1")]

use std::path::Path;
use std::sync::Arc;

use diaspor_core::{Result, VfsBackend, VfsError};

/// A `WinFsp` mount point. Returned by [`mount`].
///
/// Dropping this value unmounts the filesystem.
pub struct WinFspMount {
    _backend: Arc<dyn VfsBackend>,
}

/// Mounts `backend` at the given host path using `WinFsp`.
///
/// Currently returns [`VfsError::Unsupported`] — full implementation lands in roadmap
/// milestone M4.
///
/// # Errors
///
/// Always returns [`VfsError::Unsupported`] in v0.1.0-alpha.
pub fn mount(_backend: Arc<dyn VfsBackend>, _mount_point: &Path) -> Result<WinFspMount> {
    Err(VfsError::Unsupported {
        operation: "winfsp::mount (planned for M4)",
    })
}
