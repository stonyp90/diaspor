/**
 * @diaspor/sdk — official TypeScript client SDK for the Diaspor non-verbal
 * video AI API.
 *
 * Thin Apache-2.0-licensed client. Calls `api.diaspor.io`. The heavy
 * self-hosted core is the Rust workspace at github.com/stonyp90/diaspor
 * (dual-licensed AGPL-3.0 or commercial).
 *
 * Public surface:
 *
 * - {@link DiasporClient} — REST client (`/v1/analyze`, per-modality
 *   endpoints, polling).
 * - {@link LiveSession} — WebSocket live-streaming client (WHIP push and
 *   meeting-bot wrapper for Zoom / Google Meet / Microsoft Teams).
 * - {@link parseScoreRecord} — runtime validator for the score-v1 schema.
 * - Score-record model types: {@link ScoreRecord}, {@link Modalities}, and
 *   per-modality interfaces.
 * - Error hierarchy: {@link DiasporError}, {@link ApiError},
 *   {@link RateLimitedError}, {@link VerticalRefusedError},
 *   {@link NotImplementedYetError}, {@link SchemaValidationError}.
 * - {@link VERSION} — SDK semver string.
 */

export {
  DiasporClient,
  DEFAULT_BASE_URL,
  DEFAULT_TIMEOUT_MS,
  type AnalyzeOptions,
  type DiasporClientOptions,
  type JudgeOptions,
  type ModalityKey,
  type UploadablePayload,
} from "./client.js";

export {
  LiveSession,
  DEFAULT_LIVE_BASE_URL,
  type IngestEvent,
  type LiveCloseReason,
  type LiveSessionCommonOptions,
  type MeetingBotProvider,
  type MeetingBotSessionOptions,
  type MeetingPlatform,
  type WhipSessionOptions,
} from "./streaming.js";

export {
  parseScoreRecord,
  type CredibilityConfidenceBand,
  type CredibilityModality,
  type CredibilityVertical,
  type FaceModality,
  type GazeDirection,
  type JudgeModality,
  type Keypoint3d,
  type ModelProvenance,
  type ModelRuntime,
  type Modalities,
  type PoseModality,
  type ProsodyModality,
  type ScoreRecord,
  type ScoreRecordKind,
  type ScoreSchemaVersion,
  type Severity,
} from "./models.js";

export {
  DiasporError,
  ApiError,
  RateLimitedError,
  VerticalRefusedError,
  NotImplementedYetError,
  SchemaValidationError,
} from "./errors.js";

export { VERSION } from "./version.js";
