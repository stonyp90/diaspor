//! # cairn-backend-memory
//!
//! A simple in-memory implementation of [`cairn_core::VfsBackend`]. Useful for tests,
//! demos, and as a reference implementation of the trait surface.
//!
//! ```no_run
//! use cairn_backend_memory::MemoryBackend;
//! use cairn_core::{OpenFlags, VfsBackend, VfsPath};
//!
//! # async fn run() -> cairn_core::Result<()> {
//! let backend = MemoryBackend::new();
//! let hello = VfsPath::new("/hello.txt").unwrap();
//! let mut h = backend.open(&hello, OpenFlags::CREATE | OpenFlags::WRITE).await?;
//! h.write(0, b"world").await?;
//! # Ok(())
//! # }
//! ```

#![doc(html_root_url = "https://docs.rs/cairn-backend-memory/0.1.0-alpha.1")]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;
use cairn_core::{
    NodeKind, OpenFlags, Result, VfsBackend, VfsError, VfsHandle, VfsMetadata, VfsPath,
};
use time::OffsetDateTime;

/// In-memory representation of a single filesystem node (file or directory).
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
        Self {
            kind: NodeKind::File,
            data: Vec::new(),
            modified: now,
            created: now,
        }
    }

    fn directory() -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            kind: NodeKind::Directory,
            data: Vec::new(),
            modified: now,
            created: now,
        }
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

/// An in-memory [`VfsBackend`] suitable for tests and small demos.
///
/// Internally the tree is an `Arc<RwLock<HashMap<VfsPath, Node>>>`. We use
/// `parking_lot::RwLock` rather than `tokio::sync::RwLock` because:
///
/// 1. Every critical section is tiny — a `HashMap` lookup plus a `Vec` mutation — so the
///    extra overhead of `tokio::sync::RwLock`'s async coordination is pure cost.
/// 2. No await point is held across the lock; the guard is dropped before any caller can
///    suspend, so we never block the runtime by holding it.
/// 3. `parking_lot` is also faster and smaller than `std::sync::RwLock`, which matters when
///    a test suite hammers the backend with thousands of operations.
#[derive(Clone, Default, Debug)]
pub struct MemoryBackend {
    nodes: Arc<RwLock<HashMap<VfsPath, Node>>>,
}

impl MemoryBackend {
    /// Creates a new, empty in-memory backend with a single root directory.
    #[must_use]
    pub fn new() -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(VfsPath::root(), Node::directory());
        Self {
            nodes: Arc::new(RwLock::new(nodes)),
        }
    }
}

// Every method in this impl follows the pattern `acquire RwLock guard, do the work,
// return`. The guard is held for the full body by design — there's no later work
// that would benefit from an early drop. We silence `significant_drop_tightening`
// at the impl level so the bodies stay readable.
#[allow(
    clippy::significant_drop_tightening,
    reason = "guard held for the whole method body by design"
)]
#[async_trait]
impl VfsBackend for MemoryBackend {
    fn name(&self) -> &'static str {
        "memory"
    }

    async fn metadata(&self, path: &VfsPath) -> Result<VfsMetadata> {
        let guard = self.nodes.read();
        guard
            .get(path)
            .map(Node::metadata)
            .ok_or_else(|| VfsError::not_found(path.as_str()))
    }

    async fn list(&self, path: &VfsPath) -> Result<Vec<VfsPath>> {
        let guard = self.nodes.read();
        let node = guard
            .get(path)
            .ok_or_else(|| VfsError::not_found(path.as_str()))?;
        if node.kind != NodeKind::Directory {
            return Err(VfsError::KindMismatch {
                path: path.as_str().to_string(),
                expected: "directory",
                found: node.kind.as_str(),
            });
        }
        // Direct-child prefix: "/" for root, "/dir/" for any other directory.
        let prefix = if path.is_root() {
            String::from("/")
        } else {
            format!("{}/", path.as_str())
        };
        let children: Vec<VfsPath> = guard
            .keys()
            .filter(|k| {
                let s = k.as_str();
                if s == "/" {
                    return false;
                }
                if !s.starts_with(&prefix) {
                    return false;
                }
                let tail = &s[prefix.len()..];
                !tail.is_empty() && !tail.contains('/')
            })
            .cloned()
            .collect();
        Ok(children)
    }

    async fn open(&self, path: &VfsPath, flags: OpenFlags) -> Result<Box<dyn VfsHandle>> {
        if path.is_root() {
            return Err(VfsError::KindMismatch {
                path: path.as_str().to_string(),
                expected: "file",
                found: "directory",
            });
        }
        let mut guard = self.nodes.write();
        let exists = guard.contains_key(path);

        if exists {
            // Existing node: reject if EXCL set, reject if it's a directory.
            if flags.contains(OpenFlags::CREATE) && flags.contains(OpenFlags::EXCL) {
                return Err(VfsError::already_exists(path.as_str()));
            }
            if let Some(n) = guard.get(path)
                && n.kind != NodeKind::File
            {
                return Err(VfsError::KindMismatch {
                    path: path.as_str().to_string(),
                    expected: "file",
                    found: n.kind.as_str(),
                });
            }
        } else {
            if !flags.contains(OpenFlags::CREATE) {
                return Err(VfsError::not_found(path.as_str()));
            }
            // The parent must exist and be a directory. No `mkdir -p`.
            let parent = path
                .parent()
                .ok_or_else(|| VfsError::invalid_path(path.as_str()))?;
            match guard.get(&parent) {
                None => return Err(VfsError::not_found(parent.as_str())),
                Some(p) if p.kind != NodeKind::Directory => {
                    return Err(VfsError::KindMismatch {
                        path: parent.as_str().to_string(),
                        expected: "directory",
                        found: p.kind.as_str(),
                    });
                }
                Some(_) => {}
            }
            guard.insert(path.clone(), Node::file());
        }

        if flags.contains(OpenFlags::TRUNC)
            && let Some(n) = guard.get_mut(path)
        {
            n.data.clear();
            n.modified = OffsetDateTime::now_utc();
        }

        Ok(Box::new(MemoryHandle {
            path: path.clone(),
            backend: self.clone(),
        }))
    }

    async fn create_dir(&self, path: &VfsPath) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| VfsError::invalid_path(path.as_str()))?;
        let mut guard = self.nodes.write();
        match guard.get(&parent) {
            None => return Err(VfsError::not_found(parent.as_str())),
            Some(p) if p.kind != NodeKind::Directory => {
                return Err(VfsError::KindMismatch {
                    path: parent.as_str().to_string(),
                    expected: "directory",
                    found: p.kind.as_str(),
                });
            }
            Some(_) => {}
        }
        if guard.contains_key(path) {
            return Err(VfsError::already_exists(path.as_str()));
        }
        guard.insert(path.clone(), Node::directory());
        Ok(())
    }

    async fn remove(&self, path: &VfsPath) -> Result<()> {
        if path.is_root() {
            return Err(VfsError::permission_denied(path.as_str()));
        }
        let mut guard = self.nodes.write();
        let node = guard
            .get(path)
            .ok_or_else(|| VfsError::not_found(path.as_str()))?;
        if node.kind == NodeKind::Directory {
            let prefix = format!("{}/", path.as_str());
            if guard.keys().any(|k| k.as_str().starts_with(&prefix)) {
                return Err(VfsError::backend("directory not empty"));
            }
        }
        guard.remove(path);
        Ok(())
    }
}

/// Handle returned by [`MemoryBackend::open`].
///
/// Holds a clone of the backend's `Arc` so the underlying storage stays alive while the
/// handle is open. Each read or write reacquires the shared `RwLock` so concurrent writers
/// can mutate the same file safely.
struct MemoryHandle {
    path: VfsPath,
    backend: MemoryBackend,
}

#[allow(
    clippy::significant_drop_tightening,
    reason = "guard held for the whole method body by design"
)]
#[async_trait]
impl VfsHandle for MemoryHandle {
    async fn read(&mut self, offset: u64, len: usize) -> Result<Bytes> {
        let guard = self.backend.nodes.read();
        let node = guard
            .get(&self.path)
            .ok_or_else(|| VfsError::not_found(self.path.as_str()))?;
        if node.kind != NodeKind::File {
            return Err(VfsError::KindMismatch {
                path: self.path.as_str().to_string(),
                expected: "file",
                found: node.kind.as_str(),
            });
        }
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
            .get_mut(&self.path)
            .ok_or_else(|| VfsError::not_found(self.path.as_str()))?;
        if node.kind != NodeKind::File {
            return Err(VfsError::KindMismatch {
                path: self.path.as_str().to_string(),
                expected: "file",
                found: node.kind.as_str(),
            });
        }
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
        // In-memory backend has no buffering — writes are already durable in RAM.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> VfsPath {
        VfsPath::new(s).expect("test path must be valid")
    }

    // 1. A fresh backend is empty except for the root directory.
    #[tokio::test]
    async fn empty_backend_has_only_root() {
        let backend = MemoryBackend::new();
        let meta = backend.metadata(&VfsPath::root()).await.unwrap();
        assert_eq!(meta.kind, NodeKind::Directory);
        let children = backend.list(&VfsPath::root()).await.unwrap();
        assert!(children.is_empty(), "fresh backend should have no children");
    }

    // 2. Creating a file via open(CREATE | WRITE) succeeds and is visible via metadata.
    #[tokio::test]
    async fn create_file_with_create_flag() {
        let backend = MemoryBackend::new();
        let path = p("/a.txt");
        let _h = backend
            .open(&path, OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        let meta = backend.metadata(&path).await.unwrap();
        assert_eq!(meta.kind, NodeKind::File);
        assert_eq!(meta.size, 0);
    }

    // 3. Write then read returns the same bytes from the same offset.
    #[tokio::test]
    async fn write_then_read_roundtrip() {
        let backend = MemoryBackend::new();
        let path = p("/hello.txt");
        let mut h = backend
            .open(&path, OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        let n = h.write(0, b"hello world").await.unwrap();
        assert_eq!(n, 11);
        h.flush().await.unwrap();
        drop(h);

        let mut h2 = backend.open(&path, OpenFlags::READ).await.unwrap();
        let got = h2.read(0, 1024).await.unwrap();
        assert_eq!(&got[..], b"hello world");
        let meta = backend.metadata(&path).await.unwrap();
        assert_eq!(meta.size, 11);
    }

    // 4. list() returns direct children only, not deep descendants.
    #[tokio::test]
    async fn list_directory_returns_direct_children_only() {
        let backend = MemoryBackend::new();
        backend.create_dir(&p("/d")).await.unwrap();
        backend.create_dir(&p("/d/sub")).await.unwrap();
        backend
            .open(&p("/d/a.txt"), OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        backend
            .open(&p("/d/sub/deep.txt"), OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();

        let mut children: Vec<String> = backend
            .list(&p("/d"))
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.as_str().to_string())
            .collect();
        children.sort();
        assert_eq!(children, vec!["/d/a.txt".to_string(), "/d/sub".to_string()]);
    }

    // 5. remove() of a file makes it disappear.
    #[tokio::test]
    async fn remove_file() {
        let backend = MemoryBackend::new();
        let path = p("/gone.txt");
        backend
            .open(&path, OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        backend.remove(&path).await.unwrap();
        let err = backend.metadata(&path).await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound { .. }));
    }

    // 6. An empty directory can be removed.
    #[tokio::test]
    async fn remove_empty_directory() {
        let backend = MemoryBackend::new();
        let dir = p("/d");
        backend.create_dir(&dir).await.unwrap();
        backend.remove(&dir).await.unwrap();
        let err = backend.metadata(&dir).await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound { .. }));
    }

    // 7. Reading a path that doesn't exist returns NotFound.
    // Note: `Box<dyn VfsHandle>` doesn't implement Debug, so we use `let...else`
    // to discard the Ok handle cleanly instead of `unwrap_err`.
    #[tokio::test]
    async fn read_nonexistent_is_not_found() {
        let backend = MemoryBackend::new();
        let Err(err) = backend.open(&p("/missing.txt"), OpenFlags::READ).await else {
            panic!("expected error")
        };
        assert!(matches!(err, VfsError::NotFound { .. }));
    }

    // 8. open(WRITE) without CREATE on a missing file returns NotFound.
    #[tokio::test]
    async fn write_to_nonexistent_without_create_is_not_found() {
        let backend = MemoryBackend::new();
        let Err(err) = backend.open(&p("/nope.txt"), OpenFlags::WRITE).await else {
            panic!("expected error")
        };
        assert!(matches!(err, VfsError::NotFound { .. }));
    }

    // 9. list() on a file (not a directory) returns KindMismatch.
    #[tokio::test]
    async fn list_on_file_is_kind_mismatch() {
        let backend = MemoryBackend::new();
        let path = p("/file.txt");
        backend
            .open(&path, OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        let err = backend.list(&path).await.unwrap_err();
        assert!(matches!(err, VfsError::KindMismatch { .. }));
    }

    // 10. Removing a non-empty directory fails.
    #[tokio::test]
    async fn remove_nonempty_directory_fails() {
        let backend = MemoryBackend::new();
        let dir = p("/d");
        backend.create_dir(&dir).await.unwrap();
        backend
            .open(&p("/d/a.txt"), OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        let err = backend.remove(&dir).await.unwrap_err();
        assert!(matches!(err, VfsError::Backend(_)));
    }

    // 11. EXCL on an existing file returns AlreadyExists.
    #[tokio::test]
    async fn create_excl_on_existing_is_already_exists() {
        let backend = MemoryBackend::new();
        let path = p("/x.txt");
        backend
            .open(&path, OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        let Err(err) = backend
            .open(
                &path,
                OpenFlags::CREATE | OpenFlags::EXCL | OpenFlags::WRITE,
            )
            .await
        else {
            panic!("expected error")
        };
        assert!(matches!(err, VfsError::AlreadyExists { .. }));
    }

    // 12. TRUNC clears existing contents.
    #[tokio::test]
    async fn trunc_clears_contents() {
        let backend = MemoryBackend::new();
        let path = p("/t.txt");
        let mut h = backend
            .open(&path, OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .unwrap();
        h.write(0, b"old data").await.unwrap();
        drop(h);

        let _h2 = backend
            .open(&path, OpenFlags::WRITE | OpenFlags::TRUNC)
            .await
            .unwrap();
        let meta = backend.metadata(&path).await.unwrap();
        assert_eq!(meta.size, 0);
    }
}
