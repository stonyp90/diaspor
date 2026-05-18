//! # diaspor-stream-ingest
//!
//! Stream-ingest layer for `diaspor`. Pluggable adapters that turn an external media
//! source (a file on disk, an HLS manifest, a WHIP push from a browser, or a meeting
//! recording bot) into a uniform stream of [`IngestEvent`] values that downstream
//! crates — `diaspor-frame-pipeline`, `diaspor-vision`, `diaspor-index` — can consume
//! without caring where the bytes came from.
//!
//! ## Why a single trait
//!
//! Every ingest path produces the same logical event sequence:
//!
//! 1. `SessionStarted` — a stable [`SessionId`] is minted, downstream subscribers can
//!    open writers / pipelines keyed on it.
//! 2. Zero or more `FramesArrived` — opaque encoded media bytes plus a monotonically
//!    increasing presentation timestamp in microseconds.
//! 3. `SessionEnded` — terminal, with a [`SessionEndReason`] explaining whether the
//!    source closed cleanly, the client vanished, the bot was ejected, or a participant
//!    declined consent (meeting-bot specific).
//!
//! Adapters differ only in *how* the bytes arrive (a file handle, an HTTP poll, a WebRTC
//! transport, a Recall.ai webhook). The pipeline above does not need to know.
//!
//! ## Adapters at a glance
//!
//! ```text
//!   ┌────────────────────┐
//!   │ FileIngest         │ ── disk bytes ──▶ ┐
//!   ├────────────────────┤                   │
//!   │ WhipIngest         │ ── WebRTC ──────▶ │
//!   ├────────────────────┤                   ├──▶ Stream<IngestEvent>
//!   │ HlsIngest          │ ── HTTP pull ───▶ │
//!   ├────────────────────┤                   │
//!   │ MeetingBotIngest   │ ── Recall.ai ───▶ ┘
//!   └────────────────────┘
//! ```
//!
//! ## Privacy & compliance contract
//!
//! - **No network calls by default.** [`FileIngest`] is the only adapter that runs without
//!   external dependencies. [`WhipIngest`], [`HlsIngest`] and [`MeetingBotIngest`] are
//!   *placeholder* surfaces in v0.1 — wiring lands later in the roadmap (see [Status]).
//! - **All-party consent gating in `meeting_bot`.** See the module-level docs of
//!   [`meeting_bot`] for the legal invariants (Loi 25, BIPA, GDPR, EU AI Act) that any
//!   real implementation must honor before emitting a single `FramesArrived` event.
//! - **No telemetry.** Events stay in-process. Persisting them is the caller's choice.
//!
//! ## Status
//!
//! v0.1.0-alpha ships the trait surface only. File ingest lands in M7 (batch); WHIP +
//! meeting-bot via Recall.ai land in M8 (live); LL-HLS pull and direct platform SDKs in
//! M9.
//!
//! [Status]: #status

#![doc(html_root_url = "https://docs.rs/diaspor-stream-ingest/0.1.0-alpha.1")]

use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use diaspor_core::{Result, VfsError};
use futures::Stream;
use thiserror::Error;
use time::OffsetDateTime;

pub mod file;
pub mod hls;
pub mod meeting_bot;
pub mod whip;

pub use file::FileIngest;
pub use hls::{HlsConfig, HlsIngest};
pub use meeting_bot::{BotProvider, MeetingBotConfig, MeetingBotIngest};
pub use whip::{WhipConfig, WhipIngest};

/// Convenient alias for the boxed event stream returned by [`StreamIngest::start`].
///
/// Pinned and `Send` so it can cross task boundaries; items are `Result<IngestEvent>`
/// so an adapter can surface mid-stream errors without tearing down the whole stream.
pub type IngestStream = Pin<Box<dyn Stream<Item = Result<IngestEvent>> + Send>>;

/// Opaque identifier minted by an adapter when it opens a new ingest session.
///
/// Downstream crates key writers, pipelines, and sidecar records on this value. The
/// inner string is adapter-defined (UUIDs, ULIDs, meeting IDs, …) and SHOULD be stable
/// across the whole `SessionStarted` → `SessionEnded` window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    /// Constructs a session id from any string-like value.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrows the inner id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why an ingest session terminated.
///
/// Adapters MUST emit exactly one [`IngestEvent::SessionEnded`] per session with one of
/// these reasons attached. The meeting-bot specific variants exist so downstream
/// retention / consent logging can branch without string-matching error messages.
#[derive(Debug, Clone)]
pub enum SessionEndReason {
    /// The source closed cleanly (end of file, end of stream, host ended meeting).
    Completed,
    /// The client / source disappeared without sending a graceful close.
    ClientDisconnected,
    /// Adapter-internal failure — message is operator-facing, not user-facing.
    Failed(String),
    /// Meeting-bot specific: the meeting host removed the bot from the call. Treated as
    /// implicit consent withdrawal — any cached bytes for this session should be deleted
    /// per the retention policy enforced by the caller.
    BotEjected,
    /// Meeting-bot specific: a participant declined the recording consent prompt. The
    /// bot MUST stop emitting frames before this event lands and MUST NOT have persisted
    /// any pre-consent buffer.
    BotConsentDeclined,
}

/// One discrete event from an ingest adapter.
///
/// See the crate-level docs for the invariant event sequence each adapter emits.
#[derive(Debug, Clone)]
pub enum IngestEvent {
    /// A new ingest session has opened. Always the first event for a given
    /// [`SessionId`].
    SessionStarted {
        /// Identifier callers should key on for the rest of the session.
        session_id: SessionId,
        /// Wall-clock time the session opened, in UTC.
        started_at: OffsetDateTime,
    },

    /// A batch of encoded media bytes arrived.
    ///
    /// `batch_bytes` is opaque — codec / container handling is the downstream pipeline's
    /// job. `timestamp_us` is the presentation timestamp of the first sample in the
    /// batch, in microseconds since session start, and MUST be monotonically
    /// non-decreasing within a session.
    FramesArrived {
        /// Session this batch belongs to.
        session_id: SessionId,
        /// Encoded media bytes (codec-specific). Cheaply cloneable.
        batch_bytes: Bytes,
        /// Presentation timestamp of the first sample, in microseconds since session
        /// start.
        timestamp_us: u64,
    },

    /// The session terminated. Always the last event for a given [`SessionId`].
    SessionEnded {
        /// Session that just ended.
        session_id: SessionId,
        /// Why it ended.
        reason: SessionEndReason,
        /// Wall-clock time the session ended, in UTC.
        ended_at: OffsetDateTime,
    },
}

/// Things that can go wrong specifically in a stream-ingest adapter.
///
/// Wraps cleanly into a [`diaspor_core::VfsError::Backend`] when bubbled up — every
/// variant has a string representation through `thiserror`, so adapters can return
/// `diaspor_core::Result<T>` directly via the [`From`] impl below.
#[derive(Debug, Error)]
pub enum StreamIngestError {
    /// The adapter exists as a trait stub but no real implementation has shipped yet.
    /// Carry the transport name so logs make it obvious which milestone is gating the
    /// caller.
    #[error("stream-ingest transport `{transport}` is not implemented yet")]
    NotImplemented {
        /// Short transport name: `"file"`, `"whip"`, `"hls"`, `"meeting-bot"`, …
        transport: &'static str,
    },

    /// The adapter could not be configured from the given inputs.
    #[error("invalid stream-ingest config: {0}")]
    InvalidConfig(String),

    /// The underlying transport (HTTP, WebRTC, file IO) failed.
    #[error("transport failure: {0}")]
    Transport(String),

    /// A consent invariant was violated — see [`meeting_bot`] for the rules. Adapters
    /// MUST surface this rather than emit `FramesArrived` past a declined / withdrawn
    /// consent boundary.
    #[error("consent violation: {0}")]
    ConsentViolation(String),
}

impl From<StreamIngestError> for VfsError {
    fn from(err: StreamIngestError) -> Self {
        Self::Backend(err.to_string())
    }
}

/// A pluggable stream-ingest adapter.
///
/// Implementors are usually constructed once with their configuration, then
/// [`StreamIngest::start`] is called to begin emitting events. Adapters SHOULD be safe
/// to construct multiple times for separate sessions; sharing a single `start()`-ed
/// stream across consumers is not supported here (use `futures::Stream` combinators if
/// fan-out is needed).
#[async_trait]
pub trait StreamIngest: Send + Sync {
    /// Human-readable name of the adapter, for logs and metrics.
    fn name(&self) -> &'static str;

    /// Opens a new ingest session and returns a boxed event stream.
    ///
    /// The returned stream MUST emit exactly one [`IngestEvent::SessionStarted`] first,
    /// zero or more [`IngestEvent::FramesArrived`] in monotonic-timestamp order, and
    /// terminate with exactly one [`IngestEvent::SessionEnded`].
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be opened at all (invalid config, transport
    /// refused). Mid-stream errors surface as `Err` items inside the stream, not by
    /// short-circuiting `start()`.
    async fn start(&self) -> Result<IngestStream>;
}
