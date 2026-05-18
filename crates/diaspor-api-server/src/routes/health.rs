//! `GET /v1/health` — liveness probe.
//!
//! Returns a tiny JSON document used by load balancers and uptime
//! monitors. This is the **one route that is real at v0.1.0-alpha** —
//! everything else 501s.

use axum::Json;
use serde::{Deserialize, Serialize};

/// Server version reported by `GET /v1/health`.
///
/// Hard-coded to match `workspace.package.version` so the probe response
/// is stable for monitoring tools. Updated in lock-step when the
/// workspace bumps.
pub const HEALTH_VERSION: &str = "0.1.0-alpha.1";

/// JSON body returned by `GET /v1/health`.
///
/// The two fields are guaranteed: monitoring tools and the published
/// SDKs depend on them. Additional fields may be appended in future
/// releases (serde will round-trip them as long as new fields are
/// optional on the client).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// `"ok"` when the server is serving requests.
    pub status: &'static str,
    /// Semantic version of the server binary.
    pub version: &'static str,
}

/// `GET /v1/health` handler — always returns `200 OK` with a [`HealthResponse`].
///
/// This is the canonical liveness probe. Deeper health (Postgres reachable,
/// inference pool warm, `ClickHouse` meter flushing) lives on a separate
/// `/v1/health/deep` route gated by an operator-only auth scope in M10.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: HEALTH_VERSION,
    })
}
