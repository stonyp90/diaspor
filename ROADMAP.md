# Roadmap

Six milestones over 12 months. The plan is shaped around the
[NGI Zero Commons Fund](https://nlnet.nl/commonsfund/) milestone-based payment model:
each row below corresponds to one funding tranche, with a public release tag, a
checked-in test suite, and a short progress note in `RELEASES.md`.

Estimated working capacity for the funded portion: **50 % FTE (~20 h/week)** for 12
months, delivered by Anthony Paquet (sole maintainer at the time of writing).

The roadmap is intentionally **VFS-first, AI-optional**: milestones M1–M4 deliver a
cross-platform virtual filesystem that meets explicit measurable acceptance criteria
(≥ 85 % line coverage on memory + local backends, CI green on Linux/macOS/Windows,
published conformance suite, ≥ 1 external user integration tracked publicly). The
content-understanding layer (FFmpeg + Whisper + local LLM) lands in M5–M6 as an
**opt-in** crate (`diaspor-index`) that downstream applications can leave out
entirely. This phasing reduces NLnet review risk: the core deliverable does not
depend on the AI tooling chain being friction-free.

| ID  | Window        | Theme                                                                   |
|-----|---------------|-------------------------------------------------------------------------|
| M1  | Month 1–2     | Core API frozen + memory backend at 100 %                               |
| M2  | Month 3–4     | Local backend meets acceptance criteria, cross-platform path tests               |
| M3  | Month 5–6     | FUSE adapter end-to-end on Linux and macOS                              |
| M4  | Month 7–8     | WinFsp adapter end-to-end on Windows                                    |
| M5  | Month 9–10    | **FFmpeg + `whisper.cpp` transcription pipeline** shipped (opt-in crate)|
| M6  | Month 11–12   | **Local-LLM auto-tagging + sidecar persistence + v1.0 release**         |

---

## M1 — Core API frozen + memory backend at 100 % (Month 1–2)

**Goal:** lock the `VfsBackend` / `VfsHandle` traits, ship a reference in-memory backend
that passes a published conformance test suite, and publish the first crates to
crates.io.

**Deliverables**

- `diaspor-core` v0.1.0 published to crates.io with full rustdoc.
- `diaspor-backend-memory` v0.1.0 with > 90 % line coverage.
- `diaspor-conformance` test crate: a `conformance::run(backend)` helper any third
  party can use to validate their own backend implementation against the spec.
- `cargo doc` warnings-as-errors clean.
- Public release notes + tagged `v0.1.0` git tag.

**Success criteria**

- `cargo test --workspace` green on Linux / macOS / Windows in CI.
- Conformance suite covers every method on every flag combination of `OpenFlags`.
- At least one external developer runs the suite against a stub backend and reports
  back (tracked via a public GitHub issue).

---

## M2 — Local backend meets acceptance criteria (Month 3–4)

**Goal:** make `diaspor-backend-local` a credible drop-in for code that currently uses
`tokio::fs` directly. Cover the long tail of platform quirks that bite real apps.

**Deliverables**

- Symlink handling (resolved vs raw modes), extended attributes (best-effort), file
  locking semantics.
- Cross-platform path normalization with a dedicated property-based test suite.
- Bench suite (`criterion`) comparing `diaspor-backend-local` to `tokio::fs` on a
  read/write workload — published in `BENCHMARKS.md` with reproducible scripts.
- `diaspor-backend-local` v0.2.0 release.

**Success criteria**

- All happy-path and error-path tests pass on Linux, macOS, and Windows.
- Bench overhead vs `tokio::fs` under 10 % on the read path and under 15 % on the write
  path for typical workloads.
- One downstream user (existing OSS project) has integrated the local backend behind a
  feature flag (tracked publicly).

---

## M3 — FUSE adapter end-to-end (Month 5–6)

**Goal:** the milestone the userbase has been waiting for — actually mounting a backend
as a real filesystem on Linux and macOS via the `fuser` crate.

**Deliverables**

- `diaspor-fuse` v0.1.0 with `mount()` returning a working `FuseMount`.
- Async-to-sync bridge that lets the FUSE thread call into the async `VfsBackend` API
  without blocking the entire runtime.
- macFUSE compatibility tested on macOS via CI runners.
- `examples/fuse-mount.rs` showing how to mount any backend.
- Integration tests that mount a backend, do file IO via host tools (`cp`, `cat`, `ls`),
  and unmount cleanly.

**Success criteria**

- A user can `cargo run --example fuse-mount -- /mnt/foo` and `cat`, `ls`, `touch`
  against it from a normal shell on Linux and macOS.
- Unmount is safe under SIGINT and after panics inside the FUSE thread.

---

## M4 — WinFsp adapter end-to-end (Month 7–8)

**Goal:** parity with M3 on Windows via the WinFsp user-mode filesystem driver.

**Deliverables**

- `diaspor-winfsp` v0.1.0 with `mount()` returning a working `WinFspMount`.
- Bindings against the WinFsp DLL with proper RAII unmount.
- Integration tests on a Windows CI runner using PowerShell to drive file operations.
- `examples/winfsp-mount.rs` mirroring `examples/fuse-mount.rs`.

**Success criteria**

- A user can mount a memory backend as a Windows drive letter and read/write files via
  Explorer or PowerShell.
- Mount/unmount survives a Ctrl-C in the host process.

---

## M5 — FFmpeg + Whisper transcription pipeline (Month 9–10)

**Goal:** ship the first piece of `diaspor-index`'s content-understanding pipeline:
take a media file inside any VFS backend, run FFmpeg to extract its audio, and produce
a Whisper transcript — all locally, no cloud calls.

This is the milestone that turns `diaspor` from "yet another VFS abstraction" into
**"the privacy-first filesystem that understands your media."**

**Deliverables**

- `diaspor-index` v0.2.0 with two production implementations:
  - `FfmpegExtractor` — wraps the `ffmpeg` binary, probes media (codec, duration,
    streams), and losslessly extracts the primary audio track as 16 kHz mono PCM.
  - `WhisperCppTranscriber` — wraps `whisper.cpp` via FFI (`whisper-rs`), runs entirely
    on-device, supports CPU and (optionally) GPU inference.
- Pluggable model selection: the caller chooses the GGUF model
  (`ggml-tiny.en.bin` for fast CI tests, `ggml-large-v3.bin` for production).
- Conformance test using a public-domain audio clip with a known transcript; CI fails
  if word-error rate exceeds a documented threshold.
- `examples/transcribe-folder.rs`: walks any backend, transcribes media files it finds,
  writes sidecar JSON records back through the VFS itself.
- `docs/PRIVACY.md` explaining the zero-cloud guarantee and how to audit it.

**Success criteria**

- End-to-end demo: drop an MP4 into a `MemoryBackend`, get a JSON sidecar with a usable
  transcript, all without making a network call (verified by a sandbox test that
  blocks egress).
- Word-error rate on the canonical test clip ≤ 12 % using `ggml-base.en` (the smallest
  practical model). The threshold is a regression guard, not a research target.
- An external maintainer of a related OSS project (Whisper.cpp-based desktop app,
  podcast indexer, etc.) reviews the architecture publicly.

---

## M6 — Local-LLM auto-tagging + sidecar persistence + v1.0 (Month 11–12)

**Goal:** complete the content-understanding pipeline with a local LLM that turns each
transcript into a small set of semantic tags, categories, and a one-line summary. Ship
the documentation site and tag the v1.0 release.

**Deliverables**

- `OllamaTagger` and `LlamaCppTagger` implementations of the `Tagger` trait, both
  running on-device with no network calls by default.
- Stable JSON schema for `SidecarRecord` published in-repo at
  `docs/schema/sidecar-v1.json` (and mirrored to GitHub Pages once the site is up),
  with `serde` derive + a versioned `schema_version` field.
- Sidecar persistence: the indexer writes `/.index/<path>.json` back into the backend
  it wraps, so transcripts are queryable through normal VFS reads.
- `examples/grep-the-podcasts.rs`: a CLI that mounts a folder, transcribes it, and lets
  the user `grep` across transcripts via standard shell tools.
- mdbook documentation site hosted at `stonyp90.github.io/diaspor` (GitHub Pages,
  free; a custom domain can be added later if useful) covering architecture, recipes,
  FAQ, security model, threat model.
- Three example applications shipped under `examples/apps/`:
  1. A privacy-respecting note-taking app that auto-transcribes voice memos.
  2. A backup tool that indexes media-bearing archives before storing them.
  3. A small "library server" that exposes a transcribed folder over HTTP.
- Public **v1.0 release** with stability guarantees on the core traits and the sidecar
  schema.

**Success criteria**

- Documentation site live, with at least 90 % public-API coverage in rustdoc.
- Each example app builds and runs from a clean clone.
- The privacy contract is independently auditable: a sandbox test confirms no egress
  during a full pipeline run, and the audit is reproducible from a clean clone.
- A public retrospective written and published, including a frank accounting of what
  worked, what did not, and what the next year of work would prioritize.

---

## Out of scope for this 12-month plan

These items are explicitly **not** part of the NLnet-funded work, to keep the
deliverables honest:

- **Cloud storage backends** (S3, Azure Blob, etc.) — designed for, but implemented
  later by community contributors or follow-up funding.
- **A user-facing GUI** — that lives in downstream applications, not in this library.
- **Synchronization protocols** (CRDTs, vector clocks, peer-to-peer transports). The
  trait surface is sync-friendly, but the sync logic itself is out of scope.
- **Sandboxed plugin / WASM modules.**
- **Hosted inference fallbacks.** The library will ship only on-device implementations
  by default. If a downstream application wants OpenAI / Anthropic fallbacks, it
  implements the `Transcriber` / `Tagger` traits itself — no cloud code lives in the
  core repo.

If the funding cycle is renewed for a year two, the natural candidates are
particularly cloud backends, a CRDT-based sync layer, and the **non-verbal video
analysis pipeline** scoped in M7–M10 below.

---

# Post-v1.0 roadmap (year 2+, post-NLnet)

The milestones below describe the **non-verbal video analysis pipeline** the
project is positioned around (pose, facial landmarks, vocal prosody, credibility
signals, sport judging). They are **not part of the NLnet-funded year-one work**
— they live outside the M1–M6 funding envelope and ship under a separate funding
arrangement (commercial revenue from the early-access cohort, follow-on grants,
or a successor NLnet cycle).

The architectural seam is in place at v0.1.0-alpha: six trait-surface-only crates
ship in the repo with no production implementations behind them. The
corresponding score-record schema (`docs/schema/score-v1.json`) is published
alongside the existing sidecar schema so downstream tooling can target a stable
shape from day one. See [ADR 0007](docs/adr/0007-score-sidecar-schema.md) for
the schema separation decision and ARCHITECTURE.md for the broader plan
(`/Users/tony/.claude/plans/we-are-claiming-that-velvet-bumblebee.md` is the
working build plan).

## M7 — Vision pipeline batch on uploaded video (target: month 13–16)

**Goal:** turn the `diaspor-vision` trait surface into a working batch pipeline.
Pose + face landmarks + prosody extracted from an mp4 dropped into a backend,
persisted as a v1 score record.

**Deliverables**

- `diaspor-vision` v0.2.0 with three production extractors:
  - `MediaPipeBlazePoseExtractor` — 33 keypoints in BlazePose topology.
  - `MediaPipeFaceMeshExtractor` — 478 landmarks (`refine_landmarks=True`).
  - `OpensmileProsodyExtractor` — eGeMAPSv02 + ComParE2016 feature set
    (final dimensionality locked at integration time; the `6552` constant in
    `diaspor-vision` is a placeholder pending openSMILE config freeze).
- `diaspor-frame-pipeline` v0.2.0 with the `FfmpegDecodeBackend` subprocess
  wired and `UniformFrameSampler` returning real frame indices.
- `diaspor-infer` v0.2.0 with `CoreMLInferenceBackend` and
  `OrtCpuInferenceBackend` wired. No Triton yet — Phase 1 is batch.
- `diaspor-stream-ingest::ingest::file` (already shipped as real code in
  v0.1.0-alpha) integrated end-to-end.
- `diaspor-events::VfsEventSink` wired — writes
  `/.streams/<stream_id>/windows/<timestamp>.score.json` into the backend.
- Score schema (`docs/schema/score-v1.json`) round-trip tested in CI.
- `examples/vision-batch-folder.rs` — drop mp4 → score.json appears.

**Success criteria**

- Pose PCK@0.05 ≥ 0.85 on a public FineDiving sample clip; per-keypoint
  failure rate ≤ 0.05.
- Face NME within 5% of MediaPipe reference on a 300W subset.
- Prosody extractor produces deterministic features on the same audio
  (byte-identical output across two runs).
- No network calls during a full batch run (sandbox: `unshare --net`, same
  gate as M5).
- Score v1 round-trips: write → read → parse → re-serialize is
  byte-identical against the schema.
- One external maintainer (sports-vision OSS project or research lab)
  reviews the trait surface publicly.

## M8 — Live ingest (WHIP + meeting-bot for Zoom/Meet/Teams) + Triton serving (target: month 17–22)

**Goal:** real-time analysis from WHIP push and from meeting-bot ingest.
Triton ensemble graph + per-tenant LoRA adapters. This is the milestone that
makes the "live on Zoom · Meet · Teams" capability claim truthful.

**Deliverables**

- `diaspor-stream-ingest::ingest::whip` wired against a Pion or mediasoup
  SFU sidecar (chosen during M8 kick-off; rolling our own `webrtc-rs` WHIP
  receiver remains an open question).
- `diaspor-stream-ingest::ingest::meeting_bot` wired against Recall.ai
  (webhook for lifecycle, WebSocket for raw video+audio frames). A single
  `BotProvider::RecallAi` config covers Zoom, Google Meet, and Microsoft
  Teams through Recall.ai's unified API.
- `diaspor-infer::TritonInferenceBackend` wired (gRPC client against an
  in-cluster Triton instance). DeepStream colocated decode path for L4/A10G.
- `diaspor-events::WebSocketEventSink` and `WebhookEventSink` wired with
  HMAC-SHA256 signatures (`X-Diaspor-Signature` header).
- LoRA-per-tenant routing on a shared GPU pool (one Triton model store, an
  adapter loaded per request keyed by JWT claim).
- `helm` chart under a new `deploy/diaspor/` directory.
- TOS clauses: all-party consent for meeting-bot use cases; EU workplace and
  education contexts blocked at the API for credibility outputs.

**Success criteria**

- WHIP push: glass-to-event under 500 ms on a single L4 GPU.
- Triton ensemble `pose-detect → 33-keypoint → judge-head` returns per-frame
  tensors at 10 fps for a single 1080p stream.
- L4 sustains 8 concurrent 1080p streams at 10 fps under sustained load.
- Recall.ai bot lands in test Zoom, Meet, and Teams meetings within 10 s of
  scheduling; identical-schema score events stream out within 3 s of live
  audio. All three platforms produce byte-identical schema output (proof of
  unified abstraction).
- Per-tenant LoRA swap: two tenants pushing concurrently get different
  outputs, verified by the `model_provenance.adapter_id` field in the score
  record.

## M9 — Custom-tier LoRA training + LL-HLS + diving judge (target: month 23–28)

**Goal:** ship the custom-tier training pipeline (per-customer LoRA on
InternVideo2-1B) + the first sport-judge discipline + LL-HLS pull ingest.

**Deliverables**

- `diaspor-train` v0.2.0 with the full pipeline wired:
  `InternVideo2Backbone` against a downloaded InternVideo2-1B (Apache 2.0)
  checkpoint, embedding-cache in per-tenant Parquet, `LoraTrainer` against
  the `default_credibility_lora_config()` / `default_judge_lora_config()`
  defaults, eval gate that refuses to sign an adapter that does not beat
  baseline by the contractual delta on the customer's held-out set,
  Ed25519-signed handoff (`AdapterArtifact::path_in_tenant_bucket()`).
- `diaspor-stream-ingest::ingest::hls` wired for LL-HLS (CMAF partial
  segments) pull ingest. Plain HLS pull supported as a fallback.
- First production `diaspor-judge-v1`: diving discipline, fine-tuned on
  FineDiving + MTL-AQA.
- First federation pilot live (Diving Canada or a provincial federation).
- Compliance refusal list enforced in `diaspor-train`: training jobs whose
  tenant vertical is `forensic` / `hiring` / `insurance` /
  `law_enforcement` / `eu_workplace` / `eu_education` fail at the API
  layer before any compute is consumed.

**Success criteria**

- Customer corpus → signed adapter delivered in under 6 weeks
  (annotation + training + eval).
- LoRA training on a 1K-clip diving corpus: under 24 h on a single A100 80 GB.
- Eval gate: Spearman ρ ≥ 0.80 against a human-judge panel on the AQA-7
  held-out split.
- Customer can `aws s3 cp` their adapter and verify both our and their
  signatures.
- Diving judge running live during a federation event with a
  judge-in-the-loop UI.

## M10 — Public SDK + API (target: month 29–34)

**Goal:** developer-grade, self-serve SDK and hosted API. A new revenue
motion distinct from enterprise sales — pay-as-you-go, no procurement cycle.

**Deliverables**

- Public REST API at `api.diaspor.io`: `/v1/analyze` (batch),
  per-modality endpoints (`/v1/pose`, `/v1/face-mesh`, `/v1/prosody`,
  `/v1/credibility`, `/v1/judge`), polling for long-running jobs.
- WebSocket API for live: direct WHIP push and meeting-bot wrappers.
- Thin client SDKs (Apache-2.0 — no AGPL exposure for closed-source
  embedders): `pip install diaspor`, `npm install @diaspor/sdk`,
  `cargo add diaspor-client`.
- Heavy self-hosted core dual-licensed (AGPL-3.0 or commercial; the
  decision and CLA structure to be filed as ADR 0008 ahead of this
  milestone).
- API gateway: Kong or Cloudflare Workers + a custom meter writing to
  ClickHouse for billing aggregation, with Stripe metered billing keyed
  off the meter.
- Per-key vertical attestation enforced at the gateway: customer declares
  vertical at key creation; credibility endpoint refuses calls from keys
  in any forbidden vertical. EU IP geo-block as a second layer.
- Hard per-key per-day caps with configurable soft alerts (the
  "bot-left-in-meeting-overnight = $400 surprise" mitigation).
- `developers.diaspor.io` docs site.

**Success criteria**

- Public API GA. ≥ 10 paying API customers in the first quarter.
- Free tier: 100 batch minutes/month, no credibility, no live, no training.
  Conversion to paid > 5%.
- p99 latency for `/v1/analyze` under 30 s for a 60-second mp4 input on
  the shared-GPU pool.
- Zero credibility-endpoint calls from forbidden verticals (geo-fence +
  attestation-gate verified by gateway logs).
- "Powered by Diaspor" badge program live with an explicit approval
  workflow; TOS prohibits unbadged commercial use of the API.

---

## Year-2+ open decisions

These get a follow-up ADR each before their owning milestone begins:

- **AGPL vs dual-license on the heavy crates** (decision before M10
  begins; default recommendation: dual-license, Sentry/PostHog-style CLA).
- **WHIP receiver: rolled-our-own vs SFU sidecar (Pion / mediasoup)**
  (decision before M8 begins; default recommendation: SFU sidecar over
  gRPC until `webrtc-rs` is production-mature).
- **Second sport-judge discipline after diving** (decision before M9
  closes; default recommendation: weightlifting or martial-arts forms —
  objective rubrics, not gymnastics where Fujitsu JSS is already at
  ~90% element spotting).
