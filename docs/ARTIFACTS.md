# Artifacts and their contracts

Every file this repository **produces** (release archives, installers, images,
adapters, sidecars) or **consumes across a boundary** (model weights, schemas,
update manifests), with the exact name/shape contract, where each side of the
contract lives, and what enforces it.

Written for humans and coding agents alike (`AGENTS.md` links here). Rule of
thumb: **if you change one side of a contract, change the other side in the same
commit and run the guard listed.** Where the guard column says *none*, nothing
automated will catch drift, so check it by hand.

---

## 1. Release artifacts (GitHub Releases, tag `v*`)

A single `git push origin vX.Y.Z…` fires `release.yml` and `release-desktop.yml`
in parallel; `release-heavy.yml` is `workflow_dispatch` only.

| Artifact | Name pattern | Producer | Consumers | Guard |
|---|---|---|---|---|
| CLI archive | `diaspor-<tag>-<target>.tar.gz` (Windows: `.zip`) containing `diaspor`, `diaspor-agent`, `README.md`, `LICENSE` | `release.yml` → `build-cli` | Website `initCliDownloads` (`website/index.html`), users | `tools/check_asset_pattern.py` (CI Website) |
| API server archive | `diaspor-api-server-<tag>-<target>.<ext>` | `release.yml` → `build-api-server` | Operators | none |
| Checksums | `<archive>.sha256` | same jobs | Users | none |
| Signatures | `<archive>.minisig` (only when `MINISIGN_SECRET` is set) | `release.yml` → `publish` (rsign2) | Users | optional by design (`fail_on_unmatched_files: false`) |
| Build provenance | SLSA in-toto attestation per archive | `actions/attest-build-provenance` | `gh attestation verify` | none |
| Desktop installers | exactly `diaspor-vfs.dmg`, `diaspor-vfs.msi`, `diaspor-vfs.AppImage` (renamed from Tauri bundle output into `release-artifacts/`) | `release-desktop.yml` | Website `initDesktopDownloads` `ASSETS` map (EN + FR) | `tools/check_asset_pattern.py` |
| CUDA variant | `diaspor-cuda-<tag>-<target>.tar.gz` (+ bundled `models/`) | `release-heavy.yml` | Operators | none |
| Bundled-models variant | `diaspor-bundled-<tag>-<target>.tar.gz` | `release-heavy.yml` | Operators | none |
| Container image | `ghcr.io/stonyp90/diaspor-api-server:<tag>` and `:<tag-without-v>`, `linux/amd64` + `linux/arm64`, with provenance attestation | `release-heavy.yml` (`deploy/docker/diaspor-api-server/Dockerfile`) | Helm chart `deploy/helm/diaspor-api-server/` | none |

**CLI target matrix** (`release.yml` `build-cli.strategy.matrix.include`) must
equal the website `ARCHIVES` map exactly:

| Target | Archive |
|---|---|
| `aarch64-apple-darwin` | `tar.gz` |
| `x86_64-apple-darwin` | `tar.gz` |
| `x86_64-unknown-linux-gnu` | `tar.gz` |
| `aarch64-unknown-linux-gnu` | `tar.gz` |
| `x86_64-pc-windows-msvc` | `zip` |

Adding, removing, or renaming a target means editing `release.yml` **and**
`website/index.html` (`const ARCHIVES = {…}`) in one change, then running
`python3 tools/check_asset_pattern.py`. The FR page may omit the `ARCHIVES` JS
but must carry the desktop `ASSETS` map.

**Desktop signing:** macOS signing and notarization are *fail-soft*. Without valid
`APPLE_*` secrets the `.dmg` ships unsigned with a warning annotation. Tauri
updater signing uses `TAURI_SIGNING_PRIVATE_KEY`. Never make signing a hard
requirement without the maintainer's say-so, and never claim on the website that
releases are signed or checksummed unless the workflows guarantee it.

### Desktop auto-update manifest (known gap)

- `apps/vfs-desktop/src-tauri/tauri.conf.json` → updater endpoint
  `https://github.com/stonyp90/diaspor/releases/latest/download/latest.json`,
  verified with the embedded minisign `pubkey`.
- `apps/vfs-desktop/scripts/generate-update-manifest.js` can build that manifest
  from an `artifacts/` dir, but **no workflow runs it or uploads `latest.json`**.
- `apps/vfs-desktop/latest.json` in the tree is a stale example (v1.1.28, empty
  signatures, different file names). Don't treat it as live.

## 2. Model weights (`models.toml` → `diaspor-infer` ModelHub)

- `models.toml` is embedded at compile time (`include_str!`) and parsed by
  `crates/diaspor-infer/src/catalog.rs`. It's the workspace's single source of
  truth for weights.
- Each `[[models]]` entry pins upstream URL, format, `bytes`, `sha256`, license.
  An entry is **pinned** only when `sha256` is 64 hex chars and `bytes > 0`.
  `"PENDING_FIRST_PULL"` entries are refused unless `DIASPOR_TRUST_UNPINNED=1`
  (which downloads, prints the observed hash, and asks you to commit it).
- Cache layout: `${DIASPOR_MODELS_DIR:-~/.diaspor/models}/<id-slug>@<sha256-prefix>/<basename>`.
  `DIASPOR_OFFLINE=1` blocks every download with a clear error.
- `nonfree = true` entries (e.g. pyannote, DINOv2) resolve only with
  `--features nonfree-models` and print a license notice on first load.
- Test fixture: `test-identity-fp32@1` ↔ `crates/diaspor-infer/tests/fixtures/identity.onnx`
  (bytes shipped in-tree so `hub_smoke.rs` runs offline).
- **Provenance:** every adapter that consumes a model must emit
  `model_provenance` rows (`id`, `sha256`, `version` echoed from the catalog)
  in score records (see §4).
- **Guard:** CI Rust runs on `models.toml` changes. Never invent a hash; only
  commit one observed from a real download.

## 3. LoRA adapter artifacts (`training/` ↔ `crates/diaspor-train`)

| Aspect | Rust side | Python side |
|---|---|---|
| Path | `AdapterArtifact::path_in_tenant_bucket` (`crates/diaspor-train/src/adapter.rs`) | `AdapterArtifactWriter.relative_path` (`training/diaspor_train/artifact.py`) |
| Config | `LoraConfig` (`crates/diaspor-train/src/lora.rs`) | `LoraConfig` (`training/diaspor_train/config.py`), same JSON field names |
| Refusal list | enforced at the API layer | `CREDIBILITY_REFUSAL_VERTICALS` |

- Canonical path: `tenants/{tenant}/adapters/{adapter}.safetensors`
  (relative to an artifact root).
- File: a single safetensors blob holding **only the LoRA delta**, never the
  frozen backbone.
- Required metadata (all strings): `diaspor_adapter_id`, `diaspor_tenant_id`,
  `diaspor_base_model`, `diaspor_trained_at` (UTC ISO-8601),
  `diaspor_format_version` = `"1"`. Extra metadata values that aren't strings
  are JSON-encoded with sorted keys.
- Vendor + tenant Ed25519 signing happens in the control plane *after* the
  recipe writes the file. Recipes don't sign.
- Refusal verticals: `forensic`, `hiring`, `insurance`, `law_enforcement`,
  `eu_workplace`, `eu_education`.
- **Guard:** `training/tests/test_artifact.py` (CI training runs on changes to
  `training/**`, `adapter.rs`, `lora.rs`). Changing the path or metadata keys
  means bumping `diaspor_format_version` and updating both sides together.

## 4. Data records and wire schemas (`docs/schema/`, `docs/api/`)

| Artifact | Contract file | Rust type | Other consumers | Guard |
|---|---|---|---|---|
| File sidecar | `docs/schema/sidecar-v1.json` | `diaspor_index::sidecar::SidecarRecord` | Downstream indexers | Schema README; provisional until v1.0 |
| Stream-window score | `docs/schema/score-v1.json` | `crates/diaspor-vision/src/record.rs`, `score.rs` | Python SDK `models.py`, TS SDK `models.ts` | `crates/diaspor-conformance/tests/score_schema.rs` validates every schema `examples` entry; CI Python re-runs on schema change |
| HTTP API | `docs/api/openapi.yaml` | `crates/diaspor-api-server/src/routes/*` | SDK clients | `tests/route_table.rs` locks auth/status behaviour; the OpenAPI file itself is **not** checked automatically |

- **On-VFS locations:** file sidecars at `/.index/<file>.json` in the same
  backend as the media. Stream event sidecars go under `<root>/.streams/<stream_id>/…`
  (`diaspor-events` `VfsEventSink`).
- **Webhooks:** `diaspor-events` signs POST bodies with HMAC-SHA256 in
  `X-Diaspor-Signature: sha256=<hex>` (locked by `tests/webhook_sink_hmac.rs`).
- **Versioning:** adding an optional field is non-breaking (readers ignore
  unknown fields). Renaming, removing, retyping, or tightening a field requires
  a new `*-v2.json` next to the old one, and the old file stays. Doc-only
  tweaks go in `RELEASES.md`. See ADR 0006 and ADR 0007.

## 5. Deployment artifacts (`deploy/`, `website/`)

- `deploy/docker/diaspor-api-server/Dockerfile`: multi-stage (cargo-chef →
  `gcr.io/distroless/cc-debian12`), runs as `nonroot`, sets
  `DIASPOR_BIND_ADDR=0.0.0.0:7733`. Build context is the **repo root**.
- `deploy/helm/diaspor-api-server/`: the source of truth for a production install.
  `DIASPOR_JWT_SECRET` is rendered into a Kubernetes Secret from `secretEnv`.
  It stays `""` in the committed `values.yaml`; pass the real value with `--set`
  or an out-of-tree values file.
- `website/`: static files synced to S3 + CloudFront by `deploy-website.yml` on
  every push to `main` touching `website/**` (OIDC role, no stored keys).
  Screenshots under `website/screenshots/` are real app captures. Replace
  them with new captures and never mock them.

## 6. Build outputs: git-ignored, never commit

`target/` (both workspaces), `dist/` (Vite, tsup), `release-artifacts/`
(desktop CI staging), `node_modules/`, `coverage/`, `*.profraw`/`*.profdata`,
`training/.venv/`, `training/**/__pycache__/`, and any local training output
such as `/tmp/diaspor-artifacts/…`.

Generated but **committed**: `apps/vfs-desktop/src-tauri/gen/schemas/*`
(from `tauri-build`), all lockfiles. Regenerate these with the tool that owns
them and never edit them by hand.

## 7. Checklist before touching an artifact contract

1. Find both sides in the tables above.
2. Change both in one commit, including EN + FR website pages when download
   names are involved.
3. Run the guard: `python3 tools/check_asset_pattern.py`,
   `cargo test -p diaspor-conformance`, `pytest training/tests/test_artifact.py`,
   `cargo test -p diaspor-api-server`, or `cargo test -p diaspor-infer --features hub`
   as applicable.
4. Record user-visible changes in `RELEASES.md`.
