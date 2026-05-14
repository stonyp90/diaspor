//! # diaspor-conformance
//!
//! A backend-agnostic conformance test suite. Every implementation of
//! [`diaspor_core::VfsBackend`] must pass [`run`] in order to be considered compliant.
//!
//! The suite is exposed as a single async function so it can be invoked from each backend
//! crate's integration tests. This guarantees that the in-memory backend, the local-disk
//! backend, and any future cloud backend all behave identically when observed through the
//! public [`VfsBackend`] trait.
//!
//! ```ignore
//! use diaspor_backend_memory::MemoryBackend;
//!
//! #[tokio::test]
//! async fn memory_passes_conformance() {
//!     diaspor_conformance::run(MemoryBackend::new()).await;
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/diaspor-conformance/0.1.0-alpha.1")]

use diaspor_core::{NodeKind, OpenFlags, VfsBackend, VfsError, VfsPath};

/// Construct a `VfsPath` from a string literal. Panics on invalid input — only call with
/// test-controlled strings.
fn p(s: &str) -> VfsPath {
    VfsPath::new(s).expect("conformance test path must be valid")
}

/// Run the full conformance suite against `backend`.
///
/// Panics if any assertion fails. The suite assumes `backend` is freshly constructed and
/// empty (i.e. only the root directory exists). It mutates the backend during the run.
///
/// # Assertions covered
///
/// 1. Fresh backend has only the root directory.
/// 2. `metadata` on root reports a directory of size 0.
/// 3. `list` on root of an empty backend returns an empty vector.
/// 4. `open(CREATE | WRITE)` on a missing file creates it.
/// 5. After creation, `metadata` reports `NodeKind::File` and size 0.
/// 6. `write(offset, data)` returns `data.len()` bytes written.
/// 7. After write, `metadata.size` reflects the new file size.
/// 8. `read(0, n)` after `write(0, ...)` returns the same bytes.
/// 9. `read` past EOF returns an empty `Bytes`.
/// 10. `create_dir` creates a directory; `metadata` reports `NodeKind::Directory`.
/// 11. `list` on a directory returns direct children only (not grandchildren).
/// 12. `remove` on a file makes it `NotFound` on next `metadata`.
/// 13. `remove` on an empty directory succeeds.
/// 14. `remove` on a non-empty directory returns an error.
/// 15. `open(READ)` on a missing path returns `NotFound`.
/// 16. `open(WRITE)` without `CREATE` on a missing path returns `NotFound`.
/// 17. `open(CREATE | EXCL)` on an existing file returns `AlreadyExists`.
/// 18. `list` on a file returns `KindMismatch`.
#[allow(
    clippy::too_many_lines,
    reason = "the suite is intentionally a single sequential script of 18 numbered \
              assertions; splitting it would obscure ordering and state dependencies"
)]
pub async fn run<B: VfsBackend>(backend: B) {
    // 1 & 2. Fresh root metadata.
    let root_meta = backend
        .metadata(&VfsPath::root())
        .await
        .expect("root metadata must succeed");
    assert_eq!(
        root_meta.kind,
        NodeKind::Directory,
        "root must be a directory"
    );
    assert_eq!(root_meta.size, 0, "directory size must be 0");

    // 3. list(root) on fresh backend is empty.
    let empty = backend
        .list(&VfsPath::root())
        .await
        .expect("list(root) must succeed on fresh backend");
    assert!(
        empty.is_empty(),
        "fresh backend should have no children, got {empty:?}"
    );

    // 4 & 5. Create a file.
    let file_path = p("/conformance.txt");
    {
        let _h = backend
            .open(&file_path, OpenFlags::CREATE | OpenFlags::WRITE)
            .await
            .expect("create file must succeed");
    }
    let meta = backend
        .metadata(&file_path)
        .await
        .expect("metadata on new file");
    assert_eq!(meta.kind, NodeKind::File, "new node must be a file");
    assert_eq!(meta.size, 0, "new file must have size 0");

    // 6, 7, 8. Write and read back.
    let payload: &[u8] = b"the quick brown fox";
    {
        let mut h = backend
            .open(&file_path, OpenFlags::WRITE)
            .await
            .expect("open for write");
        let n = h.write(0, payload).await.expect("write must succeed");
        assert_eq!(n, payload.len(), "write must return data.len()");
        h.flush().await.expect("flush must succeed");
    }
    let after = backend
        .metadata(&file_path)
        .await
        .expect("metadata after write");
    assert_eq!(
        after.size,
        payload.len() as u64,
        "size must reflect bytes written"
    );
    {
        let mut h = backend
            .open(&file_path, OpenFlags::READ)
            .await
            .expect("open for read");
        let got = h.read(0, 1024).await.expect("read must succeed");
        assert_eq!(&got[..], payload, "round-tripped bytes must match");

        // 9. read past EOF.
        let past = h
            .read(10_000, 16)
            .await
            .expect("read past EOF must succeed");
        assert!(past.is_empty(), "read past EOF must yield empty");
    }

    // 10. create_dir.
    let dir = p("/dir");
    backend.create_dir(&dir).await.expect("create_dir");
    let dmeta = backend.metadata(&dir).await.expect("dir metadata");
    assert_eq!(
        dmeta.kind,
        NodeKind::Directory,
        "create_dir must produce a directory"
    );

    // 11. list returns direct children only.
    let inner_file = p("/dir/a.txt");
    let inner_dir = p("/dir/sub");
    let deep_file = p("/dir/sub/deep.txt");
    backend
        .open(&inner_file, OpenFlags::CREATE | OpenFlags::WRITE)
        .await
        .expect("create /dir/a.txt");
    backend
        .create_dir(&inner_dir)
        .await
        .expect("create /dir/sub");
    backend
        .open(&deep_file, OpenFlags::CREATE | OpenFlags::WRITE)
        .await
        .expect("create /dir/sub/deep.txt");

    let mut listing: Vec<String> = backend
        .list(&dir)
        .await
        .expect("list /dir")
        .into_iter()
        .map(|p| p.as_str().to_string())
        .collect();
    listing.sort();
    assert_eq!(
        listing,
        vec!["/dir/a.txt".to_string(), "/dir/sub".to_string()],
        "list must include /dir/a.txt and /dir/sub but not /dir/sub/deep.txt"
    );

    // 12. remove file.
    backend.remove(&inner_file).await.expect("remove file");
    let err = backend
        .metadata(&inner_file)
        .await
        .expect_err("file must be gone");
    assert!(
        matches!(err, VfsError::NotFound { .. }),
        "expected NotFound, got {err:?}"
    );

    // 13. remove empty directory (clean up /dir/sub first).
    backend
        .remove(&deep_file)
        .await
        .expect("remove deep file before rmdir");
    backend.remove(&inner_dir).await.expect("remove empty dir");

    // 14. remove non-empty directory fails. Repopulate /dir with one file, then try.
    backend
        .open(&p("/dir/keep.txt"), OpenFlags::CREATE | OpenFlags::WRITE)
        .await
        .expect("repopulate /dir");
    let err = backend
        .remove(&dir)
        .await
        .expect_err("non-empty rmdir must fail");
    assert!(
        !matches!(err, VfsError::NotFound { .. }),
        "non-empty rmdir failure should not be NotFound; got {err:?}"
    );

    // 15. open(READ) on missing path -> NotFound.
    // Note: `Box<dyn VfsHandle>` doesn't implement Debug, so we can't use
    // `expect_err` here. `let...else` lets us discard the Ok handle cleanly.
    let Err(err) = backend.open(&p("/does-not-exist"), OpenFlags::READ).await else {
        panic!("read of missing must fail")
    };
    assert!(
        matches!(err, VfsError::NotFound { .. }),
        "expected NotFound, got {err:?}"
    );

    // 16. open(WRITE) without CREATE -> NotFound.
    let Err(err) = backend.open(&p("/also-missing"), OpenFlags::WRITE).await else {
        panic!("write without create on missing must fail")
    };
    assert!(
        matches!(err, VfsError::NotFound { .. }),
        "expected NotFound, got {err:?}"
    );

    // 17. CREATE | EXCL on existing -> AlreadyExists.
    let Err(err) = backend
        .open(
            &p("/dir/keep.txt"),
            OpenFlags::CREATE | OpenFlags::EXCL | OpenFlags::WRITE,
        )
        .await
    else {
        panic!("excl create on existing must fail")
    };
    assert!(
        matches!(err, VfsError::AlreadyExists { .. }),
        "expected AlreadyExists, got {err:?}"
    );

    // 18. list on a file -> KindMismatch.
    let err = backend
        .list(&p("/dir/keep.txt"))
        .await
        .expect_err("list on a file must fail");
    assert!(
        matches!(err, VfsError::KindMismatch { .. }),
        "expected KindMismatch, got {err:?}"
    );
}
