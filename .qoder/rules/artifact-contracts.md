---
trigger: model_decision
description: Apply when a task touches release or packaging workflows (.github/workflows/release*.yml), website download buttons, models.toml or model weights, docs/schema or docs/api, the Tauri updater, Docker/Helm under deploy/, or LoRA adapter output in training/ or crates/diaspor-train.
---

# Artifact contracts

Read `docs/ARTIFACTS.md` first. It lists every artifact the repo produces or
consumes, the exact naming/shape contract, both sides of each contract, and the
guard that enforces it. The contracts that most often break:

1. **Release asset names ↔ website JS.** CLI archives are
   `diaspor-<tag>-<target>.<tar.gz|zip>`. The `release.yml` `build-cli` matrix
   must equal `const ARCHIVES` in `website/index.html`. Desktop installers are
   exactly `diaspor-vfs.dmg|msi|AppImage` and must match `const ASSETS` in the EN
   and FR pages. Guard: `python3 tools/check_asset_pattern.py`.
2. **LoRA adapters.** Path `tenants/{tenant}/adapters/{adapter}.safetensors` and
   the `diaspor_*` safetensors metadata keys. Rust
   `AdapterArtifact::path_in_tenant_bucket` must match Python
   `AdapterArtifactWriter`. Guard: `pytest training/tests/test_artifact.py`.
3. **Schemas.** `docs/schema/score-v1.json` must match the Rust record types in
   `diaspor-vision` and the SDK models. Guard: `cargo test -p diaspor-conformance`.
   A breaking change means a new `*-v2.json` file.
4. **Model catalog.** A `models.toml` entry is pinned only with a real `sha256`
   and `bytes`. Never invent a hash. `nonfree` weights need `--features nonfree-models`.
5. **Desktop updater.** `latest.json` is not published by any workflow, and the
   committed `apps/vfs-desktop/latest.json` is stale. Treat this as a known gap.

When one side of a contract changes, change the other side in the same commit,
run the guard, and note user-visible changes in `RELEASES.md`.
