//! LL-HLS pull ingest adapter (Phase 2 — M9).
//!
//! Placeholder for a Low-Latency HLS pull-based ingest path. Given a manifest URL, the
//! M9 implementation will poll the HLS playlist, fetch new media segments as they
//! appear, and convert each segment into an [`crate::IngestEvent::FramesArrived`] batch.
//!
//! Unlike [`WhipIngest`](crate::WhipIngest), this adapter is *pull*-based: the source
//! publisher decides when bytes are available, and we lag behind by at most
//! [`HlsConfig::max_lag_seconds`]. If the manifest stops advancing past that threshold,
//! the adapter MUST terminate with [`SessionEndReason::ClientDisconnected`].
//!
//! ## Status
//!
//! v0.1.0-alpha exposes the [`HlsConfig`] type and a [`HlsIngest`] stub whose
//! [`StreamIngest::start`] returns [`StreamIngestError::NotImplemented`]. Full LL-HLS
//! wiring lands in milestone **M9 — live ingest extensions**.
//!
//! [`SessionEndReason::ClientDisconnected`]: crate::SessionEndReason::ClientDisconnected

use async_trait::async_trait;
use diaspor_core::Result;

use crate::{IngestStream, StreamIngest, StreamIngestError};

/// Configuration for an [`HlsIngest`] adapter.
#[derive(Debug, Clone)]
pub struct HlsConfig {
    /// URL of the LL-HLS master / media playlist to follow.
    pub manifest_url: String,
    /// Maximum tolerated lag, in seconds, between the latest segment in the manifest
    /// and the wall clock. If the manifest stalls past this threshold the adapter MUST
    /// emit `SessionEnded { reason: ClientDisconnected, .. }`.
    pub max_lag_seconds: u32,
}

/// LL-HLS pull ingest adapter — v0.1 stub.
///
/// Construct with [`HlsIngest::new`]; calling [`StreamIngest::start`] currently returns
/// [`StreamIngestError::NotImplemented`] until M9 lands.
#[derive(Debug, Clone)]
pub struct HlsIngest {
    /// Pull config carried for the future real implementation.
    config: HlsConfig,
}

impl HlsIngest {
    /// Constructs a new LL-HLS adapter from `config`.
    #[must_use]
    pub const fn new(config: HlsConfig) -> Self {
        Self { config }
    }

    /// Returns the configuration this adapter was constructed with.
    #[must_use]
    pub const fn config(&self) -> &HlsConfig {
        &self.config
    }
}

#[async_trait]
impl StreamIngest for HlsIngest {
    fn name(&self) -> &'static str {
        "hls"
    }

    async fn start(&self) -> Result<IngestStream> {
        tracing::info!(
            target: "diaspor_stream_ingest::hls",
            manifest_url = %self.config.manifest_url,
            max_lag_seconds = self.config.max_lag_seconds,
            "LL-HLS ingest is a Phase 2 placeholder (lands in M9); returning NotImplemented",
        );
        Err(StreamIngestError::NotImplemented { transport: "hls" }.into())
    }
}
