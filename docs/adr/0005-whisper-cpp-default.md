# ADR 0005: Default to `whisper.cpp` for on-device transcription

## Status

Accepted — 2026-02-25.

## Context

The indexing pipeline needs to turn extracted audio into a transcript. The realistic
on-device options were:

- **`whisper.cpp`** — C/C++ port of OpenAI Whisper, runs CPU-only or on GPU/Metal/CUDA,
  GGML-quantised models, MIT licensed, ships as a single binary.
- **`faster-whisper`** — Python, CTranslate2-based, fastest CPU implementation, but
  pulls in a Python runtime as a hard dependency.
- **Apple MLX Whisper** — fastest on Apple Silicon, but macOS-only and tied to the MLX
  framework's lifecycle.
- **OpenAI Whisper API** — server-side, fast and accurate, but defeats the entire
  privacy premise of the project.

The library has to work on Linux, macOS (both Intel and Apple Silicon), and Windows,
without requiring a Python interpreter, without requiring a network connection, and
without locking users into a single hardware vendor.

## Decision

The default `Transcriber` implementation in `diaspor-index` wraps **`whisper.cpp`**.
The wrapper invokes the `whisper-cli` binary as a subprocess (see ADR 0004 for the
analogous reasoning) and parses its JSON output. The `Transcriber` trait is public, so
callers can plug in `faster-whisper`, MLX, or even the OpenAI API if they choose to
trade privacy for accuracy — the library does not make that choice for them.

## Consequences

Positive: works on all three platforms with no Python dependency. Apple Silicon users
get Metal acceleration automatically. CUDA users get GPU acceleration automatically.
Models are user-supplied GGUF files, so accuracy and disk footprint are knobs.

Negative: `whisper.cpp` is not the fastest implementation on every platform. Users who
want maximum throughput on Apple Silicon can swap to MLX without forking the library.
