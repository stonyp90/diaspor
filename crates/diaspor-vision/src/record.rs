//! Record types — the combined output of one full vision-pipeline run.
//!
//! Mirrors the sidecar pattern from `diaspor-index`: a single struct that bundles every
//! modality's output plus enough provenance to reproduce or audit the run. Unlike
//! `diaspor-index::sidecar::SidecarRecord`, this record is *not* serialized in the alpha;
//! it is a pure in-memory carrier between pipeline stages and downstream consumers
//! (judging-rule evaluators, classifiers, dashboards).

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::face::FaceLandmarkFrame;
use crate::pose::PoseFrame;
use crate::prosody::ProsodyFeatures;

/// Identifies the model + runtime that produced one modality's output.
///
/// One [`ModelProvenance`] is recorded per modality so that downstream consumers can
/// route signals appropriately (e.g. only fuse pose + face if both were produced by
/// known-compatible model versions) and so audits can attribute a record to an exact
/// model artifact.
///
/// This is the in-memory shape — it carries a `runtime: String` for ergonomics. The
/// wire shape that maps onto `docs/schema/score-v1.json`'s `ModelProvenance` is
/// [`crate::score::WireModelProvenance`], reached via `From<&ModelProvenance>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ModelProvenance {
    /// Human-readable model name, e.g. `"BlazePose-GHUM-Heavy"`.
    pub model_name: String,
    /// Content hash of the model artifact (e.g. `sha256:…`), if known.
    ///
    /// `None` for the no-op stubs and for backends that have not pinned a model file.
    pub model_hash: Option<String>,
    /// Runtime backend used to execute the model, e.g. `"mediapipe-tasks-0.10"`,
    /// `"onnxruntime-1.18"`, `"opensmile-3.0"`.
    pub runtime: String,
}

impl ModelProvenance {
    /// Convenience constructor for a fully-specified provenance record.
    #[must_use]
    pub fn new(
        model_name: impl Into<String>,
        model_hash: Option<String>,
        runtime: impl Into<String>,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            model_hash,
            runtime: runtime.into(),
        }
    }

    /// Convenience constructor for a stub / no-op provenance with no model hash.
    #[must_use]
    pub fn noop(name: impl Into<String>) -> Self {
        Self {
            model_name: name.into(),
            model_hash: None,
            runtime: "noop".to_string(),
        }
    }
}

/// One full vision-pipeline run's output for a single (video frame, audio clip) pair.
///
/// The alpha shape is intentionally minimal — one [`PoseFrame`], one
/// [`FaceLandmarkFrame`], one [`ProsodyFeatures`] vector — because the pipeline is still
/// single-frame at this milestone. Multi-frame aggregation (clip-level pose statistics,
/// expression sequences) is a future extension that will wrap rather than replace this
/// record.
///
/// This is the in-memory shape — it does not carry the stream-window framing
/// (`stream_id`, `tenant`, `t_start_ms`, `t_end_ms`, `kind`) that
/// `docs/schema/score-v1.json` requires. To emit a record onto the wire, lift it into
/// [`crate::score::WireScoreRecord`] via [`crate::score::WireScoreRecord::from_vision`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct VisionRecord {
    /// When this record was produced.
    #[serde(with = "time::serde::rfc3339")]
    pub extracted_at: OffsetDateTime,
    /// Pose modality output.
    pub pose: PoseFrame,
    /// Face-landmark modality output.
    pub face: FaceLandmarkFrame,
    /// Prosody modality output.
    pub prosody: ProsodyFeatures,
    /// Provenance for the pose extractor that produced [`Self::pose`].
    pub pose_provenance: ModelProvenance,
    /// Provenance for the face-landmark extractor that produced [`Self::face`].
    pub face_provenance: ModelProvenance,
    /// Provenance for the prosody extractor that produced [`Self::prosody`].
    pub prosody_provenance: ModelProvenance,
}
