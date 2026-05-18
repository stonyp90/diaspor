//! # diaspor-events
//!
//! Event fan-out layer for `diaspor` inference outputs.
//!
//! The inference pipeline (`diaspor-infer`) emits two kinds of structured events as it
//! consumes a media stream:
//!
//! 1. **Per-second window aggregates** — a [`WindowEvent`] carries the rolling score for
//!    a one-second window of the stream, JSON-encoded by the upstream producer.
//! 2. **Threshold crossings** — a [`ThresholdEvent`] carries a detector-specific signal
//!    (e.g. tremor onset, lie-detection score over `0.8`) at the instant it fires.
//!
//! This crate routes those events to one or more delivery channels — a [`VfsEventSink`]
//! that drops sidecar JSON into the backing filesystem, a [`WebSocketEventSink`] that
//! broadcasts to a subscribed client pool, and a [`WebhookEventSink`] that POSTs to a
//! configured URL with an HMAC-SHA256 signature.
//!
//! ## Fan-out at a glance
//!
//! ```text
//!   ┌──────────────────────┐
//!   │ diaspor-infer        │
//!   │  window/threshold    │
//!   └──────────┬───────────┘
//!              │
//!              ▼
//!   ┌──────────────────────┐    ┌──────────────────────┐
//!   │   MultiSink::emit    │───▶│ VfsEventSink         │
//!   │   (fan-out)          │    │  /.streams/<id>/...  │
//!   └──────────┬───────────┘    └──────────────────────┘
//!              │
//!              ├──────────────▶ ┌──────────────────────┐
//!              │                │ WebSocketEventSink   │
//!              │                │  broadcast pool      │
//!              │                └──────────────────────┘
//!              │
//!              └──────────────▶ ┌──────────────────────┐
//!                               │ WebhookEventSink     │
//!                               │  POST + HMAC sig     │
//!                               └──────────────────────┘
//! ```
//!
//! ## Privacy contract
//!
//! - **VFS sink is local-only** — windows and threshold events land inside the same
//!   [`diaspor_core::VfsBackend`] that the rest of the system already trusts.
//! - **WebSocket and webhook sinks are opt-in** — callers wire them up explicitly with
//!   destination credentials; nothing in this crate dials home.
//! - **Payload bytes are opaque.** This crate does not parse, mutate, or log the
//!   per-event `payload_bytes` — it only routes them.
//!
//! ## Status
//!
//! v0.1.0-alpha ships real `VfsEventSink`, `WebhookEventSink` (HMAC-SHA256 signed), and
//! `WebSocketEventSink` (`broadcast::Sender` backed) implementations. Schema sidecar
//! wiring to upstream `diaspor-vision` is M7.

#![doc(html_root_url = "https://docs.rs/diaspor-events/0.1.0-alpha.1")]

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod event;
pub mod sink;

pub use event::{Event, Severity, ThresholdEvent, WindowEvent};
pub use sink::{EventSink, MultiSink, VfsEventSink, WebSocketEventSink, WebhookEventSink};

// TODO(consolidation): unify with diaspor-infer::TenantId in diaspor-core
/// Identifies the tenant that owns the inference stream emitting this event.
///
/// Defined locally for now so this crate can compile independently of `diaspor-infer`.
/// Both newtypes wrap the same `String` shape and will be lifted into `diaspor-core`
/// once the cross-crate contract is settled.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TenantId(pub String);

impl TenantId {
    /// Constructs a new [`TenantId`] from anything string-like.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrows the underlying tenant identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// TODO(consolidation): unify with diaspor-infer::SessionId in diaspor-core
/// Identifies the inference session / live stream the event belongs to.
///
/// Stable for the lifetime of a single stream; sidecar files in the VFS are keyed by
/// this identifier under `/.streams/<stream-id>/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

impl SessionId {
    /// Constructs a new [`SessionId`] from anything string-like.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrows the underlying session identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Things that can go wrong while fanning an event out to its sinks.
///
/// Wraps cleanly into a [`diaspor_core::VfsError::Backend`] when bubbled up.
#[derive(Debug, Error)]
pub enum EventError {
    /// The sink is declared but its delivery path has not yet been implemented.
    ///
    /// Returned by every stub sink in v0.1.0-alpha. The `sink` field identifies which
    /// implementation deferred the work, so test assertions and operator logs can tell
    /// them apart.
    #[error("event sink {sink} is not yet implemented")]
    NotImplemented {
        /// Static name of the sink that returned this error (`"vfs"`, `"websocket"`,
        /// `"webhook"`).
        sink: &'static str,
    },

    /// The destination rejected the event (HTTP non-2xx, WebSocket close frame, etc.).
    #[error("sink {sink} rejected event: {reason}")]
    Rejected {
        /// Static name of the sink that rejected the event.
        sink: &'static str,
        /// Human-readable reason returned by the destination.
        reason: String,
    },

    /// Delivery timed out before the destination acknowledged the event.
    #[error("sink {sink} timed out after {millis} ms")]
    Timeout {
        /// Static name of the sink that timed out.
        sink: &'static str,
        /// Elapsed time before the timeout fired, in milliseconds.
        millis: u64,
    },

    /// The HMAC secret or signing routine was misconfigured.
    #[error("sink {sink} signing failure: {reason}")]
    SigningFailed {
        /// Static name of the sink whose signing pipeline failed.
        sink: &'static str,
        /// What went wrong while signing.
        reason: String,
    },

    /// Wraps a `diaspor_core::VfsError` from the underlying [`diaspor_core::VfsBackend`].
    #[error("vfs error: {0}")]
    Vfs(#[from] diaspor_core::VfsError),
}
