//! The [`InferenceBackend`] trait and the three concrete backends.
//!
//! - `TritonInferenceBackend` — stub until M8 (gRPC client lands).
//! - `CoreMLInferenceBackend` — stub until M7 (`coreml-rs` integration lands).
//! - `OrtCpuInferenceBackend` — real, behind the `ort-cpu` feature. Without the feature,
//!   the struct still exists (so the trait surface is unchanged for callers that bind to it
//!   abstractly) but `run` returns `NotImplemented`.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use diaspor_core::Result;

use crate::InferError;
use crate::tenant::{AdapterId, ModelId};
use crate::tensor::TensorBatch;

/// The single trait every inference runtime implements.
///
/// Implementors are responsible for translating [`TensorBatch`] inputs into whatever the
/// underlying runtime wants (gRPC protobufs for Triton, `MLMultiArray` for `CoreML`, ORT
/// tensors for ONNX Runtime) and back. The trait is intentionally lean: routing,
/// observability, and tenant resolution all live in [`crate::InferencePipeline`].
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Human-readable name of the backend, for logs and metrics labels.
    fn name(&self) -> &'static str;

    /// Runs inference against `model`, optionally with the `LoRA` adapter `adapter`.
    ///
    /// Implementations should populate the returned batch's `latency_us` field; the
    /// [`crate::InferencePipeline`] also stamps an overall wall-clock latency so dashboards
    /// can separate backend time from pipeline overhead.
    async fn run(
        &self,
        model: &ModelId,
        adapter: Option<&AdapterId>,
        inputs: TensorBatch,
    ) -> Result<TensorBatch>;
}

// ---------------------------------------------------------------------------
// Triton
// ---------------------------------------------------------------------------

/// Configuration for the NVIDIA Triton Inference Server backend.
///
/// Triton is the production GPU backend. The real implementation will use `tonic` to talk
/// to Triton's gRPC API, supports model versioning, ensembles, dynamic batching, and
/// per-tenant `LoRA` adapters through Triton's multi-`LoRA` feature.
#[derive(Debug, Clone)]
pub struct TritonConfig {
    /// gRPC endpoint of the Triton server, e.g. `"http://triton.svc.cluster.local:8001"`.
    ///
    /// Kept as `String` rather than a `url::Url` to avoid pulling in the `url` crate before
    /// it is actually needed; the real backend will parse and validate this on construction
    /// once the gRPC client lands.
    pub endpoint: String,
    /// Maximum wall-clock time to wait for one inference call before returning
    /// [`crate::InferError::Timeout`].
    pub timeout: Duration,
}

/// Stub backend for NVIDIA Triton Inference Server.
///
/// Wires up in milestone M8 (live path). Today this backend returns
/// [`crate::InferError::NotImplemented`] from every call so the trait surface is exercisable
/// without dragging in `tonic` and the rest of the gRPC stack.
#[derive(Debug, Clone)]
pub struct TritonInferenceBackend {
    /// gRPC endpoint + timeout for the future real client.
    pub config: TritonConfig,
}

impl TritonInferenceBackend {
    /// Constructs a stub Triton backend from its config.
    #[must_use]
    pub const fn new(config: TritonConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl InferenceBackend for TritonInferenceBackend {
    fn name(&self) -> &'static str {
        "triton"
    }

    async fn run(
        &self,
        _model: &ModelId,
        _adapter: Option<&AdapterId>,
        _inputs: TensorBatch,
    ) -> Result<TensorBatch> {
        Err(InferError::NotImplemented { backend: "triton" }.into())
    }
}

// ---------------------------------------------------------------------------
// CoreML
// ---------------------------------------------------------------------------

/// Configuration for the Apple `CoreML` backend.
///
/// Targets Apple Silicon's Neural Engine for on-device inference; used for laptop demos,
/// edge deployments, and any environment where shipping a Triton cluster is overkill.
#[derive(Debug, Clone)]
pub struct CoreMLConfig {
    /// Filesystem path to a compiled `.mlmodelc` directory or `.mlmodel` file.
    pub mlmodel_path: PathBuf,
}

/// Stub backend for Apple `CoreML`.
///
/// Wires up in milestone M7 (batch path). Today this backend returns
/// [`crate::InferError::NotImplemented`] from every call so the trait surface is exercisable
/// without dragging in `coreml-rs` / Objective-C bridging.
#[derive(Debug, Clone)]
pub struct CoreMLInferenceBackend {
    /// Filesystem path to the model file the future real backend will load.
    pub config: CoreMLConfig,
}

impl CoreMLInferenceBackend {
    /// Constructs a stub `CoreML` backend from its config.
    #[must_use]
    pub const fn new(config: CoreMLConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl InferenceBackend for CoreMLInferenceBackend {
    fn name(&self) -> &'static str {
        "coreml"
    }

    async fn run(
        &self,
        _model: &ModelId,
        _adapter: Option<&AdapterId>,
        _inputs: TensorBatch,
    ) -> Result<TensorBatch> {
        Err(InferError::NotImplemented { backend: "coreml" }.into())
    }
}

// ---------------------------------------------------------------------------
// ONNX Runtime (CPU)
// ---------------------------------------------------------------------------

/// Configuration for the ONNX Runtime CPU backend.
///
/// The portable fallback. Runs anywhere `x86_64` or `aarch64` Linux, macOS, or Windows
/// runs. EP selection (CPU / `CUDA` / `TensorRT` / `CoreML`) is decided at runtime via the `ort`
/// crate's execution-provider knobs; the same compiled binary can target any EP at runtime
/// as long as the right libonnxruntime is on the loader search path
/// (we build with `load-dynamic`).
#[derive(Debug, Clone)]
pub struct OrtCpuConfig {
    /// Filesystem path to the `.onnx` model file. Resolve catalog ids to file paths via
    /// [`crate::ModelHub`].
    pub onnx_path: PathBuf,
    /// Number of intra-op threads to give the runtime. Set to the number of physical cores
    /// on the inference machine, not logical cores: SMT consistently hurts throughput on
    /// dense matmul kernels.
    pub threads: usize,
}

/// ONNX Runtime CPU backend.
///
/// With the `ort-cpu` feature on, this is a real backend that loads the `.onnx` file at
/// `config.onnx_path` and runs inference via `ort` 2.x. Without the feature, the struct
/// still exists for trait-object compatibility but `run` returns `NotImplemented`.
///
/// Currently supports F32 inputs only — Phase 2 adapters that need F16 / INT8 will add
/// conversion paths here as they land.
pub struct OrtCpuInferenceBackend {
    /// Model path + thread count.
    pub config: OrtCpuConfig,
    #[cfg(feature = "ort-cpu")]
    session: parking_lot::Mutex<ort::session::Session>,
}

impl std::fmt::Debug for OrtCpuInferenceBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrtCpuInferenceBackend")
            .field("config", &self.config)
            // The wrapped session intentionally elides its own contents — ort::Session does
            // not implement Debug, and exposing the raw graph would be both useless to a log
            // reader and risky once we host customer LoRAs.
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "ort-cpu")]
impl OrtCpuInferenceBackend {
    /// Constructs a real ORT-CPU backend by loading the ONNX file at `config.onnx_path`.
    ///
    /// # Errors
    ///
    /// Returns [`InferError::ModelLoad`] when ORT cannot parse the model (missing file,
    /// schema mismatch, malformed bytes, …).
    pub fn new(config: OrtCpuConfig) -> std::result::Result<Self, InferError> {
        use ort::session::Session;
        let mut builder = Session::builder().map_err(|e| InferError::ModelLoad {
            backend: "ort-cpu",
            reason: format!("Session::builder: {e}"),
        })?;
        if config.threads > 0 {
            builder =
                builder
                    .with_intra_threads(config.threads)
                    .map_err(|e| InferError::ModelLoad {
                        backend: "ort-cpu",
                        reason: format!("with_intra_threads: {e}"),
                    })?;
        }
        let session =
            builder
                .commit_from_file(&config.onnx_path)
                .map_err(|e| InferError::ModelLoad {
                    backend: "ort-cpu",
                    reason: format!("commit_from_file({}): {e}", config.onnx_path.display()),
                })?;
        Ok(Self {
            config,
            session: parking_lot::Mutex::new(session),
        })
    }
}

#[cfg(not(feature = "ort-cpu"))]
impl OrtCpuInferenceBackend {
    /// Constructs a stub ORT-CPU backend. With the `ort-cpu` feature off, `run` returns
    /// `NotImplemented`; the constructor is infallible.
    pub const fn new(config: OrtCpuConfig) -> std::result::Result<Self, InferError> {
        Ok(Self { config })
    }
}

#[async_trait]
impl InferenceBackend for OrtCpuInferenceBackend {
    fn name(&self) -> &'static str {
        "ort-cpu"
    }

    #[cfg(not(feature = "ort-cpu"))]
    async fn run(
        &self,
        _model: &ModelId,
        _adapter: Option<&AdapterId>,
        _inputs: TensorBatch,
    ) -> Result<TensorBatch> {
        Err(InferError::NotImplemented { backend: "ort-cpu" }.into())
    }

    #[cfg(feature = "ort-cpu")]
    // `outputs` borrows from the `session` MutexGuard, so the guard has to live until
    // the F32 extraction loop completes — clippy's significant_drop_tightening can't
    // see through that lifetime constraint.
    #[allow(clippy::significant_drop_tightening)]
    async fn run(
        &self,
        model: &ModelId,
        _adapter: Option<&AdapterId>,
        batch: TensorBatch,
    ) -> Result<TensorBatch> {
        use std::time::Instant;

        use ndarray::{ArrayD, IxDyn};
        use ort::value::Tensor as OrtTensor;

        use crate::tensor::{DType, Tensor};

        // Convert each input tensor to an ORT Value. Phase 1 supports F32 only — Phase 2
        // adapters that need F16 / INT8 add conversion paths here.
        let mut ort_inputs: Vec<(String, OrtTensor<f32>)> = Vec::with_capacity(batch.tensors.len());
        for t in &batch.tensors {
            if !t.is_well_formed() {
                return Err(InferError::InvalidInput {
                    model: model.to_string(),
                    reason: format!(
                        "tensor {} byte length does not match shape {:?} * dtype {}",
                        t.name,
                        t.shape,
                        t.dtype.name(),
                    ),
                }
                .into());
            }
            if !matches!(t.dtype, DType::F32) {
                return Err(InferError::InvalidInput {
                    model: model.to_string(),
                    reason: format!(
                        "tensor {} has dtype {}; only F32 is supported in v0.1 ort-cpu",
                        t.name,
                        t.dtype.name(),
                    ),
                }
                .into());
            }
            let floats = bytes_to_f32_vec(&t.bytes);
            let array = ArrayD::from_shape_vec(IxDyn(&t.shape), floats).map_err(|e| {
                InferError::InvalidInput {
                    model: model.to_string(),
                    reason: format!("ndarray::from_shape_vec({:?}): {e}", t.shape),
                }
            })?;
            let value = OrtTensor::from_array(array).map_err(|e| InferError::BackendFailure {
                backend: "ort-cpu",
                reason: format!("OrtTensor::from_array: {e}"),
            })?;
            ort_inputs.push((t.name.clone(), value));
        }

        let started = Instant::now();
        let mut session = self.session.lock();
        let outputs = session
            .run(ort_inputs)
            .map_err(|e| InferError::BackendFailure {
                backend: "ort-cpu",
                reason: format!("Session::run: {e}"),
            })?;
        let latency_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);

        // Pull every output back to a F32 Tensor. We only handle F32 outputs here, again
        // for Phase 1 simplicity; Phase 2 will need F16 / INT8.
        // ort 2.x's `try_extract_tensor` returns `(&Shape, &[T])`.
        let mut out_tensors = Vec::with_capacity(outputs.len());
        for (name, value) in &outputs {
            let (shape, data) =
                value
                    .try_extract_tensor::<f32>()
                    .map_err(|e| InferError::BackendFailure {
                        backend: "ort-cpu",
                        reason: format!("try_extract_tensor::<f32>({name}): {e}"),
                    })?;
            let shape_vec: Vec<usize> = shape
                .iter()
                .copied()
                .map(|d| usize::try_from(d).unwrap_or(0))
                .collect();
            let bytes = f32_vec_to_bytes(data.to_vec());
            out_tensors.push(Tensor::new(name.to_string(), shape_vec, DType::F32, bytes));
        }
        Ok(TensorBatch {
            tensors: out_tensors,
            latency_us,
        })
    }
}

#[cfg(feature = "ort-cpu")]
fn bytes_to_f32_vec(bytes: &bytes::Bytes) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(feature = "ort-cpu")]
fn f32_vec_to_bytes(values: Vec<f32>) -> bytes::Bytes {
    let mut buf = bytes::BytesMut::with_capacity(values.len() * 4);
    for v in values {
        buf.extend_from_slice(&v.to_ne_bytes());
    }
    buf.freeze()
}
