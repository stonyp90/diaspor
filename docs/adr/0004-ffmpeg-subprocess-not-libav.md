# ADR 0004: Call FFmpeg as a subprocess rather than binding `libav-sys`

## Status

Accepted — 2026-02-19.

## Context

The indexing pipeline needs to probe media containers and extract audio tracks for
transcription. Two ways to use FFmpeg from Rust:

- **Link against `libavformat`/`libavcodec`** via the `ffmpeg-sys-next` family of
  crates. Faster (no process boundary), no PATH dependency, in-process error handling.
- **Spawn the `ffmpeg` binary** as a subprocess and read its stdout. Slower per
  invocation, requires `ffmpeg` on PATH, but trivially safe against malformed inputs
  and trivially upgradable.

The tradeoff hinges on two facts: malformed media files are an enormous attack surface,
and `libav*` has a CVE history measured in the hundreds. Even with Rust's memory safety
on our side of the FFI, an out-of-bounds read inside libavcodec compromises the host
process. A subprocess crash, by contrast, surfaces as an `ExitStatus` we can handle.

## Decision

The default `FfmpegExtractor` shipped in `cairn-index` calls **`ffmpeg` as a
subprocess** via `tokio::process::Command`. Implementations that prefer linking against
`libav` may write their own `MediaExtractor` impl; the trait is public.

## Consequences

Positive: a memory-corruption bug in any FFmpeg codec stays inside the subprocess. Users
can pin or upgrade `ffmpeg` independently of `cairn` releases. The build does not
need a C toolchain for FFmpeg headers. Distro packagers do not have to vendor codec
sources.

Negative: process-spawn overhead per indexed file (measured at ~20 ms on modern Linux).
For batches of thousands of files this matters; the pipeline amortises it by reusing a
single subprocess for multi-file probe operations where possible.
