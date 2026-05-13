# ADR 0006: Persist index records as per-file sidecar JSON, not a central SQLite database

## Status

Accepted — 2026-03-03.

## Context

The indexing pipeline produces a transcript and a set of tags per media file. Those
records need to be persisted somewhere the application can query. The candidates were:

- **Per-file sidecar JSON.** One `<hash>.json` file per indexed media file, written
  under `/.index/` inside the same VFS. Self-describing, human-readable, version-able.
- **A central SQLite database.** One file at a known path, joined-query support,
  transactions, but ties the project to a single embedded engine and complicates
  encrypted-backend interactions.
- **Embedded LMDB or sled.** Faster than SQLite for simple key-value workloads, but
  opaque to users and tools, and adds a binary dependency.

The deciding consideration was **portability and inspectability**: a user must be able
to `cat` an index record without learning a query language, copy their index to another
machine by copying their files, and grep across records with standard Unix tools. None
of these are options with a binary database.

## Decision

Index records are persisted as **per-file sidecar JSON** at `/.index/<content-hash>.json`
inside the VFS. The schema is versioned (`"schema_version": 1`) so future migrations
are explicit. Applications that want fast joined queries can build their own materialised
view on top of the sidecar files; the library does not impose one.

## Consequences

Positive: the index travels with the data. Copying media files copies their indices.
Encryption decorators encrypt the sidecars exactly like any other file. Users can grep
their transcripts with `grep -r` and the result is meaningful.

Negative: queries that span thousands of records require an external materialised view.
We accept this; the library is plumbing, not a search engine. Applications that need
full-text search ship Tantivy or Meilisearch on top.
