//! # `diaspor` CLI
//!
//! Diaspor — operator CLI for the self-hosted non-verbal AI runtime.
//!
//! `diaspor` is the one-shot operator entry point. It exercises the storage
//! backends that the long-running runtime (see `diaspor-agent`) uses for
//! recorded interviews, transcripts and scored evidence.
//!
//! For the long-running analysis daemon, see the `diaspor-agent` binary.
//!
//! ```text
//! diaspor list <backend> <vfs-path>
//! diaspor cat  <backend> <vfs-path>
//! diaspor put  <backend> <vfs-path> < input.bin
//! ```
//!
//! Backends:
//!  - `memory`             — ephemeral in-process backend (resets on every invocation)
//!  - `local:<host-path>`  — local filesystem rooted at `<host-path>`

use std::sync::Arc;

use clap::{Parser, Subcommand};
use diaspor_backend_local::LocalBackend;
use diaspor_backend_memory::MemoryBackend;
use diaspor_core::{OpenFlags, Result, VfsBackend, VfsError, VfsPath};
use diaspor_imagegen::adapters::{GeminiImageAdapter, LocalImageAdapter, OpenAiImageAdapter};
use diaspor_imagegen::{GenerateRequest, ImageCompositor, ImageStudio, Policy};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const ABOUT: &str = "Diaspor · operator CLI for the non-verbal AI runtime";

const LONG_ABOUT: &str = "\
Diaspor · self-hosted non-verbal AI for Canadian security and legal teams.

This is the one-shot operator CLI. It exercises the storage backends used
by the runtime (see `diaspor-agent` for the long-running daemon) — listing,
reading and writing recorded evidence, transcripts and scored sidecars.

Compliance: Loi 25 · PIPEDA · Bill 96 · AGPL-3.0
Site:       https://diaspor.io
Releases:   https://github.com/stonyp90/diaspor/releases
Agent:      diaspor-agent --help";

#[derive(Parser, Debug)]
#[command(
    name = "diaspor",
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
    /// List the entries of a directory at `path`.
    List { backend: String, path: String },
    /// Print the contents of `path` to stdout.
    Cat { backend: String, path: String },
    /// Write stdin into `path`, truncating any existing data.
    Put { backend: String, path: String },
    /// Generate images with the cost/quality-routed studio.
    Image {
        #[command(subcommand)]
        cmd: ImageCmd,
    },
}

#[derive(Subcommand, Debug)]
enum ImageCmd {
    /// Generate an image from a text prompt and write it to a PNG file.
    ///
    /// Uses the offline local adapter by default; if `OPENAI_API_KEY` and/or
    /// `GEMINI_API_KEY` are set, those providers join the cost/quality router.
    Generate {
        /// What to draw.
        prompt: String,
        /// Output PNG path.
        #[arg(short, long, default_value = "out.png")]
        out: String,
        /// Canvas width in pixels.
        #[arg(long, default_value_t = 1024)]
        width: u32,
        /// Canvas height in pixels.
        #[arg(long, default_value_t = 1024)]
        height: u32,
        /// Routing policy: `cost`, `quality`, or `balanced:<usd>`.
        #[arg(long, default_value = "quality")]
        policy: String,
    },
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "diaspor=info,info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::List { backend, path } => run_list(&backend, &path).await?,
        Command::Cat { backend, path } => run_cat(&backend, &path).await?,
        Command::Put { backend, path } => run_put(&backend, &path).await?,
        Command::Image { cmd } => run_image(cmd).await?,
    }
    Ok(())
}

fn parse_backend(spec: &str) -> Result<Arc<dyn VfsBackend>> {
    if spec == "memory" {
        return Ok(Arc::new(MemoryBackend::new()));
    }
    if let Some(root) = spec.strip_prefix("local:") {
        return Ok(Arc::new(LocalBackend::new(root)?));
    }
    Err(VfsError::backend(format!(
        "unknown backend spec: {spec} (expected 'memory' or 'local:<path>')"
    )))
}

fn parse_path(s: &str) -> Result<VfsPath> {
    VfsPath::new(s).ok_or_else(|| VfsError::invalid_path(s))
}

async fn run_list(backend: &str, path: &str) -> Result<()> {
    let backend = parse_backend(backend)?;
    let path = parse_path(path)?;
    for child in backend.list(&path).await? {
        println!("{child}");
    }
    Ok(())
}

async fn run_cat(backend: &str, path: &str) -> Result<()> {
    let backend = parse_backend(backend)?;
    let path = parse_path(path)?;
    let mut handle = backend.open(&path, OpenFlags::READ).await?;
    let mut offset: u64 = 0;
    let mut stdout = tokio::io::stdout();
    loop {
        let chunk = handle.read(offset, 16 * 1024).await?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len() as u64;
        stdout.write_all(&chunk).await?;
    }
    stdout.flush().await?;
    Ok(())
}

async fn run_put(backend: &str, path: &str) -> Result<()> {
    let backend = parse_backend(backend)?;
    let path = parse_path(path)?;
    let mut handle = backend
        .open(
            &path,
            OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNC,
        )
        .await?;
    let mut buf = vec![0u8; 16 * 1024];
    let mut stdin = tokio::io::stdin();
    let mut offset: u64 = 0;
    loop {
        let n = stdin.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        handle.write(offset, &buf[..n]).await?;
        offset += n as u64;
    }
    handle.flush().await?;
    Ok(())
}

/// Build a studio from whatever providers are configured: the offline local
/// adapter is always present; `OpenAI` / `Gemini` join if their API keys are set.
fn build_studio() -> diaspor_imagegen::Result<ImageStudio> {
    let local = Arc::new(LocalImageAdapter::new());
    let openai = OpenAiImageAdapter::new(None).ok().map(Arc::new);

    // Prefer the OpenAI compositor when configured, else the offline local one.
    let compositor: Arc<dyn ImageCompositor> = match &openai {
        Some(openai) => openai.clone(),
        None => local.clone(),
    };

    let mut builder = ImageStudio::builder()
        .generator(local)
        .compositor(compositor);
    if let Some(openai) = openai {
        builder = builder.generator(openai);
    }
    if let Ok(gemini) = GeminiImageAdapter::new(None) {
        builder = builder.generator(Arc::new(gemini));
    }
    builder.build()
}

fn parse_policy(spec: &str) -> std::result::Result<Policy, Box<dyn std::error::Error>> {
    if spec == "cost" {
        return Ok(Policy::CostOptimized);
    }
    if spec == "quality" {
        return Ok(Policy::QualityFirst);
    }
    if let Some(budget) = spec.strip_prefix("balanced:") {
        return Ok(Policy::Balanced {
            max_cost_usd: budget.parse()?,
        });
    }
    Err(format!("unknown policy '{spec}' (use cost|quality|balanced:<usd>)").into())
}

async fn run_image(cmd: ImageCmd) -> std::result::Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ImageCmd::Generate {
            prompt,
            out,
            width,
            height,
            policy,
        } => {
            let studio = build_studio()?;
            let policy = parse_policy(&policy)?;
            let request = GenerateRequest::new(prompt, width, height);
            let image = studio.generate(&request, &policy).await?;
            std::fs::write(&out, &image.bytes)?;
            eprintln!(
                "wrote {}x{} image ({} bytes) to {out}",
                image.width,
                image.height,
                image.bytes.len()
            );
            Ok(())
        }
    }
}
