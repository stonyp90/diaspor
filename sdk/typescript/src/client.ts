/**
 * REST client for the Diaspor v1 API.
 *
 * The {@link DiasporClient} class wraps the batch-analysis and per-modality
 * HTTP endpoints exposed by `api.diaspor.io`. It uses the host's native
 * `fetch` implementation (Node 20+ or any evergreen browser) — there are no
 * runtime dependencies.
 *
 * Wire endpoints (per ROADMAP.md M10):
 * - `POST /v1/analyze` — multipart file upload (or `{ s3_url, ... }`) →
 *   {@link ScoreRecord}.
 * - `GET /v1/analyses/{id}` — polling for long-running jobs.
 * - `POST /v1/pose | /v1/face-mesh | /v1/prosody | /v1/credibility |
 *   /v1/judge?discipline=…` — per-modality endpoints.
 *
 * The wire is not live at SDK v0.1.0-alpha.1 release time; calls to a wired
 * endpoint return a typed {@link NotImplementedYetError} on 501.
 */

import {
  ApiError,
  NotImplementedYetError,
  RateLimitedError,
  VerticalRefusedError,
} from "./errors.js";
import {
  parseScoreRecord,
  type ScoreRecord,
} from "./models.js";
import { VERSION } from "./version.js";

/** Default base URL for the public API. */
export const DEFAULT_BASE_URL = "https://api.diaspor.io";

/** Default request timeout in milliseconds. */
export const DEFAULT_TIMEOUT_MS = 60_000;

/**
 * Subset of standardized modality keys accepted by `/v1/analyze`.
 *
 * Forward-compatible: the wire accepts any subset and the API echoes back
 * whichever modalities were actually populated in the response.
 */
export type ModalityKey = "pose" | "face" | "prosody" | "credibility" | "judge";

/** Options accepted by the {@link DiasporClient} constructor. */
export interface DiasporClientOptions {
  /**
   * API key issued by api.diaspor.io. Sent as `Authorization: Bearer <key>`.
   *
   * Vertical attestation is bound to the key at creation time and enforced
   * server-side; see {@link VerticalRefusedError}.
   */
  readonly apiKey: string;
  /** Override the API base URL. Defaults to {@link DEFAULT_BASE_URL}. */
  readonly baseUrl?: string;
  /** Per-request timeout in milliseconds. Defaults to {@link DEFAULT_TIMEOUT_MS}. */
  readonly timeoutMs?: number;
  /**
   * Optional alternative `fetch` implementation.
   *
   * Useful in test environments and in older runtimes that need a polyfill.
   * Defaults to the global `fetch`.
   */
  readonly fetch?: typeof fetch;
  /**
   * Optional user-agent suffix appended to the SDK's own user-agent string.
   *
   * Helps downstream tools identify themselves in API logs.
   */
  readonly userAgentSuffix?: string;
}

/** Options for {@link DiasporClient.analyze}. */
export interface AnalyzeOptions {
  /** Restrict analysis to a subset of modalities. Server may downscope further. */
  readonly modalities?: readonly ModalityKey[];
  /** Optional discipline string when `judge` is included (e.g. `"diving"`). */
  readonly discipline?: string;
  /**
   * Optional per-tenant adapter override. Custom-tier only; ignored on the
   * free / starter tiers.
   */
  readonly adapterId?: string;
  /** Per-call timeout override. */
  readonly timeoutMs?: number;
  /** Per-call AbortSignal. */
  readonly signal?: AbortSignal;
}

/** Options for {@link DiasporClient.judge}. Same shape as analyze but discipline is required. */
export interface JudgeOptions extends Omit<AnalyzeOptions, "modalities" | "discipline"> {
  /** Sport discipline. Diving launches first; weightlifting / martial-arts forms follow. */
  readonly discipline: string;
}

/**
 * Anything the SDK is willing to upload as a video payload.
 *
 * `File` and `Blob` are native browser types (also present in Node 20+);
 * `Uint8Array` is the canonical Node binary buffer (a subtype of which is
 * `Buffer`, so existing Node code can pass a `Buffer` directly).
 */
export type UploadablePayload = File | Blob | Uint8Array | ArrayBuffer;

/**
 * REST client for the Diaspor v1 API.
 *
 * @example
 * ```ts
 * const client = new DiasporClient({ apiKey: process.env.DIASPOR_API_KEY! });
 * const score = await client.analyze(videoBlob, { modalities: ["pose", "judge"] });
 * ```
 */
export class DiasporClient {
  readonly #apiKey: string;
  readonly #baseUrl: string;
  readonly #timeoutMs: number;
  readonly #fetch: typeof fetch;
  readonly #userAgent: string;

  constructor(options: DiasporClientOptions) {
    if (typeof options?.apiKey !== "string" || options.apiKey.length === 0) {
      throw new TypeError("DiasporClient: apiKey is required and must be a non-empty string");
    }
    this.#apiKey = options.apiKey;
    this.#baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, "");
    this.#timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    // Use injected fetch if provided, otherwise fall back to the global
    // (bound so it does not lose its `this`).
    const fetchImpl = options.fetch ?? (typeof fetch !== "undefined" ? fetch : undefined);
    if (typeof fetchImpl !== "function") {
      throw new TypeError(
        "DiasporClient: global fetch not available; pass options.fetch (Node 20+ required)",
      );
    }
    this.#fetch = fetchImpl.bind(globalThis);
    const suffix = options.userAgentSuffix ? ` ${options.userAgentSuffix}` : "";
    this.#userAgent = `diaspor-sdk-typescript/${VERSION}${suffix}`;
  }

  /** Resolved API base URL (no trailing slash). */
  public get baseUrl(): string {
    return this.#baseUrl;
  }

  /** SDK version string. */
  // eslint-disable-next-line class-methods-use-this
  public get version(): string {
    return VERSION;
  }

  /**
   * Submit a video for full multi-modal batch analysis.
   *
   * Wraps `POST /v1/analyze` (multipart upload). For large files already in
   * S3, future revisions will accept a `{ s3_url }` JSON body on the same
   * endpoint; this high-level `analyze()` always uploads inline.
   *
   * @param file Video payload — `File`, `Blob`, `Buffer` (Node), or raw bytes.
   * @param opts Per-call options (modalities, discipline, abort).
   * @returns A fully-formed {@link ScoreRecord} for the entire video.
   * @throws {ApiError} on non-2xx HTTP responses.
   * @throws {RateLimitedError} on HTTP 429.
   * @throws {VerticalRefusedError} when credibility was requested but the
   *   key is attested to a forbidden vertical.
   * @throws {NotImplementedYetError} while the wire is not yet live (HTTP 501).
   */
  public async analyze(
    file: UploadablePayload,
    opts: AnalyzeOptions = {},
  ): Promise<ScoreRecord> {
    const form = new FormData();
    form.append("file", toBlob(file), filenameFor(file));
    if (opts.modalities && opts.modalities.length > 0) {
      form.append("modalities", opts.modalities.join(","));
    }
    if (opts.discipline) {
      form.append("discipline", opts.discipline);
    }
    if (opts.adapterId) {
      form.append("adapter_id", opts.adapterId);
    }
    const json = await this.#request("POST", "/v1/analyze", {
      body: form,
      timeoutMs: opts.timeoutMs ?? this.#timeoutMs,
      signal: opts.signal ?? null,
    });
    return parseScoreRecord(json);
  }

  /**
   * Poll a long-running analysis job by id.
   *
   * Wraps `GET /v1/analyses/{id}`. Returns the latest {@link ScoreRecord} for
   * the job, which may be partial while still processing — check
   * `record.modalities` for what has been populated so far.
   *
   * @param analysisId Job identifier returned by {@link analyze} when the API
   *   chooses to process asynchronously.
   */
  public async poll(analysisId: string, opts: { signal?: AbortSignal } = {}): Promise<ScoreRecord> {
    if (!analysisId) {
      throw new TypeError("DiasporClient.poll: analysisId is required");
    }
    const json = await this.#request("GET", `/v1/analyses/${encodeURIComponent(analysisId)}`, {
      timeoutMs: this.#timeoutMs,
      signal: opts.signal ?? null,
    });
    return parseScoreRecord(json);
  }

  /**
   * Run only the 3D body-pose modality.
   *
   * Wraps `POST /v1/pose`. Returns a {@link ScoreRecord} whose `modalities`
   * field carries only `pose` (and optional `model_provenance`).
   */
  public pose(file: UploadablePayload, opts: AnalyzeOptions = {}): Promise<ScoreRecord> {
    return this.#singleModality("/v1/pose", file, opts);
  }

  /**
   * Run only the face-mesh modality (478 landmarks, MediaPipe topology).
   *
   * Wraps `POST /v1/face-mesh`.
   */
  public faceMesh(file: UploadablePayload, opts: AnalyzeOptions = {}): Promise<ScoreRecord> {
    return this.#singleModality("/v1/face-mesh", file, opts);
  }

  /**
   * Run only the prosody modality (openSMILE eGeMAPSv02 + ComParE2016).
   *
   * Wraps `POST /v1/prosody`.
   */
  public prosody(file: UploadablePayload, opts: AnalyzeOptions = {}): Promise<ScoreRecord> {
    return this.#singleModality("/v1/prosody", file, opts);
  }

  /**
   * Run only the credibility modality.
   *
   * **Credibility signals are not lie detection.** They are a per-window
   * indicator of stress and incongruence, surfaced with a disclosed accuracy
   * ceiling (~0.74) and a peer-reviewed human baseline (~0.54).
   *
   * Calls from API keys attested to the `forensic`, `hiring`, `insurance`,
   * `law_enforcement`, `eu_workplace`, or `eu_education` verticals will be
   * refused server-side with a {@link VerticalRefusedError}. EU workplace
   * and education contexts are blocked under the EU AI Act (effective
   * August 2026).
   *
   * Wraps `POST /v1/credibility`.
   */
  public credibility(file: UploadablePayload, opts: AnalyzeOptions = {}): Promise<ScoreRecord> {
    return this.#singleModality("/v1/credibility", file, opts);
  }

  /**
   * Run a sport-judge model against a clip.
   *
   * Wraps `POST /v1/judge?discipline=<discipline>`. Diving launches first
   * (FINA 2025 rubric); weightlifting / martial-arts forms follow. The
   * returned {@link ScoreRecord} carries a `judge` modality with the
   * discipline-specific score on the rubric's native scale.
   */
  public async judge(file: UploadablePayload, opts: JudgeOptions): Promise<ScoreRecord> {
    if (!opts?.discipline) {
      throw new TypeError("DiasporClient.judge: opts.discipline is required");
    }
    const form = new FormData();
    form.append("file", toBlob(file), filenameFor(file));
    if (opts.adapterId) {
      form.append("adapter_id", opts.adapterId);
    }
    const json = await this.#request(
      "POST",
      `/v1/judge?discipline=${encodeURIComponent(opts.discipline)}`,
      {
        body: form,
        timeoutMs: opts.timeoutMs ?? this.#timeoutMs,
        signal: opts.signal ?? null,
      },
    );
    return parseScoreRecord(json);
  }

  // ───────────────────────────────────────────────────────────────────────
  // Internals
  // ───────────────────────────────────────────────────────────────────────

  async #singleModality(
    path: string,
    file: UploadablePayload,
    opts: AnalyzeOptions,
  ): Promise<ScoreRecord> {
    const form = new FormData();
    form.append("file", toBlob(file), filenameFor(file));
    if (opts.adapterId) {
      form.append("adapter_id", opts.adapterId);
    }
    const json = await this.#request("POST", path, {
      body: form,
      timeoutMs: opts.timeoutMs ?? this.#timeoutMs,
      signal: opts.signal ?? null,
    });
    return parseScoreRecord(json);
  }

  async #request(
    method: "GET" | "POST",
    path: string,
    args: {
      body?: BodyInit | null;
      timeoutMs: number;
      signal: AbortSignal | null;
    },
  ): Promise<unknown> {
    const url = `${this.#baseUrl}${path}`;
    const ac = new AbortController();
    const timer = setTimeout(() => ac.abort(new Error(`request timed out after ${args.timeoutMs}ms`)), args.timeoutMs);
    if (args.signal) {
      if (args.signal.aborted) {
        ac.abort(args.signal.reason);
      } else {
        args.signal.addEventListener("abort", () => ac.abort(args.signal?.reason), { once: true });
      }
    }
    const headers: Record<string, string> = {
      Authorization: `Bearer ${this.#apiKey}`,
      Accept: "application/json",
      "User-Agent": this.#userAgent,
      "X-Diaspor-Client": this.#userAgent,
    };
    let response: Response;
    try {
      response = await this.#fetch(url, {
        method,
        headers,
        body: args.body ?? null,
        signal: ac.signal,
      });
    } finally {
      clearTimeout(timer);
    }
    return await this.#parseResponse(response, path);
  }

  async #parseResponse(response: Response, path: string): Promise<unknown> {
    const requestId = response.headers.get("x-request-id");
    if (response.ok) {
      const ct = response.headers.get("content-type") ?? "";
      if (!ct.includes("application/json")) {
        // Permit empty bodies on 204 etc.; otherwise fall through to text and
        // raise an ApiError so the caller can debug.
        if (response.status === 204) {
          return null;
        }
        const text = await response.text();
        throw new ApiError(`expected JSON response, got content-type "${ct}"`, {
          status: response.status,
          body: text,
          requestId,
        });
      }
      return await response.json();
    }
    // Non-2xx: parse body best-effort, then map to the right error subclass.
    const body = await readBodyBestEffort(response);
    const msg = extractErrorMessage(body) ?? `${response.status} ${response.statusText}`;

    if (response.status === 429) {
      throw new RateLimitedError(msg, {
        status: response.status,
        body,
        requestId,
        retryAfterSec: parseRetryAfter(response.headers.get("retry-after")),
      });
    }
    if (response.status === 451 || extractErrorCode(body) === "vertical_refused") {
      throw new VerticalRefusedError(msg, {
        status: response.status,
        body,
        requestId,
        attestedVertical: extractStringField(body, "attested_vertical"),
        endpoint: path,
      });
    }
    if (response.status === 501) {
      throw new NotImplementedYetError(msg, {
        status: response.status,
        body,
        requestId,
        blockedOnMilestone: extractStringField(body, "blocked_on_milestone") ?? "M10",
      });
    }
    throw new ApiError(msg, {
      status: response.status,
      body,
      requestId,
    });
  }
}

// ─────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────

function toBlob(file: UploadablePayload): Blob {
  if (file instanceof Blob) {
    return file;
  }
  if (file instanceof Uint8Array) {
    // TS 5.7+ widened Uint8Array to be generic over ArrayBufferLike, which the
    // Blob constructor's BlobPart union doesn't accept (it wants the narrower
    // ArrayBuffer-backed variant). Cast through BlobPart since at runtime any
    // Uint8Array is a valid Blob input regardless of backing buffer.
    return new Blob([file as BlobPart], { type: "application/octet-stream" });
  }
  if (file instanceof ArrayBuffer) {
    return new Blob([file], { type: "application/octet-stream" });
  }
  throw new TypeError(
    "DiasporClient: unsupported upload payload; expected File, Blob, Uint8Array, or ArrayBuffer",
  );
}

function filenameFor(file: UploadablePayload): string {
  if (file instanceof File) {
    return file.name || "upload.bin";
  }
  return "upload.bin";
}

async function readBodyBestEffort(response: Response): Promise<unknown> {
  const ct = response.headers.get("content-type") ?? "";
  try {
    if (ct.includes("application/json")) {
      return await response.json();
    }
    return await response.text();
  } catch {
    return null;
  }
}

function extractErrorMessage(body: unknown): string | null {
  if (body && typeof body === "object" && "error" in body) {
    const err = (body as { error: unknown }).error;
    if (typeof err === "string") return err;
    if (err && typeof err === "object" && "message" in err) {
      const m = (err as { message: unknown }).message;
      if (typeof m === "string") return m;
    }
  }
  if (body && typeof body === "object" && "message" in body) {
    const m = (body as { message: unknown }).message;
    if (typeof m === "string") return m;
  }
  return null;
}

function extractErrorCode(body: unknown): string | null {
  if (body && typeof body === "object") {
    if ("code" in body) {
      const c = (body as { code: unknown }).code;
      if (typeof c === "string") return c;
    }
    if ("error" in body) {
      const err = (body as { error: unknown }).error;
      if (err && typeof err === "object" && "code" in err) {
        const c = (err as { code: unknown }).code;
        if (typeof c === "string") return c;
      }
    }
  }
  return null;
}

function extractStringField(body: unknown, field: string): string | null {
  if (body && typeof body === "object" && field in body) {
    const v = (body as Record<string, unknown>)[field];
    if (typeof v === "string") return v;
  }
  if (body && typeof body === "object" && "error" in body) {
    const err = (body as { error: unknown }).error;
    if (err && typeof err === "object" && field in err) {
      const v = (err as Record<string, unknown>)[field];
      if (typeof v === "string") return v;
    }
  }
  return null;
}

function parseRetryAfter(header: string | null): number | null {
  if (!header) return null;
  const asNumber = Number(header);
  if (Number.isFinite(asNumber)) {
    return Math.max(0, asNumber);
  }
  const asDate = Date.parse(header);
  if (Number.isFinite(asDate)) {
    return Math.max(0, Math.round((asDate - Date.now()) / 1000));
  }
  return null;
}
