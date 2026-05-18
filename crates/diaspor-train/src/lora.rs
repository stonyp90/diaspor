//! `LoRA` trainer trait + configuration shape.
//!
//! "`LoRA`" — Low-Rank Adaptation — fine-tunes a foundation model by injecting trainable
//! rank-`r` matrices into selected linear layers while leaving the base weights frozen.
//! For `diaspor`'s custom tier this gives us a ~1–10 MB tenant-specific delta that
//! composes with a shared 1B-parameter backbone at serving time, instead of a per-tenant
//! 1B checkpoint we'd have to hot-swap on every inference.
//!
//! The [`LoraTrainer`] trait is intentionally narrow: it consumes the embeddings produced
//! by [`crate::foundation::FoundationBackbone`], the labels produced by
//! [`crate::annotate::Annotator`], and a [`LoraConfig`], and emits an
//! [`crate::adapter::AdapterArtifact`] (modulo the eval gate and signature, which the
//! orchestrating pipeline handles).
//!
//! Two named `LoRA` targets ship presets:
//!
//! - [`default_credibility_lora_config`] — credibility / lie-detection adapters. Wider
//!   rank, longer training, because the signal is subtle.
//! - [`default_judge_lora_config`] — sport-judging / scoring adapters. Standard rank,
//!   shorter training, because the rubric is well-defined.

use async_trait::async_trait;
use diaspor_core::{Result, VfsError};
use serde::{Deserialize, Serialize};

use crate::TrainError;
use crate::adapter::{AdapterArtifact, AdapterId, TenantId};
use crate::annotate::AnnotationSet;
use crate::foundation::EmbeddingSet;

/// Hyperparameters for one `LoRA` training run.
///
/// Defaults are the "good enough for most tenants" preset: rank 16, alpha 32, learning
/// rate `1e-4`, 8 epochs. Tenants on the custom tier can override any of these through
/// the control plane; the trainer just consumes the struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct LoraConfig {
    /// `LoRA` rank — the inner dimension of the low-rank decomposition. Higher = more
    /// capacity, larger artifact, slower training.
    pub rank: u8,
    /// `LoRA` alpha — scaling factor applied to the `LoRA` delta at inference time.
    /// Conventionally `2 * rank` for a 1.0 effective scale.
    pub alpha: u8,
    /// Names of the linear modules inside the backbone the `LoRA` should attach to.
    /// Backbone-specific; for `InternVideo2` this is typically `["q_proj", "v_proj"]`.
    pub target_modules: Vec<String>,
    /// `AdamW` learning rate.
    pub learning_rate: f32,
    /// Number of training epochs over the corpus.
    pub epochs: u32,
}

impl Default for LoraConfig {
    fn default() -> Self {
        Self {
            rank: 16,
            alpha: 32,
            target_modules: vec!["q_proj".to_string(), "v_proj".to_string()],
            learning_rate: 1e-4,
            epochs: 8,
        }
    }
}

/// Default [`LoraConfig`] for credibility / lie-detection adapters.
///
/// Wider rank (32) and more epochs (12) than the generic default because the credibility
/// signal is subtle and the per-tenant corpus is usually small. Learning rate is dropped
/// to `5e-5` to keep the `LoRA` from overfitting on the smaller dataset.
///
/// IMPORTANT: this preset MUST NOT be used to train against corpora from `forensic`,
/// `hiring`, `insurance`, `law_enforcement`, `eu_workplace`, or `eu_education`
/// verticals — see the crate-level compliance note. That refusal is enforced at the API
/// layer, but the preset itself is named to make the policy intent obvious.
#[must_use]
pub fn default_credibility_lora_config() -> LoraConfig {
    LoraConfig {
        rank: 32,
        alpha: 64,
        target_modules: vec![
            "q_proj".to_string(),
            "k_proj".to_string(),
            "v_proj".to_string(),
        ],
        learning_rate: 5e-5,
        epochs: 12,
    }
}

/// Default [`LoraConfig`] for sport-judging / scoring adapters.
///
/// Standard rank (16), fewer epochs (6) because the judging rubric is well-defined and
/// the corpus is usually larger and more uniform. Higher learning rate (`2e-4`) speeds up
/// convergence on a tighter loss landscape.
#[must_use]
pub fn default_judge_lora_config() -> LoraConfig {
    LoraConfig {
        rank: 16,
        alpha: 32,
        target_modules: vec!["q_proj".to_string(), "v_proj".to_string()],
        learning_rate: 2e-4,
        epochs: 6,
    }
}

/// Trains a `LoRA` adapter from foundation embeddings + labels.
///
/// The trait deliberately stays narrow: it does not know about the foundation backbone,
/// the eval gate, or signing. Those concerns live in the orchestrating pipeline. The
/// returned artifact is in *vendor-signed-only* state — the orchestrating layer attaches
/// the vendor signature, runs the eval gate, and only then makes it available to the
/// tenant for counter-signing.
#[async_trait]
pub trait LoraTrainer: Send + Sync {
    /// Human-readable name of the trainer backend, for logs and adapter provenance.
    fn name(&self) -> &'static str;

    /// Runs `LoRA` training and returns the resulting artifact.
    ///
    /// `tenant_id` and `adapter_id` are stamped into the artifact so that even a leaked
    /// safetensors blob is attributable to the tenant that received it.
    async fn train(
        &self,
        tenant_id: &TenantId,
        adapter_id: &AdapterId,
        embeddings: &EmbeddingSet,
        labels: &AnnotationSet,
        config: &LoraConfig,
    ) -> Result<AdapterArtifact>;
}

/// No-op `LoRA` trainer used for trait-surface scaffolding and tests.
///
/// Always returns [`TrainError::NotImplemented`] wrapped into a [`VfsError::Backend`].
/// Replace with the real trainer (PEFT + safetensors export) in milestone M9.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopLoraTrainer;

#[async_trait]
impl LoraTrainer for NoopLoraTrainer {
    fn name(&self) -> &'static str {
        "noop-lora"
    }

    async fn train(
        &self,
        _tenant_id: &TenantId,
        _adapter_id: &AdapterId,
        _embeddings: &EmbeddingSet,
        _labels: &AnnotationSet,
        _config: &LoraConfig,
    ) -> Result<AdapterArtifact> {
        Err(VfsError::from(TrainError::NotImplemented {
            stage: "noop-lora-trainer",
        }))
    }
}
