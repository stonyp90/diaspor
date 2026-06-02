//! Live end-to-end test of the **generate → composite** pipeline through
//! [`ImageStudio::generate_and_compose`], using the `OpenAI` adapter for both
//! steps (`/images/generations` then multipart `/images/edits`).
//!
//! `#[ignore]`d — makes several real, billed `OpenAI` calls. Run with:
//!
//! ```bash
//! OPENAI_API_KEY=... cargo test -p diaspor-imagegen --features remote \
//!     --test live_compose -- --include-ignored --nocapture
//! ```
#![cfg(feature = "remote")]

use std::sync::Arc;

use diaspor_imagegen::adapters::OpenAiImageAdapter;
use diaspor_imagegen::{GenerateRequest, ImageStudio, Policy};

#[tokio::test]
#[ignore = "makes several real, billed OpenAI API calls; needs OPENAI_API_KEY"]
async fn openai_generate_then_compose() {
    let openai =
        Arc::new(OpenAiImageAdapter::new(None).expect("OPENAI_API_KEY must be set in the env"));

    let studio = ImageStudio::builder()
        .generator(openai.clone())
        .compositor(openai)
        .build()
        .expect("studio builds");

    // Generate two isolated elements, then let the studio composite them.
    let prompts = [
        GenerateRequest::new(
            "a single ripe banana, centered, isolated on a plain white background, product cutout",
            1024,
            1024,
        ),
        GenerateRequest::new(
            "a white ceramic coffee mug, centered, isolated on a plain white background, product cutout",
            1024,
            1024,
        ),
    ];

    let image = studio
        .generate_and_compose(
            &prompts,
            "Combine the provided items into one clean product photo on a white desk: the banana resting against the coffee mug, with a soft natural shadow.",
            1024,
            1024,
            &Policy::QualityFirst,
        )
        .await
        .expect("generate_and_compose should succeed");

    assert!(!image.bytes.is_empty(), "composited image must have bytes");

    let out = std::env::var("IMAGEGEN_SMOKE_OUT").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("diaspor_compose.png")
            .to_string_lossy()
            .into_owned()
    });
    std::fs::write(&out, &image.bytes).expect("write output png");
    eprintln!(
        "SMOKE_OK: composed {} bytes, {}x{} -> {out}",
        image.bytes.len(),
        image.width,
        image.height,
    );
}
