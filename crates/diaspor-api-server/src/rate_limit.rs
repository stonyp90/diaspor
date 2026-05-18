//! Per-API-key rate limiting and per-day hard caps.
//!
//! Two independent gates, both keyed on the JWT `api_key_id` claim:
//!
//! 1. **Token-bucket rate limit**, default 60 req/min — burst protection.
//!    Refills continuously; on overflow returns `429` with a `Retry-After`
//!    header (whole seconds, rounded up).
//!
//! 2. **Per-day hard cap**, default 10 000 req/day — abuse protection,
//!    the "bot left in a meeting overnight" mitigation called out in the
//!    build plan §10. Counter resets at UTC midnight; `Retry-After` is
//!    "seconds until UTC midnight".
//!
//! Both pieces of state live in-process inside [`crate::AppState`]
//! ([`dashmap::DashMap`]). That's deliberate at v0.1.0-alpha.1 —
//! `ClickHouse`-backed counters that survive restarts ship in M10. The
//! in-memory copy is the production runtime path *plus* a redundant
//! second-of-defence even after M10.
//!
//! The middleware deliberately runs **after** auth so it can key on
//! `ApiKey::api_key_id`; mounting it on a route without auth in front is
//! a routing bug, not a request-time auth failure.

use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, Request, Response};
use axum::middleware::Next;
use axum::response::IntoResponse;
use dashmap::DashMap;
use time::{Date, OffsetDateTime, Time, UtcOffset};

use crate::auth::ApiKey;
use crate::error::ApiError;

/// Number of seconds in a day. Used to compute `Retry-After` for the
/// "your daily cap resets at UTC midnight" path.
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

/// In-memory state shared across every request that the middleware
/// stack key on the authenticated `api_key_id`.
///
/// Created by [`Self::new`] at server start and stored as
/// `Arc<AppState>`; cloning the `Arc` per request is the only allocation
/// the rate-limit layer does outside of the first-request-per-key path.
#[derive(Debug)]
pub struct AppState {
    /// Process-loaded configuration (JWT secret, default limits).
    pub config: crate::auth::Config,
    /// Per-key token bucket (rate limit).
    pub rate_buckets: DashMap<String, TokenBucket>,
    /// Per-key daily counter `(utc_date, requests_today)`.
    pub daily_counts: DashMap<String, DailyCounter>,
}

impl AppState {
    /// Builds a fresh `AppState` for the given [`crate::auth::Config`].
    #[must_use]
    pub fn new(config: crate::auth::Config) -> Arc<Self> {
        Arc::new(Self {
            config,
            rate_buckets: DashMap::new(),
            daily_counts: DashMap::new(),
        })
    }
}

/// A leaky / token-bucket rate limiter.
///
/// Holds a fractional `tokens` value that refills linearly at
/// `refill_per_sec` up to `max_tokens`. Each accepted request consumes
/// one token; if `tokens < 1.0` the request is rejected and the bucket
/// reports the seconds-until-1-token as `Retry-After`.
///
/// The math is fully stateless past `last_refill_at`, so this struct is
/// trivially testable — construct one and call [`Self::try_consume`].
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Current credit, in (possibly fractional) tokens.
    pub tokens: f64,
    /// When `tokens` was last computed.
    pub last_refill_at: Instant,
    /// Cap on `tokens` — also the burst size.
    pub max_tokens: f64,
    /// Refill rate, in tokens per second (e.g. `1.0` = 60 req/min).
    pub refill_per_sec: f64,
}

/// Outcome of a [`TokenBucket::try_consume`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketOutcome {
    /// Request is allowed; the bucket has been debited.
    Allowed,
    /// Request is denied; the bucket cannot afford it yet. Carries the
    /// seconds the caller should wait before retrying.
    RateLimited {
        /// Whole seconds, rounded up, until at least one token is
        /// available. Always `>= 1` so a `Retry-After: 0` is never sent.
        retry_after_seconds: u32,
    },
}

impl TokenBucket {
    /// Constructs a fresh bucket starting full.
    ///
    /// `requests_per_minute` is the *steady-state* throughput; the
    /// bucket's burst capacity is the same value, i.e. up to one
    /// minute's worth of requests can land in zero seconds.
    #[must_use]
    pub fn new(requests_per_minute: u32) -> Self {
        let max_tokens = f64::from(requests_per_minute);
        Self {
            tokens: max_tokens,
            last_refill_at: Instant::now(),
            max_tokens,
            refill_per_sec: max_tokens / 60.0,
        }
    }

    /// Refills the bucket up to `max_tokens` based on time since the
    /// last refill. Pure function over `(now - last_refill_at)`.
    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill_at).as_secs_f64();
        // `mul_add` is the FP-accurate form clippy nudges us to use; the
        // numbers in play are small (< 1e6 either way) so the precision
        // win is theoretical, but the lint is canonical.
        self.tokens = elapsed
            .mul_add(self.refill_per_sec, self.tokens)
            .min(self.max_tokens);
        self.last_refill_at = now;
    }

    /// Attempts to debit one token at `now`.
    ///
    /// On success the bucket has been mutated to reflect the consumption
    /// and [`BucketOutcome::Allowed`] is returned. On failure the bucket
    /// is left at its refilled state (so subsequent calls reflect the
    /// same clock) and [`BucketOutcome::RateLimited`] is returned with a
    /// realistic `Retry-After`.
    pub fn try_consume(&mut self, now: Instant) -> BucketOutcome {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            BucketOutcome::Allowed
        } else {
            // Seconds until tokens >= 1.0, rounded up to the nearest
            // whole second. Always at least 1 — otherwise we'd send
            // `Retry-After: 0` and clients would just hot-loop.
            let needed = 1.0 - self.tokens;
            let seconds_f = if self.refill_per_sec > 0.0 {
                needed / self.refill_per_sec
            } else {
                f64::from(u32::MAX)
            };
            // `clamp` keeps the value in `[1.0, u32::MAX as f64]`, so the
            // cast back to u32 is in-range and non-negative. Clippy's
            // generic warnings about FP→int conversion don't apply once
            // those bounds are explicit, but it can't prove that, so
            // silence the lint locally.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let retry_after_seconds =
                seconds_f.ceil().clamp(1.0, f64::from(u32::MAX)) as u32;
            BucketOutcome::RateLimited {
                retry_after_seconds,
            }
        }
    }
}

/// Per-API-key per-day counter.
///
/// Tracks `(date, count)` so the next request from a new UTC day finds
/// `date != today` and resets to zero rather than carrying yesterday's
/// total forward.
#[derive(Debug, Clone)]
pub struct DailyCounter {
    /// The UTC date this counter is for.
    pub date: Date,
    /// Number of requests recorded under `date`.
    pub count: u32,
}

/// Outcome of [`DailyCounter::try_increment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyOutcome {
    /// Request is allowed; the counter has been incremented.
    Allowed,
    /// Daily cap exceeded; client should retry after the UTC-midnight
    /// rollover. `retry_after_seconds` is "seconds until tomorrow".
    CapReached {
        /// Whole seconds until UTC midnight. Always `>= 1` so we never
        /// send `Retry-After: 0` at the cusp of the rollover.
        retry_after_seconds: u32,
    },
}

impl DailyCounter {
    /// Builds a counter at zero for the UTC date of `now`.
    #[must_use]
    pub const fn new(now: OffsetDateTime) -> Self {
        // `to_offset` and `date` are both `const` on `OffsetDateTime` in
        // the `time` crate we use, so this whole constructor is `const`.
        let date = now.to_offset(UtcOffset::UTC).date();
        Self { date, count: 0 }
    }

    /// Attempts to charge one request against the cap at `now`.
    ///
    /// Resets the counter when `now`'s UTC date is later than the
    /// stored `date` — the daily cap is a sliding **calendar** window,
    /// not a rolling 24-hour one.
    pub fn try_increment(&mut self, now: OffsetDateTime, cap: u32) -> DailyOutcome {
        let utc_now = now.to_offset(UtcOffset::UTC);
        let today = utc_now.date();

        if today > self.date {
            // Roll over the calendar; this is the standard reset path.
            self.date = today;
            self.count = 0;
        }

        if self.count >= cap {
            DailyOutcome::CapReached {
                retry_after_seconds: seconds_until_utc_midnight(utc_now),
            }
        } else {
            self.count = self.count.saturating_add(1);
            DailyOutcome::Allowed
        }
    }
}

/// Seconds remaining until 00:00:00 UTC of the *next* calendar day.
///
/// `now` is assumed to already be in UTC. The result is clamped to
/// `[1, SECONDS_PER_DAY]` so the client always sees a non-zero
/// `Retry-After` even at the millisecond cusp of midnight.
fn seconds_until_utc_midnight(now: OffsetDateTime) -> u32 {
    let midnight = now
        .replace_time(Time::MIDNIGHT)
        + time::Duration::days(1);
    let secs = (midnight - now).whole_seconds();
    let clamped = secs.clamp(1, SECONDS_PER_DAY);
    u32::try_from(clamped).unwrap_or(u32::MAX)
}

/// Axum middleware: token-bucket rate limit + daily cap.
///
/// Mounts **after** the auth middleware so the request already carries
/// an [`ApiKey`] extension. Runs both gates in order — the rate limit
/// first (cheaper), the daily cap second — and returns the first refusal
/// as a [`ApiError::RateLimited`] response with a `Retry-After` header.
///
/// On `429` the response body is the standard `ApiErrorBody` JSON shape
/// so SDKs can decode rate-limit and other errors with one type.
pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    // Auth middleware always inserts an `ApiKey` before this layer
    // runs. If it's missing, the router was wired wrong — fail closed
    // with a 500 so the bug surfaces in CI rather than letting an
    // unauthenticated request through to a real handler.
    let Some(api_key) = req.extensions().get::<ApiKey>().cloned() else {
        return ApiError::Internal(
            "rate_limit middleware mounted without auth middleware in front".into(),
        )
        .into_response();
    };

    let max_per_min = state.config.default_rate_limit_per_min;
    let daily_cap = state.config.default_daily_cap;

    // -- Token bucket --
    let bucket_outcome = {
        let mut entry = state
            .rate_buckets
            .entry(api_key.api_key_id.clone())
            .or_insert_with(|| TokenBucket::new(max_per_min));
        entry.try_consume(Instant::now())
    };

    if let BucketOutcome::RateLimited {
        retry_after_seconds,
    } = bucket_outcome
    {
        return rate_limited_response(retry_after_seconds);
    }

    // -- Daily cap --
    let daily_outcome = {
        let now = OffsetDateTime::now_utc();
        let mut entry = state
            .daily_counts
            .entry(api_key.api_key_id.clone())
            .or_insert_with(|| DailyCounter::new(now));
        entry.try_increment(now, daily_cap)
    };

    if let DailyOutcome::CapReached {
        retry_after_seconds,
    } = daily_outcome
    {
        return rate_limited_response(retry_after_seconds);
    }

    next.run(req).await
}

/// Builds the canonical `429 Too Many Requests` response: standard error
/// envelope in the body plus a `Retry-After: <seconds>` header.
fn rate_limited_response(retry_after_seconds: u32) -> Response<Body> {
    let mut resp = ApiError::RateLimited {
        retry_after_seconds,
    }
    .into_response();

    // Best-effort `Retry-After` header. `u32::to_string` is always
    // valid ASCII so the `unwrap_or` branch never triggers in practice.
    let header_value = HeaderValue::from_str(&retry_after_seconds.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("1"));
    resp.headers_mut()
        .insert(axum::http::header::RETRY_AFTER, header_value);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn token_bucket_allows_burst_up_to_capacity() {
        let mut bucket = TokenBucket::new(3);
        let now = Instant::now();

        // 3 immediate consumptions all succeed.
        assert_eq!(bucket.try_consume(now), BucketOutcome::Allowed);
        assert_eq!(bucket.try_consume(now), BucketOutcome::Allowed);
        assert_eq!(bucket.try_consume(now), BucketOutcome::Allowed);

        // The 4th, at the same instant, is rate-limited.
        match bucket.try_consume(now) {
            BucketOutcome::RateLimited {
                retry_after_seconds,
            } => assert!(retry_after_seconds >= 1),
            o @ BucketOutcome::Allowed => panic!("expected RateLimited, got {o:?}"),
        }
    }

    #[test]
    fn token_bucket_refills_over_time() {
        let mut bucket = TokenBucket::new(60);
        let t0 = Instant::now();

        // Drain every token at t0.
        for _ in 0..60 {
            assert_eq!(bucket.try_consume(t0), BucketOutcome::Allowed);
        }
        assert!(matches!(
            bucket.try_consume(t0),
            BucketOutcome::RateLimited { .. }
        ));

        // After 30 seconds we have 30 fresh tokens.
        let t1 = t0 + Duration::from_secs(30);
        for _ in 0..30 {
            assert_eq!(bucket.try_consume(t1), BucketOutcome::Allowed);
        }
        assert!(matches!(
            bucket.try_consume(t1),
            BucketOutcome::RateLimited { .. }
        ));
    }

    #[test]
    fn daily_counter_caps_then_resets_next_day() {
        let day1 = time::macros::datetime!(2026-05-15 12:00 UTC);
        let day2 = time::macros::datetime!(2026-05-16 00:00:01 UTC);
        let mut counter = DailyCounter::new(day1);

        for _ in 0..3 {
            assert_eq!(counter.try_increment(day1, 3), DailyOutcome::Allowed);
        }
        // Cap of 3 hit — fourth call rejected.
        match counter.try_increment(day1, 3) {
            DailyOutcome::CapReached {
                retry_after_seconds,
            } => {
                let day_secs = u32::try_from(SECONDS_PER_DAY).unwrap_or(u32::MAX);
                assert!(retry_after_seconds <= day_secs);
            }
            o @ DailyOutcome::Allowed => panic!("expected CapReached, got {o:?}"),
        }

        // After UTC midnight rollover, fresh tokens.
        assert_eq!(counter.try_increment(day2, 3), DailyOutcome::Allowed);
    }

    #[test]
    fn seconds_until_midnight_handles_late_evening() {
        let just_before = time::macros::datetime!(2026-05-15 23:59:00 UTC);
        let s = seconds_until_utc_midnight(just_before);
        assert!((1..=120).contains(&s), "expected 1..=120 secs, got {s}");
    }
}
