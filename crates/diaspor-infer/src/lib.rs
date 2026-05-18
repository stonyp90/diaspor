//! # diaspor-infer
//!
//! Multi-runtime ML inference abstraction for `diaspor`.
//!
//! This crate defines the backend-agnostic trait surface for running inference against the
//! non-verbal-AI models that power `diaspor`'s vision, prosody, and judgement pipelines.
//! A single [`InferenceBackend`] trait abstracts over the three runtimes the project
//! targets, so a single [`InferencePipeline`] can be re-pointed from one to another without
//! touching call sites:
//!
//! - **NVIDIA Triton Inference Server** (`TritonInferenceBackend`) — production GPU
//!   inference over gRPC, model versioning, ensembles, dynamic batching, multi-tenancy via
//!   per-tenant `LoRA` adapters. v0.1: stub.
//! - **Apple `CoreML`** (`CoreMLInferenceBackend`) — on-device inference on Apple Silicon,
//!   for laptops, edge deployments, and demo / CI machines. v0.1: stub.
//! - **ONNX Runtime CPU** (`OrtCpuInferenceBackend`) — real, behind the `ort-cpu` feature.
//!   Wraps `ort` 2.x with `load-dynamic` so users can swap in `CUDA` / `TensorRT` / `CoreML` EP
//!   variants of libonnxruntime without rebuilding.
//!
//! Plus a [`ModelHub`] that resolves catalog ids (e.g. `mediapipe-pose-heavy@1`) to local
//! ONNX file paths, downloading and sha256-verifying on first use. Behind the `hub` feature.
//!
//! ## Architecture at a glance
//!
//! ```text
//!     ┌───────────────────────────┐         ┌───────────────────────┐
//!     │   InferencePipeline<B>    │         │       ModelHub        │
//!     │                           │ ◀────── │  models.toml catalog  │
//!     │   tenant + model routing  │   path  │  ~/.diaspor/models/   │
//!     │   TensorBatch in/out      │         │  sha256-verified      │
//!     │   latency_us telemetry    │         │  offline-blockable    │
//!     └─────────────┬─────────────┘         └───────────────────────┘
//!                   │
//!         ┌─────────┼─────────┬──────────────────┐
//!         ▼         ▼         ▼                  ▼
//!     ┌───────┐ ┌───────┐ ┌─────────┐      (future: TensorRT-LLM,
//!     │Triton │ │CoreML │ │ORT (CPU │       DeepStream colocated)
//!     │ stub  │ │ stub  │ │+CUDA EP)│
//!     └───────┘ └───────┘ └─────────┘
//! ```
//!
//! ## Privacy contract
//!
//! When `DIASPOR_OFFLINE=1` is set in the environment, the hub refuses to download any model
//! not already cached locally. The `no-network` job in `.github/workflows/ci-rust.yml` runs
//! the entire test suite inside an `unshare(--net)` sandbox to guarantee structurally that
//! no default code path opens a socket.

#![doc(html_root_url = "https://docs.rs/diaspor-infer/0.1.0-alpha.1")]

use thiserror::Error;

pub mod backend;
#[cfg(feature = "hub")]
pub mod catalog;
#[cfg(feature = "hub")]
pub mod hub;
pub mod pipeline;
pub mod tenant;
pub mod tensor;

pub use backend::{
    CoreMLConfig, CoreMLInferenceBackend, InferenceBackend, OrtCpuConfig, OrtCpuInferenceBackend,
    TritonConfig, TritonInferenceBackend,
};
#[cfg(feature = "hub")]
pub use catalog::{Catalog, CatalogError, ModelRef};
#[cfg(feature = "hub")]
pub use hub::{HubConfig, HubError, ModelHub};
pub use pipeline::InferencePipeline;
pub use tenant::{AdapterId, ModelId, TenantId};
pub use tensor::{DType, Tensor, TensorBatch};

/// Things that can go wrong specifically in the inference pipeline.
///
/// Wraps cleanly into a [`diaspor_core::VfsError::Backend`] when bubbled up to a VFS-level
/// caller, but is most useful unwrapped: the higher layers of `diaspor` need to distinguish
/// between "the model rejected the input" (caller bug) and "the backend is unreachable"
/// (infra problem) for SLO accounting.
#[derive(Debug, Error)]
pub enum InferError {
    /// The requested backend has not been wired up yet. The scaffolding ships a no-op stub
    /// for each runtime so the architecture is reviewable before any real ML deps land.
    #[error("backend not implemented: {backend}")]
    NotImplemented {
        /// Human-readable name of the backend that returned this stub.
        backend: &'static str,
    },

    /// The backend rejected the input tensor shape, dtype, or count.
    #[error("invalid input for model {model}: {reason}")]
    InvalidInput {
        /// Identifier of the model that rejected the input.
        model: String,
        /// What was wrong (shape mismatch, unsupported dtype, etc.).
        reason: String,
    },

    /// The pipeline did not find a backend or adapter for the given tenant + model pair.
    #[error("no route for tenant={tenant} model={model}")]
    NoRoute {
        /// Tenant that was requested.
        tenant: String,
        /// Model that was requested.
        model: String,
    },

    /// The backend itself failed mid-inference (network error, OOM, crashed worker, …).
    #[error("backend failure ({backend}): {reason}")]
    BackendFailure {
        /// Human-readable name of the failing backend.
        backend: &'static str,
        /// Underlying failure reason.
        reason: String,
    },

    /// The inference timed out before the backend produced an output tensor.
    #[error("inference timed out after {elapsed_ms} ms on backend {backend}")]
    Timeout {
        /// Human-readable name of the timing-out backend.
        backend: &'static str,
        /// How long we waited, in milliseconds, before giving up.
        elapsed_ms: u64,
    },

    /// Loading the model file (parsing the ONNX graph, locating `CoreML` resources, …) failed.
    /// Distinct from `BackendFailure` because the fix is usually a config change rather than
    /// an infra reset.
    #[error("model load failed ({backend}): {reason}")]
    ModelLoad {
        /// Human-readable name of the backend whose model load failed.
        backend: &'static str,
        /// Underlying load failure (file missing, schema mismatch, license error, …).
        reason: String,
    },
}

impl From<InferError> for diaspor_core::VfsError {
    fn from(err: InferError) -> Self {
        Self::Backend(err.to_string())
    }
}
