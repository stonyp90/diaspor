//! `live-meeting-bot` — public API surface contract for milestone **M8**.
//!
//! At `v0.1.0-alpha.1` this example **does not actually join a meeting**. Its job is to
//! demonstrate that the live-ingest trait surface ([`StreamIngest`], [`IngestEvent`],
//! [`BotProvider`]) wires through [`MeetingBotIngest`] today, and that the
//! [`diaspor_events::MultiSink`] fan-out composes a [`diaspor_events::VfsEventSink`]
//! + [`diaspor_events::WebhookEventSink`] cleanly — so callers can build downstream
//!   pipelines against the stable signatures before M8 lands the real `Recall.ai` wiring.
//!
//! What the example demonstrates end-to-end:
//!
//! 1. Building a [`MeetingBotConfig`] for [`BotProvider::RecallAi`] (the Phase 1.5
//!    default; see `meeting_bot` module docs for the per-platform context).
//! 2. Calling [`StreamIngest::start`] on it — at the alpha this returns
//!    [`StreamIngestError::NotImplemented`], which is the expected M8 deliverable line.
//! 3. Constructing a [`diaspor_events::MultiSink`] over a stub [`diaspor_events::VfsEventSink`]
//!    (backed by an in-memory VFS) and a stub [`diaspor_events::WebhookEventSink`], to
//!    show how live ingest will fan score events out once the bot is producing frames.
//! 4. The `IngestEvent` stream consumption pattern (with `StreamExt::next`) — kept so the
//!    code path is exercised at compile time even though it's unreachable at v0.1 today.
//!
//! Run it with:
//!
//! ```bash
//! # Default placeholder meeting URL:
//! cargo run --example live-meeting-bot
//!
//! # Or pass your own meeting URL:
//! cargo run --example live-meeting-bot -- https://meet.example.com/abc-defg-hij
//! ```
//!
//! See ROADMAP.md milestone M8 for the implementation tracking work.

use std::env;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use diaspor_backend_memory::MemoryBackend;
use diaspor_events::{MultiSink, VfsEventSink, WebhookEventSink};
use diaspor_stream_ingest::{
    BotProvider, IngestEvent, MeetingBotConfig, MeetingBotIngest, SessionEndReason, StreamIngest,
};
use futures::StreamExt;

const DEFAULT_PLACEHOLDER_MEETING_URL: &str = "https://meet.example.com/diaspor-demo-call";

#[tokio::main]
async fn main() -> ExitCode {
    let meeting_url = env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_PLACEHOLDER_MEETING_URL.to_string());

    println!("======================================================================");
    println!("  diaspor live-meeting-bot (v0.1.0-alpha.1, M8 surface only)");
    println!("======================================================================");
    println!();

    // ------------------------------------------------------------------------
    // Step 1 — compose the event fan-out the running bot will write into.
    //
    // MultiSink is the only piece of `diaspor-events` that ships as real code in
    // v0.1.0-alpha (see `diaspor-events::sink::MultiSink` docs). The underlying
    // VfsEventSink + WebhookEventSink return NotImplemented from their `emit` — but
    // composing them through MultiSink demonstrates the integration shape M8 will use.
    // ------------------------------------------------------------------------
    let vfs: Arc<dyn diaspor_core::VfsBackend> = Arc::new(MemoryBackend::new());
    let vfs_sink = VfsEventSink::new(vfs, "/.streams");
    let webhook_sink = WebhookEventSink::new(
        "https://example.test/diaspor-webhook",
        "demo-hmac-secret",
        Duration::from_secs(5),
    );
    let multi_sink = MultiSink::new(vec![Box::new(vfs_sink), Box::new(webhook_sink)]);
    println!(
        "Composed MultiSink with {} stub event sinks (vfs + webhook).",
        multi_sink.len()
    );
    println!("  sink[0] = vfs       (production: writes /.streams/<id>/windows/*.score.json)");
    println!("  sink[1] = webhook   (production: POST + X-Diaspor-Signature HMAC-SHA256)");
    println!();

    // ------------------------------------------------------------------------
    // Step 2 — build a MeetingBotIngest targeting Recall.ai (Phase 1.5 default for M8).
    // ------------------------------------------------------------------------
    let bot_config = MeetingBotConfig {
        provider: BotProvider::RecallAi,
        api_key: "demo-recall-ai-key".to_string(),
        bot_display_name: "Diaspor Recording Bot".to_string(),
        consent_script: "This meeting is being recorded for sport-judging review. \
             Please decline if you do not consent."
            .to_string(),
        recording_delay_seconds: 5,
        meeting_url: meeting_url.clone(),
    };
    let bot = MeetingBotIngest::new(bot_config);

    println!("Constructed MeetingBotIngest:");
    println!("  name        = {}", bot.name());
    println!("  provider    = {}", bot.provider().as_str());
    println!("  meeting_url = {meeting_url}");
    println!();

    // ------------------------------------------------------------------------
    // Step 3 — attempt to start the session. v0.1.0-alpha SHORT-CIRCUITS here.
    // ------------------------------------------------------------------------
    println!("Calling StreamIngest::start() ...");
    let stream_result = bot.start().await;

    let mut stream = match stream_result {
        Ok(stream) => {
            // Unreachable at v0.1.0-alpha.1 — kept so the IngestEvent consumption
            // pattern compiles as a forward-looking template for M8.
            println!("UNEXPECTED Ok at the alpha — entering the event-stream consumer loop.");
            stream
        }
        Err(err) => {
            println!();
            println!("Bot start failed (this is the EXPECTED v0.1.0-alpha.1 behavior).");
            println!();
            println!("  transport = meeting-bot (Recall.ai)");
            println!("  error     = {err}");
            println!();
            println!("This is correct: the trait surface composes but the transport is");
            println!("a stub. Real Recall.ai webhook + WebSocket wiring lands in milestone");
            println!("M8 — see ROADMAP.md for the tracking work.");
            return ExitCode::FAILURE;
        }
    };

    // ------------------------------------------------------------------------
    // Step 4 — IngestEvent stream consumption pattern (unreachable at v0.1).
    //
    // Kept here so the compile-time contract is exercised: every adapter must emit
    // SessionStarted → 0..N FramesArrived → SessionEnded, and the downstream pipeline
    // dispatches the FramesArrived bytes into diaspor-vision while routing window
    // aggregates through the MultiSink composed above.
    // ------------------------------------------------------------------------
    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(IngestEvent::SessionStarted {
                session_id,
                started_at,
            }) => {
                println!("session_started: id={session_id} at={started_at}");
            }
            Ok(IngestEvent::FramesArrived {
                session_id,
                batch_bytes,
                timestamp_us,
            }) => {
                println!(
                    "frames_arrived: id={session_id} ts={timestamp_us}us bytes={}",
                    batch_bytes.len()
                );
                // Real M8: feed `batch_bytes` into diaspor-frame-pipeline +
                // diaspor-vision, then emit aggregate Events through `multi_sink`.
            }
            Ok(IngestEvent::SessionEnded {
                session_id,
                reason,
                ended_at,
            }) => {
                let reason_str = match reason {
                    SessionEndReason::Completed => "completed",
                    SessionEndReason::ClientDisconnected => "client_disconnected",
                    SessionEndReason::Failed(ref msg) => msg.as_str(),
                    SessionEndReason::BotEjected => "bot_ejected",
                    SessionEndReason::BotConsentDeclined => "bot_consent_declined",
                };
                println!("session_ended: id={session_id} reason={reason_str} at={ended_at}");
            }
            Err(err) => {
                eprintln!("ingest stream error: {err}");
            }
        }
    }

    // Defensive: keep the multi_sink alive past the loop above so the consumer-pattern
    // borrow chain stays valid for a future real run. At the alpha this is unreachable.
    let _keep = &multi_sink;
    ExitCode::SUCCESS
}
