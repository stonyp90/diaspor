//! `vision-batch-folder` — public API surface contract for milestone **M7**.
//!
//! At `v0.1.0-alpha.1` this example **does not actually run pose / face / prosody
//! extraction**. Its job is to demonstrate that the `diaspor-vision` trait surface
//! ([`PoseExtractor`], [`FaceLandmarkExtractor`], [`ProsodyExtractor`]) composes through
//! [`VisionPipeline`] today, so callers can build against the stable signatures before
//! M7 lands the real `MediaPipe BlazePose` + `MediaPipe FaceMesh` + `openSMILE`
//! integration.
//!
//! Running the example end-to-end exercises the composition path and surfaces a
//! [`VisionError::NotImplemented`] from the [`NoopPoseExtractor`] (the first modality in
//! the pipeline). The non-zero exit status is part of the demo: it tells the operator
//! that the trait surface is in place but the backends have not been wired yet.
//!
//! Run it with:
//!
//! ```bash
//! # Default placeholder path:
//! cargo run --example vision-batch-folder
//!
//! # Or pass your own mp4 path:
//! cargo run --example vision-batch-folder -- /path/to/clip.mp4
//! ```
//!
//! See ROADMAP.md milestone M7 for the implementation tracking work.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use bytes::Bytes;
use diaspor_vision::{
    NoopFaceLandmarkExtractor, NoopPoseExtractor, NoopProsodyExtractor, VisionPipeline,
};

/// Default mp4 path used when the caller does not pass one on argv. The file is NOT
/// required to exist — at the alpha stage no bytes are read; the trait surface short-
/// circuits long before any I/O.
const DEFAULT_PLACEHOLDER_MP4: &str = "/tmp/diaspor-placeholder-clip.mp4";

#[tokio::main]
async fn main() -> ExitCode {
    let input: PathBuf = env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from(DEFAULT_PLACEHOLDER_MP4), PathBuf::from);

    println!("======================================================================");
    println!("  diaspor vision-batch-folder (v0.1.0-alpha.1, M7 surface only)");
    println!("======================================================================");
    println!();
    println!("Composing VisionPipeline with the three Noop extractors:");
    println!("  pose    = NoopPoseExtractor          (production: MediaPipe BlazePose 3D)");
    println!("  face    = NoopFaceLandmarkExtractor  (production: MediaPipe FaceMesh)");
    println!("  prosody = NoopProsodyExtractor       (production: openSMILE eGeMAPSv02)");
    println!();
    println!("Input mp4: {}", input.display());
    println!();

    // Build the same shape M7 will use: three real extractors swapped for Noop stubs.
    let pipeline = VisionPipeline {
        pose: NoopPoseExtractor,
        face: NoopFaceLandmarkExtractor,
        prosody: NoopProsodyExtractor,
    };

    // The alpha trait surface takes a single decoded frame + the matching audio window.
    // M7 will wrap this with diaspor-frame-pipeline's UniformFrameSampler, but for the
    // composition demo we feed empty bytes — every Noop stub short-circuits without
    // looking at the input.
    let frame_bytes = Bytes::new();
    let audio_pcm = Bytes::new();

    println!("Calling VisionPipeline::process(&frame_bytes, &audio_pcm) ...");
    match pipeline.process(&frame_bytes, &audio_pcm).await {
        Ok(record) => {
            // This branch is unreachable at v0.1.0-alpha.1 — kept so the example
            // compiles as a forward-looking template once M7 lands real backends.
            println!();
            println!("UNEXPECTED Ok at the alpha — VisionRecord produced:");
            println!("  extracted_at      = {}", record.extracted_at);
            println!("  pose_provenance   = {}", record.pose_provenance.model_name);
            println!("  face_provenance   = {}", record.face_provenance.model_name);
            println!(
                "  prosody_provenance = {}",
                record.prosody_provenance.model_name
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            println!();
            println!("Pipeline failed (this is the EXPECTED v0.1.0-alpha.1 behavior).");
            println!();
            println!("  stage     = pose (first modality in the chain)");
            println!("  backend   = noop-pose");
            println!("  error     = {err}");
            println!();
            println!("This is correct: the trait surface composes but the backends are");
            println!("stubs. Real MediaPipe / openSMILE wiring lands in milestone M7 —");
            println!("see ROADMAP.md for the tracking work.");
            ExitCode::FAILURE
        }
    }
}
