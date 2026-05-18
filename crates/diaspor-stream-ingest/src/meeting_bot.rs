//! Meeting-bot ingest adapter (Phase 1.5 — M8).
//!
//! Pulls audio + video from third-party meeting platforms (Zoom, Google Meet, Microsoft
//! Teams) by joining the call as a bot participant. The default Phase 1.5 transport is
//! [`Recall.ai`][recall] — a vendor that abstracts the per-platform SDK churn behind a
//! single webhook + WebSocket API. Phase 2 (M9) will add direct platform SDK adapters
//! ([`BotProvider::ZoomSdk`], [`BotProvider::GoogleMeetMediaApi`],
//! [`BotProvider::TeamsBotFramework`]) once the lower-cost-per-minute math and SDK
//! coverage justify the maintenance burden.
//!
//! # Compliance invariant — all-party consent (permanent, non-negotiable)
//!
//! Meeting recordings carry a strict legal floor that *every* real implementation of
//! this adapter MUST honor before emitting a single
//! [`IngestEvent::FramesArrived`](crate::IngestEvent::FramesArrived):
//!
//! - **Quebec — Loi 25 (LPRPDE-QC).** All identifiable individuals on the call must have
//!   given valid, informed consent before any biometric or audio capture begins. Implicit
//!   consent (e.g. "joining the meeting = consent") is *not* sufficient.
//! - **Illinois — BIPA (740 ILCS 14/).** Any pipeline downstream of this adapter that
//!   extracts face or voiceprint embeddings (see `diaspor-vision`) triggers BIPA's
//!   written-consent regime. The adapter MUST surface explicit per-participant consent
//!   before frames are forwarded.
//! - **EU — GDPR Art. 6 + Art. 9.** Biometric data is a *special category*. Lawful basis
//!   under Art. 9(2)(a) requires *explicit* consent — silence or absence of objection is
//!   not consent.
//! - **EU AI Act — Title II, Art. 5(1)(f).** "AI systems for emotion recognition in the
//!   workplace and educational institutions" are prohibited. Any credibility / emotion
//!   inference downstream of this adapter MUST be gated on context (sport judging,
//!   security screening — yes; HR / hiring / classroom — banned).
//!
//! Real implementations MUST:
//!
//! 1. Wait for the meeting-platform's consent prompt to be accepted by every
//!    participant before transitioning the session from "joining" to "recording".
//! 2. Surface a participant's mid-call consent withdrawal as
//!    [`SessionEndReason::BotConsentDeclined`] and drop any buffered pre-consent bytes.
//! 3. Treat a host-initiated removal as
//!    [`SessionEndReason::BotEjected`] — implicit revocation, same retention semantics.
//! 4. Never persist bytes received before the `SessionStarted` event corresponding to
//!    the post-consent transition.
//!
//! Adapters that cannot guarantee the above MUST return
//! [`StreamIngestError::ConsentViolation`] from [`StreamIngest::start`] rather than
//! emit a partially-compliant stream.
//!
//! ## Status
//!
//! v0.1.0-alpha exposes the [`MeetingBotConfig`] type and a [`MeetingBotIngest`] stub
//! whose [`StreamIngest::start`] returns [`StreamIngestError::NotImplemented`]. Recall.ai
//! wiring lands in milestone **M8 — live ingest**; direct SDK adapters in **M9**.
//!
//! [recall]: https://recall.ai
//! [`SessionEndReason::BotConsentDeclined`]: crate::SessionEndReason::BotConsentDeclined
//! [`SessionEndReason::BotEjected`]: crate::SessionEndReason::BotEjected

use async_trait::async_trait;
use diaspor_core::Result;

use crate::{IngestStream, StreamIngest, StreamIngestError};

/// Which meeting-platform transport a [`MeetingBotIngest`] uses.
///
/// [`BotProvider::RecallAi`] is the Phase 1.5 default — a single vendor API that
/// abstracts per-platform SDK churn. The direct-SDK variants are Phase 2 evolution
/// targets; they are listed here so the trait surface is stable across the milestone
/// transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotProvider {
    /// [Recall.ai][recall] — webhook + WebSocket adapter. Phase 1.5 default.
    ///
    /// [recall]: https://recall.ai
    RecallAi,
    /// Native Zoom Meeting SDK. Phase 2.
    ZoomSdk,
    /// Google Meet Media API. Phase 2.
    GoogleMeetMediaApi,
    /// Microsoft Teams Bot Framework + Communications Calling. Phase 2.
    TeamsBotFramework,
}

impl BotProvider {
    /// Short, log-friendly name for this provider.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RecallAi => "recall-ai",
            Self::ZoomSdk => "zoom-sdk",
            Self::GoogleMeetMediaApi => "google-meet-media-api",
            Self::TeamsBotFramework => "teams-bot-framework",
        }
    }
}

/// Configuration for a [`MeetingBotIngest`] adapter.
#[derive(Debug, Clone)]
pub struct MeetingBotConfig {
    /// Which meeting-platform transport to use.
    pub provider: BotProvider,
    /// API key / Bearer token for the chosen provider.
    pub api_key: String,
    /// Display name the bot will join the meeting under.
    ///
    /// Visible to every other participant — by policy this MUST make the recording
    /// purpose obvious (e.g. `"Diaspor Recording Bot"`) so participants can refuse
    /// consent meaningfully.
    pub bot_display_name: String,
    /// Verbatim script the bot speaks (or types into chat) when it joins, to obtain
    /// active consent from every participant. Non-empty. Localization is the caller's
    /// responsibility for now.
    pub consent_script: String,
    /// Seconds the bot MUST wait after joining before it starts forwarding any frames,
    /// giving participants time to object or leave. Floor is enforced by the real M8
    /// implementation; stubs accept any value.
    pub recording_delay_seconds: u32,
    /// URL of the meeting the bot will join (Zoom join link, Meet URL, Teams meeting
    /// URL).
    pub meeting_url: String,
}

/// Meeting-bot ingest adapter — v0.1 stub.
///
/// Construct with [`MeetingBotIngest::new`]; calling [`StreamIngest::start`] currently
/// returns [`StreamIngestError::NotImplemented`] for every provider until M8 lands
/// Recall.ai and M9 lands the direct SDK adapters.
#[derive(Debug, Clone)]
pub struct MeetingBotIngest {
    /// Provider + auth + consent config carried for the future real implementation.
    config: MeetingBotConfig,
}

impl MeetingBotIngest {
    /// Constructs a new meeting-bot adapter from `config`.
    #[must_use]
    pub const fn new(config: MeetingBotConfig) -> Self {
        Self { config }
    }

    /// Returns the configuration this adapter was constructed with.
    #[must_use]
    pub const fn config(&self) -> &MeetingBotConfig {
        &self.config
    }

    /// Returns the provider this adapter is configured for.
    #[must_use]
    pub const fn provider(&self) -> BotProvider {
        self.config.provider
    }
}

#[async_trait]
impl StreamIngest for MeetingBotIngest {
    fn name(&self) -> &'static str {
        "meeting-bot"
    }

    async fn start(&self) -> Result<IngestStream> {
        match self.config.provider {
            BotProvider::RecallAi => {
                tracing::info!(
                    target: "diaspor_stream_ingest::meeting_bot",
                    provider = BotProvider::RecallAi.as_str(),
                    meeting_url = %self.config.meeting_url,
                    "meeting-bot ingest: Recall.ai is the Phase 1.5 default (lands in M8); returning NotImplemented",
                );
            }
            BotProvider::ZoomSdk
            | BotProvider::GoogleMeetMediaApi
            | BotProvider::TeamsBotFramework => {
                tracing::info!(
                    target: "diaspor_stream_ingest::meeting_bot",
                    provider = self.config.provider.as_str(),
                    meeting_url = %self.config.meeting_url,
                    "meeting-bot ingest: direct SDK adapters are Phase 2 evolution (land in M9); returning NotImplemented",
                );
            }
        }
        Err(StreamIngestError::NotImplemented {
            transport: "meeting-bot",
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StreamIngest;
    use diaspor_core::VfsError;

    fn sample_config(provider: BotProvider) -> MeetingBotConfig {
        MeetingBotConfig {
            provider,
            api_key: "test-key".to_string(),
            bot_display_name: "Diaspor Recording Bot".to_string(),
            consent_script: "This meeting is being recorded for sport judging review."
                .to_string(),
            recording_delay_seconds: 5,
            meeting_url: "https://meet.example.com/abc-defg-hij".to_string(),
        }
    }

    #[tokio::test]
    async fn recall_ai_start_returns_not_implemented() {
        let ingest = MeetingBotIngest::new(sample_config(BotProvider::RecallAi));
        assert_eq!(ingest.name(), "meeting-bot");
        assert_eq!(ingest.provider(), BotProvider::RecallAi);

        // `IngestStream` is a boxed dyn Stream that does not impl Debug, so we can't
        // use `expect_err` — match on the result instead.
        match ingest.start().await {
            Err(VfsError::Backend(msg)) => {
                assert!(
                    msg.contains("meeting-bot") && msg.contains("not implemented"),
                    "expected NotImplemented backend error, got: {msg}",
                );
            }
            Err(other) => panic!("expected VfsError::Backend, got {other:?}"),
            Ok(_) => panic!("v0.1 Recall.ai stub must return NotImplemented, got Ok"),
        }
    }
}
