//! Ollama API Client for AI Features
//!
//! Provides a wrapper around Ollama API for:
//! - Image/video analysis using vision models (LLaVA)
//! - Audio transcription using Whisper models
//!
//! Supports models like llava, llava:13b, whisper, and other multimodal models.

use anyhow::{Context, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Ollama API client configuration
#[derive(Debug, Clone)]
pub struct OllamaClient {
    base_url: String,
    timeout: Duration,
}

/// Ollama model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub model: String,
    pub size: u64,
    pub digest: String,
    pub modified_at: String,
}

/// Ollama models list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelsResponse {
    pub models: Vec<OllamaModel>,
}

/// Transcription request to Ollama
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionRequest {
    pub model: String,
    pub prompt: String,
    pub stream: bool,
    pub options: Option<TranscriptionOptions>,
}

/// Transcription options
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Transcription response from Ollama
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    pub model: String,
    pub created_at: String,
    pub response: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration: Option<u64>,
}

/// Streaming transcription chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionChunk {
    pub model: String,
    pub created_at: String,
    pub response: String,
    pub done: bool,
}

/// Vision analysis request to Ollama (for LLaVA and similar models)
#[derive(Debug, Clone, Serialize)]
pub struct VisionRequest {
    pub model: String,
    pub prompt: String,
    pub images: Vec<String>, // Base64 encoded images
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<VisionOptions>,
}

/// Vision analysis options
#[derive(Debug, Clone, Serialize)]
pub struct VisionOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<i32>,
}

/// Vision analysis response from Ollama
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionResponse {
    pub model: String,
    pub created_at: String,
    pub response: String,
    pub done: bool,
}

/// Extracted tags from image analysis
#[derive(Debug, Clone)]
pub struct ImageAnalysisResult {
    pub tags: Vec<String>,
    pub description: String,
    pub objects: Vec<String>,
    pub scene: Option<String>,
}

impl OllamaClient {
    /// Create a new Ollama client
    pub fn new(base_url: Option<String>) -> Self {
        let base_url = base_url
            .or_else(|| std::env::var("OLLAMA_URL").ok())
            .unwrap_or_else(|| "http://localhost:11434".to_string());

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            timeout: Duration::from_secs(300), // 5 minutes default timeout
        }
    }

    /// Check if Ollama is available
    pub async fn is_available(&self) -> bool {
        match self.list_models().await {
            Ok(_) => true,
            Err(e) => {
                debug!("Ollama not available: {}", e);
                false
            }
        }
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<OllamaModel>> {
        let url = format!("{}/api/tags", self.base_url);
        debug!("Fetching models from: {}", url);

        let client = reqwest::Client::new();
        let response = timeout(self.timeout, client.get(&url).send())
            .await
            .context("Request timeout")?
            .context("Failed to connect to Ollama")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Ollama API error: {}",
                response.status()
            ));
        }

        let models_response: OllamaModelsResponse = response
            .json()
            .await
            .context("Failed to parse models response")?;

        Ok(models_response.models)
    }

    /// Check if a model supports transcription (whisper-based models)
    pub fn is_transcription_model(model_name: &str) -> bool {
        let name_lower = model_name.to_lowercase();
        name_lower.contains("whisper")
            || name_lower.contains("transcribe")
            || name_lower.contains("audio")
    }

    /// Get available transcription models
    pub async fn get_transcription_models(&self) -> Result<Vec<OllamaModel>> {
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| Self::is_transcription_model(&m.name))
            .collect())
    }

    /// Transcribe audio file using Ollama
    /// The audio file should be base64 encoded or provided as a file path
    pub async fn transcribe_audio(
        &self,
        model: &str,
        audio_data: &[u8],
        language: Option<String>,
    ) -> Result<String> {
        // Convert audio to base64
        use base64::{Engine as _, engine::general_purpose};
        let audio_base64 = general_purpose::STANDARD.encode(audio_data);
        
        // For whisper models, we need to send the audio as a prompt with base64 data
        // Ollama's whisper models accept audio data in the prompt
        let prompt = format!("data:audio/wav;base64,{}", audio_base64);
        
        let request = TranscriptionRequest {
            model: model.to_string(),
            prompt,
            stream: false,
            options: Some(TranscriptionOptions {
                temperature: Some(0.0), // Lower temperature for more accurate transcription
                language,
            }),
        };

        let url = format!("{}/api/generate", self.base_url);
        debug!("Transcribing audio with model: {} ({} bytes)", model, audio_data.len());

        let client = reqwest::Client::new();
        let response = timeout(
            self.timeout,
            client.post(&url).json(&request).send(),
        )
        .await
        .context("Transcription request timeout")?
        .context("Failed to send transcription request")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Ollama transcription error: {} - {}",
                status,
                error_text
            ));
        }

        let transcription: TranscriptionResponse = response
            .json()
            .await
            .context("Failed to parse transcription response")?;

        Ok(transcription.response.trim().to_string())
    }

    /// Transcribe audio file from path
    pub async fn transcribe_audio_file(
        &self,
        model: &str,
        audio_path: &std::path::Path,
        language: Option<String>,
    ) -> Result<String> {
        let audio_data = tokio::fs::read(audio_path)
            .await
            .context("Failed to read audio file")?;
        
        self.transcribe_audio(model, &audio_data, language).await
    }

    /// Transcribe audio with streaming (for real-time transcription)
    pub async fn transcribe_audio_streaming(
        &self,
        model: &str,
        audio_data: &[u8],
        language: Option<String>,
        mut on_chunk: impl FnMut(&str) -> Result<()>,
    ) -> Result<String> {
        use base64::{Engine as _, engine::general_purpose};
        let audio_base64 = general_purpose::STANDARD.encode(audio_data);
        let prompt = format!("data:audio/wav;base64,{}", audio_base64);
        
        let request = TranscriptionRequest {
            model: model.to_string(),
            prompt,
            stream: true,
            options: Some(TranscriptionOptions {
                temperature: Some(0.0),
                language,
            }),
        };

        let url = format!("{}/api/generate", self.base_url);
        let client = reqwest::Client::new();
        let response = timeout(
            self.timeout,
            client.post(&url).json(&request).send(),
        )
        .await
        .context("Transcription request timeout")?
        .context("Failed to send transcription request")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Ollama transcription error: {} - {}",
                status,
                error_text
            ));
        }

        let mut full_text = String::new();
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.context("Failed to read stream chunk")?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);
            
            // Parse JSON chunks (Ollama streams JSON lines)
            // Handle incomplete lines by keeping the last incomplete line in buffer
            let lines: Vec<&str> = buffer.lines().collect();
            let last_line_incomplete = !buffer.ends_with('\n');
            
            // Process all complete lines
            let lines_to_process = if last_line_incomplete && !lines.is_empty() {
                &lines[..lines.len() - 1]
            } else {
                &lines[..]
            };
            
            for line in lines_to_process {
                if let Ok(chunk_data) = serde_json::from_str::<TranscriptionChunk>(line) {
                    full_text.push_str(&chunk_data.response);
                    on_chunk(&chunk_data.response)?;
                    
                    if chunk_data.done {
                        return Ok(full_text.trim().to_string());
                    }
                }
            }
            
            // Keep the last incomplete line in buffer
            if last_line_incomplete && !lines.is_empty() {
                buffer = lines.last().unwrap().to_string();
            } else {
                buffer.clear();
            }
        }
        
        // Process any remaining buffer content
        if !buffer.trim().is_empty() {
            if let Ok(chunk_data) = serde_json::from_str::<TranscriptionChunk>(&buffer) {
                full_text.push_str(&chunk_data.response);
                let _ = on_chunk(&chunk_data.response);
            }
        }

        Ok(full_text.trim().to_string())
    }

    // =========================================================================
    // Vision Analysis Methods (LLaVA)
    // =========================================================================

    /// Check if a model supports vision (LLaVA and similar)
    pub fn is_vision_model(model_name: &str) -> bool {
        let name_lower = model_name.to_lowercase();
        name_lower.contains("llava")
            || name_lower.contains("bakllava")
            || name_lower.contains("moondream")
            || name_lower.contains("cogvlm")
            || name_lower.contains("vision")
    }

    /// Get available vision models
    pub async fn get_vision_models(&self) -> Result<Vec<OllamaModel>> {
        let models = self.list_models().await?;
        Ok(models
            .into_iter()
            .filter(|m| Self::is_vision_model(&m.name))
            .collect())
    }

    /// Analyze an image using a vision model (LLaVA)
    /// Returns a structured analysis with tags, description, and detected objects
    pub async fn analyze_image(
        &self,
        model: &str,
        image_data: &[u8],
    ) -> Result<ImageAnalysisResult> {
        use base64::{Engine as _, engine::general_purpose};
        let image_base64 = general_purpose::STANDARD.encode(image_data);
        
        // Craft a prompt that will generate structured, parseable output
        let prompt = r#"Analyze this image and provide:
1. A brief description (1-2 sentences)
2. A list of relevant tags for organizing this file (single words or short phrases)
3. Main objects/subjects visible
4. The type of scene (e.g., outdoor, indoor, portrait, landscape, product, document, etc.)

Format your response exactly like this:
DESCRIPTION: [your description]
TAGS: [tag1], [tag2], [tag3], ...
OBJECTS: [object1], [object2], ...
SCENE: [scene type]"#;

        let request = VisionRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            images: vec![image_base64],
            stream: false,
            options: Some(VisionOptions {
                temperature: Some(0.3), // Lower temperature for more consistent output
                num_predict: Some(500), // Limit response length
            }),
        };

        let url = format!("{}/api/generate", self.base_url);
        debug!("Analyzing image with model: {} ({} bytes)", model, image_data.len());

        let client = reqwest::Client::new();
        let response = timeout(
            self.timeout,
            client.post(&url).json(&request).send(),
        )
        .await
        .context("Vision analysis request timeout")?
        .context("Failed to send vision analysis request")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Ollama vision error: {} - {}",
                status,
                error_text
            ));
        }

        let vision_response: VisionResponse = response
            .json()
            .await
            .context("Failed to parse vision response")?;

        // Parse the structured response
        let result = self.parse_vision_response(&vision_response.response);
        info!("Image analysis complete: {} tags extracted", result.tags.len());
        
        Ok(result)
    }

    /// Analyze an image file from path
    pub async fn analyze_image_file(
        &self,
        model: &str,
        image_path: &Path,
    ) -> Result<ImageAnalysisResult> {
        let image_data = tokio::fs::read(image_path)
            .await
            .context("Failed to read image file")?;
        
        self.analyze_image(model, &image_data).await
    }

    /// Generate tags for an image (simplified version of analyze_image)
    pub async fn generate_tags(
        &self,
        model: &str,
        image_data: &[u8],
    ) -> Result<Vec<String>> {
        use base64::{Engine as _, engine::general_purpose};
        let image_base64 = general_purpose::STANDARD.encode(image_data);
        
        let prompt = "List 5-10 relevant tags for organizing this image in a file manager. \
                      Return only the tags as a comma-separated list, nothing else. \
                      Use single words or short phrases. Examples: nature, sunset, beach, family, portrait";

        let request = VisionRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            images: vec![image_base64],
            stream: false,
            options: Some(VisionOptions {
                temperature: Some(0.3),
                num_predict: Some(200),
            }),
        };

        let url = format!("{}/api/generate", self.base_url);
        info!("Generating tags for image with model: {}", model);

        let client = reqwest::Client::new();
        let response = timeout(
            self.timeout,
            client.post(&url).json(&request).send(),
        )
        .await
        .context("Tag generation request timeout")?
        .context("Failed to send tag generation request")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Ollama tag generation error: {} - {}",
                status,
                error_text
            ));
        }

        let vision_response: VisionResponse = response
            .json()
            .await
            .context("Failed to parse tag response")?;

        // Parse comma-separated tags
        let tags: Vec<String> = vision_response
            .response
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty() && s.len() < 50) // Filter out empty or overly long "tags"
            .collect();

        info!("Generated {} tags for image", tags.len());
        Ok(tags)
    }

    /// Parse the structured vision response into ImageAnalysisResult
    fn parse_vision_response(&self, response: &str) -> ImageAnalysisResult {
        let mut description = String::new();
        let mut tags = Vec::new();
        let mut objects = Vec::new();
        let mut scene = None;

        for line in response.lines() {
            let line = line.trim();
            
            if let Some(desc) = line.strip_prefix("DESCRIPTION:") {
                description = desc.trim().to_string();
            } else if let Some(tags_str) = line.strip_prefix("TAGS:") {
                tags = tags_str
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty() && s.len() < 50)
                    .collect();
            } else if let Some(objects_str) = line.strip_prefix("OBJECTS:") {
                objects = objects_str
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
            } else if let Some(scene_str) = line.strip_prefix("SCENE:") {
                let s = scene_str.trim().to_lowercase();
                if !s.is_empty() {
                    scene = Some(s);
                }
            }
        }

        // If structured parsing failed, try to extract any useful content
        if tags.is_empty() && description.is_empty() {
            // Fallback: treat the entire response as potential tags/description
            warn!("Structured parsing failed, using fallback extraction");
            
            // Try to extract any comma-separated words as tags
            let words: Vec<String> = response
                .split([',', '\n'])
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty() && s.len() < 30 && s.split_whitespace().count() <= 3)
                .take(10)
                .collect();
            
            if !words.is_empty() {
                tags = words;
            }
            
            // Use first sentence as description
            if let Some(first_sentence) = response.split('.').next() {
                description = first_sentence.trim().to_string();
            }
        }

        // Add objects as tags if they're meaningful
        for obj in &objects {
            if !tags.contains(obj) && obj.len() < 30 {
                tags.push(obj.clone());
            }
        }

        // Add scene as a tag
        if let Some(ref s) = scene {
            if !tags.contains(s) {
                tags.push(s.clone());
            }
        }

        ImageAnalysisResult {
            tags,
            description,
            objects,
            scene,
        }
    }
}
