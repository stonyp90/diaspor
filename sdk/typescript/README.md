# @diaspor/sdk

Official TypeScript client SDK for the Diaspor non-verbal video AI API.

**Thin client — calls `api.diaspor.io`.** Heavy self-hosted core is the Rust
workspace at [github.com/stonyp90/diaspor](https://github.com/stonyp90/diaspor)
(dual-licensed AGPL-3.0 or commercial).

This SDK is **Apache-2.0** licensed so it can be embedded inside closed-source
applications without any AGPL exposure. Only the hosted API (and the Rust core
behind it) carries the copyleft obligation.

## What this wraps

- `POST /v1/analyze` — batch analysis of a video file (multipart upload or
  signed S3 URL). Returns a score record matching
  [`docs/schema/score-v1.json`](https://github.com/stonyp90/diaspor/blob/main/docs/schema/score-v1.json).
- `GET /v1/analyses/{id}` — polling for long-running batch jobs.
- `POST /v1/pose`, `POST /v1/face-mesh`, `POST /v1/prosody`,
  `POST /v1/credibility`, `POST /v1/judge?discipline=…` — per-modality
  endpoints when you only want one signal.
- `wss://api.diaspor.io/v1/stream?ingest=whip&token=…` — real-time WHIP
  ingest, scores stream out as JSON events.
- `wss://api.diaspor.io/v1/stream?bot=meeting&platform=zoom&meeting_url=…` —
  meeting-bot wrapper (Zoom / Meet / Teams via Recall.ai), same event shape.

## Install

```bash
npm install @diaspor/sdk
# or
pnpm add @diaspor/sdk
# or
yarn add @diaspor/sdk
```

Requires Node.js 20+ (native `fetch` and `WebSocket`) or any evergreen browser.
There are no runtime dependencies.

## Quickstart

### 1. Batch analyze a file

```ts
import { DiasporClient } from "@diaspor/sdk";
import { readFile } from "node:fs/promises";

const client = new DiasporClient({ apiKey: process.env.DIASPOR_API_KEY! });

const file = new Blob([await readFile("./dive.mp4")], { type: "video/mp4" });
const score = await client.analyze(file, { modalities: ["pose", "judge"] });

console.log(score.modalities.judge?.score); // e.g. 24.7 for a clean dive
```

### 2. Stream live from a WHIP source

```ts
import { LiveSession } from "@diaspor/sdk";

const session = await LiveSession.whip({
  token: process.env.DIASPOR_LIVE_TOKEN!,
  onEvent: (event) => {
    console.log("window", event.t_start_ms, event.modalities);
  },
});

// or use the async iterator
for await (const event of session) {
  if (event.modalities.credibility?.confidence_band === "high") {
    console.log("strong signal", event.modalities.credibility.score);
  }
}

await session.close();
```

### 3. Attach a meeting bot to a Zoom call

```ts
import { LiveSession } from "@diaspor/sdk";

const session = await LiveSession.meetingBot({
  token: process.env.DIASPOR_LIVE_TOKEN!,
  provider: "recall_ai",
  platform: "zoom",
  meetingUrl: "https://zoom.us/j/1234567890",
  consentScript: "This meeting is being analyzed by Diaspor for coaching purposes.",
  onEvent: (event) => {
    console.log("event", event.kind, event.t_start_ms);
  },
});

// session.close() removes the bot from the meeting.
```

## Credibility, responsibly

The credibility signal is **not lie detection**. It is a per-window indicator
of stress and behavioral incongruence, surfaced alongside a disclosed accuracy
ceiling (~0.74) and a peer-reviewed human baseline (~0.54) so consumers can
make informed decisions.

Calls to `client.credibility(...)` from API keys attested to `forensic`,
`hiring`, `insurance`, `law_enforcement`, `eu_workplace`, or `eu_education`
verticals will be refused server-side with a `VerticalRefusedError`. EU
workplace and education contexts are blocked under the EU AI Act (effective
August 2026).

## License separation

| Component                          | License             |
| ---------------------------------- | ------------------- |
| This SDK (`@diaspor/sdk`)          | Apache-2.0          |
| Hosted API at `api.diaspor.io`     | Proprietary service |
| Rust core (`github.com/stonyp90/diaspor`) | AGPL-3.0 OR commercial |

You can embed this SDK in any application, including closed-source commercial
products. You do **not** need a commercial license for the Rust core unless
you are running it yourself.

## Status

`v0.1.0-alpha.1` — pre-release. The wire is not live yet; this SDK is shipped
ahead of the API so downstream tooling can develop against a stable shape.
Treat every method as subject to change until `v0.1.0` final.

## Links

- API docs: [developers.diaspor.io](https://developers.diaspor.io)
- Repo: [github.com/stonyp90/diaspor](https://github.com/stonyp90/diaspor)
- Issues: [github.com/stonyp90/diaspor/issues](https://github.com/stonyp90/diaspor/issues)
