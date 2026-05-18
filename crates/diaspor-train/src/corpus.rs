//! Corpus ingest — turn a tenant-scoped storage location into a hashed [`CorpusManifest`].
//!
//! A "corpus" is the customer-provided pile of video (or audio) clips against which a
//! custom-tier `LoRA` will be fine-tuned. The corpus may already live in the tenant's own
//! S3 bucket (preferred — "weights stay yours" extends to the training data) or inside
//! a `diaspor` VFS path the tenant has uploaded into. Either way, the [`CorpusIngest`]
//! trait normalizes both into a manifest the rest of the pipeline can consume.
//!
//! Ingest is responsible for:
//!
//! 1. Walking the source to count clips and total duration.
//! 2. Computing a `sha256` over the canonicalized clip list so the eventual adapter can
//!    cite its training corpus by digest.
//! 3. Returning a [`CorpusManifest`] that downstream stages key on.
//!
//! The alpha trait surface ships a single [`NoopCorpusIngest`] backend that returns
//! [`crate::TrainError::NotImplemented`].

use std::path::PathBuf;

use async_trait::async_trait;
use diaspor_core::{Result, VfsError};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::TrainError;
use crate::adapter::TenantId;

/// Where a tenant's training corpus lives.
///
/// Two flavors at the alpha stage:
///
/// - [`CorpusSource::S3Prefix`] — the customer points us at a prefix inside their own
///   S3-compatible bucket. The ingest backend MUST honor whatever IAM / KMS scoping the
///   tenant has configured; nothing in this crate elevates privileges.
/// - [`CorpusSource::VfsPath`] — the customer uploaded clips into a `diaspor` VFS path
///   ahead of time and is letting us read from there.
///
/// JSON shape: adjacent-tagged with `kind` + `value`, so an S3 source becomes
/// `{ "kind": "s3_prefix", "value": "..." }` and a VFS source becomes
/// `{ "kind": "vfs_path", "value": "..." }`. Adjacent tagging keeps the JSON
/// shape easy to evolve when a third source (e.g. `gcs_prefix`) is added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CorpusSource {
    /// `s3://bucket/prefix/...` shaped reference into a tenant-owned bucket. The
    /// inner string is the prefix; bucket + region are resolved from the tenant's
    /// configuration in the control plane.
    S3Prefix(String),
    /// A `diaspor` VFS path the tenant has uploaded clips into. Useful for on-prem
    /// deployments where the customer doesn't run S3 themselves.
    VfsPath(PathBuf),
}

/// Hashed summary of a corpus, produced by [`CorpusIngest::ingest`].
///
/// Cheap to clone and to serialize into the eventual [`crate::adapter::AdapterArtifact`]
/// metadata so a deployed adapter can be traced back to the exact corpus that produced
/// it. Auditors verify that `sha256_manifest` matches by re-walking the source under the
/// tenant's read credentials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct CorpusManifest {
    /// Tenant this corpus belongs to. The ingest backend MUST refuse to mix corpora
    /// from different tenants in one manifest.
    pub tenant_id: TenantId,
    /// Number of clips discovered in the source.
    pub clip_count: u64,
    /// Total clip duration in seconds, summed across the corpus.
    pub total_duration_seconds: f64,
    /// SHA-256 of the canonicalized clip list (sorted, newline-joined, full paths).
    /// Hex-encoded, lowercase. Empty in the alpha stub.
    pub sha256_manifest: String,
    /// Wall-clock time the ingest walk finished, in UTC.
    #[serde(with = "time::serde::rfc3339")]
    pub ingested_at: OffsetDateTime,
}

/// Walks a [`CorpusSource`] and produces a [`CorpusManifest`].
///
/// Implementations MUST be idempotent for a given `(TenantId, CorpusSource)` pair —
/// re-running ingest over the same source produces a manifest with the same
/// `sha256_manifest`. Mutation of the source under our feet is not the ingest layer's
/// problem; the eval gate will catch any drift.
#[async_trait]
pub trait CorpusIngest: Send + Sync {
    /// Human-readable name of the backend, for logs and provenance records.
    fn name(&self) -> &'static str;

    /// Walks `source` under `tenant_id`'s scope and returns its manifest.
    async fn ingest(&self, tenant_id: &TenantId, source: &CorpusSource) -> Result<CorpusManifest>;
}

/// No-op corpus ingest used for trait-surface scaffolding and tests.
///
/// Always returns [`TrainError::NotImplemented`] wrapped into a [`VfsError::Backend`].
/// Replace with the S3 + VFS walking implementation in milestone M9.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopCorpusIngest;

#[async_trait]
impl CorpusIngest for NoopCorpusIngest {
    fn name(&self) -> &'static str {
        "noop-corpus"
    }

    async fn ingest(
        &self,
        _tenant_id: &TenantId,
        _source: &CorpusSource,
    ) -> Result<CorpusManifest> {
        Err(VfsError::from(TrainError::NotImplemented {
            stage: "noop-corpus",
        }))
    }
}
