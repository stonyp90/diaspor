//! # diaspor-frame-pipeline
//!
//! Video frame decoding and sampling primitives for `diaspor-vision`.
//!
//! This crate sits between raw on-disk (or in-VFS) video files and the model-serving
//! layer that consumes decoded frames. It separates two concerns that real-world
//! pipelines tend to conflate:
//!
//! 1. **Decoding** — turning a container + codec into pixel data. Pluggable per platform
//!    via the [`DecodeBackend`] trait (`FFmpeg` subprocess on every host, NVIDIA `DeepStream`
//!    for GPU-colocated decode + inference on Linux/CUDA, Apple `VideoToolbox` for
//!    hardware decode on Apple Silicon).
//! 2. **Sampling** — deciding *which* of the decoded frames are worth handing to a
//!    downstream model. Pluggable via the [`FrameSampler`] trait (uniform fps targeting,
//!    keyframe-only, face-triggered, ...).
//!
//! Both traits are async and yield a [`FrameBatchStream`] of [`FrameBatch`] values, so
//! callers can compose them with any `futures::Stream` adapter — backpressure, batching,
//! and parallel inference all stay outside this crate's responsibility.
//!
//! ## Architecture at a glance
//!
//! ```text
//!   ┌───────────────┐   raw frames   ┌────────────────┐   sampled frames  ┌─────────┐
//!   │ DecodeBackend │ ─────────────▶ │  FrameSampler  │ ────────────────▶ │  model  │
//!   │ (ffmpeg/…)    │                │ (uniform/…)    │                   │  layer  │
//!   └───────────────┘                └────────────────┘                   └─────────┘
//! ```
//!
//! Each stage emits a [`FrameBatchStream`]; the consumer drives the pipeline by polling
//! the final stream, so neither decode nor sampling does work the consumer never asked
//! for.
//!
//! ## Privacy and locality
//!
//! All backends in this crate are local — no network, no telemetry. The default
//! [`FfmpegDecodeBackend`] shells out to a user-provided `ffmpeg` binary;
//! [`DeepStreamDecodeBackend`] and [`VideoToolboxDecodeBackend`] target on-device
//! hardware. Cloud or remote decode is out of scope.
//!
//! ## Status
//!
//! v0.1.0-alpha ships the trait surface only. `FFmpeg` subprocess wiring lands in
//! milestone M7; `DeepStream` and `VideoToolbox` in M8.

#![doc(html_root_url = "https://docs.rs/diaspor-frame-pipeline/0.1.0-alpha.1")]

use thiserror::Error;

pub mod batch;
pub mod decode;
pub mod sampler;

pub use batch::{FrameBatch, FrameBatchStream, PixelFormat};
pub use decode::{
    DecodeBackend, DeepStreamDecodeBackend, FfmpegDecodeBackend, VideoToolboxDecodeBackend,
};
pub use sampler::{
    FaceTriggeredFrameSampler, FrameSampler, KeyframeAlignedFrameSampler, UniformFrameSampler,
};

/// Things that can go wrong specifically inside the frame pipeline.
///
/// Wraps cleanly into a [`diaspor_core::VfsError::Backend`] when bubbled up through the
/// VFS layer.
#[derive(Debug, Error)]
pub enum FramePipelineError {
    /// The underlying decoder binary or library is missing, mis-versioned, or refused
    /// to start.
    #[error("decoder unavailable ({backend}): {reason}")]
    DecoderUnavailable {
        /// Name of the backend that failed to initialize (`ffmpeg`, `deepstream`, ...).
        backend: &'static str,
        /// Human-readable failure reason.
        reason: String,
    },

    /// The decoder started but failed mid-stream (codec error, truncated file, ...).
    #[error("decode failed ({backend}): {reason}")]
    DecodeFailed {
        /// Name of the backend that produced the failure.
        backend: &'static str,
        /// Human-readable failure reason.
        reason: String,
    },

    /// The container or codec is not something the selected backend can handle.
    #[error("unsupported codec '{codec}' on backend '{backend}'")]
    UnsupportedCodec {
        /// Name of the backend rejecting the codec.
        backend: &'static str,
        /// `FourCC` / codec name as reported by the probe step.
        codec: String,
    },

    /// The sampler rejected its input (bad config, missing keyframe data, ...).
    #[error("sampler failed: {reason}")]
    SamplerFailed {
        /// Human-readable failure reason.
        reason: String,
    },

    /// A backend method exists in the trait but has not been wired up yet for this
    /// implementation. Used by the v0.1.0-alpha stubs.
    #[error("backend '{backend}' is not implemented yet")]
    NotImplemented {
        /// Name of the unfinished backend.
        backend: &'static str,
    },
}

impl From<FramePipelineError> for diaspor_core::VfsError {
    fn from(err: FramePipelineError) -> Self {
        Self::Backend(err.to_string())
    }
}
