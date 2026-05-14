//! # diaspor-fuse
//!
//! FUSE mount adapter for [`diaspor_core::VfsBackend`] implementations on Linux and
//! macOS.
//!
//! This crate is a **stub** in the v0.1.0-alpha line. The full FUSE integration arrives in
//! milestone M3 of the roadmap; until then, callers should depend on
//! `diaspor-backend-memory` or `diaspor-backend-local` directly.

#![doc(html_root_url = "https://docs.rs/diaspor-fuse/0.1.0-alpha.1")]

use std::path::Path;
use std::sync::Arc;

use diaspor_core::{Result, VfsBackend, VfsError};

/// A FUSE mount point. Returned by [`mount`].
///
/// Dropping this value unmounts the filesystem.
pub struct FuseMount {
    _backend: Arc<dyn VfsBackend>,
}

/// Mounts `backend` at the given host path using FUSE.
///
/// Currently returns [`VfsError::Unsupported`] — full implementation lands in roadmap
/// milestone M3.
///
/// # Errors
///
/// Always returns [`VfsError::Unsupported`] in v0.1.0-alpha.
pub fn mount(_backend: Arc<dyn VfsBackend>, _mount_point: &Path) -> Result<FuseMount> {
    Err(VfsError::Unsupported {
        operation: "fuse::mount (planned for M3)",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use diaspor_core::{OpenFlags, VfsHandle, VfsMetadata, VfsPath};

    struct Stub;

    #[async_trait::async_trait]
    impl VfsBackend for Stub {
        fn name(&self) -> &'static str {
            "stub"
        }
        async fn metadata(&self, _: &VfsPath) -> Result<VfsMetadata> {
            unimplemented!()
        }
        async fn list(&self, _: &VfsPath) -> Result<Vec<VfsPath>> {
            unimplemented!()
        }
        async fn open(&self, _: &VfsPath, _: OpenFlags) -> Result<Box<dyn VfsHandle>> {
            unimplemented!()
        }
        async fn create_dir(&self, _: &VfsPath) -> Result<()> {
            unimplemented!()
        }
        async fn remove(&self, _: &VfsPath) -> Result<()> {
            unimplemented!()
        }
    }

    #[test]
    fn mount_currently_unsupported() {
        let backend: Arc<dyn VfsBackend> = Arc::new(Stub);
        let result = mount(backend, &PathBuf::from("/tmp/x"));
        assert!(matches!(result, Err(VfsError::Unsupported { .. })));
    }
}
