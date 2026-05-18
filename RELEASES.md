# Release notes

Release-by-release notes live here. ROADMAP.md describes the *intended*
milestones; this file is the *actual* shipping log written after each tag.

## v0.1.0-alpha.6 — pending

Polish pass on top of alpha.5:

- CI/CD hardening: `release.yml` publish job + `release-desktop.yml` attach
  job now tolerate a single matrix entry's `Post Cache` post-step flaking
  (we hit this once on alpha.1's Windows runner). Real failures still
  bubble up via an explicit artifact-count guard.
- macOS signing path wired in `release-desktop.yml`: when the
  `APPLE_CERTIFICATE` / `APPLE_SIGNING_IDENTITY` / `APPLE_ID` /
  `APPLE_PASSWORD` / `APPLE_TEAM_ID` secrets are configured, the .dmg
  ships signed + notarized; falls back cleanly to an unsigned build with
  a warning annotation when they're not.
- v0.1.0-alpha.4 → v0.1.0-alpha.5 brand-directory migration: vfs-desktop
  runs `migrate_config::migrate_ursly_to_diaspor` once at startup and
  moves `$XDG_*/ursly` → `$XDG_*/diaspor` for any user upgrading from a
  pre-rebrand install.

## v0.1.0-alpha.5 — 2026-05-18

Full Diaspor brand sweep through the vfs-desktop binary:

- Rust crate `ursly-vfs` → `diaspor-vfs`, lib `diaspor_vfs_lib`.
- Filesystem struct `UrslyFS` → `DiasporFS` (~20 callsites).
- Config / cache / clipboard dirs renamed from `.../ursly` to
  `.../diaspor`.
- Default mount: `/Volumes/Ursly` → `/Volumes/Diaspor`, `U:\` → `D:\`,
  `~/ursly-vfs` → `~/diaspor-vfs`; mount label `"Cairn"` → `"Diaspor"`.
- Env var prefix `URSLY_` → `DIASPOR_`; S3 test bucket default
  `ursly-vfs-test-1766795299` → `diaspor-vfs-test`.

Preserved: `*.ursly.io` URLs in the Tauri CSP, since the deployed
production infrastructure still uses those hostnames.

## v0.1.0-alpha.4 — 2026-05-18

Bug fix for cross-platform builds. The vfs-desktop `resolve_brew_path`
helper was gated on `#[cfg(target_os = "macos")]` but called from six
sites in `commands.rs` and `use_cases/ai.rs` without their own cfg guard,
so Linux + Windows builds failed with E0425. Added a `#[cfg(not(target_os
= "macos"))]` stub that returns `None`. v0.1.0-alpha.3 shipped without
the desktop binaries on the release page because of this bug;
v0.1.0-alpha.4 is the first tag with `diaspor-vfs.{dmg,msi,AppImage}` on
the release.

## v0.1.0-alpha.3 — 2026-05-18

Added the vfs-desktop Tauri app to the repository under `apps/vfs-desktop`,
along with `.github/workflows/release-desktop.yml` (mirrors the
diaspor-agent pattern: fires on `v*` tag push, builds .dmg/.msi/.AppImage
across macOS/Windows/Linux). Bumped workspace version to keep binary
`--version` aligned with the tag.

Desktop release on this tag was incomplete — only the macOS DMG was
uploaded as a workflow artifact; the Linux + Windows builds failed (see
alpha.4 for the fix). The CLI archives shipped cleanly.

## v0.1.0-alpha.2 — 2026-05-18

Extended `release.yml`'s api-server matrix to all five CLI targets so
the API server ships on the same five platforms as the CLI (macOS
arm64/x86_64, Linux x86_64/arm64, Windows x86_64). v0.1.0-alpha.1 had
the api-server on Linux + macOS arm64 only.

## v0.1.0-alpha.1 — 2026-05-13

Initial public release. Anchors the NLnet NGI Zero Commons Fund
application submission package.

What ships:

- `diaspor-core` — async trait surface, path/error/metadata types,
  unit tests. Stable shape; minor API churn expected pre-1.0.
- `diaspor-backend-memory` — reference in-memory backend. Full
  implementation; covered by the conformance suite.
- `diaspor-backend-local` — local-disk backend, happy path. Cross-
  platform path edge cases land in M2.
- `diaspor-fuse` — adapter stub with full trait surface. Real
  implementation in M3.
- `diaspor-winfsp` — adapter stub with full trait surface. Real
  implementation in M4.
- `diaspor-index` — pipeline trait surface (FFmpeg + Whisper + LLM
  tagger). No real binary calls yet; M5/M6 land them.
- `diaspor-conformance` — published conformance test suite that
  third-party backends can run against their own implementations.
- `diaspor-cli` — operator CLI with memory/local mount examples.

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
