# ADR 0002: Implement in Rust 2024 (MSRV 1.85) rather than Go or C++

## Status

Accepted — 2026-02-04.

## Context

A virtual filesystem with FUSE and WinFsp adapters, FFI to `whisper.cpp`, and an
indexing pipeline has three sharp non-functional requirements: it must be memory-safe
against untrusted file contents, it must support precise lifetime control over
mmap-backed buffers, and it must produce binaries that are easy to ship to end-user
desktops on Linux, macOS, and Windows. The realistic candidates were:

- **Rust 2024 edition.** Memory safety, mature async, first-class FFI, single static
  binary per target, well-supported FUSE and WinFsp crates.
- **Go.** Excellent cross-compilation, simple build, but GC pauses interact badly with
  kernel-callback patterns in FUSE and WinFsp, and the FFI ergonomics for `whisper.cpp`
  and FFmpeg are worse than Rust's.
- **C++.** Maximum control, mature audio/video tooling, but no memory safety against
  malformed media inputs, slow toolchain, and worse package management.

## Decision

We adopt **Rust 2024 edition** with MSRV pinned to **1.85**, the first stable release
shipping Rust 2024 features the project depends on (`let-else` in nested patterns,
stabilised `async fn` in traits behind `dyn`-compatible workarounds, improved
diagnostics).

## Consequences

Positive: memory safety against untrusted media inputs is a free property, not a
discipline. `cargo` makes cross-compilation and binary distribution straightforward, and
the FUSE / WinFsp / `whisper-cpp` crate ecosystems are healthy.

Negative: MSRV 1.85 excludes very old distributions; we accept this because the audience
is desktop applications, not embedded systems. The async-trait surface still requires
the `async_trait` macro until native AFIT becomes `dyn`-compatible — see ADR 0003.
