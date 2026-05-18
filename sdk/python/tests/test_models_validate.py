"""Validation tests against the canonical score-v1 example.

Loads ``docs/schema/score-v1.json`` from the repo root, pulls its
``examples[0]`` block, and confirms the SDK's :class:`ScoreRecord` model
accepts it without modification. This is the contract test that pins the
SDK to the schema — if the schema changes shape, this test fails and we
update the models in lockstep.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from diaspor import (
    CredibilityModality,
    FaceModality,
    JudgeModality,
    PoseModality,
    ProsodyModality,
    RecordKind,
    ScoreRecord,
    Severity,
    VerticalAttestation,
)

# Walk up from this test file to find the repo root, then point at the
# canonical schema file. Layout: <repo>/sdk/python/tests/this_file.py
# So the schema is four levels up + docs/schema/.
_SCHEMA_PATH = Path(__file__).resolve().parents[3] / "docs" / "schema" / "score-v1.json"


def _load_example() -> dict[str, Any]:
    raw = _SCHEMA_PATH.read_text(encoding="utf-8")
    schema = json.loads(raw)
    examples = schema.get("examples")
    assert isinstance(examples, list) and examples, (
        f"Expected at least one example in {_SCHEMA_PATH}"
    )
    example = examples[0]
    assert isinstance(example, dict)
    return example


def test_canonical_example_validates_into_score_record() -> None:
    example = _load_example()
    record = ScoreRecord.model_validate(example)

    # Spot-check fields we'd notice immediately if the wire shape drifted.
    assert record.schema_version == "1"
    assert record.stream_id == "abc123"
    assert record.tenant == "acme"
    assert record.t_start_ms == 12000
    assert record.t_end_ms == 13000
    assert record.kind == RecordKind.WINDOW
    assert record.extracted_at.isoformat() == "2026-05-15T12:30:13+00:00"


def test_pose_modality_has_full_33_keypoint_topology() -> None:
    example = _load_example()
    record = ScoreRecord.model_validate(example)

    assert record.modalities.pose is not None
    assert isinstance(record.modalities.pose, PoseModality)
    # BlazePose topology is exactly 33 keypoints.
    assert len(record.modalities.pose.keypoints) == 33
    # Visibility must be in [0,1]; the example uses 0.99 for the head.
    assert 0.0 <= record.modalities.pose.keypoints[0].visibility <= 1.0
    # Joint angles should round-trip when present.
    assert record.modalities.pose.joint_angles_deg is not None
    assert "left_elbow" in record.modalities.pose.joint_angles_deg


def test_face_modality_includes_gaze_and_microexpr() -> None:
    example = _load_example()
    record = ScoreRecord.model_validate(example)

    assert record.modalities.face is not None
    assert isinstance(record.modalities.face, FaceModality)
    assert record.modalities.face.microexpr is not None
    assert record.modalities.face.microexpr["AU4"] == pytest.approx(0.31)
    assert record.modalities.face.gaze is not None
    assert record.modalities.face.gaze.yaw_deg == pytest.approx(-2.4)


def test_prosody_modality_carries_summary_features() -> None:
    example = _load_example()
    record = ScoreRecord.model_validate(example)

    assert record.modalities.prosody is not None
    assert isinstance(record.modalities.prosody, ProsodyModality)
    assert record.modalities.prosody.tremor_index == pytest.approx(0.07)
    assert record.modalities.prosody.features_dim == 6552


def test_credibility_modality_discloses_human_baseline_and_ceiling() -> None:
    example = _load_example()
    record = ScoreRecord.model_validate(example)

    assert record.modalities.credibility is not None
    assert isinstance(record.modalities.credibility, CredibilityModality)
    cred = record.modalities.credibility
    assert cred.score == pytest.approx(0.41)
    assert cred.confidence_band == Severity.LOW
    # The honesty disclosures are not optional — every credibility record
    # carries them. If this test fails, the schema or the model is wrong.
    assert cred.human_baseline_disclosed == pytest.approx(0.54)
    assert cred.ceiling_disclosed == pytest.approx(0.74)
    assert cred.labs_preview is True
    assert cred.vertical_attestation == VerticalAttestation.COACHING


def test_model_provenance_round_trip() -> None:
    example = _load_example()
    record = ScoreRecord.model_validate(example)

    assert record.model_provenance is not None
    assert len(record.model_provenance) == 2
    pose_prov = record.model_provenance[0]
    assert pose_prov.model_name == "diaspor-pose-3d-v1@blazepose-heavy"
    assert pose_prov.runtime is not None
    assert pose_prov.runtime.value == "coreml"
    assert pose_prov.latency_us == 8400


def test_record_serializes_back_to_dict() -> None:
    # Loose round-trip: a record we parsed should serialize back to a
    # superset-or-equal JSON shape. We don't assert byte-equality because
    # Pydantic emits a deterministic key order that may differ from the
    # input; we just confirm core fields survive the round trip.
    example = _load_example()
    record = ScoreRecord.model_validate(example)
    dumped = record.model_dump(mode="json", exclude_none=True)
    assert dumped["schema_version"] == "1"
    assert dumped["stream_id"] == example["stream_id"]
    assert dumped["modalities"]["pose"]["model"] == example["modalities"]["pose"]["model"]


def test_judge_modality_field_shape() -> None:
    # The canonical example does not include a judge modality, but the
    # type still needs to accept the minimum-required field set so a
    # diving judge response validates. Construct one synthetically.
    judge = JudgeModality.model_validate(
        {
            "model": "diaspor-judge-v1",
            "discipline": "diving",
            "score": 24.7,
            "execution_score": 8.5,
            "difficulty_multiplier": 2.9,
            "rubric_version": "fina-2025",
        },
    )
    assert judge.discipline == "diving"
    assert judge.score == pytest.approx(24.7)
    assert judge.rubric_version == "fina-2025"


def test_unknown_fields_are_rejected() -> None:
    # extra="forbid" means a typo'd field at the top level fails fast.
    example = _load_example()
    bad = dict(example)
    bad["definitely_not_a_real_field"] = True
    with pytest.raises(Exception):
        ScoreRecord.model_validate(bad)
