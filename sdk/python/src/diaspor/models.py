"""Pydantic v2 models for the score-v1 wire schema.

These mirror ``docs/schema/score-v1.json`` in the main Diaspor repo
field-for-field. The schema is the source of truth; if this file and the
schema disagree, the schema wins and this file is the bug.

Each ``Modality`` is optional inside :class:`Modalities` because the API
returns only the modalities the caller asked for (a pose-only call gets a
pose-only record). Field-level required vs optional is documented in each
class's docstring and enforced by the Pydantic typing — ``Optional[...]`` /
``| None`` fields are nullable per the schema, everything else is required.
"""

from __future__ import annotations

from datetime import datetime
from enum import Enum
from typing import Annotated

from pydantic import BaseModel, ConfigDict, Field


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class Severity(str, Enum):
    """Confidence band buckets for credibility scores.

    The wire schema only defines low/medium/high right now, but the enum
    exists as a typed surface so downstream UI code can ``match`` on it
    without stringly-typed comparisons. Future bands (e.g. ``critical``
    for threshold-crossing events) will be added here in a minor release.
    """

    LOW = "low"
    MEDIUM = "medium"
    HIGH = "high"


class RecordKind(str, Enum):
    """Whether a record is a periodic window aggregate or a threshold event."""

    WINDOW = "window"
    EVENT = "event"


class VerticalAttestation(str, Enum):
    """Permitted vertical attestations recorded inside a credibility output.

    Mirrors the enum at ``$defs.CredibilityModality.properties.vertical_attestation``
    in score-v1.json. Forbidden verticals (forensic, hiring, insurance,
    law_enforcement, eu_workplace, eu_education) are refused at the API
    gateway and never appear on a successfully returned record.
    """

    COACHING = "coaching"
    SPORT_JUDGING = "sport_judging"
    INTERVIEW_PLATFORM = "interview_platform"
    DEPOSITION_RECORDING = "deposition_recording"
    RESEARCH = "research"


class InferenceRuntime(str, Enum):
    """Inference backend identifiers recorded in ``ModelProvenance.runtime``."""

    TRITON = "triton"
    COREML = "coreml"
    ORT_CPU = "ort-cpu"
    DEEPSTREAM = "deepstream"


# ---------------------------------------------------------------------------
# Shared base
# ---------------------------------------------------------------------------


class _DiasporModel(BaseModel):
    """Base class enforcing strict-but-tolerant validation.

    - ``extra="forbid"``: unknown fields raise. The wire schema is
      ``additionalProperties: false`` everywhere, so the SDK refuses to
      silently accept fields it does not understand — that protects callers
      from typo'd field names and from accidentally relying on transient
      server-side fields.
    - ``populate_by_name=True``: allows alias-by-name population for fields
      that need a Python-friendly attribute name distinct from the wire
      key. None of the score-v1 fields currently need this, but it keeps
      the door open for additions without a breaking-change bump.
    - ``frozen=False``: records are mutable so callers can construct them
      in tests and tools without ceremony.
    """

    model_config = ConfigDict(
        extra="forbid",
        populate_by_name=True,
        str_strip_whitespace=False,
    )


# ---------------------------------------------------------------------------
# Keypoint primitive
# ---------------------------------------------------------------------------


class Keypoint3d(_DiasporModel):
    """One 3D pose keypoint with visibility.

    All fields **required**. x/y are normalized to [0,1] (frame-relative),
    z is normalized depth (negative is closer to the camera), visibility
    is the keypoint detection confidence in [0,1].
    """

    x: float = Field(description="Normalized x coordinate in [0,1] (frame-relative).")
    y: float = Field(description="Normalized y coordinate in [0,1] (frame-relative).")
    z: float = Field(description="Normalized depth (negative is closer to the camera).")
    visibility: Annotated[float, Field(ge=0.0, le=1.0)] = Field(
        description="Visibility score in [0,1]; lower indicates occluded or out-of-frame.",
    )


# ---------------------------------------------------------------------------
# Gaze (nested inside FaceModality)
# ---------------------------------------------------------------------------


class GazeDirection(_DiasporModel):
    """Head-relative gaze direction. Both fields **required**.

    Yaw is rotation around the vertical axis (left/right), pitch around the
    horizontal axis (up/down). Degrees, relative to head pose.
    """

    yaw_deg: float = Field(description="Yaw rotation in degrees, relative to head pose.")
    pitch_deg: float = Field(description="Pitch rotation in degrees, relative to head pose.")


# ---------------------------------------------------------------------------
# Per-modality records
# ---------------------------------------------------------------------------


class PoseModality(_DiasporModel):
    """33-keypoint 3D body pose (MediaPipe BlazePose topology).

    Required: ``model``, ``keypoints`` (exactly 33 entries in BlazePose
    order). Optional: ``joint_angles_deg`` (dict keyed by joint name),
    ``velocity_mps`` (per-keypoint velocity in normalized units/second,
    same length and order as ``keypoints`` when present).
    """

    model: Annotated[str, Field(min_length=1, max_length=128)] = Field(
        description="Identifier of the pose model used.",
    )
    keypoints: Annotated[list[Keypoint3d], Field(min_length=33, max_length=33)] = Field(
        description="Exactly 33 keypoints in BlazePose topology order.",
    )
    joint_angles_deg: dict[str, float] | None = Field(
        default=None,
        description="Optional joint-angle measurements in degrees, keyed by joint name.",
    )
    velocity_mps: list[float] | None = Field(
        default=None,
        description="Optional per-keypoint velocity in normalized units per second.",
    )


class FaceModality(_DiasporModel):
    """478-landmark facial geometry (MediaPipe FaceMesh topology).

    Required: ``model``. Optional / nullable: ``landmarks_quantized``
    (base64 INT8 478×3 quantization — 1434 bytes; full-precision floats
    live in the binary sidecar), ``microexpr`` (FAU intensities in [0,1]),
    ``gaze`` (head-relative direction).
    """

    model: Annotated[str, Field(min_length=1, max_length=128)] = Field(
        description="Identifier of the face-mesh model.",
    )
    landmarks_quantized: str | None = Field(
        default=None,
        description="Base64-encoded INT8 quantization of 478 (x,y,z) triples.",
    )
    microexpr: dict[str, Annotated[float, Field(ge=0.0, le=1.0)]] | None = Field(
        default=None,
        description="Facial Action Unit intensities, keyed by AU code; values in [0,1].",
    )
    gaze: GazeDirection | None = Field(
        default=None,
        description="Optional gaze direction (yaw_deg, pitch_deg).",
    )


class ProsodyModality(_DiasporModel):
    """Vocal prosody features (default backend: openSMILE eGeMAPSv02 + ComParE2016).

    Required: ``model``. Optional / nullable: ``tremor_index`` (in [0,1]),
    ``f0_var`` (Hz²), ``pace_words_per_minute``, ``features_dim`` (full
    feature-vector dimensionality; 6552 for eGeMAPSv02+ComParE2016 deduped).
    """

    model: Annotated[str, Field(min_length=1, max_length=128)] = Field(
        description="Identifier of the prosody extractor.",
    )
    tremor_index: Annotated[float, Field(ge=0.0, le=1.0)] | None = Field(
        default=None,
        description="Composite tremor indicator in [0,1].",
    )
    f0_var: Annotated[float, Field(ge=0.0)] | None = Field(
        default=None,
        description="Variance of fundamental frequency (Hz^2) over the window.",
    )
    pace_words_per_minute: Annotated[float, Field(ge=0.0)] | None = Field(
        default=None,
        description="Estimated speaking rate over the window.",
    )
    features_dim: Annotated[int, Field(ge=1)] | None = Field(
        default=None,
        description="Dimensionality of the full feature vector emitted alongside this summary.",
    )


class CredibilityModality(_DiasporModel):
    """Composite credibility-signal output.

    **NOT a lie-detection verdict.** Per-window indicator of stress +
    incongruence, with the human baseline and the peer-reviewed accuracy
    ceiling disclosed on every response for honesty.

    Required: ``model``, ``score``, ``confidence_band``,
    ``human_baseline_disclosed``, ``ceiling_disclosed``. Optional:
    ``labs_preview`` (default ``True``), ``vertical_attestation`` (nullable).
    """

    model: Annotated[str, Field(min_length=1, max_length=128)] = Field(
        description="Identifier of the credibility model.",
    )
    score: Annotated[float, Field(ge=0.0, le=1.0)] = Field(
        description="Indicator score in [0,1]. Higher = more stress/incongruence. Not a P(deception).",
    )
    confidence_band: Severity = Field(
        description="Calibrated uncertainty bucket. Display prominently next to the score.",
    )
    human_baseline_disclosed: Annotated[float, Field(ge=0.0, le=1.0)] = Field(
        description="Human baseline accuracy for video-based deception inference (~0.54).",
    )
    ceiling_disclosed: Annotated[float, Field(ge=0.0, le=1.0)] = Field(
        description="Accuracy ceiling for video-based deception inference in peer-reviewed lit (~0.74).",
    )
    labs_preview: bool = Field(
        default=True,
        description="True if the model is still in private beta. Label preview-quality.",
    )
    vertical_attestation: VerticalAttestation | None = Field(
        default=None,
        description="Vertical declared at key creation. Forbidden verticals are refused upstream.",
    )


class JudgeModality(_DiasporModel):
    """Sport-judging score (per-discipline model fine-tuned on the discipline rubric).

    Required: ``model``, ``discipline``, ``score``. Optional / nullable:
    ``execution_score``, ``difficulty_multiplier``, ``rubric_version``.
    """

    model: Annotated[str, Field(min_length=1, max_length=128)] = Field(
        description="Identifier of the judge model.",
    )
    discipline: Annotated[str, Field(min_length=1, max_length=64)] = Field(
        description="Sport discipline this score applies to (e.g. 'diving').",
    )
    score: float = Field(
        description="Discipline-specific score on the rubric's native scale.",
    )
    execution_score: float | None = Field(
        default=None,
        description="Optional execution-only sub-score on the rubric's native scale.",
    )
    difficulty_multiplier: float | None = Field(
        default=None,
        description="Optional difficulty multiplier (degree of difficulty), where applicable.",
    )
    rubric_version: Annotated[str, Field(max_length=64)] | None = Field(
        default=None,
        description="Identifier of the discipline rubric used (e.g. 'fina-2025').",
    )


# ---------------------------------------------------------------------------
# Top-level grouping
# ---------------------------------------------------------------------------


class Modalities(_DiasporModel):
    """Container for per-modality outputs on a single record.

    Every field is optional individually, but the schema requires at least
    one of them to be present (``minProperties: 1``). A pose-only judging
    record carries only ``pose`` and ``judge``; a credibility request
    typically carries ``face``, ``prosody``, and ``credibility``.

    The minProperties=1 invariant is enforced server-side; on the SDK side
    we rely on the server's guarantee rather than re-validating here, so
    that callers constructing records in tests can build up modalities
    incrementally.
    """

    pose: PoseModality | None = None
    face: FaceModality | None = None
    prosody: ProsodyModality | None = None
    credibility: CredibilityModality | None = None
    judge: JudgeModality | None = None


class ModelProvenance(_DiasporModel):
    """Which exact model produced one modality's output.

    Required: ``model_name``. Optional / nullable: ``model_hash``
    (hex-encoded SHA-256 by convention), ``adapter_id`` (per-tenant LoRA
    id, present only for custom-tier deployments), ``runtime``,
    ``latency_us``.
    """

    model_name: Annotated[str, Field(min_length=1, max_length=256)] = Field(
        description="Identifier of the model (often '<model-id>@<backbone>+<lora>').",
    )
    model_hash: Annotated[str, Field(pattern=r"^[0-9a-fA-F]{32,128}$")] | None = Field(
        default=None,
        description="Optional cryptographic hash of the model file (hex SHA-256).",
    )
    adapter_id: Annotated[str, Field(min_length=1, max_length=256)] | None = Field(
        default=None,
        description="Per-tenant LoRA adapter id; present only for custom-tier deployments.",
    )
    runtime: InferenceRuntime | None = Field(
        default=None,
        description="Inference backend that ran the model.",
    )
    latency_us: Annotated[int, Field(ge=0)] | None = Field(
        default=None,
        description="End-to-end inference latency for this modality in microseconds.",
    )


class ScoreRecord(_DiasporModel):
    """A v1 score record describing one stream window (or one threshold event).

    Required: ``schema_version`` (always ``"1"`` for v1), ``stream_id``,
    ``tenant``, ``t_start_ms``, ``t_end_ms`` (must be > t_start_ms),
    ``modalities``, ``extracted_at`` (RFC 3339 with timezone). Optional:
    ``kind`` (defaults to ``WINDOW``), ``model_provenance``.

    Maps onto ``/.streams/<stream_id>/windows/<timestamp>.score.json`` (and
    the equivalent event path) in the on-disk VFS layout.
    """

    schema_version: str = Field(
        description="Schema version constant. v1 records always carry the literal string '1'.",
    )
    stream_id: Annotated[str, Field(min_length=1, max_length=256)] = Field(
        description="Opaque, tenant-unique identifier for the analyzed stream.",
    )
    tenant: Annotated[str, Field(min_length=1, max_length=256)] = Field(
        description="Opaque tenant identifier the stream belongs to.",
    )
    t_start_ms: Annotated[int, Field(ge=0)] = Field(
        description="Inclusive lower bound of the analyzed window, in ms from stream start.",
    )
    t_end_ms: Annotated[int, Field(ge=0)] = Field(
        description="Exclusive upper bound of the analyzed window, in ms. Must be > t_start_ms.",
    )
    kind: RecordKind = Field(
        default=RecordKind.WINDOW,
        description="Window aggregate or threshold-crossing event. Defaults to 'window'.",
    )
    modalities: Modalities = Field(
        description="Per-modality outputs for this window.",
    )
    extracted_at: datetime = Field(
        description="RFC 3339 timestamp of when the record was finalized (must include tz).",
    )
    model_provenance: list[ModelProvenance] | None = Field(
        default=None,
        description="One ModelProvenance entry per modality that contributed to this record.",
    )


# ---------------------------------------------------------------------------
# Streaming event envelope
# ---------------------------------------------------------------------------


class IngestEvent(_DiasporModel):
    """One event delivered over the live WebSocket stream.

    Carries a :class:`ScoreRecord` payload plus an envelope of metadata that
    is meaningful only on the wire (sequence number, session id). The
    envelope is intentionally minimal so callers can use ``event.record``
    interchangeably with batch :meth:`Client.analyze` output.
    """

    session_id: str = Field(description="Live-ingest session identifier.")
    seq: Annotated[int, Field(ge=0)] = Field(
        description="Monotonic per-session sequence number, useful for ordered logging.",
    )
    record: ScoreRecord = Field(
        description="The score record carried by this event.",
    )


__all__ = [
    "CredibilityModality",
    "FaceModality",
    "GazeDirection",
    "InferenceRuntime",
    "IngestEvent",
    "JudgeModality",
    "Keypoint3d",
    "Modalities",
    "ModelProvenance",
    "PoseModality",
    "ProsodyModality",
    "RecordKind",
    "ScoreRecord",
    "Severity",
    "VerticalAttestation",
]
