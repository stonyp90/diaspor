//! Integration test for [`diaspor_events::MultiSink`] with a mix of real + stub sinks.
//!
//! Verifies that:
//!
//! 1. fan-out routes the same event to every underlying sink concurrently;
//! 2. when the VFS sink succeeds and the WebSocket sink also succeeds (no subscribers is
//!    not an error), the overall `MultiSink::emit` returns `Ok(())`;
//! 3. when one sink fails, the rest still run to completion before the error is surfaced.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use bytes::Bytes;
use diaspor_backend_memory::MemoryBackend;
use diaspor_core::{OpenFlags, VfsBackend, VfsPath};
use diaspor_events::{
    Event, EventError, EventSink, MultiSink, SessionId, Severity, TenantId, ThresholdEvent,
    VfsEventSink, WebSocketEventSink,
};

/// A test-only sink that counts every call to `emit`. Lives in this test module rather
/// than the crate's public API because nobody outside tests has a reason to compose one.
struct CountingSink {
    counter: Arc<AtomicUsize>,
}

#[async_trait]
impl EventSink for CountingSink {
    fn name(&self) -> &'static str {
        "counting"
    }

    async fn emit(&self, _event: Event) -> Result<(), EventError> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// A test-only sink that always fails. Used to verify that a failing sink does not
/// prevent earlier sinks (declared first in the `MultiSink`) from running to completion.
struct FailingSink;

#[async_trait]
impl EventSink for FailingSink {
    fn name(&self) -> &'static str {
        "failing"
    }

    async fn emit(&self, _event: Event) -> Result<(), EventError> {
        Err(EventError::Rejected {
            sink: "failing",
            reason: "always fails by design".to_string(),
        })
    }
}

fn sample_event() -> Event {
    Event::Threshold(ThresholdEvent {
        tenant_id: TenantId::new("tenant-1"),
        stream_id: SessionId::new("session-1"),
        timestamp_ms: 1_700_000_000_000,
        detector: "tremor_onset".to_string(),
        severity: Severity::Warn,
        payload_bytes: Bytes::from_static(b"{\"score\":0.92}"),
    })
}

#[tokio::test]
async fn multi_sink_fans_out_to_real_vfs_and_real_websocket() {
    let backend: Arc<dyn VfsBackend> = Arc::new(MemoryBackend::new());
    let vfs_sink = VfsEventSink::new(Arc::clone(&backend), "/");
    let ws_sink = WebSocketEventSink::new();

    // Subscribe before emitting so the WS branch has a receiver.
    let mut rx = ws_sink.subscribe();

    let multi = MultiSink::new(vec![Box::new(vfs_sink), Box::new(ws_sink)]);
    assert_eq!(multi.len(), 2);

    multi
        .emit(sample_event())
        .await
        .expect("both real sinks succeed");

    // Verify the VFS branch wrote the sidecar.
    let path = VfsPath::new("/.streams/session-1/events/1700000000000.event.json").unwrap();
    let mut handle = backend
        .open(&path, OpenFlags::READ)
        .await
        .expect("VFS sink should have created the sidecar file");
    let body = handle.read(0, 4096).await.unwrap();
    assert_eq!(&body[..], b"{\"score\":0.92}");

    // Verify the WebSocket branch broadcast the payload.
    let received = rx
        .recv()
        .await
        .expect("WS sink should have broadcast the payload");
    assert_eq!(received, b"{\"score\":0.92}");
}

#[tokio::test]
async fn multi_sink_runs_every_branch_even_when_one_fails() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counting = CountingSink {
        counter: Arc::clone(&counter),
    };
    let failing = FailingSink;
    let counting_after = CountingSink {
        counter: Arc::clone(&counter),
    };

    let multi = MultiSink::new(vec![
        Box::new(counting),
        Box::new(failing),
        Box::new(counting_after),
    ]);

    let err = multi
        .emit(sample_event())
        .await
        .expect_err("the failing branch must surface an error");

    match err {
        EventError::Rejected { sink, .. } => {
            assert_eq!(sink, "failing");
        }
        other => panic!("expected Rejected from failing sink, got {other:?}"),
    }

    // The counter must reach 2 — both `CountingSink`s ran to completion, even though
    // the failing sink between / after them returned an error.
    assert_eq!(
        counter.load(Ordering::SeqCst),
        2,
        "fan-out must not short-circuit on the first error"
    );
}
