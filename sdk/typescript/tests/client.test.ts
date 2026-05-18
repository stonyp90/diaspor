/**
 * Vitest unit tests for the @diaspor/sdk public surface.
 *
 * Covers:
 *
 * 1. Constructing a {@link DiasporClient} without throwing (happy path) and
 *    with the obvious bad inputs (throws on missing apiKey).
 * 2. Round-tripping the canonical example from `docs/schema/score-v1.json`
 *    through {@link parseScoreRecord} and back to JSON.
 * 3. Constructing every concrete error class and confirming the type
 *    hierarchy is wired correctly (every concrete class is a `DiasporError`,
 *    every API error is an `ApiError`, etc.).
 *
 * The wire is not yet live, so there are no network tests here. When the
 * server starts returning real bodies in M10, this file will grow happy-path
 * integration tests behind a `DIASPOR_API_KEY` env-gated suite.
 */

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { describe, expect, it } from "vitest";

import {
  ApiError,
  DiasporClient,
  DiasporError,
  NotImplementedYetError,
  parseScoreRecord,
  RateLimitedError,
  SchemaValidationError,
  VERSION,
  VerticalRefusedError,
  type ScoreRecord,
} from "../src/index.js";

const HERE = dirname(fileURLToPath(import.meta.url));
const SCHEMA_PATH = resolve(HERE, "../../../docs/schema/score-v1.json");

describe("VERSION", () => {
  it("is the alpha.1 string we're shipping", () => {
    expect(VERSION).toBe("0.1.0-alpha.1");
  });
});

describe("DiasporClient construction", () => {
  it("constructs without throwing when given an apiKey", () => {
    const client = new DiasporClient({ apiKey: "sk_test_diaspor_abc123" });
    expect(client).toBeInstanceOf(DiasporClient);
    expect(client.baseUrl).toBe("https://api.diaspor.io");
    expect(client.version).toBe(VERSION);
  });

  it("respects a custom baseUrl, stripping trailing slashes", () => {
    const client = new DiasporClient({
      apiKey: "sk_test_diaspor_abc123",
      baseUrl: "https://api.staging.diaspor.io/",
    });
    expect(client.baseUrl).toBe("https://api.staging.diaspor.io");
  });

  it("throws when apiKey is missing", () => {
    // @ts-expect-error — intentionally bad input
    expect(() => new DiasporClient({})).toThrow(TypeError);
  });

  it("throws when apiKey is empty", () => {
    expect(() => new DiasporClient({ apiKey: "" })).toThrow(TypeError);
  });

  it("throws when no fetch is available and none is provided", () => {
    const originalFetch = globalThis.fetch;
    delete (globalThis as unknown as { fetch?: unknown }).fetch;
    try {
      expect(() => new DiasporClient({ apiKey: "sk_test" })).toThrow(TypeError);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});

describe("parseScoreRecord", () => {
  it("accepts the canonical example from score-v1.json and round-trips it", async () => {
    const schemaText = await readFile(SCHEMA_PATH, "utf8");
    const schema = JSON.parse(schemaText) as { examples?: unknown[] };
    expect(Array.isArray(schema.examples)).toBe(true);
    expect(schema.examples?.length).toBeGreaterThan(0);

    const example = schema.examples?.[0];
    const parsed: ScoreRecord = parseScoreRecord(example);

    // Identity / required fields.
    expect(parsed.schema_version).toBe("1");
    expect(parsed.stream_id).toBe("abc123");
    expect(parsed.tenant).toBe("acme");
    expect(parsed.t_start_ms).toBe(12000);
    expect(parsed.t_end_ms).toBe(13000);
    expect(parsed.kind).toBe("window");

    // Modalities.
    expect(parsed.modalities.pose?.keypoints.length).toBe(33);
    expect(parsed.modalities.pose?.model).toBe("diaspor-pose-3d-v1");
    expect(parsed.modalities.face?.model).toBe("diaspor-face-mesh-v1");
    expect(parsed.modalities.prosody?.features_dim).toBe(6552);
    expect(parsed.modalities.credibility?.confidence_band).toBe("low");
    expect(parsed.modalities.credibility?.human_baseline_disclosed).toBe(0.54);
    expect(parsed.modalities.credibility?.ceiling_disclosed).toBe(0.74);

    // Model provenance preserved.
    expect(parsed.model_provenance?.length).toBe(2);

    // JSON round-trip is structurally identical.
    const reserialized = JSON.parse(JSON.stringify(parsed));
    expect(reserialized).toEqual(example);
  });

  it("rejects an empty object", () => {
    expect(() => parseScoreRecord({})).toThrow(SchemaValidationError);
  });

  it("rejects an unknown schema_version", () => {
    expect(() =>
      parseScoreRecord({
        schema_version: "2",
        stream_id: "x",
        tenant: "y",
        t_start_ms: 0,
        t_end_ms: 1,
        modalities: { pose: { model: "m", keypoints: new Array(33).fill({ x: 0, y: 0, z: 0, visibility: 1 }) } },
        extracted_at: "2026-05-15T12:30:13Z",
      }),
    ).toThrow(SchemaValidationError);
  });

  it("rejects a pose modality with the wrong keypoint count", () => {
    expect(() =>
      parseScoreRecord({
        schema_version: "1",
        stream_id: "x",
        tenant: "y",
        t_start_ms: 0,
        t_end_ms: 1,
        modalities: { pose: { model: "m", keypoints: [{ x: 0, y: 0, z: 0, visibility: 1 }] } },
        extracted_at: "2026-05-15T12:30:13Z",
      }),
    ).toThrow(/33 keypoints/);
  });

  it("rejects an extracted_at without a timezone", () => {
    expect(() =>
      parseScoreRecord({
        schema_version: "1",
        stream_id: "x",
        tenant: "y",
        t_start_ms: 0,
        t_end_ms: 1,
        modalities: { pose: { model: "m", keypoints: new Array(33).fill({ x: 0, y: 0, z: 0, visibility: 1 }) } },
        extracted_at: "2026-05-15T12:30:13",
      }),
    ).toThrow(/timezone offset/);
  });

  it("rejects t_end_ms <= t_start_ms", () => {
    expect(() =>
      parseScoreRecord({
        schema_version: "1",
        stream_id: "x",
        tenant: "y",
        t_start_ms: 1000,
        t_end_ms: 1000,
        modalities: { pose: { model: "m", keypoints: new Array(33).fill({ x: 0, y: 0, z: 0, visibility: 1 }) } },
        extracted_at: "2026-05-15T12:30:13Z",
      }),
    ).toThrow(/t_start_ms/);
  });

  it("rejects a credibility score outside [0, 1]", () => {
    expect(() =>
      parseScoreRecord({
        schema_version: "1",
        stream_id: "x",
        tenant: "y",
        t_start_ms: 0,
        t_end_ms: 1,
        modalities: {
          credibility: {
            model: "m",
            score: 1.5,
            confidence_band: "low",
            human_baseline_disclosed: 0.54,
            ceiling_disclosed: 0.74,
          },
        },
        extracted_at: "2026-05-15T12:30:13Z",
      }),
    ).toThrow(/\[0, 1\]/);
  });
});

describe("error class hierarchy", () => {
  it("ApiError extends DiasporError and carries status + body", () => {
    const err = new ApiError("boom", { status: 500, body: { error: "boom" }, requestId: "req_1" });
    expect(err).toBeInstanceOf(DiasporError);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toBeInstanceOf(Error);
    expect(err.name).toBe("ApiError");
    expect(err.code).toBe("api_error");
    expect(err.status).toBe(500);
    expect(err.body).toEqual({ error: "boom" });
    expect(err.requestId).toBe("req_1");
  });

  it("RateLimitedError extends ApiError and carries retryAfterSec", () => {
    const err = new RateLimitedError("slow down", {
      status: 429,
      body: { error: "rate_limited" },
      retryAfterSec: 30,
    });
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toBeInstanceOf(DiasporError);
    expect(err.code).toBe("rate_limited");
    expect(err.status).toBe(429);
    expect(err.retryAfterSec).toBe(30);
  });

  it("VerticalRefusedError extends ApiError and carries vertical + endpoint", () => {
    const err = new VerticalRefusedError("vertical not permitted", {
      status: 451,
      body: { error: { code: "vertical_refused", attested_vertical: "hiring" } },
      attestedVertical: "hiring",
      endpoint: "/v1/credibility",
    });
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toBeInstanceOf(DiasporError);
    expect(err.code).toBe("vertical_refused");
    expect(err.attestedVertical).toBe("hiring");
    expect(err.endpoint).toBe("/v1/credibility");
  });

  it("NotImplementedYetError extends ApiError and carries the blocking milestone", () => {
    const err = new NotImplementedYetError("not wired yet", {
      status: 501,
      body: { error: "not_implemented", blocked_on_milestone: "M10" },
      blockedOnMilestone: "M10",
    });
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toBeInstanceOf(DiasporError);
    expect(err.code).toBe("not_implemented_yet");
    expect(err.blockedOnMilestone).toBe("M10");
  });

  it("SchemaValidationError extends DiasporError but not ApiError", () => {
    const err = new SchemaValidationError("bad shape", "/modalities/pose");
    expect(err).toBeInstanceOf(DiasporError);
    expect(err).not.toBeInstanceOf(ApiError);
    expect(err.code).toBe("schema_validation");
    expect(err.path).toBe("/modalities/pose");
    expect(err.message).toContain("/modalities/pose");
  });
});
