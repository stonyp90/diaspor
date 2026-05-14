//! Minimal example: mount a directory on the host filesystem as a backend and walk it.
//!
//! Run with: `cargo run --example local-mount -- /path/to/dir`

use std::env;
use std::process::ExitCode;

use cairn_backend_local::LocalBackend;
use cairn_core::{VfsBackend, VfsPath};

#[tokio::main]
async fn main() -> ExitCode {
    let Some(root) = env::args().nth(1) else {
        eprintln!("usage: local-mount <host-directory>");
        return ExitCode::from(2);
    };

    let backend = match LocalBackend::new(&root) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("failed to open backend: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("Listing {} via {}:", root, backend.name());
    match backend.list(&VfsPath::root()).await {
        Ok(entries) => {
            for entry in entries {
                println!("  {entry}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("list error: {e}");
            ExitCode::FAILURE
        }
    }
}
