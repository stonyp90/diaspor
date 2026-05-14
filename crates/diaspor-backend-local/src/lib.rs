//! # diaspor-backend-local
//!
//! A backend that maps a [`VfsPath`] onto a real directory on the host filesystem. Every
//! VFS path is interpreted relative to a configured root directory; the backend prevents
//! escapes via `..` segments.
//!
//! This is a starter implementation: it covers `metadata`, `list`, `open`, `create_dir`
//! and `remove` for the happy path. Edge cases (symlinks, holes, extended attributes)
//! will be filled in across milestones M2 and M3.

#![doc(html_root_url = "https://docs.rs/diaspor-backend-local/0.1.0-alpha.1")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use diaspor_core::{
    NodeKind, OpenFlags, Result, VfsBackend, VfsError, VfsHandle, VfsMetadata, VfsPath,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;

/// A backend rooted at a host-filesystem directory.
#[derive(Clone, Debug)]
pub struct LocalBackend {
    root: Arc<PathBuf>,
}

impl LocalBackend {
    /// Creates a backend whose root is the given host-filesystem directory.
    ///
    /// The directory must exist; the backend does not create it.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::InvalidPath`] if `root` does not exist or is not a directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(VfsError::invalid_path(root.display().to_string()));
        }
        Ok(Self {
            root: Arc::new(root.to_path_buf()),
        })
    }

    /// Translates a [`VfsPath`] into a host path under [`Self::root`].
    ///
    /// Refuses any path whose normalized segments include `..`.
    fn host_path(&self, vfs_path: &VfsPath) -> Result<PathBuf> {
        let trimmed = vfs_path.as_str().trim_start_matches('/');
        if trimmed.split('/').any(|seg| seg == ".." || seg == ".") {
            return Err(VfsError::invalid_path(vfs_path.as_str()));
        }
        Ok(self.root.join(trimmed))
    }
}

#[async_trait]
impl VfsBackend for LocalBackend {
    fn name(&self) -> &'static str {
        "local"
    }

    async fn metadata(&self, path: &VfsPath) -> Result<VfsMetadata> {
        let host = self.host_path(path)?;
        let meta = tokio::fs::metadata(&host).await?;
        let kind = if meta.is_dir() {
            NodeKind::Directory
        } else if meta.file_type().is_symlink() {
            NodeKind::Symlink
        } else {
            NodeKind::File
        };
        Ok(VfsMetadata {
            kind,
            size: meta.len(),
            modified: meta.modified().ok().map(Into::into),
            created: meta.created().ok().map(Into::into),
            read_only: meta.permissions().readonly(),
        })
    }

    async fn list(&self, path: &VfsPath) -> Result<Vec<VfsPath>> {
        let host = self.host_path(path)?;
        let mut entries = tokio::fs::read_dir(&host).await?;
        let mut out = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            out.push(path.join(name.as_ref()));
        }
        Ok(out)
    }

    async fn open(&self, path: &VfsPath, flags: OpenFlags) -> Result<Box<dyn VfsHandle>> {
        let host = self.host_path(path)?;
        let mut opts = tokio::fs::OpenOptions::new();
        opts.read(flags.contains(OpenFlags::READ))
            .write(flags.contains(OpenFlags::WRITE))
            .create(flags.contains(OpenFlags::CREATE))
            .create_new(flags.contains(OpenFlags::CREATE | OpenFlags::EXCL))
            .truncate(flags.contains(OpenFlags::TRUNC))
            .append(flags.contains(OpenFlags::APPEND));
        let file = opts.open(&host).await?;
        Ok(Box::new(LocalHandle {
            file: Mutex::new(file),
        }))
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<()> {
        let host = self.host_path(path)?;
        tokio::fs::create_dir(&host).await?;
        Ok(())
    }

    async fn remove(&self, path: &VfsPath) -> Result<()> {
        let host = self.host_path(path)?;
        let meta = tokio::fs::metadata(&host).await?;
        if meta.is_dir() {
            tokio::fs::remove_dir(&host).await?;
        } else {
            tokio::fs::remove_file(&host).await?;
        }
        Ok(())
    }
}

struct LocalHandle {
    file: Mutex<tokio::fs::File>,
}

// The handle owns a `Mutex<File>`. Each method body is a single lock-and-operate
// sequence — the guard naturally drops at the end of the method, which is what we
// want. Clippy's `significant_drop_tightening` lint suggests inserting an explicit
// `drop(file)` immediately before the trailing expression, but that doesn't change
// behaviour here (no other work happens after the IO) and only adds noise.
#[allow(
    clippy::significant_drop_tightening,
    reason = "lock held for the entire method body by design"
)]
#[async_trait]
impl VfsHandle for LocalHandle {
    async fn read(&mut self, offset: u64, len: usize) -> Result<Bytes> {
        let mut file = self.file.lock().await;
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        let mut buf = vec![0u8; len];
        let n = file.read(&mut buf).await?;
        buf.truncate(n);
        Ok(Bytes::from(buf))
    }

    async fn write(&mut self, offset: u64, data: &[u8]) -> Result<usize> {
        let mut file = self.file.lock().await;
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        file.write_all(data).await?;
        Ok(data.len())
    }

    async fn flush(&mut self) -> Result<()> {
        let mut file = self.file.lock().await;
        file.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn roundtrip_a_file() {
        let dir = tempdir().unwrap();
        let backend = LocalBackend::new(dir.path()).unwrap();
        let path = VfsPath::new("/hello.txt").unwrap();
        let mut h = backend
            .open(&path, OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        h.write(0, b"world").await.unwrap();
        h.flush().await.unwrap();
        drop(h);

        let meta = backend.metadata(&path).await.unwrap();
        assert_eq!(meta.kind, NodeKind::File);
        assert_eq!(meta.size, 5);
    }

    #[tokio::test]
    async fn rejects_dot_dot() {
        let dir = tempdir().unwrap();
        let backend = LocalBackend::new(dir.path()).unwrap();
        let bad = VfsPath::new("/../escape").unwrap();
        let result = backend.metadata(&bad).await;
        assert!(matches!(result, Err(VfsError::InvalidPath { .. })));
    }
}
