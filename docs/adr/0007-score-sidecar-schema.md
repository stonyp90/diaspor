# ADR 0007: Stream-window score records get their own schema, not a sidecar v2

## Status

Accepted — 2026-05-15.

## Context

The `diaspor-vision` + `diaspor-events` pipeline (planned for milestones M7–M9)
needs a stable JSON shape for the records it emits per analyzed time window of a
media stream. These records carry per-modality outputs (pose, face landmarks,
prosody, credibility signals, sport-judge score) plus stream identity (`stream_id`,
`tenant`, `t_start_ms`, `t_end_ms`) and model provenance.

Two candidates considered:

- **Extend `sidecar-v1.json` to v2** by adding a `modalities` block and the
  stream-window identity fields. One schema, one stabilization commitment.
- **Publish a new `score-v1.json`** alongside the existing `sidecar-v1.json`.
  Two schemas that coexist; sidecar describes a file, score describes a window
  of a stream.

The records differ along three load-bearing dimensions:

1. **Identity.** Sidecar records are keyed by `path` (one record per file). Score
   records are keyed by `(stream_id, tenant, t_start_ms, t_end_ms)` — many records
   per stream, none of which describes a single file.
2. **Cadence.** Sidecar records are written once per file ingest. Score records
   are written once per window (typically every second) plus once per
   threshold-crossing event. The serialized payloads, retention policies, and
   downstream consumers diverge accordingly.
3. **Privacy posture.** The credibility modality carries disclosure fields
   (`human_baseline_disclosed`, `ceiling_disclosed`, `vertical_attestation`,
   `labs_preview`) that have no analogue in transcript+tags records. Folding them
   into sidecar v2 would force every sidecar consumer to reason about
   credibility-specific compliance metadata even when they only care about
   transcripts.

## Decision

Publish a **new `docs/schema/score-v1.json` schema**, distinct from
`sidecar-v1.json`. Both schemas use `"schema_version": "1"` in their respective
namespaces — the version number is per-schema, not per-repo. Sidecar v1 stays
stable; if it ever needs to evolve, it gets its own ADR.

VFS placement convention:

- Sidecars: `/.index/<path>.json` (one record per indexed file)
- Score windows: `/.streams/<stream_id>/windows/<timestamp>.score.json`
- Score events: `/.streams/<stream_id>/events/<timestamp>.event.json` (same
  shape as windows, `kind: "event"` discriminator)

The in-memory Rust types live in `crates/diaspor-vision/src/record.rs` (the
window aggregate) and `crates/diaspor-events/src/event.rs` (the threshold-event
variant). They both serialize to the same on-disk schema; the `kind` field
discriminates.

## Consequences

Positive: existing sidecar v1 tooling (`grep`, scripts, downstream indexers)
keeps working without retraining on a new shape. Compliance-sensitive fields
that only matter for credibility outputs (ceiling disclosure, vertical
attestation) live where they're meaningful and don't pollute the transcript
record type. The two schemas can evolve at independent cadences; sidecar
stability is decoupled from the much-faster-moving score pipeline.

Negative: consumers that want a unified view across both (e.g. "all metadata
known about file `foo.mp4` plus any live-stream scores associated with it") must
join across two record types. We accept this; the cross-join is uncommon and
straightforward when needed, and the alternative (one bloated schema) is worse.

A small risk is naming confusion — there is `sidecar-v1` and `score-v1`, both
at "v1", but they are different shapes. The schema `$id` and `title` fields make
the distinction explicit at the JSON level; the file path makes it explicit at
the repo level. Tools that load by `$id` will not mix them up. Tools that load
by file path are already obliged to know which they want.
