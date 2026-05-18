//! File-based ingest adapter (Phase 1 — M7).
//!
//! Wraps a path to a media file on the local filesystem and replays it as if it were a
//! live stream: a single [`IngestEvent::SessionStarted`], a sequence of
//! [`IngestEvent::FramesArrived`] sliced from the file's bytes, and one final
//! [`IngestEvent::SessionEnded`] with [`SessionEndReason::Completed`].
//!
//! This is the simplest adapter: it has no network IO and no consent gate. It exists
//! primarily so the rest of the pipeline (`diaspor-frame-pipeline`, `diaspor-vision`,
//! `diaspor-index`) has a deterministic input source for tests and demos before the
//! live transports (WHIP, meeting-bot) come online.
//!
//! ## Status
//!
//! The trait surface is stable. Full wiring (chunked reads through a
//! [`diaspor_core::VfsBackend`], real PTS extraction, codec-aware batching) lands in
//! milestone **M7 — batch ingest**.

use std::path::PathBuf;
use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use diaspor_core::Result;
use futures::Stream;
use time::OffsetDateTime;

use crate::{
    IngestEvent, IngestStream, SessionEndReason, SessionId, StreamIngest, StreamIngestError,
};

/// Size of each `FramesArrived` batch when re-reading a file, in bytes.
///
/// 64 KiB is a placeholder — final value will be codec-aware (NAL unit boundaries for
/// H.264, packet boundaries for Opus) once the M7 wiring lands.
const FILE_BATCH_BYTES: usize = 64 * 1024;

/// File-based ingest adapter.
///
/// Construct with [`FileIngest::new`], then call [`StreamIngest::start`] to drain the
/// file as a stream of [`IngestEvent`] values.
#[derive(Debug, Clone)]
pub struct FileIngest {
    /// Path to the file that will be replayed as a stream.
    path: PathBuf,
    /// Optional explicit session id; if `None`, one will be derived from the path on
    /// each call to `start()`.
    session_id: Option<SessionId>,
}

impl FileIngest {
    /// Constructs a new file-ingest adapter pointing at `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            session_id: None,
        }
    }

    /// Overrides the auto-derived [`SessionId`] for this adapter.
    ///
    /// Useful when the caller has an external correlation id (e.g. a job id) it wants
    /// to tag downstream sidecar records with.
    #[must_use]
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Returns the path this adapter is bound to.
    #[must_use]
    pub const fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Derives a [`SessionId`] from the wrapped path when the caller did not supply one.
    fn derive_session_id(&self) -> SessionId {
        self.session_id
            .clone()
            .unwrap_or_else(|| SessionId::new(format!("file::{}", self.path.display())))
    }
}

#[async_trait]
impl StreamIngest for FileIngest {
    fn name(&self) -> &'static str {
        "file"
    }

    async fn start(&self) -> Result<IngestStream> {
        // Read the full file once. M7 will swap this for a chunked VfsBackend read so
        // arbitrarily large files don't materialize entirely in memory.
        let bytes = tokio::fs::read(&self.path).await.map_err(|err| {
            StreamIngestError::Transport(format!("failed to read {}: {err}", self.path.display()))
        })?;

        let session_id = self.derive_session_id();
        let started_at = OffsetDateTime::now_utc();

        let stream = build_file_stream(session_id, bytes, started_at);
        Ok(Box::pin(stream) as Pin<Box<dyn Stream<Item = Result<IngestEvent>> + Send>>)
    }
}

/// Builds the `SessionStarted` → `FramesArrived`\* → `SessionEnded` sequence for a
/// file's bytes. Factored out so it stays testable without touching the filesystem.
fn build_file_stream(
    session_id: SessionId,
    bytes: Vec<u8>,
    started_at: OffsetDateTime,
) -> impl Stream<Item = Result<IngestEvent>> + Send {
    let bytes = Bytes::from(bytes);
    futures::stream::unfold(
        FileStreamState {
            session_id,
            bytes,
            offset: 0,
            started_at,
            emitted_start: false,
            emitted_end: false,
        },
        |mut state| async move {
            // 1) Emit SessionStarted exactly once.
            if !state.emitted_start {
                state.emitted_start = true;
                let event = IngestEvent::SessionStarted {
                    session_id: state.session_id.clone(),
                    started_at: state.started_at,
                };
                return Some((Ok(event), state));
            }

            // 2) Emit FramesArrived until the buffer is drained.
            if state.offset < state.bytes.len() {
                let end = (state.offset + FILE_BATCH_BYTES).min(state.bytes.len());
                let batch = state.bytes.slice(state.offset..end);
                #[allow(clippy::cast_possible_truncation)]
                let timestamp_us = state.offset as u64;
                let event = IngestEvent::FramesArrived {
                    session_id: state.session_id.clone(),
                    batch_bytes: batch,
                    timestamp_us,
                };
                state.offset = end;
                return Some((Ok(event), state));
            }

            // 3) Emit SessionEnded exactly once, then terminate.
            if !state.emitted_end {
                state.emitted_end = true;
                let event = IngestEvent::SessionEnded {
                    session_id: state.session_id.clone(),
                    reason: SessionEndReason::Completed,
                    ended_at: OffsetDateTime::now_utc(),
                };
                return Some((Ok(event), state));
            }

            None
        },
    )
}

/// Internal driver state for [`build_file_stream`].
struct FileStreamState {
    session_id: SessionId,
    bytes: Bytes,
    offset: usize,
    started_at: OffsetDateTime,
    emitted_start: bool,
    emitted_end: bool,
}
