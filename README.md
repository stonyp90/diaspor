# diaspor

> A privacy-first virtual filesystem for Rust with **local AI transcription and
> auto-tagging built in**. Your media library understands itself — without sending a
> byte to the cloud.

[![License: AGPL--3.0-only](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024_edition-orange.svg)](https://www.rust-lang.org)

<!-- CI badge intentionally omitted until the repository is live on GitHub
and the first workflow run reports green. Re-add once stable:
[![CI](https://github.com/stonyp90/diaspor/actions/workflows/ci.yml/badge.svg)](https://github.com/stonyp90/diaspor/actions/workflows/ci.yml)
-->


`diaspor` is a Rust workspace that combines two ideas that usually live apart:

1. a **backend-agnostic virtual filesystem** that gives applications a single async API
   over memory, local disk, FUSE mounts, and WinFsp mounts;
2. an opt-in **content-aware indexing layer** that, when a media file lands in the
   filesystem, automatically:
   - probes it with **FFmpeg**,
   - extracts the audio track,
   - **transcribes** it with `whisper.cpp` (or any other on-device backend you plug in),
   - **auto-tags** the transcript with a small local LLM (no cloud calls),
   - stores the transcript + tags as a sidecar JSON record retrievable through the
     filesystem itself.

The result: a desktop or server application can hand its users `cat`, `grep`, `find`
over the *contents* of their videos and podcasts, with zero data leaving the device.

The library is written from a clean specification in Rust 2024 edition under AGPL-3.0.
It is being released openly to support a [NGI Zero Commons Fund](https://nlnet.nl/commonsfund/)
application; once the grant cycle clears, sustained 50%-FTE development through
v1.0 is the plan.

---

## Status

**v0.1.0-alpha** — public scaffolding. The traits and the in-memory backend are usable;
the local backend covers the happy path; FUSE, WinFsp, and the indexing pipeline (`crates/diaspor-index/`) are scaffolded with a complete trait surface but the
heavy implementations land in roadmap milestones M3–M6. See [ROADMAP.md](ROADMAP.md).

## Why does this matter?

Cloud providers (Google Drive, OneDrive, Dropbox, iCloud) all index user media
**server-side**: your audio is uploaded, transcribed, indexed, and tagged in their data
centres so their search box can find "the meeting where we discussed pricing." That is
a useful feature — and an enormous privacy trade-off.

`diaspor` lets a downstream application offer the same user experience without ever
sending the audio anywhere. The transcription and tagging run on the user's own machine
via FFmpeg + `whisper.cpp` + a small local LLM. The library is the plumbing; the
application picks the models and policies.

This aligns with the
[NGI Zero Commons Fund](https://nlnet.nl/commonsfund/) mission of building infrastructure
for the digital commons that respects user sovereignty and does not rely on Big Tech
intermediaries.

## What's in the box

| Crate                          | Purpose                                                  |
|--------------------------------|----------------------------------------------------------|
| `diaspor-core`              | Traits, types, paths, errors — no IO dependencies.       |
| `diaspor-backend-memory`    | In-memory backend for tests and demos.                   |
| `diaspor-backend-local`     | Local filesystem backend (POSIX + Windows).              |
| `diaspor-fuse`              | FUSE mount adapter (Linux/macOS). **Stub until M3.**     |
| `diaspor-winfsp`            | WinFsp mount adapter (Windows). **Stub until M4.**       |
| `diaspor-index`             | **FFmpeg + transcription + auto-tag pipeline.** Trait surface today; full implementation M5–M6. |
| `diaspor-cli`               | Operator CLI: `list`, `cat`, `put` against any backend.  |

## Architecture at a glance

```
  ┌───────────────────────────────────────────────────────────────────────┐
  │                            diaspor-cli                             │
  └───────────────────────┬───────────────────────┬───────────────────────┘
                          │                       │
                          ▼                       ▼
              ┌──────────────────┐    ┌────────────────────────────────┐
              │  Indexing layer  │    │           Backends             │
              │  (decorator)     │    │ ┌──────┐ ┌──────┐ ┌──────────┐ │
              │ ┌──────────────┐ │    │ │memory│ │local │ │ future:  │ │
              │ │  FFmpeg      │ │    │ └──────┘ └──────┘ │ cloud/CAS│ │
              │ │  probe+audio │ │    │                   └──────────┘ │
              │ └──────┬───────┘ │    └────────────────┬───────────────┘
              │        ▼         │                     │
              │ ┌──────────────┐ │                     │
              │ │ Transcriber  │ │       wraps         │
              │ │ (whisper.cpp)│ │ ───────────────────▶│
              │ └──────┬───────┘ │                     │
              │        ▼         │                     │
              │ ┌──────────────┐ │                     │
              │ │   Tagger     │ │                     │
              │ │ (local LLM)  │ │                     │
              │ └──────┬───────┘ │                     │
              │        ▼         │                     │
              │ sidecar JSON     │                     │
              │ /.index/*.json   │                     │
              └──────────────────┘                     │
                                                       ▼
                                          ┌────────────────────────┐
                                          │   diaspor-core      │
                                          │ traits / paths / types │
                                          └────────┬───────────────┘
                                                   │
                                ┌──────────────────┼──────────────────┐
                                │                                     │
                       ┌────────▼─────────┐                ┌──────────▼──────────┐
                       │  FUSE adapter    │                │   WinFsp adapter    │
                       │  (Linux/macOS)   │                │     (Windows)       │
                       │    [M3 stub]     │                │      [M4 stub]      │
                       └──────────────────┘                └─────────────────────┘
```

Full design notes in [ARCHITECTURE.md](ARCHITECTURE.md).

## Design philosophy

- **Privacy-by-default.** No telemetry. No implicit network calls. The transcription and
  tagging models run on the user's device unless the caller explicitly chooses
  otherwise.
- **Async-first.** Built on `tokio`; every IO method returns a future.
- **Cross-platform parity.** Identical behaviour on Linux, macOS, Windows; backends own
  the platform quirks.
- **Small core, big edges.** `diaspor-core` has no IO dependencies — only traits.
- **Composability over inheritance.** Indexing, encryption, dedup attach as decorators
  around any backend.
- **Bring-your-own-model.** The library ships traits, not weights. Callers point the
  pipeline at their preferred `whisper.cpp` build, GGUF model, or LLM runtime.

## Install the desktop app

Prebuilt installers for the multi-tier cloud file browser (Tauri 2 frontend
over the diaspor VFS) are published on the [GitHub releases page][releases].

| Platform | File | Requirements |
|----------|------|--------------|
| **macOS** (Apple Silicon + Intel, universal) | `diaspor-vfs.dmg` | macOS 10.15+ |
| **Windows** | `diaspor-vfs.msi` | Windows 10/11 |
| **Linux** | `diaspor-vfs.AppImage` | glibc 2.31+ (Ubuntu 22.04+, Fedora 36+) |

The macOS .dmg is unsigned at pre-1.0 alpha. Gatekeeper will warn on first
launch — right-click the app and choose "Open" once, or run
`xattr -d com.apple.quarantine /Applications/Diaspor.app` from the
Terminal. Signing + notarization will land before the first non-alpha tag
(see `.github/workflows/release-desktop.yml` for the wired-but-dormant
APPLE_SIGNING_IDENTITY path).

State (settings, audit log, cache) lives under the standard XDG paths:
`~/.config/diaspor`, `~/.local/share/diaspor`, and `~/.cache/diaspor` on
Linux, with platform equivalents on macOS / Windows. Anyone upgrading from
v0.1.0-alpha.4 or earlier (when the brand was Ursly) is migrated
automatically on first launch — the old `<dir>/ursly` directories are
renamed to `<dir>/diaspor` if the new path doesn't already exist.

[releases]: https://github.com/stonyp90/diaspor/releases/latest

## Quick start — plain VFS

```toml
[dependencies]
diaspor-core = "0.1.0-alpha.6"
diaspor-backend-memory = "0.1.0-alpha.6"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
use diaspor_backend_memory::MemoryBackend;
use diaspor_core::{OpenFlags, VfsBackend, VfsPath};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = MemoryBackend::new();
    let path = VfsPath::new("/hello.txt").ok_or("invalid path")?;

    let mut h = backend.open(&path, OpenFlags::CREATE | OpenFlags::WRITE).await?;
    h.write(0, b"hello, world\n").await?;
    h.flush().await?;

    for entry in backend.list(&VfsPath::root()).await? {
        let m = backend.metadata(&entry).await?;
        println!("{entry}  ({} bytes, {})", m.size, m.kind.as_str());
    }
    Ok(())
}
```

## Indexing pipeline preview (M5/M6)

The trait surface is already public so downstream projects can start designing against
it. The full pipeline ships in roadmap milestones M5 and M6:

```rust
use diaspor_index::{ContentPipeline, MediaExtractor, Tagger, Transcriber};
// (Once M5/M6 land, the default impls land in feature-gated submodules.)

// Wire your own implementations or use the bundled defaults (planned):
//   - FfmpegExtractor   — wraps the `ffmpeg` binary
//   - WhisperCppTranscriber — wraps whisper.cpp via ffi
//   - OllamaTagger     — talks to a local ollama daemon
let pipeline = ContentPipeline {
    extractor:   /* FfmpegExtractor::new()      */ todo!(),
    transcriber: /* WhisperCppTranscriber::new()*/ todo!(),
    tagger:      /* OllamaTagger::new("llama3") */ todo!(),
};

let record = pipeline.process(&path, &bytes_of_an_mp4).await?;
println!("transcript snippet: {}", &record.transcript.text[..200]);
for tag in &record.tags.tags {
    println!("#{tag}");
}
```

## Supported platforms

| Platform   | Core | Memory | Local | FUSE        | WinFsp      | Index pipeline        |
|------------|:----:|:------:|:-----:|:-----------:|:-----------:|:----------------------|
| Linux      |  ✅  |   ✅   |  ✅   | M3 (planned)|     —       | M5/M6 (planned)       |
| macOS      |  ✅  |   ✅   |  ✅   | M3 (planned)|     —       | M5/M6 (planned)       |
| Windows    |  ✅  |   ✅   |  ✅   |      —      | M4 (planned)| M5/M6 (planned)       |

## Building from source

```bash
git clone https://github.com/stonyp90/diaspor.git
cd diaspor
cargo build --workspace
cargo test --workspace
cargo run -p diaspor-cli -- --help
```

Minimum supported Rust version: **1.85** (Rust 2024 edition).

For the indexing pipeline (M5+), you will additionally need `ffmpeg` available on `PATH`
and a `whisper.cpp` binary or compatible model runtime. The library itself does not
bundle either — those are the user's choice.

## Roadmap

Six milestones across 12 months — see [ROADMAP.md](ROADMAP.md) for the full breakdown.

| ID  | Theme                                                  | Target month |
|-----|--------------------------------------------------------|--------------|
| M1  | Core traits + memory backend stable                    | Month 1–2    |
| M2  | Local backend meets acceptance criteria + benchmarks   | Month 3–4    |
| M3  | FUSE adapter end-to-end (Linux + macOS)                | Month 5–6    |
| M4  | WinFsp adapter end-to-end                              | Month 7–8    |
| **M5**  | **FFmpeg + Whisper transcription pipeline shipped**| Month 9–10   |
| **M6**  | **Local-LLM auto-tagging + sidecar persistence + v1.0** | Month 11–12 |

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the development
workflow, coding standards, and PR process. All participants agree to the
[Code of Conduct](CODE_OF_CONDUCT.md) (Contributor Covenant 2.1).

The repository is mirrored on [Codeberg](https://codeberg.org/stonyp90/diaspor) for
European visibility and resilience.

## Funding

The 12-month development plan is under review for the
[NLnet NGI Zero Commons Fund](https://nlnet.nl/commonsfund/). If funded, that
acknowledgement will appear here and in release notes.

The author was supported during the design phase by Canadian SR&ED tax credits
(four active claims documenting prior R&D effort on related private projects involving
FFmpeg pipelines, Whisper integration, and multimodal AI). The code in this repository
is new, written from scratch under AGPL-3.0, and is not encumbered by any prior employer, IP
licence, or consortium agreement.

## Recognized contributors

Today this is a single-maintainer project — but the goal is for that to change. Every
person who lands a commit appears on the
[contributors graph](https://github.com/stonyp90/diaspor/graphs/contributors) and
is named in CHANGELOG.md for the release that ships their work.

Want to be on this list? See [CONTRIBUTING.md](CONTRIBUTING.md) — even a typo fix counts.

## Licence

[AGPL-3.0](LICENSE) © 2026 Anthony Paquet
