//! Integration test for [`diaspor_events::WebSocketEventSink`].
//!
//! Two independent subscribers register *before* anything is emitted, then two events are
//! emitted in order. Both receivers must observe both payloads, in the same order, with
//! the original `payload_bytes` byte-for-byte intact.

use bytes::Bytes;
use diaspor_events::{
    Event, EventSink, SessionId, Severity, TenantId, ThresholdEvent, WebSocketEventSink,
    WindowEvent,
};

#[tokio::test]
async fn websocket_sink_broadcasts_to_every_subscriber_in_order() {
    let sink = WebSocketEventSink::new();
    assert_eq!(sink.subscriber_count(), 0);

    let mut rx_a = sink.subscribe();
    let mut rx_b = sink.subscribe();
    assert_eq!(sink.subscriber_count(), 2);

    let window_payload = b"{\"score\":0.42}";
    let threshold_payload = b"{\"score\":0.91,\"severity\":\"critical\"}";

    sink.emit(Event::Window(WindowEvent {
        tenant_id: TenantId::new("t"),
        stream_id: SessionId::new("s"),
        t_start_ms: 0,
        t_end_ms: 1000,
        payload_bytes: Bytes::from_static(window_payload),
    }))
    .await
    .expect("emit Ok with subscribers attached");

    sink.emit(Event::Threshold(ThresholdEvent {
        tenant_id: TenantId::new("t"),
        stream_id: SessionId::new("s"),
        timestamp_ms: 500,
        detector: "lie_score".to_string(),
        severity: Severity::Critical,
        payload_bytes: Bytes::from_static(threshold_payload),
    }))
    .await
    .expect("emit Ok with subscribers attached");

    // Receiver A sees both messages in order.
    let a1 = rx_a.recv().await.expect("first msg for A");
    let a2 = rx_a.recv().await.expect("second msg for A");
    assert_eq!(a1, window_payload);
    assert_eq!(a2, threshold_payload);

    // Receiver B sees both messages in order, independently.
    let b1 = rx_b.recv().await.expect("first msg for B");
    let b2 = rx_b.recv().await.expect("second msg for B");
    assert_eq!(b1, window_payload);
    assert_eq!(b2, threshold_payload);
}

#[tokio::test]
async fn websocket_sink_emit_with_zero_subscribers_is_ok() {
    let sink = WebSocketEventSink::new();
    let event = Event::Window(WindowEvent {
        tenant_id: TenantId::new("t"),
        stream_id: SessionId::new("s"),
        t_start_ms: 0,
        t_end_ms: 1000,
        payload_bytes: Bytes::from_static(b"{}"),
    });
    // Emitting before any subscriber registers must not be an error — the channel is
    // empty by design.
    sink.emit(event).await.expect("emit Ok with no subscribers");
}
