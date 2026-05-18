# diaspor-train (Python)

LoRA training recipes for diaspor. Each recipe fine-tunes one of the open-source
foundation backbones cataloged in [`../models.toml`](../models.toml) against a
tenant-owned corpus, and writes the resulting LoRA delta as a `.safetensors`
file at the canonical path `tenants/{tenant}/adapters/{adapter}.safetensors`.

This package is the Python side of the custom-tier training pipeline. The Rust
trait surface that consumes the artifacts produced here lives in
[`../crates/diaspor-train/`](../crates/diaspor-train/). The two share:

- The path convention (`AdapterArtifact::path_in_tenant_bucket` in Rust ↔
  `AdapterArtifactWriter.relative_path` in Python).
- The LoRA config shape (`LoraConfig` in Rust ↔ `LoraConfig` in Python, both
  with the same JSON field names).
- The compliance refusal list (`forensic`, `hiring`, `insurance`,
  `law_enforcement`, `eu_workplace`, `eu_education` — enforced at the API
  layer, mirrored in `diaspor_train.config.CREDIBILITY_REFUSAL_VERTICALS`).

## Install

```bash
cd training
python -m venv .venv && source .venv/bin/activate
pip install -e .[dev]
```

## Run the demo recipe

```bash
python -m recipes.prosody_lora \
    --tenant cust_demo \
    --adapter adapter_demo_001 \
    --artifact-root /tmp/diaspor-artifacts \
    --epochs 2
```

`prosody_lora.py` LoRA-fine-tunes `facebook/wav2vec2-base` (pinned at
`wav2vec2-base@1` in `../models.toml`) on a synthetic two-class dataset (noise
vs. 440 Hz tone). It's small enough to run on a laptop CPU in ~5 minutes; the
point is to exercise every step end-to-end so the production recipes that
substitute real datasets stay on a path that already works.

The recipe writes:

```
/tmp/diaspor-artifacts/tenants/cust_demo/adapters/adapter_demo_001.safetensors
```

Inspect the metadata to verify the artifact is attributable:

```bash
python -c '
from safetensors import safe_open
with safe_open("/tmp/diaspor-artifacts/tenants/cust_demo/adapters/adapter_demo_001.safetensors", framework="pt") as f:
    for k, v in f.metadata().items():
        print(f"{k}: {v}")
'
```

## What's not in this scaffold

- Real video recipes (judge_diving_lora.py, action_lora.py). The video extras
  in `pyproject.toml` (`[video]`) cover the `decord` / `av` deps these will
  need; we hold off on the recipes themselves until milestone M9 because each
  one wants a curated public eval set and a non-trivial sport-rubric annotator.
- Vendor + tenant Ed25519 signing. That happens in the control plane after the
  recipe finishes writing the file, not inside the recipe itself.
- Distributed training. Recipes target single-GPU and CPU; the production
  pipeline parallelises via the orchestrator layer outside this package.
