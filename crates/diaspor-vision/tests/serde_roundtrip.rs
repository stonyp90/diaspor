//! Round-trip + schema-conformance test for the score-v1 wire shape.
//!
//! Builds a realistic [`diaspor_vision::WireScoreRecord`] with pose, face, prosody,
//! credibility, and judge modalities populated, then verifies three properties end-
//! to-end:
//!
//! 1. Serialization to JSON produces output that validates against
//!    `docs/schema/score-v1.json` under a JSON Schema 2020-12 validator.
//! 2. Re-deserializing that JSON yields a struct equal to the original (by
//!    `PartialEq`). This is the load-bearing proof that score records are real and
//!    not just shape-on-paper.
//! 3. The wire shape produced from an in-memory `VisionRecord` round-trips through
//!    JSON without losing information.
//!
//! Schema is loaded from the repo at `docs/schema/score-v1.json`. The path is
//! resolved relative to `CARGO_MANIFEST_DIR` so the test works regardless of where
//! `cargo test` is invoked from.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use bytes::Bytes;
use diaspor_vision::{
    FaceLandmark, FaceLandmarkFrame, ModelProvenance, PoseFrame, PoseKeypoint, ProsodyFeatures,
    VisionRecord, WireConfidenceBand, WireCredibilityModality, WireFaceModality, WireGaze,
    WireJudgeModality, WireKeypoint3d, WireModalities, WireModelNames, WireModelProvenance,
    WirePoseModality, WireProsodyModality, WireRecordKind, WireRuntime, WireScoreFraming,
    WireScoreRecord, WireVerticalAttestation, face::FACE_LANDMARK_COUNT, pose::POSE_LANDMARK_COUNT,
};
use jsonschema::validator_for;
use serde_json::Value;
use time::OffsetDateTime;
use time::macros::datetime;

/// Workspace-rooted path to the v1 score schema.
fn score_schema_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent() // crates/
        .and_then(std::path::Path::parent) // workspace root
        .expect("CARGO_MANIFEST_DIR must live two levels below the workspace root")
        .join("docs")
        .join("schema")
        .join("score-v1.json")
}

/// Build a non-trivial `WireScoreRecord` with all five modalities and per-modality
/// provenance populated. Values are deliberately spread across the valid range
/// (visibility < 1.0, AU intensities < 1.0, joint angles ~ realistic dive form)
/// so a regression that clamps to defaults shows up as a `PartialEq` mismatch.
fn realistic_record() -> WireScoreRecord {
    // Use short, bit-exact decimals so the serialize → parse → deserialize
    // round-trip preserves PartialEq on the f32 fields. `0.99 - 0.01 * (i/33)`
    // is fine on the way out but does not round-trip exactly through
    // `serde_json::Number` parsing (see the matching note in the infer test).
    let mut keypoints = Vec::with_capacity(POSE_LANDMARK_COUNT);
    for i in 0..POSE_LANDMARK_COUNT {
        // Cast through u16 → f32 to keep clippy::cast_precision_loss happy
        // (33 fits in 16 bits, well within an f32 mantissa).
        let idx = u16::try_from(i).expect("33 keypoints fit in u16");
        let frac = f32::from(idx) / f32::from(u16::try_from(POSE_LANDMARK_COUNT).unwrap_or(1));
        keypoints.push(WireKeypoint3d {
            x: 0.01_f32.mul_add(frac, 0.5),
            y: 0.7_f32.mul_add(frac, 0.2),
            z: 0.05 * frac,
            visibility: 0.01_f32.mul_add(-frac, 0.99),
        });
    }

    let mut joint_angles = BTreeMap::new();
    joint_angles.insert("left_elbow".to_string(), 142.3);
    joint_angles.insert("right_elbow".to_string(), 138.7);

    let pose = WirePoseModality {
        model: "diaspor-pose-3d-v1".to_string(),
        keypoints,
        joint_angles_deg: Some(joint_angles),
        velocity_mps: None,
    };

    // 478 landmarks × 3 channels = 1434 bytes of base64 INT8 payload.
    let face_payload = Bytes::from(vec![128u8; FACE_LANDMARK_COUNT * 3]);
    let mut microexpr = BTreeMap::new();
    microexpr.insert("AU4".to_string(), 0.31_f64);
    microexpr.insert("AU7".to_string(), 0.18_f64);
    let face = WireFaceModality {
        model: "diaspor-face-mesh-v1".to_string(),
        landmarks_quantized: Some(face_payload),
        microexpr: Some(microexpr),
        gaze: Some(WireGaze {
            yaw_deg: -2.4,
            pitch_deg: 5.1,
        }),
    };

    let prosody = WireProsodyModality {
        model: "diaspor-prosody-v1".to_string(),
        tremor_index: Some(0.07),
        f0_var: Some(12.4),
        pace_words_per_minute: Some(142.0),
        features_dim: Some(6552),
    };

    let credibility = WireCredibilityModality {
        model: "diaspor-credibility-v1".to_string(),
        score: 0.41,
        confidence_band: WireConfidenceBand::Low,
        human_baseline_disclosed: 0.54,
        ceiling_disclosed: 0.74,
        labs_preview: true,
        vertical_attestation: Some(WireVerticalAttestation::Coaching),
    };

    let judge = WireJudgeModality {
        model: "diaspor-judge-v1".to_string(),
        discipline: "diving".to_string(),
        score: 24.5,
        execution_score: Some(8.5),
        difficulty_multiplier: Some(2.9),
        rubric_version: Some("fina-2025".to_string()),
    };

    let modalities = WireModalities {
        pose: Some(pose),
        face: Some(face),
        prosody: Some(prosody),
        credibility: Some(credibility),
        judge: Some(judge),
    };

    let model_provenance = vec![
        WireModelProvenance {
            model_name: "diaspor-pose-3d-v1@blazepose-heavy".to_string(),
            model_hash: Some(
                "3a7f1c2b9d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a".to_string(),
            ),
            adapter_id: None,
            runtime: Some(WireRuntime::Coreml),
            latency_us: Some(8400),
        },
        WireModelProvenance {
            model_name: "diaspor-credibility-v1@internvideo2-1b+lora-r16".to_string(),
            model_hash: None,
            adapter_id: Some("acme-credibility-v3".to_string()),
            runtime: Some(WireRuntime::Triton),
            latency_us: Some(31200),
        },
    ];

    WireScoreRecord {
        schema_version: "1".to_string(),
        stream_id: "abc123".to_string(),
        tenant: "acme".to_string(),
        t_start_ms: 12_000,
        t_end_ms: 13_000,
        kind: WireRecordKind::Window,
        modalities,
        extracted_at: datetime!(2026-05-15 12:30:13 UTC),
        model_provenance: Some(model_provenance),
    }
}

/// Loads + parses the score-v1 schema. Panics on any I/O or parse error — the
/// test is meaningless without the schema.
fn load_schema() -> Value {
    let path = score_schema_path();
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read score schema at {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("score schema at {} is not valid JSON: {e}", path.display()))
}

#[test]
fn wire_score_record_serializes_validates_and_roundtrips() {
    let original = realistic_record();

    // 1. Serialize.
    let json_string = serde_json::to_string_pretty(&original)
        .expect("WireScoreRecord serialization to JSON must succeed");

    // 2. Validate against the published schema.
    let schema = load_schema();
    let validator = validator_for(&schema)
        .expect("docs/schema/score-v1.json must be a well-formed JSON Schema 2020-12 doc");
    let parsed: Value =
        serde_json::from_str(&json_string).expect("serialized output must be valid JSON");
    let errors: Vec<String> = validator
        .iter_errors(&parsed)
        .map(|err| format!("  - {} (at {})", err, err.instance_path))
        .collect();
    assert!(
        errors.is_empty(),
        "WireScoreRecord serialization did not validate against score-v1.json:\n{}\n---\n{json_string}",
        errors.join("\n")
    );

    // 3. Deserialize back.
    let round_tripped: WireScoreRecord =
        serde_json::from_str(&json_string).expect("WireScoreRecord must round-trip via JSON");

    // 4. Round-tripped struct equals the original (modulo the f32 quantization
    //    artifact: this test deliberately stays in WireScoreRecord-only land
    //    so the comparison is field-for-field).
    assert_eq!(
        original, round_tripped,
        "round-tripped WireScoreRecord must equal the original"
    );
}

#[test]
fn wire_score_record_lifted_from_vision_record_validates() {
    // Build an in-memory VisionRecord that the alpha pipeline could plausibly
    // emit, then lift it through `WireScoreRecord::from_vision`.
    let pose = PoseFrame {
        timestamp_ms: 12_500,
        keypoints: [PoseKeypoint {
            x: 0.5,
            y: 0.5,
            z: 0.0,
            visibility: 0.95,
        }; POSE_LANDMARK_COUNT],
        joint_angles: Vec::new(),
        joint_velocities: Vec::new(),
    };
    let face = FaceLandmarkFrame {
        timestamp_ms: 12_500,
        landmarks: Box::new(
            [FaceLandmark {
                x: 0.1,
                y: 0.2,
                z: 0.0,
            }; FACE_LANDMARK_COUNT],
        ),
        action_units: Vec::new(),
    };
    let prosody = ProsodyFeatures {
        sample_rate_hz: 16_000,
        channels: 1,
        duration_ms: 1_000,
        features: vec![0.0; 88],
    };
    let record = VisionRecord {
        extracted_at: OffsetDateTime::from_unix_timestamp(1_768_000_000)
            .expect("hard-coded epoch must be in range"),
        pose,
        face,
        prosody,
        pose_provenance: ModelProvenance {
            model_name: "diaspor-pose-3d-v1".to_string(),
            model_hash: None,
            runtime: "coreml".to_string(),
        },
        face_provenance: ModelProvenance {
            model_name: "diaspor-face-mesh-v1".to_string(),
            model_hash: None,
            runtime: "coreml".to_string(),
        },
        prosody_provenance: ModelProvenance {
            model_name: "diaspor-prosody-v1".to_string(),
            model_hash: None,
            runtime: "ort-cpu".to_string(),
        },
    };

    let framing = WireScoreFraming {
        stream_id: "stream-from-vision-1".to_string(),
        tenant: "tenant-acme".to_string(),
        t_start_ms: 12_000,
        t_end_ms: 13_000,
        kind: WireRecordKind::Window,
    };
    let models = WireModelNames {
        pose: "diaspor-pose-3d-v1".to_string(),
        face: "diaspor-face-mesh-v1".to_string(),
        prosody: "diaspor-prosody-v1".to_string(),
    };
    let lifted = WireScoreRecord::from_vision(framing, models, &record);

    let json_string =
        serde_json::to_string_pretty(&lifted).expect("lifted WireScoreRecord must serialize");
    let schema = load_schema();
    let validator = validator_for(&schema).expect("schema must parse");
    let parsed: Value = serde_json::from_str(&json_string).expect("must be valid JSON");
    let errors: Vec<String> = validator
        .iter_errors(&parsed)
        .map(|err| format!("  - {} (at {})", err, err.instance_path))
        .collect();
    assert!(
        errors.is_empty(),
        "VisionRecord-lifted WireScoreRecord did not validate against score-v1.json:\n{}\n---\n{json_string}",
        errors.join("\n")
    );

    let round_tripped: WireScoreRecord = serde_json::from_str(&json_string)
        .expect("VisionRecord-lifted WireScoreRecord must round-trip");
    assert_eq!(
        lifted, round_tripped,
        "lifted record must round-trip identity"
    );
}
