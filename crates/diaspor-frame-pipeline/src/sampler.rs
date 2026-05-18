//! Frame samplers — strategies for deciding which decoded frames to forward.
//!
//! A [`DecodeBackend`](crate::DecodeBackend) hands back every frame it decodes; a
//! [`FrameSampler`] thins that stream down to the frames a downstream model
//! actually wants to see. Three policies ship with the crate:
//!
//! - [`UniformFrameSampler`] — target a fixed output fps regardless of source fps.
//! - [`KeyframeAlignedFrameSampler`] — only emit frames the decoder flagged as
//!   keyframes (I-frames). Cheap, lossy, useful for thumbnailing and scene-change
//!   detection.
//! - [`FaceTriggeredFrameSampler`] — only emit frames where a lightweight face
//!   detector saw a face. The detector is a placeholder in v0.1.0-alpha and slots
//!   in for a real on-device model later.
//!
//! All three are stateless across `sample` calls and cheap to clone.

use async_trait::async_trait;
use diaspor_core::Result;

use crate::batch::FrameBatchStream;

/// A strategy for downsampling a [`FrameBatchStream`].
///
/// Implementors receive the raw decoder stream and return a (typically) shorter
/// stream of the frames worth keeping. Samplers are not allowed to introduce new
/// frames or reorder existing ones — they may only drop frames and forward the rest
/// in presentation order.
#[async_trait]
pub trait FrameSampler: Send + Sync {
    /// Stable human-readable name of the sampler, used for logs.
    fn name(&self) -> &'static str;

    /// Wraps `input` in a sampling stage and returns the thinned-out stream.
    ///
    /// The returned stream is responsible for driving `input` itself; the caller
    /// only polls the output. Errors from `input` pass through unchanged.
    async fn sample(&self, input: FrameBatchStream) -> Result<FrameBatchStream>;
}

/// Emits one frame every N source frames, chosen so that the output frame rate
/// approximates [`Self::fps`].
///
/// The exact selection algorithm is "decimate by `round(source_fps / target_fps)`",
/// computed from the [`crate::FrameBatch::timestamp_us`] of the first few frames.
/// The implementation lands in milestone M7 alongside the `FFmpeg` decoder; this
/// struct is just the configuration carrier.
///
/// Best policy for the Phase 1 batch indexing path, where uniform temporal coverage
/// matters more than scene-aware sampling.
#[derive(Debug, Clone, Copy)]
pub struct UniformFrameSampler {
    /// Target output frame rate, in frames per second. Must be > 0.
    pub fps: u32,
}

impl UniformFrameSampler {
    /// Constructs a `UniformFrameSampler` targeting `fps` frames per second.
    #[must_use]
    pub const fn new(fps: u32) -> Self {
        Self { fps }
    }
}

#[async_trait]
impl FrameSampler for UniformFrameSampler {
    fn name(&self) -> &'static str {
        "uniform"
    }

    async fn sample(&self, input: FrameBatchStream) -> Result<FrameBatchStream> {
        // v0.1.0-alpha stub: forwards every frame unchanged. The real decimation
        // logic lands in M7. Returning the input unchanged keeps the trait surface
        // honest — the sampler is a pure pass-through filter shape.
        Ok(input)
    }
}

/// Emits only frames the decoder flagged as keyframes (I-frames).
///
/// The decoder is responsible for surfacing the keyframe bit; this sampler simply
/// filters on it. Cheap, deterministic, and a good default for thumbnailing or
/// scene-change detection where temporal density does not matter.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyframeAlignedFrameSampler;

impl KeyframeAlignedFrameSampler {
    /// Constructs a `KeyframeAlignedFrameSampler`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FrameSampler for KeyframeAlignedFrameSampler {
    fn name(&self) -> &'static str {
        "keyframe-aligned"
    }

    async fn sample(&self, input: FrameBatchStream) -> Result<FrameBatchStream> {
        // v0.1.0-alpha stub: forwards every frame unchanged. The real keyframe
        // filter lands in M7, once the decoder side wires up the keyframe bit on
        // each [`crate::FrameBatch`].
        Ok(input)
    }
}

/// Emits only frames where a lightweight face detector flagged at least one face.
///
/// The detector is intentionally a placeholder in v0.1.0-alpha — picking the right
/// on-device model (`BlazeFace`, RetinaFace-mobile, ...) is part of the M8 vision
/// work. Once that lands, this sampler becomes the default for the lie-detection
/// and sport-judging pipelines, where "frames without a face in them" is almost
/// always wasted inference budget.
///
/// # Privacy
///
/// All detection happens on-device. No frame ever leaves the host as part of the
/// sampling decision.
#[derive(Debug, Clone, Copy, Default)]
pub struct FaceTriggeredFrameSampler {
    /// Minimum face-detection confidence (`0.0`–`1.0`) required to forward a frame.
    /// Defaults to `0.5`. Honored by the M8 implementation; ignored by the
    /// v0.1.0-alpha stub.
    pub min_confidence: f32,
}

impl FaceTriggeredFrameSampler {
    /// Constructs a `FaceTriggeredFrameSampler` with the default `0.5` confidence
    /// threshold.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            min_confidence: 0.5,
        }
    }

    /// Constructs a `FaceTriggeredFrameSampler` with a custom confidence threshold.
    #[must_use]
    pub const fn with_confidence(min_confidence: f32) -> Self {
        Self { min_confidence }
    }
}

#[async_trait]
impl FrameSampler for FaceTriggeredFrameSampler {
    fn name(&self) -> &'static str {
        "face-triggered"
    }

    async fn sample(&self, input: FrameBatchStream) -> Result<FrameBatchStream> {
        // v0.1.0-alpha stub: forwards every frame unchanged. The real face-detector
        // gate lands in M8 alongside the on-device vision model selection.
        Ok(input)
    }
}
