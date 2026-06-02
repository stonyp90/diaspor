//! # diaspor-imagegen
//!
//! Provider-agnostic **image generation and composition** for diaspor, built as
//! ports & adapters so the domain stays independent of any single vendor.
//!
//! Two ports — [`ImageGenerator`] (text → image) and [`ImageCompositor`]
//! (many images → one) — are implemented by interchangeable adapters:
//!
//! * [`adapters::LocalImageAdapter`] — offline, zero-cost, deterministic. The
//!   air-gapped tier and the test double; needs no network or API key.
//! * `adapters::GeminiImageAdapter` — Google *Nano Banana* (Gemini 2.5 Flash
//!   Image); generates and composites. Requires the `remote` feature.
//! * `adapters::OpenAiImageAdapter` — `OpenAI` `gpt-image-1` via `api.openai.com`;
//!   generates and composites. Requires the `remote` feature.
//! * `adapters::AzureOpenAiImageAdapter` — Azure `OpenAI` images (`gpt-image-1`).
//!   Requires the `remote` feature.
//!
//! [`ImageStudio`] is the cost/quality router: register the adapters you have,
//! hand it a [`Policy`], and it picks the cheapest / best / best-under-budget
//! provider, then (optionally) composites the layers into one final image.
//!
//! ```
//! # #[tokio::main]
//! # async fn main() -> diaspor_imagegen::Result<()> {
//! use diaspor_imagegen::{ImageStudio, Policy, GenerateRequest};
//! use diaspor_imagegen::adapters::LocalImageAdapter;
//! use std::sync::Arc;
//!
//! let local = Arc::new(LocalImageAdapter::new());
//! let studio = ImageStudio::builder()
//!     .generator(local.clone())
//!     .compositor(local)
//!     .build()?;
//!
//! let img = studio
//!     .generate(&GenerateRequest::new("a calm lake at dawn", 512, 512), &Policy::CostOptimized)
//!     .await?;
//! assert_eq!((img.width, img.height), (512, 512));
//! # Ok(()) }
//! ```

#![forbid(unsafe_code)]

pub mod adapters;
mod domain;
mod ports;
mod studio;

pub use domain::{
    CompositeRequest, GenerateRequest, Image, ImageError, ImageFormat, Layer, Policy,
    ProviderProfile, Result,
};
pub use ports::{ImageCompositor, ImageGenerator};
pub use studio::{ImageStudio, ImageStudioBuilder};
