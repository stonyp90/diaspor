//! End-to-end route-table test for `diaspor-api-server`.
//!
//! Calls [`build_router`] and exercises every route via
//! [`tower::ServiceExt::oneshot`]. Locks the following contract:
//!
//! 1. `GET  /v1/health` is real **and** is reachable without auth.
//! 2. Every other route requires a valid HS256 JWT in the
//!    `Authorization: Bearer …` header. Missing / malformed / expired /
//!    wrong-audience tokens all return `401 Unauthorized`.
//! 3. A valid token lets the request through to the (still-stubbed)
//!    handlers, which return `501 Not Implemented` — except for the
//!    credibility endpoint, which refuses tokens whose attested vertical
//!    is on the closed refusal list at `403` *before* the model is invoked.
//! 4. The per-key rate-limit and per-day-cap middleware return `429 Too
//!    Many Requests` with a `Retry-After` header once their respective
//!    budgets are exhausted.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use diaspor_api_server::auth::VerticalAttestation;
use diaspor_api_server::build_router;
use serde_json::Value;
use tower::ServiceExt;

// `pub(super)` is intentional here — the helpers live in a sub-module
// because tests should treat their construction surface as a small,
// named API rather than free functions, and the workspace's
// `unreachable_pub` rust lint would catch raw `pub` items. The clippy
// nursery lint `redundant_pub_crate` then fires because the helpers
// could be `pub`; the two lints contradict, so we silence the clippy
// half locally.
#[allow(clippy::redundant_pub_crate)]
mod test_helpers {
    //! Helpers shared across every test in this file.
    //!
    //! Wraps three concerns:
    //!
    //! 1. Build an [`AppState`] with a known secret, optionally
    //!    overriding the rate-limit / daily-cap defaults.
    //! 2. Mint an HS256 JWT for that secret with the requested vertical.
    //! 3. Build a `Bearer` auth header value from such a JWT.
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use diaspor_api_server::auth::{Claims, JWT_AUDIENCE, JWT_ISSUER, VerticalAttestation};
    use diaspor_api_server::{AppState, Config};
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

    /// The single HS256 secret every test in this file shares. Picked to
    /// be obviously non-production. Tests build their own `Config` from
    /// it via [`test_config`] / [`test_state`] without going through
    /// `std::env`, so no global state is touched.
    pub(super) const TEST_SECRET: &[u8] = b"test_secret_for_route_table_tests";

    /// Builds a `Config` whose secret matches [`TEST_SECRET`] but with
    /// production-sized rate-limit / cap defaults.
    pub(super) fn test_config() -> Config {
        Config::for_test(TEST_SECRET)
    }

    /// Builds a `Config` with explicit rate-limit / cap overrides.
    /// Used by the rate-limit and daily-cap tests.
    pub(super) fn test_config_with_limits(rate_limit_per_min: u32, daily_cap: u32) -> Config {
        Config::for_test_with_limits(TEST_SECRET, rate_limit_per_min, daily_cap)
    }

    /// Wraps `test_config` into a fresh `AppState`.
    pub(super) fn test_state() -> Arc<AppState> {
        AppState::new(test_config())
    }

    /// Wraps `test_config_with_limits` into a fresh `AppState`.
    pub(super) fn test_state_with_limits(rate_limit_per_min: u32, daily_cap: u32) -> Arc<AppState> {
        AppState::new(test_config_with_limits(rate_limit_per_min, daily_cap))
    }

    /// Mints a fresh, valid JWT for the given tenant / vertical with a
    /// 1-hour expiry from now.
    pub(super) fn valid_jwt(tenant: &str, vertical: VerticalAttestation) -> String {
        let claims = Claims::fresh(tenant, format!("key_{tenant}"), vertical);
        claims
            .encode_hs256(TEST_SECRET)
            .expect("test JWT encodes against the test secret")
    }

    /// Mints a JWT that has already expired — `exp` is 10 seconds before
    /// `iat`. Used by the `expired_jwt_returns_401` test.
    pub(super) fn expired_jwt() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let claims = Claims {
            tenant_id: "acme".to_string(),
            api_key_id: "key_acme".to_string(),
            vertical: VerticalAttestation::Coaching,
            iss: JWT_ISSUER.to_string(),
            aud: JWT_AUDIENCE.to_string(),
            iat: now.saturating_sub(20),
            exp: now.saturating_sub(10),
        };
        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET),
        )
        .expect("expired test JWT encodes")
    }

    /// Mints a JWT whose audience is *not* [`JWT_AUDIENCE`] — used by
    /// the `wrong_audience_returns_401` test.
    pub(super) fn wrong_audience_jwt() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let claims = Claims {
            tenant_id: "acme".to_string(),
            api_key_id: "key_acme".to_string(),
            vertical: VerticalAttestation::Coaching,
            iss: JWT_ISSUER.to_string(),
            aud: "other".to_string(),
            iat: now,
            exp: now + 3600,
        };
        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET),
        )
        .expect("wrong-aud test JWT encodes")
    }

    /// Format a token as the `Authorization` header value.
    pub(super) fn bearer(token: &str) -> String {
        format!("Bearer {token}")
    }
}

/// Reads the entire response body into a `serde_json::Value`. Convenience
/// wrapper for the assertion block in each test.
async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("body collects");
    serde_json::from_slice(&bytes).expect("body is valid json")
}

// -----------------------------------------------------------------
//  /v1/health bypasses auth + rate limiting.
// -----------------------------------------------------------------

#[tokio::test]
async fn health_returns_200_with_status_ok() {
    let app = build_router(test_helpers::test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string(), "version must be a string");
}

#[tokio::test]
async fn health_works_without_auth() {
    // Explicit no-auth-header variant of `health_returns_200_with_status_ok`,
    // wired so a future regression that adds auth to /v1/health fails CI.
    let app = build_router(test_helpers::test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// -----------------------------------------------------------------
//  Auth gate — non-health routes require a valid JWT.
// -----------------------------------------------------------------

#[tokio::test]
async fn analyze_requires_auth() {
    let app = build_router(test_helpers::test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/analyze")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "no Authorization header must yield 401, not 501"
    );
    let body = body_json(resp).await;
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn analyze_with_valid_jwt_returns_501() {
    let app = build_router(test_helpers::test_state());
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Coaching);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/analyze")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "not_implemented");
    assert_eq!(body["details"]["component"], "analyze");
}

#[tokio::test]
async fn expired_jwt_returns_401() {
    let app = build_router(test_helpers::test_state());
    let token = test_helpers::expired_jwt();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/analyze")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn wrong_audience_returns_401() {
    let app = build_router(test_helpers::test_state());
    let token = test_helpers::wrong_audience_jwt();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/analyze")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// -----------------------------------------------------------------
//  Credibility gate — vertical attestation enforcement.
// -----------------------------------------------------------------

#[tokio::test]
async fn credibility_refuses_forensic_vertical_in_jwt() {
    let app = build_router(test_helpers::test_state());
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Forensic);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/credibility")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "JWT vertical=forensic must be refused at the API layer with 403"
    );
    let body = body_json(resp).await;
    assert_eq!(body["code"], "vertical_refused");
    assert_eq!(body["details"]["attested_vertical"], "forensic");
    assert_eq!(body["details"]["endpoint"], "/v1/credibility");

    let allowed = body["details"]["allowed_verticals"]
        .as_array()
        .expect("allowed_verticals is an array");
    assert!(
        allowed.iter().any(|v| v == "coaching"),
        "coaching must be in the allowed set"
    );
    assert!(
        !allowed.iter().any(|v| v == "forensic"),
        "forensic must NOT be in the allowed set"
    );
}

#[tokio::test]
async fn credibility_allows_coaching_vertical_in_jwt() {
    let app = build_router(test_helpers::test_state());
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Coaching);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/credibility")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Vertical check passes — the stub then 501s.
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "not_implemented");
    assert_eq!(body["details"]["component"], "credibility");
}

#[tokio::test]
async fn credibility_refuses_every_forbidden_vertical() {
    // Cover the whole closed refusal set in one test so a future edit
    // that accidentally widens `is_allowed_for_credibility` fails CI.
    let forbidden = [
        VerticalAttestation::Forensic,
        VerticalAttestation::Hiring,
        VerticalAttestation::Insurance,
        VerticalAttestation::LawEnforcement,
        VerticalAttestation::EuWorkplace,
        VerticalAttestation::EuEducation,
    ];
    for vertical in forbidden {
        let app = build_router(test_helpers::test_state());
        let token = test_helpers::valid_jwt("acme", vertical);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/credibility")
                    .header("Authorization", test_helpers::bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "vertical {vertical:?} must be refused with 403"
        );
        let body = body_json(resp).await;
        assert_eq!(body["code"], "vertical_refused");
    }
}

// -----------------------------------------------------------------
//  Stubbed routes — auth passes, handler 501s.
// -----------------------------------------------------------------

#[tokio::test]
async fn pose_face_prosody_judge_all_return_501() {
    let routes = [
        ("POST", "/v1/pose"),
        ("POST", "/v1/face-mesh"),
        ("POST", "/v1/prosody"),
        ("POST", "/v1/judge?discipline=diving"),
    ];
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Coaching);
    for (method, path) in routes {
        let app = build_router(test_helpers::test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("Authorization", test_helpers::bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_IMPLEMENTED,
            "{method} {path} must 501 after auth passes"
        );
        let body = body_json(resp).await;
        assert_eq!(body["code"], "not_implemented");
    }
}

#[tokio::test]
async fn train_endpoints_return_501() {
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Coaching);

    let app = build_router(test_helpers::test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/train")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);

    let app = build_router(test_helpers::test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/adapters/abc123/activate")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn analyses_get_returns_501() {
    let app = build_router(test_helpers::test_state());
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Coaching);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/analyses/job-xyz")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "not_implemented");
}

// -----------------------------------------------------------------
//  Rate limit + daily cap.
// -----------------------------------------------------------------

#[tokio::test]
async fn rate_limit_kicks_in_after_burst() {
    // Override rate limit to 5/min so the test does not need a 61-iter
    // hot loop. The middleware path is identical at any limit; the
    // budget is just smaller.
    let state = test_helpers::test_state_with_limits(5, 10_000);
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Coaching);

    // First 5 burst through; the 6th hits the bucket.
    for i in 0..5 {
        let app = build_router(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/analyze")
                    .header("Authorization", test_helpers::bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_IMPLEMENTED,
            "request #{i} within burst should reach the (still-stubbed) handler"
        );
    }

    let app = build_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/analyze")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "6th request within the burst window must be rate-limited"
    );
    let retry_after = resp
        .headers()
        .get("retry-after")
        .expect("Retry-After header must be set on 429")
        .to_str()
        .unwrap()
        .to_string();
    let retry_after: u32 = retry_after.parse().expect("Retry-After is a u32");
    assert!(retry_after >= 1, "Retry-After must be at least 1 second");

    let body = body_json(resp).await;
    assert_eq!(body["code"], "rate_limited");
    assert!(body["details"]["retry_after_seconds"].is_number());
}

#[tokio::test]
async fn daily_cap_returns_429_until_midnight() {
    // Daily cap of 3, very generous rate limit so the rate gate never
    // fires inside the test.
    let state = test_helpers::test_state_with_limits(60, 3);
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Coaching);

    for i in 0..3 {
        let app = build_router(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/analyze")
                    .header("Authorization", test_helpers::bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_IMPLEMENTED,
            "request #{i} within daily cap should reach the handler"
        );
    }

    let app = build_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/analyze")
                .header("Authorization", test_helpers::bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "4th request past the daily cap must 429"
    );
    let retry_after = resp
        .headers()
        .get("retry-after")
        .expect("Retry-After header must be set on 429")
        .to_str()
        .unwrap()
        .to_string();
    let retry_after: u32 = retry_after.parse().expect("Retry-After is a u32");
    assert!(
        (1..=86_400).contains(&retry_after),
        "Retry-After must be 1..=86400 seconds (until UTC midnight)"
    );
}

// ── /v1/images/generate ─────────────────────────────────────────────────────

#[tokio::test]
async fn images_generate_requires_auth() {
    // Unauthenticated → 401
    let app = build_router(test_helpers::test_state());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generate")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"prompt":"a banana"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn images_generate_with_valid_jwt_succeeds() {
    // A valid JWT gets through auth + rate-limit and reaches the handler, which
    // falls back to the offline local adapter (no API keys are set in the test
    // environment) and returns a 200 with a base64-encoded PNG.
    let app = build_router(test_helpers::test_state());
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Coaching);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generate")
                .header("Authorization", test_helpers::bearer(&token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"prompt":"a banana","width":64,"height":64,"policy":"cost"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_json(resp).await;
    assert!(
        body["b64_data"].is_string(),
        "response must include b64_data"
    );
    assert_eq!(body["format"], "image/png");
    assert_eq!(body["width"], 64);
    assert_eq!(body["height"], 64);
}

#[tokio::test]
async fn images_generate_bad_policy_returns_400() {
    let app = build_router(test_helpers::test_state());
    let token = test_helpers::valid_jwt("acme", VerticalAttestation::Coaching);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generate")
                .header("Authorization", test_helpers::bearer(&token))
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"prompt":"x","policy":"bogus"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_json(resp).await;
    assert_eq!(body["code"], "bad_request");
}
