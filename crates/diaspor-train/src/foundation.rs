//! Foundation backbones — produce frozen embeddings the `LoRA` trainer fine-tunes on top of.
//!
//! Custom-tier training does NOT touch the foundation model weights themselves. Instead,
//! every clip in the annotated corpus is run through a frozen backbone to produce a
//! compact embedding, and the `LoRA` delta is trained over those embeddings. This keeps the
//! training pass cheap enough to run on per-tenant GPUs while still benefiting from the
//! representation quality of a 1B-parameter video model.
//!
//! Three backbones are wired through the trait surface at the alpha stage:
//!
//! - [`InternVideo2Backbone`] — production default. `InternVideo2` 1B, Apache-2.0
//!   licensed, video understanding focus. The shipped artifact is the
//!   `InternVideo2-Stage2_1B-224p-f4` checkpoint repackaged for our serving stack.
//! - [`VideoMaeV2Backbone`] — fallback for clips where `InternVideo2` underperforms (very
//!   short, very low-resolution, single-shot sports clips).
//! - [`HuBertLargeBackbone`] — audio modality. Used when the `LoRA` target depends on
//!   prosody / paralinguistic features rather than video.
//!
//! All three are stubs in the alpha. Real wiring lands in milestone M9.

use std::path::PathBuf;

use async_trait::async_trait;
use bytes::Bytes;
use diaspor_core::{Result, VfsError};
use serde::{Deserialize, Serialize};

use crate::TrainError;
use crate::annotate::AnnotationSet;

/// Frozen embeddings extracted from a corpus by a foundation backbone.
///
/// One embedding per clip in the [`AnnotationSet`] the backbone was called with, in the
/// same logical order. The bytes are opaque — layout (FP16 vs FP32, row-major shape) is
/// dictated by the backbone and is documented in its module-level docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct EmbeddingSet {
    /// One opaque tensor blob per clip.
    pub embeddings: Vec<ClipEmbedding>,
    /// Embedding dimensionality (the trailing axis of each blob).
    pub dim: u32,
    /// Name of the backbone that produced these embeddings, stamped through to the
    /// adapter artifact for provenance.
    pub backbone_name: String,
}

/// One clip's embedding blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ClipEmbedding {
    /// Clip identifier matching the [`crate::annotate::LabeledClip`] this came from.
    pub clip_id: String,
    /// Opaque tensor bytes. Shape and dtype are documented per-backbone.
    pub tensor: Bytes,
}

/// Tunable parameters for [`InternVideo2Backbone`].
///
/// Defaults match the values we use in production: 4 frames per clip at 224×224, FP16
/// output. Bump `frames_per_clip` for longer-context clips at the cost of inference
/// time; the trainer downstream is shape-agnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct InternVideoParams {
    /// Number of frames sampled per clip before they're fed through the backbone.
    pub frames_per_clip: u32,
    /// Spatial resolution the backbone expects.
    pub input_resolution: u32,
    /// Whether to emit FP16 (`true`) or FP32 (`false`) embeddings.
    pub use_fp16: bool,
}

impl Default for InternVideoParams {
    fn default() -> Self {
        Self {
            frames_per_clip: 4,
            input_resolution: 224,
            use_fp16: true,
        }
    }
}

/// Produces frozen embeddings from a [`crate::annotate::AnnotationSet`].
///
/// Implementations are stateful at the backbone level (loading 1B parameters is not
/// free) and SHOULD reuse a single loaded model across calls within a training job.
#[async_trait]
pub trait FoundationBackbone: Send + Sync {
    /// Human-readable name of the backbone, for logs and adapter provenance.
    fn name(&self) -> &'static str;

    /// Runs the backbone over the clips in `annotations` and returns their embeddings.
    async fn extract_embeddings(&self, annotations: &AnnotationSet) -> Result<EmbeddingSet>;
}

/// `InternVideo2` 1B backbone stub — the production default.
///
/// `InternVideo2` is Apache-2.0 licensed and the strongest open video-understanding
/// foundation model in the size class. The real backend will load the safetensors
/// checkpoint at `model_path` and run inference through `diaspor-infer`; the alpha just
/// returns [`TrainError::NotImplemented`].
#[derive(Debug, Clone)]
pub struct InternVideo2Backbone {
    /// On-disk path to the `InternVideo2` safetensors checkpoint.
    pub model_path: PathBuf,
    /// Tunables for sampling, resolution, and precision.
    pub params: InternVideoParams,
}

impl InternVideo2Backbone {
    /// Constructs an `InternVideo2` backbone stub from its checkpoint path and params.
    pub const fn new(model_path: PathBuf, params: InternVideoParams) -> Self {
        Self { model_path, params }
    }

    /// Constructs an `InternVideo2` backbone with the default [`InternVideoParams`].
    pub fn with_defaults(model_path: PathBuf) -> Self {
        Self::new(model_path, InternVideoParams::default())
    }
}

#[async_trait]
impl FoundationBackbone for InternVideo2Backbone {
    fn name(&self) -> &'static str {
        "internvideo2-1b"
    }

    async fn extract_embeddings(&self, _annotations: &AnnotationSet) -> Result<EmbeddingSet> {
        Err(VfsError::from(TrainError::NotImplemented {
            stage: "internvideo2-backbone",
        }))
    }
}

/// `VideoMAE` v2 backbone stub — fallback for clips where `InternVideo2` underperforms.
///
/// Used when the corpus is dominated by very short or low-resolution clips. Real wiring
/// in milestone M9; the alpha returns [`TrainError::NotImplemented`].
#[derive(Debug, Clone)]
pub struct VideoMaeV2Backbone {
    /// On-disk path to the `VideoMAE` v2 safetensors checkpoint.
    pub model_path: PathBuf,
}

impl VideoMaeV2Backbone {
    /// Constructs a `VideoMAE` v2 backbone stub from its checkpoint path.
    pub const fn new(model_path: PathBuf) -> Self {
        Self { model_path }
    }
}

#[async_trait]
impl FoundationBackbone for VideoMaeV2Backbone {
    fn name(&self) -> &'static str {
        "videomae-v2"
    }

    async fn extract_embeddings(&self, _annotations: &AnnotationSet) -> Result<EmbeddingSet> {
        Err(VfsError::from(TrainError::NotImplemented {
            stage: "videomae-v2-backbone",
        }))
    }
}

/// HuBERT-Large backbone stub — audio modality.
///
/// Selected when the `LoRA` target depends on prosody / paralinguistic features rather
/// than video frames. Real wiring in milestone M9; the alpha returns
/// [`TrainError::NotImplemented`].
#[derive(Debug, Clone)]
pub struct HuBertLargeBackbone {
    /// On-disk path to the HuBERT-Large safetensors checkpoint.
    pub model_path: PathBuf,
}

impl HuBertLargeBackbone {
    /// Constructs a HuBERT-Large backbone stub from its checkpoint path.
    pub const fn new(model_path: PathBuf) -> Self {
        Self { model_path }
    }
}

#[async_trait]
impl FoundationBackbone for HuBertLargeBackbone {
    fn name(&self) -> &'static str {
        "hubert-large"
    }

    async fn extract_embeddings(&self, _annotations: &AnnotationSet) -> Result<EmbeddingSet> {
        Err(VfsError::from(TrainError::NotImplemented {
            stage: "hubert-large-backbone",
        }))
    }
}
