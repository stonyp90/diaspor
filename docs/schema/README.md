# Sidecar JSON schema

This directory hosts the **forward-looking** JSON Schema definitions for
artifacts produced by `diaspor-index`. It is published in the v0.1.0-alpha.1
release to anchor the stability contract referenced in the project's NLnet
application (§10: "Stable JSON schema for `SidecarRecord` published in-repo at
`docs/schema/sidecar-v1.json`").

## Status at v0.1.0-alpha.1

- The Rust type `diaspor_index::sidecar::SidecarRecord` exists at the
  trait-surface level but no production transcriber / tagger writes it yet.
  M5 lands the FFmpeg + `whisper.cpp` pipeline; M6 lands the local-LLM tagger
  and the sidecar persistence layer that produces JSON conforming to the
  schema below.
- The schema in `sidecar-v1.json` is the **target shape** for that work. We
  publish it now so that:
  1. Downstream tooling (indexers, search UIs, backup utilities) can be
     designed against a fixed contract from day one.
  2. Anyone forking the project to write their own transcriber implementation
     can ship records that the canonical readers will accept.
  3. The §10 NLnet roadmap claim is independently verifiable from a
     v0.1.0-alpha.1 checkout — the file exists, is documented, and is linked
     from the workspace `ROADMAP.md`.
- Until M6 ships, the schema is **provisional**: minor field additions and
  documentation changes may occur as a result of M5 implementation experience.
  After v1.0 (end of M6), the schema is frozen and changes follow the
  versioning rules in `versioning` below.

## Files

| File              | Purpose                                                                 |
|-------------------|-------------------------------------------------------------------------|
| `sidecar-v1.json` | JSON Schema draft 2020-12 definition for sidecar records (one per file). |
| `score-v1.json`   | JSON Schema draft 2020-12 definition for score records (one per stream window). Forward-looking for M7+. See [ADR 0007](../adr/0007-score-sidecar-schema.md) for the decision to keep this schema separate from sidecar. |
| `README.md`       | This file — overview, stability contract, and validation instructions.  |

## Versioning

A new `schema_version` integer (encoded as a string for cross-language
ergonomics) is introduced whenever a breaking change is made:

- **Adding** an optional field is **not** a breaking change. Readers must
  ignore unknown fields (see `P4` in
  [`crates/diaspor-conformance/SPEC.md`](../../crates/diaspor-conformance/SPEC.md#m6--sidecar-persistence-and-v10)).
- **Renaming or removing** a field, **changing its type**, or **tightening a
  validation rule** is a breaking change and requires a new schema version
  file (e.g. `sidecar-v2.json`). The old schema file stays in this directory
  so historic records can still be validated.
- Patch-level documentation tweaks to an already-published schema file (typo
  fixes, clarifying comments, additional examples) are tracked in
  `RELEASES.md`.

## Validating a record

Any JSON Schema 2020-12 validator works. Examples:

```bash
# Python (jsonschema 4.x)
pip install jsonschema
python -c "import json, jsonschema; \
  schema = json.load(open('docs/schema/sidecar-v1.json')); \
  record = json.load(open('record.json')); \
  jsonschema.validate(record, schema); \
  print('OK')"
```

```bash
# Node (ajv 8.x)
npm install ajv ajv-formats
node -e "const Ajv = require('ajv').default; \
  const addFormats = require('ajv-formats'); \
  const ajv = new Ajv({strict: false}); addFormats(ajv); \
  const schema = require('./docs/schema/sidecar-v1.json'); \
  const record = require('./record.json'); \
  console.log(ajv.validate(schema, record) ? 'OK' : ajv.errors);"
```

In Rust, the canonical path is to use `serde_json::from_str::<SidecarRecord>`
once `diaspor-index` exposes `serde` derives on `SidecarRecord` (M6
deliverable). Until then, treat the schema as the source of truth.

## See also

- [`crates/diaspor-index/src/sidecar.rs`](../../crates/diaspor-index/src/sidecar.rs)
  for the in-memory Rust type that this schema mirrors.
- [`ROADMAP.md`](../../ROADMAP.md) M5 / M6 sections for the milestones that
  bring the schema from "published, not yet enforced" to "frozen and round-
  trip tested in CI."
- [`crates/diaspor-conformance/SPEC.md`](../../crates/diaspor-conformance/SPEC.md)
  for the conformance invariants that will lock the schema into the test
  suite once M6 ships.
