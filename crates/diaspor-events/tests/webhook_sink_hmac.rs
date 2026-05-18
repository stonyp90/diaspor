//! Integration test for [`diaspor_events::WebhookEventSink`].
//!
//! Spins up a one-shot HTTP/1.1 listener on `127.0.0.1:0`, lets the sink POST a single
//! event to it, parses the request with [`httparse`], and verifies:
//!
//! 1. The path is `/`.
//! 2. The `Content-Type` is `application/json`.
//! 3. The body equals the event's `payload_bytes` byte-for-byte.
//! 4. The `X-Diaspor-Signature` header is `sha256=<expected-hex>`, where `<expected-hex>`
//!    is the lower-case HMAC-SHA256 of the *body that was actually received* using the
//!    configured secret. This proves the sink's HMAC math agrees with an independent
//!    re-computation against the wire bytes.
//! 5. An RFC 4231 test vector check, performed via [`WebhookEventSink::sign`] directly,
//!    confirms the math is correct against a published reference.

use std::time::Duration;

use bytes::Bytes;
use diaspor_events::{
    Event, EventSink, SessionId, Severity, TenantId, ThresholdEvent, WebhookEventSink,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type HmacSha256 = Hmac<Sha256>;

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{b:02x}").unwrap();
    }
    out
}

fn independent_hmac(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).unwrap();
    mac.update(body);
    hex_lower(&mac.finalize().into_bytes())
}

#[tokio::test]
async fn webhook_sink_posts_payload_with_correct_hmac_signature() {
    // Bind on an ephemeral port. The OS picks a free one for us.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");
    let url = format!("http://{addr}/");

    let secret = "spec-known-shared-secret";
    let body_bytes: &'static [u8] =
        b"{\"detector\":\"lie_score\",\"score\":0.91,\"window\":[1000,2000]}";

    // Server task: accept one connection, slurp the request, send 204, return the bytes.
    let server = tokio::spawn(async move {
        let (mut stream, _peer) = listener.accept().await.expect("accept");
        let mut buf = Vec::with_capacity(4096);
        let mut chunk = [0u8; 1024];

        loop {
            let n = stream.read(&mut chunk).await.expect("read chunk");
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);

            // Try to parse what we have so far. If headers are complete, check if we've
            // also received the full body declared by Content-Length.
            let mut headers = [httparse::EMPTY_HEADER; 32];
            let mut req = httparse::Request::new(&mut headers);
            let status = req.parse(&buf).expect("parse HTTP request");
            if let httparse::Status::Complete(header_len) = status {
                let content_length = req
                    .headers
                    .iter()
                    .find(|h| h.name.eq_ignore_ascii_case("content-length"))
                    .and_then(|h| std::str::from_utf8(h.value).ok())
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                if buf.len() >= header_len + content_length {
                    break;
                }
            }
        }

        // Send a minimal 204 response so reqwest sees a successful close.
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .expect("write response");
        stream.flush().await.ok();
        // Read & discard any further bytes the client may write (unlikely on HTTP/1.1
        // with `Connection: close`) so we close cleanly.
        let _ = stream.shutdown().await;
        buf
    });

    // Client side: build sink and emit.
    let sink = WebhookEventSink::new(&url, secret, Duration::from_secs(5));
    let event = Event::Threshold(ThresholdEvent {
        tenant_id: TenantId::new("tenant-1"),
        stream_id: SessionId::new("stream-1"),
        timestamp_ms: 1_700_000_000_000,
        detector: "lie_score".to_string(),
        severity: Severity::Critical,
        payload_bytes: Bytes::from_static(body_bytes),
    });
    sink.emit(event).await.expect("webhook emit Ok on 204");

    // Drain the server's captured bytes.
    let raw = server.await.expect("server task finished cleanly");

    // Parse the request to lift out headers + body offset.
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut req = httparse::Request::new(&mut headers);
    let status = req.parse(&raw).expect("parse");
    let header_len = match status {
        httparse::Status::Complete(n) => n,
        httparse::Status::Partial => panic!("server captured an incomplete request"),
    };

    assert_eq!(req.method, Some("POST"), "method must be POST");
    assert_eq!(req.path, Some("/"), "path must be `/`");

    // Lift out the headers we care about.
    let mut content_type = None;
    let mut content_length = None;
    let mut signature = None;
    for h in req.headers.iter() {
        if h.name.eq_ignore_ascii_case("content-type") {
            content_type = std::str::from_utf8(h.value).ok().map(str::to_owned);
        } else if h.name.eq_ignore_ascii_case("content-length") {
            content_length = std::str::from_utf8(h.value)
                .ok()
                .and_then(|s| s.parse::<usize>().ok());
        } else if h.name.eq_ignore_ascii_case("x-diaspor-signature") {
            signature = std::str::from_utf8(h.value).ok().map(str::to_owned);
        }
    }

    assert_eq!(
        content_type.as_deref(),
        Some("application/json"),
        "Content-Type must be application/json"
    );

    let received_body = &raw[header_len..header_len + content_length.expect("Content-Length")];
    assert_eq!(
        received_body, body_bytes,
        "body bytes must round-trip verbatim"
    );

    let sig = signature.expect("X-Diaspor-Signature header must be present");
    let expected_hex = independent_hmac(secret.as_bytes(), received_body);
    assert_eq!(
        sig,
        format!("sha256={expected_hex}"),
        "X-Diaspor-Signature must equal sha256=<HMAC-SHA256(body)> independently computed",
    );
}

#[test]
fn webhook_sign_matches_rfc4231_test_vector_1() {
    // RFC 4231 §4.2 — Test Case 1
    //   key  = 0x0b * 20
    //   data = "Hi There"
    //   sha256 mac = b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7
    let key = [0x0bu8; 20];
    let data = b"Hi There";
    let expected = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";
    assert_eq!(WebhookEventSink::sign(&key, data), expected);
}
