//! Domain types for image generation and composition.
//!
//! Framework-agnostic value objects shared by every adapter and by the
//! [`crate::ImageStudio`] router. No I/O, no vendor SDKs.

use serde::{Deserialize, Serialize};

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, ImageError>;

/// Errors raised by generation, composition, or routing.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ImageError {
    /// No registered provider satisfies the requested [`Policy`].
    #[error("no image provider satisfies policy: {0}")]
    NoProvider(String),

    /// A provider is registered but not usable (missing key/endpoint, etc.).
    #[error("provider `{provider}` is not configured: {reason}")]
    NotConfigured {
        /// Provider name.
        provider: String,
        /// Why it is unusable.
        reason: String,
    },

    /// The upstream provider returned an error.
    #[error("provider `{provider}` failed: {message}")]
    Provider {
        /// Provider name.
        provider: String,
        /// Upstream message.
        message: String,
    },

    /// Encoding or decoding raster bytes failed.
    #[error("image codec error: {0}")]
    Codec(String),

    /// The request was malformed (e.g. zero-sized canvas, no layers).
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

/// Encoded raster formats this crate can emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    /// PNG (lossless, alpha). The default and the only format the offline
    /// adapter emits.
    Png,
    /// JPEG (lossy, no alpha).
    Jpeg,
    /// WebP.
    Webp,
}

impl ImageFormat {
    /// The IANA media type, e.g. `image/png`.
    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
        }
    }

    /// Best-effort mapping from a media type to a format (defaults to PNG).
    #[must_use]
    pub fn from_mime(mime: &str) -> Self {
        match mime.split(';').next().unwrap_or(mime).trim() {
            "image/jpeg" | "image/jpg" => Self::Jpeg,
            "image/webp" => Self::Webp,
            _ => Self::Png,
        }
    }
}

/// An in-memory raster image: encoded bytes plus decoded dimensions.
#[derive(Clone, Serialize, Deserialize)]
pub struct Image {
    /// Encoded bytes (in `format`).
    pub bytes: Vec<u8>,
    /// Encoding of `bytes`.
    pub format: ImageFormat,
    /// Width in pixels (`0` if unknown).
    pub width: u32,
    /// Height in pixels (`0` if unknown).
    pub height: u32,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("format", &self.format)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .finish()
    }
}

/// A text-to-image request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// What to draw.
    pub prompt: String,
    /// Target width in pixels.
    pub width: u32,
    /// Target height in pixels.
    pub height: u32,
    /// Optional things to avoid.
    pub negative_prompt: Option<String>,
    /// Optional seed for reproducibility (honoured by adapters that support it;
    /// always honoured by the offline adapter).
    pub seed: Option<u64>,
}

impl GenerateRequest {
    /// A request with no negative prompt or seed.
    #[must_use]
    pub fn new(prompt: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            prompt: prompt.into(),
            width,
            height,
            negative_prompt: None,
            seed: None,
        }
    }

    /// Set a reproducibility seed.
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set a negative prompt.
    #[must_use]
    pub fn with_negative(mut self, negative: impl Into<String>) -> Self {
        self.negative_prompt = Some(negative.into());
        self
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.prompt.trim().is_empty() {
            return Err(ImageError::InvalidRequest("empty prompt".into()));
        }
        if self.width == 0 || self.height == 0 {
            return Err(ImageError::InvalidRequest("zero-sized canvas".into()));
        }
        Ok(())
    }
}

/// One image placed onto the composition canvas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    /// The image to place.
    pub image: Image,
    /// Opacity in `0.0..=1.0` (the offline compositor multiplies alpha by this).
    pub opacity: f32,
    /// Top-left placement offset on the canvas, in pixels.
    pub offset: (i64, i64),
}

impl Layer {
    /// A fully-opaque layer at the canvas origin.
    #[must_use]
    pub const fn new(image: Image) -> Self {
        Self {
            image,
            opacity: 1.0,
            offset: (0, 0),
        }
    }

    /// Place this layer at `(x, y)`.
    #[must_use]
    pub const fn at(mut self, x: i64, y: i64) -> Self {
        self.offset = (x, y);
        self
    }

    /// Set opacity (clamped to `0.0..=1.0`).
    #[must_use]
    pub const fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

/// A request to merge several [`Layer`]s into one image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeRequest {
    /// Natural-language instruction for model-based compositors (e.g. Nano
    /// Banana). The offline compositor ignores it and stacks layers by offset.
    pub instruction: String,
    /// Layers, painted bottom-first (index 0 is the background).
    pub layers: Vec<Layer>,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
}

impl CompositeRequest {
    /// Stack `layers` onto a `width`×`height` canvas with `instruction`.
    #[must_use]
    pub fn new(
        instruction: impl Into<String>,
        layers: Vec<Layer>,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            instruction: instruction.into(),
            layers,
            width,
            height,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.layers.is_empty() {
            return Err(ImageError::InvalidRequest("no layers to composite".into()));
        }
        if self.width == 0 || self.height == 0 {
            return Err(ImageError::InvalidRequest("zero-sized canvas".into()));
        }
        Ok(())
    }
}

/// How a provider advertises itself so the router can choose between providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    /// Stable provider name, e.g. `"gemini-nano-banana"`.
    pub name: String,
    /// Subjective quality tier in `0..=100` (higher is better).
    pub quality: u8,
    /// Approximate cost per generated image, in USD (`0.0` for offline).
    pub cost_usd_per_image: f64,
    /// Whether this provider can composite multiple images.
    pub supports_compose: bool,
    /// Whether this provider runs with no network / no API key.
    pub offline: bool,
}

/// Cost/quality routing policy used by [`crate::ImageStudio`].
#[derive(Debug, Clone, PartialEq)]
pub enum Policy {
    /// Cheapest provider (ties broken by higher quality).
    CostOptimized,
    /// Highest quality regardless of cost (ties broken by lower cost).
    QualityFirst,
    /// Best quality whose cost is `<= max_cost_usd` (ties broken by lower cost);
    /// errors if nothing fits the budget.
    Balanced {
        /// Per-image USD ceiling.
        max_cost_usd: f64,
    },
}

impl std::fmt::Display for Policy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CostOptimized => write!(f, "cost-optimized"),
            Self::QualityFirst => write!(f, "quality-first"),
            Self::Balanced { max_cost_usd } => write!(f, "balanced(<= ${max_cost_usd:.4})"),
        }
    }
}
