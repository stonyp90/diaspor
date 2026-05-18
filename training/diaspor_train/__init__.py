"""diaspor-train — LoRA training recipes for diaspor.

This package is the Python side of the custom-tier training pipeline whose
Rust trait surface lives in `crates/diaspor-train/`. Recipes here produce
safetensors artifacts that:

1. Match the path convention `tenants/{tenant}/adapters/{adapter}.safetensors`
   defined by `diaspor_train::adapter::AdapterArtifact::path_in_tenant_bucket`
   in the Rust crate.
2. Embed `{tenant_id, adapter_id, base_model}` in safetensors metadata so a
   leaked artifact is attributable.
3. Are signed by the vendor (out-of-band, via the control-plane KMS) and
   counter-signed by the tenant on acceptance.
"""

from .artifact import AdapterArtifactWriter
from .config import LoraConfig, credibility_preset, judge_preset

__all__ = [
    "AdapterArtifactWriter",
    "LoraConfig",
    "credibility_preset",
    "judge_preset",
]
