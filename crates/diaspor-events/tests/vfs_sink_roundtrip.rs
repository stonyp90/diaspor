//! Integration test for [`diaspor_events::VfsEventSink`].
//!
//! Wires the real sink against a [`MemoryBackend`] and verifies that
//!
//! 1. a `WindowEvent` lands at `/.streams/<id>/windows/<t_start_ms>.score.json`
//! 2. a `ThresholdEvent` lands at `/.streams/<id>/events/<timestamp_ms>.event.json`
//! 3. the file bodies equal the original `payload_bytes` byte-for-byte (the sink does not
//!    re-serialize the upstream's JSON)
//! 4. the intermediate `/.streams`, `/.streams/<id>`, `windows`, and `events` directories
//!    are created lazily — the caller does not need to pre-create them.

use std::sync::Arc;

use bytes::Bytes;
use diaspor_backend_memory::MemoryBackend;
use diaspor_core::{OpenFlags, VfsBackend, VfsPath};
use diaspor_events::{
    Event, EventSink, SessionId, Severity, TenantId, ThresholdEvent, VfsEventSink, WindowEvent,
};

#[tokio::test]
async fn vfs_sink_writes_window_event_at_expected_path() {
    let backend: Arc<dyn VfsBackend> = Arc::new(MemoryBackend::new());
    let sink = VfsEventSink::new(Arc::clone(&backend), "/");

    let payload = b"{\"window\":{\"t_start_ms\":1000,\"t_end_ms\":2000},\"score\":0.42}";
    let event = Event::Window(WindowEvent {
        tenant_id: TenantId::new("tenant-a"),
        stream_id: SessionId::new("stream-7"),
        t_start_ms: 1000,
        t_end_ms: 2000,
        payload_bytes: Bytes::from_static(payload),
    });

    sink.emit(event).await.expect("emit succeeds");

    // Read it back through the same backend.
    let expected_path =
        VfsPath::new("/.streams/stream-7/windows/1000.score.json").expect("valid path");
    let mut handle = backend
        .open(&expected_path, OpenFlags::READ)
        .await
        .expect("file exists at expected path");
    let body = handle.read(0, 4096).await.expect("read succeeds");
    assert_eq!(
        &body[..],
        payload,
        "file body must be the payload_bytes verbatim"
    );
}

#[tokio::test]
async fn vfs_sink_writes_threshold_event_at_expected_path() {
    let backend: Arc<dyn VfsBackend> = Arc::new(MemoryBackend::new());
    let sink = VfsEventSink::new(Arc::clone(&backend), "/");

    let payload = b"{\"detector\":\"lie_score\",\"score\":0.91}";
    let event = Event::Threshold(ThresholdEvent {
        tenant_id: TenantId::new("tenant-a"),
        stream_id: SessionId::new("stream-7"),
        timestamp_ms: 1_700_000_000_000,
        detector: "lie_score".to_string(),
        severity: Severity::Critical,
        payload_bytes: Bytes::from_static(payload),
    });

    sink.emit(event).await.expect("emit succeeds");

    let expected_path =
        VfsPath::new("/.streams/stream-7/events/1700000000000.event.json").expect("valid path");
    let mut handle = backend
        .open(&expected_path, OpenFlags::READ)
        .await
        .expect("file exists at expected path");
    let body = handle.read(0, 4096).await.expect("read succeeds");
    assert_eq!(&body[..], payload, "threshold body must round-trip exactly");
}

#[tokio::test]
async fn vfs_sink_creates_intermediate_directories_lazily() {
    let backend: Arc<dyn VfsBackend> = Arc::new(MemoryBackend::new());
    let sink = VfsEventSink::new(Arc::clone(&backend), "/");

    let event = Event::Window(WindowEvent {
        tenant_id: TenantId::new("tenant-z"),
        stream_id: SessionId::new("brand-new-stream"),
        t_start_ms: 0,
        t_end_ms: 1000,
        payload_bytes: Bytes::from_static(b"{}"),
    });

    // Before emit: none of the .streams/... dirs exist.
    assert!(
        backend
            .metadata(&VfsPath::new("/.streams").unwrap())
            .await
            .is_err(),
        "/.streams should not exist before emit"
    );

    sink.emit(event).await.expect("emit succeeds");

    // After emit: every intermediate directory exists.
    for dir in [
        "/.streams",
        "/.streams/brand-new-stream",
        "/.streams/brand-new-stream/windows",
    ] {
        let path = VfsPath::new(dir).unwrap();
        let meta = backend
            .metadata(&path)
            .await
            .unwrap_or_else(|_| panic!("expected {dir} to exist after emit"));
        assert_eq!(meta.kind, diaspor_core::NodeKind::Directory);
    }
}

#[tokio::test]
async fn vfs_sink_overwrites_existing_event_for_same_window() {
    let backend: Arc<dyn VfsBackend> = Arc::new(MemoryBackend::new());
    let sink = VfsEventSink::new(Arc::clone(&backend), "/");

    let make = |payload: &'static [u8]| {
        Event::Window(WindowEvent {
            tenant_id: TenantId::new("t"),
            stream_id: SessionId::new("s"),
            t_start_ms: 42,
            t_end_ms: 1042,
            payload_bytes: Bytes::from_static(payload),
        })
    };

    sink.emit(make(b"{\"v\":1}")).await.unwrap();
    sink.emit(make(b"{\"v\":2}")).await.unwrap();

    let path = VfsPath::new("/.streams/s/windows/42.score.json").unwrap();
    let mut handle = backend.open(&path, OpenFlags::READ).await.unwrap();
    let body = handle.read(0, 4096).await.unwrap();
    assert_eq!(
        &body[..],
        b"{\"v\":2}",
        "second emit should overwrite (TRUNC) the first"
    );
}
