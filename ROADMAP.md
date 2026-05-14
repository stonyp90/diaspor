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
**opt-in** crate (`cairn-index`) that downstream applications can leave out
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

- `cairn-core` v0.1.0 published to crates.io with full rustdoc.
- `cairn-backend-memory` v0.1.0 with > 90 % line coverage.
- `cairn-conformance` test crate: a `conformance::run(backend)` helper any third
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

**Goal:** make `cairn-backend-local` a credible drop-in for code that currently uses
`tokio::fs` directly. Cover the long tail of platform quirks that bite real apps.

**Deliverables**

- Symlink handling (resolved vs raw modes), extended attributes (best-effort), file
  locking semantics.
- Cross-platform path normalization with a dedicated property-based test suite.
- Bench suite (`criterion`) comparing `cairn-backend-local` to `tokio::fs` on a
  read/write workload — published in `BENCHMARKS.md` with reproducible scripts.
- `cairn-backend-local` v0.2.0 release.

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

- `cairn-fuse` v0.1.0 with `mount()` returning a working `FuseMount`.
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

- `cairn-winfsp` v0.1.0 with `mount()` returning a working `WinFspMount`.
- Bindings against the WinFsp DLL with proper RAII unmount.
- Integration tests on a Windows CI runner using PowerShell to drive file operations.
- `examples/winfsp-mount.rs` mirroring `examples/fuse-mount.rs`.

**Success criteria**

- A user can mount a memory backend as a Windows drive letter and read/write files via
  Explorer or PowerShell.
- Mount/unmount survives a Ctrl-C in the host process.

---

## M5 — FFmpeg + Whisper transcription pipeline (Month 9–10)

**Goal:** ship the first piece of `cairn-index`'s content-understanding pipeline:
take a media file inside any VFS backend, run FFmpeg to extract its audio, and produce
a Whisper transcript — all locally, no cloud calls.

This is the milestone that turns `cairn` from "yet another VFS abstraction" into
**"the privacy-first filesystem that understands your media."**

**Deliverables**

- `cairn-index` v0.2.0 with two production implementations:
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
- mdbook documentation site hosted at `stonyp90.github.io/cairn` (GitHub Pages,
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

These items are explicitly **not** part of the funded work, to keep the deliverables
honest:

- **Cloud storage backends** (S3, Azure Blob, etc.) — designed for, but implemented
  later by community contributors or follow-up funding.
- **A user-facing GUI** — that lives in downstream applications, not in this library.
- **Synchronization protocols** (CRDTs, vector clocks, peer-to-peer transports). The
  trait surface is sync-friendly, but the sync logic itself is out of scope.
- **Sandboxed plugin / WASM modules.**
- **Vision / OCR / image tagging.** The pipeline is audio-first in v1.0; visual
  modalities are an obvious year-two extension.
- **Hosted inference fallbacks.** The library will ship only on-device implementations
  by default. If a downstream application wants OpenAI / Anthropic fallbacks, it
  implements the `Transcriber` / `Tagger` traits itself — no cloud code lives in the
  core repo.

If the funding cycle is renewed for a year two, these are the natural candidates —
particularly cloud backends, OCR, and a CRDT-based sync layer.
