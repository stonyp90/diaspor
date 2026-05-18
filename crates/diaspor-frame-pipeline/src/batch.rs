//! Frame batch types — the unit of data flowing through the pipeline.

use std::pin::Pin;

use bytes::Bytes;
use diaspor_core::Result;
use futures::Stream;

/// Pixel layout of the raw frame bytes inside a [`FrameBatch`].
///
/// The pipeline does not perform colorspace conversion automatically; downstream
/// consumers must inspect this enum and convert if their model expects a different
/// layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// Planar YUV 4:2:0, 8-bit per channel. The default output of most software
    /// decoders for H.264 / H.265 sources.
    Yuv420p,
    /// Semi-planar YUV 4:2:0 (one Y plane + one interleaved UV plane). The native
    /// output of NVIDIA NVDEC and Apple `VideoToolbox`.
    Nv12,
    /// Packed 24-bit RGB, no alpha. Useful for models expecting interleaved RGB
    /// without color conversion overhead at the consumer.
    Rgb24,
    /// Packed 32-bit BGRA. Common on macOS and Windows for hardware-decoded paths.
    Bgra,
}

/// One decoded frame plus its metadata.
///
/// `FrameBatch` is the wire format flowing between [`crate::DecodeBackend`] and
/// [`crate::FrameSampler`], and ultimately the consumer. The name is "batch" rather
/// than "frame" because a future revision may pack multiple temporally-adjacent frames
/// into a single allocation for GPU upload efficiency; the current shape carries one
/// frame.
///
/// The pixel data lives in a [`bytes::Bytes`] so that decode backends can hand out
/// reference-counted slices of a larger arena without copying, and so that downstream
/// consumers can cheaply hold on to a frame after the pipeline has moved on.
#[derive(Debug, Clone)]
pub struct FrameBatch {
    /// Raw pixel data, laid out according to [`Self::pixel_format`]. No padding /
    /// stride guarantees beyond what the named [`PixelFormat`] implies.
    pub data: Bytes,
    /// Width of the frame in pixels.
    pub width: u32,
    /// Height of the frame in pixels.
    pub height: u32,
    /// How [`Self::data`] is laid out.
    pub pixel_format: PixelFormat,
    /// Presentation timestamp in microseconds from the start of the source stream.
    pub timestamp_us: u64,
    /// Monotonically increasing frame index assigned by the decoder. Useful for
    /// stable ordering across the async stream boundary.
    pub frame_index: u64,
}

/// A pinned, boxed, `Send` stream of [`FrameBatch`] results.
///
/// This is the return type for both [`crate::DecodeBackend::decode`] and
/// [`crate::FrameSampler::sample`]. Using a type alias keeps signatures readable and
/// lets us swap the underlying stream impl later (e.g. to an `async fn`-returning
/// trait) without churning every call site.
pub type FrameBatchStream = Pin<Box<dyn Stream<Item = Result<FrameBatch>> + Send>>;
