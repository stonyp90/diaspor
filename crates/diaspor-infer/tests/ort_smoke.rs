//! End-to-end smoke test for the ORT-CPU backend.
//!
//! Loads the 119-byte identity ONNX fixture, builds an `OrtCpuInferenceBackend`, runs a
//! [1, 4] float32 tensor through it, and asserts the output bytes match the input.
//!
//! Gated on the `ort-cpu` feature. Skipped by default builds; `cargo test -p diaspor-infer
//! --features ort-cpu` is the invocation that exercises it. The on-tag release workflow
//! does not run this path — heavy / CUDA / EP-specific tests live in `release-heavy.yml`
//! and on local dev machines.

#![cfg(feature = "ort-cpu")]

use std::panic;
use std::path::PathBuf;

use bytes::Bytes;
use diaspor_infer::{
    AdapterId, DType, ModelId, OrtCpuConfig, OrtCpuInferenceBackend, Tensor, TenantId,
};

fn fixture_path() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.join("tests").join("fixtures").join("identity.onnx")
}

/// Builds an `OrtCpuInferenceBackend`, returning `None` when libonnxruntime is missing.
///
/// `ort` 2.x's `load-dynamic` mode panics (not Errs) when the system can't dlopen
/// libonnxruntime. We catch that panic here so the test skips cleanly on a dev machine
/// that hasn't installed onnxruntime yet — CI is expected to install the dylib in a
/// dedicated step before running these tests.
fn try_new_backend(config: OrtCpuConfig) -> Option<OrtCpuInferenceBackend> {
    let prior_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {})); // silence the panic message during the probe
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        OrtCpuInferenceBackend::new(config)
    }));
    panic::set_hook(prior_hook);
    match result {
        Ok(Ok(b)) => Some(b),
        Ok(Err(e)) => {
            eprintln!("skipping: ort backend constructor returned Err: {e}");
            None
        }
        Err(_) => {
            eprintln!(
                "skipping: ort panicked (libonnxruntime not on loader path). \
                 Install onnxruntime to run this test."
            );
            None
        }
    }
}

/// Builds an `OrtCpuInferenceBackend` from the on-disk identity fixture, runs a tiny
/// inference, and asserts the output bytes match the input.
///
/// Skipped when libonnxruntime is not present on the loader's search path (the `ort` crate
/// is built with `load-dynamic`, so a missing system library shows up as a runtime error
/// on `OrtCpuInferenceBackend::new`). In that case we emit a `tracing` event and pass,
/// so the test stays green on machines that have not yet installed onnxruntime.
#[tokio::test]
async fn identity_roundtrip() {
    use diaspor_infer::InferenceBackend;

    let fixture = fixture_path();
    assert!(
        fixture.is_file(),
        "fixture not found at {} — run the python generator in tests/fixtures/README.md",
        fixture.display()
    );

    let Some(backend) = try_new_backend(OrtCpuConfig {
        onnx_path: fixture,
        threads: 1,
    }) else {
        return;
    };

    // f32 input [1.0, 2.0, 3.0, 4.0], shape [1, 4].
    let floats: [f32; 4] = [1.0, 2.0, 3.0, 4.0];
    let mut bytes = Vec::with_capacity(floats.len() * 4);
    for f in floats {
        bytes.extend_from_slice(&f.to_ne_bytes());
    }
    let input = Tensor::new("input", vec![1, 4], DType::F32, Bytes::from(bytes.clone()));
    let batch = diaspor_infer::TensorBatch::new(vec![input]);

    let _tenant = TenantId::new("test");
    let model = ModelId::new("identity");
    let adapter: Option<&AdapterId> = None;
    let out = backend
        .run(&model, adapter, batch)
        .await
        .expect("ORT identity inference must succeed");

    assert_eq!(out.tensors.len(), 1, "expected one output tensor");
    let t = &out.tensors[0];
    assert_eq!(t.shape, vec![1, 4]);
    assert_eq!(t.dtype, DType::F32);
    assert_eq!(
        t.bytes.as_ref(),
        bytes.as_slice(),
        "identity should round-trip every byte"
    );
    assert!(out.latency_us > 0, "latency_us must be populated");
}

/// Rejects inputs whose byte length does not match shape * dtype, before ORT sees them.
#[tokio::test]
async fn malformed_input_rejected_early() {
    use diaspor_infer::InferenceBackend;

    let Some(backend) = try_new_backend(OrtCpuConfig {
        onnx_path: fixture_path(),
        threads: 1,
    }) else {
        return;
    };

    // Shape says [1, 4] = 4 floats = 16 bytes; we pass 8 bytes.
    let bad = Tensor::new("input", vec![1, 4], DType::F32, Bytes::from(vec![0u8; 8]));
    let batch = diaspor_infer::TensorBatch::new(vec![bad]);

    let err = backend
        .run(&ModelId::new("identity"), None, batch)
        .await
        .expect_err("ill-formed input must error before reaching ORT");
    let msg = err.to_string();
    assert!(
        msg.contains("does not match"),
        "expected 'does not match' in error, got: {msg}"
    );
}
