//! Decode backends — pluggable adapters from container bytes to a [`FrameBatchStream`].
//!
//! Three implementations ship with the crate:
//!
//! - [`FfmpegDecodeBackend`] — universal fallback, spawns an `ffmpeg` subprocess.
//! - [`DeepStreamDecodeBackend`] — NVIDIA `DeepStream` / `GStreamer` for GPU-colocated
//!   decode + inference on Linux + CUDA hosts.
//! - [`VideoToolboxDecodeBackend`] — Apple `VideoToolbox` hardware decode on Apple
//!   Silicon.
//!
//! All three are stubs in v0.1.0-alpha and return
//! [`crate::FramePipelineError::NotImplemented`] from [`DecodeBackend::decode`]. The
//! actual subprocess / FFI plumbing lands in milestone M7 (`FFmpeg`) and M8 (`DeepStream`
//! and `VideoToolbox`).

use async_trait::async_trait;
use diaspor_core::{Result, VfsPath};

use crate::FramePipelineError;
use crate::batch::FrameBatchStream;

/// A platform-specific adapter that turns container bytes into a stream of decoded
/// frames.
///
/// Implementations are expected to be cheap to construct (no decoder process should
/// start until [`Self::decode`] is called) and stateless across calls — one decode
/// session per call.
#[async_trait]
pub trait DecodeBackend: Send + Sync {
    /// Stable human-readable name of the backend, used for logs and error messages.
    fn name(&self) -> &'static str;

    /// Begins a decode session for the file at `path` whose bytes are `bytes`.
    ///
    /// Returns a stream that yields one [`crate::FrameBatch`] per decoded source
    /// frame, in presentation order. Errors mid-stream surface as
    /// `Err(FramePipelineError::DecodeFailed { .. })` items rather than aborting the
    /// whole stream, so callers can choose their own recovery policy.
    ///
    /// The `bytes` argument is borrowed; backends that need to hand the data to a
    /// subprocess or FFI layer are responsible for copying or streaming it
    /// themselves.
    async fn decode(&self, path: &VfsPath, bytes: &[u8]) -> Result<FrameBatchStream>;
}

/// Default decode backend — wraps an `ffmpeg` subprocess.
///
/// This is the Phase 1 batch-path decoder: it runs on every host that has an
/// `ffmpeg` binary on `PATH`, handles every container/codec `FFmpeg` supports, and
/// requires no GPU. The tradeoff is that frame data has to cross the
/// subprocess/pipe boundary, which is fine for the offline batch path but is the
/// reason a GPU-colocated backend exists for Phase 1.5.
///
/// # Status
///
/// Stub in v0.1.0-alpha. [`Self::decode`] returns
/// [`FramePipelineError::NotImplemented`]. Real subprocess wiring arrives in
/// milestone M7.
#[derive(Debug, Clone, Default)]
pub struct FfmpegDecodeBackend {
    /// Path to the `ffmpeg` binary. `None` means "search `PATH`". Honored by the M7
    /// implementation; ignored by the v0.1.0-alpha stub.
    pub ffmpeg_path: Option<String>,
}

impl FfmpegDecodeBackend {
    /// Constructs an `FfmpegDecodeBackend` that will resolve `ffmpeg` from the system
    /// `PATH` at decode time.
    #[must_use]
    pub const fn new() -> Self {
        Self { ffmpeg_path: None }
    }

    /// Constructs an `FfmpegDecodeBackend` pinned to a specific `ffmpeg` binary path.
    #[must_use]
    pub fn with_binary(path: impl Into<String>) -> Self {
        Self {
            ffmpeg_path: Some(path.into()),
        }
    }
}

#[async_trait]
impl DecodeBackend for FfmpegDecodeBackend {
    fn name(&self) -> &'static str {
        "ffmpeg"
    }

    async fn decode(&self, _path: &VfsPath, _bytes: &[u8]) -> Result<FrameBatchStream> {
        Err(FramePipelineError::NotImplemented { backend: "ffmpeg" }.into())
    }
}

/// GPU-colocated decode via NVIDIA `DeepStream` / `GStreamer`.
///
/// Targets Linux hosts with CUDA-capable GPUs. The win over [`FfmpegDecodeBackend`]
/// is that decoded frames stay in GPU memory and can be handed straight to a
/// downstream inference engine (`TensorRT`, Triton) without a host roundtrip — the
/// Phase 1.5 hot path for high-throughput inference.
///
/// # Status
///
/// Stub in v0.1.0-alpha. [`Self::decode`] returns
/// [`FramePipelineError::NotImplemented`]. `DeepStream` wiring arrives in milestone
/// M8.
#[derive(Debug, Clone, Default)]
pub struct DeepStreamDecodeBackend {
    /// Optional `GStreamer` pipeline override string for advanced users. `None` means
    /// "use the crate's default `nvv4l2decoder` pipeline".
    pub pipeline_override: Option<String>,
}

impl DeepStreamDecodeBackend {
    /// Constructs a `DeepStreamDecodeBackend` with the default pipeline.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pipeline_override: None,
        }
    }
}

#[async_trait]
impl DecodeBackend for DeepStreamDecodeBackend {
    fn name(&self) -> &'static str {
        "deepstream"
    }

    async fn decode(&self, _path: &VfsPath, _bytes: &[u8]) -> Result<FrameBatchStream> {
        Err(FramePipelineError::NotImplemented {
            backend: "deepstream",
        }
        .into())
    }
}

/// Hardware decode on Apple Silicon via Apple `VideoToolbox`.
///
/// Targets macOS hosts with M-series silicon. Decoded frames land in `CVPixelBuffer`
/// objects that the M8 implementation will marshal into [`crate::FrameBatch`] via a
/// zero-copy bridge where the layout allows it.
///
/// # Status
///
/// Stub in v0.1.0-alpha. [`Self::decode`] returns
/// [`FramePipelineError::NotImplemented`]. `VideoToolbox` wiring arrives in milestone
/// M8.
#[derive(Debug, Clone, Default)]
pub struct VideoToolboxDecodeBackend {
    /// Whether to request hardware decode explicitly (vs. letting `VideoToolbox`
    /// choose). Honored by the M8 implementation; ignored by the v0.1.0-alpha stub.
    pub require_hardware: bool,
}

impl VideoToolboxDecodeBackend {
    /// Constructs a `VideoToolboxDecodeBackend` that lets `VideoToolbox` pick the
    /// best available decoder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            require_hardware: false,
        }
    }

    /// Constructs a `VideoToolboxDecodeBackend` that errors out if hardware decode
    /// is unavailable for the input codec.
    #[must_use]
    pub const fn hardware_only() -> Self {
        Self {
            require_hardware: true,
        }
    }
}

#[async_trait]
impl DecodeBackend for VideoToolboxDecodeBackend {
    fn name(&self) -> &'static str {
        "videotoolbox"
    }

    async fn decode(&self, _path: &VfsPath, _bytes: &[u8]) -> Result<FrameBatchStream> {
        Err(FramePipelineError::NotImplemented {
            backend: "videotoolbox",
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diaspor_core::VfsError;

    #[tokio::test]
    async fn ffmpeg_backend_returns_not_implemented() {
        let backend = FfmpegDecodeBackend::new();
        assert_eq!(backend.name(), "ffmpeg");

        let path = VfsPath::root();
        // FrameBatchStream is `dyn Stream + Send` which does not implement Debug,
        // so we cannot use `Result::expect_err` here. Match the variant explicitly.
        match backend.decode(&path, &[]).await {
            Ok(_) => panic!("v0.1.0-alpha stub must return an error"),
            Err(VfsError::Backend(msg)) => {
                assert!(
                    msg.contains("not implemented"),
                    "expected NotImplemented in error message, got: {msg}"
                );
                assert!(
                    msg.contains("ffmpeg"),
                    "expected backend name 'ffmpeg' in error message, got: {msg}"
                );
            }
            Err(other) => panic!("expected VfsError::Backend, got {other:?}"),
        }
    }
}
