//! Eval gate — block adapter handoff unless it beats the baseline.
//!
//! Every adapter produced by the custom-tier pipeline is run against a tenant-supplied
//! held-out set BEFORE the vendor signature is countersigned by the tenant and BEFORE
//! the adapter is loaded into a serving slot. If `EvalReport::passed` is `false`, the
//! orchestrating pipeline drops the artifact, logs the regression, and surfaces an
//! actionable error to the tenant.
//!
//! The metric itself is opaque — the eval backend picks something appropriate to the
//! `LoRA` target (AUC for credibility, mean-squared-error for judging, top-1 for sport
//! classification) and returns it as a `(metric_name, baseline_score, new_score)` tuple
//! plus a pre-computed `delta` and a `passed` bit. The pipeline does not interpret the
//! number; it just gates on the bit.
//!
//! The alpha ships a single [`NoopEvalGate`] backend that returns
//! [`crate::TrainError::NotImplemented`].

use std::path::Path;

use async_trait::async_trait;
use diaspor_core::{Result, VfsError};
use serde::{Deserialize, Serialize};

use crate::TrainError;
use crate::adapter::AdapterArtifact;

/// Outcome of one eval-gate run.
///
/// `delta` is `new_score - baseline_score` and is pre-computed by the backend so the
/// pipeline doesn't have to know the metric's directionality (higher-is-better vs
/// lower-is-better — the `passed` bit encodes the gate's verdict either way).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct EvalReport {
    /// Name of the metric the backend chose (e.g. `"auc_roc"`, `"mse"`, `"top1"`).
    pub metric_name: String,
    /// Score of the un-adapted base model on the held-out set.
    pub baseline_score: f64,
    /// Score of the LoRA-adapted model on the held-out set.
    pub new_score: f64,
    /// `new_score - baseline_score`. Sign convention follows the metric.
    pub delta: f64,
    /// `true` if the backend's policy says the new score clears the gate.
    pub passed: bool,
}

/// Runs evaluation against a held-out set and produces an [`EvalReport`].
///
/// The `held_out_set` path is opaque — it can be an S3 prefix, a VFS path, or a local
/// directory; the backend picks how to read it. The orchestrating pipeline does not
/// inspect the contents.
#[async_trait]
pub trait EvalGate: Send + Sync {
    /// Human-readable name of the backend, for logs and adapter provenance.
    fn name(&self) -> &'static str;

    /// Evaluates `adapter` against `held_out_set` and returns the verdict.
    async fn evaluate(
        &self,
        adapter: &AdapterArtifact,
        held_out_set: &Path,
    ) -> Result<EvalReport>;
}

/// No-op eval gate used for trait-surface scaffolding and tests.
///
/// Always returns [`TrainError::NotImplemented`] wrapped into a [`VfsError::Backend`].
/// Replace with the real eval harness in milestone M9.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopEvalGate;

#[async_trait]
impl EvalGate for NoopEvalGate {
    fn name(&self) -> &'static str {
        "noop-eval"
    }

    async fn evaluate(
        &self,
        _adapter: &AdapterArtifact,
        _held_out_set: &Path,
    ) -> Result<EvalReport> {
        Err(VfsError::from(TrainError::NotImplemented {
            stage: "noop-eval-gate",
        }))
    }
}
