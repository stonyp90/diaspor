# ADR 0003: Async-first trait surface, using `async_trait` until AFIT is `dyn`-compatible

## Status

Accepted — 2026-02-12. Re-evaluation scheduled for v0.4 (post-M4).

## Context

`VfsBackend` is the public trait through which every application uses the library. It
must support:

- being held as `Arc<dyn VfsBackend>` so application code is not forced to be generic
  over the backend type;
- defining `async fn` methods so backends can perform real IO without blocking the
  runtime;
- being object-safe across Rust 1.85+ stable.

As of MSRV 1.85, native `async fn` in traits (AFIT) is stable but the resulting traits
are not yet straightforwardly `dyn`-compatible without workarounds. The `async_trait`
macro from the `async-trait` crate solves this by desugaring `async fn` into methods
that return `Pin<Box<dyn Future + Send>>`, at the cost of one heap allocation per call.

## Decision

The public trait surface uses **`#[async_trait]` macro-based async**. Specifically:

- All public traits in `stony-vdfs-core` are annotated with `#[async_trait]`.
- All blanket impls and decorator backends follow the same pattern.
- The boxing cost is documented as dominated by IO cost in realistic workloads.

## Consequences

Positive: `Arc<dyn VfsBackend>` works without ceremony. Application code is not generic
over the backend; testing with `MemoryBackend` and shipping with `LocalBackend` is a
one-line config change. The pattern is broadly understood in the Rust async community.

Negative: one heap allocation per call. We accept this. When native AFIT becomes
`dyn`-compatible on stable (tracked in `rust-lang/rust` issue stream), we will migrate
in a single semver-major release with the `async_trait` macro retained as a
non-breaking shim for downstream callers.
