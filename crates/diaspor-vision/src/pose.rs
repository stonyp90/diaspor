//! Pose extraction — body skeleton in 3D, joint angles, and per-joint velocity.
//!
//! The default production backend (lands in milestone M7) wraps **`MediaPipe` `BlazePose`
//! `GHUM` 3D**, which emits 33 named landmarks with `(x, y, z)` coordinates plus a
//! per-landmark visibility / presence score. For the alpha trait surface we model the
//! geometry only; the [`NoopPoseExtractor`] returns `NotImplemented` so callers can wire
//! the trait through their composition without committing to a runtime yet.

use async_trait::async_trait;
use bytes::Bytes;
use diaspor_core::{Result, VfsError};
use serde::{Deserialize, Serialize};

use crate::VisionError;

/// Number of body landmarks produced by `BlazePose` `GHUM` 3D.
///
/// Fixed by the upstream model topology (nose, eyes, ears, shoulders, elbows, wrists,
/// finger landmarks, hips, knees, ankles, feet). Kept as a constant so callers can
/// allocate fixed-size buffers when they want to avoid heap traffic.
pub const POSE_LANDMARK_COUNT: usize = 33;

/// A single 3D body landmark in normalized image coordinates.
///
/// `x` and `y` are normalized to `[0.0, 1.0]` against the source frame's width and height.
/// `z` is normalized to roughly the same scale as `x` and is *relative to the hip
/// midpoint*; smaller `z` is closer to the camera. `visibility` is the model's confidence
/// that the landmark is present and not occluded.
///
/// Serializes as a JSON object `{ x, y, z, visibility }`, matching the
/// `Keypoint3d` definition in `docs/schema/score-v1.json`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PoseKeypoint {
    /// Normalized x coordinate in `[0.0, 1.0]`.
    pub x: f32,
    /// Normalized y coordinate in `[0.0, 1.0]`.
    pub y: f32,
    /// Z coordinate relative to the hip midpoint, in the same scale as `x`.
    pub z: f32,
    /// Visibility / presence score in `[0.0, 1.0]`.
    pub visibility: f32,
}

/// One frame's worth of pose information for a single tracked person.
///
/// Frame-level container produced once per processed video frame. `joint_angles` and
/// `joint_velocities` are placeholders for the M7 wiring; their semantics (which joints,
/// in what order, radians vs degrees) are intentionally undefined at the alpha stage and
/// will be locked in when the `MediaPipe` integration lands.
///
/// `keypoints` serializes as a JSON array of 33 `{ x, y, z, visibility }` objects, in
/// upstream `BlazePose` topology order, matching the `PoseModality.keypoints` shape in
/// `docs/schema/score-v1.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PoseFrame {
    /// Monotonic timestamp of the source frame in milliseconds from the start of the clip.
    pub timestamp_ms: u64,
    /// The 33 `BlazePose` landmarks for this frame.
    ///
    /// Indexed positionally per the upstream `BlazePose` topology; ordering is preserved
    /// from the model output so downstream consumers (e.g. judging-rule evaluators) can
    /// index by the published landmark names.
    #[serde(with = "serde_keypoints")]
    pub keypoints: [PoseKeypoint; POSE_LANDMARK_COUNT],
    /// Derived joint angles, in radians.
    ///
    /// Placeholder for M7. The vector will hold elbow / knee / shoulder / hip / spine
    /// angles in a stable order once the derivation is implemented; empty in the alpha.
    pub joint_angles: Vec<f32>,
    /// Per-landmark velocity vectors `(dx/dt, dy/dt, dz/dt)`, in normalized units per
    /// second.
    ///
    /// Placeholder for M7. Empty in the alpha; populated once temporal smoothing and
    /// per-landmark first-difference computation are wired in.
    pub joint_velocities: Vec<(f32, f32, f32)>,
}

/// Serde adapter for the fixed-size `[PoseKeypoint; 33]` array.
///
/// `serde` ships array `Deserialize` impls for any `N` since 1.0.146, but the
/// fixed-size array path is gated behind `min_const_generics` features that some
/// older toolchains do not pick up cleanly; bouncing through a `Vec<PoseKeypoint>`
/// keeps the deserialization explicit, validates the length (33 — strictly enforced
/// at the schema layer), and surfaces a clean error on drift.
mod serde_keypoints {
    use super::{POSE_LANDMARK_COUNT, PoseKeypoint};
    use serde::de::{Error, SeqAccess, Visitor};
    use serde::ser::SerializeSeq;
    use serde::{Deserializer, Serializer};

    pub(super) fn serialize<S>(
        value: &[PoseKeypoint; POSE_LANDMARK_COUNT],
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(POSE_LANDMARK_COUNT))?;
        for kp in value {
            seq.serialize_element(kp)?;
        }
        seq.end()
    }

    pub(super) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<[PoseKeypoint; POSE_LANDMARK_COUNT], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct KeypointArrayVisitor;

        impl<'de> Visitor<'de> for KeypointArrayVisitor {
            type Value = [PoseKeypoint; POSE_LANDMARK_COUNT];

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(
                    f,
                    "an array of exactly {POSE_LANDMARK_COUNT} PoseKeypoint objects"
                )
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut out: [PoseKeypoint; POSE_LANDMARK_COUNT] = [PoseKeypoint {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    visibility: 0.0,
                };
                    POSE_LANDMARK_COUNT];
                let mut idx = 0usize;
                while let Some(kp) = seq.next_element::<PoseKeypoint>()? {
                    if idx >= POSE_LANDMARK_COUNT {
                        return Err(A::Error::invalid_length(idx + 1, &"exactly 33 keypoints"));
                    }
                    out[idx] = kp;
                    idx += 1;
                }
                if idx != POSE_LANDMARK_COUNT {
                    return Err(A::Error::invalid_length(idx, &"exactly 33 keypoints"));
                }
                Ok(out)
            }
        }

        deserializer.deserialize_seq(KeypointArrayVisitor)
    }
}

impl PoseFrame {
    /// Builds an empty [`PoseFrame`] at `timestamp_ms` with all keypoints zeroed.
    ///
    /// Useful for tests and for backends that want a zero-initialized frame before
    /// populating it.
    #[must_use]
    pub const fn empty(timestamp_ms: u64) -> Self {
        Self {
            timestamp_ms,
            keypoints: [PoseKeypoint {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                visibility: 0.0,
            }; POSE_LANDMARK_COUNT],
            joint_angles: Vec::new(),
            joint_velocities: Vec::new(),
        }
    }
}

/// Extracts a [`PoseFrame`] from a single video frame's raw bytes.
///
/// Implementations are expected to be stateful at the *backend* level (loading models is
/// expensive) but stateless at the *frame* level — a single `extract` call must not
/// depend on prior frames. Temporal aggregation (velocities, smoothing) is the
/// implementer's responsibility once it's implemented.
#[async_trait]
pub trait PoseExtractor: Send + Sync {
    /// Human-readable name of the backend, for logs and provenance records.
    fn name(&self) -> &'static str;

    /// Runs pose extraction over a single decoded video frame.
    ///
    /// `frame_bytes` is the raw decoded frame in the implementer's expected pixel format
    /// (the trait deliberately does not pin this — backends document it). `timestamp_ms`
    /// is the source-frame timestamp from the start of the clip.
    async fn extract(&self, frame_bytes: &Bytes, timestamp_ms: u64) -> Result<PoseFrame>;
}

/// No-op pose extractor used for trait-surface scaffolding and tests.
///
/// Always returns [`VisionError::NotImplemented`] wrapped into a [`VfsError::Backend`].
/// Replace with the `MediaPipe` `BlazePose` backend in milestone M7.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopPoseExtractor;

#[async_trait]
impl PoseExtractor for NoopPoseExtractor {
    fn name(&self) -> &'static str {
        "noop-pose"
    }

    async fn extract(&self, _frame_bytes: &Bytes, _timestamp_ms: u64) -> Result<PoseFrame> {
        Err(VfsError::from(VisionError::NotImplemented {
            backend: "noop-pose",
        }))
    }
}
