//! `POST /v1/analyze` and `GET /v1/analyses/:id` — batch analysis.
//!
//! `POST /v1/analyze` accepts either a multipart file upload or a JSON
//! body carrying a `signed_s3_url`. Short jobs complete inline and return
//! `200 OK` with a `ScoreRecord` body (see `docs/schema/score-v1.json`).
//! Longer jobs return `202 Accepted` with a `Location` header pointing
//! at `/v1/analyses/:id` for polling.
//!
//! At v0.1.0-alpha.1 both handlers are stubs that return `501 Not
//! Implemented` — but they now authenticate the caller first via the
//! [`crate::auth::ApiKey`] extractor. The real handlers compose
//! `diaspor-vision` + `diaspor-infer` + `diaspor-events` and ship in M10.

use axum::extract::Path;
use axum::response::IntoResponse;

use crate::auth::ApiKey;
use crate::error::ApiError;

/// `POST /v1/analyze` — submit a media stream for multi-modal analysis.
///
/// In production: parses either the multipart body or the
/// `signed_s3_url` JSON envelope, allocates a `stream_id`, dispatches
/// the job to a `diaspor-infer` worker, and either inlines the result
/// or returns `202 Accepted` with a polling location.
///
/// At v0.1.0-alpha.1: authenticates the caller, then returns
/// `501 Not Implemented`.
pub async fn analyze(_key: ApiKey) -> Result<impl IntoResponse, ApiError> {
    Err::<axum::Json<()>, _>(ApiError::not_implemented("analyze"))
}

/// `GET /v1/analyses/:id` — poll the status of a previously-submitted job.
///
/// In production: looks up the job by `id` against the analyses table,
/// returns either `200 OK` with the finished `ScoreRecord`, `202 Accepted`
/// with progress, or `404 Not Found` if the id is unknown to this tenant.
///
/// At v0.1.0-alpha.1: authenticates the caller, then returns
/// `501 Not Implemented`.
pub async fn get_analysis_status(
    _key: ApiKey,
    Path(_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    Err::<axum::Json<()>, _>(ApiError::not_implemented("analyses_get"))
}
