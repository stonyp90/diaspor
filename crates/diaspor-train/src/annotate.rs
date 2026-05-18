//! Annotation — turn an ingested corpus into labeled training examples.
//!
//! The training pipeline only consumes labels through the [`Annotator`] trait, so the
//! same downstream `LoRA` trainer works whether labels arrive from CVAT, Label Studio, an
//! in-house tool, or a hand-rolled JSON sidecar. The alpha ships two named backend stubs
//! for the two annotation servers `diaspor` deployments use most often:
//!
//! - [`CvatAnnotator`] — self-hosted Computer Vision Annotation Tool. The customer's
//!   CVAT instance keeps the raw labels; the trainer pulls a normalized export at
//!   training time.
//! - [`LabelStudioAnnotator`] — Label Studio Enterprise or community edition. Same
//!   shape: server URL + API key, normalized export at training time.
//!
//! Both backends return [`crate::TrainError::NotImplemented`] in the alpha; real REST
//! wiring lands in milestone M9.

use async_trait::async_trait;
use diaspor_core::{Result, VfsError};

use crate::TrainError;
use crate::corpus::CorpusManifest;

/// One label associated with one clip in the corpus.
///
/// The `label` string is opaque — its meaning is dictated by the `LoRA` target (a judging
/// rubric score, a credibility binary, a sport-specific class) and is preserved through
/// to the trainer unchanged.
#[derive(Debug, Clone)]
pub struct LabeledClip {
    /// Identifier of the clip inside the corpus. Matches the path that came out of
    /// [`crate::corpus::CorpusIngest`].
    pub clip_id: String,
    /// The annotation value. Opaque string; the trainer interprets it via the `LoRA`
    /// target configuration.
    pub label: String,
    /// Optional inter-annotator agreement score in `[0.0, 1.0]`. `None` for backends
    /// that don't run multi-annotator consensus.
    pub agreement: Option<f32>,
}

/// Result of an annotation pass over a corpus.
///
/// Labels are emitted in the same order the corpus manifest enumerated clips, but the
/// trainer does NOT rely on that order — it always joins on `clip_id`.
#[derive(Debug, Clone)]
pub struct AnnotationSet {
    /// One entry per labeled clip. Clips that were skipped by the annotator (low
    /// quality, withdrawn consent) are simply omitted; the trainer treats them as
    /// missing.
    pub clips: Vec<LabeledClip>,
}

/// Pulls labels from an annotation backend keyed on a corpus manifest.
#[async_trait]
pub trait Annotator: Send + Sync {
    /// Human-readable name of the backend, for logs and provenance records.
    fn name(&self) -> &'static str;

    /// Fetches the annotation set for the clips described by `manifest`.
    async fn annotate(&self, manifest: &CorpusManifest) -> Result<AnnotationSet>;
}

/// Self-hosted CVAT (Computer Vision Annotation Tool) backend stub.
///
/// CVAT exposes a REST API at `/api/v1` and pages results; the real backend will paginate
/// and project per-task labels into [`LabeledClip`] values. The alpha just carries the
/// connection parameters and returns [`TrainError::NotImplemented`].
#[derive(Debug, Clone)]
pub struct CvatAnnotator {
    /// Base URL of the customer's CVAT instance, e.g. `https://cvat.example.com`.
    pub server_url: String,
    /// API key with read access to the customer's CVAT projects. Treated as a secret —
    /// callers must not log this verbatim.
    pub api_key: String,
}

impl CvatAnnotator {
    /// Constructs a CVAT annotator stub from its connection parameters.
    pub fn new(server_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            api_key: api_key.into(),
        }
    }
}

#[async_trait]
impl Annotator for CvatAnnotator {
    fn name(&self) -> &'static str {
        "cvat"
    }

    async fn annotate(&self, _manifest: &CorpusManifest) -> Result<AnnotationSet> {
        Err(VfsError::from(TrainError::NotImplemented {
            stage: "cvat-annotator",
        }))
    }
}

/// Label Studio backend stub.
///
/// Same shape as [`CvatAnnotator`] — connection parameters now, real REST wiring in
/// milestone M9.
#[derive(Debug, Clone)]
pub struct LabelStudioAnnotator {
    /// Base URL of the customer's Label Studio instance.
    pub server_url: String,
    /// API key with read access to the customer's Label Studio projects. Treated as a
    /// secret — callers must not log this verbatim.
    pub api_key: String,
}

impl LabelStudioAnnotator {
    /// Constructs a Label Studio annotator stub from its connection parameters.
    pub fn new(server_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            api_key: api_key.into(),
        }
    }
}

#[async_trait]
impl Annotator for LabelStudioAnnotator {
    fn name(&self) -> &'static str {
        "label-studio"
    }

    async fn annotate(&self, _manifest: &CorpusManifest) -> Result<AnnotationSet> {
        Err(VfsError::from(TrainError::NotImplemented {
            stage: "label-studio-annotator",
        }))
    }
}
