//! End-to-end smoke test for [`diaspor_infer::ModelHub`].
//!
//! Exercises the catalog lookup -> download -> sha256-verify -> cache path against the
//! in-repo identity ONNX fixture (via `file://` URL). Gated on the `hub` feature.

#![cfg(feature = "hub")]

use std::path::PathBuf;

use diaspor_infer::{Catalog, HubConfig, HubError, ModelHub};

/// The test-only catalog points at the in-repo identity.onnx via `file://` so the test
/// does not touch the network. The `sha256` is left unpinned so the test can flip
/// `trust_unpinned = true` and verify that path; a separate test pins the value and
/// confirms successful checksum verification.
fn unpinned_catalog() -> Catalog {
    // Resolve the workspace-relative path the way the embedded catalog's test entry does.
    let toml_str = r#"schema_version = 1

[[models]]
id = "test-identity-fp32@1"
description = "identity fixture"
format = "onnx"
url = "file://crates/diaspor-infer/tests/fixtures/identity.onnx"
sha256 = "PENDING_FIRST_PULL"
bytes = 0
license = "Apache-2.0"
feature_gate = "ort-cpu"
nonfree = false
"#;
    Catalog::from_toml_str(toml_str).expect("test catalog must parse")
}

fn pinned_catalog() -> Catalog {
    // Pinned to the actual sha256 + bytes of identity.onnx as committed to the repo.
    // If the fixture is ever regenerated, update both this string and
    // tests/fixtures/README.md.
    let toml_str = r#"schema_version = 1

[[models]]
id = "test-identity-fp32@1"
description = "identity fixture"
format = "onnx"
url = "file://crates/diaspor-infer/tests/fixtures/identity.onnx"
sha256 = "5c1b582a48c94cb880ba53f9f1453e3a922979d51f94fd4e961e009e634a5b2a"
bytes = 119
license = "Apache-2.0"
feature_gate = "ort-cpu"
nonfree = false
"#;
    Catalog::from_toml_str(toml_str).expect("test catalog must parse")
}

#[tokio::test]
async fn unpinned_resolve_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let config = HubConfig {
        root: tmp.path().to_path_buf(),
        offline: false,
        trust_unpinned: true,
        allow_nonfree: false,
    };
    let hub = ModelHub::with_catalog(config, unpinned_catalog());

    // First call: copies the file from the repo to the cache.
    let path = hub
        .resolve("test-identity-fp32@1")
        .await
        .expect("first resolve must succeed");
    assert!(path.is_file(), "resolved path must exist on disk");
    let first_bytes = std::fs::read(&path).unwrap();
    assert_eq!(first_bytes.len(), 119);

    // Second call: hits the cache, returns the same path.
    let again = hub
        .resolve("test-identity-fp32@1")
        .await
        .expect("second resolve must hit cache");
    assert_eq!(path, again);
}

#[tokio::test]
async fn pinned_resolve_verifies_checksum() {
    let tmp = tempfile::tempdir().unwrap();
    let config = HubConfig {
        root: tmp.path().to_path_buf(),
        offline: false,
        trust_unpinned: false,
        allow_nonfree: false,
    };
    let hub = ModelHub::with_catalog(config, pinned_catalog());

    let path = hub
        .resolve("test-identity-fp32@1")
        .await
        .expect("pinned resolve must succeed when the file matches");
    assert!(path.is_file());
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 119);
}

#[tokio::test]
async fn offline_blocks_cold_resolve() {
    let tmp = tempfile::tempdir().unwrap();
    let config = HubConfig {
        root: tmp.path().to_path_buf(),
        offline: true,
        trust_unpinned: false,
        allow_nonfree: false,
    };
    let hub = ModelHub::with_catalog(config, pinned_catalog());

    let err = hub
        .resolve("test-identity-fp32@1")
        .await
        .expect_err("offline + cold cache must refuse the fetch");
    assert!(
        matches!(err, HubError::NetworkBlocked { .. }),
        "expected NetworkBlocked, got: {err:?}"
    );
}

#[tokio::test]
async fn offline_allows_warm_cache_hit() {
    let tmp = tempfile::tempdir().unwrap();
    // First, warm the cache with offline=false.
    let warm = HubConfig {
        root: tmp.path().to_path_buf(),
        offline: false,
        trust_unpinned: false,
        allow_nonfree: false,
    };
    let hub_warm = ModelHub::with_catalog(warm, pinned_catalog());
    let warmed = hub_warm.resolve("test-identity-fp32@1").await.unwrap();
    assert!(warmed.is_file());

    // Now flip to offline. The cache hit should still succeed.
    let cold = HubConfig {
        root: tmp.path().to_path_buf(),
        offline: true,
        trust_unpinned: false,
        allow_nonfree: false,
    };
    let hub_cold = ModelHub::with_catalog(cold, pinned_catalog());
    let again = hub_cold.resolve("test-identity-fp32@1").await.unwrap();
    assert_eq!(warmed, again);
}

#[tokio::test]
async fn local_path_does_not_download() {
    let tmp = tempfile::tempdir().unwrap();
    let config = HubConfig {
        root: tmp.path().to_path_buf(),
        offline: true,
        trust_unpinned: false,
        allow_nonfree: false,
    };
    let hub = ModelHub::with_catalog(config, pinned_catalog());

    // local_path returns what *would* be the cache path; it must NOT touch disk.
    let speculative = hub
        .local_path("test-identity-fp32@1")
        .expect("known id must yield a speculative path");
    assert!(
        speculative.starts_with(tmp.path()),
        "speculative path {} must be inside cache root {}",
        speculative.display(),
        tmp.path().display()
    );
    // And of course the file doesn't exist yet.
    assert!(!speculative.exists());
}

/// Catches a regression where someone updates `models.toml` without committing the
/// matching fixture sha256 here. If the bytes change, this fails until both are updated.
#[test]
fn identity_fixture_sha256_is_stable() {
    use sha2::{Digest, Sha256};

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("identity.onnx");
    let bytes = std::fs::read(&path).expect("identity.onnx must exist");
    let mut h = Sha256::new();
    h.update(&bytes);
    let actual = hex::encode(h.finalize());
    assert_eq!(
        actual, "5c1b582a48c94cb880ba53f9f1453e3a922979d51f94fd4e961e009e634a5b2a",
        "identity.onnx sha256 drift — update tests/fixtures/README.md AND pinned_catalog()"
    );
    assert_eq!(bytes.len(), 119);
}
