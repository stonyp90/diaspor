//! Prosody feature extraction — voice quality, pitch, energy, spectral shape.
//!
//! The default production backend (lands in milestone M7) wraps **`openSMILE`** with the
//! combined **`eGeMAPSv02` + `ComParE_2016`** feature set, producing a fixed-width
//! per-clip feature vector that downstream classifiers (deception / valence / arousal)
//! consume.
//!
//! For the alpha trait surface we model only the vector shape; the
//! [`NoopProsodyExtractor`] returns `NotImplemented`.

use async_trait::async_trait;
use bytes::Bytes;
use diaspor_core::{Result, VfsError};
use serde::{Deserialize, Serialize};

use crate::VisionError;

/// Total feature count for the combined `openSMILE` `eGeMAPSv02` + `ComParE_2016`
/// configuration.
///
/// This is a placeholder dimension for the alpha — the exact count depends on the final
/// `openSMILE` config that ships with M7 (`eGeMAPSv02` ~88, `ComParE_2016` ~6373, plus
/// derived functionals). 6552 is reserved as the target width so callers can size buffers
/// today. The constant will be re-locked when the M7 config is frozen.
pub const PROSODY_FEATURE_COUNT: usize = 6552;

/// A fixed-width prosody feature vector extracted from a single audio clip.
///
/// The vector's ordering and semantics are pinned by the `openSMILE` configuration file
/// referenced in [`ModelProvenance`](crate::record::ModelProvenance). Consumers should
/// treat the vector as opaque except via that configuration; reordering features without
/// re-running the extractor will silently invalidate downstream models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ProsodyFeatures {
    /// Sample rate of the audio the features were extracted from, in Hz.
    pub sample_rate_hz: u32,
    /// Number of audio channels (typically 1 — `openSMILE` consumes mono).
    pub channels: u16,
    /// Duration of the analyzed audio, in milliseconds.
    pub duration_ms: u64,
    /// The packed feature vector.
    ///
    /// Expected width is [`PROSODY_FEATURE_COUNT`] when populated by the production
    /// backend. The alpha [`NoopProsodyExtractor`] leaves this empty.
    pub features: Vec<f32>,
}

impl ProsodyFeatures {
    /// Builds an empty [`ProsodyFeatures`] with the given audio metadata and no features.
    #[must_use]
    pub const fn empty(sample_rate_hz: u32, channels: u16, duration_ms: u64) -> Self {
        Self {
            sample_rate_hz,
            channels,
            duration_ms,
            features: Vec::new(),
        }
    }
}

/// Extracts a fixed-width prosody feature vector from a chunk of PCM audio.
///
/// Unlike the per-frame visual extractors, prosody is computed per *clip* — `openSMILE`'s
/// functionals (mean, stddev, percentiles, …) need the full window to be meaningful. The
/// trait method therefore takes the full PCM blob, not a per-frame slice.
#[async_trait]
pub trait ProsodyExtractor: Send + Sync {
    /// Human-readable name of the backend, for logs and provenance records.
    fn name(&self) -> &'static str;

    /// Runs prosody extraction over a chunk of decoded mono PCM audio.
    ///
    /// `audio_pcm` is expected to be 16 kHz mono 16-bit PCM (matching the
    /// `diaspor-index` audio extraction contract). Backends document any deviation.
    async fn extract(&self, audio_pcm: &Bytes) -> Result<ProsodyFeatures>;
}

/// No-op prosody extractor used for trait-surface scaffolding and tests.
///
/// Always returns [`VisionError::NotImplemented`] wrapped into a [`VfsError::Backend`].
/// Replace with the `openSMILE` backend in milestone M7.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProsodyExtractor;

#[async_trait]
impl ProsodyExtractor for NoopProsodyExtractor {
    fn name(&self) -> &'static str {
        "noop-prosody"
    }

    async fn extract(&self, _audio_pcm: &Bytes) -> Result<ProsodyFeatures> {
        Err(VfsError::from(VisionError::NotImplemented {
            backend: "noop-prosody",
        }))
    }
}
