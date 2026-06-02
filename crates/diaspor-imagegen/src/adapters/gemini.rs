//! Google *Nano Banana* — Gemini 2.5 Flash Image — generator + compositor.
//!
//! Calls the Generative Language API `:generateContent` endpoint. The API key
//! comes from the constructor or the `GEMINI_API_KEY` env var; the model and
//! base URL can be overridden via `GEMINI_IMAGE_MODEL` / `GEMINI_BASE_URL`.

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::domain::{
    CompositeRequest, GenerateRequest, Image, ImageError, ImageFormat, ProviderProfile, Result,
};
use crate::ports::{ImageCompositor, ImageGenerator};

const PROVIDER: &str = "gemini-nano-banana";
const DEFAULT_MODEL: &str = "gemini-2.5-flash-image";
const DEFAULT_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Adapter for Gemini 2.5 Flash Image ("Nano Banana"). Generates from text and
/// composites multiple input images, both via `:generateContent`.
pub struct GeminiImageAdapter {
    http: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl GeminiImageAdapter {
    /// Build from an explicit key, or fall back to `GEMINI_API_KEY`.
    ///
    /// # Errors
    /// Returns [`ImageError::NotConfigured`] when no key is available.
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let api_key = api_key
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
            .filter(|k| !k.is_empty())
            .ok_or_else(|| ImageError::NotConfigured {
                provider: PROVIDER.into(),
                reason: "set GEMINI_API_KEY or pass a key".into(),
            })?;
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            model: std::env::var("GEMINI_IMAGE_MODEL")
                .unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            base_url: std::env::var("GEMINI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string()),
        })
    }

    async fn call(&self, parts: Vec<Part>) -> Result<Image> {
        let url = format!(
            "{}/models/{}:generateContent",
            self.base_url.trim_end_matches('/'),
            self.model
        );
        let body = GenerateContentRequest {
            contents: vec![Content {
                role: "user",
                parts,
            }],
            generation_config: GenerationConfig {
                response_modalities: vec!["TEXT", "IMAGE"],
            },
        };
        let response = self
            .http
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| reqwest_err(&e))?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ImageError::Provider {
                provider: PROVIDER.into(),
                message: format!("{status}: {text}"),
            });
        }
        let parsed: GenerateContentResponse = response.json().await.map_err(|e| reqwest_err(&e))?;
        decode_first_image(&parsed)
    }
}

fn reqwest_err(e: &reqwest::Error) -> ImageError {
    ImageError::Provider {
        provider: PROVIDER.into(),
        message: e.to_string(),
    }
}

fn gemini_profile() -> ProviderProfile {
    ProviderProfile {
        name: PROVIDER.into(),
        quality: 90,
        cost_usd_per_image: 0.039,
        supports_compose: true,
        offline: false,
    }
}

fn text_part(text: &str) -> Part {
    Part {
        text: Some(text.to_string()),
        inline_data: None,
    }
}

fn image_part(image: &Image) -> Part {
    Part {
        text: None,
        inline_data: Some(InlineData {
            mime_type: image.format.mime().to_string(),
            data: STANDARD.encode(&image.bytes),
        }),
    }
}

fn decode_first_image(response: &GenerateContentResponse) -> Result<Image> {
    for candidate in &response.candidates {
        for part in &candidate.content.parts {
            if let Some(inline) = &part.inline_data {
                let bytes = STANDARD
                    .decode(inline.data.as_bytes())
                    .map_err(|e| ImageError::Codec(e.to_string()))?;
                let (width, height) =
                    image::load_from_memory(&bytes).map_or((0, 0), |i| (i.width(), i.height()));
                return Ok(Image {
                    bytes,
                    format: ImageFormat::from_mime(&inline.mime_type),
                    width,
                    height,
                });
            }
        }
    }
    Err(ImageError::Provider {
        provider: PROVIDER.into(),
        message: "response contained no image".into(),
    })
}

#[async_trait]
impl ImageGenerator for GeminiImageAdapter {
    fn profile(&self) -> ProviderProfile {
        gemini_profile()
    }

    async fn generate(&self, request: &GenerateRequest) -> Result<Image> {
        request.validate()?;
        let mut prompt = format!(
            "Generate a {width}x{height} image. {prompt}",
            width = request.width,
            height = request.height,
            prompt = request.prompt
        );
        if let Some(negative) = &request.negative_prompt {
            prompt.push_str(" Avoid: ");
            prompt.push_str(negative);
            prompt.push('.');
        }
        self.call(vec![text_part(&prompt)]).await
    }
}

#[async_trait]
impl ImageCompositor for GeminiImageAdapter {
    fn profile(&self) -> ProviderProfile {
        gemini_profile()
    }

    async fn composite(&self, request: &CompositeRequest) -> Result<Image> {
        request.validate()?;
        let mut parts = Vec::with_capacity(request.layers.len() + 1);
        parts.push(text_part(&format!(
            "Composite these {n} images into a single {w}x{h} image. {instruction}",
            n = request.layers.len(),
            w = request.width,
            h = request.height,
            instruction = request.instruction
        )));
        for layer in &request.layers {
            parts.push(image_part(&layer.image));
        }
        self.call(parts).await
    }
}

// ---- wire shapes (camelCase per the Gemini REST API) ----

#[derive(Serialize)]
struct GenerateContentRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct GenerationConfig {
    // Image models must be told to emit an image part, not just text.
    #[serde(rename = "responseModalities")]
    response_modalities: Vec<&'static str>,
}

#[derive(Serialize)]
struct Content {
    role: &'static str,
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(rename = "inlineData", skip_serializing_if = "Option::is_none")]
    inline_data: Option<InlineData>,
}

#[derive(Serialize, Deserialize)]
struct InlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

#[derive(Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: RespContent,
}

#[derive(Deserialize)]
struct RespContent {
    #[serde(default)]
    parts: Vec<RespPart>,
}

#[derive(Deserialize)]
struct RespPart {
    #[serde(rename = "inlineData", default)]
    inline_data: Option<InlineData>,
}

#[cfg(test)]
mod tests {
    use super::{
        Content, GenerateContentRequest, GenerateContentResponse, GenerationConfig,
        decode_first_image, text_part,
    };
    use crate::domain::ImageFormat;

    // 1x1 transparent PNG.
    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    #[test]
    fn parses_inline_image_from_response() {
        let json = format!(
            r#"{{"candidates":[{{"content":{{"parts":[{{"inlineData":{{"mimeType":"image/png","data":"{PNG_1X1}"}}}}]}}}}]}}"#
        );
        let resp: GenerateContentResponse = serde_json::from_str(&json).unwrap();
        let img = decode_first_image(&resp).unwrap();
        assert_eq!(img.format, ImageFormat::Png);
        assert!(!img.bytes.is_empty());
    }

    #[test]
    fn errors_when_no_image_in_response() {
        let resp: GenerateContentResponse = serde_json::from_str(r#"{"candidates":[]}"#).unwrap();
        assert!(decode_first_image(&resp).is_err());
    }

    #[test]
    fn request_serializes_text_part_camelcase() {
        let body = GenerateContentRequest {
            contents: vec![Content {
                role: "user",
                parts: vec![text_part("hello")],
            }],
            generation_config: GenerationConfig {
                response_modalities: vec!["TEXT", "IMAGE"],
            },
        };
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(v["contents"][0]["parts"][0]["text"], "hello");
    }
}
