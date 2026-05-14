# Benchmarks

This file will track measured performance characteristics of the
`cairn` workspace over the milestone series.

## Status: draft, populated through M2–M6

At v0.1.0-alpha.1 the project ships only trait surfaces and reference
implementations; meaningful benchmark numbers require the real local
backend (M2), the FUSE adapter (M3), the WinFsp adapter (M4), and the
indexing pipeline (M5–M6) to land first.

## Planned benchmarks

The following measurements will land tagged to the corresponding
milestone:

### M2 — `cairn-backend-local` micro-benchmarks

- Sequential read / write throughput vs `tokio::fs` baseline.
- `list()` of directories with 10, 100, 1 000, 10 000 entries.
- Path-normalisation hot path latency (single-call cost).
- Symlink resolution overhead.

### M3 — FUSE adapter

- Mount-to-first-read latency.
- 4 KB random read IOPS against the memory backend over FUSE.
- Concurrent reader scaling on a 16-core Linux box.

### M4 — WinFsp adapter

- Equivalent of the M3 benchmarks under WinFsp.
- Comparison to native NTFS for the same workloads.

### M5 — Indexing pipeline (FFmpeg + whisper.cpp)

- End-to-end transcription latency for 1, 5, 30, 120 minute audio inputs.
- Peak RSS during streaming audio extraction (target: < 50 MB for any
  input size).
- whisper.cpp model-size vs WER trade-off, holding hardware constant.

### M6 — Tagging pipeline (local LLM)

- Tag-generation latency vs prompt length.
- Tag quality (manual rubric on a small labelled set).
- End-to-end pipeline: file in → sidecar JSON committed.

## Hardware baseline

All numbers will be reported on:

- **Linux:** Ubuntu 24.04 LTS, x86_64, 16 GB RAM, NVMe SSD.
- **macOS:** macOS 14, Apple M1 (8-core), 16 GB unified memory.
- **Windows:** Windows 11 Pro, x86_64, 16 GB RAM, NVMe SSD.

Each table will record the commit SHA, the toolchain channel, and the
exact command run, so any reviewer can reproduce the numbers.
