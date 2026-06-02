//! Live smoke test for the Gemini ("Nano Banana") adapter.
//!
//! `#[ignore]`d by default because it makes a real, billed Gemini API call.
//! Run it explicitly once `GEMINI_API_KEY` is set:
//!
//! ```bash
//! GEMINI_API_KEY=... cargo test -p diaspor-imagegen --features remote \
//!     --test live_gemini -- --ignored --nocapture
//! ```
//!
//! Set `IMAGEGEN_SMOKE_OUT=/path/to.png` to control where the result is written.
#![cfg(feature = "remote")]

use diaspor_imagegen::adapters::GeminiImageAdapter;
use diaspor_imagegen::{GenerateRequest, ImageGenerator};

#[tokio::test]
#[ignore = "makes a real, billed Gemini API call; needs GEMINI_API_KEY"]
async fn nano_banana_generates_a_real_image() {
    let adapter =
        GeminiImageAdapter::new(None).expect("GEMINI_API_KEY must be set in the environment");

    let request = GenerateRequest::new(
        "a single ripe banana centered on a clean white studio background, soft shadow, product photo",
        1024,
        1024,
    );

    let image = adapter
        .generate(&request)
        .await
        .expect("Nano Banana generation should succeed");

    assert!(!image.bytes.is_empty(), "generated image must have bytes");

    let out = std::env::var("IMAGEGEN_SMOKE_OUT").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("diaspor_imagegen_smoke.png")
            .to_string_lossy()
            .into_owned()
    });
    std::fs::write(&out, &image.bytes).expect("write output png");
    eprintln!(
        "SMOKE_OK: {} bytes, {}x{}, format={:?} -> {out}",
        image.bytes.len(),
        image.width,
        image.height,
        image.format,
    );
}
