//! Sidecar record types — the JSON shape persisted next to each indexed file.

use time::OffsetDateTime;

use crate::{MediaInfo, TagSet, Transcript};

/// One indexed file's full sidecar record.
///
/// Persisted as JSON at the conventional path `/.index/<file>.json` inside whichever
/// backend the indexer is wrapping. The exact serialization format is stabilized in
/// roadmap milestone M5 — for now this is a Rust struct only.
#[derive(Debug, Clone)]
pub struct SidecarRecord {
    /// Path of the file inside the VFS this record describes.
    pub path: String,
    /// `FFmpeg` probe output.
    pub media: MediaInfo,
    /// Whisper-style transcript.
    pub transcript: Transcript,
    /// Tag set produced by the local LLM.
    pub tags: TagSet,
    /// Identifier of the transcriber + tagger backends used, e.g.
    /// `"whisper.cpp-large-v3+ollama-llama3:8b"`.
    pub extracted_with: String,
    /// When the indexing run finished.
    pub extracted_at: OffsetDateTime,
}
