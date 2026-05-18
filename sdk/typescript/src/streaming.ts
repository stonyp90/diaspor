/**
 * WebSocket-based live streaming client.
 *
 * Wraps the two real-time ingest paths exposed by the Diaspor API:
 *
 * 1. **WHIP push** —
 *    `wss://api.diaspor.io/v1/stream?ingest=whip&token=…`. The caller is
 *    expected to push media via a separate WHIP HTTP request; this socket
 *    only carries the outbound score-event stream.
 * 2. **Meeting-bot wrapper** —
 *    `wss://api.diaspor.io/v1/stream?bot=meeting&platform=zoom&meeting_url=…`.
 *    The API dispatches a Recall.ai bot to the requested platform (Zoom /
 *    Meet / Teams); the same outbound event stream is delivered here.
 *
 * The {@link LiveSession} class is the only thing callers interact with. Use
 * the {@link LiveSession.whip} or {@link LiveSession.meetingBot} static
 * factories to construct one. Both produce the same shape of stream so
 * consumer code is identical regardless of ingest path.
 *
 * Two consumption modes:
 *
 * - Push: pass `onEvent` to the factory.
 * - Pull: use `for await (const event of session) { … }`.
 *
 * Both can be combined; events go to the callback *and* the iterator.
 */

import { parseScoreRecord, type ScoreRecord } from "./models.js";

/** Default base URL for the live-stream WebSocket. */
export const DEFAULT_LIVE_BASE_URL = "wss://api.diaspor.io";

/** Reasons a session may close, beyond a clean caller-initiated `close()`. */
export type LiveCloseReason =
  | "client_close"
  | "server_close"
  | "transport_error"
  | "auth_failed"
  | "vertical_refused";

/**
 * Event delivered by an active live session.
 *
 * Two shapes:
 * - `kind: "score"` carries a full {@link ScoreRecord} for one window or
 *   threshold-crossing event. The wire calls this `"window"` or `"event"`
 *   on the score record's own `kind` field; this outer envelope flattens
 *   both into a single `"score"` event for ergonomic iteration.
 * - `kind: "control"` carries non-score control messages (lifecycle events,
 *   bot status, server-side warnings).
 */
export type IngestEvent =
  | { readonly kind: "score"; readonly score: ScoreRecord }
  | {
      readonly kind: "control";
      readonly type: string;
      readonly data: Readonly<Record<string, unknown>>;
    };

/** Supported meeting-bot providers. */
export type MeetingBotProvider = "recall_ai";

/** Supported meeting platforms when using the bot wrapper. */
export type MeetingPlatform = "zoom" | "google_meet" | "microsoft_teams";

/** Common options shared by all {@link LiveSession} factories. */
export interface LiveSessionCommonOptions {
  /** Live-API auth token (separate from the REST API key; short-lived). */
  readonly token: string;
  /** Override the live base URL. Defaults to {@link DEFAULT_LIVE_BASE_URL}. */
  readonly baseUrl?: string;
  /** Push-mode callback invoked once per inbound event. */
  readonly onEvent?: (event: IngestEvent) => void | Promise<void>;
  /** Callback invoked when the socket closes. */
  readonly onClose?: (reason: LiveCloseReason, info?: { code?: number; message?: string }) => void;
  /** Optional alternative WebSocket constructor (for tests / older runtimes). */
  readonly webSocket?: typeof WebSocket;
}

/** Options for {@link LiveSession.whip}. */
export interface WhipSessionOptions extends LiveSessionCommonOptions {
  /**
   * Modalities to compute on the stream.
   *
   * Defaults to whatever the server chooses based on key vertical and
   * subscription tier.
   */
  readonly modalities?: readonly ("pose" | "face" | "prosody" | "credibility" | "judge")[];
  /** Optional discipline for sport-judge use cases (e.g. `"diving"`). */
  readonly discipline?: string;
}

/** Options for {@link LiveSession.meetingBot}. */
export interface MeetingBotSessionOptions extends LiveSessionCommonOptions {
  /** Bot provider. Only `"recall_ai"` is supported today; covers Zoom/Meet/Teams. */
  readonly provider: MeetingBotProvider;
  /** Target meeting platform. */
  readonly platform: MeetingPlatform;
  /** Full meeting URL the bot should join. */
  readonly meetingUrl: string;
  /**
   * Plain-text consent script the bot will read when it joins.
   *
   * Required for compliance with the "all-party consent" TOS clause from
   * ROADMAP.md M8.
   */
  readonly consentScript: string;
  /** Optional display name for the bot in the meeting. */
  readonly botName?: string;
  /** Optional modality subset (same semantics as {@link WhipSessionOptions.modalities}). */
  readonly modalities?: readonly ("pose" | "face" | "prosody" | "credibility" | "judge")[];
}

/**
 * Active live session.
 *
 * Construct via {@link LiveSession.whip} or {@link LiveSession.meetingBot}.
 * Once awaited (factory returns), the underlying WebSocket is connected and
 * the session is delivering events.
 */
export class LiveSession implements AsyncIterable<IngestEvent> {
  readonly #ws: WebSocket;
  readonly #onEvent?: (event: IngestEvent) => void | Promise<void>;
  readonly #onClose?: (reason: LiveCloseReason, info?: { code?: number; message?: string }) => void;

  // Pull-mode buffer. Events arriving before the iterator has resolved a
  // pending `next()` queue up here; pending consumers queue up in
  // `#pendingResolvers` when there is nothing to deliver yet.
  readonly #buffer: IngestEvent[] = [];
  readonly #pendingResolvers: Array<(value: IteratorResult<IngestEvent>) => void> = [];
  #closed = false;
  #closeReason: LiveCloseReason | null = null;

  private constructor(
    ws: WebSocket,
    opts: { onEvent?: LiveSessionCommonOptions["onEvent"]; onClose?: LiveSessionCommonOptions["onClose"] },
  ) {
    this.#ws = ws;
    this.#onEvent = opts.onEvent;
    this.#onClose = opts.onClose;
    this.#wireHandlers();
  }

  /**
   * Open a WHIP-ingest live session.
   *
   * Equivalent to `wss://api.diaspor.io/v1/stream?ingest=whip&token=…`. The
   * caller is responsible for performing the WHIP HTTP push separately
   * (typically via a WebRTC stack); this socket carries only the outbound
   * score events.
   */
  public static async whip(opts: WhipSessionOptions): Promise<LiveSession> {
    const url = buildLiveUrl(opts.baseUrl ?? DEFAULT_LIVE_BASE_URL, {
      ingest: "whip",
      token: opts.token,
      modalities: opts.modalities?.join(","),
      discipline: opts.discipline,
    });
    return await LiveSession.#connect(url, opts);
  }

  /**
   * Open a meeting-bot live session.
   *
   * Dispatches a Recall.ai bot to the requested platform (Zoom / Meet /
   * Teams). The bot reads the supplied `consentScript` when it joins,
   * satisfying the all-party-consent TOS clause.
   */
  public static async meetingBot(opts: MeetingBotSessionOptions): Promise<LiveSession> {
    const url = buildLiveUrl(opts.baseUrl ?? DEFAULT_LIVE_BASE_URL, {
      bot: "meeting",
      provider: opts.provider,
      platform: opts.platform,
      meeting_url: opts.meetingUrl,
      consent_script: opts.consentScript,
      bot_name: opts.botName,
      modalities: opts.modalities?.join(","),
      token: opts.token,
    });
    return await LiveSession.#connect(url, opts);
  }

  /** `true` once the session has closed for any reason. */
  public get isClosed(): boolean {
    return this.#closed;
  }

  /** Reason the session closed, if known. */
  public get closeReason(): LiveCloseReason | null {
    return this.#closeReason;
  }

  /**
   * Close the session cleanly.
   *
   * For meeting-bot sessions, this also removes the bot from the meeting.
   * Idempotent — safe to call more than once.
   */
  public async close(code = 1000, reason = "client_close"): Promise<void> {
    if (this.#closed) return;
    try {
      this.#ws.close(code, reason);
    } catch {
      // ignored — socket may already be gone
    }
    this.#handleClose("client_close", { code, message: reason });
  }

  /**
   * Async iterator over inbound events.
   *
   * Yields every event the server pushes until the session closes. After
   * close the iterator terminates cleanly (returns `done: true`).
   */
  public [Symbol.asyncIterator](): AsyncIterator<IngestEvent> {
    return {
      next: (): Promise<IteratorResult<IngestEvent>> => {
        const buffered = this.#buffer.shift();
        if (buffered !== undefined) {
          return Promise.resolve({ value: buffered, done: false });
        }
        if (this.#closed) {
          return Promise.resolve({ value: undefined, done: true });
        }
        return new Promise<IteratorResult<IngestEvent>>((resolve) => {
          this.#pendingResolvers.push(resolve);
        });
      },
      return: (): Promise<IteratorResult<IngestEvent>> => {
        // Iterator was abandoned (break / early return). Close the socket so
        // we don't leak the connection.
        void this.close();
        return Promise.resolve({ value: undefined, done: true });
      },
    };
  }

  // ───────────────────────────────────────────────────────────────────────
  // Internals
  // ───────────────────────────────────────────────────────────────────────

  static async #connect(
    url: string,
    opts: LiveSessionCommonOptions,
  ): Promise<LiveSession> {
    const Ctor = opts.webSocket ?? (typeof WebSocket !== "undefined" ? WebSocket : undefined);
    if (typeof Ctor !== "function") {
      throw new TypeError(
        "LiveSession: global WebSocket not available; pass options.webSocket (Node 20+ required)",
      );
    }
    const ws = new Ctor(url);
    await new Promise<void>((resolve, reject) => {
      const onOpen = () => {
        ws.removeEventListener("open", onOpen as EventListener);
        ws.removeEventListener("error", onError as EventListener);
        resolve();
      };
      const onError = (ev: Event) => {
        ws.removeEventListener("open", onOpen as EventListener);
        ws.removeEventListener("error", onError as EventListener);
        const msg = "message" in ev ? String((ev as unknown as { message: unknown }).message) : "websocket error";
        reject(new Error(`LiveSession: failed to connect: ${msg}`));
      };
      ws.addEventListener("open", onOpen as EventListener);
      ws.addEventListener("error", onError as EventListener);
    });
    return new LiveSession(ws, { onEvent: opts.onEvent, onClose: opts.onClose });
  }

  #wireHandlers(): void {
    this.#ws.addEventListener("message", (ev: MessageEvent) => {
      this.#handleMessage(ev.data);
    });
    this.#ws.addEventListener("close", (ev: CloseEvent) => {
      this.#handleClose(ev.code === 1000 ? "client_close" : "server_close", {
        code: ev.code,
        message: ev.reason,
      });
    });
    this.#ws.addEventListener("error", () => {
      this.#handleClose("transport_error");
    });
  }

  #handleMessage(data: unknown): void {
    let parsed: unknown;
    try {
      if (typeof data === "string") {
        parsed = JSON.parse(data);
      } else if (data instanceof ArrayBuffer) {
        parsed = JSON.parse(new TextDecoder().decode(data));
      } else if (ArrayBuffer.isView(data)) {
        parsed = JSON.parse(new TextDecoder().decode(data as ArrayBufferView));
      } else if (data && typeof data === "object" && "text" in data && typeof (data as { text: () => Promise<string> }).text === "function") {
        // Blob (browser): decode async and re-enter. We swallow the promise
        // intentionally because the caller is the WebSocket message loop.
        void (data as Blob).text().then((t) => this.#handleMessage(t));
        return;
      } else {
        return;
      }
    } catch {
      // Malformed JSON on the wire — surface as a control event so the
      // consumer can log and decide what to do, rather than killing the
      // session.
      this.#dispatch({
        kind: "control",
        type: "malformed_message",
        data: { raw_type: typeof data },
      });
      return;
    }
    if (parsed && typeof parsed === "object" && "type" in parsed) {
      const evt = parsed as { type: unknown };
      if (evt.type === "score" && "score" in parsed) {
        try {
          const score = parseScoreRecord((parsed as { score: unknown }).score);
          this.#dispatch({ kind: "score", score });
          return;
        } catch (err) {
          this.#dispatch({
            kind: "control",
            type: "schema_error",
            data: { message: err instanceof Error ? err.message : String(err) },
          });
          return;
        }
      }
      const type = typeof evt.type === "string" ? evt.type : "unknown";
      const dataField = (parsed as { data?: unknown }).data;
      const data = isPlainObject(dataField) ? dataField : {};
      this.#dispatch({ kind: "control", type, data });
      return;
    }
    // Bare score record (no envelope) — try parsing directly.
    try {
      const score = parseScoreRecord(parsed);
      this.#dispatch({ kind: "score", score });
    } catch {
      this.#dispatch({
        kind: "control",
        type: "unrecognized_message",
        data: { keys: parsed && typeof parsed === "object" ? Object.keys(parsed) : [] },
      });
    }
  }

  #dispatch(event: IngestEvent): void {
    // Push-mode callback.
    if (this.#onEvent) {
      try {
        const r = this.#onEvent(event);
        if (r && typeof (r as Promise<void>).catch === "function") {
          (r as Promise<void>).catch(() => undefined);
        }
      } catch {
        // User callback errors should not kill the session.
      }
    }
    // Pull-mode delivery.
    const pending = this.#pendingResolvers.shift();
    if (pending) {
      pending({ value: event, done: false });
    } else {
      this.#buffer.push(event);
    }
  }

  #handleClose(reason: LiveCloseReason, info?: { code?: number; message?: string }): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#closeReason = reason;
    // Drain pending iterator consumers with a clean done signal.
    while (this.#pendingResolvers.length > 0) {
      const r = this.#pendingResolvers.shift();
      r?.({ value: undefined, done: true });
    }
    try {
      this.#onClose?.(reason, info);
    } catch {
      // Swallowed — user handler errors should not propagate.
    }
  }
}

// ─────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────

function buildLiveUrl(baseUrl: string, params: Record<string, string | undefined>): string {
  const trimmed = baseUrl.replace(/\/+$/, "");
  const qs = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== "") {
      qs.set(key, value);
    }
  }
  return `${trimmed}/v1/stream?${qs.toString()}`;
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}
