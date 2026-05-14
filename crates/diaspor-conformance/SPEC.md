# Diaspor Conformance Specification (v0.1.0-alpha.1)

This document is the authoritative list of invariants that every implementation of
[`diaspor_core::VfsBackend`](https://docs.rs/diaspor-core/0.1.0-alpha.1/diaspor_core/trait.VfsBackend.html)
must satisfy in order to be considered compliant with the Diaspor reference suite.

It pairs with the executable test suite published in
[`diaspor-conformance`](https://docs.rs/diaspor-conformance/0.1.0-alpha.1/diaspor_conformance/).
Every numbered invariant below is followed by the **exact test function** in
[`src/lib.rs`](./src/lib.rs) that proves it. When a future invariant is added,
this document and the test suite are updated in the same pull request.

## What conformance means

A backend `B: VfsBackend` is *conformant at version `X.Y.Z`* if and only if:

1. `diaspor_conformance::run(B::new())` returns without panicking when linked
   against `diaspor-conformance = "X.Y.Z"`.
2. The backend's documentation lists which optional capabilities it supports
   (currently only the M1 baseline is mandatory; M2+ deliverables such as xattrs,
   symlinks, locking, and long paths are tracked in the "Future invariants"
   section below and become mandatory only when their crate-level features land).
3. The backend behaves the same on Linux, macOS, and Windows for every invariant
   below that is platform-agnostic. Backends that intentionally restrict themselves
   to one host operating system must say so in their crate-level rustdoc.

The conformance suite is single-threaded and assumes a freshly constructed,
empty backend (only the root directory exists). It mutates the backend during
the run. Backends that hold persistent state across constructor calls must
provide a way to start from a clean slate before the suite is invoked.

## Who runs it

- **First-party backends** in this workspace
  (`diaspor-backend-memory`, `diaspor-backend-local`) run the suite from
  their own integration tests on every CI build.
- **Third-party backends** are encouraged to add `diaspor-conformance` as a
  `dev-dependency` and call `diaspor_conformance::run` from a `#[tokio::test]`.
- **CI gating**: a backend is not eligible for the conformance badge listed in
  the project README until its public repository links to a green CI run.

## Reading the invariant tables

Each section corresponds to one method on `VfsBackend` or `VfsHandle`. Within a
section, invariants are numbered. The **Test** column points to a contiguous
block of assertions in the single `run` function in
[`src/lib.rs`](./src/lib.rs); the numbers in that file's doc comment line up
1:1 with the numbers in this document.

The suite is intentionally written as **one sequential script** rather than
many separate `#[test]` functions. Conformance assertions have state
dependencies (creating a file, then reading it, then removing it), and keeping
the ordering explicit makes failures easier to diagnose than a forest of
independent tests would.

---

## `VfsBackend::metadata`

Returns the metadata of the node at `path`.

| #  | Invariant                                                                 | Test (in `run`)                           |
|----|---------------------------------------------------------------------------|-------------------------------------------|
| 1  | A fresh backend has only the root directory.                              | "1 & 2. Fresh root metadata."             |
| 2  | `metadata(VfsPath::root())` returns `kind = Directory, size = 0`.         | "1 & 2. Fresh root metadata."             |
| 5  | After creating an empty file, `metadata` returns `kind = File, size = 0`.| "4 & 5. Create a file."                   |
| 7  | After a successful `write(offset, data)`, `metadata.size` reflects the on-disk size (`offset + data.len()` for the simple case where the file was previously empty). | "6, 7, 8. Write and read back."           |
| 10 | After `create_dir`, `metadata` on the new path returns `kind = Directory`.| "10. create_dir."                         |
| 12 | After `remove(path)` on a file, `metadata(path)` returns `VfsError::NotFound`. | "12. remove file."                  |

Behavioral notes:

- `metadata` must not have side effects. Calling it on the same path twice in a
  row must return the same result modulo concurrent mutation by another caller.
- `metadata` on a syntactically invalid path must return `VfsError::InvalidPath`
  rather than `VfsError::NotFound`. (Not yet covered by the suite — see "Future
  invariants" below.)

## `VfsBackend::list`

Lists the children of a directory at `path`.

| #  | Invariant                                                                          | Test (in `run`)                                  |
|----|------------------------------------------------------------------------------------|--------------------------------------------------|
| 3  | `list(root)` on a fresh backend returns an empty vector.                           | "3. list(root) on fresh backend is empty."       |
| 11 | `list(directory)` returns **direct children only** — never grandchildren.          | "11. list returns direct children only."         |
| 18 | `list(file_path)` returns `VfsError::KindMismatch`.                                | "18. list on a file -> KindMismatch."            |

Behavioral notes:

- The returned `Vec<VfsPath>` is unordered. Callers that need a stable order must
  sort by `as_str()` themselves; the conformance suite does this to compare
  against a known fixture.
- A backend may return paths in any deterministic order, but it may not return
  duplicates and it may not skip valid children.
- `list` on the root path must succeed even when the backend has zero children
  (it returns `Ok(vec![])`).

## `VfsBackend::open`

Opens a handle on the node at `path` with the given [`OpenFlags`](https://docs.rs/diaspor-core/0.1.0-alpha.1/diaspor_core/struct.OpenFlags.html).

| #  | Invariant                                                                          | Test (in `run`)                                          |
|----|------------------------------------------------------------------------------------|----------------------------------------------------------|
| 4  | `open(path, CREATE | WRITE)` on a missing path creates the file and returns a writable handle. | "4 & 5. Create a file."              |
| 15 | `open(path, READ)` on a missing path returns `VfsError::NotFound`.                 | "15. open(READ) on missing path -> NotFound."            |
| 16 | `open(path, WRITE)` on a missing path (no `CREATE`) returns `VfsError::NotFound`.  | "16. open(WRITE) without CREATE -> NotFound."            |
| 17 | `open(path, CREATE | EXCL | WRITE)` on an **existing** path returns `VfsError::AlreadyExists`. | "17. CREATE | EXCL on existing -> AlreadyExists."   |

Behavioral notes:

- `OpenFlags` is a bitflags set. Backends must honor the documented combinations
  even when callers pass redundant flags (e.g. `CREATE | WRITE | READ`).
- The handle returned by `open` lives independently of the backend. Closing the
  handle (by dropping it) must not invalidate other concurrent handles on the
  same path.
- A backend that does not support a particular flag must return
  `VfsError::Unsupported { operation }` rather than silently downgrading
  behavior. The first-party memory and local backends support all flags
  documented in `OpenFlags`.

## `VfsBackend::create_dir`

Creates a directory at `path`. Parents must already exist (no implicit `mkdir -p`).

| #  | Invariant                                                                          | Test (in `run`)                                  |
|----|------------------------------------------------------------------------------------|--------------------------------------------------|
| 10 | `create_dir(p)` makes `p` a directory; subsequent `metadata(p)` reports `Directory`.| "10. create_dir."                            |

Behavioral notes:

- Calling `create_dir` on an existing path (file or directory) must return
  `VfsError::AlreadyExists`. (Not yet covered by the suite — see "Future
  invariants" below.)
- Calling `create_dir` when the parent does not exist must return
  `VfsError::NotFound` rather than recursively creating it. Backends that want
  recursive-create semantics should expose a separate helper, not bake it into
  the trait method.

## `VfsBackend::remove`

Removes the node at `path`. For directories, behaviour matches POSIX `rmdir`
(must be empty).

| #  | Invariant                                                                          | Test (in `run`)                                  |
|----|------------------------------------------------------------------------------------|--------------------------------------------------|
| 12 | `remove(file_path)` makes the file disappear; `metadata` then returns `NotFound`.  | "12. remove file."                               |
| 13 | `remove(empty_dir)` succeeds.                                                      | "13. remove empty directory…"                    |
| 14 | `remove(non_empty_dir)` returns an error (any variant other than `NotFound`).      | "14. remove non-empty directory fails."          |

Behavioral notes:

- Invariant 14 deliberately does not pin a single error variant: different
  backends naturally report this as `Backend("not empty")`, `Io(...)`, or a
  bespoke variant. The conformance suite only asserts that it is **not**
  `NotFound`, because a `NotFound` here would silently let callers misinterpret
  a "directory not empty" condition as "directory already gone."
- A future M2 refinement will introduce `VfsError::DirectoryNotEmpty` and tighten
  this assertion; see "Future invariants."

---

## `VfsHandle::read`

Reads up to `len` bytes starting at byte offset `offset`.

| #  | Invariant                                                                          | Test (in `run`)                                  |
|----|------------------------------------------------------------------------------------|--------------------------------------------------|
| 8  | `read(0, n)` after `write(0, payload)` returns `payload` (round-trip).             | "6, 7, 8. Write and read back."                  |
| 9  | `read(offset, len)` past EOF returns an **empty** `Bytes`, not an error.           | "9. read past EOF."                              |

Behavioral notes:

- `read` is allowed to return fewer than `len` bytes even when the file is
  longer than `offset + len`, as long as it returns at least one byte. Callers
  must loop until they receive an empty slice. The first-party memory and
  local backends always return as many bytes as are immediately available.
- Reading from a handle that was opened without `OpenFlags::READ` must return
  `VfsError::PermissionDenied` rather than the actual file contents. (Not yet
  covered by the suite — see "Future invariants.")

## `VfsHandle::write`

Writes `data` at byte offset `offset`. Returns the number of bytes written.

| #  | Invariant                                                                          | Test (in `run`)                                  |
|----|------------------------------------------------------------------------------------|--------------------------------------------------|
| 6  | `write(offset, data)` returns `data.len()` on success.                             | "6, 7, 8. Write and read back."                  |
| 7  | After `write(offset, data)` and a `flush`, `metadata.size` reflects the on-disk size. | "6, 7, 8. Write and read back."             |

Behavioral notes:

- A partial write (`Ok(n)` where `n < data.len()`) is allowed by the trait
  contract but is not exercised by the conformance suite at the v0.1 baseline.
  Backends that may legitimately partial-write (cloud backends with chunked
  uploads, for instance) must document the conditions under which this occurs.
- Writing to a handle that was opened without `OpenFlags::WRITE` must return
  `VfsError::PermissionDenied`. (Not yet covered by the suite — see "Future
  invariants.")
- Writing past the current end of the file extends the file. Whether the gap is
  zero-filled or sparse is backend-dependent; the suite does not assert either
  way at the v0.1 baseline.

## `VfsHandle::flush`

Flushes any buffered data to the backend.

| #  | Invariant                                                                          | Test (in `run`)                                  |
|----|------------------------------------------------------------------------------------|--------------------------------------------------|
| 7  | After `flush`, a subsequent `metadata` call on the same path observes the bytes that were written via the handle. | "6, 7, 8. Write and read back."  |

Behavioral notes:

- `flush` is not a barrier across handles or processes. A backend that backs a
  cross-process resource (the local backend with concurrent writers, for
  instance) makes no guarantee about other writers seeing the flushed bytes
  before they close their own handle.
- `flush` on a read-only handle is a no-op that must return `Ok(())`.

---

## Test-to-invariant matrix

The single `run()` function in `src/lib.rs` is internally organized as 18
numbered assertion blocks (search for `// N.` comments). The mapping below is a
quick reference for anyone diagnosing a conformance failure:

| Assertion # | What it proves                                            | `VfsBackend` method exercised        |
|-------------|-----------------------------------------------------------|--------------------------------------|
| 1           | Root exists on fresh construction.                        | `metadata`                           |
| 2           | Root is a directory of size 0.                            | `metadata`                           |
| 3           | Fresh backend has no children.                            | `list`                               |
| 4           | `CREATE | WRITE` makes a missing file.                    | `open`                               |
| 5           | New files have `kind = File, size = 0`.                   | `metadata`                           |
| 6           | `write` returns `data.len()`.                             | `VfsHandle::write`                   |
| 7           | `metadata.size` reflects the written bytes after `flush`. | `metadata` + `flush`                 |
| 8           | Round-trip: `read` returns what `write` wrote.            | `VfsHandle::read` + `write`          |
| 9           | `read` past EOF returns empty.                            | `VfsHandle::read`                    |
| 10          | `create_dir` produces a directory.                        | `create_dir` + `metadata`            |
| 11          | `list` returns direct children only.                      | `list`                               |
| 12          | `remove` on a file deletes it.                            | `remove` + `metadata`                |
| 13          | `remove` on an empty directory succeeds.                  | `remove`                             |
| 14          | `remove` on a non-empty directory fails (non-NotFound).   | `remove`                             |
| 15          | `open(READ)` on missing path returns `NotFound`.          | `open`                               |
| 16          | `open(WRITE)` on missing path returns `NotFound`.         | `open`                               |
| 17          | `open(CREATE | EXCL)` on existing returns `AlreadyExists`.| `open`                               |
| 18          | `list` on a file returns `KindMismatch`.                  | `list`                               |

---

## Future invariants (M2–M6 deliverables)

The invariants below are **not yet** part of the executable suite. They are
tracked here so backend authors can plan ahead and so the v0.1.0-alpha.1 NLnet
claim ("239 lines of conformance suite") has a clear forward path to the full
v1.0 coverage. Each will be added to `run()` (or to a dedicated optional
suite) in the milestone listed.

### M2 — Local backend acceptance criteria

- **L1. Long paths.** Backends must accept paths up to at least 4096 bytes
  total, with individual segments up to 255 bytes (POSIX `NAME_MAX`).
- **L2. Unicode normalization.** Paths containing Unicode (NFC vs. NFD on macOS)
  must round-trip; `list` must return whatever form the backend stores
  internally, but `metadata`/`open` must accept both forms on macOS.
- **L3. `create_dir` on existing path returns `AlreadyExists`.** (Currently not
  exercised; M2 will add it.)
- **L4. `create_dir` with missing parent returns `NotFound`.** (Currently not
  exercised; M2 will add it.)
- **L5. `metadata` on a syntactically invalid path returns `InvalidPath`.**
- **L6. Read-only handle write returns `PermissionDenied`.**
- **L7. Write-only handle read returns `PermissionDenied`.**
- **L8. `TRUNC` flag on open zeroes an existing file's size.**
- **L9. `APPEND` flag forces writes to the current end of file regardless of
  the `offset` argument.**

### M2 — Symbolic links

- **S1. `metadata` follows symlinks by default and returns the target's
  metadata.**
- **S2. A backend that exposes raw-symlink reads must document the API and
  return `NodeKind::Symlink` from that API.** (Diaspor will likely add a
  `metadata_no_follow` method in M2.)
- **S3. `remove` on a symlink removes the link, not the target.**

### M2 — Extended attributes

- **X1. A backend that advertises xattr support must round-trip values via
  `set_xattr`/`get_xattr` (methods to be added in M2).**
- **X2. Listing xattr keys on a node with no xattrs returns `Ok(vec![])`.**
- **X3. `get_xattr` on a missing key returns `NotFound { key }`.**

### M2 — File locking

- **F1. A backend that advertises locking must support `lock_exclusive` and
  `lock_shared` with the documented blocking semantics. (Trait methods land
  in M2.)**

### M3 / M4 — Mount adapters (FUSE / WinFsp)

- **M1. Mounting a memory backend via the FUSE adapter must let host tools
  (`ls`, `cat`, `cp`, `touch`) operate on the backend's contents.**
- **M2. Unmount must be safe under SIGINT and after panics inside the FUSE
  thread.** (Tested via integration tests that hold a watchdog on the host
  process.)
- **M3. WinFsp parity: mounting on Windows must produce identical externally
  observable behavior to FUSE on Linux/macOS for every assertion already in
  this document.**

### M5 — Transcription pipeline

- **T1. A `Transcriber` implementation must accept 16 kHz mono 16-bit PCM
  audio and return a `Transcript` whose `text` is non-empty for non-silent
  input.**
- **T2. A `MediaExtractor` implementation must produce 16 kHz mono PCM regardless
  of the source container's native sample rate.**
- **T3. Pipeline runs must produce a `SidecarRecord` whose JSON serialization
  validates against `docs/schema/sidecar-v1.json`.** (Cross-validates this
  document with the M6 schema.)
- **T4. No-egress invariant.** A sandboxed end-to-end run must produce a
  transcript with zero outbound network calls. (Tested via an egress-blocking
  CI job; see `BENCHMARKS.md` for the planned methodology.)

### M6 — Sidecar persistence and v1.0

- **P1. After `pipeline.process(file)`, the indexer writes
  `/.index/<file>.json` back into the wrapped backend, and that path round-trips
  through `metadata` / `list` / `read`.**
- **P2. The sidecar JSON must validate against
  [`docs/schema/sidecar-v1.json`](../../docs/schema/sidecar-v1.json) under the
  `schema_version = "1"` constant.**
- **P3. Reading a sidecar back into Rust via `serde` produces a
  `SidecarRecord` whose fields equal the in-memory value that produced it
  (round-trip stability).**
- **P4. Forward compatibility.** A reader for `schema_version = "1"` must
  ignore unknown fields rather than fail.

---

## Versioning

This specification is versioned with the `diaspor-conformance` crate.
v0.1.0-alpha.1 freezes the **18 baseline assertions** documented above. The
"Future invariants" section is informative only at v0.1; each item moves into
the "current" section when it lands in `src/lib.rs` and a new minor version of
the crate is published.

Breaking changes to existing assertions (renumbering, semantic changes, removal)
require a major version bump of `diaspor-conformance` and a corresponding
note in `RELEASES.md`. Adding new assertions in the "current" sections is a
**minor** version bump as long as no backend that previously passed the suite
would now fail it.

## Reporting failures

If `diaspor_conformance::run` panics on your backend:

1. Read the panic message — every assertion includes a short string describing
   what was expected.
2. Cross-reference the assertion number with this document (the same number
   appears in `src/lib.rs` as a `// N.` comment).
3. If the assertion looks wrong (over-specified, platform-broken, or in
   conflict with the trait documentation), open an issue on
   [github.com/stonyp90/diaspor](https://github.com/stonyp90/diaspor)
   with the `conformance` label.

Genuine bugs in the suite (as opposed to bugs in backends it is testing) are
fixed in a patch release of `diaspor-conformance` and noted in
`RELEASES.md`.
