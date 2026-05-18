//! The composed [`InferencePipeline`] that holds one backend and a tenant→adapter table.
//!
//! Call sites build a pipeline once, then call `infer()` per request. Routing, adapter
//! resolution, and overall-wall-clock latency stamping all live here so backends stay lean.

use std::collections::HashMap;
use std::time::Instant;

use diaspor_core::Result;
use parking_lot::RwLock;

use crate::backend::InferenceBackend;
use crate::tenant::{AdapterId, ModelId, TenantId};
use crate::tensor::TensorBatch;

/// Generic inference pipeline parameterized over a single backend.
///
/// The pipeline owns:
/// - one [`InferenceBackend`] (Triton, `CoreML`, or ONNX CPU today),
/// - a `(TenantId, ModelId) → AdapterId` lookup table for `LoRA`-per-tenant routing,
/// - and a `latency_us` stamp on every output [`TensorBatch`] for SLO observability.
///
/// The table is wrapped in a `parking_lot::RwLock` so adapters can be registered or removed
/// at runtime (e.g. when a new tenant onboards) without taking the pipeline out of
/// circulation.
pub struct InferencePipeline<B>
where
    B: InferenceBackend,
{
    /// The wrapped backend.
    pub backend: B,
    /// `(tenant, model) → adapter` routing table. Absence of an entry means "no adapter,
    /// use the bare base model" — that is the common case, so absence is not an error.
    adapters: RwLock<HashMap<(TenantId, ModelId), AdapterId>>,
}

impl<B> InferencePipeline<B>
where
    B: InferenceBackend,
{
    /// Constructs a pipeline around `backend` with an empty adapter table.
    #[must_use]
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            adapters: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a `LoRA` adapter for a `(tenant, model)` pair.
    ///
    /// Replaces any previously registered adapter for the same pair. Returns the
    /// adapter that was displaced, if any.
    pub fn register_adapter(
        &self,
        tenant: TenantId,
        model: ModelId,
        adapter: AdapterId,
    ) -> Option<AdapterId> {
        self.adapters.write().insert((tenant, model), adapter)
    }

    /// Removes a `LoRA` adapter registration. Returns the adapter that was removed, if any.
    pub fn unregister_adapter(&self, tenant: &TenantId, model: &ModelId) -> Option<AdapterId> {
        self.adapters
            .write()
            .remove(&(tenant.clone(), model.clone()))
    }

    /// Returns the adapter, if any, currently registered for `(tenant, model)`.
    pub fn adapter_for(&self, tenant: &TenantId, model: &ModelId) -> Option<AdapterId> {
        self.adapters
            .read()
            .get(&(tenant.clone(), model.clone()))
            .cloned()
    }

    /// Runs inference for `(tenant, model)` on `inputs`.
    ///
    /// Resolves any tenant-specific `LoRA` adapter, delegates to the backend, and stamps the
    /// total wall-clock latency (in microseconds) on the returned batch so downstream SLO
    /// dashboards can read it without instrumenting every call site.
    ///
    /// # Errors
    ///
    /// Bubbles up whatever the backend returns. v0.1.0-alpha backends always return
    /// [`crate::InferError::NotImplemented`].
    pub async fn infer(
        &self,
        tenant: &TenantId,
        model: &ModelId,
        inputs: TensorBatch,
    ) -> Result<TensorBatch> {
        let adapter = self.adapter_for(tenant, model);
        let started = Instant::now();
        let mut output = self.backend.run(model, adapter.as_ref(), inputs).await?;
        let elapsed_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        // Keep whichever latency is larger: backend-reported (if any) vs pipeline-observed.
        // The pipeline-observed number is the one customers see, so prefer it on ties.
        output.latency_us = output.latency_us.max(elapsed_us);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use bytes::Bytes;

    use super::*;
    use crate::InferError;
    use crate::backend::{CoreMLConfig, CoreMLInferenceBackend};
    use crate::tensor::{DType, Tensor};

    #[tokio::test]
    async fn coreml_pipeline_returns_not_implemented() {
        let backend = CoreMLInferenceBackend::new(CoreMLConfig {
            mlmodel_path: PathBuf::from("/tmp/diaspor-test-not-a-real-model.mlmodelc"),
        });
        let pipeline = InferencePipeline::new(backend);

        let tenant = TenantId::new("test-tenant");
        let model = ModelId::new("test-model");
        let inputs = TensorBatch::new(vec![Tensor::new(
            "pixel_values",
            vec![1, 3, 224, 224],
            DType::F32,
            Bytes::from_static(&[0u8; 4]),
        )]);

        let err = pipeline
            .infer(&tenant, &model, inputs)
            .await
            .expect_err("stub backend must error");

        // Bubbles up through diaspor_core::VfsError::Backend(string).
        let msg = err.to_string();
        assert!(
            msg.contains("coreml"),
            "expected error to mention coreml backend, got: {msg}",
        );
        assert!(
            msg.contains("not implemented") || msg.contains("backend not implemented"),
            "expected NotImplemented marker in error, got: {msg}",
        );

        // Also sanity-check that an unwrapped InferError formats the same way: this is
        // the contract higher layers rely on.
        let inner = InferError::NotImplemented { backend: "coreml" };
        assert!(inner.to_string().contains("coreml"));
    }
}
