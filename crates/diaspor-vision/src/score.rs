//! Wire-shape types for `docs/schema/score-v1.json`.
//!
//! The in-memory pipeline types in [`crate::record`], [`crate::pose`], [`crate::face`],
//! and [`crate::prosody`] carry runtime concerns (per-frame timestamps, raw landmark
//! arrays, internal provenance shape) that the on-the-wire score record does not need.
//! Rather than fold those concerns onto every consumer of the score schema (or rip the
//! in-memory shape to match an external schema), this module ships a parallel **wire
//! shape** whose serde representation is *exactly* what `score-v1.json` mandates.
//!
//! Wire types are constructed via `From` and helper conversions on the in-memory types —
//! see [`WireScoreRecord::from_vision`] for the full conversion path.
//!
//! ## Naming
//!
//! Everything in this module is prefixed `Wire*` so a grep for "what serializes to the
//! score schema" yields one obvious answer. The in-memory types intentionally do NOT
//! gain a `Wire*` alias.
//!
//! ## Round-trip guarantee
//!
//! `tests/serde_roundtrip.rs` constructs a `WireScoreRecord`, serializes it via
//! `serde_json::to_string_pretty`, validates the resulting JSON against
//! `docs/schema/score-v1.json` with `jsonschema`, deserializes back to a
//! `WireScoreRecord`, and asserts equality. The test is the load-bearing proof that
//! score records emitted by the M7+ pipeline are conformant.

use std::collections::BTreeMap;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::face::FaceLandmarkFrame;
use crate::pose::{POSE_LANDMARK_COUNT, PoseFrame, PoseKeypoint};
use crate::prosody::ProsodyFeatures;
use crate::record::{ModelProvenance, VisionRecord};

/// The literal value of the `schema_version` field for every v1 score record.
pub const SCORE_SCHEMA_VERSION: &str = "1";

/// What kind of stream-window record this is — a periodic aggregate or a single
/// threshold-crossing event.
///
/// Mirrors the `kind` enum in `docs/schema/score-v1.json` exactly. Defaults to
/// `Window` because that is what the schema's `default` keyword specifies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireRecordKind {
    /// Periodic per-window aggregate (default).
    #[default]
    Window,
    /// Single threshold-crossing event.
    Event,
}

/// Calibrated uncertainty bucket for a credibility score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireConfidenceBand {
    /// Low-confidence verdict; consumers should display the score with prominent
    /// uncertainty signalling.
    Low,
    /// Medium-confidence verdict.
    Medium,
    /// High-confidence verdict relative to the model's calibration.
    High,
}

/// Vertical declared by the tenant at API-key creation time, used by the API layer
/// to refuse forbidden verticals before invoking the credibility model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireVerticalAttestation {
    /// Coaching / personal-development.
    Coaching,
    /// Sport judging (diving, gymnastics, weightlifting, martial arts forms).
    SportJudging,
    /// Interview platform / pre-employment screening.
    InterviewPlatform,
    /// Legal deposition or pre-trial recording.
    DepositionRecording,
    /// Academic research deployment.
    Research,
}

/// Inference backend that ran a model, recorded for audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireRuntime {
    /// NVIDIA Triton Inference Server (production GPU path).
    Triton,
    /// Apple `CoreML` (on-device, Apple Silicon).
    Coreml,
    /// ONNX Runtime CPU (portable INT8 fallback).
    #[serde(rename = "ort-cpu")]
    OrtCpu,
    /// NVIDIA `DeepStream` (live, colocated pipeline).
    Deepstream,
}

/// Wire shape for a 3D pose keypoint as it appears inside the `pose.keypoints` array
/// in `docs/schema/score-v1.json`.
///
/// Identical layout to [`crate::pose::PoseKeypoint`]; declared separately so the wire
/// module owns the schema-aligned `Serialize` / `Deserialize` derives without
/// re-exporting in-memory ones.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WireKeypoint3d {
    /// Normalized x coordinate in `[0,1]` (frame-relative).
    pub x: f32,
    /// Normalized y coordinate in `[0,1]` (frame-relative).
    pub y: f32,
    /// Normalized depth (negative is closer to the camera).
    pub z: f32,
    /// Visibility score in `[0,1]`.
    pub visibility: f32,
}

impl From<&PoseKeypoint> for WireKeypoint3d {
    fn from(kp: &PoseKeypoint) -> Self {
        Self {
            x: kp.x,
            y: kp.y,
            z: kp.z,
            visibility: kp.visibility,
        }
    }
}

impl From<WireKeypoint3d> for PoseKeypoint {
    fn from(kp: WireKeypoint3d) -> Self {
        Self {
            x: kp.x,
            y: kp.y,
            z: kp.z,
            visibility: kp.visibility,
        }
    }
}

/// Wire shape for the `pose` modality entry of `docs/schema/score-v1.json`.
///
/// `keypoints` MUST contain exactly 33 entries in `BlazePose` topology order; the
/// `from_pose` constructor enforces this from a [`PoseFrame`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WirePoseModality {
    /// Identifier of the pose model used (e.g. `"diaspor-pose-3d-v1"`).
    pub model: String,
    /// 33-keypoint pose in `BlazePose` topology order.
    pub keypoints: Vec<WireKeypoint3d>,
    /// Optional joint-angle measurements in degrees, keyed by joint name.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub joint_angles_deg: Option<BTreeMap<String, f64>>,
    /// Optional per-keypoint velocity in normalized units per second.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub velocity_mps: Option<Vec<f64>>,
}

impl WirePoseModality {
    /// Lifts a [`PoseFrame`] into the wire shape under a given model name.
    ///
    /// `velocity_mps` is populated from the per-keypoint Euclidean norm of
    /// [`PoseFrame::joint_velocities`] *if* it has the expected 33-entry length; any
    /// other length is treated as "not populated" and the field is omitted from the
    /// serialized output. `joint_angles_deg` is left empty in the alpha — the
    /// in-memory `joint_angles` vector is a placeholder without joint names; the
    /// production backend will populate this map from the named-joint table.
    #[must_use]
    pub fn from_pose(model: impl Into<String>, pose: &PoseFrame) -> Self {
        let keypoints: Vec<WireKeypoint3d> =
            pose.keypoints.iter().map(WireKeypoint3d::from).collect();
        let velocity_mps = if pose.joint_velocities.len() == POSE_LANDMARK_COUNT {
            Some(
                pose.joint_velocities
                    .iter()
                    .map(|(dx, dy, dz)| {
                        f64::from(dx.mul_add(*dx, dy.mul_add(*dy, dz * dz)).sqrt())
                    })
                    .collect(),
            )
        } else {
            None
        };
        Self {
            model: model.into(),
            keypoints,
            joint_angles_deg: None,
            velocity_mps,
        }
    }
}

/// Wire shape for the optional `face.gaze` field of `docs/schema/score-v1.json`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WireGaze {
    /// Yaw in degrees relative to head pose.
    pub yaw_deg: f64,
    /// Pitch in degrees relative to head pose.
    pub pitch_deg: f64,
}

/// Wire shape for the `face` modality entry of `docs/schema/score-v1.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WireFaceModality {
    /// Identifier of the face-mesh model used.
    pub model: String,
    /// Base64-encoded INT8 quantization of the 478 landmark `(x, y, z)` triples.
    ///
    /// Encoding contract: `478 × 3 × 1 byte = 1434 bytes`; recover with
    /// `(byte_i - 128) / 127.0 → [-1, 1]`. `None` for windows where the face mesh
    /// was not run or was suppressed. See `docs/schema/score-v1.json`.
    #[serde(with = "serde_landmarks_quantized")]
    pub landmarks_quantized: Option<Bytes>,
    /// Optional FACS Action Unit intensities in `[0, 1]`, keyed by AU code.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub microexpr: Option<BTreeMap<String, f64>>,
    /// Optional gaze direction in degrees relative to head pose.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gaze: Option<WireGaze>,
}

impl WireFaceModality {
    /// Lifts a [`FaceLandmarkFrame`] into the wire shape under a given model name.
    ///
    /// The 478 `FaceLandmark` triples are INT8-quantized inline. `microexpr` and
    /// `gaze` are left empty at the alpha — the in-memory `action_units` vector is
    /// a placeholder and the production backend will populate these maps once
    /// FACS-AU + iris-vector derivation lands.
    #[must_use]
    pub fn from_face(model: impl Into<String>, face: &FaceLandmarkFrame) -> Self {
        let mut quantized: Vec<u8> = Vec::with_capacity(face.landmarks.len() * 3);
        for lm in face.landmarks.iter() {
            quantized.push(quantize_i8(lm.x));
            quantized.push(quantize_i8(lm.y));
            quantized.push(quantize_i8(lm.z));
        }
        Self {
            model: model.into(),
            landmarks_quantized: Some(Bytes::from(quantized)),
            microexpr: None,
            gaze: None,
        }
    }
}

fn quantize_i8(value: f32) -> u8 {
    // Clamp value to [-1, 1], scale by 127, shift to [1, 255] then [0, 255].
    let clamped = value.clamp(-1.0, 1.0);
    // Round-half-away-from-zero is what `as i32` does after `+ 0.5_f32.copysign(x)`
    // — we use it here so the round-trip is symmetric around zero.
    let scaled = clamped * 127.0;
    let rounded = scaled + 0.5_f32.copysign(scaled);
    #[allow(clippy::cast_possible_truncation)]
    let int_i32 = rounded as i32;
    let clamped_i32 = int_i32.clamp(-127, 127);
    // Final shift to u8: byte = i8 + 128.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    {
        (clamped_i32 + 128) as u8
    }
}

/// Serde adapter for `Option<Bytes>` ↔ base64 (or JSON null).
///
/// The schema accepts either a base64 string OR an explicit JSON null. Bytes is the
/// in-memory carrier; base64 is the wire encoding. Failing to base64-decode produces a
/// clean `serde::de::Error` so a malformed payload is rejected at parse time, not at
/// downstream INT8-dequantization time.
mod serde_landmarks_quantized {
    use super::{BASE64_STANDARD, Bytes};
    use base64::Engine as _;
    use serde::de::{Error, Unexpected};
    use serde::{Deserialize, Deserializer, Serializer};

    // serde's `with` adapter convention requires `&Option<Bytes>` here; the
    // `clippy::ref_option` pedantic lint disagrees but the alternative —
    // `Option<&Bytes>` — does not satisfy what `#[serde(with = "...")]`
    // generates.
    #[allow(clippy::ref_option)]
    pub(super) fn serialize<S>(value: &Option<Bytes>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value.as_ref() {
            Some(bytes) => {
                let encoded = BASE64_STANDARD.encode(bytes);
                serializer.serialize_str(&encoded)
            }
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<Bytes>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?.map_or_else(
            || Ok(None),
            |s| {
                BASE64_STANDARD
                    .decode(s.as_bytes())
                    .map(|v| Some(Bytes::from(v)))
                    .map_err(|_| {
                        D::Error::invalid_value(
                            Unexpected::Str(&s),
                            &"a base64-encoded INT8 quantization",
                        )
                    })
            },
        )
    }
}

/// Wire shape for the `prosody` modality entry of `docs/schema/score-v1.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WireProsodyModality {
    /// Identifier of the prosody extractor used.
    pub model: String,
    /// Composite tremor indicator in `[0, 1]`.
    #[serde(default)]
    pub tremor_index: Option<f64>,
    /// Variance of fundamental frequency (Hz²) over the window.
    #[serde(default)]
    pub f0_var: Option<f64>,
    /// Estimated speaking rate in words per minute.
    #[serde(default)]
    pub pace_words_per_minute: Option<f64>,
    /// Dimensionality of the full feature vector emitted alongside this summary.
    #[serde(default)]
    pub features_dim: Option<u32>,
}

impl WireProsodyModality {
    /// Lifts a [`ProsodyFeatures`] vector into the wire shape under a given model name.
    ///
    /// `features_dim` is populated from the vector length when non-empty;
    /// `tremor_index`, `f0_var`, and `pace_words_per_minute` are left empty at the alpha
    /// — they are derived by the production backend's functional layer, which hasn't
    /// landed yet.
    #[must_use]
    pub fn from_prosody(
        model: impl Into<String>,
        prosody: &ProsodyFeatures,
    ) -> Self {
        let features_dim = if prosody.features.is_empty() {
            None
        } else {
            Some(u32::try_from(prosody.features.len()).unwrap_or(u32::MAX))
        };
        Self {
            model: model.into(),
            tremor_index: None,
            f0_var: None,
            pace_words_per_minute: None,
            features_dim,
        }
    }
}

/// Wire shape for the `credibility` modality entry of `docs/schema/score-v1.json`.
///
/// Carries the per-window indicator score alongside the human-baseline + accuracy-
/// ceiling disclosures that the schema requires alongside every credibility record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WireCredibilityModality {
    /// Identifier of the credibility model used.
    pub model: String,
    /// Indicator score in `[0, 1]`. Higher = more stress/incongruence signal.
    pub score: f64,
    /// Calibrated uncertainty bucket.
    pub confidence_band: WireConfidenceBand,
    /// Disclosed human baseline accuracy for video-based deception inference
    /// (~0.54 per peer-reviewed meta-analysis).
    pub human_baseline_disclosed: f64,
    /// Disclosed accuracy ceiling for video-based deception inference (~0.74).
    pub ceiling_disclosed: f64,
    /// `true` if the model is still in private beta. Defaults to true on
    /// deserialization to err on the side of clearly labelling preview output.
    #[serde(default = "default_true")]
    pub labs_preview: bool,
    /// Vertical attestation declared by the tenant at API-key creation time.
    #[serde(default)]
    pub vertical_attestation: Option<WireVerticalAttestation>,
}

const fn default_true() -> bool {
    true
}

/// Wire shape for the `judge` modality entry of `docs/schema/score-v1.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WireJudgeModality {
    /// Identifier of the judge model used.
    pub model: String,
    /// Sport discipline this score applies to (e.g. `"diving"`, `"gymnastics"`).
    pub discipline: String,
    /// Discipline-specific score on the rubric's native scale.
    pub score: f64,
    /// Optional execution-only sub-score.
    #[serde(default)]
    pub execution_score: Option<f64>,
    /// Optional difficulty multiplier (degree of difficulty), where applicable.
    #[serde(default)]
    pub difficulty_multiplier: Option<f64>,
    /// Identifier of the discipline rubric the model was calibrated against
    /// (e.g. `"fina-2025"`, `"fig-2025"`).
    #[serde(default)]
    pub rubric_version: Option<String>,
}

/// Wire shape for the `modalities` container of `docs/schema/score-v1.json`.
///
/// Each entry is optional; the schema requires `minProperties: 1`, which the
/// [`WireModalities::is_non_empty`] check exposes for callers that want to enforce
/// it before serialization.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WireModalities {
    /// Pose modality output (33-keypoint 3D body skeleton).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pose: Option<WirePoseModality>,
    /// Face modality output (478-landmark `FaceMesh` + AUs + gaze).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub face: Option<WireFaceModality>,
    /// Prosody modality output (eGeMAPSv02 + `ComParE2016` functionals).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prosody: Option<WireProsodyModality>,
    /// Credibility modality output (composite stress/incongruence indicator).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub credibility: Option<WireCredibilityModality>,
    /// Judge modality output (sport-judging score).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub judge: Option<WireJudgeModality>,
}

impl WireModalities {
    /// `true` if at least one modality is populated. The schema requires
    /// `minProperties: 1`, so a `WireScoreRecord` containing an empty
    /// `WireModalities` will fail validation.
    #[must_use]
    pub const fn is_non_empty(&self) -> bool {
        self.pose.is_some()
            || self.face.is_some()
            || self.prosody.is_some()
            || self.credibility.is_some()
            || self.judge.is_some()
    }
}

/// Wire shape for an entry in the top-level `model_provenance` array of
/// `docs/schema/score-v1.json`.
///
/// `model_hash` is constrained at the schema layer to a hex SHA-(any-len) pattern;
/// validation happens at the schema boundary, not in Rust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WireModelProvenance {
    /// Model identifier (e.g. `"diaspor-pose-3d-v1@blazepose-heavy"`).
    pub model_name: String,
    /// Optional cryptographic hash of the model file (hex-encoded).
    #[serde(default)]
    pub model_hash: Option<String>,
    /// Optional per-tenant `LoRA` adapter identifier active for this inference.
    #[serde(default)]
    pub adapter_id: Option<String>,
    /// Inference backend that ran the model.
    #[serde(default)]
    pub runtime: Option<WireRuntime>,
    /// End-to-end inference latency for this modality in microseconds.
    #[serde(default)]
    pub latency_us: Option<u64>,
}

impl WireModelProvenance {
    /// Lifts an in-memory [`ModelProvenance`] into the wire shape.
    ///
    /// The in-memory `runtime: String` is mapped to the enum where possible (the
    /// schema enumerates `triton`, `coreml`, `ort-cpu`, `deepstream`); unknown
    /// runtime strings (e.g. the `"noop"` value used by the alpha stubs) drop the
    /// runtime field rather than emit a non-conformant value. `model_hash` and
    /// `adapter_id` are left empty here — they are populated by the inference
    /// layer in the M7+ wiring.
    #[must_use]
    pub fn from_in_memory(prov: &ModelProvenance) -> Self {
        let runtime = match prov.runtime.as_str() {
            "triton" => Some(WireRuntime::Triton),
            "coreml" => Some(WireRuntime::Coreml),
            "ort-cpu" => Some(WireRuntime::OrtCpu),
            "deepstream" => Some(WireRuntime::Deepstream),
            _ => None,
        };
        Self {
            model_name: prov.model_name.clone(),
            model_hash: prov.model_hash.clone(),
            adapter_id: None,
            runtime,
            latency_us: None,
        }
    }
}

/// The top-level wire shape for `docs/schema/score-v1.json`.
///
/// `schema_version` defaults to [`SCORE_SCHEMA_VERSION`]; if a deserialized payload
/// carries a different version the consumer's responsibility is to detect that and
/// route through a version-shim rather than to extend this struct in place — the
/// v2 wire shape will live in its own module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WireScoreRecord {
    /// Schema version constant — always `"1"` for this struct.
    pub schema_version: String,
    /// Opaque, tenant-unique identifier for the stream being analyzed.
    pub stream_id: String,
    /// Opaque tenant identifier the stream belongs to.
    pub tenant: String,
    /// Inclusive lower bound of the analyzed window, ms from stream start.
    pub t_start_ms: u64,
    /// Exclusive upper bound of the analyzed window, ms from stream start.
    pub t_end_ms: u64,
    /// Periodic window aggregate vs single threshold-crossing event.
    #[serde(default)]
    pub kind: WireRecordKind,
    /// Per-modality outputs. At least one modality must be populated for the
    /// record to validate against the schema (`minProperties: 1`).
    pub modalities: WireModalities,
    /// ISO 8601 / RFC 3339 timestamp of when the record was finalized.
    #[serde(with = "time::serde::rfc3339")]
    pub extracted_at: OffsetDateTime,
    /// Optional array of per-modality model provenance records.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model_provenance: Option<Vec<WireModelProvenance>>,
}

/// Stream-window framing the orchestration layer supplies when it lifts an in-memory
/// [`VisionRecord`] into the wire shape.
///
/// These five fields are not carried on `VisionRecord` itself — they are properties
/// of the *window* the extractor was run inside, owned by the stream-routing layer
/// (e.g. `diaspor-stream-ingest`). Bundling them keeps
/// [`WireScoreRecord::from_vision`] under the pedantic argument-count threshold and
/// makes future additions (e.g. a `session_id`) a backward-compatible field
/// addition rather than a signature break.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireScoreFraming {
    /// Opaque, tenant-unique stream identifier.
    pub stream_id: String,
    /// Opaque tenant identifier.
    pub tenant: String,
    /// Inclusive lower bound of the analyzed window, ms from stream start.
    pub t_start_ms: u64,
    /// Exclusive upper bound, ms from stream start.
    pub t_end_ms: u64,
    /// Periodic window vs threshold-crossing event.
    pub kind: WireRecordKind,
}

/// Per-modality model identifiers stamped into the modality `model` fields when
/// an in-memory [`VisionRecord`] is lifted onto the wire.
///
/// Decoupled from the extractor's `name()` because the production wire shape uses
/// publication-style names (e.g. `"diaspor-pose-3d-v1"`) while extractor names are
/// runtime-internal (e.g. `"mediapipe-blazepose-heavy"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireModelNames {
    /// Pose model name to stamp into `modalities.pose.model`.
    pub pose: String,
    /// Face model name to stamp into `modalities.face.model`.
    pub face: String,
    /// Prosody model name to stamp into `modalities.prosody.model`.
    pub prosody: String,
}

impl WireScoreRecord {
    /// Constructs a [`WireScoreRecord`] from an in-memory [`VisionRecord`].
    ///
    /// `framing` carries the stream-window properties the per-frame extractors do
    /// not own; `models` carries the publication-style model names that get
    /// stamped into the `modalities.{pose,face,prosody}.model` fields. The
    /// `extracted_at` timestamp and per-modality provenance are carried through
    /// from the in-memory record verbatim.
    #[must_use]
    pub fn from_vision(
        framing: WireScoreFraming,
        models: WireModelNames,
        record: &VisionRecord,
    ) -> Self {
        let modalities = WireModalities {
            pose: Some(WirePoseModality::from_pose(models.pose, &record.pose)),
            face: Some(WireFaceModality::from_face(models.face, &record.face)),
            prosody: Some(WireProsodyModality::from_prosody(
                models.prosody,
                &record.prosody,
            )),
            credibility: None,
            judge: None,
        };
        let model_provenance = vec![
            WireModelProvenance::from_in_memory(&record.pose_provenance),
            WireModelProvenance::from_in_memory(&record.face_provenance),
            WireModelProvenance::from_in_memory(&record.prosody_provenance),
        ];
        Self {
            schema_version: SCORE_SCHEMA_VERSION.to_string(),
            stream_id: framing.stream_id,
            tenant: framing.tenant,
            t_start_ms: framing.t_start_ms,
            t_end_ms: framing.t_end_ms,
            kind: framing.kind,
            modalities,
            extracted_at: record.extracted_at,
            model_provenance: Some(model_provenance),
        }
    }
}
