---
trigger: always_on
---

# diaspor guardrails

`AGENTS.md` at the repo root is the canonical project memory (map, commands,
conventions, known inconsistencies). If it isn't already in your context, read
it before planning a change. These rules are the ones never to break:

- **Privacy contract.** No default code path may open a network connection.
  Networked code goes behind an opt-in cargo feature (`hub`, `remote`, …) and
  honours `DIASPOR_OFFLINE=1`. No telemetry, analytics, or update checks in the
  library crates.
- **Verify like CI.** For Rust changes, run `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
  `cargo test --workspace --lib --tests`. For other areas, run the commands in
  `AGENTS.md` §3.
- **Library code:** no `unwrap()`, rustdoc on every public item, errors via
  `thiserror`, async on tokio, `VfsPath` above the backend boundary.
  `diaspor-core` stays IO-free.
- **Two build systems:** `apps/vfs-desktop/src-tauri` is a standalone Cargo
  workspace (edition 2021) and is not part of the root workspace (edition 2024).
- **Cross-boundary contracts change together, in one commit.** See
  `docs/ARTIFACTS.md`.
- **Website:** edit EN (`website/index.html`) and FR (`website/fr/index.html`)
  together. Don't add marketing claims the code doesn't back. A push to `main`
  deploys to production.
- **Never** commit secrets, hand-edit lockfiles or `src-tauri/gen/schemas`,
  rewrite an accepted ADR, or edit a published schema in a breaking way (add a
  `*-v2.json` instead).
- Stubs marked with a roadmap milestone (M3–M10) are intentional. Don't
  implement one unless asked.
