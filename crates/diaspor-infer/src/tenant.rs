//! Tenant, model, and adapter identifiers used to route inference requests.
//!
//! `diaspor` production deployments serve many tenants from a single inference cluster.
//! Each `(TenantId, ModelId)` pair resolves to a base model on the backend and, optionally,
//! a tenant-specific `LoRA` adapter ([`AdapterId`]). The mapping lives in the
//! [`crate::InferencePipeline`] — the backend itself sees only opaque strings.
//!
//! All three newtypes serialize **transparently** as JSON strings so that downstream
//! schemas (e.g. `docs/schema/score-v1.json`'s `tenant` and `adapter_id` fields) see
//! exactly the inner `String` shape they expect.

use serde::{Deserialize, Serialize};

/// Identifier of a tenant inside a multi-tenant `diaspor` deployment.
///
/// Newtype over `String` rather than a raw `String` so call sites cannot accidentally pass
/// a [`ModelId`] where a [`TenantId`] is expected. The string itself is opaque — it is
/// typically a `cust_<uuid>` value minted by the control plane, but the inference layer
/// makes no claim about its shape.
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

/// Identifier of a model registered with the inference backend.
///
/// On Triton, this is the model name as it appears in the model repository. On `CoreML` and
/// ONNX Runtime, it is the logical name the [`crate::InferencePipeline`] maps to a concrete
/// `.mlmodel` / `.onnx` file. Versions are *not* encoded here — version selection lives in
/// the backend config and is opaque to call sites.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(String);

impl ModelId {
    /// Constructs a model identifier from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identifier of a `LoRA` adapter registered for a tenant + model combination.
///
/// Adapters let one base model serve tenant-specific fine-tunes without paying for a full
/// model load per tenant. Triton resolves these by name through its multi-`LoRA` support;
/// `CoreML` and ONNX backends will fold the adapter weights into the base graph at load
/// time in a future milestone.
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
