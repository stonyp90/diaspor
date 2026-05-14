# Privacy contract

`cairn` ships with a structural — not aspirational — privacy
contract. This file documents what the library does and does not do
with user data, what an integrator must do to preserve those
guarantees, and how the contract is enforced in CI.

## What `cairn` promises

- **No telemetry, ever.** The library makes zero outbound network
  calls of its own accord. There is no analytics, no auto-update
  check, no "anonymous usage stats." If you observe one, that is a
  bug — please file a security report (see `SECURITY.md`).
- **No implicit cloud transcription.** The default `Transcriber`
  trait implementation runs on-device via `whisper.cpp`. Cloud
  variants (OpenAI Whisper API, AssemblyAI, etc.) are *opt-in* via
  separate feature flags and require explicit construction by the
  integrating application.
- **No implicit cloud tagging.** The default `Tagger` runs a local
  LLM (`llama.cpp` or `ollama`) on-device. Same opt-in rule as
  transcription.
- **Sidecar metadata stays inside the VFS.** Transcripts and tags
  produced by the pipeline are persisted as sidecar JSON files
  through the same `VfsBackend` instance that holds the source media.
  Nothing is uploaded, mirrored, or synced outside the integrator's
  control.
- **No "leak through dependencies."** Direct and transitive dependencies
  are vetted: every dependency added in M1 and beyond is reviewed for
  network behaviour, and `cargo-deny` flags any new dependency whose
  licence or supply-chain provenance is suspect.

## What integrators must do to preserve the contract

- **Choose the on-device defaults.** If you swap in a `CloudTranscriber`
  or `CloudTagger`, you are operating outside the privacy contract.
  Disclose to your users.
- **Audit your sidecar destination.** If your VFS backend is
  `cairn-backend-cloud-s3` (hypothetical future backend), the
  sidecar JSON files containing transcripts will write to S3. The
  library does not know that — it just calls `backend.open(...).write()`.
  Decide your data residency policy at the backend choice.
- **Do not log transcripts or audio.** The library does not log
  payload data, but if you pipe pipeline outputs into a log shipper
  you have created a side channel. Treat transcripts as PII.

## How the contract is enforced in CI

The CI pipeline (`.github/workflows/ci.yml`) includes a **`no-network`**
job that runs the entire test suite inside a network-disabled namespace:

```bash
sudo --preserve-env=PATH,HOME,CARGO_HOME,RUSTUP_HOME unshare --net -- bash -c '
  cargo build --workspace --all-features --offline
  cargo test --workspace --all-features --offline
'
```

If any default code path opens a socket, the syscall fails inside the
namespace and the job fails. The job is *gating* on every push to
`main` — a failing run blocks merging. This is the structural
enforcement: the contract holds because the build fails when it breaks,
not because anyone remembered to check.

**Scope today (v0.1.0-alpha.1):** the test suite is gated. Milestone
M6 extends the same gate to the actual index pipeline running against
a real audio sample (Issue #14), at which point the gate also covers
FFmpeg / `whisper.cpp` / `llama.cpp` subprocess behaviour, not just
the Rust test suite.

## What this contract does *not* cover

- The behaviour of third-party `Transcriber` / `Tagger` implementations
  in other crates. Authors of `cairn-transcribe-azure` etc. are
  outside the contract.
- The behaviour of the FFmpeg, whisper.cpp, or llama.cpp binaries
  themselves. We invoke them with no network arguments, but if you
  set a custom `PATH` or shim those binaries with network-enabled
  versions, the contract is bypassed.
- Memory-disclosure attacks within the host (heap inspection, GPU
  memory scraping, OS-level keyloggers). Threat-model details in
  `THREAT_MODEL.md`.

## Reporting a violation

If you believe a code change has compromised the contract above, see
`SECURITY.md` for the disclosure process. A regression of the privacy
contract is treated with the same severity as a remote-code-execution
vulnerability.
