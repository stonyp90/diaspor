//! # diaspor-train
//!
//! Custom-tier `LoRA` training pipeline for `diaspor`.
//!
//! This crate is the orchestration surface for `diaspor`'s **Custom tier**: per-tenant
//! `LoRA` fine-tuning of a frozen video (or audio) foundation model — `InternVideo2` 1B by
//! default, with `VideoMAE` v2 and HuBERT-Large as alternates — against the tenant's own
//! corpus, followed by an eval-gated, dual-signed handoff back to the tenant. The
//! "weights stay yours" guarantee is enforced by:
//!
//! 1. Training happens in a per-tenant compute boundary; the corpus never crosses into
//!    a shared pool.
//! 2. The resulting [`adapter::AdapterArtifact`] is signed by the vendor (`diaspor`
//!    `SaaS`) AND counter-signed by the tenant on acceptance, so even an internal copy
//!    cannot be loaded into a serving slot without both keys.
//! 3. The artifact lands in the tenant's own S3 bucket at the canonical path
//!    [`adapter::AdapterArtifact::path_in_tenant_bucket`]; nothing in this crate writes
//!    to a vendor-owned location.
//!
//! ## Pipeline at a glance
//!
//! ```text
//!   ┌──────────┐   ingest   ┌──────────┐   labels   ┌──────────┐   embed   ┌─────────┐
//!   │  tenant  │ ─────────▶ │ manifest │ ─────────▶ │annotated │ ────────▶ │embeddings│
//!   │  corpus  │            │ + sha256 │            │ corpus   │           │  frozen │
//!   └──────────┘            └──────────┘            └──────────┘           └────┬────┘
//!                                                                                │
//!                       ┌───────── eval-gate ◀──── LoRA train ◀──────────────────┘
//!                       ▼
//!                ┌────────────────┐
//!                │  AdapterArt.   │  vendor-signed ─▶ tenant counter-sign ─▶ serve
//!                │  + EvalReport  │
//!                └────────────────┘
//! ```
//!
//! ## Composition
//!
//! The full pipeline is generic over five swappable stages:
//!
//! - [`corpus::CorpusIngest`] — walks an S3 prefix or VFS path into a manifest.
//! - [`annotate::Annotator`] — pulls labels from CVAT, Label Studio, or in-house.
//! - [`foundation::FoundationBackbone`] — produces frozen embeddings.
//! - [`lora::LoraTrainer`] — trains the `LoRA` delta.
//! - [`eval::EvalGate`] — gates handoff on a held-out evaluation.
//!
//! Use [`TrainingPipeline`] to compose them; see [`TrainingPipeline::train`] for the
//! orchestration.
//!
//! ## Compliance — credibility-LoRA refusal list
//!
//! The credibility-LoRA target (see [`lora::default_credibility_lora_config`]) MUST NOT
//! be trained against corpora whose tenant has declared one of these verticals:
//!
//! - `forensic`
//! - `hiring`
//! - `insurance`
//! - `law_enforcement`
//! - `eu_workplace`
//! - `eu_education`
//!
//! This refusal is enforced at the **API layer** (Phase 4 of the build plan) — the
//! control plane checks the tenant's declared vertical against this list before it ever
//! calls into `diaspor-train`. The invariant is documented here because it is load-
//! bearing: training a credibility adapter for any of those verticals would put us on
//! the wrong side of EU AI Act Article 5, Loi 25, BIPA, or sector-specific use-case
//! bans, regardless of how strong our internal eval gate is.
//!
//! ## Status
//!
//! v0.1.0-alpha ships the trait surface only. Full `LoRA` training pipeline lands in
//! milestone M9 (custom tier).

#![doc(html_root_url = "https://docs.rs/diaspor-train/0.1.0-alpha.1")]

use std::path::Path;

use diaspor_core::Result;
use thiserror::Error;

pub mod adapter;
pub mod annotate;
pub mod corpus;
pub mod eval;
pub mod foundation;
pub mod lora;

pub use adapter::{AdapterArtifact, AdapterId, TenantId};
pub use annotate::{AnnotationSet, Annotator, CvatAnnotator, LabelStudioAnnotator, LabeledClip};
pub use corpus::{CorpusIngest, CorpusManifest, CorpusSource, NoopCorpusIngest};
pub use eval::{EvalGate, EvalReport, NoopEvalGate};
pub use foundation::{
    ClipEmbedding, EmbeddingSet, FoundationBackbone, HuBertLargeBackbone, InternVideo2Backbone,
    InternVideoParams, VideoMaeV2Backbone,
};
pub use lora::{
    LoraConfig, LoraTrainer, NoopLoraTrainer, default_credibility_lora_config,
    default_judge_lora_config,
};

/// Things that can go wrong inside the custom-tier training pipeline.
///
/// Wraps cleanly into a [`diaspor_core::VfsError::Backend`] when bubbled up through the
/// VFS layer. Most call sites consume this type unwrapped: the control plane needs to
/// distinguish between "the corpus walk failed" (tenant misconfiguration) and "the eval
/// gate rejected the adapter" (a normal regression we surface to the tenant).
#[derive(Debug, Error)]
pub enum TrainError {
    /// A pipeline stage exists as a trait stub but no real implementation has shipped
    /// yet. The string identifies which stage hit the stub so the operator log shows
    /// exactly which milestone is gating the caller.
    #[error("training stage `{stage}` is not implemented yet")]
    NotImplemented {
        /// Short stage name (`"noop-corpus"`, `"cvat-annotator"`, `"internvideo2-backbone"`,
        /// `"noop-lora-trainer"`, `"noop-eval-gate"`, ...).
        stage: &'static str,
    },

    /// The pipeline was asked to train for a tenant + vertical combination that the
    /// compliance refusal list blocks. See the crate-level compliance note.
    #[error("vertical `{vertical}` is on the credibility-LoRA refusal list for tenant `{tenant}`")]
    VerticalRefused {
        /// Tenant the refusal applies to.
        tenant: String,
        /// Vertical that triggered the refusal.
        vertical: String,
    },

    /// The eval gate ran successfully but reported [`EvalReport::passed`] as `false`. The
    /// adapter is dropped and the tenant is notified.
    #[error(
        "eval gate rejected adapter: metric={metric} baseline={baseline} new={new} delta={delta}"
    )]
    EvalGateRejected {
        /// Name of the metric that failed.
        metric: String,
        /// Baseline score.
        baseline: f64,
        /// New (LoRA-adapted) score.
        new: f64,
        /// Difference (`new - baseline`).
        delta: f64,
    },

    /// Signing the adapter failed (KMS unreachable, key rotation in progress, ...).
    #[error("signing failed: {0}")]
    SigningFailed(String),

    /// A pipeline stage was given inputs it could not consume (e.g. an empty corpus or a
    /// label set with no matching clips).
    #[error("invalid pipeline input at stage `{stage}`: {reason}")]
    InvalidInput {
        /// Stage that rejected the input.
        stage: &'static str,
        /// Human-readable rejection reason.
        reason: String,
    },
}

impl From<TrainError> for diaspor_core::VfsError {
    fn from(err: TrainError) -> Self {
        Self::Backend(err.to_string())
    }
}

/// The composed custom-tier training pipeline.
///
/// Generic over the five swappable stages so production code can wire real
/// implementations while tests wire the `Noop*` stubs. The five type parameters keep the
/// pipeline static-dispatch — there is no boxed trait object overhead — at the cost of a
/// slightly chunky signature.
pub struct TrainingPipeline<C, A, F, T, E> {
    /// Corpus ingest stage (S3 prefix walker, VFS walker, ...).
    pub corpus: C,
    /// Annotation stage (CVAT, Label Studio, ...).
    pub annotator: A,
    /// Foundation backbone stage (`InternVideo2`, `VideoMAE` v2, HuBERT-Large, ...).
    pub backbone: F,
    /// `LoRA` training stage.
    pub trainer: T,
    /// Eval-gate stage that decides whether the artifact is fit for tenant handoff.
    pub eval_gate: E,
}

impl<C, A, F, T, E> TrainingPipeline<C, A, F, T, E>
where
    C: CorpusIngest,
    A: Annotator,
    F: FoundationBackbone,
    T: LoraTrainer,
    E: EvalGate,
{
    /// Runs the full pipeline end-to-end: corpus → annotate → embed → train → eval-gate.
    ///
    /// The vendor signature on the returned [`AdapterArtifact`] is a placeholder in the
    /// alpha — real Ed25519 signing lands with the rest of the pipeline in milestone M9.
    /// The artifact returned here is in *vendor-signed-only* state; the tenant counter-
    /// signature is attached out-of-band when the tenant accepts the handoff.
    ///
    /// # Errors
    ///
    /// Bubbles up the first error from any stage. The eval gate's "rejected" verdict is
    /// surfaced as a [`TrainError::EvalGateRejected`] (not a backend failure) so the
    /// control plane can route it to the tenant without paging an operator.
    pub async fn train(
        &self,
        tenant: &TenantId,
        source: &CorpusSource,
        lora: &LoraConfig,
        eval_set: &Path,
    ) -> Result<AdapterArtifact> {
        let manifest = self.corpus.ingest(tenant, source).await?;
        let labels = self.annotator.annotate(&manifest).await?;
        let embeddings = self.backbone.extract_embeddings(&labels).await?;
        // The adapter id is minted up-front so the trainer can stamp it into the
        // safetensors metadata; the real pipeline will source this from the control
        // plane to keep ids globally unique. For the alpha trait surface a placeholder
        // is fine — the trainer stub never reads it.
        let adapter_id = AdapterId::new(format!("adapter-{tenant}"));
        let artifact = self
            .trainer
            .train(tenant, &adapter_id, &embeddings, &labels, lora)
            .await?;
        let report = self.eval_gate.evaluate(&artifact, eval_set).await?;
        if !report.passed {
            return Err(diaspor_core::VfsError::from(TrainError::EvalGateRejected {
                metric: report.metric_name,
                baseline: report.baseline_score,
                new: report.new_score,
                delta: report.delta,
            }));
        }
        // Attach the eval report; vendor signature is left as the placeholder the trainer
        // populated, and tenant_signature stays `None` until the tenant counter-signs.
        let mut artifact = artifact;
        artifact.eval_report = Some(report);
        Ok(artifact)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[tokio::test]
    async fn full_stub_pipeline_bubbles_not_implemented_from_corpus_stage() {
        let pipeline = TrainingPipeline {
            corpus: NoopCorpusIngest,
            annotator: CvatAnnotator::new("https://cvat.example.com", "fake-api-key"),
            backbone: InternVideo2Backbone::with_defaults(PathBuf::from(
                "/nonexistent/internvideo2.safetensors",
            )),
            trainer: NoopLoraTrainer,
            eval_gate: NoopEvalGate,
        };

        let tenant = TenantId::new("cust_test");
        let source = CorpusSource::S3Prefix("s3://tenant-bucket/corpus/".to_string());
        let lora = LoraConfig::default();
        let eval_set = PathBuf::from("/nonexistent/held-out");

        let err = pipeline
            .train(&tenant, &source, &lora, &eval_set)
            .await
            .expect_err("stub corpus must short-circuit the pipeline");

        let msg = err.to_string();
        assert!(
            msg.contains("noop-corpus") && msg.contains("not implemented"),
            "expected the noop-corpus NotImplemented error to bubble up, got: {msg}",
        );
    }
}
