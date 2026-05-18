//! Conformance lock for `docs/schema/score-v1.json`.
//!
//! Loads the published schema from the repository root, walks its `examples`
//! array, and validates every example against the schema using a JSON Schema
//! 2020-12 validator. If any example drifts out of compliance with the schema
//! it claims to illustrate, this test fails the build before the change can
//! reach a published release — keeping downstream consumers honest.
//!
//! The corresponding in-memory Rust types live in
//! `crates/diaspor-vision/src/record.rs` (per ADR 0007).

use std::fs;
use std::path::PathBuf;

use jsonschema::validator_for;
use serde_json::Value;

/// Workspace-rooted path to the v1 score schema.
///
/// `CARGO_MANIFEST_DIR` is `crates/diaspor-conformance`, so the workspace root
/// is two directories above. Building the path this way keeps the test
/// runnable regardless of where `cargo test` is invoked from.
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

#[test]
fn score_v1_examples_validate_against_score_v1_schema() {
    let schema_path = score_schema_path();
    let raw = fs::read_to_string(&schema_path).unwrap_or_else(|e| {
        panic!(
            "failed to read score schema at {}: {e}",
            schema_path.display()
        )
    });
    let schema: Value = serde_json::from_str(&raw).unwrap_or_else(|e| {
        panic!(
            "score schema at {} is not valid JSON: {e}",
            schema_path.display()
        )
    });

    let examples = schema
        .get("examples")
        .unwrap_or_else(|| {
            panic!(
                "score schema at {} is missing a top-level `examples` array; \
                 the conformance lock requires at least one example",
                schema_path.display()
            )
        })
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "score schema at {} has a non-array `examples` field",
                schema_path.display()
            )
        });

    assert!(
        !examples.is_empty(),
        "score schema at {} declared `examples` but the array is empty; \
         add at least one canonical example so this test has something to lock",
        schema_path.display()
    );

    let validator = validator_for(&schema).unwrap_or_else(|e| {
        panic!(
            "score schema at {} is not itself a well-formed JSON Schema 2020-12 \
             document: {e}",
            schema_path.display()
        )
    });

    let mut failures: Vec<String> = Vec::new();
    for (idx, example) in examples.iter().enumerate() {
        let errors: Vec<String> = validator
            .iter_errors(example)
            .map(|err| format!("  - {} (at {})", err, err.instance_path))
            .collect();
        if !errors.is_empty() {
            failures.push(format!(
                "examples[{idx}] failed schema validation:\n{}",
                errors.join("\n")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "score-v1.json examples diverged from the schema they illustrate:\n\n{}",
        failures.join("\n\n")
    );
}
