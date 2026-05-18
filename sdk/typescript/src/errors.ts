/**
 * Error hierarchy raised by the Diaspor TypeScript SDK.
 *
 * All errors thrown by {@link DiasporClient} and {@link LiveSession} extend
 * {@link DiasporError}, so callers can catch the entire family with one
 * `instanceof` check.
 *
 * The concrete subclasses (`ApiError`, `RateLimitedError`,
 * `VerticalRefusedError`, `NotImplementedYetError`) carry the structured
 * fields needed to write recovery logic — retry-after seconds, the offending
 * HTTP status, the parsed response body, the API-key vertical that was
 * refused, and so on.
 */

/**
 * Abstract base class for every error raised by the SDK.
 *
 * Catch this if you want a single handler for all SDK failures regardless of
 * whether they came from the REST API, the WebSocket stream, or local input
 * validation.
 */
export abstract class DiasporError extends Error {
  /**
   * Stable string identifier for the error class.
   *
   * Useful for log aggregation: the `name` field is human-readable, but
   * `code` is contractually stable across SDK versions even if we rename the
   * class itself.
   */
  public abstract readonly code: string;

  protected constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = new.target.name;
    // Restore prototype chain when transpiled to ES5-style targets.
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/**
 * Thrown when the API returns a non-2xx HTTP response that the SDK does not
 * recognize as one of the specialized cases below.
 *
 * Carries the HTTP status, the raw response body (best-effort decoded as
 * JSON, falling back to text), and any `request_id` the API surfaced in the
 * `X-Request-Id` header. Always include the request id when filing a bug —
 * it lets us correlate against gateway and inference logs.
 */
export class ApiError extends DiasporError {
  // Typed as `string` (not a literal) so subclasses can narrow it to their
  // own discriminant — e.g. `"vertical_refused"` on VerticalRefusedError.
  public override readonly code: string = "api_error";

  /** HTTP status code returned by the API. */
  public readonly status: number;

  /** Best-effort parsed response body (JSON object, string, or null). */
  public readonly body: unknown;

  /** Opaque request identifier surfaced by the API for support tickets. */
  public readonly requestId: string | null;

  constructor(
    message: string,
    args: { status: number; body: unknown; requestId?: string | null; cause?: unknown },
  ) {
    super(message, args.cause !== undefined ? { cause: args.cause } : undefined);
    this.status = args.status;
    this.body = args.body;
    this.requestId = args.requestId ?? null;
  }
}

/**
 * Thrown on `429 Too Many Requests`.
 *
 * `retryAfterSec` is parsed from the `Retry-After` response header (RFC 7231;
 * seconds or HTTP-date). Use it to back off — the API enforces per-key
 * per-day caps as the "bot-left-in-meeting-overnight" mitigation called out
 * in the M10 roadmap, so retrying immediately will keep hitting 429.
 */
export class RateLimitedError extends ApiError {
  public override readonly code = "rate_limited";

  /** Seconds the caller should wait before retrying, parsed from `Retry-After`. */
  public readonly retryAfterSec: number | null;

  constructor(
    message: string,
    args: {
      status: number;
      body: unknown;
      requestId?: string | null;
      retryAfterSec?: number | null;
      cause?: unknown;
    },
  ) {
    super(message, args);
    this.retryAfterSec = args.retryAfterSec ?? null;
  }
}

/**
 * Thrown when the API refuses a credibility-related call because the calling
 * key was attested to a forbidden vertical at key creation time.
 *
 * Forbidden verticals for credibility outputs:
 * - `forensic`
 * - `hiring`
 * - `insurance`
 * - `law_enforcement`
 * - `eu_workplace` (blocked under the EU AI Act, effective August 2026)
 * - `eu_education` (same)
 *
 * This refusal is enforced at the gateway before the model runs. If your use
 * case genuinely falls outside these categories and you're seeing this error,
 * the fix is to re-issue the API key against the correct vertical
 * attestation — not to retry.
 */
export class VerticalRefusedError extends ApiError {
  public override readonly code = "vertical_refused";

  /** The vertical attested at key-creation time that triggered the refusal. */
  public readonly attestedVertical: string | null;

  /** The endpoint path that refused the call. */
  public readonly endpoint: string | null;

  constructor(
    message: string,
    args: {
      status: number;
      body: unknown;
      requestId?: string | null;
      attestedVertical?: string | null;
      endpoint?: string | null;
      cause?: unknown;
    },
  ) {
    super(message, args);
    this.attestedVertical = args.attestedVertical ?? null;
    this.endpoint = args.endpoint ?? null;
  }
}

/**
 * Thrown on `501 Not Implemented` while the API is still under construction.
 *
 * The Diaspor M10 SDK ships ahead of the API itself so downstream tooling can
 * develop against a stable shape; endpoints that are not yet wired return 501
 * with a stub body. Catching this lets you fall back to local development
 * data without crashing your app.
 */
export class NotImplementedYetError extends ApiError {
  public override readonly code = "not_implemented_yet";

  /** Roadmap milestone tag (e.g. "M10", "M8") this endpoint is blocked on. */
  public readonly blockedOnMilestone: string | null;

  constructor(
    message: string,
    args: {
      status: number;
      body: unknown;
      requestId?: string | null;
      blockedOnMilestone?: string | null;
      cause?: unknown;
    },
  ) {
    super(message, args);
    this.blockedOnMilestone = args.blockedOnMilestone ?? null;
  }
}

/**
 * Thrown by `parseScoreRecord()` when the input value does not match the
 * score-v1.json schema.
 *
 * Pure client-side validation error — never raised by network calls.
 */
export class SchemaValidationError extends DiasporError {
  public readonly code = "schema_validation";

  /** JSON-Pointer-ish path into the offending value (e.g. `/modalities/pose/keypoints/3`). */
  public readonly path: string;

  constructor(message: string, path: string, options?: { cause?: unknown }) {
    super(`${message} (at ${path})`, options);
    this.path = path;
  }
}
