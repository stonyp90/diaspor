//! Axum middleware layers — auth + rate limit.
//!
//! Two functions; the order they're layered in [`crate::build_router`] is
//! **load-bearing**: auth must run first, because the rate-limit layer
//! keys on the authenticated `api_key_id`. Reversing the order is a
//! latent bug — every request would race the per-key bucket using
//! whatever `ApiKey` extension happened to be (typically `None`, which
//! the rate-limit layer treats as a 500).
//!
//! [`crate::routes::health`] is **deliberately mounted outside** this
//! stack — load balancers must be able to probe liveness without
//! presenting credentials.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::auth::extract_api_key_from_request;
use crate::rate_limit::AppState;

/// Axum middleware: decode the inbound `Authorization: Bearer <jwt>`
/// header and stash the resulting [`crate::auth::ApiKey`] into request
/// extensions for handlers / downstream layers to pick up.
///
/// On failure (no header, malformed JWT, bad `iss` / `aud`, expired)
/// returns `401 Unauthorized` with the standard `ApiErrorBody` envelope.
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let api_key = match extract_api_key_from_request(req.headers(), &state.config) {
        Ok(key) => key,
        Err(err) => return err.into_response(),
    };
    req.extensions_mut().insert(api_key);
    next.run(req).await
}
