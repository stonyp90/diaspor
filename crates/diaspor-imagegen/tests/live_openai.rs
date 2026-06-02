//! Live smoke test for the `OpenAI` (`gpt-image-1`) adapter, exercised through
//! the [`ImageStudio`] router so it proves the whole integration, not just the
//! adapter.
//!
//! `#[ignore]`d because it makes a real, billed `OpenAI` API call. Run with:
//!
//! ```bash
//! OPENAI_API_KEY=... cargo test -p diaspor-imagegen --features remote \
//!     --test live_openai -- --ignored --nocapture
//! ```
#![cfg(feature = "remote")]

use std::sync::Arc;

use diaspor_imagegen::adapters::{LocalImageAdapter, OpenAiImageAdapter};
use diaspor_imagegen::{GenerateRequest, ImageStudio, Policy};

#[tokio::test]
#[ignore = "makes a real, billed OpenAI API call; needs OPENAI_API_KEY"]
async fn openai_generates_through_the_studio() {
    let openai =
        Arc::new(OpenAiImageAdapter::new(None).expect("OPENAI_API_KEY must be set in the env"));

    // Register OpenAI + the offline local adapter, then let the router choose.
    let studio = ImageStudio::builder()
        .generator(openai.clone())
        .generator(Arc::new(LocalImageAdapter::new()))
        .compositor(openai)
        .build()
        .expect("studio builds");

    // QualityFirst must route to OpenAI (quality 89) over local (25).
    let image = studio
        .generate(
            &GenerateRequest::new(
                "a single ripe banana centered on a clean white studio background, soft shadow, product photo",
                1024,
                1024,
            ),
            &Policy::QualityFirst,
        )
        .await
        .expect("studio generation should succeed");

    assert!(!image.bytes.is_empty(), "generated image must have bytes");

    let out = std::env::var("IMAGEGEN_SMOKE_OUT").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("diaspor_imagegen_openai.png")
            .to_string_lossy()
            .into_owned()
    });
    std::fs::write(&out, &image.bytes).expect("write output png");
    eprintln!(
        "SMOKE_OK: {} bytes, {}x{} via studio(QualityFirst) -> {out}",
        image.bytes.len(),
        image.width,
        image.height,
    );
}
