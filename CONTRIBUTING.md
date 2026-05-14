# Contributing to cairn

Thanks for considering a contribution. The project is small and the maintainer responds
to issues and PRs on a best-effort basis; please be patient, and please keep things
focused so we can keep things moving.

By participating you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md)
(Contributor Covenant 2.1).

## How to help

- **Found a bug?** Open an issue with a minimal reproduction. Even a failing test in
  prose is fine — we'd rather have a clear bug report than silence.
- **Want a new feature?** Open an issue first to scope it before writing code. The
  roadmap is deliberately narrow, but composable extensions (decorator backends,
  conformance test improvements, new examples) are welcome.
- **Have a fix?** PRs are welcome. Small, targeted PRs land faster than big ones.
- **Improving docs?** Always welcome. Doc-only PRs skip most of the test gating.

## Good first issues

Issues tagged [`good first issue`](https://github.com/stonyp90/cairn/labels/good%20first%20issue)
are scoped to land in a single, focused PR without needing a deep tour of the codebase.
If the label is empty, the documentation tracker is always a safe place to start — see
the categories below.

## What we're looking for in PRs

Realistic contributions that land cleanly, in roughly increasing order of scope:

- **Typo and grammar fixes** in rustdoc, README, or other markdown.
- **Doc clarifications** — a paragraph that confused you is probably confusing others.
- **Test additions** — extra cases for the conformance suite, edge cases for the
  memory or local backend, fuzz inputs.
- **Conformance suite expansion** — new shared tests that any backend must pass.
- **ADR proposals** for non-trivial design decisions (post an issue first; the ADR is
  the artefact that comes out of the discussion).
- **Examples** under `examples/` showing real downstream uses (CLI tools, decorators,
  custom backends).
- **Benchmark additions** with reproducible inputs and a brief note on what they measure.

Larger trait or API changes are best discussed in an issue before code is written —
see the PR process below.

## Development setup

You need:

- Rust **1.85 stable** or newer (the `rust-toolchain.toml` will install it for you).
- A Unix-y shell on Linux/macOS, or PowerShell on Windows.
- Optional: `cargo-deny` and `cargo-audit` for the security workflows.

```bash
git clone https://github.com/stonyp90/cairn.git
cd cairn
cargo build --workspace
cargo test --workspace
```

## Coding standards

Three checks run in CI; all three must be clean before review:

| Check    | Command                                                          |
|----------|------------------------------------------------------------------|
| Format   | `cargo fmt --all -- --check`                                     |
| Lints    | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| Tests    | `cargo test --workspace --all-features`                          |

Additional rules:

- **Public items get rustdoc comments.** `missing_docs` is a workspace lint set to
  `warn`; CI runs `cargo doc` with `-D warnings`.
- **No `unwrap()` in library code.** Use `?`, or `expect("reason")` when an invariant
  truly cannot fail and you can articulate why in the message.
- **Error types use `thiserror`.** New errors go into the `VfsError` enum unless they
  belong to a specific crate, in which case they live next to the crate's main type.
- **Async-only.** All IO returns a future. If you need a sync surface, propose it in an
  issue first.

## Commit messages

Conventional Commits, lightly enforced:

```
type(scope): summary

Longer body, wrapped at 72 chars where reasonable.

Closes #123
```

Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `ci`, `chore`. Scope is the
crate name (`core`, `memory`, `local`, `fuse`, `winfsp`, `cli`) or `workspace` for
top-level changes.

## Pull request process

1. Open an issue first if the change is non-trivial or alters a public trait.
2. Fork, branch (`fix/<thing>` or `feat/<thing>`), open a PR against `main`.
3. Make sure CI is green. If it's red, fix it (or push a `wip:` commit and say so).
4. The maintainer will review within a week, typically faster. Expect one or two rounds
   of feedback on style and API.
5. Once approved, the maintainer merges via squash with a Conventional Commit message.

## Recognition

All contributors who land a commit are credited in CHANGELOG.md and on the repository
README's contributors graph. Doc fixes and typo PRs count equally — a name on the
contributors list is a name on the contributors list.

## Licence

Contributions are accepted under the project's [MIT licence](LICENSE). By submitting a
PR you agree your contribution is licensed under those terms. The project does not
require a separate CLA.

## Reporting security issues

Please do **not** open public issues for security problems. Email the maintainer at
`anthonypaquet1508@gmail.com` with a clear description and reproduction. You will get an
acknowledgement within 72 hours and a coordinated disclosure plan.
