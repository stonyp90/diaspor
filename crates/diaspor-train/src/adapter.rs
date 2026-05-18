//! Adapter artifact + identifier newtypes for the custom-tier training pipeline.
//!
//! The [`AdapterArtifact`] is the load-bearing handoff object from `diaspor-train` to the
//! tenant-facing storage layer. It bundles:
//!
//! 1. The opaque safetensors bytes of the trained `LoRA` delta.
//! 2. The eval report that gated the artifact through the [`crate::eval::EvalGate`].
//! 3. Two Ed25519 signatures — one minted by the vendor (`diaspor` `SaaS`) and one
//!    optionally minted by the tenant once they accept the handoff.
//!
//! Both signatures are modeled as opaque [`bytes::Bytes`] placeholders at the alpha
//! stage; no cryptographic dependency is pulled in until the milestone M9 wiring lands.
//! See the crate-level docs for the "weights stay yours" invariant the dual signature
//! is meant to enforce.
//!
//! [`TenantId`] and [`AdapterId`] are defined locally here so this crate compiles
//! independently of `diaspor-infer`. The `TODO(consolidation)` markers above each one
//! flag the future hoist into `diaspor-core`.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::eval::EvalReport;

// TODO(consolidation): unify with diaspor-infer::TenantId in diaspor-core
/// Identifier of a tenant inside a multi-tenant `diaspor` deployment.
///
/// Newtype over `String` so call sites cannot accidentally pass an [`AdapterId`] where a
/// [`TenantId`] is expected. The inner string is opaque — typically a `cust_<uuid>`
/// value minted by the control plane. Defined locally for now so this crate can compile
/// independently of `diaspor-infer`; both newtypes wrap the same `String` shape and will
/// be lifted into `diaspor-core` once the cross-crate contract is settled.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TenantId(String);

impl TenantId {
    /// Constructs a tenant identifier from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// TODO(consolidation): unify with diaspor-infer::AdapterId in diaspor-core
/// Identifier of a `LoRA` adapter trained for a given tenant + base-model combination.
///
/// Stable across the adapter's lifetime: minted when training starts, stamped into the
/// safetensors metadata, used as the path component in the tenant's S3 bucket, and
/// referenced from the inference layer when the adapter is loaded for serving. Defined
/// locally for now so this crate can compile independently of `diaspor-infer`; will be
/// lifted into `diaspor-core` alongside [`TenantId`] once the cross-crate contract is
/// settled.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AdapterId(String);

impl AdapterId {
    /// Constructs an adapter identifier from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AdapterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A trained `LoRA` adapter packaged for handoff to a tenant.
///
/// Produced by [`crate::TrainingPipeline::train`] after a successful eval-gate pass. The
/// artifact is dual-signed: the vendor signs first to attest that the training pipeline
/// produced these exact bytes from the declared corpus, and the tenant counter-signs on
/// acceptance to attest custody. Until [`Self::tenant_signature`] is `Some`, the artifact
/// is in *vendor-signed-only* state and SHOULD NOT be loaded into a serving slot.
///
/// `safetensors_bytes` is opaque — the alpha scaffolding does not pull in the
/// `safetensors` crate. Real bytes land in milestone M9.
///
/// JSON shape: the three `Bytes` fields (`safetensors_bytes`, `vendor_signature`,
/// optional `tenant_signature`) serialize via `bytes/serde` — at the alpha they
/// become byte arrays in JSON, which keeps the round-trip lossless. The
/// production envelope will likely wrap this in a versioned "handoff" record that
/// base64-encodes the safetensors blob and signs the canonical JSON; that
/// envelope lives outside this crate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct AdapterArtifact {
    /// Stable identifier minted when training started.
    pub adapter_id: AdapterId,
    /// Tenant the adapter was trained for. Encoded into the S3 path (see
    /// [`Self::path_in_tenant_bucket`]) and into the safetensors metadata so a leaked
    /// artifact is attributable.
    pub tenant_id: TenantId,
    /// Name of the foundation backbone the `LoRA` was trained over (e.g.
    /// `"InternVideo2-1B"`, `"VideoMAE-v2-Base"`, `"HuBERT-Large"`). The serving layer
    /// uses this string to pick the correct base model when loading the adapter.
    pub base_model: String,
    /// Opaque safetensors-formatted bytes of the `LoRA` delta. Placeholder in the alpha —
    /// real serialization lands in M9.
    pub safetensors_bytes: Bytes,
    /// Vendor-side Ed25519 signature over `safetensors_bytes`. Placeholder bytes at the
    /// alpha; the real signing key lives in the control-plane KMS.
    pub vendor_signature: Bytes,
    /// Tenant-side Ed25519 signature, present once the tenant has accepted the handoff.
    /// `None` while the artifact is in vendor-signed-only state.
    #[serde(default)]
    pub tenant_signature: Option<Bytes>,
    /// Wall-clock time training finished, in UTC.
    #[serde(with = "time::serde::rfc3339")]
    pub trained_at: OffsetDateTime,
    /// Eval report that gated this artifact through [`crate::eval::EvalGate`]. `None`
    /// only for artifacts constructed in tests where eval was bypassed.
    #[serde(default)]
    pub eval_report: Option<EvalReport>,
}

impl AdapterArtifact {
    /// Returns the canonical S3 key shape this artifact lives at inside the tenant's
    /// bucket.
    ///
    /// Format: `tenants/{tenant_id}/adapters/{adapter_id}.safetensors`. The serving layer
    /// uses this exact shape to resolve an adapter from a `(TenantId, AdapterId)` pair
    /// without an extra database round-trip.
    #[must_use]
    pub fn path_in_tenant_bucket(tenant_id: &TenantId, adapter_id: &AdapterId) -> String {
        format!(
            "tenants/{tenant}/adapters/{adapter}.safetensors",
            tenant = tenant_id.as_str(),
            adapter = adapter_id.as_str(),
        )
    }
}
