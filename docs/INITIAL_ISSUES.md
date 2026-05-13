# Initial GitHub Issues to Open

These 15 issues seed the public roadmap. Open them as **GitHub issues** on
`github.com/stonyp90/stony-vdfs` right after the initial push so the repo
looks like an active project, not a one-shot drop. Suggested labels in
square brackets — create the labels first via `gh label create` or the
GitHub UI.

Labels to create (one-time):

```
help wanted          colour #008672
good first issue     colour #7057ff
docs                 colour #0075ca
M1                   colour #B60205
M2                   colour #D93F0B
M3                   colour #FBCA04
M4                   colour #0E8A16
M5                   colour #1D76DB
M6                   colour #5319E7
infrastructure       colour #c5def5
```

---

## Issue 1 — Define conformance test suite scope

**Labels:** `M1`, `help wanted`, `good first issue`

The conformance suite in `stony-vdfs-conformance` must exercise every
method on `VfsBackend` against every meaningful `OpenFlags` combination.
Today the trait surface is set; we need an explicit checklist of
behaviours each backend must demonstrate (read-after-write, partial
writes, EOF semantics, concurrent open/read, etc.).

**Acceptance criteria:** a markdown spec in `crates/stony-vdfs-conformance/SPEC.md`
listing each invariant and the test function that proves it.

---

## Issue 2 — Memory backend: implement extended attributes

**Labels:** `M1`

`stony-vdfs-backend-memory` currently has no xattr support. The trait
needs to be extended (or a sibling trait introduced) so that backends
that support xattrs can expose them.

**Acceptance criteria:** Trait method, memory implementation, conformance
test, doc note in `ARCHITECTURE.md`.

---

## Issue 3 — Document the `VfsPath` invariants

**Labels:** `docs`, `good first issue`, `M1`

`VfsPath` enforces several invariants (always absolute, no `..` components,
NFC-normalised). These need a rustdoc page that says it explicitly so
downstream developers don't have to read the source.

**Acceptance criteria:** `path` module-level rustdoc paragraph + one
unit test per invariant.

---

## Issue 4 — Local backend: cross-platform symlink behaviour matrix

**Labels:** `M2`

POSIX and Windows treat symlinks differently. We need a behaviour matrix
(create, follow, raw stat, remove) and a deliberate policy in
`stony-vdfs-backend-local`.

**Acceptance criteria:** matrix in docstring + tests on each platform in CI.

---

## Issue 5 — Local backend: handle long paths on Windows (`\\?\` prefix)

**Labels:** `M2`

Windows defaults cap path length at 260 chars unless the long-path
manifest is set *and* paths are prefixed with `\\?\`. The local backend
should wrap paths transparently.

**Acceptance criteria:** integration test on Windows CI with a 280-char path.

---

## Issue 6 — FUSE adapter: scaffold against `fuser` 0.14

**Labels:** `M3`

`stony-vdfs-fuse` today is a stub. Wire it against the `fuser` crate
and implement `getattr` / `readdir` / `read` end-to-end against the
memory backend so we have a "hello world" mount.

**Acceptance criteria:** `cargo run --example fuse-memory-mount` mounts
a memory backend at `/tmp/stony-vdfs-demo` and `ls` works.

---

## Issue 7 — FUSE adapter: macOS support via macFUSE / fuse-t

**Labels:** `M3`, `help wanted`

macFUSE has licence friction; `fuse-t` is the FOSS alternative. Pick a
target and document the choice.

**Acceptance criteria:** ADR in `docs/adr/0001-macos-fuse.md`.

---

## Issue 8 — WinFsp adapter: feasibility spike

**Labels:** `M4`

`winfsp-rs` exists but is sparsely maintained. Spike a 1-day attempt at
wiring it to `VfsBackend` and write up findings, including any blocker
that justifies an alternative (e.g. raw FFI).

**Acceptance criteria:** spike branch + writeup in `docs/spikes/winfsp.md`.

---

## Issue 9 — Index pipeline: FFmpeg probe via `ffprobe -of json`

**Labels:** `M5`

Replace the no-op probe in `stony-vdfs-index` with a real
`ffprobe -of json` subprocess that fills `MediaInfo`.

**Acceptance criteria:** probing a 10-second wav, mp3, mp4, and mkv
produces the right `container`, `audio_codec`, `audio_sample_rate`.

---

## Issue 10 — Index pipeline: stream audio extraction without buffering full file

**Labels:** `M5`

The pipeline must extract 16 kHz mono PCM from arbitrarily large files
**without** loading the whole input into memory. Build the extractor as
an async byte stream.

**Acceptance criteria:** processing a 2 GB MP4 uses < 50 MB peak heap.

---

## Issue 11 — Transcriber: bind `whisper.cpp` via `whisper-rs`

**Labels:** `M5`

Default `Transcriber` implementation: wrap `whisper-rs` (or call the
`whisper-cli` binary as a fallback), accept a `Model` path, support at
least the `tiny`, `base`, and `small` GGML/GGUF models.

**Acceptance criteria:** `cargo run --example transcribe-demo` on a
30-second WAV produces a transcript and segment timestamps.

---

## Issue 12 — Tagger: bind a small GGUF model via `llama.cpp` for tag generation

**Labels:** `M6`

Default `Tagger` implementation: prompt a small instruction-tuned model
(e.g. Qwen 1.5 1.8B, Phi 3 Mini) to return JSON `{tags, categories, summary}`.

**Acceptance criteria:** end-to-end demo on the M5 sample produces a
plausible tag set; no network call observed.

---

## Issue 13 — Sidecar persistence inside the VFS

**Labels:** `M6`

When the pipeline produces a `SidecarRecord` for `/foo.mp4`, persist it
to `/.index/foo.mp4.json` *through the same backend*, so the sidecar
survives backend switches.

**Acceptance criteria:** writing then reading a sidecar round-trips
identically on memory and local backends.

---

## Issue 14 — Privacy assertion test: no network calls in the default pipeline

**Labels:** `M6`, `infrastructure`

Add a CI job that runs the index pipeline inside a network-disabled
sandbox (e.g. `unshare -n`) and confirms the default path completes
successfully — proving by construction that no cloud call is made.

**Acceptance criteria:** CI green; if any default path opens a socket,
job fails.

---

## Issue 15 — v1.0 release checklist

**Labels:** `M6`

Pre-1.0 freeze: write the release checklist, finalise the CHANGELOG,
publish all crates to crates.io, tag `v1.0.0`, write the announcement
blog post.

**Acceptance criteria:** all checklist items complete + 1.0 tag pushed.
