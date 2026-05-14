# Architecture Decision Records

This directory captures the load-bearing decisions made during the design of
`cairn`. Each record uses [Michael Nygard's ADR format][nygard]: Title, Status,
Context, Decision, Consequences. ADRs are immutable once accepted; later decisions that
supersede an earlier one are filed as new ADRs that reference the original.

[nygard]: https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions

## Index

| #    | Title                                                                                   | Summary                                                                |
|------|-----------------------------------------------------------------------------------------|------------------------------------------------------------------------|
| 0001 | [License under MIT rather than Apache-2.0 or AGPL-3.0](0001-license-mit.md)             | MIT chosen for maximum downstream compatibility, commercial and open.  |
| 0002 | [Implement in Rust 2024 (MSRV 1.85) rather than Go or C++](0002-rust-2024-edition.md)   | Memory safety against untrusted media inputs is a free property.       |
| 0003 | [Async-first trait surface using `async_trait`](0003-async-trait-surface.md)            | `Arc<dyn VfsBackend>` works today; AFIT migration deferred to post-M4. |
| 0004 | [Call FFmpeg as a subprocess rather than binding libav-sys](0004-ffmpeg-subprocess-not-libav.md) | Process boundary contains codec memory bugs; users can upgrade ffmpeg independently. |
| 0005 | [Default to whisper.cpp for on-device transcription](0005-whisper-cpp-default.md)       | Cross-platform, no Python, swappable via the public `Transcriber` trait. |
| 0006 | [Per-file sidecar JSON, not a central SQLite database](0006-sidecar-json-not-sqlite.md) | Index travels with data; grep-able; encrypts transparently with VFS.   |

## Proposing a new ADR

1. Copy the most recent ADR as a template.
2. Increment the number, write the file, and open a pull request.
3. The status starts as **Proposed**; reviewers debate in the PR.
4. On merge, status becomes **Accepted** and the file is immutable.

Superseding a decision: file a new ADR, set its status to **Accepted**, and update the
superseded ADR's status to `Superseded by NNNN`. Do not delete or rewrite the original
text — the project's history is part of the documentation.
