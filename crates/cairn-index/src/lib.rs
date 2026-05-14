//! # cairn-index
//!
//! Content-aware indexing layer for `cairn` backends.
//!
//! This crate adds *local-first* content understanding to any [`cairn_core::VfsBackend`].
//! When a media file lands in the filesystem, the indexer pipeline:
//!
//! 1. **Probes** the file with `FFmpeg` to identify codec / streams / duration.
//! 2. **Extracts** the audio track losslessly into 16 kHz mono PCM via `FFmpeg`.
//! 3. **Transcribes** the audio using a pluggable [`Transcriber`] (default backend:
//!    `whisper.cpp`, all inference local).
//! 4. **Tags** the resulting transcript using a pluggable [`Tagger`] (default backend:
//!    a small local LLM via `ollama` or `llama.cpp`).
//! 5. **Stores** transcript + tags as a *sidecar* JSON record retrievable through the
//!    VFS itself — no cloud, no telemetry, no external service unless the caller
//!    explicitly configures one.
//!
//! ## Pipeline at a glance
//!
//! ```text
//!   ┌──────────┐   `FFmpeg` probe   ┌──────────┐   `FFmpeg` extract   ┌──────────┐
//!   │  file in │ ───────────────▶ │  metadata│ ─────────────────▶ │  PCM 16k │
//!   │ backend  │                  │ {codec…} │                    │  mono    │
//!   └────┬─────┘                  └──────────┘                    └────┬─────┘
//!        │                                                              │
//!        │  sidecar                                                     ▼
//!        │  {transcript, tags, lang}            ┌─────────────────────────────┐
//!        │ ◀───────────────────────────────────┤  Transcriber (whisper.cpp)  │
//!        │                                      └────────────┬────────────────┘
//!        │                                                   ▼
//!        │                                      ┌─────────────────────────────┐
//!        └─────────────────────────────────────▶│  Tagger (local LLM)         │
//!                                               └─────────────────────────────┘
//! ```
//!
//! ## Privacy contract
//!
//! - **No network calls by default.** The default [`Transcriber`] and [`Tagger`] run
//!   on-device. Cloud variants (`OpenAI`, Anthropic, etc.) are opt-in via separate feature
//!   flags and require the caller to construct them explicitly.
//! - **Bring-your-own-model.** The crate ships traits, not models. Callers point the
//!   pipeline at their preferred whisper.cpp build, GGUF model, or LLM runtime.
//! - **Sidecar storage stays in the backend.** Transcripts never leave the VFS unless
//!   the caller explicitly copies them out.
//!
//! ## Status
//!
//! v0.1.0-alpha ships **the trait surface and a no-op probe** so the architecture is
//! reviewable. Full `FFmpeg` integration arrives in milestone M5 of the roadmap, with
//! transcription and auto-tagging in M6.

#![doc(html_root_url = "https://docs.rs/cairn-index/0.1.0-alpha.1")]

use async_trait::async_trait;
use bytes::Bytes;
use cairn_core::{Result, VfsPath};
use thiserror::Error;

pub mod sidecar;

/// Things that can go wrong specifically in the indexer pipeline.
///
/// Wraps cleanly into a [`cairn_core::VfsError::Backend`] when bubbled up.
#[derive(Debug, Error)]
pub enum IndexError {
    /// `FFmpeg` binary not found or returned a non-zero exit code.
    #[error("ffmpeg failed: {0}")]
    FfmpegFailed(String),

    /// The transcriber backend rejected the audio or failed mid-stream.
    #[error("transcriber failed: {0}")]
    TranscriberFailed(String),

    /// The tagger backend rejected the input or failed mid-stream.
    #[error("tagger failed: {0}")]
    TaggerFailed(String),

    /// The file is not a media file the pipeline knows how to process.
    #[error("unsupported media format at {path}")]
    UnsupportedMedia {
        /// VFS path of the offending file.
        path: String,
    },
}

/// Output of [`MediaExtractor::probe`].
#[derive(Debug, Clone)]
pub struct MediaInfo {
    /// Container format reported by `FFmpeg` (mp4, mkv, mp3, wav, …).
    pub container: String,
    /// Duration in seconds, if known.
    pub duration_seconds: Option<f64>,
    /// Codec name of the primary audio stream, if any.
    pub audio_codec: Option<String>,
    /// Sample rate of the primary audio stream, if any.
    pub audio_sample_rate: Option<u32>,
    /// Number of audio channels, if any.
    pub audio_channels: Option<u16>,
    /// Codec name of the primary video stream, if any.
    pub video_codec: Option<String>,
}

/// Extracts audio (and metadata) from media files via `FFmpeg`.
#[async_trait]
pub trait MediaExtractor: Send + Sync {
    /// Probes a file's metadata without decoding it.
    async fn probe(&self, path: &VfsPath, bytes: &[u8]) -> Result<MediaInfo>;

    /// Decodes the file's primary audio stream into 16 kHz mono 16-bit PCM bytes.
    async fn extract_audio_pcm(&self, path: &VfsPath, bytes: &[u8]) -> Result<Bytes>;
}

/// Output of [`Transcriber::transcribe`].
#[derive(Debug, Clone)]
pub struct Transcript {
    /// Detected language code (`en`, `fr`, `de`, …) or `None` if unknown.
    pub language: Option<String>,
    /// The full transcript text.
    pub text: String,
    /// Optional segment timestamps (`start_seconds`, `end_seconds`, `segment_text`).
    pub segments: Vec<TranscriptSegment>,
}

/// One timestamped segment of a transcript.
#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    /// Start of the segment in seconds from the beginning of the audio.
    pub start_seconds: f64,
    /// End of the segment in seconds from the beginning of the audio.
    pub end_seconds: f64,
    /// Text of the segment.
    pub text: String,
}

/// Turns raw audio PCM into a [`Transcript`]. Default implementation will wrap
/// `whisper.cpp` for on-device inference.
#[async_trait]
pub trait Transcriber: Send + Sync {
    /// Human-readable name of the transcriber, for logs.
    fn name(&self) -> &'static str;

    /// Transcribes 16 kHz mono 16-bit PCM audio.
    async fn transcribe(&self, audio_pcm: &[u8]) -> Result<Transcript>;
}

/// Output of [`Tagger::tag`].
#[derive(Debug, Clone)]
pub struct TagSet {
    /// Semantic tags inferred from the transcript or other text content.
    pub tags: Vec<String>,
    /// Suggested categories (broader than tags — e.g. "meeting", "interview").
    pub categories: Vec<String>,
    /// One-sentence summary, if the tagger produces one.
    pub summary: Option<String>,
}

/// Produces semantic tags from text. Default implementation will wrap a small local LLM
/// via `ollama` or `llama.cpp`.
#[async_trait]
pub trait Tagger: Send + Sync {
    /// Human-readable name of the tagger, for logs.
    fn name(&self) -> &'static str;

    /// Generates tags / categories / summary for the given text.
    async fn tag(&self, text: &str) -> Result<TagSet>;
}

/// The composed pipeline.
///
/// In production: wrap an existing [`cairn_core::VfsBackend`] so that every newly
/// written media file is automatically processed. Sidecar records land in a parallel
/// path tree (`/.index/foo.mp4.json` for `/foo.mp4`) and are queryable via normal VFS
/// reads.
pub struct ContentPipeline<E, T, G> {
    /// `FFmpeg`-backed media extractor.
    pub extractor: E,
    /// Transcriber (default: whisper.cpp).
    pub transcriber: T,
    /// Tagger (default: local LLM).
    pub tagger: G,
}

impl<E, T, G> ContentPipeline<E, T, G>
where
    E: MediaExtractor,
    T: Transcriber,
    G: Tagger,
{
    /// Runs the full pipeline on a file's bytes and returns a sidecar record.
    ///
    /// # Errors
    ///
    /// Bubbles up the first error from probe, extraction, transcription, or tagging.
    pub async fn process(&self, path: &VfsPath, bytes: &[u8]) -> Result<sidecar::SidecarRecord> {
        let info = self.extractor.probe(path, bytes).await?;
        let pcm = self.extractor.extract_audio_pcm(path, bytes).await?;
        let transcript = self.transcriber.transcribe(&pcm).await?;
        let tags = self.tagger.tag(&transcript.text).await?;
        Ok(sidecar::SidecarRecord {
            path: path.as_str().to_string(),
            media: info,
            transcript,
            tags,
            extracted_with: format!(
                "{transcriber}+{tagger}",
                transcriber = self.transcriber.name(),
                tagger = self.tagger.name(),
            ),
            extracted_at: time::OffsetDateTime::now_utc(),
        })
    }
}
