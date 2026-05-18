//! API-key authentication and per-key vertical attestation.
//!
//! Every caller of `api.diaspor.io` presents a signed JWT in the
//! `Authorization: Bearer <token>` header. The JWT carries a closed set of
//! claims that identify the tenant, the API-key id, and the **attested
//! vertical** — the load-bearing compliance gate around the credibility
//! endpoint:
//!
//! ```json
//! { "tenant_id":  "acme",
//!   "api_key_id": "key_01HXYZ...",
//!   "vertical":   "coaching",
//!   "iss":        "diaspor",
//!   "aud":        "api.diaspor.io",
//!   "exp":        1234567890,
//!   "iat":        1234567000 }
//! ```
//!
//! The JWT is verified with HS256 against the secret in
//! `DIASPOR_JWT_SECRET`, which is read **once at startup** via
//! [`Config::from_env`]. Missing or empty secrets panic the process at
//! boot, not per request — the API server refuses to come up at all if
//! its compliance gate has no key. Requests with no `Authorization`
//! header, a malformed JWT, an expired JWT, or wrong `iss` / `aud`
//! return `401 Unauthorized` with an empty `details` object.
//!
//! ## How handlers see the key
//!
//! The router layers an `axum::middleware::from_fn` that decodes the JWT
//! and inserts an [`ApiKey`] into `request.extensions()`. Handlers then
//! pick it up with the standard [`axum::extract::FromRequestParts`]
//! mechanism:
//!
//! ```ignore
//! async fn analyze(ApiKey { tenant_id, vertical, .. }: ApiKey) -> Result<_, ApiError> { … }
//! ```
//!
//! `GET /v1/health` bypasses this middleware entirely — load-balancer
//! probes must reach the route with no credentials.
//!
//! ## Vertical attestation is JWT-only
//!
//! At v0.1.0-alpha.0 the attested vertical lived in the
//! `X-Diaspor-Vertical` request header. At v0.1.0-alpha.1 it moves into
//! the JWT claim and the credibility-route gate consults
//! [`ApiKey::vertical`]; the header is ignored when a valid JWT is
//! present, which after this milestone is every call.
//!
//! ## What lands in M10
//!
//! - Postgres-backed key revocation list (today: trust any JWT signed
//!   with the secret).
//! - JWK-based rotation (today: a single HS256 secret).
//! - `ClickHouse`-backed usage meter (today: per-key counters live in
//!   process memory; resets on restart).

use std::env::{self, VarError};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum::http::request::Parts;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::error::ApiError;

/// Header that historically carried the caller's attested vertical.
///
/// Kept as a constant so SDKs that still send it can compile against the
/// crate, but the routing layer **no longer reads it**: the attested
/// vertical comes from [`ApiKey::vertical`] (i.e. the JWT claim) starting
/// at v0.1.0-alpha.1.
pub const VERTICAL_HEADER: &str = "x-diaspor-vertical";

/// JWT issuer the server requires.
///
/// Tokens whose `iss` claim is not exactly this string are rejected with
/// `401 Unauthorized`.
pub const JWT_ISSUER: &str = "diaspor";

/// JWT audience the server requires.
///
/// Tokens whose `aud` claim is not exactly this string are rejected with
/// `401 Unauthorized`.
pub const JWT_AUDIENCE: &str = "api.diaspor.io";

/// Environment variable holding the HS256 signing secret.
pub const ENV_JWT_SECRET: &str = "DIASPOR_JWT_SECRET";

/// Environment variable holding the default per-key rate limit
/// (requests per minute). Optional — falls back to
/// [`Config::DEFAULT_RATE_LIMIT_PER_MIN`].
pub const ENV_DEFAULT_RATE_LIMIT: &str = "DIASPOR_DEFAULT_RATE_LIMIT";

/// Environment variable holding the default per-key daily request cap.
/// Optional — falls back to [`Config::DEFAULT_DAILY_CAP`].
pub const ENV_DEFAULT_DAILY_CAP: &str = "DIASPOR_DEFAULT_DAILY_CAP";

/// Fatal startup-time configuration error.
///
/// Raised by [`Config::from_env`] when required env vars are missing or
/// unparseable. Hoisted into a real error type rather than a `panic!` so
/// the binary `main` can `?` it and emit a clean exit.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The `DIASPOR_JWT_SECRET` env var is missing.
    ///
    /// The API refuses to start without it — the compliance gate has no
    /// key otherwise.
    #[error("DIASPOR_JWT_SECRET is required but not set")]
    MissingJwtSecret,
    /// The `DIASPOR_JWT_SECRET` env var was set to the empty string.
    ///
    /// HS256 against the empty key would let anything signed by anyone
    /// through. Refuse.
    #[error("DIASPOR_JWT_SECRET is set but empty")]
    EmptyJwtSecret,
    /// A numeric override env var failed to parse.
    #[error("invalid integer in {var}: {value}")]
    InvalidInt {
        /// The env var name.
        var: &'static str,
        /// The offending value.
        value: String,
    },
}

/// Server-side configuration, read once from the process environment.
///
/// Loaded by [`Self::from_env`] before the router is built; the resulting
/// `Arc<Config>` is attached as an Axum router extension so middleware
/// can read it without an extra clone-per-request.
#[derive(Debug, Clone)]
pub struct Config {
    /// HS256 signing secret used to verify every inbound JWT.
    pub jwt_secret: Arc<Vec<u8>>,
    /// Default per-key requests-per-minute rate limit when the key
    /// itself has no override (Postgres lookup, M10).
    pub default_rate_limit_per_min: u32,
    /// Default per-key per-day hard cap when the key itself has no
    /// override (Postgres lookup, M10).
    pub default_daily_cap: u32,
}

impl Config {
    /// Default per-key rate limit when nothing else is configured.
    pub const DEFAULT_RATE_LIMIT_PER_MIN: u32 = 60;

    /// Default per-key per-day cap when nothing else is configured. The
    /// 10000/day value is the "bot left in meeting overnight" mitigation
    /// from the build plan §10 — a runaway integration is bounded.
    pub const DEFAULT_DAILY_CAP: u32 = 10_000;

    /// Loads the configuration from the process environment.
    ///
    /// # Errors
    ///
    /// - [`ConfigError::MissingJwtSecret`] if `DIASPOR_JWT_SECRET` is unset.
    /// - [`ConfigError::EmptyJwtSecret`] if the secret is the empty string.
    /// - [`ConfigError::InvalidInt`] if a rate-limit / cap override does
    ///   not parse as `u32`.
    pub fn from_env() -> Result<Self, ConfigError> {
        let jwt_secret = match env::var(ENV_JWT_SECRET) {
            Ok(s) if s.is_empty() => return Err(ConfigError::EmptyJwtSecret),
            Ok(s) => s.into_bytes(),
            Err(VarError::NotPresent) => return Err(ConfigError::MissingJwtSecret),
            Err(VarError::NotUnicode(_)) => return Err(ConfigError::EmptyJwtSecret),
        };

        let default_rate_limit_per_min =
            parse_optional_u32(ENV_DEFAULT_RATE_LIMIT, Self::DEFAULT_RATE_LIMIT_PER_MIN)?;

        let default_daily_cap = parse_optional_u32(ENV_DEFAULT_DAILY_CAP, Self::DEFAULT_DAILY_CAP)?;

        Ok(Self {
            jwt_secret: Arc::new(jwt_secret),
            default_rate_limit_per_min,
            default_daily_cap,
        })
    }

    /// Convenience constructor for unit / integration tests that builds
    /// a `Config` from an already-known secret without going through the
    /// environment.
    #[must_use]
    pub fn for_test(secret: impl Into<Vec<u8>>) -> Self {
        Self {
            jwt_secret: Arc::new(secret.into()),
            default_rate_limit_per_min: Self::DEFAULT_RATE_LIMIT_PER_MIN,
            default_daily_cap: Self::DEFAULT_DAILY_CAP,
        }
    }

    /// Same as [`Self::for_test`] but with explicit limit overrides for
    /// rate-limit / daily-cap tests that need small numbers.
    #[must_use]
    pub fn for_test_with_limits(
        secret: impl Into<Vec<u8>>,
        default_rate_limit_per_min: u32,
        default_daily_cap: u32,
    ) -> Self {
        Self {
            jwt_secret: Arc::new(secret.into()),
            default_rate_limit_per_min,
            default_daily_cap,
        }
    }
}

fn parse_optional_u32(var: &'static str, default: u32) -> Result<u32, ConfigError> {
    match env::var(var) {
        Ok(s) if s.is_empty() => Ok(default),
        Ok(s) => s
            .parse::<u32>()
            .map_err(|_| ConfigError::InvalidInt { var, value: s }),
        Err(_) => Ok(default),
    }
}

/// An authenticated API key, decoded from the inbound JWT.
///
/// Inserted into `request.extensions()` by the auth middleware. Handlers
/// pick it up via the [`FromRequestParts`] impl below — pattern-matching
/// the fields directly is the recommended style:
///
/// ```ignore
/// async fn pose(ApiKey { vertical, .. }: ApiKey) -> Result<_, ApiError> { … }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Tenant that owns this key. Free-form opaque string (today
    /// usually a slug like `"acme"`).
    pub tenant_id: String,
    /// Stable id of this key. Safe to log — the secret is the JWT
    /// signature, not the id.
    pub api_key_id: String,
    /// Vertical the customer declared at key creation. **Load-bearing**
    /// for the credibility-endpoint refusal gate.
    pub vertical: VerticalAttestation,
}

/// Shape of the JWT body the server accepts.
///
/// Field names are the JWT-claim wire form (`snake_case`). Field names
/// map 1:1 to [`ApiKey`] for the three identity claims plus the
/// standard `iss`, `aud`, `exp`, `iat`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Tenant id (custom claim).
    pub tenant_id: String,
    /// API-key id (custom claim).
    pub api_key_id: String,
    /// Vertical attestation (custom claim).
    pub vertical: VerticalAttestation,
    /// Standard JWT issuer claim — must equal [`JWT_ISSUER`].
    pub iss: String,
    /// Standard JWT audience claim — must equal [`JWT_AUDIENCE`].
    pub aud: String,
    /// Standard JWT expiry, seconds since UNIX epoch.
    pub exp: u64,
    /// Standard JWT issue-at, seconds since UNIX epoch.
    pub iat: u64,
}

impl Claims {
    /// Builds a fresh `Claims` for `tenant` / `vertical`, with a unique
    /// `api_key_id` and a 1-hour expiry from now.
    ///
    /// Used by the test helpers to mint valid JWTs without dragging
    /// every test through manual `iat` / `exp` calculation.
    #[must_use]
    pub fn fresh(
        tenant_id: impl Into<String>,
        api_key_id: impl Into<String>,
        vertical: VerticalAttestation,
    ) -> Self {
        let now = unix_now();
        Self {
            tenant_id: tenant_id.into(),
            api_key_id: api_key_id.into(),
            vertical,
            iss: JWT_ISSUER.to_string(),
            aud: JWT_AUDIENCE.to_string(),
            exp: now + 3600,
            iat: now,
        }
    }

    /// Serialises `self` as an HS256 JWT signed by `secret`.
    ///
    /// # Errors
    ///
    /// Returns the underlying `jsonwebtoken::errors::Error` if the
    /// header/key combination cannot be encoded — in practice this only
    /// fails on a malformed key, never on valid claims.
    pub fn encode_hs256(&self, secret: &[u8]) -> Result<String, jsonwebtoken::errors::Error> {
        encode(
            &Header::new(Algorithm::HS256),
            self,
            &EncodingKey::from_secret(secret),
        )
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The vertical a customer declares at API-key creation time.
///
/// The attestation is **load-bearing**: any vertical for which
/// [`Self::is_allowed_for_credibility`] returns `false` is refused from
/// `/v1/credibility` at the API layer, *before* any inference is run.
///
/// Serialized as `snake_case` to match the schema's
/// `credibility.vertical_attestation` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalAttestation {
    // ---- allowed-for-credibility set ----
    /// Performance coaching (sport, public speaking, sales reps).
    Coaching,
    /// Sport judging (diving, weightlifting, gymnastics, martial-arts forms).
    SportJudging,
    /// Interview-coaching platforms (consensual self-reflection).
    InterviewPlatform,
    /// Deposition-recording tooling (consensual, attorney-mediated).
    DepositionRecording,
    /// Academic / non-profit research with IRB approval.
    Research,

    // ---- refused-for-credibility set ----
    /// Forensic investigation. Refused at the API layer.
    Forensic,
    /// Pre-employment hiring screening. Refused at the API layer.
    Hiring,
    /// Insurance-claim adjudication. Refused at the API layer.
    Insurance,
    /// Law-enforcement / interrogation. Refused at the API layer.
    LawEnforcement,
    /// EU workplace use — blocked under the EU AI Act (Aug 2026).
    EuWorkplace,
    /// EU education use — blocked under the EU AI Act (Aug 2026).
    EuEducation,
}

impl VerticalAttestation {
    /// Returns `true` iff this vertical is permitted to call
    /// `POST /v1/credibility`.
    ///
    /// The allowed set is closed: only the first five variants
    /// ([`Self::Coaching`], [`Self::SportJudging`],
    /// [`Self::InterviewPlatform`], [`Self::DepositionRecording`],
    /// [`Self::Research`]) ever return `true`. Adding a new allowed
    /// vertical requires an ADR and an audit.
    #[must_use]
    pub const fn is_allowed_for_credibility(&self) -> bool {
        matches!(
            self,
            Self::Coaching
                | Self::SportJudging
                | Self::InterviewPlatform
                | Self::DepositionRecording
                | Self::Research,
        )
    }

    /// Returns the closed allowed-for-credibility set as static strings.
    ///
    /// Used by [`crate::error::ApiError::VerticalRefused`] so the SDK can
    /// surface a helpful "you said `forensic`, allowed are …" message.
    #[must_use]
    pub const fn credibility_allowed_set() -> &'static [&'static str] {
        &[
            "coaching",
            "sport_judging",
            "interview_platform",
            "deposition_recording",
            "research",
        ]
    }

    /// Parses a `snake_case` string into a [`VerticalAttestation`].
    ///
    /// Returns `None` if the string does not match any variant.
    #[must_use]
    pub fn from_snake_case(s: &str) -> Option<Self> {
        match s {
            "coaching" => Some(Self::Coaching),
            "sport_judging" => Some(Self::SportJudging),
            "interview_platform" => Some(Self::InterviewPlatform),
            "deposition_recording" => Some(Self::DepositionRecording),
            "research" => Some(Self::Research),
            "forensic" => Some(Self::Forensic),
            "hiring" => Some(Self::Hiring),
            "insurance" => Some(Self::Insurance),
            "law_enforcement" => Some(Self::LawEnforcement),
            "eu_workplace" => Some(Self::EuWorkplace),
            "eu_education" => Some(Self::EuEducation),
            _ => None,
        }
    }
}

/// Optional helper for code paths that still want to read the legacy
/// `X-Diaspor-Vertical` request header.
///
/// **The routing layer no longer calls this** as of v0.1.0-alpha.1 — the
/// JWT claim wins. Retained for SDK authors who want to inspect what the
/// client *sent*, e.g. for debug logging on the proxy side.
///
/// # Errors
///
/// - [`ApiError::Unauthorized`] if no vertical header is present.
/// - [`ApiError::BadRequest`] if the header value is not a known vertical.
pub fn extract_vertical_from_headers(headers: &HeaderMap) -> Result<VerticalAttestation, ApiError> {
    let raw = headers
        .get(VERTICAL_HEADER)
        .ok_or(ApiError::Unauthorized)?
        .to_str()
        .map_err(|_| ApiError::BadRequest("vertical header is not valid utf-8".into()))?;
    VerticalAttestation::from_snake_case(raw)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown vertical attestation: {raw}")))
}

/// Decodes and verifies a Bearer JWT from a header map.
///
/// On success returns the [`ApiKey`] the request should be considered
/// authenticated as. The function is the single seam between "raw HTTP"
/// and "authenticated identity" — both the production middleware and
/// the unit tests call this directly.
///
/// # Errors
///
/// - [`ApiError::Unauthorized`] if the `Authorization` header is absent,
///   malformed, or carries a token that fails to decode, has the wrong
///   `iss` / `aud`, or has an `exp` in the past.
pub fn extract_api_key_from_request(
    headers: &HeaderMap,
    config: &Config,
) -> Result<ApiKey, ApiError> {
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .ok_or(ApiError::Unauthorized)?
        .to_str()
        .map_err(|_| ApiError::Unauthorized)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(ApiError::Unauthorized)?
        .trim();

    if token.is_empty() {
        return Err(ApiError::Unauthorized);
    }

    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[JWT_ISSUER]);
    validation.set_audience(&[JWT_AUDIENCE]);
    // `exp` is already a required claim by default; validate signature
    // timestamps strictly.
    validation.leeway = 0;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(&config.jwt_secret),
        &validation,
    )
    .map_err(|_| ApiError::Unauthorized)?;

    let Claims {
        tenant_id,
        api_key_id,
        vertical,
        ..
    } = token_data.claims;

    Ok(ApiKey {
        tenant_id,
        api_key_id,
        vertical,
    })
}

/// Reads the [`ApiKey`] out of request extensions.
///
/// The auth middleware (`crate::middleware::auth`) inserts an `ApiKey`
/// into `request.extensions_mut()` after successfully decoding the JWT.
/// Handlers then pull it back out via this extractor:
///
/// ```ignore
/// async fn pose(ApiKey { vertical, .. }: ApiKey) -> Result<_, ApiError> { … }
/// ```
///
/// If a handler is mounted without the auth middleware in front of it,
/// the extractor returns `500 Internal Server Error` — that means the
/// router was built incorrectly, which is a bug, not a runtime
/// authentication failure.
#[async_trait::async_trait]
impl<S> FromRequestParts<S> for ApiKey
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Self>().cloned().ok_or_else(|| {
            ApiError::Internal("api_key extension missing — auth middleware not mounted".into())
        })
    }
}
