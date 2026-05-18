//! `custom-tier-train` — public API surface contract for milestone **M9**.
//!
//! At `v0.1.0-alpha.1` this example **does not actually train a `LoRA` adapter**. Its job
//! is to demonstrate that the [`TrainingPipeline`] orchestration —
//! corpus → annotate → embed → train → eval → sign — composes through the trait surface
//! today, so callers can build the per-tenant custom tier against the stable signatures
//! before M9 lands the real `InternVideo2` + PEFT + Ed25519 wiring.
//!
//! What the example demonstrates end-to-end:
//!
//! 1. Building a [`TrainingPipeline`] from the five [`Noop*`] stubs (one per stage).
//! 2. Defining a [`CorpusSource::S3Prefix`] reference into a placeholder tenant bucket.
//! 3. Driving the pipeline with [`default_judge_lora_config`] — the sport-judging
//!    preset (lower rank, fewer epochs) rather than the credibility preset (the
//!    credibility preset carries a refusal list — see the `diaspor-train` crate docs).
//! 4. Printing the **canonical** S3 key the trained artifact would land at —
//!    [`AdapterArtifact::path_in_tenant_bucket`] — so the contract surface is visible
//!    even when training fails at the corpus stage.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example custom-tier-train
//! ```
//!
//! See ROADMAP.md milestone M9 for the implementation tracking work.

use std::path::PathBuf;
use std::process::ExitCode;

use diaspor_train::{
    AdapterArtifact, AdapterId, CorpusSource, CvatAnnotator, InternVideo2Backbone,
    NoopCorpusIngest, NoopEvalGate, NoopLoraTrainer, TenantId, TrainingPipeline,
    default_judge_lora_config,
};

#[tokio::main]
async fn main() -> ExitCode {
    println!("======================================================================");
    println!("  diaspor custom-tier-train (v0.1.0-alpha.1, M9 surface only)");
    println!("======================================================================");
    println!();

    // ------------------------------------------------------------------------
    // Step 1 — pin the (tenant, adapter) identity. The control plane mints these
    // in production; the alpha just hard-codes the example values from the
    // task spec so the printed canonical path is reproducible.
    // ------------------------------------------------------------------------
    let tenant = TenantId::new("acme");
    let adapter_id = AdapterId::new("v1");
    let canonical_path = AdapterArtifact::path_in_tenant_bucket(&tenant, &adapter_id);

    println!("Tenant + adapter identity:");
    println!("  tenant_id  = {tenant}");
    println!("  adapter_id = {adapter_id}");
    println!();
    println!("Canonical S3 key the trained adapter will live at:");
    println!("  {canonical_path}");
    println!();
    println!("(This is the AdapterArtifact::path_in_tenant_bucket() contract — the serving");
    println!("layer resolves a (TenantId, AdapterId) pair into this exact shape with no");
    println!("extra database round-trip.)");
    println!();

    // ------------------------------------------------------------------------
    // Step 2 — compose the TrainingPipeline from the five Noop stubs.
    //
    // Real M9 wiring replaces each stage; the orchestration signature is fixed.
    // ------------------------------------------------------------------------
    let pipeline = TrainingPipeline {
        corpus: NoopCorpusIngest,
        annotator: CvatAnnotator::new("https://cvat.example.com", "demo-cvat-api-key"),
        backbone: InternVideo2Backbone::with_defaults(PathBuf::from(
            "/nonexistent/internvideo2-1b.safetensors",
        )),
        trainer: NoopLoraTrainer,
        eval_gate: NoopEvalGate,
    };

    println!("Composed TrainingPipeline:");
    println!("  corpus    = noop-corpus               (production: S3 prefix walker)");
    println!("  annotator = cvat                      (production: CVAT REST paging)");
    println!("  backbone  = internvideo2-1b           (production: InternVideo2-Stage2)");
    println!("  trainer   = noop-lora                 (production: PEFT + safetensors)");
    println!("  eval_gate = noop-eval                 (production: held-out AUC/MSE)");
    println!();

    // ------------------------------------------------------------------------
    // Step 3 — drive the pipeline. v0.1.0-alpha SHORT-CIRCUITS at the corpus stage.
    // ------------------------------------------------------------------------
    let corpus_source =
        CorpusSource::S3Prefix("s3://acme-diaspor-corpus/judge-v1/clips/".to_string());
    let lora_config = default_judge_lora_config();
    let held_out_set = PathBuf::from("/nonexistent/acme-judge-v1-heldout");

    println!("Calling TrainingPipeline::train(...) ...");
    println!("  source     = {corpus_source:?}");
    println!(
        "  lora       = rank={} alpha={} epochs={} lr={}",
        lora_config.rank, lora_config.alpha, lora_config.epochs, lora_config.learning_rate
    );
    println!("  eval_set   = {}", held_out_set.display());
    println!();

    match pipeline
        .train(&tenant, &corpus_source, &lora_config, &held_out_set)
        .await
    {
        Ok(_artifact) => {
            // Unreachable at v0.1.0-alpha.1 — kept as a forward-looking template
            // for the M9 happy path (artifact lands at `canonical_path` post-sign).
            println!();
            println!("UNEXPECTED Ok at the alpha — artifact ready for vendor + tenant signing.");
            println!("Expected handoff path: {canonical_path}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            println!();
            println!("Pipeline failed (this is the EXPECTED v0.1.0-alpha.1 behavior).");
            println!();
            println!("  stage   = corpus (first stage in the chain)");
            println!("  backend = noop-corpus");
            println!("  error   = {err}");
            println!();
            println!("This is correct: the trait surface orchestrates the five stages but");
            println!("the backends are stubs. Real InternVideo2 + PEFT + Ed25519 wiring");
            println!("lands in milestone M9 — see ROADMAP.md for the tracking work.");
            println!();
            println!("Expected M9 handoff path (printed regardless of training failure to");
            println!("demonstrate the contract surface):");
            println!("  {canonical_path}");
            ExitCode::FAILURE
        }
    }
}
