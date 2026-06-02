//! Offline, deterministic adapter — zero cost, no network, no API key.
//!
//! [`LocalImageAdapter`] is the air-gapped tier and the test double. Its
//! generator synthesises a reproducible diagonal gradient keyed on the prompt
//! (and seed), and its compositor stacks layers with real alpha blending via
//! the `image` crate. It is not a photoreal model; wire a local Stable
//! Diffusion / `ComfyUI` endpoint as a separate adapter for that.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)]

use std::hash::{Hash, Hasher};
use std::io::Cursor;

use async_trait::async_trait;
use image::{DynamicImage, ImageFormat as RasterFormat, Rgba, RgbaImage};

use crate::domain::{
    CompositeRequest, GenerateRequest, Image, ImageError, ImageFormat, ProviderProfile, Result,
};
use crate::ports::{ImageCompositor, ImageGenerator};

/// Offline deterministic generator + compositor. See module docs.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalImageAdapter;

impl LocalImageAdapter {
    /// Create the adapter (no configuration required).
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

fn local_profile() -> ProviderProfile {
    ProviderProfile {
        name: "local-offline".into(),
        quality: 25,
        cost_usd_per_image: 0.0,
        supports_compose: true,
        offline: true,
    }
}

fn encode_png(rgba: RgbaImage) -> Result<Image> {
    let (width, height) = (rgba.width(), rgba.height());
    let mut bytes = Vec::new();
    DynamicImage::ImageRgba8(rgba)
        .write_to(&mut Cursor::new(&mut bytes), RasterFormat::Png)
        .map_err(|e| ImageError::Codec(e.to_string()))?;
    Ok(Image {
        bytes,
        format: ImageFormat::Png,
        width,
        height,
    })
}

fn decode_rgba(image: &Image) -> Result<RgbaImage> {
    Ok(image::load_from_memory(&image.bytes)
        .map_err(|e| ImageError::Codec(e.to_string()))?
        .to_rgba8())
}

fn seed_of(request: &GenerateRequest) -> u64 {
    if let Some(seed) = request.seed {
        return seed;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    request.prompt.hash(&mut hasher);
    hasher.finish()
}

/// Linear interpolation between two channel values.
fn blend(a: u8, b: u8, t: f32) -> u8 {
    f32::from(a).mul_add(1.0 - t, f32::from(b) * t).round() as u8
}

#[async_trait]
impl ImageGenerator for LocalImageAdapter {
    fn profile(&self) -> ProviderProfile {
        local_profile()
    }

    async fn generate(&self, request: &GenerateRequest) -> Result<Image> {
        request.validate()?;
        let seed = seed_of(request);
        let a = [(seed >> 16) as u8, (seed >> 8) as u8, seed as u8];
        let b = [(seed >> 40) as u8, (seed >> 32) as u8, (seed >> 24) as u8];
        let (w, h) = (request.width, request.height);
        let span = (w + h).max(1) as f32;
        let canvas = RgbaImage::from_fn(w, h, |x, y| {
            let t = (x + y) as f32 / span;
            Rgba([
                blend(a[0], b[0], t),
                blend(a[1], b[1], t),
                blend(a[2], b[2], t),
                255,
            ])
        });
        encode_png(canvas)
    }
}

#[async_trait]
impl ImageCompositor for LocalImageAdapter {
    fn profile(&self) -> ProviderProfile {
        local_profile()
    }

    async fn composite(&self, request: &CompositeRequest) -> Result<Image> {
        request.validate()?;
        let mut canvas = RgbaImage::new(request.width, request.height);
        for layer in &request.layers {
            let mut top = decode_rgba(&layer.image)?;
            if layer.opacity < 1.0 {
                for px in top.pixels_mut() {
                    px.0[3] = (f32::from(px.0[3]) * layer.opacity).round() as u8;
                }
            }
            image::imageops::overlay(&mut canvas, &top, layer.offset.0, layer.offset.1);
        }
        encode_png(canvas)
    }
}

#[cfg(test)]
mod tests {
    use super::{LocalImageAdapter, decode_rgba};
    use crate::domain::{CompositeRequest, GenerateRequest, ImageFormat, Layer};
    use crate::ports::{ImageCompositor, ImageGenerator};

    #[tokio::test]
    async fn generates_png_of_requested_size() {
        let adapter = LocalImageAdapter::new();
        let img = adapter
            .generate(&GenerateRequest::new("a red barn at dusk", 64, 48))
            .await
            .unwrap();
        assert_eq!((img.width, img.height), (64, 48));
        assert_eq!(img.format, ImageFormat::Png);
        let decoded = decode_rgba(&img).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 48));
    }

    #[tokio::test]
    async fn generation_is_deterministic_for_a_seed() {
        let adapter = LocalImageAdapter::new();
        let req = GenerateRequest::new("same prompt", 32, 32).with_seed(7);
        let one = adapter.generate(&req).await.unwrap();
        let two = adapter.generate(&req).await.unwrap();
        assert_eq!(one.bytes, two.bytes);
    }

    #[tokio::test]
    async fn composites_layers_onto_canvas() {
        let adapter = LocalImageAdapter::new();
        let bg = adapter
            .generate(&GenerateRequest::new("background", 100, 100))
            .await
            .unwrap();
        let fg = adapter
            .generate(&GenerateRequest::new("foreground", 40, 40))
            .await
            .unwrap();
        let out = adapter
            .composite(&CompositeRequest::new(
                "fg over bg",
                vec![Layer::new(bg), Layer::new(fg).at(30, 30).with_opacity(0.5)],
                100,
                100,
            ))
            .await
            .unwrap();
        assert_eq!((out.width, out.height), (100, 100));
        assert!(decode_rgba(&out).is_ok());
    }

    #[tokio::test]
    async fn rejects_empty_prompt_and_zero_canvas() {
        let adapter = LocalImageAdapter::new();
        assert!(
            adapter
                .generate(&GenerateRequest::new("   ", 10, 10))
                .await
                .is_err()
        );
        assert!(
            adapter
                .generate(&GenerateRequest::new("ok", 0, 10))
                .await
                .is_err()
        );
    }
}
