"""Adapter artifact writer — produces safetensors files that match the path
convention enforced by the Rust `AdapterArtifact::path_in_tenant_bucket`.

The Rust side embeds vendor + tenant Ed25519 signatures in a wrapping
"handoff" envelope (defined outside the `diaspor-train` crate). Here we only
produce the inner safetensors blob; signing is the control plane's job and
happens after the recipe writes the file.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from safetensors.torch import save_file


@dataclass(frozen=True)
class AdapterArtifactWriter:
    """Writes a LoRA delta to the canonical tenant-bucket path.

    The output is a single `.safetensors` file. Metadata fields embedded inside
    it follow the convention the Rust serving layer reads when loading the
    adapter.
    """

    artifact_root: Path
    tenant_id: str
    adapter_id: str
    base_model: str

    def relative_path(self) -> Path:
        # Mirrors the format string in
        # crates/diaspor-train/src/adapter.rs::AdapterArtifact::path_in_tenant_bucket.
        return Path("tenants") / self.tenant_id / "adapters" / f"{self.adapter_id}.safetensors"

    def absolute_path(self) -> Path:
        return self.artifact_root / self.relative_path()

    def write(self, state_dict: dict, extra_metadata: dict | None = None) -> Path:
        """Saves `state_dict` as a safetensors file at the canonical path.

        `state_dict` is a `{name: torch.Tensor}` map — typically the LoRA delta
        weights extracted from a peft-wrapped model via
        `peft.get_peft_model_state_dict(model)`. We do not save the frozen
        backbone weights; those stay shared across tenants on the serving side.
        """
        metadata: dict[str, str] = {
            "diaspor_adapter_id": self.adapter_id,
            "diaspor_tenant_id": self.tenant_id,
            "diaspor_base_model": self.base_model,
            "diaspor_trained_at": datetime.now(tz=timezone.utc).isoformat(),
            "diaspor_format_version": "1",
        }
        if extra_metadata:
            for k, v in extra_metadata.items():
                # safetensors metadata values must be strings; non-string
                # values get JSON-encoded so the round-trip stays lossless.
                metadata[k] = v if isinstance(v, str) else json.dumps(v, sort_keys=True)

        out = self.absolute_path()
        out.parent.mkdir(parents=True, exist_ok=True)
        save_file(state_dict, str(out), metadata=metadata)
        return out
