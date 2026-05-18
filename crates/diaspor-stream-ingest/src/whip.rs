//! WHIP push ingest adapter (Phase 1.5 — M8).
//!
//! Placeholder for an [RFC 9725 WHIP][rfc9725] (`WebRTC-HTTP Ingestion Protocol`) push
//! receiver. The intended deployment shape is a sidecar SFU (`Pion` or `mediasoup`) that
//! terminates the WebRTC transport and forwards encoded RTP packets to this adapter over
//! a local control channel; this crate stays pure-Rust and free of `webrtc-rs` /
//! `tonic` / `tungstenite` dependencies.
//!
//! ## Status
//!
//! v0.1.0-alpha exposes the [`WhipConfig`] type and a [`WhipIngest`] stub whose
//! [`StreamIngest::start`] returns [`StreamIngestError::NotImplemented`]. Full WHIP
//! wiring lands in milestone **M8 — live ingest**.
//!
//! [rfc9725]: https://www.rfc-editor.org/rfc/rfc9725

use async_trait::async_trait;
use diaspor_core::Result;

use crate::{IngestStream, StreamIngest, StreamIngestError};

/// Configuration for a [`WhipIngest`] adapter.
///
/// In the M8 implementation, the adapter will POST a session-description offer to the
/// SFU sidecar at `sfu_endpoint`, authenticated with `auth_token` (Bearer), and receive
/// forwarded RTP through whatever local channel the SFU exposes.
#[derive(Debug, Clone)]
pub struct WhipConfig {
    /// Base URL of the SFU sidecar that terminates the WebRTC transport.
    pub sfu_endpoint: String,
    /// Bearer token presented to the SFU on session creation.
    pub auth_token: String,
}

/// WHIP push ingest adapter — v0.1 stub.
///
/// Construct with [`WhipIngest::new`]; calling [`StreamIngest::start`] currently returns
/// [`StreamIngestError::NotImplemented`] until M8 lands.
#[derive(Debug, Clone)]
pub struct WhipIngest {
    /// SFU + auth config carried for the future real implementation.
    config: WhipConfig,
}

impl WhipIngest {
    /// Constructs a new WHIP adapter from `config`.
    #[must_use]
    pub const fn new(config: WhipConfig) -> Self {
        Self { config }
    }

    /// Returns the configuration this adapter was constructed with.
    #[must_use]
    pub const fn config(&self) -> &WhipConfig {
        &self.config
    }
}

#[async_trait]
impl StreamIngest for WhipIngest {
    fn name(&self) -> &'static str {
        "whip"
    }

    async fn start(&self) -> Result<IngestStream> {
        tracing::info!(
            target: "diaspor_stream_ingest::whip",
            sfu_endpoint = %self.config.sfu_endpoint,
            "WHIP ingest is a Phase 1.5 placeholder (lands in M8); returning NotImplemented",
        );
        Err(StreamIngestError::NotImplemented { transport: "whip" }.into())
    }
}
