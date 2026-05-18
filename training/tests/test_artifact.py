"""Smoke tests for AdapterArtifactWriter — runnable without torch/transformers.

The recipe-level smoke test that actually fine-tunes wav2vec2 lives in
`tests/test_recipe_prosody_lora.py` and is gated behind the heavyweight
extras; this file covers the path convention + metadata embedding so the
fast CI path catches drift in the contract before the slow path runs.
"""

from __future__ import annotations

from pathlib import Path

import pytest

# safetensors is a light dep; torch is heavy. We import torch lazily inside
# the test so a CI environment that only wants to verify the contract can
# install `pip install diaspor-train[no-torch]` once we ship that extras.
torch = pytest.importorskip("torch")

from safetensors import safe_open  # noqa: E402

from diaspor_train import AdapterArtifactWriter  # noqa: E402


def test_relative_path_matches_rust_convention():
    writer = AdapterArtifactWriter(
        artifact_root=Path("/ignored"),
        tenant_id="cust_test",
        adapter_id="adapter_xyz",
        base_model="facebook/wav2vec2-base",
    )
    assert (
        writer.relative_path().as_posix() == "tenants/cust_test/adapters/adapter_xyz.safetensors"
    )


def test_write_embeds_metadata(tmp_path: Path):
    writer = AdapterArtifactWriter(
        artifact_root=tmp_path,
        tenant_id="cust_test",
        adapter_id="adapter_xyz",
        base_model="facebook/wav2vec2-base",
    )
    state_dict = {"lora_A.weight": torch.zeros(4, 8), "lora_B.weight": torch.zeros(8, 4)}
    out = writer.write(state_dict, extra_metadata={"diaspor_lora_config": {"rank": 16}})

    assert out == tmp_path / "tenants" / "cust_test" / "adapters" / "adapter_xyz.safetensors"
    assert out.exists()

    with safe_open(str(out), framework="pt") as f:
        meta = f.metadata()
        assert meta["diaspor_adapter_id"] == "adapter_xyz"
        assert meta["diaspor_tenant_id"] == "cust_test"
        assert meta["diaspor_base_model"] == "facebook/wav2vec2-base"
        assert meta["diaspor_format_version"] == "1"
        # extra_metadata dicts are JSON-encoded when they're not strings.
        assert meta["diaspor_lora_config"] == '{"rank": 16}'
        keys = list(f.keys())
        assert set(keys) == {"lora_A.weight", "lora_B.weight"}
