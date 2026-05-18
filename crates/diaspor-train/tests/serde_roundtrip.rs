//! Round-trip + schema-conformance tests for `diaspor-train` wire types.
//!
//! `diaspor-train` produces two flavours of JSON-shaped artifacts that the
//! API server and the tenant-facing handoff pipeline consume:
//!
//! 1. The [`AdapterArtifact`] envelope (with its [`EvalReport`] inside) is the
//!    handoff object that the tenant counter-signs on acceptance. It is
//!    JSON-round-trippable by itself; we test that here.
//!
//! 2. The minted [`AdapterId`] flows downstream to the inference pipeline and
//!    ends up in the `model_provenance[].adapter_id` field of every
//!    `docs/schema/score-v1.json` record produced while that adapter is
//!    active. We test that an `AdapterId` minted here, when stitched into a
//!    minimal score-v1 record, validates against the published schema.

use std::fs;
use std::path::PathBuf;

use bytes::Bytes;
use diaspor_train::{
    AdapterArtifact, AdapterId, CorpusManifest, CorpusSource, EvalReport, LoraConfig, TenantId,
};
use jsonschema::validator_for;
use serde_json::{Value, json};
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

fn load_schema() -> Value {
    let path = score_schema_path();
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read score schema at {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("score schema at {} is not valid JSON: {e}", path.display()))
}

fn realistic_artifact() -> AdapterArtifact {
    let eval = EvalReport {
        metric_name: "auc_roc".to_string(),
        baseline_score: 0.54,
        new_score: 0.72,
        delta: 0.18,
        passed: true,
    };
    AdapterArtifact {
        adapter_id: AdapterId::new("acme-credibility-v3"),
        tenant_id: TenantId::new("cust_acme"),
        base_model: "InternVideo2-1B".to_string(),
        safetensors_bytes: Bytes::from_static(b"placeholder-safetensors-bytes"),
        vendor_signature: Bytes::from_static(b"placeholder-vendor-signature"),
        tenant_signature: Some(Bytes::from_static(b"placeholder-tenant-countersignature")),
        trained_at: datetime!(2026-05-15 09:00:00 UTC),
        eval_report: Some(eval),
    }
}

#[test]
fn adapter_artifact_roundtrips_through_json() {
    let original = realistic_artifact();

    let json_string =
        serde_json::to_string_pretty(&original).expect("AdapterArtifact must serialize");
    let round_tripped: AdapterArtifact =
        serde_json::from_str(&json_string).expect("AdapterArtifact must deserialize");

    assert_eq!(
        original, round_tripped,
        "AdapterArtifact must round-trip identity through JSON"
    );
}

#[test]
fn corpus_manifest_roundtrips_through_json() {
    let original = CorpusManifest {
        tenant_id: TenantId::new("cust_acme"),
        clip_count: 1024,
        total_duration_seconds: 12_345.678,
        sha256_manifest: "3a7f1c2b9d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a"
            .to_string(),
        ingested_at: OffsetDateTime::from_unix_timestamp(1_768_000_000)
            .expect("hard-coded epoch must parse"),
    };

    let json_string =
        serde_json::to_string_pretty(&original).expect("CorpusManifest must serialize");
    let round_tripped: CorpusManifest =
        serde_json::from_str(&json_string).expect("CorpusManifest must deserialize");

    assert_eq!(
        original, round_tripped,
        "CorpusManifest must round-trip identity through JSON"
    );
}

#[test]
fn corpus_source_adjacent_tag_roundtrips() {
    let s3 = CorpusSource::S3Prefix("s3://tenant-bucket/corpus/".to_string());
    let s3_json = serde_json::to_string(&s3).expect("CorpusSource::S3Prefix must serialize");
    assert!(
        s3_json.contains("\"kind\":\"s3_prefix\""),
        "S3Prefix should carry an `s3_prefix` discriminator: {s3_json}"
    );
    let s3_round: CorpusSource =
        serde_json::from_str(&s3_json).expect("CorpusSource::S3Prefix must deserialize");
    assert_eq!(s3, s3_round);

    let vfs = CorpusSource::VfsPath(PathBuf::from("/tenant/clips"));
    let vfs_json = serde_json::to_string(&vfs).expect("CorpusSource::VfsPath must serialize");
    assert!(
        vfs_json.contains("\"kind\":\"vfs_path\""),
        "VfsPath should carry a `vfs_path` discriminator: {vfs_json}"
    );
    let vfs_round: CorpusSource =
        serde_json::from_str(&vfs_json).expect("CorpusSource::VfsPath must deserialize");
    assert_eq!(vfs, vfs_round);
}

#[test]
fn lora_config_default_roundtrips() {
    let original = LoraConfig::default();
    let json_string = serde_json::to_string_pretty(&original).expect("LoraConfig must serialize");
    let round_tripped: LoraConfig =
        serde_json::from_str(&json_string).expect("LoraConfig must deserialize");
    assert_eq!(original, round_tripped);
}

#[test]
fn adapter_id_propagates_into_valid_score_v1_record() {
    // An adapter trained here is referenced in every downstream score record
    // produced while it's loaded for inference. Verify the AdapterId-as-string
    // wire shape composes cleanly into the `model_provenance[].adapter_id`
    // field of `docs/schema/score-v1.json`.
    let artifact = realistic_artifact();

    let adapter_id_string = serde_json::to_string(&artifact.adapter_id)
        .expect("AdapterId must serialize as a JSON string");
    let adapter_inner = adapter_id_string.trim_matches('"').to_string();
    let tenant_id_string = serde_json::to_string(&artifact.tenant_id)
        .expect("TenantId must serialize as a JSON string");
    let tenant_inner = tenant_id_string.trim_matches('"').to_string();

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
        "stream_id": "stream-from-train",
        "tenant": tenant_inner,
        "t_start_ms": 12_000,
        "t_end_ms": 13_000,
        "kind": "window",
        "modalities": {
            "credibility": {
                "model": "diaspor-credibility-v1",
                "score": 0.41,
                "confidence_band": "low",
                "human_baseline_disclosed": 0.54,
                "ceiling_disclosed": 0.74,
                "labs_preview": true,
                "vertical_attestation": "coaching"
            },
            "pose": {
                "model": "diaspor-pose-3d-v1",
                "keypoints": keypoints,
            }
        },
        "extracted_at": "2026-05-15T12:30:13Z",
        "model_provenance": [
            {
                "model_name": format!("{}+lora-r16", artifact.base_model),
                "adapter_id": adapter_inner,
                "runtime": "triton",
                "latency_us": 31_200
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
        "train-supplied score-v1 record failed schema validation:\n{}\n---\n{score_record:#}",
        errors.join("\n")
    );
}

#[test]
fn eval_report_roundtrips_inside_adapter_artifact() {
    // The EvalReport lives inside AdapterArtifact and must survive a full
    // round-trip without losing any field — the gating-bit (`passed`) is
    // load-bearing for the handoff state machine.
    let mut artifact = realistic_artifact();
    artifact.eval_report = Some(EvalReport {
        metric_name: "mse".to_string(),
        baseline_score: 0.18,
        new_score: 0.09,
        delta: -0.09,
        passed: true,
    });
    artifact.tenant_signature = None;

    let json_string = serde_json::to_string_pretty(&artifact)
        .expect("AdapterArtifact (no tenant sig) must serialize");
    let round_tripped: AdapterArtifact = serde_json::from_str(&json_string)
        .expect("AdapterArtifact (no tenant sig) must deserialize");

    assert_eq!(artifact, round_tripped);
    let report = round_tripped
        .eval_report
        .as_ref()
        .expect("eval_report must round-trip Some(..)");
    assert!(report.passed);
}
