//! # `diaspor-agent`
//!
//! Diaspor — long-running non-verbal AI runtime daemon.
//!
//! `diaspor-agent` is the runtime that loads the `diaspor-*` model family
//! (speech, vision, face-mesh, pose-3d, prosody, credibility, judge, rag)
//! and exposes them on the local network. The companion CLI is `diaspor`.
//!
//! ```text
//! diaspor-agent serve --local --gpu=auto --port 7733
//! diaspor-agent status
//! diaspor-agent stop
//! ```
//!
//! At `v0.1.0-alpha.1` this binary is a branded scaffold: argument
//! parsing and help text are wired, the runtime body is not yet
//! implemented (tracked in ROADMAP M7–M10).

use clap::{Parser, Subcommand};

const ABOUT: &str = "Diaspor · long-running non-verbal AI runtime daemon";

const LONG_ABOUT: &str = "\
Diaspor · self-hosted non-verbal AI for Canadian security and legal teams.

This is the long-running runtime daemon. It loads the diaspor-* model
family — speech, vision, face-mesh, pose-3d, prosody, credibility,
judge, rag — and exposes them on the local network via the runtime
protocol consumed by the SDKs and the optional Cloud API gateway.

Decision-aid only. Mandatory human review for credibility signals.
Excluded from EU workplace and education contexts (AI Act, Aug 2026)
and from forensic, hiring, insurance and law-enforcement adjudication
use cases.

Compliance: Loi 25 · PIPEDA · Bill 96 · AGPL-3.0
Site:       https://diaspor.io
Releases:   https://github.com/stonyp90/diaspor/releases
Operator:   diaspor --help";

#[derive(Parser, Debug)]
#[command(
    name = "diaspor-agent",
    version,
    about = ABOUT,
    long_about = LONG_ABOUT,
    propagate_version = true,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the runtime daemon and bind to the configured port.
    Serve {
        /// Bind to local interfaces only — no external traffic.
        #[arg(long)]
        local: bool,
        /// GPU selection: `auto`, `none`, or a specific device id.
        #[arg(long, default_value = "auto")]
        gpu: String,
        /// Bind port. Default: 7733.
        #[arg(long, default_value_t = 7733_u16)]
        port: u16,
    },
    /// Report runtime status — loaded models, GPU, port, health.
    Status,
    /// Stop a running daemon via its local control socket.
    Stop,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "diaspor=info,info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Serve { local, gpu, port } => {
            eprintln!("diaspor-agent serve · local={local} · gpu={gpu} · port={port}");
            not_yet_implemented("runtime daemon");
        }
        Command::Status => not_yet_implemented("status"),
        Command::Stop => not_yet_implemented("stop"),
    }
}

fn not_yet_implemented(what: &str) -> ! {
    eprintln!(
        "diaspor-agent: {what} is not yet implemented in v0.1.0-alpha.1 \
         (tracked in ROADMAP M7-M10). See https://diaspor.io for the runtime \
         roadmap."
    );
    std::process::exit(1);
}
