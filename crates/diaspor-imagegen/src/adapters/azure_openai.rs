//! Azure `OpenAI` image generation (`gpt-image-1` / DALL·E 3).
//!
//! Reads `AZURE_OPENAI_ENDPOINT`, `AZURE_OPENAI_KEY`, and
//! `AZURE_OPENAI_IMAGE_DEPLOYMENT` (or takes them explicitly), plus an optional
//! `AZURE_OPENAI_API_VERSION`.

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::domain::{GenerateRequest, Image, ImageError, ImageFormat, ProviderProfile, Result};
use crate::ports::ImageGenerator;

const PROVIDER: &str = "azure-openai-image";

/// Adapter for Azure `OpenAI` image generations.
pub struct AzureOpenAiImageAdapter {
    http: reqwest::Client,
    endpoint: String,
    api_key: String,
    deployment: String,
    api_version: String,
}

impl AzureOpenAiImageAdapter {
    /// Build from explicit values or the `AZURE_OPENAI_*` env vars.
    ///
    /// # Errors
    /// Returns [`ImageError::NotConfigured`] when the endpoint or key is
    /// missing.
    pub fn new(
        endpoint: Option<String>,
        api_key: Option<String>,
        deployment: Option<String>,
    ) -> Result<Self> {
        let from_env = |val: Option<String>, var: &str| {
            val.or_else(|| std::env::var(var).ok())
                .filter(|v| !v.is_empty())
        };
        let endpoint = from_env(endpoint, "AZURE_OPENAI_ENDPOINT").ok_or_else(|| {
            ImageError::NotConfigured {
                provider: PROVIDER.into(),
                reason: "set AZURE_OPENAI_ENDPOINT".into(),
            }
        })?;
        let api_key =
            from_env(api_key, "AZURE_OPENAI_KEY").ok_or_else(|| ImageError::NotConfigured {
                provider: PROVIDER.into(),
                reason: "set AZURE_OPENAI_KEY".into(),
            })?;
        let deployment = from_env(deployment, "AZURE_OPENAI_IMAGE_DEPLOYMENT")
            .unwrap_or_else(|| "gpt-image-1".to_string());
        Ok(Self {
            http: reqwest::Client::new(),
            endpoint: endpoint.trim_end_matches('/').to_string(),
            api_key,
            deployment,
            api_version: std::env::var("AZURE_OPENAI_API_VERSION")
                .unwrap_or_else(|_| "2025-04-01-preview".to_string()),
        })
    }
}

/// Map an arbitrary canvas to the nearest size `gpt-image-1` supports.
const fn nearest_size(width: u32, height: u32) -> &'static str {
    if width > height {
        "1536x1024"
    } else if height > width {
        "1024x1536"
    } else {
        "1024x1024"
    }
}

fn provider_err(e: &reqwest::Error) -> ImageError {
    ImageError::Provider {
        provider: PROVIDER.into(),
        message: e.to_string(),
    }
}

#[derive(Serialize)]
struct ImageRequest<'a> {
    prompt: &'a str,
    n: u32,
    size: &'a str,
    output_format: &'a str,
}

#[derive(Deserialize)]
struct ImageResponse {
    #[serde(default)]
    data: Vec<ImageDatum>,
}

#[derive(Deserialize)]
struct ImageDatum {
    #[serde(default)]
    b64_json: Option<String>,
}

#[async_trait]
impl ImageGenerator for AzureOpenAiImageAdapter {
    fn profile(&self) -> ProviderProfile {
        ProviderProfile {
            name: PROVIDER.into(),
            quality: 88,
            cost_usd_per_image: 0.040,
            supports_compose: false,
            offline: false,
        }
    }

    async fn generate(&self, request: &GenerateRequest) -> Result<Image> {
        request.validate()?;
        let url = format!(
            "{}/openai/deployments/{}/images/generations?api-version={}",
            self.endpoint, self.deployment, self.api_version
        );
        let body = ImageRequest {
            prompt: &request.prompt,
            n: 1,
            size: nearest_size(request.width, request.height),
            output_format: "png",
        };
        let response = self
            .http
            .post(&url)
            .header("api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| provider_err(&e))?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ImageError::Provider {
                provider: PROVIDER.into(),
                message: format!("{status}: {text}"),
            });
        }
        let parsed: ImageResponse = response.json().await.map_err(|e| provider_err(&e))?;
        let b64 = parsed
            .data
            .into_iter()
            .find_map(|d| d.b64_json)
            .ok_or_else(|| ImageError::Provider {
                provider: PROVIDER.into(),
                message: "response contained no image".into(),
            })?;
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
}

#[cfg(test)]
mod tests {
    use super::{ImageResponse, nearest_size};

    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    #[test]
    fn parses_b64_image_response() {
        let json = format!(r#"{{"data":[{{"b64_json":"{PNG_1X1}"}}]}}"#);
        let resp: ImageResponse = serde_json::from_str(&json).unwrap();
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
