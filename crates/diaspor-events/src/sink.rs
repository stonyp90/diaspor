//! Sinks — the delivery channels events fan out to.
//!
//! Three concrete sinks ship in this crate:
//!
//! - [`VfsEventSink`] — writes sidecar JSON into a [`diaspor_core::VfsBackend`].
//! - [`WebSocketEventSink`] — broadcasts to a subscribed client pool.
//! - [`WebhookEventSink`] — POSTs with an HMAC-SHA256 signature in the
//!   `X-Diaspor-Signature` header.
//!
//! All three are real, working implementations. The composing [`MultiSink`] fans events
//! out to every sink concurrently and surfaces the first failure (if any) after the rest
//! have run to completion.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use diaspor_core::{OpenFlags, VfsBackend, VfsPath};
use futures::future::join_all;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::sync::broadcast;

use crate::event::Event;
use crate::EventError;

type HmacSha256 = Hmac<Sha256>;

/// Writes events as sidecar JSON files inside a backing [`VfsBackend`].
///
/// Path convention (see ADR 0007):
///
/// - [`crate::Event::Window`] → `<root>/.streams/<stream-id>/windows/<t_start_ms>.score.json`
/// - [`crate::Event::Threshold`] → `<root>/.streams/<stream-id>/events/<timestamp_ms>.event.json`
///
/// The file body is the event's `payload_bytes` verbatim — the upstream producer
/// (`diaspor-infer`) already JSON-encoded the per-window aggregate, so this sink does not
/// re-serialize. The wrapping `Event` envelope is *not* written; the sidecar tree mirrors
/// the schema in `docs/schema/score-v1.json`.
///
/// The sink will lazily `create_dir` the `/.streams`, `/.streams/<id>`, and the
/// `windows`/`events` subdirectory whenever they're missing. Existing directories are
/// tolerated. Existing files are truncated and overwritten — events for the same window /
/// timestamp are deduplicated by replacement.
pub struct VfsEventSink {
    /// Backing VFS the sidecar JSON is written into.
    pub vfs: Arc<dyn VfsBackend>,
    /// Root path beneath which `.streams/...` sidecar trees live.
    pub root_path: String,
}

impl VfsEventSink {
    /// Constructs a new VFS event sink.
    #[must_use]
    pub fn new(vfs: Arc<dyn VfsBackend>, root_path: impl Into<String>) -> Self {
        Self {
            vfs,
            root_path: root_path.into(),
        }
    }

    /// Ensures that `path` exists as a directory, creating it (but not its parents) if
    /// necessary. An `AlreadyExists` error from the backend is treated as success — the
    /// goal is idempotence, not exclusive creation.
    async fn ensure_dir(&self, path: &VfsPath) -> Result<(), EventError> {
        match self.vfs.create_dir(path).await {
            // `Ok(())` and `AlreadyExists` are both "the directory now exists" — we
            // collapse them into a single success branch so the caller sees idempotent
            // semantics regardless of pre-existing state.
            Ok(()) | Err(diaspor_core::VfsError::AlreadyExists { .. }) => Ok(()),
            Err(other) => Err(EventError::Vfs(other)),
        }
    }

    /// Builds the `VfsPath` for a sidecar file and ensures every intermediate directory
    /// exists. Returns the leaf path that the caller should `open(CREATE | WRITE | TRUNC)`.
    async fn prepare_path(
        &self,
        stream_id: &str,
        kind_dir: &str,
        filename: &str,
    ) -> Result<VfsPath, EventError> {
        let root = VfsPath::new(&self.root_path).ok_or_else(|| {
            EventError::Vfs(diaspor_core::VfsError::invalid_path(self.root_path.clone()))
        })?;

        let streams_root = root.join(".streams");
        self.ensure_dir(&streams_root).await?;

        let session_root = streams_root.join(stream_id);
        self.ensure_dir(&session_root).await?;

        let kind_path = session_root.join(kind_dir);
        self.ensure_dir(&kind_path).await?;

        Ok(kind_path.join(filename))
    }
}

#[async_trait]
impl EventSink for VfsEventSink {
    fn name(&self) -> &'static str {
        "vfs"
    }

    async fn emit(&self, event: Event) -> Result<(), EventError> {
        let (path, body) = match event {
            Event::Window(w) => {
                let filename = format!("{}.score.json", w.t_start_ms);
                let path = self
                    .prepare_path(w.stream_id.as_str(), "windows", &filename)
                    .await?;
                (path, w.payload_bytes)
            }
            Event::Threshold(t) => {
                let filename = format!("{}.event.json", t.timestamp_ms);
                let path = self
                    .prepare_path(t.stream_id.as_str(), "events", &filename)
                    .await?;
                (path, t.payload_bytes)
            }
        };

        let mut handle = self
            .vfs
            .open(
                &path,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNC,
            )
            .await?;
        handle.write(0, &body).await?;
        handle.flush().await?;
        Ok(())
    }
}

/// A delivery channel for [`Event`]s.
///
/// Implementations are expected to be cheap to clone (typically `Arc`-wrapped) so the
/// same sink can be shared across many [`MultiSink`] compositions and async tasks.
#[async_trait]
pub trait EventSink: Send + Sync {
    /// Human-readable name of the sink, for logs and error attribution.
    fn name(&self) -> &'static str;

    /// Routes an event to its destination.
    ///
    /// # Errors
    ///
    /// Each sink returns its own [`EventError`] variants; see the per-sink docs.
    async fn emit(&self, event: Event) -> Result<(), EventError>;
}

/// Broadcasts events to a pool of subscribed clients via a [`tokio::sync::broadcast`]
/// channel.
///
/// Subscribers receive the raw `payload_bytes` of every emitted event in the order they
/// were emitted. `emit` is non-blocking: if no receivers are currently subscribed it
/// returns `Ok(())` (a "no listeners" condition is normal, not an error). Slow consumers
/// that lag behind the channel capacity (1024 events) will see
/// [`broadcast::error::RecvError::Lagged`] on their next `recv`.
pub struct WebSocketEventSink {
    /// Inner broadcaster. `Vec<u8>` is the serialized event payload; we ship the
    /// upstream's JSON bytes directly so consumers can parse against
    /// `docs/schema/score-v1.json` without a second hop.
    sender: broadcast::Sender<Vec<u8>>,
}

impl WebSocketEventSink {
    /// Constructs a new WebSocket sink backed by a broadcast channel of capacity 1024.
    #[must_use]
    pub fn new() -> Self {
        // `broadcast::channel` returns a paired sender + receiver; we keep only the
        // sender and immediately drop the receiver. Subsequent subscribers register
        // via [`Self::subscribe`].
        let (sender, _) = broadcast::channel(1024);
        Self { sender }
    }

    /// Registers a new subscriber and returns its [`broadcast::Receiver`].
    ///
    /// The receiver will see every event emitted *after* the call to `subscribe` — events
    /// emitted before this point are not replayed.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.sender.subscribe()
    }

    /// Returns the current subscriber count. Useful in tests and operator dashboards.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for WebSocketEventSink {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventSink for WebSocketEventSink {
    fn name(&self) -> &'static str {
        "websocket"
    }

    async fn emit(&self, event: Event) -> Result<(), EventError> {
        let payload = match event {
            Event::Window(w) => w.payload_bytes,
            Event::Threshold(t) => t.payload_bytes,
        };
        // broadcast::send returns Err when there are zero receivers. That's not an error
        // for us — a sink with no live subscribers is a valid steady state.
        let _ = self.sender.send(payload.to_vec());
        Ok(())
    }
}

/// POSTs events to a configured HTTP endpoint with an HMAC-SHA256 signature.
///
/// The signature is computed over the request body using `hmac_secret` and transmitted in
/// the `X-Diaspor-Signature` header as `sha256=<hex>`. Receivers verify the signature to
/// confirm the event originated from this diaspor instance.
///
/// The body is the event's `payload_bytes` verbatim — the upstream producer
/// (`diaspor-infer`) already JSON-encoded the per-window aggregate, so this sink does not
/// re-serialize. The `Content-Type` is set to `application/json`.
pub struct WebhookEventSink {
    /// Destination URL events are `POST`ed to.
    pub url: String,
    /// Shared secret used to compute the HMAC-SHA256 signature.
    pub hmac_secret: String,
    /// Per-request timeout. Delivery attempts that exceed this return
    /// [`EventError::Timeout`].
    pub timeout: Duration,
    /// Lazily-cached `reqwest` client. Constructed once with the configured timeout so
    /// every request shares the underlying connection pool.
    client: reqwest::Client,
}

impl WebhookEventSink {
    /// Constructs a new webhook sink.
    ///
    /// The `reqwest` client is built once with the configured timeout. If the client
    /// fails to build (e.g. invalid TLS configuration on the host), the call panics —
    /// this is intentional: a misconfigured sink is a configuration bug, not a runtime
    /// condition, and we want to surface it at startup rather than per-request.
    #[must_use]
    pub fn new(
        url: impl Into<String>,
        hmac_secret: impl Into<String>,
        timeout: Duration,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("reqwest client builder cannot fail with default config");
        Self {
            url: url.into(),
            hmac_secret: hmac_secret.into(),
            timeout,
            client,
        }
    }

    /// Computes the lowercase-hex HMAC-SHA256 of `body` using `secret` as the key.
    ///
    /// Exposed so consumers (and the integration test) can verify the math without
    /// duplicating the construction.
    #[must_use]
    pub fn sign(secret: &[u8], body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret)
            .expect("HMAC-SHA256 accepts keys of any length");
        mac.update(body);
        let result = mac.finalize().into_bytes();
        let mut hex = String::with_capacity(result.len() * 2);
        for b in result {
            use std::fmt::Write as _;
            write!(&mut hex, "{b:02x}").expect("writing to a String never fails");
        }
        hex
    }
}

#[async_trait]
impl EventSink for WebhookEventSink {
    fn name(&self) -> &'static str {
        "webhook"
    }

    async fn emit(&self, event: Event) -> Result<(), EventError> {
        let body = match event {
            Event::Window(w) => w.payload_bytes,
            Event::Threshold(t) => t.payload_bytes,
        };

        let signature = format!(
            "sha256={}",
            Self::sign(self.hmac_secret.as_bytes(), &body)
        );

        let response = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("X-Diaspor-Signature", &signature)
            .body(body.to_vec())
            .send()
            .await
            .map_err(|err| {
                if err.is_timeout() {
                    EventError::Timeout {
                        sink: "webhook",
                        millis: u64::try_from(self.timeout.as_millis()).unwrap_or(u64::MAX),
                    }
                } else {
                    EventError::Rejected {
                        sink: "webhook",
                        reason: err.to_string(),
                    }
                }
            })?;

        if !response.status().is_success() {
            return Err(EventError::Rejected {
                sink: "webhook",
                reason: format!("HTTP {}", response.status()),
            });
        }

        Ok(())
    }
}

/// Fans an event out to multiple downstream sinks concurrently.
///
/// `MultiSink::emit` drives every underlying [`EventSink::emit`] future in parallel via
/// [`join_all`] and clones the event once per sink. The composed call returns:
///
/// - `Ok(())` if **every** sink succeeded.
/// - The **first** [`EventError`] (in declaration order) otherwise. Successful sinks
///   still ran to completion before the error is surfaced — fan-out is not short-circuited.
///
/// This is the integration point that `diaspor-infer` calls once per emitted event.
pub struct MultiSink {
    sinks: Vec<Box<dyn EventSink>>,
}

impl MultiSink {
    /// Constructs a new [`MultiSink`] over a list of owned sinks.
    #[must_use]
    pub fn new(sinks: Vec<Box<dyn EventSink>>) -> Self {
        Self { sinks }
    }

    /// Number of underlying sinks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sinks.len()
    }

    /// Returns `true` if there are no underlying sinks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sinks.is_empty()
    }
}

#[async_trait]
impl EventSink for MultiSink {
    fn name(&self) -> &'static str {
        "multi"
    }

    async fn emit(&self, event: Event) -> Result<(), EventError> {
        let futures = self
            .sinks
            .iter()
            .map(|sink| {
                let event = event.clone();
                async move { sink.emit(event).await }
            })
            .collect::<Vec<_>>();

        // Drive every sink to completion first, then surface the first error (if any)
        // in declaration order. Fan-out is not short-circuited — even if the webhook
        // sink fails, the VFS sink still has a chance to persist the event locally.
        let results = join_all(futures).await;

        for result in results {
            result?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::WebhookEventSink;

    // Known-vector check from RFC 4231 (HMAC-SHA256 test case 1):
    //   key  = 0x0b * 20
    //   data = "Hi There"
    //   mac  = b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7
    #[test]
    fn hmac_matches_rfc4231_test_vector_1() {
        let key = [0x0bu8; 20];
        let data = b"Hi There";
        let expected = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
        assert_eq!(WebhookEventSink::sign(&key, data), expected);
    }

    #[test]
    fn hmac_signs_bytes_directly() {
        let body = Bytes::from_static(b"{\"score\":0.92}");
        let sig = WebhookEventSink::sign(b"shared-secret", &body);
        // 64-char hex output regardless of key / body shape.
        assert_eq!(sig.len(), 64);
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
