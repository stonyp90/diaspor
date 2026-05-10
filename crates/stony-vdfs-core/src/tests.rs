//! Unit tests for the core types.

use crate::{NodeKind, OpenFlags, VfsError, VfsPath};

#[test]
fn root_path_is_root() {
    let root = VfsPath::root();
    assert!(root.is_root());
    assert_eq!(root.as_str(), "/");
    assert!(root.parent().is_none());
    assert!(root.name().is_none());
}

#[test]
fn path_normalizes_separators() {
    let path = VfsPath::new(r"foo\bar\baz").unwrap();
    assert_eq!(path.as_str(), "/foo/bar/baz");
}

#[test]
fn path_collapses_double_slashes() {
    let path = VfsPath::new("///foo//bar///").unwrap();
    assert_eq!(path.as_str(), "/foo/bar");
}

#[test]
fn path_rejects_nul() {
    assert!(VfsPath::new("foo\0bar").is_none());
}

#[test]
fn path_rejects_empty() {
    assert!(VfsPath::new("").is_none());
}

#[test]
fn path_parent_and_name() {
    let p = VfsPath::new("/a/b/c").unwrap();
    assert_eq!(p.name(), Some("c"));
    let parent = p.parent().unwrap();
    assert_eq!(parent.as_str(), "/a/b");
    assert_eq!(parent.parent().unwrap().as_str(), "/a");
    assert_eq!(parent.parent().unwrap().parent().unwrap().as_str(), "/");
}

#[test]
fn path_join() {
    let p = VfsPath::root().join("foo").join("bar");
    assert_eq!(p.as_str(), "/foo/bar");
    let p2 = p.join("/baz/");
    assert_eq!(p2.as_str(), "/foo/bar/baz");
}

#[test]
fn open_flags_compose() {
    let f = OpenFlags::READ | OpenFlags::WRITE;
    assert!(f.contains(OpenFlags::READ));
    assert!(f.contains(OpenFlags::WRITE));
    assert!(!f.contains(OpenFlags::APPEND));
}

#[test]
fn node_kind_labels() {
    assert_eq!(NodeKind::File.as_str(), "file");
    assert_eq!(NodeKind::Directory.as_str(), "directory");
    assert_eq!(NodeKind::Symlink.as_str(), "symlink");
}

#[test]
fn error_constructors() {
    let e = VfsError::not_found("/missing");
    assert!(matches!(e, VfsError::NotFound { .. }));
    let e = VfsError::invalid_path("\0bad");
    assert!(matches!(e, VfsError::InvalidPath { .. }));
}
