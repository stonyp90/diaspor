//! API error envelope.
//!
//! Every endpoint maps every failure mode into one [`ApiError`] variant.
//! [`IntoResponse`] then renders the variant into a stable JSON shape that
//! every client SDK (`diaspor` Python, `@diaspor/sdk` TypeScript,
//! `diaspor-client` Rust) decodes the same way:
//!
//! ```json
//! { "code": "<machine-readable kebab-case code>",
//!   "message": "<human-readable English string>",
//!   "details": { /* per-variant extras */ } }
//! ```
//!
//! Keeping the envelope identical across all error variants is what lets
//! the SDKs share one error type instead of N.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::auth::VerticalAttestation;

/// Every failure mode the API can surface to a caller.
///
/// Variants are deliberately coarse — they map 1:1 to the HTTP status
/// codes a well-behaved client should branch on.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Requested capability is not yet implemented at this milestone.
    ///
    /// Used as the default stub response at `v0.1.0-alpha`. Maps to
    /// HTTP `501 Not Implemented`.
    #[error("not implemented: {component}")]
    NotImplemented {
        /// Name of the missing component (e.g. `"analyze"`, `"auth"`).
        component: String,
    },

    /// The API key's attested vertical is on the credibility refusal list.
    ///
    /// This is the load-bearing compliance gate around `/v1/credibility`.
    /// Maps to HTTP `403 Forbidden`.
    #[error("vertical {attested:?} is not allowed to call {endpoint}")]
    VerticalRefused {
        /// The vertical the calling key was attested under.
        attested: VerticalAttestation,
        /// The endpoint that refused the call.
        endpoint: String,
    },

    /// Caller exceeded their per-key / per-day quota or burst limit.
    ///
    /// Maps to HTTP `429 Too Many Requests` with a `Retry-After` header
    /// hint encoded in the body.
    #[error("rate limited; retry after {retry_after_seconds}s")]
    RateLimited {
        /// Seconds the caller should wait before retrying.
        retry_after_seconds: u32,
    },

    /// Request was malformed in a way the API can name.
    ///
    /// Maps to HTTP `400 Bad Request`.
    #[error("bad request: {0}")]
    BadRequest(String),

    /// Caller did not present a usable API key.
    ///
    /// Maps to HTTP `401 Unauthorized`.
    #[error("unauthorized")]
    Unauthorized,

    /// Server-side failure the API doesn't want to name in detail.
    ///
    /// Maps to HTTP `500 Internal Server Error`. The `String` is a
    /// short opaque tag — full context goes to logs, not to the client.
    #[error("internal error: {0}")]
    Internal(String),
}

impl ApiError {
    /// Constructs a [`ApiError::NotImplemented`] for `component`.
    #[must_use]
    pub fn not_implemented(component: impl Into<String>) -> Self {
        Self::NotImplemented {
            component: component.into(),
        }
    }

    /// Returns the HTTP status code this error maps to.
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::NotImplemented { .. } => StatusCode::NOT_IMPLEMENTED,
            Self::VerticalRefused { .. } => StatusCode::FORBIDDEN,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Returns the machine-readable kebab-case code for this error.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotImplemented { .. } => "not_implemented",
            Self::VerticalRefused { .. } => "vertical_refused",
            Self::RateLimited { .. } => "rate_limited",
            Self::BadRequest(_) => "bad_request",
            Self::Unauthorized => "unauthorized",
            Self::Internal(_) => "internal",
        }
    }
}

/// Wire shape of every error body the API emits.
///
/// Mirrors what the published client SDKs (`diaspor` Python,
/// `@diaspor/sdk` TypeScript, `diaspor-client` Rust) deserialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    /// Stable machine-readable code, e.g. `"not_implemented"`.
    pub code: String,
    /// Human-readable English message.
    pub message: String,
    /// Per-variant structured details (always an object, never `null`).
    pub details: serde_json::Value,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.code().to_string();
        let message = self.to_string();

        // Per-variant detail payload. Keep keys snake_case to match the
        // SDK contract.
        let details = match &self {
            Self::NotImplemented { component } => serde_json::json!({
                "component": component,
            }),
            Self::VerticalRefused { attested, endpoint } => serde_json::json!({
                "attested_vertical": attested,
                "endpoint": endpoint,
                "allowed_verticals": VerticalAttestation::credibility_allowed_set(),
            }),
            Self::RateLimited {
                retry_after_seconds,
            } => serde_json::json!({
                "retry_after_seconds": retry_after_seconds,
            }),
            Self::BadRequest(reason) => serde_json::json!({
                "reason": reason,
            }),
            Self::Unauthorized => serde_json::json!({}),
            Self::Internal(tag) => serde_json::json!({
                "tag": tag,
            }),
        };

        let body = ApiErrorBody {
            code,
            message,
            details,
        };

        (status, Json(body)).into_response()
    }
}
