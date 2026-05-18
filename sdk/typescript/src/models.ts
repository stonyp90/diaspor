/**
 * TypeScript type definitions for the Diaspor score-v1 record shape.
 *
 * Mirrors the canonical JSON Schema at
 * `docs/schema/score-v1.json` (root of the diaspor repository). Every field
 * here is documented against that schema; the schema is the source of truth.
 *
 * The runtime validator {@link parseScoreRecord} performs minimal hand-rolled
 * shape checking — required fields are present and of the right primitive
 * kind. It deliberately does **not** pull in a runtime dependency like zod,
 * because the SDK contract is "no runtime deps". Full structural validation
 * is performed server-side; this validator exists to catch obvious wire
 * corruption and to give callers a typed object they can rely on.
 */

import { SchemaValidationError } from "./errors.js";

/** Schema version literal carried on every record. */
export type ScoreSchemaVersion = "1";

/** Whether a record represents a periodic aggregate or a threshold-crossing event. */
export type ScoreRecordKind = "window" | "event";

/** Confidence band attached to a credibility score. */
export type CredibilityConfidenceBand = "low" | "medium" | "high";

/**
 * Vertical attested by the tenant at API-key creation time.
 *
 * The full enum in the schema also includes the *forbidden* verticals
 * (`forensic`, `hiring`, `insurance`, `law_enforcement`, `eu_workplace`,
 * `eu_education`) — but those values are refused at the gateway before a
 * record is ever emitted with them, so they're not expressible in this type.
 */
export type CredibilityVertical =
  | "coaching"
  | "sport_judging"
  | "interview_platform"
  | "deposition_recording"
  | "research";

/** Inference backend recorded in {@link ModelProvenance}. */
export type ModelRuntime = "triton" | "coreml" | "ort-cpu" | "deepstream";

/**
 * Severity hint for downstream event consumers.
 *
 * Not part of the v1 schema's enumerated fields, but emitted by
 * `diaspor-events` event sinks alongside the score record when threshold
 * crossings fire. Re-exported here so callers can type their event handlers
 * uniformly.
 */
export type Severity = "info" | "warn" | "alert";

/**
 * One 3D keypoint, in BlazePose topology order when used inside
 * {@link PoseModality}.
 */
export interface Keypoint3d {
  /** Normalized x coordinate in [0, 1] (frame-relative). */
  readonly x: number;
  /** Normalized y coordinate in [0, 1] (frame-relative). */
  readonly y: number;
  /** Normalized depth; negative is closer to the camera. */
  readonly z: number;
  /** Visibility score in [0, 1]; lower indicates occluded or out-of-frame. */
  readonly visibility: number;
}

/**
 * 33-keypoint 3D body pose output.
 *
 * Topology matches MediaPipe BlazePose 3D, in the canonical order
 * `nose, left_eye_inner, left_eye, … right_foot_index`.
 */
export interface PoseModality {
  /** Identifier of the pose model, e.g. `"diaspor-pose-3d-v1"`. */
  readonly model: string;
  /** Exactly 33 keypoints, in BlazePose topology order. */
  readonly keypoints: readonly Keypoint3d[];
  /** Optional joint-angle measurements in degrees, keyed by joint name. */
  readonly joint_angles_deg?: Readonly<Record<string, number>>;
  /** Optional per-keypoint velocity in normalized units per second. */
  readonly velocity_mps?: readonly number[];
}

/** Gaze direction relative to head pose, in degrees. */
export interface GazeDirection {
  readonly yaw_deg: number;
  readonly pitch_deg: number;
}

/**
 * 478-landmark facial geometry output.
 *
 * Topology matches MediaPipe FaceMesh with `refine_landmarks=True`. Landmarks
 * are typically delivered as a base64 INT8 quantization to keep the JSON
 * payload small; the full-precision floats are available via the binary
 * sidecar (out of scope for this SDK).
 */
export interface FaceModality {
  /** Identifier of the face-mesh model. */
  readonly model: string;
  /**
   * Base64-encoded INT8 quantization of the 478 (x, y, z) triples.
   *
   * Decode: `landmark_i_axis = (byte_i - 128) / 127.0` → normalized [-1, 1].
   */
  readonly landmarks_quantized?: string | null;
  /** Optional Facial Action Unit intensities, keyed by AU code, values in [0, 1]. */
  readonly microexpr?: Readonly<Record<string, number>>;
  /** Optional gaze direction. */
  readonly gaze?: GazeDirection | null;
}

/**
 * Vocal prosody features over the window.
 *
 * Default backend: openSMILE eGeMAPSv02 + ComParE2016. The summary record
 * carries a handful of interpretable indicators; the full feature vector
 * (`features_dim`-wide) is persisted to the binary sidecar.
 */
export interface ProsodyModality {
  /** Identifier of the prosody extractor. */
  readonly model: string;
  /** Composite tremor indicator in [0, 1], derived from jitter and shimmer. */
  readonly tremor_index?: number | null;
  /** Variance of fundamental frequency (Hz²) over the window. */
  readonly f0_var?: number | null;
  /** Estimated speaking rate, words per minute. */
  readonly pace_words_per_minute?: number | null;
  /** Dimensionality of the full feature vector (eGeMAPSv02 + ComParE2016 deduped ≈ 6552). */
  readonly features_dim?: number | null;
}

/**
 * Composite credibility-signal output.
 *
 * **NOT a lie-detection verdict.** A per-window indicator of stress and
 * incongruence, surfaced with a disclosed accuracy ceiling (~0.74) and a
 * peer-reviewed human baseline (~0.54) so callers can make informed
 * decisions.
 *
 * Forensic, hiring, insurance, and law-enforcement adjudication use cases
 * are refused at the API layer. EU workplace and education contexts are
 * blocked under the EU AI Act (effective August 2026).
 */
export interface CredibilityModality {
  /** Identifier of the credibility model. */
  readonly model: string;
  /** Indicator score in [0, 1]. Higher means more stress/incongruence signal present. */
  readonly score: number;
  /** Calibrated uncertainty bucket. Display this prominently next to `score`. */
  readonly confidence_band: CredibilityConfidenceBand;
  /** Human baseline accuracy for video-based deception inference (~0.54). */
  readonly human_baseline_disclosed: number;
  /** Accuracy ceiling for video-based deception inference in the literature (~0.74). */
  readonly ceiling_disclosed: number;
  /** True if the model is still in private beta. Label scores as preview-quality. */
  readonly labs_preview?: boolean;
  /** Vertical declared at API-key creation; recorded for audit. */
  readonly vertical_attestation?: CredibilityVertical | null;
}

/**
 * Sport-judging score output.
 *
 * Per-discipline model fine-tuned on that discipline's reference rubric.
 * Diving launches first (FINA 2025 rubric), followed by weightlifting,
 * martial-arts forms, and gymnastics.
 */
export interface JudgeModality {
  /** Identifier of the judge model, e.g. `"diaspor-judge-v1"`. */
  readonly model: string;
  /** Sport discipline, e.g. `"diving"`. */
  readonly discipline: string;
  /** Discipline-specific score on the rubric's native scale. */
  readonly score: number;
  /** Optional execution-only sub-score on the rubric's native scale. */
  readonly execution_score?: number | null;
  /** Optional difficulty multiplier (degree of difficulty), where applicable. */
  readonly difficulty_multiplier?: number | null;
  /** Identifier of the rubric the model was calibrated against, e.g. `"fina-2025"`. */
  readonly rubric_version?: string | null;
}

/**
 * Per-modality outputs.
 *
 * Each key is optional — a window record may carry only a subset (pose-only
 * for sport judging, or face + prosody for credibility). At least one
 * modality must be present.
 */
export interface Modalities {
  readonly pose?: PoseModality;
  readonly face?: FaceModality;
  readonly prosody?: ProsodyModality;
  readonly credibility?: CredibilityModality;
  readonly judge?: JudgeModality;
}

/**
 * Provenance record for one model's contribution to a score.
 *
 * Same shape as the `ModelProvenance` in `sidecar-v1.json`, intentionally
 * shared across both record schemas.
 */
export interface ModelProvenance {
  /** Identifier of the model, e.g. `"diaspor-pose-3d-v1@blazepose-heavy"`. */
  readonly model_name: string;
  /** Optional hex-encoded SHA-256 of the model file. */
  readonly model_hash?: string | null;
  /** Optional per-tenant LoRA adapter identifier (custom tier only). */
  readonly adapter_id?: string | null;
  /** Inference backend that ran the model. */
  readonly runtime?: ModelRuntime | null;
  /** End-to-end inference latency for this modality, microseconds. */
  readonly latency_us?: number | null;
}

/**
 * A single Diaspor score record.
 *
 * Mirrors the root object of `docs/schema/score-v1.json`. Emitted per
 * analyzed window of a stream (one per second by default, configurable). For
 * batch uploads, the entire video collapses to a single record with the full
 * `[0, duration_ms]` window.
 */
export interface ScoreRecord {
  /** Schema version, always the literal `"1"` for v1 records. */
  readonly schema_version: ScoreSchemaVersion;
  /** Opaque, tenant-unique identifier for the stream. */
  readonly stream_id: string;
  /** Opaque tenant identifier the stream belongs to. */
  readonly tenant: string;
  /** Inclusive lower bound of the analyzed window, ms from stream start. */
  readonly t_start_ms: number;
  /** Exclusive upper bound; must be > `t_start_ms`. */
  readonly t_end_ms: number;
  /** Whether this is a periodic window aggregate or a threshold-crossing event. */
  readonly kind?: ScoreRecordKind;
  /** Per-modality outputs. */
  readonly modalities: Modalities;
  /** RFC 3339 timestamp of when the record was finalized; must include a TZ offset. */
  readonly extracted_at: string;
  /** Optional model-provenance records, one per modality that contributed. */
  readonly model_provenance?: readonly ModelProvenance[];
}

// ─────────────────────────────────────────────────────────────────────────
// Runtime validator
// ─────────────────────────────────────────────────────────────────────────

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireString(value: unknown, path: string): string {
  if (typeof value !== "string") {
    throw new SchemaValidationError(`expected string, got ${typeof value}`, path);
  }
  return value;
}

function requireNumber(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new SchemaValidationError(`expected finite number, got ${typeof value}`, path);
  }
  return value;
}

function requireInteger(value: unknown, path: string): number {
  const n = requireNumber(value, path);
  if (!Number.isInteger(n)) {
    throw new SchemaValidationError(`expected integer, got ${n}`, path);
  }
  return n;
}

function requireObject(value: unknown, path: string): Record<string, unknown> {
  if (!isObject(value)) {
    throw new SchemaValidationError(`expected object, got ${typeof value}`, path);
  }
  return value;
}

function validateModality(
  modality: unknown,
  path: string,
  requiredKeys: readonly string[],
): Record<string, unknown> {
  const obj = requireObject(modality, path);
  for (const key of requiredKeys) {
    if (!(key in obj)) {
      throw new SchemaValidationError(`missing required field "${key}"`, path);
    }
  }
  return obj;
}

/**
 * Parse an unknown value into a {@link ScoreRecord}, validating shape.
 *
 * Hand-rolled minimal validator that checks every required field exists and
 * has the right primitive kind. It is deliberately permissive about extras
 * (forward compatibility) and about optional fields (only emitted when
 * present). On failure it throws a {@link SchemaValidationError} pointing at
 * the offending field.
 *
 * This is intended for parsing JSON that came in over the wire. Server-side
 * validation against the full JSON Schema is authoritative; this is a cheap
 * client-side smoke test that gives you a typed object.
 *
 * @param value Anything you got from `JSON.parse()` or a WebSocket frame.
 * @returns A frozen {@link ScoreRecord} matching the v1 schema.
 * @throws {SchemaValidationError} if the input does not conform.
 */
export function parseScoreRecord(value: unknown): ScoreRecord {
  const root = requireObject(value, "/");

  const schemaVersion = requireString(root["schema_version"], "/schema_version");
  if (schemaVersion !== "1") {
    throw new SchemaValidationError(
      `unsupported schema_version "${schemaVersion}"; this SDK only understands "1"`,
      "/schema_version",
    );
  }

  const streamId = requireString(root["stream_id"], "/stream_id");
  if (streamId.length === 0) {
    throw new SchemaValidationError("must be non-empty", "/stream_id");
  }
  const tenant = requireString(root["tenant"], "/tenant");
  if (tenant.length === 0) {
    throw new SchemaValidationError("must be non-empty", "/tenant");
  }

  const tStartMs = requireInteger(root["t_start_ms"], "/t_start_ms");
  if (tStartMs < 0) {
    throw new SchemaValidationError("must be >= 0", "/t_start_ms");
  }
  const tEndMs = requireInteger(root["t_end_ms"], "/t_end_ms");
  if (tEndMs <= tStartMs) {
    throw new SchemaValidationError(
      `must be greater than t_start_ms (${tStartMs})`,
      "/t_end_ms",
    );
  }

  let kind: ScoreRecordKind | undefined;
  if (root["kind"] !== undefined) {
    const k = requireString(root["kind"], "/kind");
    if (k !== "window" && k !== "event") {
      throw new SchemaValidationError(`expected "window" or "event", got "${k}"`, "/kind");
    }
    kind = k;
  }

  const modalities = requireObject(root["modalities"], "/modalities");
  const modalityKeys = Object.keys(modalities);
  if (modalityKeys.length === 0) {
    throw new SchemaValidationError("at least one modality required", "/modalities");
  }
  for (const key of modalityKeys) {
    if (!["pose", "face", "prosody", "credibility", "judge"].includes(key)) {
      throw new SchemaValidationError(`unknown modality "${key}"`, `/modalities/${key}`);
    }
  }

  // Per-modality required-field validation.
  if ("pose" in modalities) {
    const pose = validateModality(modalities["pose"], "/modalities/pose", ["model", "keypoints"]);
    requireString(pose["model"], "/modalities/pose/model");
    const kp = pose["keypoints"];
    if (!Array.isArray(kp) || kp.length !== 33) {
      throw new SchemaValidationError(
        `expected 33 keypoints, got ${Array.isArray(kp) ? kp.length : typeof kp}`,
        "/modalities/pose/keypoints",
      );
    }
  }
  if ("face" in modalities) {
    const face = validateModality(modalities["face"], "/modalities/face", ["model"]);
    requireString(face["model"], "/modalities/face/model");
  }
  if ("prosody" in modalities) {
    const prosody = validateModality(modalities["prosody"], "/modalities/prosody", ["model"]);
    requireString(prosody["model"], "/modalities/prosody/model");
  }
  if ("credibility" in modalities) {
    const cred = validateModality(modalities["credibility"], "/modalities/credibility", [
      "model",
      "score",
      "confidence_band",
      "human_baseline_disclosed",
      "ceiling_disclosed",
    ]);
    requireString(cred["model"], "/modalities/credibility/model");
    const score = requireNumber(cred["score"], "/modalities/credibility/score");
    if (score < 0 || score > 1) {
      throw new SchemaValidationError("must be in [0, 1]", "/modalities/credibility/score");
    }
    const band = requireString(cred["confidence_band"], "/modalities/credibility/confidence_band");
    if (band !== "low" && band !== "medium" && band !== "high") {
      throw new SchemaValidationError(
        `expected "low" | "medium" | "high", got "${band}"`,
        "/modalities/credibility/confidence_band",
      );
    }
    requireNumber(cred["human_baseline_disclosed"], "/modalities/credibility/human_baseline_disclosed");
    requireNumber(cred["ceiling_disclosed"], "/modalities/credibility/ceiling_disclosed");
  }
  if ("judge" in modalities) {
    const judge = validateModality(modalities["judge"], "/modalities/judge", [
      "model",
      "discipline",
      "score",
    ]);
    requireString(judge["model"], "/modalities/judge/model");
    requireString(judge["discipline"], "/modalities/judge/discipline");
    requireNumber(judge["score"], "/modalities/judge/score");
  }

  const extractedAt = requireString(root["extracted_at"], "/extracted_at");
  // Cheap RFC-3339 sniff: must end in `Z` or `+HH:MM`/`-HH:MM` and contain `T`.
  if (!/T/.test(extractedAt) || !/(Z|[+-]\d{2}:\d{2})$/.test(extractedAt)) {
    throw new SchemaValidationError(
      "must be RFC 3339 with timezone offset (e.g. 2026-05-15T12:30:13Z)",
      "/extracted_at",
    );
  }

  if (root["model_provenance"] !== undefined) {
    const mp = root["model_provenance"];
    if (!Array.isArray(mp)) {
      throw new SchemaValidationError("expected array", "/model_provenance");
    }
    mp.forEach((entry, i) => {
      const entryObj = requireObject(entry, `/model_provenance/${i}`);
      requireString(entryObj["model_name"], `/model_provenance/${i}/model_name`);
    });
  }

  // Cast through unknown because the validator has guaranteed shape but the
  // structural type check happens at compile time, not at runtime.
  return root as unknown as ScoreRecord;
}
