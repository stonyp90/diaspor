//! `diaspor-api-server` binary — the host process for `api.diaspor.io`.
//!
//! Calls [`diaspor_api_server::build_router`] and serves it on the address
//! read from the `DIASPOR_BIND_ADDR` environment variable, falling back to
//! `127.0.0.1:7733` for local development. At v0.1.0-alpha.1 the
//! binary also reads its JWT signing secret and rate-limit defaults from
//! the environment via [`diaspor_api_server::Config::from_env`] — see
//! that doc for the list of recognised vars.
//!
//! Production deployments wrap this binary with TLS termination,
//! structured logging configuration, graceful shutdown, and the
//! `ClickHouse` meter flush task.
//!
//! ## Bind address
//!
//! - Local dev (no env var): `127.0.0.1:7733` — loopback only.
//! - Container / Kubernetes: set `DIASPOR_BIND_ADDR=0.0.0.0:7733` so the
//!   listener is reachable from outside the pod. The container image
//!   bundled in `deploy/docker/diaspor-api-server/Dockerfile` does this
//!   automatically via `ENV DIASPOR_BIND_ADDR=0.0.0.0:7733`.

use std::env;
use std::net::SocketAddr;

use diaspor_api_server::{AppState, Config, build_router};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

/// Default bind address when `DIASPOR_BIND_ADDR` is unset. `127.0.0.1`
/// only — production deployments put the TLS terminator (Cloudflare /
/// Kong) in front of this binary, and containerised deployments override
/// this via the `DIASPOR_BIND_ADDR` env var to bind `0.0.0.0`.
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:7733";

/// Environment variable read at startup to override the bind address.
const BIND_ADDR_ENV: &str = "DIASPOR_BIND_ADDR";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Loaded once at startup. Fails the binary if `DIASPOR_JWT_SECRET`
    // is missing — the API refuses to run without a compliance key.
    let config = Config::from_env()?;
    let state = AppState::new(config);

    let bind = env::var(BIND_ADDR_ENV).unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());
    let addr: SocketAddr = bind.parse()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(
        bind = %addr,
        "diaspor-api-server: listening (v0.1.0-alpha.1 — HS256 JWT auth, per-key rate limit, per-day cap; handlers still 501 until M10)"
    );

    axum::serve(listener, build_router(state)).await?;
    Ok(())
}
