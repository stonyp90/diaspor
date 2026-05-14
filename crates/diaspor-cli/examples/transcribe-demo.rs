//! `transcribe-demo` — the public API surface contract for milestone M5.
//!
//! At v0.1.0-alpha.1 this example **does not transcribe anything**. Its job is
//! to demonstrate that the trait surface (`MediaExtractor`, `Transcriber`,
//! `Tagger`, `ContentPipeline`) compiles and composes today, so that downstream
//! crates can be written against the stable signatures before M5 lands the
//! real `FFmpeg` + `whisper.cpp` integration.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example transcribe-demo -- --input /path/to/audio.wav
//! cargo run --example transcribe-demo -- --input ./clip.mp3 \
//!     --output ./clip.sidecar.json --model ./ggml-base.en.bin
//! ```
//!
//! Today every invocation prints a clearly-labelled "TODO M5" banner, runs the
//! pipeline with no-op stub backends, and reports the sidecar shape that would
//! be produced. Real transcription work lives behind GitHub issue #11.

use std::path::PathBuf;

use async_trait::async_trait;
use bytes::Bytes;
use clap::Parser;
use diaspor_core::{Result, VfsPath};
use diaspor_index::{
    ContentPipeline, MediaExtractor, MediaInfo, TagSet, Tagger, Transcriber, Transcript,
};

/// Command-line arguments for the transcribe-demo example.
#[derive(Parser, Debug)]
#[command(
    name = "transcribe-demo",
    about = "Diaspor — M5 public API surface contract demo (does not yet transcribe)",
    long_about = "A no-op demonstration of the diaspor-index trait surface. Today this \
                  example prints the pipeline shape and exits; M5 swaps the stub backends \
                  out for FFmpeg + whisper.cpp. Track real work on GitHub issue #11."
)]
struct Args {
    /// Path to the input media file. At v0.1.0-alpha.1 the file is not actually
    /// read — only its path is propagated into the sidecar shape.
    #[arg(long, short)]
    input: PathBuf,

    /// Optional path for the sidecar JSON output. When M5 lands this will be
    /// the path the example writes the produced `SidecarRecord` to.
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Optional GGUF model path for the eventual `WhisperCppTranscriber`.
    /// Ignored today.
    #[arg(long, short)]
    model: Option<PathBuf>,
}

/// No-op extractor used so the example compiles end-to-end against the public
/// trait surface. Real M5 implementation: `FfmpegExtractor` (shells out to the
/// `ffmpeg` binary).
struct StubExtractor;

#[async_trait]
impl MediaExtractor for StubExtractor {
    async fn probe(&self, _path: &VfsPath, _bytes: &[u8]) -> Result<MediaInfo> {
        Ok(MediaInfo {
            container: "unknown".to_string(),
            duration_seconds: None,
            audio_codec: None,
            audio_sample_rate: None,
            audio_channels: None,
            video_codec: None,
        })
    }

    async fn extract_audio_pcm(&self, _path: &VfsPath, _bytes: &[u8]) -> Result<Bytes> {
        // Empty PCM. Real implementation will return 16 kHz mono 16-bit PCM.
        Ok(Bytes::new())
    }
}

/// No-op transcriber that returns an empty transcript. Real M5 implementation:
/// `WhisperCppTranscriber` via `whisper-rs`.
struct StubTranscriber;

#[async_trait]
impl Transcriber for StubTranscriber {
    fn name(&self) -> &'static str {
        "stub-transcriber"
    }

    async fn transcribe(&self, _audio_pcm: &[u8]) -> Result<Transcript> {
        Ok(Transcript {
            language: None,
            text: String::new(),
            segments: Vec::new(),
        })
    }
}

/// No-op tagger that returns an empty tag set. Real M6 implementation:
/// `OllamaTagger` or `LlamaCppTagger`.
struct StubTagger;

#[async_trait]
impl Tagger for StubTagger {
    fn name(&self) -> &'static str {
        "stub-tagger"
    }

    async fn tag(&self, _text: &str) -> Result<TagSet> {
        Ok(TagSet {
            tags: Vec::new(),
            categories: Vec::new(),
            summary: None,
        })
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("======================================================================");
    println!("  diaspor transcribe-demo (v0.1.0-alpha.1, public API surface only)");
    println!("======================================================================");
    println!();
    println!("TODO: M5 — wire whisper.cpp here. For v0.1.0-alpha.1 we only have the");
    println!("trait surface. See ROADMAP.md milestones M5 and M6, and GitHub issue");
    println!("#11 for the implementation tracking ticket.");
    println!();
    println!("Inputs received:");
    println!("  input  = {}", args.input.display());
    if let Some(output) = args.output.as_ref() {
        println!("  output = {}", output.display());
    } else {
        println!("  output = <unset> (M5 will default to <input>.sidecar.json)");
    }
    if let Some(model) = args.model.as_ref() {
        println!("  model  = {}", model.display());
    } else {
        println!("  model  = <unset> (M5 will require a GGUF model path or env var)");
    }
    println!();

    // Construct a no-op pipeline so callers can see the composition shape.
    let pipeline = ContentPipeline {
        extractor: StubExtractor,
        transcriber: StubTranscriber,
        tagger: StubTagger,
    };

    // Use the VFS path that the input would have *inside* a backend. Today the
    // example does not mount the host file into a backend — that wiring is M5
    // work. We only need a syntactically valid VfsPath.
    let vfs_path_string = format!(
        "/{}",
        args.input.file_name().map_or_else(
            || "unknown".to_string(),
            |n| n.to_string_lossy().into_owned()
        ),
    );
    let vfs_path = VfsPath::new(&vfs_path_string).ok_or("invalid VFS path derived from input")?;

    let record = pipeline.process(&vfs_path, &[]).await?;

    println!("Pipeline composed successfully. Stub SidecarRecord shape:");
    println!("  schema_version (will be \"1\" once serde lands in M6)");
    println!("  path           = {}", record.path);
    println!("  media.container = {}", record.media.container);
    println!("  transcript.text = {:?}", record.transcript.text);
    println!("  tags.tags       = {:?}", record.tags.tags);
    println!("  tags.categories = {:?}", record.tags.categories);
    println!("  extracted_with  = {}", record.extracted_with);
    println!("  extracted_at    = {}", record.extracted_at);
    println!();
    println!("Public schema reference: docs/schema/sidecar-v1.json");
    println!("Run finished. No transcription was performed (by design at v0.1.0-alpha.1).");

    Ok(())
}
