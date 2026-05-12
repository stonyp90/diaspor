//! # `stony-vdfs` CLI
//!
//! Operator tooling for exercising backends. Useful for demos, smoke tests, and as a
//! reference implementation of common workflows on top of the library.
//!
//! ```text
//! stony-vdfs list <backend> <vfs-path>
//! stony-vdfs cat  <backend> <vfs-path>
//! stony-vdfs put  <backend> <vfs-path> < input.bin
//! ```
//!
//! Backends:
//!  - `memory`             — ephemeral in-process backend (resets on every invocation)
//!  - `local:<host-path>`  — local filesystem rooted at `<host-path>`

use std::sync::Arc;

use clap::{Parser, Subcommand};
use stony_vdfs_backend_local::LocalBackend;
use stony_vdfs_backend_memory::MemoryBackend;
use stony_vdfs_core::{OpenFlags, Result, VfsBackend, VfsError, VfsPath};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Parser, Debug)]
#[command(name = "stony-vdfs", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List the entries of a directory at <path>.
    List { backend: String, path: String },
    /// Print the contents of <path> to stdout.
    Cat { backend: String, path: String },
    /// Write stdin into <path>, truncating any existing data.
    Put { backend: String, path: String },
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "stony_vdfs=info,info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::List { backend, path } => run_list(&backend, &path).await?,
        Command::Cat { backend, path } => run_cat(&backend, &path).await?,
        Command::Put { backend, path } => run_put(&backend, &path).await?,
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
