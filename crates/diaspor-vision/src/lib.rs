//! # diaspor-vision
//!
//! Non-verbal video analysis pipeline for `diaspor`. Three pluggable modalities:
//!
//! 1. **Pose** ([`pose::PoseExtractor`]) — 33-keypoint 3D body skeleton, joint angles,
//!    per-joint velocity. Production backend: `MediaPipe` `BlazePose` `GHUM` 3D.
//! 2. **Face** ([`face::FaceLandmarkExtractor`]) — 478-landmark dense face mesh + FACS
//!    Action Unit intensities for micro-expression detection. Production backend:
//!    `MediaPipe` `FaceMesh`.
//! 3. **Prosody** ([`prosody::ProsodyExtractor`]) — fixed-width voice-feature vector
//!    spanning pitch, energy, voice quality, and spectral shape. Production backend:
//!    `openSMILE` with the combined `eGeMAPSv02` + `ComParE_2016` configuration.
//!
//! Each modality's output is bundled — alongside a per-modality [`record::ModelProvenance`]
//! — into a [`record::VisionRecord`] by the composition struct [`VisionPipeline`].
//!
//! ## Pipeline at a glance
//!
//! ```text
//!   ┌────────────┐                  ┌────────────────┐
//!   │ frame +    │ ───┬───────────▶│ PoseExtractor  │──┐
//!   │ audio in   │    │             └────────────────┘  │
//!   └────────────┘    │             ┌────────────────┐  │   ┌──────────────┐
//!                     ├───────────▶│ FaceLandmark   │──┼──▶│ VisionRecord │
//!                     │             │   Extractor    │  │   │ + provenance │
//!                     │             └────────────────┘  │   └──────────────┘
//!                     │             ┌────────────────┐  │
//!                     └───────────▶│ ProsodyExtractor│──┘
//!                                   └────────────────┘
//! ```
//!
//! ## Privacy contract
//!
//! Mirrors `diaspor-index`: every default backend runs on-device. No network calls, no
//! telemetry, no third-party model APIs by default. Callers can substitute remote
//! backends but must construct them explicitly.
//!
//! ## Use cases
//!
//! `diaspor-vision` exists to feed the non-verbal signal layer used by lie-detection,
//! sport-judging, and scoring applications built on top of `diaspor`. The crate ships
//! the trait surface only; rule engines and classifiers live in downstream crates so
//! they can iterate independently of the extraction pipeline.
//!
//! ## Status
//!
//! v0.1.0-alpha ships the trait surface only. Full `MediaPipe` `BlazePose` 3D +
//! `MediaPipe` `FaceMesh` + `openSMILE` wiring lands in milestone M7.

#![doc(html_root_url = "https://docs.rs/diaspor-vision/0.1.0-alpha.1")]

use bytes::Bytes;
use diaspor_core::{Result, VfsError};
use thiserror::Error;

pub mod face;
pub mod pose;
#[cfg(feature = "rtmpose-ort")]
pub mod pose_rtmpose;
pub mod prosody;
pub mod record;
pub mod score;

pub use face::{FaceLandmark, FaceLandmarkExtractor, FaceLandmarkFrame, NoopFaceLandmarkExtractor};
pub use pose::{NoopPoseExtractor, PoseExtractor, PoseFrame, PoseKeypoint};
#[cfg(feature = "rtmpose-ort")]
pub use pose_rtmpose::{
    DEFAULT_RTMPOSE_MODEL_ID, IMAGENET_MEAN, IMAGENET_STD, RTMPOSE_INPUT_HEIGHT,
    RTMPOSE_INPUT_WIDTH, RTMPOSE_KEYPOINTS, RtmposePoseExtractor, decode_rtmpose_heatmap,
    encode_imagenet_chw,
};
pub use prosody::{NoopProsodyExtractor, ProsodyExtractor, ProsodyFeatures};
pub use record::{ModelProvenance, VisionRecord};
pub use score::{
    WireConfidenceBand, WireCredibilityModality, WireFaceModality, WireGaze, WireJudgeModality,
    WireKeypoint3d, WireModalities, WireModelNames, WireModelProvenance, WirePoseModality,
    WireProsodyModality, WireRecordKind, WireRuntime, WireScoreFraming, WireScoreRecord,
    WireVerticalAttestation,
};

/// Things that can go wrong specifically in the vision pipeline.
///
/// Wraps cleanly into a [`diaspor_core::VfsError::Backend`] via the `From` impl below, so
/// trait methods can return the workspace-wide [`diaspor_core::Result`] while still
/// constructing vision-specific error variants internally.
#[derive(Debug, Error)]
pub enum VisionError {
    /// A modality backend has not been wired up yet.
    ///
    /// Returned by every `NoopXxx` stub and by production backends that have not yet
    /// integrated a runtime. Carries the backend name so logs can identify which
    /// modality stubbed out.
    #[error("vision backend not implemented: {backend}")]
    NotImplemented {
        /// Human-readable name of the backend that returned the stub.
        backend: &'static str,
    },

    /// The pose extractor backend rejected the frame or failed mid-inference.
    #[error("pose extractor failed: {0}")]
    PoseFailed(String),

    /// The face-landmark extractor backend rejected the frame or failed mid-inference.
    #[error("face extractor failed: {0}")]
    FaceFailed(String),

    /// The prosody extractor backend rejected the audio or failed mid-inference.
    #[error("prosody extractor failed: {0}")]
    ProsodyFailed(String),

    /// The supplied frame or audio bytes are malformed or in an unsupported format.
    #[error("malformed input: {0}")]
    MalformedInput(String),
}

impl From<VisionError> for VfsError {
    fn from(err: VisionError) -> Self {
        Self::Backend(err.to_string())
    }
}

/// The composed vision pipeline.
///
/// Generic over three extractor implementations so callers can mix and match (e.g. real
/// `MediaPipe` pose + face but a stubbed prosody during integration). Construct with
/// concrete types; `process` orchestrates all three in sequence and returns a
/// [`VisionRecord`] with one [`ModelProvenance`] per modality.
///
/// # Example
///
/// ```ignore
/// use diaspor_vision::{
///     NoopFaceLandmarkExtractor, NoopPoseExtractor, NoopProsodyExtractor, VisionPipeline,
/// };
///
/// let pipeline = VisionPipeline {
///     pose: NoopPoseExtractor,
///     face: NoopFaceLandmarkExtractor,
///     prosody: NoopProsodyExtractor,
/// };
/// // pipeline.process(&frame_bytes, &audio_pcm).await
/// ```
pub struct VisionPipeline<P, F, R> {
    /// Pose extractor (production default: `MediaPipe` `BlazePose` `GHUM` 3D).
    pub pose: P,
    /// Face-landmark extractor (production default: `MediaPipe` `FaceMesh`).
    pub face: F,
    /// Prosody extractor (production default: `openSMILE` `eGeMAPSv02` + `ComParE_2016`).
    pub prosody: R,
}

impl<P, F, R> VisionPipeline<P, F, R>
where
    P: PoseExtractor,
    F: FaceLandmarkExtractor,
    R: ProsodyExtractor,
{
    /// Runs all three modalities over the given frame + audio and assembles a
    /// [`VisionRecord`].
    ///
    /// `frame_bytes` is a single decoded video frame in the format the pose and face
    /// extractors expect. `audio_pcm` is the audio clip aligned with that frame's source
    /// window, in the format the prosody extractor expects (typically 16 kHz mono 16-bit
    /// PCM to match the `diaspor-index` audio contract).
    ///
    /// The frame timestamp is set to `0` in the alpha; clip-level timing is the caller's
    /// responsibility until the multi-frame pipeline lands.
    ///
    /// # Errors
    ///
    /// Bubbles up the first error from any modality. With the `Noop*` stubs in place,
    /// every call returns [`VisionError::NotImplemented`] for the pose modality (the
    /// first one in the chain).
    pub async fn process(
        &self,
        frame_bytes: &Bytes,
        audio_pcm: &Bytes,
    ) -> Result<VisionRecord> {
        let timestamp_ms: u64 = 0;
        let pose = self.pose.extract(frame_bytes, timestamp_ms).await?;
        let face = self.face.extract(frame_bytes, timestamp_ms).await?;
        let prosody = self.prosody.extract(audio_pcm).await?;
        Ok(VisionRecord {
            extracted_at: time::OffsetDateTime::now_utc(),
            pose,
            face,
            prosody,
            pose_provenance: ModelProvenance::noop(self.pose.name()),
            face_provenance: ModelProvenance::noop(self.face.name()),
            prosody_provenance: ModelProvenance::noop(self.prosody.name()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_pipeline_reports_not_implemented() {
        let pipeline = VisionPipeline {
            pose: NoopPoseExtractor,
            face: NoopFaceLandmarkExtractor,
            prosody: NoopProsodyExtractor,
        };

        let frame = Bytes::from_static(&[0u8; 16]);
        let audio = Bytes::from_static(&[0u8; 16]);
        let result = pipeline.process(&frame, &audio).await;

        let err = result.expect_err("noop pipeline must error");
        match err {
            VfsError::Backend(msg) => {
                assert!(
                    msg.contains("not implemented") && msg.contains("noop-pose"),
                    "expected NotImplemented for noop-pose, got: {msg}"
                );
            }
            other => panic!("expected VfsError::Backend, got {other:?}"),
        }
    }
}
