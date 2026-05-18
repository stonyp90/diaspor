//! Facial landmark extraction — dense face mesh + micro-expression Action Units.
//!
//! The default production backend (lands in milestone M7) wraps **`MediaPipe` `FaceMesh`**,
//! which emits 478 3D landmarks per detected face (468 face surface points plus 10 iris
//! points). Action Unit (AU) intensities are derived from the geometry following the
//! Facial Action Coding System (FACS) and feed the downstream micro-expression and
//! lie-detection signals.
//!
//! As with [`crate::pose`], the alpha trait surface fixes only the type shape; the
//! [`NoopFaceLandmarkExtractor`] returns `NotImplemented`.

use async_trait::async_trait;
use bytes::Bytes;
use diaspor_core::{Result, VfsError};
use serde::{Deserialize, Serialize};

use crate::VisionError;

/// Number of facial landmarks produced by `MediaPipe` `FaceMesh` (with iris refinement).
///
/// 468 face-surface landmarks + 10 iris landmarks (5 per eye). Fixed by the upstream
/// topology.
pub const FACE_LANDMARK_COUNT: usize = 478;

/// A single facial landmark in 3D, in normalized image coordinates.
///
/// `x` and `y` are normalized to `[0.0, 1.0]` against the source frame's width and
/// height. `z` is in the same scale and is centered on the face — smaller `z` is closer
/// to the camera.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct FaceLandmark {
    /// Normalized x coordinate in `[0.0, 1.0]`.
    pub x: f32,
    /// Normalized y coordinate in `[0.0, 1.0]`.
    pub y: f32,
    /// Z coordinate centered on the face, in the same scale as `x`.
    pub z: f32,
}

/// One frame's facial landmarks plus derived Action Unit intensities.
///
/// `action_units` is a placeholder for the M7 wiring. Once derived, it will hold the FACS
/// Action Unit intensities relevant to micro-expression detection (AU01, AU02, AU04,
/// AU06, AU07, AU09, AU12, AU15, AU17, AU20, AU23, AU25, AU26, AU45) in a stable order.
/// Empty in the alpha.
///
/// The in-memory shape carries raw `FaceLandmark` values for downstream processing.
/// The on-the-wire shape mandated by `docs/schema/score-v1.json` is INT8-quantized into
/// a base64 string ([`crate::score::WireFaceModality`]); the quantization happens at
/// the wire boundary and is documented there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct FaceLandmarkFrame {
    /// Monotonic timestamp of the source frame in milliseconds from the start of the clip.
    pub timestamp_ms: u64,
    /// The 478 `FaceMesh` landmarks for this frame.
    ///
    /// Boxed because 478 × `f32` × 3 = ~5.6 KB per frame; keeping the heap allocation
    /// explicit lets `clippy` see we're not blowing the stack on every clone.
    #[serde(with = "serde_landmarks")]
    pub landmarks: Box<[FaceLandmark; FACE_LANDMARK_COUNT]>,
    /// FACS Action Unit intensities derived from the landmark geometry.
    ///
    /// Placeholder for M7. Empty in the alpha; populated once geometry-to-AU derivation
    /// rules are implemented.
    pub action_units: Vec<f32>,
}

/// Serde adapter for the boxed fixed-size landmark array.
///
/// Mirrors `crate::pose::serde_keypoints`: the size constant is documented at the
/// schema layer, the adapter validates the length on the deserialize side so a
/// round-trip with the wrong number of landmarks fails fast.
mod serde_landmarks {
    use super::{FACE_LANDMARK_COUNT, FaceLandmark};
    use serde::de::{Error, SeqAccess, Visitor};
    use serde::ser::SerializeSeq;
    use serde::{Deserializer, Serializer};

    pub(super) fn serialize<S>(
        value: &[FaceLandmark; FACE_LANDMARK_COUNT],
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(FACE_LANDMARK_COUNT))?;
        for lm in value {
            seq.serialize_element(lm)?;
        }
        seq.end()
    }

    pub(super) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Box<[FaceLandmark; FACE_LANDMARK_COUNT]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LandmarkArrayVisitor;

        impl<'de> Visitor<'de> for LandmarkArrayVisitor {
            type Value = Box<[FaceLandmark; FACE_LANDMARK_COUNT]>;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "an array of exactly {FACE_LANDMARK_COUNT} FaceLandmark objects"
                )
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut out: Box<[FaceLandmark; FACE_LANDMARK_COUNT]> = Box::new(
                    [FaceLandmark {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    }; FACE_LANDMARK_COUNT],
                );
                let mut idx = 0usize;
                while let Some(lm) = seq.next_element::<FaceLandmark>()? {
                    if idx >= FACE_LANDMARK_COUNT {
                        return Err(A::Error::invalid_length(idx + 1, &"exactly 478 landmarks"));
                    }
                    out[idx] = lm;
                    idx += 1;
                }
                if idx != FACE_LANDMARK_COUNT {
                    return Err(A::Error::invalid_length(idx, &"exactly 478 landmarks"));
                }
                Ok(out)
            }
        }

        deserializer.deserialize_seq(LandmarkArrayVisitor)
    }
}

impl FaceLandmarkFrame {
    /// Builds an empty [`FaceLandmarkFrame`] at `timestamp_ms` with all landmarks zeroed.
    #[must_use]
    pub fn empty(timestamp_ms: u64) -> Self {
        Self {
            timestamp_ms,
            landmarks: Box::new(
                [FaceLandmark {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                }; FACE_LANDMARK_COUNT],
            ),
            action_units: Vec::new(),
        }
    }
}

/// Extracts a [`FaceLandmarkFrame`] from a single video frame's raw bytes.
///
/// Single-face by design at the alpha stage. Multi-face support (returning a
/// `Vec<FaceLandmarkFrame>`) is a deliberate future extension; baking it into the trait
/// shape before the model is wired up would be premature.
#[async_trait]
pub trait FaceLandmarkExtractor: Send + Sync {
    /// Human-readable name of the backend, for logs and provenance records.
    fn name(&self) -> &'static str;

    /// Runs face-landmark extraction over a single decoded video frame.
    async fn extract(&self, frame_bytes: &Bytes, timestamp_ms: u64) -> Result<FaceLandmarkFrame>;
}

/// No-op face-landmark extractor used for trait-surface scaffolding and tests.
///
/// Always returns [`VisionError::NotImplemented`] wrapped into a [`VfsError::Backend`].
/// Replace with the `MediaPipe` `FaceMesh` backend in milestone M7.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopFaceLandmarkExtractor;

#[async_trait]
impl FaceLandmarkExtractor for NoopFaceLandmarkExtractor {
    fn name(&self) -> &'static str {
        "noop-face"
    }

    async fn extract(&self, _frame_bytes: &Bytes, _timestamp_ms: u64) -> Result<FaceLandmarkFrame> {
        Err(VfsError::from(VisionError::NotImplemented {
            backend: "noop-face",
        }))
    }
}
