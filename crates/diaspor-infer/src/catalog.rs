//! Embedded model catalog.
//!
//! The workspace-root `models.toml` is baked into the binary at compile time
//! via `include_str!` and parsed once per process. Every entry describes a
//! single OSS model the [`crate::ModelHub`] knows how to resolve: source URL,
//! pinned sha256, license, and the cargo feature that gates its use.
//!
//! Feature-gated by `hub` — the bare trait surface of `diaspor-infer` does
//! not pull in TOML parsing or the catalog. Most callers will reach this
//! through `ModelHub::default()`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The raw `models.toml` content, embedded at compile time. The catalog
/// MUST stay short enough to fit comfortably in the binary (every entry is
/// metadata only — no weights are bundled here).
const EMBEDDED_CATALOG_TOML: &str = include_str!("../../../models.toml");

/// Sentinel sha256 value for unpinned catalog entries.
///
/// Entries with this value exist in the catalog for discovery but cannot be
/// resolved unless the operator opts into trusting unpinned entries via
/// `DIASPOR_TRUST_UNPINNED=1`. See `models.toml` for the policy.
pub const UNPINNED_SHA256: &str = "PENDING_FIRST_PULL";

/// On-disk layout of `models.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatalogToml {
    /// Version field for forward-compatibility. v1 is what this crate parses.
    pub schema_version: u32,
    /// All model entries, in declaration order.
    #[serde(default, rename = "models")]
    pub models: Vec<ModelRef>,
}

/// One model entry in the catalog.
///
/// Field naming matches the `score-v1.json` `model_provenance` shape so the
/// catalog can feed adapter output directly: `id`, `sha256`, and the
/// catalog's `notes` / `license` flow into the score sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ModelRef {
    /// Catalog id. Convention: `<short-name>@<catalog-revision>`, e.g.
    /// `mediapipe-pose-heavy@1`. The `@N` suffix lets us evolve the entry
    /// (new sha256, new URL) without breaking score sidecars that pin an
    /// older revision.
    pub id: String,
    /// One-line description for human readers (and for `--help` output).
    pub description: String,
    /// File format the URL serves: `onnx`, `tflite`, `task` (`MediaPipe`
    /// bundle), `ggml`, or `safetensors`. Adapters dispatch on this.
    pub format: String,
    /// Source URL. Use `file://` for in-repo test fixtures.
    pub url: String,
    /// Pinned sha256 of the file at `url`. Either a 64-char hex string or
    /// the sentinel [`UNPINNED_SHA256`].
    pub sha256: String,
    /// Pinned byte size of the file at `url`. `0` for unpinned entries.
    #[serde(default)]
    pub bytes: u64,
    /// SPDX license identifier (e.g. `Apache-2.0`) or a free-form string
    /// for unusual licenses (e.g. `pyannote-research-noncommercial`).
    pub license: String,
    /// Name of the cargo feature that must be enabled for this model to be
    /// useful at runtime. `ModelHub::resolve` does not check this — the
    /// adapter that asks for the model is the one whose feature gates it.
    /// Recorded here for `diaspor models list` output.
    pub feature_gate: String,
    /// `true` if this model carries a research / non-commercial license.
    /// `ModelHub` refuses to resolve `nonfree = true` entries unless built
    /// with `--features nonfree-models`.
    #[serde(default)]
    pub nonfree: bool,
    /// Optional free-form notes shown on first download (e.g. license
    /// caveats, conversion steps, upstream model-card URL).
    #[serde(default)]
    pub notes: Option<String>,
}

impl ModelRef {
    /// Returns `true` iff the entry has a real 64-char hex sha256 (not the
    /// `PENDING_FIRST_PULL` sentinel).
    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.sha256.len() == 64
            && self.sha256.chars().all(|c| c.is_ascii_hexdigit())
            && self.bytes > 0
    }
}

/// Errors specific to catalog loading.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    /// The TOML failed to parse — typically a syntax error or wrong `schema_version`.
    #[error("catalog parse failed: {0}")]
    Parse(#[from] toml::de::Error),

    /// The catalog declared an unsupported schema version. The currently
    /// supported value is `1`.
    #[error("catalog schema_version {found} is not supported (expected 1)")]
    UnsupportedSchemaVersion {
        /// The `schema_version` field as it appeared in the catalog.
        found: u32,
    },

    /// Two entries shared the same `id` — catalog ids must be unique.
    #[error("catalog has duplicate id: {id}")]
    DuplicateId {
        /// The duplicate id.
        id: String,
    },
}

/// Loaded, validated, in-memory representation of `models.toml`.
#[derive(Debug, Clone)]
pub struct Catalog {
    by_id: HashMap<String, ModelRef>,
    order: Vec<String>,
}

impl Catalog {
    /// Parses a TOML document into a [`Catalog`]. Returns [`CatalogError`] on
    /// schema-version mismatch, parse failure, or duplicate-id collisions.
    pub fn from_toml_str(toml_str: &str) -> Result<Self, CatalogError> {
        let raw: CatalogToml = toml::from_str(toml_str)?;
        if raw.schema_version != 1 {
            return Err(CatalogError::UnsupportedSchemaVersion {
                found: raw.schema_version,
            });
        }
        let mut by_id = HashMap::with_capacity(raw.models.len());
        let mut order = Vec::with_capacity(raw.models.len());
        for entry in raw.models {
            if by_id.contains_key(&entry.id) {
                return Err(CatalogError::DuplicateId { id: entry.id });
            }
            order.push(entry.id.clone());
            by_id.insert(entry.id.clone(), entry);
        }
        Ok(Self { by_id, order })
    }

    /// Loads the embedded `models.toml`. Used by `ModelHub::default()`.
    pub fn embedded() -> Result<Self, CatalogError> {
        Self::from_toml_str(EMBEDDED_CATALOG_TOML)
    }

    /// Looks up an entry by exact id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ModelRef> {
        self.by_id.get(id)
    }

    /// Iterates over every entry in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = &ModelRef> {
        self.order.iter().filter_map(|id| self.by_id.get(id))
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// `true` iff the catalog has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_parses() {
        let catalog = Catalog::embedded().expect("embedded models.toml must parse");
        // The catalog ships with at least the test fixture and a handful of
        // Phase 2 placeholders; a hard floor catches accidental empty
        // commits.
        assert!(
            catalog.len() >= 5,
            "embedded catalog should ship with at least 5 entries, got {}",
            catalog.len()
        );
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let toml_str = r#"
schema_version = 1

[[models]]
id = "x@1"
description = "first"
format = "onnx"
url = "https://example.com/x.onnx"
sha256 = "PENDING_FIRST_PULL"
license = "MIT"
feature_gate = "x"

[[models]]
id = "x@1"
description = "second copy"
format = "onnx"
url = "https://example.com/x2.onnx"
sha256 = "PENDING_FIRST_PULL"
license = "MIT"
feature_gate = "x"
"#;
        let err = Catalog::from_toml_str(toml_str).expect_err("duplicate ids must error");
        assert!(matches!(err, CatalogError::DuplicateId { .. }));
    }

    #[test]
    fn pinned_flag_distinguishes_unpinned() {
        let unpinned = ModelRef {
            id: "x@1".into(),
            description: String::new(),
            format: "onnx".into(),
            url: "https://example.com".into(),
            sha256: UNPINNED_SHA256.into(),
            bytes: 0,
            license: "MIT".into(),
            feature_gate: "x".into(),
            nonfree: false,
            notes: None,
        };
        assert!(!unpinned.is_pinned());
        let pinned = ModelRef {
            sha256: "0".repeat(64),
            bytes: 1234,
            ..unpinned
        };
        assert!(pinned.is_pinned());
    }
}
