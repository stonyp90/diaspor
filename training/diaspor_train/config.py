"""LoRA configuration shapes — must stay byte-compatible with the Rust side.

The fields here are the union of `crates/diaspor-train/src/lora.rs::LoraConfig`
and the presets at the bottom of that file. Keeping the two in sync is a
manual discipline today; a serde-roundtrip test in `tests/test_config.py`
asserts the JSON shape matches what the Rust `serde(deny_unknown_fields)`
accepts.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass, field


@dataclass(frozen=True)
class LoraConfig:
    rank: int = 16
    alpha: int = 32
    target_modules: list[str] = field(default_factory=lambda: ["q_proj", "v_proj"])
    learning_rate: float = 1e-4
    epochs: int = 8

    def to_json(self) -> dict:
        # Matches the snake_case JSON shape the Rust `LoraConfig` deserializes.
        return asdict(self)


def credibility_preset() -> LoraConfig:
    return LoraConfig(
        rank=32,
        alpha=64,
        target_modules=["q_proj", "k_proj", "v_proj"],
        learning_rate=5e-5,
        epochs=12,
    )


def judge_preset() -> LoraConfig:
    return LoraConfig(
        rank=16,
        alpha=32,
        target_modules=["q_proj", "v_proj"],
        learning_rate=2e-4,
        epochs=6,
    )


# Verticals where credibility-LoRA training MUST be refused. Mirrors the
# `diaspor-train` crate-level compliance note. Recipes that load this list and
# encounter a match should raise rather than train.
CREDIBILITY_REFUSAL_VERTICALS = frozenset({
    "forensic",
    "hiring",
    "insurance",
    "law_enforcement",
    "eu_workplace",
    "eu_education",
})
