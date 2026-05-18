//! RTMPose-l (COCO 17-keypoint) pose extractor wired through `diaspor-infer`.
//!
//! `RtmposePoseExtractor` is the first real (non-stub) [`crate::pose::PoseExtractor`]
//! implementation. It composes:
//!
//! - [`diaspor_infer::ModelHub`] to resolve `rtmpose-l-coco@1` from `models.toml`
//!   (download → sha256-verify → cache → atomic-rename).
//! - [`diaspor_infer::OrtCpuInferenceBackend`] to run the ONNX model.
//! - Local decode logic to turn a `[1, 17, 64, 48]` heatmap into 17 COCO keypoints,
//!   then to map those into the 33 `BlazePose` slots `PoseFrame` is contractually
//!   committed to via `docs/schema/score-v1.json`.
//!
//! The 16 `BlazePose` slots that COCO doesn't cover (detailed finger/foot landmarks,
//! mouth corners, etc.) are filled with `visibility = 0.0`. A future `MediaPipe`
//! `BlazePose` extractor (M7) will populate all 33; until then, downstream code that
//! reads `PoseFrame.keypoints[i].visibility` must check it rather than assume the
//! point is valid.
//!
//! # Input format
//!
//! Unlike the trait-surface `frame_bytes: &Bytes` which leaves the format to the
//! implementer, `RtmposePoseExtractor::extract` requires the caller to have already
//! preprocessed the frame into a `[1, 3, 256, 192]` `f32` NCHW tensor in `ImageNet`
//! normalization (RGB, `mean = [0.485, 0.456, 0.406]`, `std = [0.229, 0.224, 0.225]`).
//! That preprocessing lives one layer up in the caller's frame pipeline so this crate
//! does not pull in the `image` crate as a hard dependency.
//!
//! The preprocessing helper [`encode_imagenet_chw`] is provided for callers that have
//! a row-major RGB `[256, 192, 3]` `u8` buffer and want a single function to do the
//! normalize + reshape.
//!
//! # Feature gate
//!
//! Gated on the `rtmpose-ort` feature so default builds of `diaspor-vision` stay light
//! (no ORT, no ndarray). Enable with `--features rtmpose-ort`.

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use diaspor_core::{Result, VfsError};
use diaspor_infer::{
    AdapterId, DType, InferenceBackend, ModelHub, ModelId, OrtCpuConfig, OrtCpuInferenceBackend,
    Tensor, TensorBatch,
};
use tracing::{debug, info};

use crate::VisionError;
use crate::pose::{PoseExtractor, PoseFrame, PoseKeypoint};

/// Default catalog id of the RTMPose-l COCO weights this extractor uses.
pub const DEFAULT_RTMPOSE_MODEL_ID: &str = "rtmpose-l-coco@1";

/// RTMPose-l input width (pixels). The ONNX export is fixed at this size.
pub const RTMPOSE_INPUT_WIDTH: usize = 192;
/// RTMPose-l input height (pixels). The ONNX export is fixed at this size.
pub const RTMPOSE_INPUT_HEIGHT: usize = 256;
/// RTMPose-l heatmap stride relative to input. Output spatial dims are
/// `(HEIGHT / STRIDE, WIDTH / STRIDE)` = `(64, 48)`.
const RTMPOSE_STRIDE: usize = 4;
const RTMPOSE_HEATMAP_HEIGHT: usize = RTMPOSE_INPUT_HEIGHT / RTMPOSE_STRIDE; // 64
const RTMPOSE_HEATMAP_WIDTH: usize = RTMPOSE_INPUT_WIDTH / RTMPOSE_STRIDE; // 48
/// Number of keypoints produced by an `RTMPose` COCO model.
pub const RTMPOSE_KEYPOINTS: usize = 17;

/// `ImageNet` RGB normalization mean (matches `RTMPose` preprocessing).
pub const IMAGENET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
/// `ImageNet` RGB normalization std.
pub const IMAGENET_STD: [f32; 3] = [0.229, 0.224, 0.225];

/// COCO 17-keypoint index → `BlazePose` 33-keypoint index.
///
/// `BlazePose` slots that have no COCO equivalent (mouth corners, full hand and foot
/// detail, hip midpoint) stay at `visibility = 0.0`. Indexing follows the published
/// `BlazePose` topology — see the upstream `MediaPipe` docs for the slot names.
const COCO_TO_BLAZEPOSE: [(usize, usize); RTMPOSE_KEYPOINTS] = [
    (0, 0),   // nose
    (1, 2),   // left_eye
    (2, 5),   // right_eye
    (3, 7),   // left_ear
    (4, 8),   // right_ear
    (5, 11),  // left_shoulder
    (6, 12),  // right_shoulder
    (7, 13),  // left_elbow
    (8, 14),  // right_elbow
    (9, 15),  // left_wrist
    (10, 16), // right_wrist
    (11, 23), // left_hip
    (12, 24), // right_hip
    (13, 25), // left_knee
    (14, 26), // right_knee
    (15, 27), // left_ankle
    (16, 28), // right_ankle
];

/// An ORT-backed pose extractor that loads `RTMPose` via [`ModelHub`].
///
/// Use [`RtmposePoseExtractor::from_hub`] to build one — it resolves the model id,
/// asks the hub for the local path, and constructs the ORT-CPU backend. The
/// resulting extractor is `Send + Sync + Clone` (cheap clone via `Arc`).
#[derive(Clone)]
pub struct RtmposePoseExtractor {
    backend: Arc<dyn InferenceBackend>,
    model_id: ModelId,
}

impl RtmposePoseExtractor {
    /// Builds an extractor by resolving the default `RTMPose` model id through the hub.
    ///
    /// # Errors
    ///
    /// Bubbles up any [`diaspor_infer::HubError`] from `ModelHub::resolve` (download
    /// failed, sha256 mismatch, offline-blocked, etc.) or [`diaspor_infer::InferError`]
    /// from the ORT-CPU constructor.
    pub async fn from_hub(hub: &ModelHub) -> Result<Self> {
        Self::from_hub_with_id(hub, DEFAULT_RTMPOSE_MODEL_ID).await
    }

    /// Same as [`Self::from_hub`] but lets the caller pick a non-default `RTMPose` catalog id
    /// (e.g. a tenant-pinned mirror of the same weights).
    pub async fn from_hub_with_id(hub: &ModelHub, model_id: &str) -> Result<Self> {
        let model_path = hub.resolve(model_id).await.map_err(|e| {
            VfsError::Backend(format!("rtmpose-ort: hub.resolve({model_id}) failed: {e}"))
        })?;
        info!(model_id, path = ?model_path, "rtmpose-ort: resolved model path");

        let backend = OrtCpuInferenceBackend::new(OrtCpuConfig {
            onnx_path: model_path,
            threads: 1,
        })
        .map_err(|e| VfsError::Backend(format!("rtmpose-ort: backend constructor: {e}")))?;

        Ok(Self {
            backend: Arc::new(backend),
            model_id: ModelId::new(model_id),
        })
    }

    /// Builds an extractor from an already-constructed backend. The primary use is unit
    /// tests that wire in a `MockBackend`; production code should prefer
    /// [`Self::from_hub`].
    #[must_use]
    pub fn with_backend(backend: Arc<dyn InferenceBackend>, model_id: ModelId) -> Self {
        Self { backend, model_id }
    }
}

#[async_trait]
impl PoseExtractor for RtmposePoseExtractor {
    fn name(&self) -> &'static str {
        "rtmpose-l-coco-ort"
    }

    async fn extract(&self, frame_bytes: &Bytes, timestamp_ms: u64) -> Result<PoseFrame> {
        // Expect a fully-preprocessed [1, 3, H, W] f32 NCHW tensor.
        let expected_bytes = 3 * RTMPOSE_INPUT_HEIGHT * RTMPOSE_INPUT_WIDTH * 4;
        if frame_bytes.len() != expected_bytes {
            return Err(VisionError::MalformedInput(format!(
                "rtmpose: expected {expected_bytes} bytes for [1, 3, {h}, {w}] f32 NCHW input, got {got}",
                expected_bytes = expected_bytes,
                h = RTMPOSE_INPUT_HEIGHT,
                w = RTMPOSE_INPUT_WIDTH,
                got = frame_bytes.len(),
            ))
            .into());
        }

        let input = Tensor::new(
            "input",
            vec![1, 3, RTMPOSE_INPUT_HEIGHT, RTMPOSE_INPUT_WIDTH],
            DType::F32,
            frame_bytes.clone(),
        );
        let batch = TensorBatch::new(vec![input]);
        let adapter: Option<&AdapterId> = None;
        let result = self.backend.run(&self.model_id, adapter, batch).await?;
        debug!(
            latency_us = result.latency_us,
            outputs = result.tensors.len(),
            "rtmpose-ort: backend.run returned"
        );

        let heatmap = result.tensors.first().ok_or_else(|| {
            VisionError::PoseFailed("rtmpose: backend returned zero output tensors".into())
        })?;
        decode_rtmpose_heatmap(heatmap, timestamp_ms)
    }
}

/// Decodes an `RTMPose` `[1, 17, 64, 48]` `f32` heatmap into a [`PoseFrame`].
///
/// Pulled out of the trait impl so the decode logic can be unit-tested against
/// synthetic heatmaps without needing a real ORT backend.
///
/// # Errors
///
/// Returns [`VisionError::PoseFailed`] if the tensor shape doesn't match the expected
/// `[1, 17, 64, 48]` layout, or the dtype is not F32, or the byte count is wrong.
// Pixel coordinates are always small integers (<512), well under f32's 23-bit
// mantissa precision — the cast_precision_loss lint fires on the cast but there is none.
#[allow(clippy::cast_precision_loss)]
pub fn decode_rtmpose_heatmap(heatmap: &Tensor, timestamp_ms: u64) -> Result<PoseFrame> {
    let expected_shape = [
        1,
        RTMPOSE_KEYPOINTS,
        RTMPOSE_HEATMAP_HEIGHT,
        RTMPOSE_HEATMAP_WIDTH,
    ];
    if heatmap.shape != expected_shape {
        return Err(VisionError::PoseFailed(format!(
            "rtmpose: heatmap shape {:?} does not match expected {:?}",
            heatmap.shape, expected_shape,
        ))
        .into());
    }
    if heatmap.dtype != DType::F32 {
        return Err(VisionError::PoseFailed(format!(
            "rtmpose: heatmap dtype {:?} is not F32",
            heatmap.dtype,
        ))
        .into());
    }
    let expected_bytes = RTMPOSE_KEYPOINTS * RTMPOSE_HEATMAP_HEIGHT * RTMPOSE_HEATMAP_WIDTH * 4;
    if heatmap.bytes.len() != expected_bytes {
        return Err(VisionError::PoseFailed(format!(
            "rtmpose: heatmap byte length {} does not match expected {expected_bytes}",
            heatmap.bytes.len(),
        ))
        .into());
    }

    let mut frame = PoseFrame::empty(timestamp_ms);
    let stride_h = RTMPOSE_HEATMAP_HEIGHT * RTMPOSE_HEATMAP_WIDTH;
    for (coco_idx, blaze_idx) in COCO_TO_BLAZEPOSE {
        let channel_start = coco_idx * stride_h * 4;
        let (peak_y, peak_x, peak_value) =
            argmax_heatmap_channel(&heatmap.bytes[channel_start..channel_start + stride_h * 4]);
        // Normalize to [0, 1] in input-image coordinates so callers can scale to the
        // original frame. Input is 256x192, heatmap is 64x48, so we multiply by stride
        // and divide by input dims.
        let x_norm = (peak_x as f32 * RTMPOSE_STRIDE as f32) / RTMPOSE_INPUT_WIDTH as f32;
        let y_norm = (peak_y as f32 * RTMPOSE_STRIDE as f32) / RTMPOSE_INPUT_HEIGHT as f32;
        // RTMPose heatmap peak is roughly a Gaussian; clamp the visibility to [0, 1].
        let visibility = peak_value.clamp(0.0, 1.0);
        frame.keypoints[blaze_idx] = PoseKeypoint {
            x: x_norm,
            y: y_norm,
            z: 0.0, // RTMPose-l-coco is 2D only; z stays at 0.
            visibility,
        };
    }
    Ok(frame)
}

/// Returns `(y, x, value)` of the argmax over a single `f32` heatmap channel.
fn argmax_heatmap_channel(channel_bytes: &[u8]) -> (usize, usize, f32) {
    let mut best = f32::NEG_INFINITY;
    let mut best_idx = 0usize;
    let n = channel_bytes.len() / 4;
    for i in 0..n {
        let off = i * 4;
        let v = f32::from_ne_bytes([
            channel_bytes[off],
            channel_bytes[off + 1],
            channel_bytes[off + 2],
            channel_bytes[off + 3],
        ]);
        if v > best {
            best = v;
            best_idx = i;
        }
    }
    (
        best_idx / RTMPOSE_HEATMAP_WIDTH,
        best_idx % RTMPOSE_HEATMAP_WIDTH,
        best,
    )
}

/// Preprocessing helper: turns a row-major RGB `[H, W, 3]` `u8` buffer into the
/// ImageNet-normalized `[1, 3, H, W]` `f32` NCHW byte buffer the extractor expects.
///
/// `height` and `width` must equal [`RTMPOSE_INPUT_HEIGHT`] and [`RTMPOSE_INPUT_WIDTH`]
/// — this helper does not resize. Use the `image` crate (or any other) one layer up
/// if your source frame is a different size.
///
/// # Errors
///
/// Returns [`VisionError::MalformedInput`] if the buffer length does not match
/// `height * width * 3`.
pub fn encode_imagenet_chw(rgb: &[u8], height: usize, width: usize) -> Result<Bytes> {
    let expected = height * width * 3;
    if rgb.len() != expected {
        return Err(VisionError::MalformedInput(format!(
            "encode_imagenet_chw: expected {expected} bytes for [{height}, {width}, 3] RGB u8, got {}",
            rgb.len(),
        ))
        .into());
    }
    let mut out = Vec::with_capacity(3 * height * width * 4);
    // NHWC -> NCHW: iterate channel-major.
    for c in 0..3 {
        for y in 0..height {
            for x in 0..width {
                let pixel_off = (y * width + x) * 3 + c;
                let v = f32::from(rgb[pixel_off]) / 255.0;
                let normalized = (v - IMAGENET_MEAN[c]) / IMAGENET_STD[c];
                out.extend_from_slice(&normalized.to_ne_bytes());
            }
        }
    }
    Ok(Bytes::from(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use diaspor_infer::{AdapterId, InferError, InferenceBackend, ModelId, Tensor, TensorBatch};

    /// Fake backend that returns a pre-canned heatmap on every `run` call. Lets us
    /// unit-test the decode pipeline without a real ORT runtime.
    struct MockBackend {
        canned_output: Vec<u8>,
        return_zero_outputs: bool,
    }

    #[async_trait]
    impl InferenceBackend for MockBackend {
        fn name(&self) -> &'static str {
            "mock"
        }

        async fn run(
            &self,
            _model: &ModelId,
            _adapter: Option<&AdapterId>,
            _inputs: TensorBatch,
        ) -> diaspor_core::Result<TensorBatch> {
            if self.return_zero_outputs {
                return Ok(TensorBatch::new(vec![]));
            }
            Ok(TensorBatch::new(vec![Tensor::new(
                "heatmap",
                vec![
                    1,
                    RTMPOSE_KEYPOINTS,
                    RTMPOSE_HEATMAP_HEIGHT,
                    RTMPOSE_HEATMAP_WIDTH,
                ],
                DType::F32,
                Bytes::from(self.canned_output.clone()),
            )]))
        }
    }

    fn synth_heatmap_with_peak_at(channel: usize, peak_y: usize, peak_x: usize) -> Vec<u8> {
        let mut buf =
            vec![0u8; RTMPOSE_KEYPOINTS * RTMPOSE_HEATMAP_HEIGHT * RTMPOSE_HEATMAP_WIDTH * 4];
        let stride_channel = RTMPOSE_HEATMAP_HEIGHT * RTMPOSE_HEATMAP_WIDTH * 4;
        let idx = peak_y * RTMPOSE_HEATMAP_WIDTH + peak_x;
        let off = channel * stride_channel + idx * 4;
        let bytes = 0.95_f32.to_ne_bytes();
        buf[off..off + 4].copy_from_slice(&bytes);
        buf
    }

    // Tests compare against the explicit 0.0 sentinel that PoseFrame::empty stamps into
    // unmapped slots — exact f32 equality is the right test here, not approximate.
    #[allow(clippy::float_cmp)]
    #[tokio::test]
    async fn extract_returns_peak_in_correct_blazepose_slot() {
        // Synthetic heatmap with one bright pixel in channel 5 (= COCO left_shoulder ->
        // BlazePose slot 11) at (peak_y=32, peak_x=24).
        let heatmap_bytes = synth_heatmap_with_peak_at(5, 32, 24);
        let backend = Arc::new(MockBackend {
            canned_output: heatmap_bytes,
            return_zero_outputs: false,
        });
        let extractor = RtmposePoseExtractor::with_backend(backend, ModelId::new("rtmpose-test"));

        // Caller-supplied [1, 3, 256, 192] f32 NCHW; contents don't matter for the mock.
        let dummy_input = Bytes::from(vec![
            0u8;
            3 * RTMPOSE_INPUT_HEIGHT * RTMPOSE_INPUT_WIDTH * 4
        ]);
        let frame = extractor
            .extract(&dummy_input, 1234)
            .await
            .expect("decode must succeed");

        assert_eq!(frame.timestamp_ms, 1234);
        let kp = frame.keypoints[11]; // BlazePose left_shoulder
        // Heatmap stride = 4, so peak (32, 24) in heatmap maps to (128, 96) in input
        // image, which normalizes to (0.5, 0.5).
        assert!((kp.x - (96.0 / 192.0)).abs() < 1e-6, "kp.x = {}", kp.x);
        assert!((kp.y - (128.0 / 256.0)).abs() < 1e-6, "kp.y = {}", kp.y);
        assert!(
            (kp.visibility - 0.95).abs() < 1e-6,
            "kp.visibility = {}",
            kp.visibility
        );
        assert_eq!(kp.z, 0.0);

        // Slots BlazePose doesn't have a COCO source for stay zeroed.
        for unmapped in [1, 3, 4, 6, 9, 10, 17, 18, 19, 20, 21, 22, 29, 30, 31, 32] {
            let kp = frame.keypoints[unmapped];
            assert_eq!(kp.x, 0.0);
            assert_eq!(kp.y, 0.0);
            assert_eq!(kp.visibility, 0.0);
        }
    }

    #[tokio::test]
    async fn extract_rejects_wrong_input_size() {
        let backend = Arc::new(MockBackend {
            canned_output: vec![],
            return_zero_outputs: false,
        });
        let extractor = RtmposePoseExtractor::with_backend(backend, ModelId::new("rtmpose-test"));
        let result = extractor.extract(&Bytes::from(vec![0u8; 10]), 0).await;
        let err = result.expect_err("wrong-size input must error");
        assert!(err.to_string().contains("expected"), "err: {err}");
    }

    #[tokio::test]
    async fn extract_rejects_zero_output_tensors() {
        let backend = Arc::new(MockBackend {
            canned_output: vec![],
            return_zero_outputs: true,
        });
        let extractor = RtmposePoseExtractor::with_backend(backend, ModelId::new("rtmpose-test"));
        let dummy = Bytes::from(vec![
            0u8;
            3 * RTMPOSE_INPUT_HEIGHT * RTMPOSE_INPUT_WIDTH * 4
        ]);
        let err = extractor.extract(&dummy, 0).await.expect_err("must error");
        assert!(
            err.to_string().contains("zero output tensors"),
            "err: {err}"
        );
    }

    #[test]
    fn encode_imagenet_chw_roundtrip() {
        let rgb = vec![128u8; RTMPOSE_INPUT_HEIGHT * RTMPOSE_INPUT_WIDTH * 3];
        let encoded = encode_imagenet_chw(&rgb, RTMPOSE_INPUT_HEIGHT, RTMPOSE_INPUT_WIDTH)
            .expect("encode must succeed");
        assert_eq!(
            encoded.len(),
            3 * RTMPOSE_INPUT_HEIGHT * RTMPOSE_INPUT_WIDTH * 4,
            "encoded byte length matches f32 NCHW",
        );
        // First pixel R: 128/255 - 0.485 / 0.229 ≈ (0.5019 - 0.485) / 0.229 ≈ 0.07396
        let expected = (128.0_f32 / 255.0 - IMAGENET_MEAN[0]) / IMAGENET_STD[0];
        let actual = f32::from_ne_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]);
        assert!(
            (actual - expected).abs() < 1e-5,
            "actual = {actual}, expected = {expected}"
        );
    }

    #[test]
    fn encode_imagenet_chw_rejects_wrong_buffer_size() {
        let err = encode_imagenet_chw(&[0u8; 5], 256, 192).expect_err("must reject");
        assert!(err.to_string().contains("expected"), "err: {err}");
    }

    // Silence the unused warning when running the test binary without the InferError
    // import path actually being hit by this minimal test set.
    #[allow(dead_code)]
    fn _force_use_of_infer_error_for_path() -> InferError {
        InferError::NotImplemented { backend: "_" }
    }
}
