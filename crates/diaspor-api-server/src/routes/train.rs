//! `POST /v1/train` and `POST /v1/adapters/:id/activate` — custom-tier
//! `LoRA` training submission and adapter swap-in.
//!
//! Both endpoints are approval-gated in production: training jobs whose
//! attested vertical is on the refusal list fail at the API layer
//! before any compute is consumed (see ROADMAP M9 compliance section).
//!
//! At v0.1.0-alpha.1 both handlers authenticate the caller and then
//! return `501 Not Implemented`. The real handlers compose
//! `diaspor-train` and ship in M10.

use axum::extract::Path;
use axum::response::IntoResponse;

use crate::auth::ApiKey;
use crate::error::ApiError;

/// `POST /v1/train` — submit a corpus + eval config, get back an adapter id.
///
/// In production: validates the submitted corpus against the eval set,
/// enqueues a `diaspor-train` job, and returns `202 Accepted` with a
/// `{ "adapter_id": "..." }` body once the eval gate passes.
///
/// At v0.1.0-alpha.1: authenticated, then `501 Not Implemented`.
pub async fn submit_train(_key: ApiKey) -> Result<impl IntoResponse, ApiError> {
    Err::<axum::Json<()>, _>(ApiError::not_implemented("train_submit"))
}

/// `POST /v1/adapters/:id/activate` — swap an adapter into the tenant's
/// inference pool.
///
/// In production: marks the adapter as the active one for the tenant
/// and triggers a warm-load in the inference pool. Returns `204 No
/// Content` once the swap is live.
///
/// At v0.1.0-alpha.1: authenticated, then `501 Not Implemented`.
pub async fn activate_adapter(
    _key: ApiKey,
    Path(_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    Err::<axum::Json<()>, _>(ApiError::not_implemented("adapter_activate"))
}
