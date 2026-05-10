//! # stony-vdfs-backend-memory
//!
//! A simple in-memory implementation of [`stony_vdfs_core::VfsBackend`]. Useful for tests,
//! demos, and as a reference implementation of the trait surface.
//!
//! ```no_run
//! use stony_vdfs_backend_memory::MemoryBackend;
//! use stony_vdfs_core::{OpenFlags, VfsBackend, VfsPath};
//!
//! # async fn run() -> stony_vdfs_core::Result<()> {
//! let backend = MemoryBackend::new();
//! let hello = VfsPath::new("/hello.txt").unwrap();
//! let mut h = backend.open(&hello, OpenFlags::CREATE | OpenFlags::WRITE).await?;
//! h.write(0, b"world").await?;
//! # Ok(())
//! # }
//! ```

#![doc(html_root_url = "https://docs.rs/stony-vdfs-backend-memory/0.1.0-alpha.1")]

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;
use stony_vdfs_core::{
    NodeKind, OpenFlags, Result, VfsBackend, VfsError, VfsHandle, VfsMetadata, VfsPath,
};
use time::OffsetDateTime;

#[derive(Debug)]
struct Node {
    kind: NodeKind,
    data: Vec<u8>,
    modified: OffsetDateTime,
    created: OffsetDateTime,
}

impl Node {
    fn file() -> Self {
        let now = OffsetDateTime::now_utc();
        Self { kind: NodeKind::File, data: Vec::new(), modified: now, created: now }
    }

    fn directory() -> Self {
        let now = OffsetDateTime::now_utc();
        Self { kind: NodeKind::Directory, data: Vec::new(), modified: now, created: now }
    }

    fn metadata(&self) -> VfsMetadata {
        VfsMetadata {
            kind: self.kind,
            size: self.data.len() as u64,
            modified: Some(self.modified),
            created: Some(self.created),
            read_only: false,
        }
    }
}

/// An in-memory `VfsBackend` suitable for tests and small demos.
#[derive(Clone, Default, Debug)]
pub struct MemoryBackend {
    nodes: Arc<RwLock<BTreeMap<String, Node>>>,
}

impl MemoryBackend {
    /// Creates a new, empty in-memory backend with a single root directory.
    #[must_use]
    pub fn new() -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(String::from("/"), Node::directory());
        Self { nodes: Arc::new(RwLock::new(nodes)) }
    }
}

#[async_trait]
impl VfsBackend for MemoryBackend {
    fn name(&self) -> &'static str {
        "memory"
    }

    async fn metadata(&self, path: &VfsPath) -> Result<VfsMetadata> {
        let guard = self.nodes.read();
        guard
            .get(path.as_str())
            .map(Node::metadata)
            .ok_or_else(|| VfsError::not_found(path.as_str()))
    }

    async fn list(&self, path: &VfsPath) -> Result<Vec<VfsPath>> {
        let guard = self.nodes.read();
        let node = guard
            .get(path.as_str())
            .ok_or_else(|| VfsError::not_found(path.as_str()))?;
        if node.kind != NodeKind::Directory {
            return Err(VfsError::KindMismatch {
                path: path.as_str().to_string(),
                expected: "directory",
                found: node.kind.as_str(),
            });
        }
        let prefix = if path.is_root() { String::from("/") } else { format!("{}/", path.as_str()) };
        let children: Vec<VfsPath> = guard
            .keys()
            .filter(|k| {
                k.as_str() != "/" && k.starts_with(&prefix) && {
                    let tail = &k[prefix.len()..];
                    !tail.is_empty() && !tail.contains('/')
                }
            })
            .filter_map(|k| VfsPath::new(k))
            .collect();
        Ok(children)
    }

    async fn open(&self, path: &VfsPath, flags: OpenFlags) -> Result<Box<dyn VfsHandle>> {
        let mut guard = self.nodes.write();
        let exists = guard.contains_key(path.as_str());

        if !exists {
            if !flags.contains(OpenFlags::CREATE) {
                return Err(VfsError::not_found(path.as_str()));
            }
            guard.insert(path.as_str().to_string(), Node::file());
        } else if flags.contains(OpenFlags::CREATE) && flags.contains(OpenFlags::EXCL) {
            return Err(VfsError::already_exists(path.as_str()));
        }

        if flags.contains(OpenFlags::TRUNC)
            && let Some(n) = guard.get_mut(path.as_str())
        {
            n.data.clear();
        }

        Ok(Box::new(MemoryHandle {
            path: path.clone(),
            backend: self.clone(),
        }))
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<()> {
        let parent = path.parent().ok_or_else(|| VfsError::invalid_path(path.as_str()))?;
        let mut guard = self.nodes.write();
        if !guard.contains_key(parent.as_str()) {
            return Err(VfsError::not_found(parent.as_str()));
        }
        if guard.contains_key(path.as_str()) {
            return Err(VfsError::already_exists(path.as_str()));
        }
        guard.insert(path.as_str().to_string(), Node::directory());
        Ok(())
    }

    async fn remove(&self, path: &VfsPath) -> Result<()> {
        let mut guard = self.nodes.write();
        let node = guard
            .get(path.as_str())
            .ok_or_else(|| VfsError::not_found(path.as_str()))?;
        if node.kind == NodeKind::Directory {
            let prefix = if path.is_root() { String::from("/") } else { format!("{}/", path.as_str()) };
            if guard.keys().any(|k| k.starts_with(&prefix) && k.as_str() != path.as_str()) {
                return Err(VfsError::backend("directory not empty"));
            }
        }
        guard.remove(path.as_str());
        Ok(())
    }
}

/// Handle returned by [`MemoryBackend::open`]. Holds a clone of the backend's `Arc` so the
/// underlying storage stays alive while the handle is open.
struct MemoryHandle {
    path: VfsPath,
    backend: MemoryBackend,
}

#[async_trait]
impl VfsHandle for MemoryHandle {
    async fn read(&mut self, offset: u64, len: usize) -> Result<Bytes> {
        let guard = self.backend.nodes.read();
        let node = guard
            .get(self.path.as_str())
            .ok_or_else(|| VfsError::not_found(self.path.as_str()))?;
        let start = usize::try_from(offset).unwrap_or(usize::MAX);
        if start >= node.data.len() {
            return Ok(Bytes::new());
        }
        let end = start.saturating_add(len).min(node.data.len());
        Ok(Bytes::copy_from_slice(&node.data[start..end]))
    }

    async fn write(&mut self, offset: u64, data: &[u8]) -> Result<usize> {
        let mut guard = self.backend.nodes.write();
        let node = guard
            .get_mut(self.path.as_str())
            .ok_or_else(|| VfsError::not_found(self.path.as_str()))?;
        let start = usize::try_from(offset).unwrap_or(usize::MAX);
        let needed = start.saturating_add(data.len());
        if needed > node.data.len() {
            node.data.resize(needed, 0);
        }
        node.data[start..start + data.len()].copy_from_slice(data);
        node.modified = OffsetDateTime::now_utc();
        Ok(data.len())
    }

    async fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_and_read_a_file() {
        let backend = MemoryBackend::new();
        let path = VfsPath::new("/hello.txt").unwrap();
        let mut h = backend
            .open(&path, OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        let written = h.write(0, b"world").await.unwrap();
        assert_eq!(written, 5);

        let meta = backend.metadata(&path).await.unwrap();
        assert_eq!(meta.kind, NodeKind::File);
        assert_eq!(meta.size, 5);

        let mut h2 = backend.open(&path, OpenFlags::READ).await.unwrap();
        let read = h2.read(0, 1024).await.unwrap();
        assert_eq!(&read[..], b"world");
    }

    #[tokio::test]
    async fn list_directory_children() {
        let backend = MemoryBackend::new();
        let dir = VfsPath::new("/d").unwrap();
        backend.create_dir(&dir).await.unwrap();
        let f = VfsPath::new("/d/a.txt").unwrap();
        backend.open(&f, OpenFlags::CREATE | OpenFlags::WRITE).await.unwrap();
        let children = backend.list(&dir).await.unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].as_str(), "/d/a.txt");
    }

    #[tokio::test]
    async fn rm_empty_then_nonempty_dir() {
        let backend = MemoryBackend::new();
        let dir = VfsPath::new("/d").unwrap();
        backend.create_dir(&dir).await.unwrap();
        backend.remove(&dir).await.unwrap();

        backend.create_dir(&dir).await.unwrap();
        let f = VfsPath::new("/d/a.txt").unwrap();
        backend.open(&f, OpenFlags::CREATE | OpenFlags::WRITE).await.unwrap();
        assert!(backend.remove(&dir).await.is_err());
    }
}
