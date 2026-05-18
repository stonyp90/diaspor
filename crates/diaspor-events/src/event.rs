//! The event types fanned out to every [`crate::EventSink`].

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{SessionId, TenantId};

/// Severity of a [`ThresholdEvent`].
///
/// Sinks may use this to filter (e.g. a webhook subscriber that only wants `Critical`)
/// or to drive operator-facing UI styling (color, sound). Ordering is meaningful:
/// `Info < Warn < Critical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational — useful context, not actionable on its own.
    Info,
    /// Worth a notification, but not page-worthy.
    Warn,
    /// Operator-actionable signal; sinks should treat this as high priority.
    Critical,
}

/// Per-second aggregate score for a window of a live inference stream.
///
/// The `payload_bytes` is opaque to this crate — the inference pipeline encodes the
/// per-second aggregate (typically as JSON) and this crate's only job is to route those
/// bytes to every configured sink without inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowEvent {
    /// Tenant that owns the originating stream.
    pub tenant_id: TenantId,
    /// Inference session / live stream identifier.
    pub stream_id: SessionId,
    /// Window start, in milliseconds since the stream's epoch.
    pub t_start_ms: u64,
    /// Window end, in milliseconds since the stream's epoch.
    pub t_end_ms: u64,
    /// Opaque payload (JSON-encoded by the producer).
    pub payload_bytes: Bytes,
}

/// A detector-specific threshold crossing, fired at the instant it occurs.
///
/// Carries the detector's name (e.g. `"tremor_onset"`, `"lie_score"`), a severity, and
/// the same opaque `payload_bytes` contract as [`WindowEvent`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdEvent {
    /// Tenant that owns the originating stream.
    pub tenant_id: TenantId,
    /// Inference session / live stream identifier.
    pub stream_id: SessionId,
    /// When the threshold crossed, in milliseconds since the stream's epoch.
    pub timestamp_ms: u64,
    /// Identifier of the detector that produced the event (e.g. `"tremor_onset"`).
    pub detector: String,
    /// Severity classification for routing and operator UI.
    pub severity: Severity,
    /// Opaque payload (JSON-encoded by the producer).
    pub payload_bytes: Bytes,
}

/// Union of every event variant routed by this crate.
///
/// Sinks pattern-match on this enum to choose where to write the payload (e.g. the
/// [`crate::VfsEventSink`] drops [`Event::Window`] into `/.streams/<id>/windows/` and
/// [`Event::Threshold`] into `/.streams/<id>/events/`).
///
/// The serde representation uses an internally tagged enum (`"kind": "window" | "threshold"`)
/// so on-wire and on-disk forms are self-describing without losing the inner payload shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// A per-second window aggregate score.
    Window(WindowEvent),
    /// A detector-specific threshold crossing.
    Threshold(ThresholdEvent),
}

impl Event {
    /// Returns the tenant that owns the originating stream, regardless of variant.
    #[must_use]
    pub const fn tenant_id(&self) -> &TenantId {
        match self {
            Self::Window(e) => &e.tenant_id,
            Self::Threshold(e) => &e.tenant_id,
        }
    }

    /// Returns the inference session / stream identifier, regardless of variant.
    #[must_use]
    pub const fn stream_id(&self) -> &SessionId {
        match self {
            Self::Window(e) => &e.stream_id,
            Self::Threshold(e) => &e.stream_id,
        }
    }
}
