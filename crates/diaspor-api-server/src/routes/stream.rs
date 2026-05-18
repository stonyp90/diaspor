//! `GET /v1/stream` — WebSocket upgrade for live ingest.
//!
//! Two ingest modes share the same upgrade endpoint, differentiated by
//! the query string:
//!
//! - `?ingest=whip&token=...` — direct WHIP push from a client that
//!   already produces WebRTC media (browsers, OBS, hardware encoders).
//! - `?bot=meeting&platform=zoom&meeting_url=...` — Diaspor-managed
//!   meeting-bot wrapper that joins the meeting and pipes A/V into the
//!   pipeline (Zoom / Meet / Teams).
//!
//! In production the upgraded socket carries `diaspor_events::Event`s
//! back to the client as JSON frames. At
//! v0.1.0-alpha.1 the handler authenticates the caller, accepts the
//! upgrade, sends a single
//! `{"type":"error","code":"not_implemented","message":"M8 deliverable"}`
//! frame, then closes — so SDK authors can verify the protocol shape.

use axum::extract::Query;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::auth::ApiKey;

/// Query parameters accepted by `GET /v1/stream`.
///
/// Either the WHIP fields (`ingest=whip`, `token`) or the meeting-bot
/// fields (`bot=meeting`, `platform`, `meeting_url`) are populated
/// depending on the chosen ingest mode. Production validates that
/// exactly one mode is requested.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamQuery {
    /// `"whip"` for direct WHIP push, absent for meeting-bot mode.
    pub ingest: Option<String>,
    /// Short-lived WHIP credential bound to the API key.
    pub token: Option<String>,
    /// `"meeting"` to request the meeting-bot wrapper, absent for WHIP.
    pub bot: Option<String>,
    /// `"zoom"` / `"meet"` / `"teams"` when `bot=meeting`.
    pub platform: Option<String>,
    /// Meeting join URL when `bot=meeting`.
    pub meeting_url: Option<String>,
}

/// `GET /v1/stream` — upgrade to a WebSocket connection.
///
/// In production: parses the query, validates the mode, instantiates
/// the matching `diaspor-stream-ingest` ingester, wires its event
/// stream into the upgraded socket.
///
/// At v0.1.0-alpha.1: authenticates, accepts the upgrade, immediately
/// tells the client this is an M8 deliverable, so the protocol shape is
/// observable end-to-end.
pub async fn stream_upgrade(
    _key: ApiKey,
    ws: WebSocketUpgrade,
    Query(_q): Query<StreamQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(stream_handler)
}

/// Sends a single structured "not implemented" frame and closes.
///
/// Match shape with the [`crate::error::ApiError::NotImplemented`]
/// JSON body so clients can decode HTTP errors and WS errors with one
/// type.
async fn stream_handler(mut socket: WebSocket) {
    let body = serde_json::json!({
        "type": "error",
        "code": "not_implemented",
        "message": "M8 deliverable",
    });
    let payload = serde_json::to_string(&body).unwrap_or_else(|_| String::from("{}"));

    // Best-effort send + close; errors during shutdown are logged only.
    if let Err(err) = socket.send(Message::Text(payload)).await {
        tracing::debug!(error = %err, "diaspor-api-server: ws send failed during stub close");
    }
    if let Err(err) = socket.close().await {
        tracing::debug!(error = %err, "diaspor-api-server: ws close failed during stub close");
    }
}
