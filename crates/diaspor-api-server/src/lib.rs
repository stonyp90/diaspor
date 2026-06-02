//! # diaspor-api-server
//!
//! Hosted HTTP + WebSocket API server for `diaspor` — the binary that runs
//! `api.diaspor.io`. Composes the other M7+ crates (`diaspor-vision`,
//! `diaspor-infer`, `diaspor-events`, `diaspor-stream-ingest`,
//! `diaspor-train`) behind a clean REST + WebSocket surface, applies
//! **per-key vertical attestation** for the credibility endpoint, and meters
//! usage for billing.
//!
//! ## Route table
//!
//! ```text
//!   ┌────────────────────────────────────────────────────────────────────────────┐
//!   │  Method   Path                              Purpose                        │
//!   ├────────────────────────────────────────────────────────────────────────────┤
//!   │  POST     /v1/analyze                       Multi-modal analysis (batch)   │
//!   │  GET      /v1/analyses/:id                  Poll long-running analysis     │
//!   │                                                                            │
//!   │  POST     /v1/pose                          Pose-only inference            │
//!   │  POST     /v1/face-mesh                     Face landmarks + microexpr     │
//!   │  POST     /v1/prosody                       Vocal prosody features         │
//!   │  POST     /v1/credibility       [GATED]     Composite credibility signal   │
//!   │  POST     /v1/judge?discipline=...          Sport-judging score            │
//!   │                                                                            │
//!   │  POST     /v1/train                         Submit corpus + eval config    │
//!   │  POST     /v1/adapters/:id/activate         Swap adapter into pool         │
//!   │                                                                            │
//!   │  GET      /v1/stream  (WebSocket upgrade)   Live ingest (WHIP / meeting)   │
//!   │                                                                            │
//!   │  GET      /v1/health  [OPEN]                Liveness probe                 │
//!   └────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! `[GATED]` endpoints require the calling API key to declare an allowed
//! **vertical attestation** — see [`auth::VerticalAttestation`]. Calls from
//! keys attested to `forensic` / `hiring` / `insurance` / `law_enforcement`
//! / `eu_workplace` / `eu_education` are refused with `403 Forbidden` and
//! the [`error::ApiError::VerticalRefused`] error body *before* any model is
//! invoked. This is the load-bearing compliance gate.
//!
//! `[OPEN]` denotes the single route that bypasses both auth and rate
//! limiting — load balancers must be able to probe liveness without
//! presenting credentials.
//!
//! ## Layered middleware
//!
//! ```text
//!     ┌──────────────────────────────────────────────────────────────┐
//!     │  CORS (tower-http::cors::CorsLayer)                          │
//!     ├──────────────────────────────────────────────────────────────┤
//!     │  TraceLayer (tower-http::trace)                              │
//!     ├──────────────────────────────────────────────────────────────┤
//!     │  RequestBodyLimitLayer (tower-http::limit)                   │
//!     ├──────────────────────────────────────────────────────────────┤
//!     │  Auth middleware (HS256 JWT verification)                    │ ← all routes
//!     ├──────────────────────────────────────────────────────────────┤ ← except
//!     │  Rate-limit middleware (per-key bucket + per-day cap)        │ ← /v1/health
//!     ├──────────────────────────────────────────────────────────────┤
//!     │  Route handlers (stub at v0.1.0-alpha.1)                     │
//!     └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Status
//!
//! v0.1.0-alpha.1 ships HS256 JWT auth (`DIASPOR_JWT_SECRET` env), per-key
//! sliding-window rate limit (default 60/min) and per-day hard caps
//! (default 10,000/day) on every route except `/v1/health`. Real backend
//! handlers wire in M10 — every non-health stub still returns
//! `501 Not Implemented` with a structured
//! [`error::ApiError::NotImplemented`] body, but only *after* the auth and
//! rate-limit gates have passed. `ClickHouse`, Stripe and the actual model
//! invocations are deliberately out of scope here.
//!
//! The compliance gate (`vertical_check` on `/v1/credibility`) is exercised
//! end-to-end at this milestone — it is the legal-defense gate around the
//! credibility model. See `tests/route_table.rs`.

#![doc(html_root_url = "https://docs.rs/diaspor-api-server/0.1.0-alpha.1")]

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

pub mod auth;
pub mod error;
pub mod middleware;
pub mod rate_limit;
pub mod routes;

pub use auth::{ApiKey, Config, VerticalAttestation};
pub use error::ApiError;
pub use rate_limit::AppState;

/// Maximum request body size accepted by the API in bytes (256 MiB).
///
/// Sized to comfortably hold short uploads (a 60-second 1080p mp4 is
/// usually well under 100 MiB); longer media should use the
/// `signed_s3_url` JSON variant of `POST /v1/analyze`.
pub const MAX_REQUEST_BYTES: usize = 256 * 1024 * 1024;

/// Builds the full `api.diaspor.io` `Router` for the given runtime state.
///
/// Composition rules:
///
/// - `GET /v1/health` is mounted **outside** the auth + rate-limit layer
///   so load balancers can probe without credentials.
/// - Every other route sits behind, in order, the auth middleware
///   (HS256 JWT verification) and then the rate-limit middleware
///   (per-key bucket + per-day cap). Order is load-bearing — see the
///   docstring on [`middleware`].
///
/// The returned `Router` is `Send + 'static` and can be passed to
/// `axum::serve` or to `tower::ServiceExt::oneshot` for integration
/// testing — see `tests/route_table.rs` for examples.
pub fn build_router(state: Arc<AppState>) -> Router {
    let authed = Router::new()
        // Batch analysis — `/v1/analyze` + `/v1/analyses/:id`
        .route("/v1/analyze", post(routes::analyze::analyze))
        .route(
            "/v1/analyses/:id",
            get(routes::analyze::get_analysis_status),
        )
        // Per-modality endpoints
        .route("/v1/pose", post(routes::modality::pose))
        .route("/v1/face-mesh", post(routes::modality::face_mesh))
        .route("/v1/prosody", post(routes::modality::prosody))
        .route("/v1/credibility", post(routes::modality::credibility))
        .route("/v1/judge", post(routes::modality::judge))
        // Image generation — cost/quality-routed text-to-image
        .route("/v1/images/generate", post(routes::images::generate))
        // Custom-tier training
        .route("/v1/train", post(routes::train::submit_train))
        .route(
            "/v1/adapters/:id/activate",
            post(routes::train::activate_adapter),
        )
        // Live ingest (WHIP / meeting-bot) — WebSocket upgrade
        .route("/v1/stream", get(routes::stream::stream_upgrade))
        // Innermost layer runs first per request — so rate-limit sits
        // closest to the handler, auth wraps it.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    // Health probe stays open.
    let health_only = Router::new().route("/v1/health", get(routes::health::health));

    Router::new()
        .merge(authed)
        .merge(health_only)
        // Outermost layers — order intentionally listed CORS → trace →
        // body-limit so the trace span sees the post-CORS request.
        .layer(CorsLayer::permissive())
        // Custom on_failure: 501 is documented behavior (M5-M10 endpoints are
        // intentional stubs); logging it as ERROR drowns the dev log in noise.
        // Demote 501 to DEBUG and let real 5xx still surface as ERROR.
        .layer(
            TraceLayer::new_for_http().on_failure(
                |error: ServerErrorsFailureClass, latency: std::time::Duration, _span: &tracing::Span| {
                    match error {
                        ServerErrorsFailureClass::StatusCode(code)
                            if code == axum::http::StatusCode::NOT_IMPLEMENTED =>
                        {
                            tracing::debug!(?latency, "501 Not Implemented (documented stub)");
                        }
                        _ => {
                            tracing::error!(error = ?error, ?latency, "response failed");
                        }
                    }
                },
            ),
        )
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BYTES))
        .with_state(state)
}
