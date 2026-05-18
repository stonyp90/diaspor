//! Tensor and batch types passed across the [`crate::InferenceBackend`] boundary.
//!
//! Tensors are deliberately runtime-agnostic: a [`Tensor`] is just a byte buffer plus its
//! shape, dtype, and a name. Each backend is responsible for interpreting those bytes
//! according to the model it serves (NCHW vs NHWC, normalized vs raw pixels, etc.).

use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Element type stored inside a [`Tensor`]'s byte buffer.
///
/// The set is intentionally narrow: it covers the dtypes the `diaspor` non-verbal-AI models
/// actually use (FP16 weights with FP32 activations on Triton, FP16 throughout on `CoreML`'s
/// Neural Engine, INT8 / UINT8 quantized on the ONNX Runtime CPU fallback). Add new
/// variants here when a new model needs them — do not pass arbitrary dtype strings around.
///
/// Serializes as one of `"fp16"`, `"fp32"`, `"int8"`, `"uint8"` to match the
/// `Tensor`-level dtype string convention used by Triton and ONNX Runtime — keeping
/// the JSON representation directly grep-able against those runtimes' logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DType {
    /// 16-bit floating point (IEEE 754 `binary16`). Default on Apple Neural Engine and
    /// common for vision-model activations on Triton with mixed precision.
    #[serde(rename = "fp16")]
    F16,
    /// 32-bit floating point (IEEE 754 `binary32`). Default activation dtype on CPU.
    #[serde(rename = "fp32")]
    F32,
    /// 8-bit signed integer. Used by INT8-quantized models on the ONNX Runtime CPU path.
    #[serde(rename = "int8")]
    I8,
    /// 8-bit unsigned integer. Used for raw image pixels before normalization.
    #[serde(rename = "uint8")]
    U8,
}

impl DType {
    /// Size in bytes of one element of this dtype.
    #[must_use]
    pub const fn size_bytes(self) -> usize {
        match self {
            Self::F16 => 2,
            Self::F32 => 4,
            Self::I8 | Self::U8 => 1,
        }
    }

    /// Stable human-readable name (matches Triton's dtype string convention).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::F16 => "FP16",
            Self::F32 => "FP32",
            Self::I8 => "INT8",
            Self::U8 => "UINT8",
        }
    }
}

/// A single named tensor: byte buffer + shape + dtype.
///
/// `bytes` is owned and reference-counted via [`Bytes`] so it can be cheaply shared across
/// async tasks and backend boundaries. The buffer must have exactly
/// `shape.iter().product::<usize>() * dtype.size_bytes()` bytes — backends are free to
/// reject mismatches with [`crate::InferError::InvalidInput`].
///
/// Serializes with `bytes/serde` (a base64-encoded JSON string for the buffer) so a
/// `Tensor` is round-trippable through any JSON wire boundary; downstream callers that
/// want to inspect the bytes in a denser binary format should serialize through a
/// length-prefixed codec instead of JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Tensor {
    /// Tensor name as the model expects (e.g. `"input_ids"`, `"pixel_values"`).
    pub name: String,
    /// Dimensions of the tensor in row-major order.
    pub shape: Vec<usize>,
    /// Element dtype.
    pub dtype: DType,
    /// Raw element bytes in row-major order, native endian.
    pub bytes: Bytes,
}

impl Tensor {
    /// Constructs a tensor without validating the buffer length against `shape * dtype`.
    ///
    /// Backends are expected to call [`Tensor::is_well_formed`] before reading
    /// `bytes`, but constructing a tensor remains cheap so call sites do not pay for
    /// validation they did themselves.
    #[must_use]
    pub fn new(name: impl Into<String>, shape: Vec<usize>, dtype: DType, bytes: Bytes) -> Self {
        Self {
            name: name.into(),
            shape,
            dtype,
            bytes,
        }
    }

    /// Returns `true` iff `bytes.len()` matches the product of `shape` times the dtype
    /// width. Backends should call this before doing any unsafe pointer casts.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        let elements: usize = self.shape.iter().product();
        self.bytes.len() == elements.saturating_mul(self.dtype.size_bytes())
    }

    /// Number of elements in this tensor (product of `shape`).
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.shape.iter().product()
    }
}

/// A collection of named tensors moving together across the backend boundary, plus an
/// observability sidecar.
///
/// `TensorBatch` is the input *and* the output type of [`crate::InferenceBackend::run`]:
/// most non-verbal-AI models in `diaspor` take multiple input tensors (pixel values,
/// attention mask, prompt embedding, …) and may return multiple outputs (logits, pooled
/// embedding, attention weights). Carrying them as a flat `Vec` keeps the call signature
/// stable as models evolve.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct TensorBatch {
    /// Tensors in this batch, addressable by their `name` field.
    pub tensors: Vec<Tensor>,
    /// Wall-clock time taken by the backend to produce this batch, in microseconds.
    ///
    /// Always `0` on input. Populated on output by [`crate::InferencePipeline::infer`] so
    /// SLO dashboards and per-tenant burn-rate alerts can read it without per-call-site
    /// instrumentation.
    pub latency_us: u64,
}

impl TensorBatch {
    /// Constructs a batch from a vector of tensors with `latency_us = 0`.
    #[must_use]
    pub const fn new(tensors: Vec<Tensor>) -> Self {
        Self {
            tensors,
            latency_us: 0,
        }
    }

    /// Looks up a tensor in the batch by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Tensor> {
        self.tensors.iter().find(|t| t.name == name)
    }

    /// Number of tensors in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tensors.len()
    }

    /// Returns `true` if the batch has no tensors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tensors.is_empty()
    }
}
