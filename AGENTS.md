# AGENTS.md — project memory for coding agents

Canonical, tool-agnostic instructions for AI coding agents working in this repo.
Read natively by **Codex** and **Qoder** (root `AGENTS.md`); **Claude Code** loads
it via `@AGENTS.md` in `CLAUDE.md`. Qoder additionally gets short rules from
`.qoder/rules/`. Edit *this* file when project facts change; the tool-specific
files only point here, so there is a single source of truth.

The artifact inventory (every file this repo produces or consumes across a
boundary, and the contract that pins its name/shape) lives in
[`docs/ARTIFACTS.md`](docs/ARTIFACTS.md). Read it before touching release
workflows, website download code, `models.toml`, schemas, or training output.

---

## 1. What this repo is

`diaspor` — a privacy-first virtual filesystem in Rust with on-device media
indexing (FFmpeg → whisper.cpp → local-LLM tags → sidecar JSON), growing into a
self-hosted non-verbal video AI stack (pose/face/prosody scoring, inference hub,
LoRA training, hosted API). Pre-1.0 alpha; many crates are **trait surface only**
with implementations scheduled by roadmap milestone (M1–M10, see `ROADMAP.md`).
Don't "finish" a stub unless asked — stubs are deliberate.

Sole maintainer: `@stonyp90`. Canonical remote: `github.com/stonyp90/diaspor`
(mirrored to Codeberg). Default branch: `main`.

## 2. Repository map — four independent build systems

| Area | Path | Toolchain | Notes |
|---|---|---|---|
| Rust library workspace | `Cargo.toml`, `crates/*` | Rust **1.85**, edition **2024**, resolver 3 | 16 crates, shared version |
| Desktop app (Tauri 2) | `apps/vfs-desktop/` | React 18 + Vite 5 + TS; Rust **edition 2021** | **Standalone** Cargo workspace (own `Cargo.lock`, own `target/`) — *not* a member of the root workspace |
| Client SDKs | `sdk/typescript/`, `sdk/python/` | Node ≥ 20 / Python ≥ 3.10 | Thin clients for `api.diaspor.io`; licensed **Apache-2.0** |
| Training recipes | `training/` | Python ≥ 3.10, torch/peft | Produces LoRA `.safetensors` consumed by `crates/diaspor-train` |
| Marketing site | `website/` | Plain HTML + CSS, no build step | EN `index.html` + FR `fr/index.html` |
| Deploy | `deploy/` | Docker (cargo-chef + distroless), Helm | For `diaspor-api-server` only |
| Docs / contracts | `docs/` | — | ADRs, JSON schemas, OpenAPI, grant drafts |

### Crates (`crates/`)

- `diaspor-core` — traits (`VfsBackend`, `VfsHandle`), `VfsPath`, `VfsError`. **No IO deps**; keep it that way.
- `diaspor-backend-memory`, `diaspor-backend-local` — reference backends.
- `diaspor-conformance` — shared backend test suite + JSON-schema lock tests (`tests/score_schema.rs`); spec in `SPEC.md`.
- `diaspor-fuse` (M3), `diaspor-winfsp` (M4) — mount adapters, stubs.
- `diaspor-index` — FFmpeg/transcribe/tag pipeline traits + `SidecarRecord`.
- `diaspor-cli` — two binaries: `diaspor` (operator CLI) and `diaspor-agent` (daemon).
- M7+ pipeline: `diaspor-frame-pipeline`, `diaspor-vision` (pose/face/prosody, score records), `diaspor-infer` (ONNX Runtime + `ModelHub`), `diaspor-stream-ingest` (file/HLS/WHIP/meeting-bot), `diaspor-events` (sidecar/WebSocket/webhook fan-out), `diaspor-train` (LoRA pipeline, adapter artifacts).
- `diaspor-api-server` — axum HTTP/WS server (`api.diaspor.io`), HS256 JWT, rate limits.
- `diaspor-imagegen` — ports & adapters image generation/compositing; offline `LocalImageAdapter` by default, cloud adapters behind `--features remote`.

## 3. Commands (mirror CI exactly)

CI is path-filtered per area; run the checks for every area you touched.

### Rust workspace (`.github/workflows/ci-rust.yml`, `RUSTFLAGS=-D warnings`)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --lib --tests                     # CI runs on Linux, macOS, Windows
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --all-features
cargo test -p <crate>                                    # fast loop for one crate
```

No-network job (privacy contract — must stay green):

```bash
cargo fetch --locked
DIASPOR_OFFLINE=1 cargo build --workspace --all-features --offline
DIASPOR_OFFLINE=1 cargo test  --workspace --all-features --offline   # CI wraps this in `unshare --net`
```

Also: `cargo audit` and `cargo deny check bans licenses sources advisories`
(`deny.toml`: license allow-list, `wildcards = "deny"`, crates.io only).

Live/billed tests are `#[ignore]`d and gated on `--features remote` + API keys
(e.g. `crates/diaspor-imagegen/tests/live_*.rs`). Never run them unless asked.

### Desktop app (`apps/vfs-desktop/`)

```bash
cd apps/vfs-desktop
npm ci
npm run build          # tsc && vite build
npm run lint           # eslint
npm run tauri:dev      # full app (needs platform Tauri deps)
cd src-tauri && cargo test   # Rust side; separate workspace, edition 2021
```

No PR CI covers the desktop app — only `release-desktop.yml` on tags — so build
it locally before claiming a desktop change works. `project.json` (Nx) and
`jest.config.ts` reference root files (`nx`, `jest.preset.js`) that do not exist
in this repo; the `*.spec.tsx` files are not runnable as-is. Don't "fix" this
by adding a root `package.json` unless asked.

### SDKs

```bash
cd sdk/typescript && npm ci && npx tsc --noEmit && npm test -- --run && npm run build
cd sdk/python && pip install -e ".[dev]" && ruff check src tests && ruff format --check src tests && mypy --strict src && pytest -q
```

### Training (`training/`)

```bash
cd training && pip install torch --index-url https://download.pytorch.org/whl/cpu && pip install -e ".[dev]"
pytest tests/test_artifact.py -q        # the only PR-gated test: the artifact contract
```

The full recipe (`python -m recipes.prosody_lora …`) downloads ~360 MB and
trains for minutes; it runs only in `release-heavy.yml`. Don't run it casually.

### Website (`website/`)

```bash
python3 -m http.server 8765 --directory website       # preview EN at /, FR at /fr/
npx html-validate@8 website/index.html website/fr/index.html
python3 tools/check_asset_pattern.py                  # needs PyYAML
```

## 4. Non-negotiables

1. **Privacy contract is a security property** (`PRIVACY.md`, `SECURITY.md`,
   `THREAT_MODEL.md`). No default code path may open a socket. Network-capable
   code goes behind an opt-in cargo feature (`hub`, `remote`, …) and must honour
   `DIASPOR_OFFLINE=1`. No telemetry, analytics, or update checks in the library
   crates. An unsolicited network call is treated like an RCE.
2. **`diaspor-core` stays IO-free.** New deps there need a strong reason.
3. **No `unwrap()` in library code.** Use `?`, or `expect("why this can't fail")`.
4. **Every public item has rustdoc** (`missing_docs` warns; CI denies warnings).
   Workspace lints include clippy `pedantic` + `nursery` — fix, don't blanket-`allow`.
   A narrowly scoped `#[allow(...)]` with a comment explaining why is acceptable.
5. **Errors via `thiserror`.** VFS errors go in `VfsError`; crate-specific errors
   live beside the crate's main type.
6. **Async-first on tokio.** No blocking IO on the runtime; `async_trait` for
   object-safe traits (ADR 0003). `parking_lot` locks only when never held across `.await`.
7. **Paths are `VfsPath`** (always absolute, `/`-separated) everywhere above the
   backend boundary. Convert to `std::path` only inside backends.
8. **Contracts across language/tool boundaries — change both sides in the same
   commit** (details and which ones are test-locked in `docs/ARTIFACTS.md`):
   - release asset names ↔ website download JS (`tools/check_asset_pattern.py`)
   - LoRA adapter path + metadata: Rust `AdapterArtifact::path_in_tenant_bucket` ↔ Python `AdapterArtifactWriter`
   - `docs/schema/*.json` ↔ Rust record types ↔ SDK models
   - `models.toml` pins (sha256 + bytes) ↔ `ModelHub`
   - API routes ↔ `docs/api/openapi.yaml` ↔ `tests/route_table.rs` ↔ SDK clients
9. **Schemas are versioned, not edited in place.** Adding an optional field is
   fine; renaming/removing/retyping/tightening needs a new `*-v2.json` beside the old one.
10. **ADRs are immutable once accepted.** Supersede with a new numbered ADR.
11. **Never commit secrets or credentials.** `.env*`, `setup-aws-env.local.sh`,
    `deploy-with-credentials.sh` are git-ignored on purpose. Don't print or copy
    secret values from workflows or local files into code, docs, or commits.
12. **Don't hand-edit generated/lock files**: `Cargo.lock`,
    `apps/vfs-desktop/src-tauri/Cargo.lock`, `package-lock.json`,
    `apps/vfs-desktop/src-tauri/gen/schemas/*` (Tauri-generated). Regenerate with the tool.

## 5. Conventions

- **Formatting:** `rustfmt.toml` (max width 100, edition 2024); `.editorconfig`
  (4-space default, 2-space for toml/yml/md/json, LF, final newline).
- **Commits:** Conventional Commits, `type(scope): summary`. Types in use:
  `feat`, `fix`, `docs`, `refactor`, `test`, `ci`, `chore`, `ui`, `site`,
  `release`. Scope = crate short name (`core`, `imagegen`, `infer`, …),
  `landing`/`website`, `vfs-desktop`, or `workspace`.
- **PRs:** fill `.github/pull_request_template.md` (What / Why / Test plan / Linked issue).
  Small, focused diffs. Non-trivial trait/API changes need an issue first.
- **Versioning:** all 16 crates share `[workspace.package] version`
  (currently `0.1.0-alpha.17`). A bump touches: root `Cargo.toml` (package +
  every `[workspace.dependencies]` internal entry), `crates/diaspor-backend-memory/Cargo.toml`
  (dev-dep on conformance), and the README quick-start snippet. The desktop app
  versions independently (`package.json` + `src-tauri/tauri.conf.json` +
  `src-tauri/Cargo.toml`). SDKs version independently.
- **Release notes:** `RELEASES.md`, newest first. Tags `v*` trigger releases.
- **Features:** heavy/networked deps are opt-in features (`diaspor-infer`:
  `ort-cpu`, `ort-cuda`, `ort-coreml`, `ort-tensorrt`, `hub`, `nonfree-models`;
  `diaspor-imagegen`: `remote`). Default features stay offline and light.
- **Model licensing:** default builds load only permissive weights; research
  weights are `nonfree = true` in `models.toml` and need `--features nonfree-models`.

## 6. Website rules

- Change **EN and FR together** (`website/index.html` + `website/fr/index.html`);
  keep hreflang/sitemap symmetric.
- Marketing copy must be **verifiable against the code**. Recent history removed
  false/contradictory claims (e.g. "zero US sub-processors", release checksum/signing
  claims). Don't reintroduce claims the code or workflows don't back.
- Responsive/a11y contract in `website/README.md` (no horizontal scroll 320–1440px,
  44×44 targets, WCAG AA). New responsive overrides go in the
  "Mobile + a11y polish" block at the bottom of `styles.css`.
- No inline `<style>` in `<body>` (html-validate fails it).
- Push to `main` touching `website/**` **deploys to production** (S3 + CloudFront
  via OIDC, `deploy-website.yml`). Never run `website/deploy.sh` yourself.

## 7. Environment variables

| Var | Used by | Meaning |
|---|---|---|
| `DIASPOR_OFFLINE=1` | `diaspor-infer` ModelHub | Refuse downloads with a clear error |
| `DIASPOR_TRUST_UNPINNED=1` | ModelHub | Allow `PENDING_FIRST_PULL` catalog entries; prints observed sha256 |
| `DIASPOR_MODELS_DIR` | ModelHub | Cache root (default `~/.diaspor/models`) |
| `DIASPOR_JWT_SECRET` | api-server | **Required**; server refuses to start without it |
| `DIASPOR_BIND_ADDR` | api-server | Default `127.0.0.1:7733` |
| `DIASPOR_DEFAULT_RATE_LIMIT`, `DIASPOR_DEFAULT_DAILY_CAP` | api-server | Per-key budgets |
| `OPENAI_API_KEY`, `GEMINI_API_KEY`, `AZURE_OPENAI_*`, `*_BASE_URL`, `*_IMAGE_MODEL` | imagegen `remote` adapters | Billed providers; live tests are `#[ignore]`d |

## 8. Known inconsistencies (don't "fix" silently — flag them)

- **License:** `LICENSE`, `Cargo.toml`, and `README.md` say **AGPL-3.0-only**
  (authoritative for the Rust workspace and `training/`). `CONTRIBUTING.md`
  and ADR 0001 still say MIT; `docs/RELEASE_PLAYBOOK.md` mentions MIT. SDKs are
  Apache-2.0 by design.
- **README crate table** lists only the original 7 crates; the workspace has 16.
- **Desktop updater:** `tauri.conf.json` points the updater at
  `releases/latest/download/latest.json`, but no workflow uploads a `latest.json`;
  the committed `apps/vfs-desktop/latest.json` is stale (v1.1.28, empty signatures).
- **Desktop versions drift:** `package.json`/`tauri.conf.json` = 2.0.7,
  `src-tauri/Cargo.toml` = 2.0.6.
- `website/README.md` calls the preview launcher `diaspor-site`; `.claude/launch.json` names it `diaspor-website`.

## 9. Definition of done

- The CI commands for every touched area pass locally (section 3), including
  the no-network build if you touched Rust deps or features.
- Both sides of any cross-boundary contract changed together (section 4.8,
  `docs/ARTIFACTS.md`).
- Public items documented; no new `unwrap()` in library code; no new default-on
  network path.
- Docs updated where behaviour changed (`README.md`, `ARCHITECTURE.md`,
  `RELEASES.md`, ADR if it's a load-bearing decision).
- Report honestly: what you ran, what passed, what you could not run (e.g. no
  macOS/Windows, no Tauri system deps) and why.
