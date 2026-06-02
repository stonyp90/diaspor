//! `OpenAI` image generation + composition (`gpt-image-1`) over `api.openai.com`.
//!
//! Reads `OPENAI_API_KEY` (or takes it explicitly); the base URL and model can
//! be overridden via `OPENAI_BASE_URL` / `OPENAI_IMAGE_MODEL`. Generation uses
//! `/images/generations`; composition uses the multipart `/images/edits`
//! endpoint, which `gpt-image-1` accepts multiple input images for.

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::domain::{
    CompositeRequest, GenerateRequest, Image, ImageError, ImageFormat, ProviderProfile, Result,
};
use crate::ports::{ImageCompositor, ImageGenerator};

const PROVIDER: &str = "openai-gpt-image";
const DEFAULT_MODEL: &str = "gpt-image-1";
const DEFAULT_BASE: &str = "https://api.openai.com/v1";

/// Adapter for `OpenAI`'s `gpt-image-1`: generates from text and composites
/// multiple images via the edits endpoint.
pub struct OpenAiImageAdapter {
    http: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiImageAdapter {
    /// Build from an explicit key, or fall back to `OPENAI_API_KEY`.
    ///
    /// # Errors
    /// Returns [`ImageError::NotConfigured`] when no key is available.
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let api_key = api_key
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .filter(|k| !k.is_empty())
            .ok_or_else(|| ImageError::NotConfigured {
                provider: PROVIDER.into(),
                reason: "set OPENAI_API_KEY or pass a key".into(),
            })?;
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            model: std::env::var("OPENAI_IMAGE_MODEL")
                .unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            base_url: std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string()),
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{path}", self.base_url.trim_end_matches('/'))
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.api_key)
    }
}

fn provider_err(e: &reqwest::Error) -> ImageError {
    ImageError::Provider {
        provider: PROVIDER.into(),
        message: e.to_string(),
    }
}

fn openai_profile() -> ProviderProfile {
    ProviderProfile {
        name: PROVIDER.into(),
        quality: 89,
        cost_usd_per_image: 0.04,
        supports_compose: true,
        offline: false,
    }
}

/// Map a canvas to the nearest size `gpt-image-1` supports.
const fn nearest_size(width: u32, height: u32) -> &'static str {
    if width > height {
        "1536x1024"
    } else if height > width {
        "1024x1536"
    } else {
        "1024x1024"
    }
}

fn decode_b64_png(b64: &str) -> Result<Image> {
    let bytes = STANDARD
        .decode(b64.as_bytes())
        .map_err(|e| ImageError::Codec(e.to_string()))?;
    let (width, height) =
        image::load_from_memory(&bytes).map_or((0, 0), |i| (i.width(), i.height()));
    Ok(Image {
        bytes,
        format: ImageFormat::Png,
        width,
        height,
    })
}

async fn read_images_response(response: reqwest::Response) -> Result<ImagesResponse> {
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(ImageError::Provider {
            provider: PROVIDER.into(),
            message: format!("{status}: {text}"),
        });
    }
    response
        .json::<ImagesResponse>()
        .await
        .map_err(|e| provider_err(&e))
}

fn first_image(parsed: ImagesResponse) -> Result<Image> {
    let b64 = parsed
        .data
        .into_iter()
        .find_map(|d| d.b64_json)
        .ok_or_else(|| ImageError::Provider {
            provider: PROVIDER.into(),
            message: "response contained no image".into(),
        })?;
    decode_b64_png(&b64)
}

#[derive(Serialize)]
struct GenerationRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    n: u32,
    size: &'a str,
    // Cheapest tier — this crate routes on cost. Callers wanting higher
    // fidelity pick a different provider/policy.
    quality: &'a str,
}

#[derive(Deserialize)]
struct ImagesResponse {
    #[serde(default)]
    data: Vec<ImageDatum>,
}

#[derive(Deserialize)]
struct ImageDatum {
    #[serde(default)]
    b64_json: Option<String>,
}

#[async_trait]
impl ImageGenerator for OpenAiImageAdapter {
    fn profile(&self) -> ProviderProfile {
        openai_profile()
    }

    async fn generate(&self, request: &GenerateRequest) -> Result<Image> {
        request.validate()?;
        let prompt = request.negative_prompt.as_ref().map_or_else(
            || request.prompt.clone(),
            |negative| format!("{}. Avoid: {negative}.", request.prompt),
        );
        let body = GenerationRequest {
            model: &self.model,
            prompt: &prompt,
            n: 1,
            size: nearest_size(request.width, request.height),
            quality: "low",
        };
        let response = self
            .http
            .post(self.endpoint("images/generations"))
            .header("Authorization", self.bearer())
            .json(&body)
            .send()
            .await
            .map_err(|e| provider_err(&e))?;
        first_image(read_images_response(response).await?)
    }
}

#[async_trait]
impl ImageCompositor for OpenAiImageAdapter {
    fn profile(&self) -> ProviderProfile {
        openai_profile()
    }

    async fn composite(&self, request: &CompositeRequest) -> Result<Image> {
        request.validate()?;
        let mut form = reqwest::multipart::Form::new()
            .text("model", self.model.clone())
            .text("prompt", request.instruction.clone())
            .text("n", "1")
            .text(
                "size",
                nearest_size(request.width, request.height).to_string(),
            )
            .text("quality", "low");
        for (i, layer) in request.layers.iter().enumerate() {
            let part = reqwest::multipart::Part::bytes(layer.image.bytes.clone())
                .file_name(format!("layer{i}.png"))
                .mime_str(layer.image.format.mime())
                .map_err(|e| ImageError::Provider {
                    provider: PROVIDER.into(),
                    message: e.to_string(),
                })?;
            form = form.part("image[]", part);
        }
        let response = self
            .http
            .post(self.endpoint("images/edits"))
            .header("Authorization", self.bearer())
            .multipart(form)
            .send()
            .await
            .map_err(|e| provider_err(&e))?;
        first_image(read_images_response(response).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::{ImagesResponse, nearest_size};

    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    #[test]
    fn parses_b64_image_response() {
        let json = format!(r#"{{"data":[{{"b64_json":"{PNG_1X1}"}}]}}"#);
        let resp: ImagesResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert!(resp.data[0].b64_json.is_some());
    }

    #[test]
    fn maps_canvas_to_supported_size() {
        assert_eq!(nearest_size(1920, 1080), "1536x1024");
        assert_eq!(nearest_size(512, 512), "1024x1024");
        assert_eq!(nearest_size(768, 1024), "1024x1536");
    }
}
