//! Round-trip + schema-conformance test for the `diaspor-infer` wire shape.
//!
//! `diaspor-infer` itself does not emit a full `score-v1` record — that happens
//! upstream in `diaspor-vision` / the API server. What it DOES emit, on every
//! inference call, are values that surface inside the score record's
//! `model_provenance[]` array:
//!
//! - the [`TenantId`] of the caller,
//! - the [`ModelId`] of the model that was run (and optionally a per-tenant
//!   [`AdapterId`]),
//! - the `runtime` label (`"triton"`, `"coreml"`, `"ort-cpu"`, `"deepstream"`),
//! - the `latency_us` from the returned [`TensorBatch`].
//!
//! This test stitches those pieces into a minimal but valid `score-v1` JSON
//! document, validates the result against `docs/schema/score-v1.json` with the
//! `jsonschema` crate, and round-trips a [`TensorBatch`] through JSON to assert
//! that the in-memory <-> wire conversion is information-preserving.

use std::fs;
use std::path::PathBuf;

use bytes::Bytes;
use diaspor_infer::{AdapterId, DType, ModelId, Tensor, TensorBatch, TenantId};
use jsonschema::validator_for;
use serde_json::{Value, json};

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

fn load_schema() -> Value {
    let path = score_schema_path();
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("failed to read score schema at {}: {e}", path.display())
    });
    serde_json::from_str(&raw).unwrap_or_else(|e| {
        panic!("score schema at {} is not valid JSON: {e}", path.display())
    })
}

#[test]
fn tensor_batch_roundtrips_through_json() {
    let original = TensorBatch::new(vec![
        Tensor::new(
            "pixel_values",
            vec![1, 3, 224, 224],
            DType::F32,
            Bytes::from_static(&[0u8; 8]),
        ),
        Tensor::new(
            "attention_mask",
            vec![1, 128],
            DType::U8,
            Bytes::from_static(&[1u8; 4]),
        ),
    ]);

    let json_string = serde_json::to_string_pretty(&original)
        .expect("TensorBatch must serialize cleanly");
    let parsed: Value =
        serde_json::from_str(&json_string).expect("serialized output must be valid JSON");
    // Spot-check the dtype rename.
    assert_eq!(
        parsed["tensors"][0]["dtype"], "fp32",
        "DType::F32 should serialize as `fp32` to match Triton convention; got {parsed}"
    );
    let round_tripped: TensorBatch = serde_json::from_str(&json_string)
        .expect("TensorBatch must deserialize cleanly");
    assert_eq!(
        original, round_tripped,
        "TensorBatch must round-trip identity through JSON"
    );
}

#[test]
fn infer_outputs_compose_into_valid_score_v1_record() {
    let tenant = TenantId::new("acme");
    let model = ModelId::new("diaspor-pose-3d-v1");
    let adapter = AdapterId::new("acme-pose-adapter-v3");
    // A faked inference output with a populated latency.
    let output = TensorBatch {
        tensors: Vec::new(),
        latency_us: 8400,
    };

    // The score-v1 record is mostly the vision crate's responsibility; here we
    // hand-build a minimal valid record so we can assert that the infer-crate
    // contributions (tenant, adapter, runtime, latency) flow into the
    // model_provenance entry without violating the schema.
    let tenant_string =
        serde_json::to_string(&tenant).expect("TenantId must serialize as a JSON string");
    let model_string =
        serde_json::to_string(&model).expect("ModelId must serialize as a JSON string");
    let adapter_string =
        serde_json::to_string(&adapter).expect("AdapterId must serialize as a JSON string");

    // Strip outer quotes — the schema embeds these as plain strings, not
    // JSON-quoted strings. (e.g. `"\"acme\""` -> `"acme"`.)
    let tenant_inner = tenant_string.trim_matches('"').to_string();
    let model_inner = model_string.trim_matches('"').to_string();
    let adapter_inner = adapter_string.trim_matches('"').to_string();

    // Keypoints use short-decimal values so the serialize/parse round-trip is
    // bit-exact; the floating-point representation of e.g. `0.99 - 0.01 * 7/33`
    // is not stable across JSON serializers and we don't want this test to
    // become a benchmark of `serde_json::Number` parsing.
    let keypoints: Vec<Value> = (0..33u32)
        .map(|_| {
            json!({
                "x": 0.5,
                "y": 0.5,
                "z": 0.0,
                "visibility": 0.95,
            })
        })
        .collect();

    let score_record = json!({
        "schema_version": "1",
        "stream_id": "stream-from-infer",
        "tenant": tenant_inner,
        "t_start_ms": 12_000,
        "t_end_ms": 13_000,
        "kind": "window",
        "modalities": {
            "pose": {
                "model": model_inner,
                "keypoints": keypoints,
            }
        },
        "extracted_at": "2026-05-15T12:30:13Z",
        "model_provenance": [
            {
                "model_name": format!("{model_inner}@blazepose-heavy"),
                "adapter_id": adapter_inner,
                "runtime": "coreml",
                "latency_us": output.latency_us,
            }
        ]
    });

    let schema = load_schema();
    let validator = validator_for(&schema)
        .expect("docs/schema/score-v1.json must parse as a JSON Schema 2020-12 doc");
    let errors: Vec<String> = validator
        .iter_errors(&score_record)
        .map(|err| format!("  - {} (at {})", err, err.instance_path))
        .collect();
    assert!(
        errors.is_empty(),
        "infer-supplied score-v1 record failed schema validation:\n{}\n---\n{score_record:#}",
        errors.join("\n")
    );

    // Serialize → parse round-trip asserts the schema-validated payload survives
    // a JSON edge. Short-decimal values above keep this exact.
    let serialized =
        serde_json::to_string(&score_record).expect("score record must serialize");
    let reparsed: Value =
        serde_json::from_str(&serialized).expect("score record must reparse");
    assert_eq!(
        score_record, reparsed,
        "score-v1 JSON must round-trip identity through serialize+parse"
    );
}
