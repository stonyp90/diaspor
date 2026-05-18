//! Per-modality endpoints — `/v1/pose`, `/v1/face-mesh`, `/v1/prosody`,
//! `/v1/credibility`, `/v1/judge`.
//!
//! Each returns the corresponding `Modality` slice of a `ScoreRecord`
//! when implemented. `credibility` is the **gated** endpoint: it consults
//! the JWT-derived [`ApiKey::vertical`] and refuses with `403` before any
//! model is invoked if the attested vertical is on the closed refusal
//! list.
//!
//! At v0.1.0-alpha.1 every handler that passes its precondition still
//! returns `501 Not Implemented` — the real backend wires in M10. The
//! vertical-refusal path is real and is exercised by
//! `tests/route_table.rs`.

use axum::extract::Query;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::auth::{ApiKey, VerticalAttestation};
use crate::error::ApiError;

/// Query parameters accepted by `POST /v1/judge`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeQuery {
    /// Sport discipline the score applies to (`"diving"`,
    /// `"weightlifting"`, `"gymnastics_floor"`, …). Required.
    pub discipline: String,
}

/// `POST /v1/pose` — body pose estimation only.
///
/// In production: 33-keypoint `BlazePose` 3D output.
/// At v0.1.0-alpha.1: `501 Not Implemented` (authenticated).
pub async fn pose(_key: ApiKey) -> Result<impl IntoResponse, ApiError> {
    Err::<axum::Json<()>, _>(ApiError::not_implemented("pose"))
}

/// `POST /v1/face-mesh` — facial landmarks + microexpressions only.
///
/// In production: 478-landmark `FaceMesh` output + optional AU intensities.
/// At v0.1.0-alpha.1: `501 Not Implemented` (authenticated).
pub async fn face_mesh(_key: ApiKey) -> Result<impl IntoResponse, ApiError> {
    Err::<axum::Json<()>, _>(ApiError::not_implemented("face_mesh"))
}

/// `POST /v1/prosody` — vocal prosody features only.
///
/// In production: `openSMILE` eGeMAPSv02 + `ComParE2016` features.
/// At v0.1.0-alpha.1: `501 Not Implemented` (authenticated).
pub async fn prosody(_key: ApiKey) -> Result<impl IntoResponse, ApiError> {
    Err::<axum::Json<()>, _>(ApiError::not_implemented("prosody"))
}

/// `POST /v1/credibility` — composite credibility signal.
///
/// This endpoint is the load-bearing compliance gate. Before any model
/// is invoked, it consults the JWT-derived [`ApiKey::vertical`] and
/// refuses with [`ApiError::VerticalRefused`] (`403 Forbidden`) if the
/// attested vertical is `forensic`, `hiring`, `insurance`,
/// `law_enforcement`, `eu_workplace`, or `eu_education`.
///
/// Only after the vertical check passes does the handler attempt the
/// (stubbed) model invocation. At v0.1.0-alpha.1 that always returns
/// `501 Not Implemented`.
pub async fn credibility(key: ApiKey) -> Result<impl IntoResponse, ApiError> {
    vertical_check(&key, "/v1/credibility")?;
    Err::<axum::Json<()>, _>(ApiError::not_implemented("credibility"))
}

/// `POST /v1/judge?discipline=...` — sport-judging score.
///
/// In production: per-discipline model fine-tuned on the discipline's
/// reference rubric (FINA for diving, FIG for gymnastics, …).
/// At v0.1.0-alpha.1: `501 Not Implemented` (authenticated).
pub async fn judge(
    _key: ApiKey,
    Query(_q): Query<JudgeQuery>,
) -> Result<impl IntoResponse, ApiError> {
    Err::<axum::Json<()>, _>(ApiError::not_implemented("judge"))
}

/// Refuses the request if the caller's attested vertical (as carried in
/// the verified JWT) is not on the credibility allowed list.
///
/// As of v0.1.0-alpha.1 the vertical is always taken from the JWT claim
/// — `X-Diaspor-Vertical` is no longer consulted by the routing layer
/// (the auth middleware always runs first, and a valid JWT always
/// carries the canonical attestation).
///
/// # Errors
///
/// [`ApiError::VerticalRefused`] if the JWT vertical is on the refusal
/// list. Maps to `403 Forbidden`.
pub(crate) fn vertical_check(key: &ApiKey, endpoint: &str) -> Result<(), ApiError> {
    if key.vertical.is_allowed_for_credibility() {
        Ok(())
    } else {
        Err(ApiError::VerticalRefused {
            attested: key.vertical,
            endpoint: endpoint.to_string(),
        })
    }
}

/// Wire shape of the `403 vertical_refused` error body for clients.
///
/// Documented for SDK authors: this struct mirrors what
/// [`ApiError::VerticalRefused`] serializes into the `details` field of
/// the standard error envelope. The five strings in `allowed_verticals`
/// are the canonical allowed set; clients can show them verbatim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalRefusedError {
    /// The vertical the calling key was attested under.
    pub attested: VerticalAttestation,
    /// Endpoint that refused the call (e.g. `"/v1/credibility"`).
    pub endpoint: String,
    /// Canonical allowed-for-credibility set, as `snake_case` strings.
    pub allowed_verticals: Vec<String>,
}
