//! [`VfsPath`] — a cross-platform, normalized path type used everywhere in the API.

use std::fmt;

/// A normalized, forward-slash, absolute path inside a virtual filesystem.
///
/// `VfsPath` deliberately abstracts away from `std::path::Path` so that:
///
/// 1. Paths behave identically on Linux, macOS, and Windows in tests.
/// 2. Backends can map the canonical VFS path to whatever native representation they need
///    at their mount boundary.
/// 3. Path segments never contain platform-specific separators.
///
/// All paths are absolute and use `/` as the separator. The root path is `/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VfsPath {
    inner: String,
}

impl VfsPath {
    /// Returns the root path `/`.
    #[must_use]
    pub fn root() -> Self {
        Self {
            inner: String::from("/"),
        }
    }

    /// Constructs a `VfsPath` from a string, normalizing it.
    ///
    /// Returns `None` if the path contains invalid characters (currently: NUL bytes).
    /// Backslashes are converted to forward slashes. Trailing slashes are removed except
    /// on root. Sequences of `//` are collapsed.
    #[must_use]
    pub fn new(s: impl AsRef<str>) -> Option<Self> {
        let s = s.as_ref();
        if s.is_empty() || s.contains('\0') {
            return None;
        }

        // Normalize: convert \ to /, collapse //, ensure leading /
        let mut normalized = String::with_capacity(s.len() + 1);
        if !s.starts_with('/') && !s.starts_with('\\') {
            normalized.push('/');
        }
        let mut last_was_slash = false;
        for ch in s.chars() {
            let ch = if ch == '\\' { '/' } else { ch };
            if ch == '/' {
                if last_was_slash {
                    continue;
                }
                last_was_slash = true;
            } else {
                last_was_slash = false;
            }
            normalized.push(ch);
        }

        // Strip trailing slash except for root
        if normalized.len() > 1 && normalized.ends_with('/') {
            normalized.pop();
        }

        Some(Self { inner: normalized })
    }

    /// The path as a `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Returns true if this is the root `/` path.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.inner == "/"
    }

    /// Returns the parent of this path, or `None` if this is the root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }
        let trimmed = self.inner.rsplit_once('/')?.0;
        if trimmed.is_empty() {
            Some(Self::root())
        } else {
            Some(Self {
                inner: trimmed.to_string(),
            })
        }
    }

    /// Returns the final path component (the "filename"), or `None` for the root.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        if self.is_root() {
            return None;
        }
        self.inner.rsplit_once('/').map(|(_, name)| name)
    }

    /// Joins another path segment onto this path.
    #[must_use]
    pub fn join(&self, segment: impl AsRef<str>) -> Self {
        let segment = segment.as_ref().trim_matches('/');
        if segment.is_empty() {
            return self.clone();
        }
        let inner = if self.is_root() {
            format!("/{segment}")
        } else {
            format!("{}/{}", self.inner, segment)
        };
        Self { inner }
    }
}

impl fmt::Display for VfsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

impl AsRef<str> for VfsPath {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}
