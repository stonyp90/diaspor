"""Toy LoRA recipe: fine-tune wav2vec2-base for a 2-class prosody task.

This is the end-to-end demonstration recipe — small enough to run on a
laptop CPU in ~5 minutes, large enough to exercise every load-bearing step
(load frozen backbone -> attach peft LoRA adapter -> train on a synthetic
dataset -> export adapter via AdapterArtifactWriter).

The backbone is `facebook/wav2vec2-base`, the same model pinned at
`wav2vec2-base@1` in `../models.toml`. The recipe deliberately uses a
synthetic dataset so the smoke test does not depend on a real labeled corpus;
real recipes swap `_make_synthetic_dataset` for a `datasets.load_dataset(...)`
or an S3 corpus walker.

Run:
    python -m recipes.prosody_lora \
        --tenant cust_demo --adapter adapter_demo_001 \
        --artifact-root /tmp/diaspor-artifacts
"""

from __future__ import annotations

import argparse
import logging
import sys
from pathlib import Path

import numpy as np
import torch
from peft import LoraConfig as PeftLoraConfig
from peft import get_peft_model, get_peft_model_state_dict
from torch.utils.data import DataLoader, Dataset
from transformers import (
    AutoFeatureExtractor,
    Wav2Vec2ForSequenceClassification,
)

# Make `import diaspor_train` work whether the user `pip install -e .`'d the
# package or runs the file directly from a fresh checkout.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from diaspor_train import AdapterArtifactWriter, LoraConfig, judge_preset  # noqa: E402

LOG = logging.getLogger("diaspor_train.recipes.prosody_lora")

BASE_MODEL = "facebook/wav2vec2-base"
SAMPLE_RATE = 16_000
# Wav2Vec2 attention projections live under `wav2vec2.encoder.layers.*.attention`
# — the bare module names `q_proj`, `k_proj`, `v_proj` are unique within those
# blocks so peft picks them up across all 12 layers without further qualifier.
WAV2VEC2_TARGET_MODULES = ["q_proj", "v_proj"]


class SyntheticProsodyDataset(Dataset):
    """Two-class dataset: noise vs. 440 Hz tone.

    Stand-in for the real "tenant-labeled prosody clips" corpus the production
    pipeline ingests. Class 0 is white noise, class 1 is a windowed sine —
    trivially separable, so a few hundred LoRA-trained parameters can solve it
    inside the training budget the smoke test allows.
    """

    def __init__(self, n_per_class: int = 32, duration_s: float = 1.0, seed: int = 0):
        rng = np.random.default_rng(seed)
        n_samples = int(duration_s * SAMPLE_RATE)
        clips: list[np.ndarray] = []
        labels: list[int] = []
        t = np.linspace(0.0, duration_s, n_samples, endpoint=False, dtype=np.float32)
        for _ in range(n_per_class):
            clips.append(rng.standard_normal(n_samples).astype(np.float32) * 0.1)
            labels.append(0)
            tone = np.sin(2 * np.pi * 440.0 * t).astype(np.float32) * 0.3
            tone += rng.standard_normal(n_samples).astype(np.float32) * 0.02
            clips.append(tone)
            labels.append(1)
        self.clips = clips
        self.labels = labels

    def __len__(self) -> int:
        return len(self.clips)

    def __getitem__(self, idx: int):
        return self.clips[idx], self.labels[idx]


def _collate(batch, feature_extractor):
    audios = [item[0] for item in batch]
    labels = torch.tensor([item[1] for item in batch], dtype=torch.long)
    enc = feature_extractor(
        audios,
        sampling_rate=SAMPLE_RATE,
        return_tensors="pt",
        padding=True,
    )
    return enc["input_values"], enc.get("attention_mask"), labels


def train(
    tenant_id: str,
    adapter_id: str,
    artifact_root: Path,
    lora: LoraConfig,
    n_per_class: int = 32,
    batch_size: int = 4,
) -> Path:
    feature_extractor = AutoFeatureExtractor.from_pretrained(BASE_MODEL)
    model = Wav2Vec2ForSequenceClassification.from_pretrained(BASE_MODEL, num_labels=2)
    # Wav2Vec2 doesn't implement `get_input_embeddings()` — it has no token table.
    # peft's default gradient-checkpointing prep calls into that path and crashes;
    # since our synthetic dataset is tiny we don't need checkpointing anyway.
    if hasattr(model, "gradient_checkpointing_disable"):
        model.gradient_checkpointing_disable()

    peft_config = PeftLoraConfig(
        # Deliberately no `task_type=` — peft's SEQ_CLS path injects an `input_ids`
        # kwarg into the wrapped forward(), but Wav2Vec2's forward() takes
        # `input_values` (audio is not tokenised). Leaving task_type unset keeps
        # peft as a pure LoRA injector that passes through whatever kwargs the
        # caller supplies.
        r=lora.rank,
        lora_alpha=lora.alpha,
        target_modules=WAV2VEC2_TARGET_MODULES,
        # Train the classifier head alongside the LoRA adapters so the artifact is
        # actually serviceable; without this, only LoRA deltas land in the file and
        # the head stays at HF's random init.
        modules_to_save=["classifier"],
        lora_dropout=0.05,
        bias="none",
    )
    model = get_peft_model(model, peft_config)
    model.print_trainable_parameters()

    dataset = SyntheticProsodyDataset(n_per_class=n_per_class)
    loader = DataLoader(
        dataset,
        batch_size=batch_size,
        shuffle=True,
        collate_fn=lambda b: _collate(b, feature_extractor),
    )

    optimizer = torch.optim.AdamW(
        [p for p in model.parameters() if p.requires_grad],
        lr=lora.learning_rate,
    )
    model.train()
    for epoch in range(lora.epochs):
        running = 0.0
        for input_values, attention_mask, labels in loader:
            optimizer.zero_grad()
            outputs = model(
                input_values=input_values,
                attention_mask=attention_mask,
                labels=labels,
            )
            outputs.loss.backward()
            optimizer.step()
            running += float(outputs.loss.detach())
        LOG.info("epoch %d loss=%.4f", epoch, running / len(loader))

    writer = AdapterArtifactWriter(
        artifact_root=artifact_root,
        tenant_id=tenant_id,
        adapter_id=adapter_id,
        base_model=BASE_MODEL,
    )
    # `get_peft_model_state_dict` returns just the LoRA delta — the frozen
    # backbone weights stay out of the artifact, matching the Rust-side
    # promise that the saved adapter is small enough to ship as a 1–10 MB
    # per-tenant blob.
    out = writer.write(
        get_peft_model_state_dict(model),
        extra_metadata={"diaspor_lora_config": lora.to_json()},
    )
    LOG.info("wrote adapter to %s", out)
    return out


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tenant", required=True, help="Tenant id (cust_*).")
    parser.add_argument("--adapter", required=True, help="Adapter id stamped into the artifact.")
    parser.add_argument(
        "--artifact-root",
        type=Path,
        required=True,
        help="Local root that the canonical tenants/<id>/adapters/<id>.safetensors path is anchored at.",
    )
    parser.add_argument(
        "--n-per-class",
        type=int,
        default=32,
        help="Per-class synthetic samples. Smoke test uses 32; production swaps in a real loader.",
    )
    parser.add_argument(
        "--epochs",
        type=int,
        default=None,
        help="Override the judge_preset() epoch count (defaults to 6).",
    )
    args = parser.parse_args(argv)

    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(name)s %(levelname)s %(message)s")

    lora = judge_preset()
    if args.epochs is not None:
        lora = LoraConfig(
            rank=lora.rank,
            alpha=lora.alpha,
            target_modules=list(lora.target_modules),
            learning_rate=lora.learning_rate,
            epochs=args.epochs,
        )

    out = train(
        tenant_id=args.tenant,
        adapter_id=args.adapter,
        artifact_root=args.artifact_root,
        lora=lora,
        n_per_class=args.n_per_class,
    )
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
