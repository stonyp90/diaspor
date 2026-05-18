//! `ModelHub` — resolves model ids to local files.
//!
//! The hub is the bridge between [`crate::catalog::ModelRef`] (catalog
//! metadata) and the on-disk paths every `InferenceBackend` actually wants:
//!
//! 1. Caller asks for a model by id.
//! 2. Hub looks it up in the embedded [`Catalog`].
//! 3. If the file is already in the cache at
//!    `~/.diaspor/models/<id-slug>@<sha256-prefix>/<basename>` and its
//!    sha256 matches the catalog entry, return that path.
//! 4. Otherwise, download from the entry's URL (unless
//!    `DIASPOR_OFFLINE=1`), verify sha256, atomically rename into the
//!    cache, and return the path.
//!
//! # Privacy contract
//!
//! When `DIASPOR_OFFLINE=1` is set, downloads are refused with
//! [`HubError::NetworkBlocked`]. The `no-network` CI job runs the entire
//! test suite under that variable to structurally guarantee that no default
//! code path attempts a fetch.
//!
//! # Trust model
//!
//! - **Pinned entries** (sha256 + bytes both set) — the file at `url` MUST
//!   match. Mismatch -> [`HubError::ChecksumMismatch`].
//! - **Unpinned entries** (`sha256 == "PENDING_FIRST_PULL"`) — the catalog
//!   is bootstrapping that entry; we refuse to use it UNLESS
//!   `DIASPOR_TRUST_UNPINNED=1` is set. When trusting, we download, record
//!   the observed hash, and ask the operator to commit it.
//! - **Nonfree entries** — refused at compile time when
//!   `--features nonfree-models` is off, refused at runtime with a
//!   license-notice print when the feature is on.

use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::catalog::{Catalog, CatalogError, ModelRef, UNPINNED_SHA256};

/// Errors that can come out of the hub.
#[derive(Debug, Error)]
pub enum HubError {
    /// No catalog entry with this id was found.
    #[error("unknown model id: {id}")]
    UnknownId {
        /// The id that was looked up.
        id: String,
    },

    /// The catalog entry is gated behind the `nonfree-models` feature and
    /// the binary was built without it. Build with
    /// `--features nonfree-models` to unlock — and read the model's license
    /// before doing so.
    #[error(
        "model {id} carries a research / non-commercial license ({license}); rebuild with --features nonfree-models to unlock"
    )]
    NonfreeBlocked {
        /// The blocked id.
        id: String,
        /// The license string from the catalog.
        license: String,
    },

    /// The catalog entry has `sha256 = PENDING_FIRST_PULL` and the operator
    /// has not set `DIASPOR_TRUST_UNPINNED=1`. `ModelHub` refuses by default
    /// to use unpinned entries because there is no way to authenticate the
    /// downloaded file.
    #[error(
        "model {id} is not pinned in the catalog (sha256 is PENDING_FIRST_PULL); set DIASPOR_TRUST_UNPINNED=1 to download anyway and record the observed hash"
    )]
    Unpinned {
        /// The unpinned id.
        id: String,
    },

    /// The downloaded file's sha256 did not match the value pinned in the
    /// catalog. This is a hard error — never trust a checksum mismatch.
    #[error("checksum mismatch for {id}: catalog={expected}, downloaded={actual}")]
    ChecksumMismatch {
        /// The catalog id.
        id: String,
        /// Hash from the catalog.
        expected: String,
        /// Hash we observed on the downloaded file.
        actual: String,
    },

    /// The privacy contract refused a network fetch. Set when
    /// `DIASPOR_OFFLINE=1` and the requested model is not already cached.
    #[error(
        "network access is disabled (DIASPOR_OFFLINE=1) and model {id} is not in the local cache"
    )]
    NetworkBlocked {
        /// The id that would have required a fetch.
        id: String,
    },

    /// The URL scheme is not one we understand. Today we accept `https`,
    /// `http` (for in-network mirrors), and `file://` (for in-repo test
    /// fixtures).
    #[error("unsupported URL scheme for {id}: {url}")]
    UnsupportedScheme {
        /// The id whose URL we choked on.
        id: String,
        /// The URL we choked on.
        url: String,
    },

    /// Catalog parsing went sideways. Should never happen for the embedded
    /// catalog — emitted only when an operator points at a custom catalog
    /// file via [`ModelHub::with_catalog_file`].
    #[error(transparent)]
    Catalog(#[from] CatalogError),

    /// I/O — could not create the cache directory, could not write a file,
    /// etc. The wrapped error carries the path.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP failure or stream read error during download.
    #[error("download failed for {id}: {source}")]
    Download {
        /// The id we were downloading.
        id: String,
        /// Underlying reqwest error.
        #[source]
        source: reqwest::Error,
    },
}

/// Configuration knobs for [`ModelHub`]. Most callers want [`HubConfig::default`].
#[derive(Debug, Clone)]
pub struct HubConfig {
    /// Absolute path to the cache root. Defaults to
    /// `${HOME}/.diaspor/models`. Override via the `DIASPOR_MODELS_DIR` env
    /// var or [`HubConfig::with_root`].
    pub root: PathBuf,
    /// When `true`, downloads are refused outright. Mirrors
    /// `DIASPOR_OFFLINE=1`. Default `false`.
    pub offline: bool,
    /// When `true`, unpinned catalog entries are downloaded anyway and the
    /// observed hash is logged for the operator to commit. Mirrors
    /// `DIASPOR_TRUST_UNPINNED=1`. Default `false`.
    pub trust_unpinned: bool,
    /// When `true`, the runtime allows resolving entries with
    /// `nonfree = true`. Mirrors the `nonfree-models` cargo feature; the
    /// compile-time gate is the source of truth, but the bool lets tests
    /// flip it independently. Default `false`.
    pub allow_nonfree: bool,
}

impl HubConfig {
    /// Builds a config from the process environment. Reads
    /// `DIASPOR_MODELS_DIR`, `DIASPOR_OFFLINE`, `DIASPOR_TRUST_UNPINNED`.
    pub fn from_env() -> Self {
        let root = env::var_os("DIASPOR_MODELS_DIR").map_or_else(default_cache_root, PathBuf::from);
        Self {
            root,
            offline: env_flag("DIASPOR_OFFLINE"),
            trust_unpinned: env_flag("DIASPOR_TRUST_UNPINNED"),
            allow_nonfree: cfg!(feature = "nonfree-models"),
        }
    }

    /// Overrides the cache root.
    #[must_use]
    pub fn with_root(mut self, root: PathBuf) -> Self {
        self.root = root;
        self
    }

    /// Forces offline mode on or off.
    #[must_use]
    pub const fn with_offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }
}

impl Default for HubConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

fn default_cache_root() -> PathBuf {
    // `dirs::home_dir()` returns `None` on systems without a $HOME (rare on
    // POSIX, more common in stripped CI containers). Fall back to a
    // process-local temp path so we never panic — the caller can always
    // override via `HubConfig::with_root`.
    dirs::home_dir().map_or_else(
        || std::env::temp_dir().join("diaspor").join("models"),
        |h| h.join(".diaspor").join("models"),
    )
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).ok().as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

/// The hub itself.
///
/// Cheap to clone — internals are wrapped in `Arc`. Spawn one per process
/// and share it across pipelines.
#[derive(Debug, Clone)]
pub struct ModelHub {
    inner: Arc<HubInner>,
}

#[derive(Debug)]
struct HubInner {
    config: HubConfig,
    catalog: Catalog,
    http: reqwest::Client,
}

impl ModelHub {
    /// Builds a hub backed by the embedded catalog and the default config.
    /// Reads env vars on construction (see [`HubConfig::from_env`]).
    pub fn from_embedded() -> Result<Self, HubError> {
        let catalog = Catalog::embedded()?;
        Ok(Self::with_catalog(HubConfig::default(), catalog))
    }

    /// Builds a hub from a config + a pre-parsed catalog. Used by tests
    /// that want to ship a single fixture entry.
    pub fn with_catalog(config: HubConfig, catalog: Catalog) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("diaspor-infer/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest::Client::build is infallible with default opts");
        Self {
            inner: Arc::new(HubInner {
                config,
                catalog,
                http,
            }),
        }
    }

    /// Reads a catalog file from disk (useful for operators with a private
    /// model fleet).
    pub async fn with_catalog_file(
        config: HubConfig,
        path: impl AsRef<Path>,
    ) -> Result<Self, HubError> {
        let raw = tokio::fs::read_to_string(path.as_ref()).await?;
        let catalog = Catalog::from_toml_str(&raw)?;
        Ok(Self::with_catalog(config, catalog))
    }

    /// Returns the underlying catalog (read-only).
    #[must_use]
    pub fn catalog(&self) -> &Catalog {
        &self.inner.catalog
    }

    /// Returns the config (read-only).
    #[must_use]
    pub fn config(&self) -> &HubConfig {
        &self.inner.config
    }

    /// Returns the local cache path a given id will resolve to once
    /// downloaded — does NOT trigger a download.
    #[must_use]
    pub fn local_path(&self, id: &str) -> Option<PathBuf> {
        let entry = self.inner.catalog.get(id)?;
        Some(self.cache_path_for(entry))
    }

    fn cache_path_for(&self, entry: &ModelRef) -> PathBuf {
        // sha256-prefix gives us a content-addressed dir name that survives
        // a catalog edit (new revision, same bytes).
        let sha_prefix: String = entry.sha256.chars().take(12).collect();
        let dir_name = format!("{}@{}", slug(&entry.id), sha_prefix);
        let basename = url_basename(&entry.url).unwrap_or("model.bin").to_string();
        self.inner.config.root.join(dir_name).join(basename)
    }

    /// Resolves an id to a local file path, downloading and verifying as
    /// needed. Returns the path of a file whose sha256 matches the catalog.
    ///
    /// # Errors
    ///
    /// See [`HubError`] for the variants.
    pub async fn resolve(&self, id: &str) -> Result<PathBuf, HubError> {
        let entry = self
            .inner
            .catalog
            .get(id)
            .ok_or_else(|| HubError::UnknownId { id: id.to_string() })?
            .clone();

        // 1. Nonfree gate
        if entry.nonfree && !self.inner.config.allow_nonfree {
            return Err(HubError::NonfreeBlocked {
                id: entry.id.clone(),
                license: entry.license.clone(),
            });
        }

        // 2. Pin gate
        let unpinned = entry.sha256 == UNPINNED_SHA256 || !entry.is_pinned();
        if unpinned && !self.inner.config.trust_unpinned {
            return Err(HubError::Unpinned { id: entry.id });
        }

        let target = self.cache_path_for(&entry);

        // 3. Cache hit?
        if target.is_file() {
            if entry.is_pinned() {
                let actual = sha256_of_file(&target).await?;
                if actual == entry.sha256 {
                    return Ok(target);
                }
                // File on disk has been corrupted or tampered with —
                // remove and re-download.
                warn!(
                    id = %entry.id,
                    expected = %entry.sha256,
                    actual = %actual,
                    "cached model checksum mismatch — re-downloading"
                );
                tokio::fs::remove_file(&target).await?;
            } else {
                // Unpinned: trust the cache as-is. The user has already
                // opted in via DIASPOR_TRUST_UNPINNED.
                return Ok(target);
            }
        }

        // 4. Offline gate
        if self.inner.config.offline {
            return Err(HubError::NetworkBlocked { id: entry.id });
        }

        // 5. Download
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let actual = download_to(&self.inner.http, &entry, &target).await?;

        // 6. Verify
        if entry.is_pinned() && actual != entry.sha256 {
            // Hard fail. Remove the bad file so we don't accidentally trust
            // it on the next call.
            let _ = tokio::fs::remove_file(&target).await;
            return Err(HubError::ChecksumMismatch {
                id: entry.id,
                expected: entry.sha256,
                actual,
            });
        }
        if !entry.is_pinned() {
            warn!(
                id = %entry.id,
                observed_sha256 = %actual,
                "downloaded unpinned model; commit this hash to models.toml to pin"
            );
        }
        if entry.nonfree {
            info!(
                id = %entry.id,
                license = %entry.license,
                "loaded nonfree model — review the upstream license before production use"
            );
        }
        Ok(target)
    }

    /// Resolves every entry in the catalog that passes the pin + nonfree
    /// gates. Returns the count of files now present in the cache.
    ///
    /// Used by `diaspor models pull --all` (Phase 2). Errors on the first
    /// failure; partial success is reported in `tracing` logs.
    pub async fn ensure_all(&self) -> Result<usize, HubError> {
        let mut count = 0;
        for entry in self.inner.catalog.iter() {
            if entry.nonfree && !self.inner.config.allow_nonfree {
                continue;
            }
            if entry.sha256 == UNPINNED_SHA256 && !self.inner.config.trust_unpinned {
                continue;
            }
            self.resolve(&entry.id).await?;
            count += 1;
        }
        Ok(count)
    }
}

fn slug(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn url_basename(url: &str) -> Option<&str> {
    url.rsplit_once('/')
        .map(|(_, base)| base)
        .filter(|b| !b.is_empty())
}

async fn sha256_of_file(path: &Path) -> Result<String, std::io::Error> {
    let bytes = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

async fn download_to(
    client: &reqwest::Client,
    entry: &ModelRef,
    target: &Path,
) -> Result<String, HubError> {
    // Local file:// URL — copy-with-hash, no network involved. Used by the
    // test fixture and any operator who mirrors models on a shared volume.
    if let Some(local) = entry.url.strip_prefix("file://") {
        let src = resolve_file_url(local)?;
        let bytes = tokio::fs::read(&src).await?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = hex::encode(hasher.finalize());
        write_atomic(target, &bytes).await?;
        return Ok(actual);
    }

    if !(entry.url.starts_with("https://") || entry.url.starts_with("http://")) {
        return Err(HubError::UnsupportedScheme {
            id: entry.id.clone(),
            url: entry.url.clone(),
        });
    }

    let resp = client
        .get(&entry.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|source| HubError::Download {
            id: entry.id.clone(),
            source,
        })?;

    let mut stream = resp.bytes_stream();
    let mut hasher = Sha256::new();

    let tmp = tmp_path_for(target);
    let mut file = tokio::fs::File::create(&tmp).await?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|source| HubError::Download {
            id: entry.id.clone(),
            source,
        })?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);

    tokio::fs::rename(&tmp, target).await?;
    Ok(hex::encode(hasher.finalize()))
}

fn resolve_file_url(local: &str) -> Result<PathBuf, std::io::Error> {
    // `file://` URLs in `models.toml` are repo-relative when they start
    // with `crates/`, `docs/`, `training/`, etc. — that's the convention
    // for the test fixture. Absolute paths are also accepted.
    let path = Path::new(local);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    // CARGO_MANIFEST_DIR for the diaspor-infer crate, two `..` up = workspace root.
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            std::io::Error::other("CARGO_MANIFEST_DIR must live two levels below workspace root")
        })?;
    Ok(workspace_root.join(path))
}

fn tmp_path_for(target: &Path) -> PathBuf {
    let mut tmp = target.as_os_str().to_owned();
    tmp.push(".partial");
    PathBuf::from(tmp)
}

async fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let tmp = tmp_path_for(target);
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, target).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::ModelRef;

    fn fixture_catalog(entry: &ModelRef) -> Catalog {
        let toml_str = format!(
            r#"schema_version = 1

[[models]]
id = "{id}"
description = "test"
format = "{fmt}"
url = "{url}"
sha256 = "{sha}"
bytes = {bytes}
license = "{license}"
feature_gate = "ort-cpu"
nonfree = {nonfree}
"#,
            id = entry.id,
            fmt = entry.format,
            url = entry.url,
            sha = entry.sha256,
            bytes = entry.bytes,
            license = entry.license,
            nonfree = entry.nonfree
        );
        Catalog::from_toml_str(&toml_str).expect("test catalog must parse")
    }

    #[tokio::test]
    async fn unknown_id_errors() {
        let hub = ModelHub::with_catalog(HubConfig::default(), Catalog::embedded().unwrap());
        let err = hub.resolve("definitely-not-here@99").await.unwrap_err();
        assert!(matches!(err, HubError::UnknownId { .. }));
    }

    #[tokio::test]
    async fn unpinned_refused_by_default() {
        let entry = ModelRef {
            id: "ghost@1".into(),
            description: String::new(),
            format: "onnx".into(),
            url: "https://example.invalid/x.onnx".into(),
            sha256: UNPINNED_SHA256.into(),
            bytes: 0,
            license: "MIT".into(),
            feature_gate: "ort-cpu".into(),
            nonfree: false,
            notes: None,
        };
        let cat = fixture_catalog(&entry);
        let tmp = tempfile::tempdir().unwrap();
        let config = HubConfig {
            root: tmp.path().to_path_buf(),
            offline: false,
            trust_unpinned: false,
            allow_nonfree: false,
        };
        let hub = ModelHub::with_catalog(config, cat);
        let err = hub.resolve("ghost@1").await.unwrap_err();
        assert!(matches!(err, HubError::Unpinned { .. }));
    }

    #[tokio::test]
    async fn nonfree_refused_when_feature_off() {
        let entry = ModelRef {
            id: "research@1".into(),
            description: String::new(),
            format: "onnx".into(),
            url: "https://example.invalid/x.onnx".into(),
            sha256: "0".repeat(64),
            bytes: 100,
            license: "noncommercial".into(),
            feature_gate: "nonfree-models".into(),
            nonfree: true,
            notes: None,
        };
        let cat = fixture_catalog(&entry);
        let tmp = tempfile::tempdir().unwrap();
        let config = HubConfig {
            root: tmp.path().to_path_buf(),
            offline: false,
            trust_unpinned: false,
            allow_nonfree: false,
        };
        let hub = ModelHub::with_catalog(config, cat);
        let err = hub.resolve("research@1").await.unwrap_err();
        assert!(matches!(err, HubError::NonfreeBlocked { .. }));
    }

    #[tokio::test]
    async fn offline_refuses_uncached_https() {
        let entry = ModelRef {
            id: "stub@1".into(),
            description: String::new(),
            format: "onnx".into(),
            url: "https://example.invalid/x.onnx".into(),
            sha256: "0".repeat(64),
            bytes: 100,
            license: "MIT".into(),
            feature_gate: "ort-cpu".into(),
            nonfree: false,
            notes: None,
        };
        let cat = fixture_catalog(&entry);
        let tmp = tempfile::tempdir().unwrap();
        let config = HubConfig {
            root: tmp.path().to_path_buf(),
            offline: true,
            trust_unpinned: false,
            allow_nonfree: false,
        };
        let hub = ModelHub::with_catalog(config, cat);
        let err = hub.resolve("stub@1").await.unwrap_err();
        assert!(matches!(err, HubError::NetworkBlocked { .. }));
    }

    #[test]
    fn slug_strips_at_and_slashes() {
        assert_eq!(slug("mediapipe-pose-heavy@1"), "mediapipe-pose-heavy-1");
        assert_eq!(slug("a/b/c"), "a-b-c");
        assert_eq!(slug("openface-3-au@2"), "openface-3-au-2");
    }
}
