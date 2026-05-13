# Frequently Asked Questions

This FAQ covers the questions that come up most often from prospective adopters,
contributors, and reviewers. If your question is not here, please open a discussion on
GitHub or Codeberg.

---

## Comparison

### How is this different from rclone?

`rclone` is an excellent command-line tool for syncing files between cloud backends. It
does one thing — move bytes — and does it well. `stony-vdfs` is a *library*, not a CLI,
aimed at applications that want a single async API across memory, disk, and
user-space mount points (FUSE / WinFsp) *and* want optional content-aware indexing built
in. There is no overlap: an application could happily use `rclone` to mirror its
`stony-vdfs` storage to an off-site backup. The two projects address different layers
of the stack.

### How is this different from Nextcloud?

Nextcloud is a self-hosted server with a web UI, sharing, calendars, contacts, and a
plugin marketplace. It is a full collaboration platform. `stony-vdfs` is a Rust library
that gives a *single user's application* a filesystem-shaped API with local AI
indexing. The two could integrate — a Nextcloud-replacement app could use `stony-vdfs`
as its local cache layer with offline transcription — but `stony-vdfs` itself ships no
server, no UI, no multi-user model. The scope is deliberately narrow so the library
stays a dependency and not a platform.

### How is this different from Otter.ai or Descript?

Otter and Descript are commercial SaaS products that transcribe meetings and podcasts
server-side. They are excellent at what they do and they are not private: your audio is
uploaded to their data centres for processing. `stony-vdfs` is the open-source plumbing
that lets a downstream application offer comparable transcription and search features
without sending any audio off-device. You bring the model weights (whisper.cpp GGUF
files, any local LLM), the library wires up the pipeline.

### How is this different from Apple's or Google's on-device indexing?

Apple's Spotlight and Google's on-device search both index media on the device, and
both are excellent. They are also closed-source, vendor-controlled, and locked to their
respective operating systems. `stony-vdfs` is cross-platform (Linux, macOS, Windows),
open-source (MIT), and the indexing models are user-chosen. An application built on
`stony-vdfs` can ship the same on-device feature across all three desktops with a
single codebase, and audit exactly what runs.

---

## Technical

### What hardware and OS support does the transcription pipeline have?

The minimum useful configuration is whatever runs `whisper.cpp`'s `base` or `small`
GGUF model at a tolerable speed for the user's workload. In practice: any x86-64 CPU
from the last decade with AVX2, any Apple Silicon Mac (M1 or later, with Metal
acceleration), or any recent Windows machine (CPU or CUDA). The core, memory, and
local backends work on Linux, macOS, and Windows today; the FUSE adapter lands in M3,
the WinFsp adapter in M4. We test Apple Silicon and x86-64 Linux in CI for every
release. The library itself is essentially free on top — the cost is the model.

### Removed-placeholder-marker-XYZ
### What hardware do I need to run the transcription pipeline?

The minimum useful configuration is whatever runs `whisper.cpp`'s `base` or `small`
GGUF model at a tolerable speed for the user's workload. In practice that means: any
x86-64 CPU from the last decade with AVX2, or any Apple Silicon Mac (M1 or later), or
any Windows machine with a recent CPU. A discrete GPU (CUDA or Metal) makes the `medium`
and `large-v3` models comfortable in real time. The library itself is essentially free
on top — the cost is the model.

### What languages does the transcription pipeline support?

Whatever the user's `whisper.cpp` model supports. Whisper itself ships multilingual
weights covering ~100 languages with varying accuracy; the `large-v3` model is the
strongest. The library does not impose a language; the `Transcriber` trait returns a
language code in its `TranscriptRecord`, and the tagging step can be configured to keep
or translate as the caller wishes. EU-language quality is broadly comparable to
English for the larger models.

### Does this work well on Apple Silicon?

Yes. `whisper.cpp` has first-class Metal support on M1, M2, M3, and M4. The local
backend uses APFS-native APIs through `tokio::fs`, and the FUSE adapter (M3) targets
macFUSE on macOS. We test on Apple Silicon in CI for every release.

### Does this work on Windows?

Yes for the core, memory, and local backends today. The WinFsp adapter lands in
milestone M4. `whisper.cpp` runs natively on Windows (CPU or CUDA). FFmpeg is available
as a static Windows binary from upstream. The library has no Linux-specific
dependencies in the core; everything platform-specific is gated behind `cfg`
attributes inside the adapter crates.

### Will there be Python or JavaScript bindings?

Not in v1.0, and not in the funded 12-month plan. The reason is that good bindings are
a substantial undertaking — `pyo3` or `napi-rs` plus packaging plus CI plus
documentation plus user support — and the project is sized to do one thing well
(Rust + FFmpeg + Whisper + LLM on three platforms). Once v1.0 ships and the library is
stable, language bindings are a natural follow-on; contributions are very welcome.

### Can I self-host this?

The library is local-first by default — there is nothing to host. "Self-hosting" in the
SaaS sense only becomes meaningful if you build a multi-user application *on top of*
`stony-vdfs`. The library happily runs inside a server process if you want to expose a
custom HTTP API over a `VfsBackend`; nothing in the design prevents that.

---

## Project

### What is the maintenance plan after the NLnet grant ends?

The grant funds the first 12 months of intensive development (M1–M6). After v1.0, the
library is small enough by design to be maintainable in part-time mode: the public
trait surface is intentionally narrow, the test matrix is automated, and the indexing
backends (FFmpeg, whisper.cpp, ollama) are external. Post-grant maintenance covers
security patches, dependency updates, and shepherding community contributions. The
author is committed to maintaining the project for the foreseeable future irrespective
of grant outcomes.

### What is the relationship to the closed-source predecessor product?

The author developed a closed-source desktop application using related FFmpeg /
Whisper / multimodal AI techniques across four CRA-acknowledged Canadian SR&ED R&D
cycles. **None of that proprietary code is in this repository.** The library is a clean
rewrite under MIT, designed from scratch, written from the trait surface outward. The
prior work informs the design intuitions; it does not contribute source code, models,
or proprietary algorithms.

### How do I contribute?

Read [CONTRIBUTING.md](../CONTRIBUTING.md). In short: fork, branch, run
`cargo test --workspace` and `cargo clippy --workspace -- -D warnings` locally, open a
PR. Small fixes do not need prior discussion; substantial features benefit from an
issue first so we can talk about fit with the roadmap. All contributions are MIT.

### What is the security model?

See [SECURITY.md](../SECURITY.md) for vulnerability disclosure. The short version: the
library treats file contents as untrusted (hence FFmpeg as a subprocess — see ADR
0004); it makes no network calls; it has no telemetry. Encryption decorators land in
M5 with a published threat model. Reports are accepted via private email or the GitHub
security advisory mechanism.

### Is there any telemetry?

No. There is no telemetry. There is no analytics call. There is no update check. There
is no anonymised counter. The library opens zero sockets unless the calling application
wires in a backend that opens sockets, in which case the caller is responsible for that
behaviour. This is enforced by `cargo-deny` audits on every release.

### Is the library accessible?

The library has no UI of its own, so accessibility applies to downstream applications.
Command-line output from `stony-vdfs-cli` follows GNU CLI conventions (machine-readable
output via flags, exit codes, no colour unless `--color always`) so it composes
cleanly with screen-reader-friendly terminals.

### Can EU public-sector organisations deploy this?

Yes. The license is MIT, the development is open, the code is mirrored on Codeberg for
European resilience, and the privacy-by-default posture means GDPR data-flow analysis
is straightforward (no personal data leaves the host). The
[EU_COAPPLICANTS.md](EU_COAPPLICANTS.md) document tracks specific public-sector
interest. Deployment in regulated environments (healthcare, education, public records)
is an explicit design audience.

---

## Licence and governance

### Is there a trademark on "stony-vdfs"?

No. The name is unregistered. Forks may keep the name with attribution, or rename
freely. There is no legal mechanism the project would use to police naming.

### Can I fork the library?

Yes. MIT explicitly permits forking, with or without modification, for any purpose
including commercial use. The only requirement is that the MIT copyright notice
travels with the source.

### Can I bundle this in a commercial closed-source product?

Yes. MIT permits this. Ship the LICENSE file in your product's third-party-notices
section and you are compliant. We would love to hear about it (open a "showcase" issue
or a discussion) but you are not required to disclose.
