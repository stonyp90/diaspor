# Release notes

Release-by-release notes live here. ROADMAP.md describes the *intended*
milestones; this file is the *actual* shipping log written after each tag.

## v0.1.0-alpha.1 — 2026-05-13

Initial public release. Anchors the NLnet NGI Zero Commons Fund
application submission package.

What ships:

- `cairn-core` — async trait surface, path/error/metadata types,
  unit tests. Stable shape; minor API churn expected pre-1.0.
- `cairn-backend-memory` — reference in-memory backend. Full
  implementation; covered by the conformance suite.
- `cairn-backend-local` — local-disk backend, happy path. Cross-
  platform path edge cases land in M2.
- `cairn-fuse` — adapter stub with full trait surface. Real
  implementation in M3.
- `cairn-winfsp` — adapter stub with full trait surface. Real
  implementation in M4.
- `cairn-index` — pipeline trait surface (FFmpeg + Whisper + LLM
  tagger). No real binary calls yet; M5/M6 land them.
- `cairn-conformance` — published conformance test suite that
  third-party backends can run against their own implementations.
- `cairn-cli` — operator CLI with memory/local mount examples.

What does *not* ship yet:

- Real FFmpeg subprocess invocation (M5).
- Real whisper.cpp / llama.cpp integration (M5–M6).
- macOS / Windows CI green (Linux only in M1; cross-platform in M2+).

Known issues at this tag:

- (none material — see `git log` for any post-tag fixes).

---

Future releases will append entries above this footer, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/),
loosely; semantic versioning kicks in at v1.0.0 (M6).
