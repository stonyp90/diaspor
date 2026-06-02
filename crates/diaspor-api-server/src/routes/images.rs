//! `POST /v1/images/generate` — text-to-image via the cost/quality-routed
//! image studio.
//!
//! Builds a studio from whatever provider keys are present in the server
//! environment (`OPENAI_API_KEY` / `GEMINI_API_KEY`), always with the offline
//! local adapter as a fallback, and returns a base64 PNG. Authenticated like
//! every non-health route.

use std::sync::Arc;

use axum::Json;
use axum::response::IntoResponse;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use diaspor_imagegen::adapters::{GeminiImageAdapter, LocalImageAdapter, OpenAiImageAdapter};
use diaspor_imagegen::{GenerateRequest, ImageCompositor, ImageStudio, Policy};
use serde::{Deserialize, Serialize};

use crate::auth::ApiKey;
use crate::error::ApiError;

/// Request body for `POST /v1/images/generate`.
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateImageRequest {
    /// What to draw.
    pub prompt: String,
    /// Canvas width in pixels (default `1024`).
    #[serde(default = "default_dim")]
    pub width: u32,
    /// Canvas height in pixels (default `1024`).
    #[serde(default = "default_dim")]
    pub height: u32,
    /// Routing policy: `cost`, `quality` (default), or `balanced:<usd>`.
    #[serde(default = "default_policy")]
    pub policy: String,
}

const fn default_dim() -> u32 {
    1024
}

fn default_policy() -> String {
    "quality".to_string()
}

/// Response body — a base64-encoded image.
#[derive(Debug, Clone, Serialize)]
pub struct GenerateImageResponse {
    /// Media type of the image, e.g. `image/png`.
    pub format: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Base64-encoded image bytes.
    pub b64_data: String,
}

/// `POST /v1/images/generate` — generate an image from a text prompt.
///
/// Authenticates the caller, routes the request through the configured
/// providers (falling back across them on failure), and returns a base64 PNG.
/// Maps a bad `policy` to `400` and any provider failure to `500`.
pub async fn generate(
    _key: ApiKey,
    Json(req): Json<GenerateImageRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let policy = parse_policy(&req.policy).map_err(ApiError::BadRequest)?;
    let studio = build_studio().map_err(|e| ApiError::Internal(format!("studio: {e}")))?;
    let image = studio
        .generate(
            &GenerateRequest::new(req.prompt, req.width, req.height),
            &policy,
        )
        .await
        .map_err(|e| ApiError::Internal(format!("image generation failed: {e}")))?;
    Ok(Json(GenerateImageResponse {
        format: image.format.mime().to_string(),
        width: image.width,
        height: image.height,
        b64_data: STANDARD.encode(&image.bytes),
    }))
}

/// Build a studio from whatever provider keys are set; the offline local
/// adapter is always present so the endpoint never fails for lack of a key.
fn build_studio() -> diaspor_imagegen::Result<ImageStudio> {
    let local = Arc::new(LocalImageAdapter::new());
    let openai = OpenAiImageAdapter::new(None).ok().map(Arc::new);

    let compositor: Arc<dyn ImageCompositor> = match &openai {
        Some(openai) => openai.clone(),
        None => local.clone(),
    };

    let mut builder = ImageStudio::builder()
        .generator(local)
        .compositor(compositor);
    if let Some(openai) = openai {
        builder = builder.generator(openai);
    }
    if let Ok(gemini) = GeminiImageAdapter::new(None) {
        builder = builder.generator(Arc::new(gemini));
    }
    builder.build()
}

fn parse_policy(spec: &str) -> Result<Policy, String> {
    if spec == "cost" {
        return Ok(Policy::CostOptimized);
    }
    if spec == "quality" {
        return Ok(Policy::QualityFirst);
    }
    if let Some(budget) = spec.strip_prefix("balanced:") {
        let max_cost_usd = budget
            .parse()
            .map_err(|_| format!("invalid balanced budget: {budget}"))?;
        return Ok(Policy::Balanced { max_cost_usd });
    }
    Err(format!(
        "unknown policy '{spec}' (use cost|quality|balanced:<usd>)"
    ))
}
