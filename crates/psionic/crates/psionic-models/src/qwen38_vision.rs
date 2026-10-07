use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    QWEN38_27B_MODEL_ID, QWEN38_27B_UPSTREAM_REVISION, Qwen35TextObservedTensorSpec,
    qwen35_text_observed_tensors_from_shards,
};

pub const QWEN38_VISION_ARTIFACT_SCHEMA_VERSION: &str =
    "psionic.qwen38.vision_artifact_admission.v1";
pub const QWEN38_VISION_PREPROCESSING_SCHEMA_VERSION: &str =
    "psionic.qwen38.vision_preprocessing.v1";
pub const QWEN38_VISION_SOURCE_SHARD: &str = "model-00001-of-00018.safetensors";
pub const QWEN38_VISION_SOURCE_SHARD_BYTES: u64 = 3_966_730_552;
pub const QWEN38_VISION_SOURCE_SHARD_SHA256: &str =
    "ba0ce20aae489ad196733da5064bcdf159a1fe84f53336648196e1ebb7751b1c";
pub const QWEN38_VISION_TENSOR_COUNT: usize = 333;
pub const QWEN38_VISION_TENSOR_BYTES: u64 = 921_460_192;
pub const QWEN38_IMAGE_PROCESSOR_CONFIG: &str = "preprocessor_config.json";
pub const QWEN38_IMAGE_PROCESSOR_SHA256: &str =
    "27225450ac9c6529872ee1924fcb0962ff5634834f817040f444118116f4e516";
pub const QWEN38_VIDEO_PROCESSOR_CONFIG: &str = "video_preprocessor_config.json";
pub const QWEN38_VIDEO_PROCESSOR_SHA256: &str =
    "7768af27c1fafa9cc9011c1dc20067e03f8915e03b63504550e11d5066986d13";
pub const QWEN38_VISION_PATCH_SIZE: usize = 16;
pub const QWEN38_VISION_TEMPORAL_PATCH_SIZE: usize = 2;
pub const QWEN38_VISION_SPATIAL_MERGE_SIZE: usize = 2;
pub const QWEN38_VISION_PATCH_VECTOR_SIZE: usize = 1_536;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38VisionTensorSpec {
    pub name: String,
    pub dtype: String,
    pub shape: Vec<usize>,
}

impl Qwen38VisionTensorSpec {
    fn byte_len(&self) -> u64 {
        self.shape.iter().fold(2u64, |bytes, dimension| {
            bytes.saturating_mul(*dimension as u64)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38VisionArtifactAdmissionStatus {
    Admitted,
    Refused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38VisionArtifactRefusalCode {
    SourceShardIdentityMismatch,
    ProcessorIdentityMismatch,
    TensorInventoryMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38VisionTensorMismatch {
    pub name: String,
    pub expected_dtype: String,
    pub observed_dtype: String,
    pub expected_shape: Vec<usize>,
    pub observed_shape: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38VisionArtifactAdmissionReport {
    pub schema_version: String,
    pub status: Qwen38VisionArtifactAdmissionStatus,
    pub source_repository: String,
    pub source_revision: String,
    pub source_shard: String,
    pub source_shard_bytes: u64,
    pub source_shard_sha256: String,
    pub image_processor_config: String,
    pub image_processor_sha256: String,
    pub video_processor_config: String,
    pub video_processor_sha256: String,
    pub expected_tensor_count: usize,
    pub observed_tensor_count: usize,
    pub expected_tensor_bytes: u64,
    pub observed_tensor_bytes: u64,
    pub missing_tensors: Vec<String>,
    pub extra_tensors: Vec<String>,
    pub tensor_mismatches: Vec<Qwen38VisionTensorMismatch>,
    pub non_vision_tensor_count_in_source_shard: usize,
    pub native_tensor_prefix: String,
    pub extraction_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_code: Option<Qwen38VisionArtifactRefusalCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

#[derive(Debug, Error)]
pub enum Qwen38VisionArtifactError {
    #[error("failed to read Qwen3.8 vision artifact `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to inspect Qwen3.8 vision safetensors: {0}")]
    SafeTensors(String),
}

pub fn qwen38_vision_expected_tensor_specs() -> Vec<Qwen38VisionTensorSpec> {
    let mut specs = Vec::with_capacity(QWEN38_VISION_TENSOR_COUNT);
    for layer in 0..27 {
        let prefix = format!("model.visual.blocks.{layer}");
        for (suffix, shape) in [
            ("attn.proj.bias", vec![1_152]),
            ("attn.proj.weight", vec![1_152, 1_152]),
            ("attn.qkv.bias", vec![3_456]),
            ("attn.qkv.weight", vec![3_456, 1_152]),
            ("mlp.linear_fc1.bias", vec![4_304]),
            ("mlp.linear_fc1.weight", vec![4_304, 1_152]),
            ("mlp.linear_fc2.bias", vec![1_152]),
            ("mlp.linear_fc2.weight", vec![1_152, 4_304]),
            ("norm1.bias", vec![1_152]),
            ("norm1.weight", vec![1_152]),
            ("norm2.bias", vec![1_152]),
            ("norm2.weight", vec![1_152]),
        ] {
            specs.push(Qwen38VisionTensorSpec {
                name: format!("{prefix}.{suffix}"),
                dtype: String::from("BF16"),
                shape,
            });
        }
    }
    for (name, shape) in [
        ("model.visual.merger.linear_fc1.bias", vec![4_608]),
        ("model.visual.merger.linear_fc1.weight", vec![4_608, 4_608]),
        ("model.visual.merger.linear_fc2.bias", vec![5_120]),
        ("model.visual.merger.linear_fc2.weight", vec![5_120, 4_608]),
        ("model.visual.merger.norm.bias", vec![1_152]),
        ("model.visual.merger.norm.weight", vec![1_152]),
        ("model.visual.patch_embed.proj.bias", vec![1_152]),
        (
            "model.visual.patch_embed.proj.weight",
            vec![1_152, 3, 2, 16, 16],
        ),
        ("model.visual.pos_embed.weight", vec![2_304, 1_152]),
    ] {
        specs.push(Qwen38VisionTensorSpec {
            name: String::from(name),
            dtype: String::from("BF16"),
            shape,
        });
    }
    specs.sort_by(|left, right| left.name.cmp(&right.name));
    specs
}

pub fn inspect_qwen38_vision_artifact(
    model_dir: impl AsRef<Path>,
) -> Result<Qwen38VisionArtifactAdmissionReport, Qwen38VisionArtifactError> {
    let model_dir = model_dir.as_ref();
    let shard_path = model_dir.join(QWEN38_VISION_SOURCE_SHARD);
    let image_processor_path = model_dir.join(QWEN38_IMAGE_PROCESSOR_CONFIG);
    let video_processor_path = model_dir.join(QWEN38_VIDEO_PROCESSOR_CONFIG);
    let source_shard_bytes = file_len(&shard_path)?;
    let source_shard_sha256 = sha256_file(&shard_path)?;
    let image_processor_sha256 = sha256_file(&image_processor_path)?;
    let video_processor_sha256 = sha256_file(&video_processor_path)?;
    let observed = qwen35_text_observed_tensors_from_shards(std::slice::from_ref(&shard_path))
        .map_err(|error| Qwen38VisionArtifactError::SafeTensors(error.to_string()))?;

    Ok(qwen38_vision_artifact_admission_from_observed(
        source_shard_bytes,
        source_shard_sha256,
        image_processor_sha256,
        video_processor_sha256,
        observed.tensors.as_slice(),
    ))
}

pub fn qwen38_vision_artifact_admission_from_observed(
    source_shard_bytes: u64,
    source_shard_sha256: String,
    image_processor_sha256: String,
    video_processor_sha256: String,
    observed: &[Qwen35TextObservedTensorSpec],
) -> Qwen38VisionArtifactAdmissionReport {
    let expected = qwen38_vision_expected_tensor_specs();
    let expected_tensor_bytes = expected
        .iter()
        .map(Qwen38VisionTensorSpec::byte_len)
        .sum::<u64>();
    let expected_by_name = expected
        .iter()
        .map(|tensor| (tensor.name.as_str(), tensor))
        .collect::<BTreeMap<_, _>>();
    let observed_vision = observed
        .iter()
        .filter(|tensor| tensor.name.starts_with("model.visual."))
        .collect::<Vec<_>>();
    let observed_by_name = observed_vision
        .iter()
        .map(|tensor| (tensor.name.as_str(), *tensor))
        .collect::<BTreeMap<_, _>>();
    let missing_tensors = expected_by_name
        .keys()
        .filter(|name| !observed_by_name.contains_key(**name))
        .map(|name| String::from(*name))
        .collect::<Vec<_>>();
    let extra_tensors = observed_by_name
        .keys()
        .filter(|name| !expected_by_name.contains_key(**name))
        .map(|name| String::from(*name))
        .collect::<Vec<_>>();
    let tensor_mismatches = expected_by_name
        .iter()
        .filter_map(|(name, expected)| {
            let observed = observed_by_name.get(name)?;
            (observed.dtype != expected.dtype || observed.shape != expected.shape).then(|| {
                Qwen38VisionTensorMismatch {
                    name: String::from(*name),
                    expected_dtype: expected.dtype.clone(),
                    observed_dtype: observed.dtype.clone(),
                    expected_shape: expected.shape.clone(),
                    observed_shape: observed.shape.clone(),
                }
            })
        })
        .collect::<Vec<_>>();
    let observed_tensor_bytes = observed_vision.iter().fold(0u64, |bytes, tensor| {
        let element_bytes = match tensor.dtype.as_str() {
            "BF16" | "F16" => 2,
            "F32" => 4,
            _ => 0,
        };
        bytes.saturating_add(
            tensor
                .shape
                .iter()
                .fold(element_bytes, |tensor_bytes, dimension| {
                    tensor_bytes.saturating_mul(*dimension as u64)
                }),
        )
    });
    let source_identity_matches = source_shard_bytes == QWEN38_VISION_SOURCE_SHARD_BYTES
        && source_shard_sha256 == QWEN38_VISION_SOURCE_SHARD_SHA256;
    let processor_identity_matches = image_processor_sha256 == QWEN38_IMAGE_PROCESSOR_SHA256
        && video_processor_sha256 == QWEN38_VIDEO_PROCESSOR_SHA256;
    let tensor_inventory_matches = observed_vision.len() == QWEN38_VISION_TENSOR_COUNT
        && observed_tensor_bytes == expected_tensor_bytes
        && missing_tensors.is_empty()
        && extra_tensors.is_empty()
        && tensor_mismatches.is_empty();
    let (status, refusal_code, refusal_detail) = if !source_identity_matches {
        (
            Qwen38VisionArtifactAdmissionStatus::Refused,
            Some(Qwen38VisionArtifactRefusalCode::SourceShardIdentityMismatch),
            Some(String::from(
                "the official source shard byte length or SHA-256 does not match the pinned Qwen3.8 revision",
            )),
        )
    } else if !processor_identity_matches {
        (
            Qwen38VisionArtifactAdmissionStatus::Refused,
            Some(Qwen38VisionArtifactRefusalCode::ProcessorIdentityMismatch),
            Some(String::from(
                "the image or video processor config SHA-256 does not match the pinned Qwen3.8 revision",
            )),
        )
    } else if !tensor_inventory_matches {
        (
            Qwen38VisionArtifactAdmissionStatus::Refused,
            Some(Qwen38VisionArtifactRefusalCode::TensorInventoryMismatch),
            Some(String::from(
                "the model.visual tensor inventory, dtype, shape, or byte count is incomplete",
            )),
        )
    } else {
        (Qwen38VisionArtifactAdmissionStatus::Admitted, None, None)
    };

    Qwen38VisionArtifactAdmissionReport {
        schema_version: String::from(QWEN38_VISION_ARTIFACT_SCHEMA_VERSION),
        status,
        source_repository: String::from(QWEN38_27B_MODEL_ID),
        source_revision: String::from(QWEN38_27B_UPSTREAM_REVISION),
        source_shard: String::from(QWEN38_VISION_SOURCE_SHARD),
        source_shard_bytes,
        source_shard_sha256,
        image_processor_config: String::from(QWEN38_IMAGE_PROCESSOR_CONFIG),
        image_processor_sha256,
        video_processor_config: String::from(QWEN38_VIDEO_PROCESSOR_CONFIG),
        video_processor_sha256,
        expected_tensor_count: QWEN38_VISION_TENSOR_COUNT,
        observed_tensor_count: observed_vision.len(),
        expected_tensor_bytes,
        observed_tensor_bytes,
        missing_tensors,
        extra_tensors,
        tensor_mismatches,
        non_vision_tensor_count_in_source_shard: observed.len() - observed_vision.len(),
        native_tensor_prefix: String::from("model.visual"),
        extraction_required: false,
        refusal_code,
        refusal_detail,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38VisionMediaKind {
    Image,
    Video,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38RgbFrame {
    pub width: usize,
    pub height: usize,
    pub rgb8: Vec<u8>,
}

impl Qwen38RgbFrame {
    pub fn new(width: usize, height: usize, rgb8: Vec<u8>) -> Result<Self, Qwen38VisionError> {
        let expected = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(3))
            .ok_or(Qwen38VisionError::SizeOverflow)?;
        if rgb8.len() != expected {
            return Err(Qwen38VisionError::InvalidRgbByteLength {
                expected,
                actual: rgb8.len(),
            });
        }
        Ok(Self {
            width,
            height,
            rgb8,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38VisionAdmissionLimits {
    pub minimum_image_pixels: usize,
    pub maximum_image_pixels: usize,
    pub minimum_video_frames: usize,
    pub maximum_video_frames: usize,
    pub maximum_video_frame_pixels: usize,
    pub maximum_total_patch_count: usize,
    pub maximum_attachment_bytes: usize,
    pub timeout_ms: u64,
    pub resize_policy: String,
}

impl Default for Qwen38VisionAdmissionLimits {
    fn default() -> Self {
        Self {
            minimum_image_pixels: 65_536,
            maximum_image_pixels: 262_144,
            minimum_video_frames: 4,
            maximum_video_frames: 8,
            maximum_video_frame_pixels: 65_536,
            maximum_total_patch_count: 1_024,
            maximum_attachment_bytes: 8 * 1_024 * 1_024,
            timeout_ms: 30_000,
            resize_policy: String::from("refuse_when_upstream_resize_is_required"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38VisionPreprocessingReceipt {
    pub schema_version: String,
    pub media_kind: Qwen38VisionMediaKind,
    pub attachment_id: String,
    pub source_mime_type: String,
    pub source_sha256: String,
    pub source_bytes: usize,
    pub source_frame_count: usize,
    pub sampled_frame_indices: Vec<usize>,
    pub padded_frame_count: usize,
    pub width: usize,
    pub height: usize,
    pub grid_thw: [usize; 3],
    pub patch_vector_size: usize,
    pub patch_count: usize,
    pub merged_token_count: usize,
    pub pixel_values_dtype: String,
    pub pixel_values_sha256: String,
    pub processor_name: String,
    pub processor_config_sha256: String,
    pub resize_applied: bool,
    pub normalization_mean: [f32; 3],
    pub normalization_std: [f32; 3],
    pub limits: Qwen38VisionAdmissionLimits,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38VisionPreprocessedInput {
    pub pixel_values: Vec<f32>,
    pub receipt: Qwen38VisionPreprocessingReceipt,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum Qwen38VisionError {
    #[error("Qwen3.8 vision input size overflow")]
    SizeOverflow,
    #[error("Qwen3.8 RGB input has {actual} bytes; expected {expected}")]
    InvalidRgbByteLength { expected: usize, actual: usize },
    #[error("Qwen3.8 vision attachment exceeds {maximum} bytes: {actual}")]
    AttachmentBytesExceeded { maximum: usize, actual: usize },
    #[error("Qwen3.8 vision input dimensions must be nonzero")]
    EmptyDimensions,
    #[error("Qwen3.8 vision input aspect ratio exceeds 200:1")]
    AspectRatioExceeded,
    #[error(
        "Qwen3.8 vision input {width}x{height} requires upstream resizing; this bounded lane admits dimensions divisible by {factor} only"
    )]
    ResizeRequired {
        width: usize,
        height: usize,
        factor: usize,
    },
    #[error("Qwen3.8 image pixel count {actual} is outside [{minimum}, {maximum}]")]
    ImagePixelsOutOfBounds {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
    #[error("Qwen3.8 video frame count {actual} is outside [{minimum}, {maximum}]")]
    VideoFramesOutOfBounds {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
    #[error("Qwen3.8 video frames must have identical dimensions")]
    VideoFrameShapeMismatch,
    #[error("Qwen3.8 video frame pixel count {actual} exceeds {maximum}")]
    VideoFramePixelsExceeded { maximum: usize, actual: usize },
    #[error("Qwen3.8 vision patch count {actual} exceeds {maximum}")]
    PatchCountExceeded { maximum: usize, actual: usize },
    #[error("Qwen3.8 video sampling requires a finite positive source fps")]
    InvalidSourceFps,
    #[error("invalid Qwen3.8 vision preprocessing receipt: {detail}")]
    InvalidPreprocessingReceipt { detail: String },
}

pub fn qwen38_preprocess_image(
    attachment_id: impl Into<String>,
    source_mime_type: impl Into<String>,
    frame: &Qwen38RgbFrame,
    limits: Qwen38VisionAdmissionLimits,
) -> Result<Qwen38VisionPreprocessedInput, Qwen38VisionError> {
    validate_common_frame(frame)?;
    validate_attachment_bytes(std::slice::from_ref(frame), &limits)?;
    let pixels = frame
        .width
        .checked_mul(frame.height)
        .ok_or(Qwen38VisionError::SizeOverflow)?;
    if pixels < limits.minimum_image_pixels || pixels > limits.maximum_image_pixels {
        return Err(Qwen38VisionError::ImagePixelsOutOfBounds {
            minimum: limits.minimum_image_pixels,
            maximum: limits.maximum_image_pixels,
            actual: pixels,
        });
    }
    let frames = [frame, frame];
    preprocess_admitted_frames(
        Qwen38VisionMediaKind::Image,
        attachment_id.into(),
        source_mime_type.into(),
        std::slice::from_ref(frame),
        frames.as_slice(),
        vec![0],
        limits,
    )
}

pub fn qwen38_preprocess_video(
    attachment_id: impl Into<String>,
    source_mime_type: impl Into<String>,
    source_frames: &[Qwen38RgbFrame],
    source_fps: f64,
    limits: Qwen38VisionAdmissionLimits,
) -> Result<Qwen38VisionPreprocessedInput, Qwen38VisionError> {
    if !source_fps.is_finite() || source_fps <= 0.0 {
        return Err(Qwen38VisionError::InvalidSourceFps);
    }
    validate_attachment_bytes(source_frames, &limits)?;
    let sampled_frame_indices = qwen38_video_sample_indices(
        source_frames.len(),
        source_fps,
        2.0,
        limits.minimum_video_frames,
        limits.maximum_video_frames,
    )?;
    let sampled_frames = sampled_frame_indices
        .iter()
        .map(|index| &source_frames[*index])
        .collect::<Vec<_>>();
    for frame in &sampled_frames {
        validate_common_frame(frame)?;
    }
    let first = sampled_frames
        .first()
        .ok_or(Qwen38VisionError::VideoFramesOutOfBounds {
            minimum: limits.minimum_video_frames,
            maximum: limits.maximum_video_frames,
            actual: 0,
        })?;
    if sampled_frames
        .iter()
        .any(|frame| frame.width != first.width || frame.height != first.height)
    {
        return Err(Qwen38VisionError::VideoFrameShapeMismatch);
    }
    let frame_pixels = first
        .width
        .checked_mul(first.height)
        .ok_or(Qwen38VisionError::SizeOverflow)?;
    if frame_pixels > limits.maximum_video_frame_pixels {
        return Err(Qwen38VisionError::VideoFramePixelsExceeded {
            maximum: limits.maximum_video_frame_pixels,
            actual: frame_pixels,
        });
    }
    let mut padded_frames = sampled_frames.clone();
    if padded_frames.len() % QWEN38_VISION_TEMPORAL_PATCH_SIZE != 0 {
        let last =
            padded_frames
                .last()
                .copied()
                .ok_or(Qwen38VisionError::VideoFramesOutOfBounds {
                    minimum: limits.minimum_video_frames,
                    maximum: limits.maximum_video_frames,
                    actual: 0,
                })?;
        padded_frames.push(last);
    }
    preprocess_admitted_frames(
        Qwen38VisionMediaKind::Video,
        attachment_id.into(),
        source_mime_type.into(),
        source_frames,
        padded_frames.as_slice(),
        sampled_frame_indices,
        limits,
    )
}

pub fn qwen38_video_sample_indices(
    total_frame_count: usize,
    source_fps: f64,
    target_fps: f64,
    minimum_frames: usize,
    maximum_frames: usize,
) -> Result<Vec<usize>, Qwen38VisionError> {
    if !source_fps.is_finite() || source_fps <= 0.0 || !target_fps.is_finite() || target_fps <= 0.0
    {
        return Err(Qwen38VisionError::InvalidSourceFps);
    }
    if total_frame_count < minimum_frames {
        return Err(Qwen38VisionError::VideoFramesOutOfBounds {
            minimum: minimum_frames,
            maximum: maximum_frames,
            actual: total_frame_count,
        });
    }
    let requested = ((total_frame_count as f64 / source_fps) * target_fps) as usize;
    let sampled = requested
        .max(minimum_frames)
        .min(maximum_frames)
        .min(total_frame_count);
    if sampled == 1 {
        return Ok(vec![0]);
    }
    let last = total_frame_count - 1;
    Ok((0..sampled)
        .map(|index| {
            let position = index as f64 * last as f64 / (sampled - 1) as f64;
            position.round_ties_even() as usize
        })
        .collect())
}

fn preprocess_admitted_frames(
    media_kind: Qwen38VisionMediaKind,
    attachment_id: String,
    source_mime_type: String,
    source_frames: &[Qwen38RgbFrame],
    padded_frames: &[&Qwen38RgbFrame],
    sampled_frame_indices: Vec<usize>,
    limits: Qwen38VisionAdmissionLimits,
) -> Result<Qwen38VisionPreprocessedInput, Qwen38VisionError> {
    let first = padded_frames
        .first()
        .ok_or(Qwen38VisionError::EmptyDimensions)?;
    let grid_t = match media_kind {
        Qwen38VisionMediaKind::Image => 1,
        Qwen38VisionMediaKind::Video => padded_frames.len() / QWEN38_VISION_TEMPORAL_PATCH_SIZE,
    };
    let grid_h = first.height / QWEN38_VISION_PATCH_SIZE;
    let grid_w = first.width / QWEN38_VISION_PATCH_SIZE;
    let patch_count = grid_t
        .checked_mul(grid_h)
        .and_then(|count| count.checked_mul(grid_w))
        .ok_or(Qwen38VisionError::SizeOverflow)?;
    if patch_count > limits.maximum_total_patch_count {
        return Err(Qwen38VisionError::PatchCountExceeded {
            maximum: limits.maximum_total_patch_count,
            actual: patch_count,
        });
    }
    let mut pixel_values = Vec::with_capacity(
        patch_count
            .checked_mul(QWEN38_VISION_PATCH_VECTOR_SIZE)
            .ok_or(Qwen38VisionError::SizeOverflow)?,
    );
    for temporal in 0..grid_t {
        for block_row in 0..(grid_h / QWEN38_VISION_SPATIAL_MERGE_SIZE) {
            for block_column in 0..(grid_w / QWEN38_VISION_SPATIAL_MERGE_SIZE) {
                for merge_row in 0..QWEN38_VISION_SPATIAL_MERGE_SIZE {
                    for merge_column in 0..QWEN38_VISION_SPATIAL_MERGE_SIZE {
                        for channel in 0..3 {
                            for temporal_inner in 0..QWEN38_VISION_TEMPORAL_PATCH_SIZE {
                                let frame = padded_frames
                                    [temporal * QWEN38_VISION_TEMPORAL_PATCH_SIZE + temporal_inner];
                                for patch_row in 0..QWEN38_VISION_PATCH_SIZE {
                                    for patch_column in 0..QWEN38_VISION_PATCH_SIZE {
                                        let y = (block_row * QWEN38_VISION_SPATIAL_MERGE_SIZE
                                            + merge_row)
                                            * QWEN38_VISION_PATCH_SIZE
                                            + patch_row;
                                        let x = (block_column * QWEN38_VISION_SPATIAL_MERGE_SIZE
                                            + merge_column)
                                            * QWEN38_VISION_PATCH_SIZE
                                            + patch_column;
                                        let value = frame.rgb8[(y * frame.width + x) * 3 + channel];
                                        let normalized = (f64::from(value) / 255.0 - 0.5) / 0.5;
                                        pixel_values.push(normalized as f32);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let source_bytes = source_frames.iter().map(|frame| frame.rgb8.len()).sum();
    let source_sha256 = rgb_frames_sha256(source_frames);
    let pixel_values_sha256 = f32_values_sha256(pixel_values.as_slice());
    let merged_token_count =
        patch_count / (QWEN38_VISION_SPATIAL_MERGE_SIZE * QWEN38_VISION_SPATIAL_MERGE_SIZE);
    let (processor_name, processor_config_sha256) = match media_kind {
        Qwen38VisionMediaKind::Image => (
            String::from("Qwen2VLImageProcessor"),
            String::from(QWEN38_IMAGE_PROCESSOR_SHA256),
        ),
        Qwen38VisionMediaKind::Video => (
            String::from("Qwen3VLVideoProcessor"),
            String::from(QWEN38_VIDEO_PROCESSOR_SHA256),
        ),
    };
    Ok(Qwen38VisionPreprocessedInput {
        pixel_values,
        receipt: Qwen38VisionPreprocessingReceipt {
            schema_version: String::from(QWEN38_VISION_PREPROCESSING_SCHEMA_VERSION),
            media_kind,
            attachment_id,
            source_mime_type,
            source_sha256,
            source_bytes,
            source_frame_count: source_frames.len(),
            sampled_frame_indices,
            padded_frame_count: padded_frames.len(),
            width: first.width,
            height: first.height,
            grid_thw: [grid_t, grid_h, grid_w],
            patch_vector_size: QWEN38_VISION_PATCH_VECTOR_SIZE,
            patch_count,
            merged_token_count,
            pixel_values_dtype: String::from("f32"),
            pixel_values_sha256,
            processor_name,
            processor_config_sha256,
            resize_applied: false,
            normalization_mean: [0.5, 0.5, 0.5],
            normalization_std: [0.5, 0.5, 0.5],
            limits,
        },
    })
}

pub fn validate_qwen38_vision_preprocessed_input(
    input: &Qwen38VisionPreprocessedInput,
) -> Result<(), Qwen38VisionError> {
    let receipt = &input.receipt;
    let invalid = |detail: &str| Qwen38VisionError::InvalidPreprocessingReceipt {
        detail: String::from(detail),
    };
    if receipt.schema_version != QWEN38_VISION_PREPROCESSING_SCHEMA_VERSION {
        return Err(invalid("schema version does not match"));
    }
    if receipt.attachment_id.is_empty() || receipt.source_mime_type.is_empty() {
        return Err(invalid("attachment identity or MIME type is empty"));
    }
    if receipt.source_sha256.len() != 64
        || !receipt
            .source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid("source SHA-256 is not a 64-character hex digest"));
    }
    if receipt.source_bytes > receipt.limits.maximum_attachment_bytes {
        return Err(invalid("source bytes exceed the admitted attachment bound"));
    }
    if receipt.pixel_values_dtype != "f32" {
        return Err(invalid("pixel tensor dtype is not f32"));
    }
    if receipt.resize_applied
        || receipt.limits.resize_policy != "refuse_when_upstream_resize_is_required"
    {
        return Err(invalid(
            "resize policy does not match the bounded no-resize lane",
        ));
    }
    if receipt.normalization_mean != [0.5, 0.5, 0.5] || receipt.normalization_std != [0.5, 0.5, 0.5]
    {
        return Err(invalid("normalization constants do not match"));
    }
    if receipt.limits.timeout_ms == 0 {
        return Err(invalid("timeout must be positive"));
    }
    if receipt.width == 0 || receipt.height == 0 {
        return Err(invalid("dimensions must be nonzero"));
    }
    let (longest, shortest) = if receipt.width > receipt.height {
        (receipt.width, receipt.height)
    } else {
        (receipt.height, receipt.width)
    };
    if longest > shortest.saturating_mul(200) {
        return Err(invalid("aspect ratio exceeds 200:1"));
    }
    let dimension_factor = QWEN38_VISION_PATCH_SIZE * QWEN38_VISION_SPATIAL_MERGE_SIZE;
    if receipt.width % dimension_factor != 0 || receipt.height % dimension_factor != 0 {
        return Err(invalid("dimensions require an unrecorded resize"));
    }
    let expected_grid_h = receipt.height / QWEN38_VISION_PATCH_SIZE;
    let expected_grid_w = receipt.width / QWEN38_VISION_PATCH_SIZE;
    if receipt.grid_thw[1] != expected_grid_h || receipt.grid_thw[2] != expected_grid_w {
        return Err(invalid("spatial grid does not match dimensions"));
    }

    let (expected_grid_t, expected_processor, expected_processor_sha256) = match receipt.media_kind
    {
        Qwen38VisionMediaKind::Image => {
            let pixels = receipt
                .width
                .checked_mul(receipt.height)
                .ok_or(Qwen38VisionError::SizeOverflow)?;
            if pixels < receipt.limits.minimum_image_pixels
                || pixels > receipt.limits.maximum_image_pixels
            {
                return Err(invalid("image pixel count is outside the admitted bounds"));
            }
            if receipt.source_frame_count != 1
                || receipt.sampled_frame_indices != [0]
                || receipt.padded_frame_count != QWEN38_VISION_TEMPORAL_PATCH_SIZE
            {
                return Err(invalid("image frame accounting does not match"));
            }
            (1, "Qwen2VLImageProcessor", QWEN38_IMAGE_PROCESSOR_SHA256)
        }
        Qwen38VisionMediaKind::Video => {
            let sampled_count = receipt.sampled_frame_indices.len();
            if sampled_count < receipt.limits.minimum_video_frames
                || sampled_count > receipt.limits.maximum_video_frames
                || receipt.source_frame_count < sampled_count
                || receipt
                    .sampled_frame_indices
                    .iter()
                    .any(|index| *index >= receipt.source_frame_count)
                || receipt
                    .sampled_frame_indices
                    .windows(2)
                    .any(|window| window[0] >= window[1])
            {
                return Err(invalid(
                    "video frame sampling does not match the admitted bounds",
                ));
            }
            let expected_padded =
                sampled_count + usize::from(sampled_count % QWEN38_VISION_TEMPORAL_PATCH_SIZE != 0);
            if receipt.padded_frame_count != expected_padded {
                return Err(invalid("video temporal padding does not match"));
            }
            let pixels = receipt
                .width
                .checked_mul(receipt.height)
                .ok_or(Qwen38VisionError::SizeOverflow)?;
            if pixels > receipt.limits.maximum_video_frame_pixels {
                return Err(invalid("video frame pixels exceed the admitted bound"));
            }
            (
                expected_padded / QWEN38_VISION_TEMPORAL_PATCH_SIZE,
                "Qwen3VLVideoProcessor",
                QWEN38_VIDEO_PROCESSOR_SHA256,
            )
        }
    };
    if receipt.grid_thw[0] != expected_grid_t {
        return Err(invalid("temporal grid does not match frame accounting"));
    }
    if receipt.processor_name != expected_processor
        || receipt.processor_config_sha256 != expected_processor_sha256
    {
        return Err(invalid("processor identity does not match the media kind"));
    }
    if receipt.patch_vector_size != QWEN38_VISION_PATCH_VECTOR_SIZE {
        return Err(invalid("patch vector width does not match"));
    }
    let expected_patch_count = receipt
        .grid_thw
        .iter()
        .try_fold(1usize, |count, dimension| {
            count
                .checked_mul(*dimension)
                .ok_or(Qwen38VisionError::SizeOverflow)
        })?;
    if receipt.patch_count != expected_patch_count
        || receipt.patch_count > receipt.limits.maximum_total_patch_count
    {
        return Err(invalid(
            "patch count does not match the grid or admitted bound",
        ));
    }
    let expected_merged_token_count =
        receipt.patch_count / (QWEN38_VISION_SPATIAL_MERGE_SIZE * QWEN38_VISION_SPATIAL_MERGE_SIZE);
    if receipt.merged_token_count != expected_merged_token_count {
        return Err(invalid("merged token count does not match"));
    }
    let expected_values = receipt
        .patch_count
        .checked_mul(receipt.patch_vector_size)
        .ok_or(Qwen38VisionError::SizeOverflow)?;
    if input.pixel_values.len() != expected_values {
        return Err(invalid("pixel tensor length does not match"));
    }
    if receipt.pixel_values_sha256 != f32_values_sha256(input.pixel_values.as_slice()) {
        return Err(invalid("pixel tensor digest does not match"));
    }
    Ok(())
}

fn validate_common_frame(frame: &Qwen38RgbFrame) -> Result<(), Qwen38VisionError> {
    if frame.width == 0 || frame.height == 0 {
        return Err(Qwen38VisionError::EmptyDimensions);
    }
    let expected = frame
        .width
        .checked_mul(frame.height)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or(Qwen38VisionError::SizeOverflow)?;
    if frame.rgb8.len() != expected {
        return Err(Qwen38VisionError::InvalidRgbByteLength {
            expected,
            actual: frame.rgb8.len(),
        });
    }
    let (longest, shortest) = if frame.width > frame.height {
        (frame.width, frame.height)
    } else {
        (frame.height, frame.width)
    };
    if longest > shortest.saturating_mul(200) {
        return Err(Qwen38VisionError::AspectRatioExceeded);
    }
    let factor = QWEN38_VISION_PATCH_SIZE * QWEN38_VISION_SPATIAL_MERGE_SIZE;
    if frame.width % factor != 0 || frame.height % factor != 0 {
        return Err(Qwen38VisionError::ResizeRequired {
            width: frame.width,
            height: frame.height,
            factor,
        });
    }
    Ok(())
}

fn validate_attachment_bytes(
    frames: &[Qwen38RgbFrame],
    limits: &Qwen38VisionAdmissionLimits,
) -> Result<(), Qwen38VisionError> {
    let actual = frames.iter().try_fold(0usize, |bytes, frame| {
        bytes
            .checked_add(frame.rgb8.len())
            .ok_or(Qwen38VisionError::SizeOverflow)
    })?;
    if actual > limits.maximum_attachment_bytes {
        return Err(Qwen38VisionError::AttachmentBytesExceeded {
            maximum: limits.maximum_attachment_bytes,
            actual,
        });
    }
    Ok(())
}

fn file_len(path: &Path) -> Result<u64, Qwen38VisionArtifactError> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|source| Qwen38VisionArtifactError::Io {
            path: path.to_path_buf(),
            source,
        })
}

fn sha256_file(path: &Path) -> Result<String, Qwen38VisionArtifactError> {
    let file = File::open(path).map_err(|source| Qwen38VisionArtifactError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|source| Qwen38VisionArtifactError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn rgb_frames_sha256(frames: &[Qwen38RgbFrame]) -> String {
    let mut hasher = Sha256::new();
    hasher.update((frames.len() as u64).to_le_bytes());
    for frame in frames {
        hasher.update((frame.width as u64).to_le_bytes());
        hasher.update((frame.height as u64).to_le_bytes());
        hasher.update((frame.rgb8.len() as u64).to_le_bytes());
        hasher.update(frame.rgb8.as_slice());
    }
    hex::encode(hasher.finalize())
}

fn f32_values_sha256(values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_limits() -> Qwen38VisionAdmissionLimits {
        Qwen38VisionAdmissionLimits {
            minimum_image_pixels: 32 * 32,
            maximum_image_pixels: 64 * 64,
            minimum_video_frames: 2,
            maximum_video_frames: 4,
            maximum_video_frame_pixels: 32 * 32,
            maximum_total_patch_count: 16,
            maximum_attachment_bytes: 64 * 64 * 3 * 4,
            timeout_ms: 1_000,
            resize_policy: String::from("refuse_when_upstream_resize_is_required"),
        }
    }

    fn solid_frame(width: usize, height: usize, rgb: [u8; 3]) -> Qwen38RgbFrame {
        let mut bytes = Vec::with_capacity(width * height * 3);
        for _ in 0..(width * height) {
            bytes.extend_from_slice(&rgb);
        }
        Qwen38RgbFrame::new(width, height, bytes).expect("valid RGB frame")
    }

    fn observed_expected_tensors() -> Vec<Qwen35TextObservedTensorSpec> {
        qwen38_vision_expected_tensor_specs()
            .into_iter()
            .map(|tensor| Qwen35TextObservedTensorSpec {
                name: tensor.name,
                dtype: tensor.dtype,
                shape: tensor.shape,
                shard: String::from(QWEN38_VISION_SOURCE_SHARD),
            })
            .collect()
    }

    #[test]
    fn qwen38_vision_inventory_is_exact() {
        let specs = qwen38_vision_expected_tensor_specs();
        assert_eq!(specs.len(), QWEN38_VISION_TENSOR_COUNT);
        assert_eq!(
            specs
                .iter()
                .map(Qwen38VisionTensorSpec::byte_len)
                .sum::<u64>(),
            QWEN38_VISION_TENSOR_BYTES
        );
    }

    #[test]
    fn qwen38_vision_artifact_admits_only_the_pinned_identity_and_inventory() {
        let observed = observed_expected_tensors();
        let report = qwen38_vision_artifact_admission_from_observed(
            QWEN38_VISION_SOURCE_SHARD_BYTES,
            String::from(QWEN38_VISION_SOURCE_SHARD_SHA256),
            String::from(QWEN38_IMAGE_PROCESSOR_SHA256),
            String::from(QWEN38_VIDEO_PROCESSOR_SHA256),
            observed.as_slice(),
        );
        assert_eq!(report.status, Qwen38VisionArtifactAdmissionStatus::Admitted);
        assert_eq!(report.observed_tensor_count, 333);
        assert_eq!(report.observed_tensor_bytes, QWEN38_VISION_TENSOR_BYTES);
        assert_eq!(report.non_vision_tensor_count_in_source_shard, 0);
        assert!(!report.extraction_required);

        let mut drifted = observed;
        drifted[0].shape = vec![1];
        let report = qwen38_vision_artifact_admission_from_observed(
            QWEN38_VISION_SOURCE_SHARD_BYTES,
            String::from(QWEN38_VISION_SOURCE_SHARD_SHA256),
            String::from(QWEN38_IMAGE_PROCESSOR_SHA256),
            String::from(QWEN38_VIDEO_PROCESSOR_SHA256),
            drifted.as_slice(),
        );
        assert_eq!(report.status, Qwen38VisionArtifactAdmissionStatus::Refused);
        assert_eq!(
            report.refusal_code,
            Some(Qwen38VisionArtifactRefusalCode::TensorInventoryMismatch)
        );
    }

    #[test]
    fn qwen38_image_preprocessing_matches_patch_order_and_normalization() {
        let frame = solid_frame(32, 32, [0, 128, 255]);
        let output = qwen38_preprocess_image("image-1", "image/raw-rgb8", &frame, test_limits())
            .expect("preprocess image");
        assert_eq!(output.receipt.grid_thw, [1, 2, 2]);
        assert_eq!(output.receipt.patch_count, 4);
        assert_eq!(output.receipt.merged_token_count, 1);
        assert_eq!(output.pixel_values.len(), 4 * 1_536);
        assert_eq!(&output.pixel_values[0..256], vec![-1.0; 256].as_slice());
        assert_eq!(&output.pixel_values[256..512], vec![-1.0; 256].as_slice());
        assert!(
            output.pixel_values[512..768]
                .iter()
                .all(|value| (*value - (1.0 / 255.0)).abs() < 1e-7)
        );
        assert!(
            output.pixel_values[768..1_024]
                .iter()
                .all(|value| (*value - (1.0 / 255.0)).abs() < 1e-7)
        );
        assert_eq!(
            &output.pixel_values[1_024..1_280],
            vec![1.0; 256].as_slice()
        );
        assert!(!output.receipt.resize_applied);
        validate_qwen38_vision_preprocessed_input(&output).expect("valid preprocessing receipt");

        let mut tampered = output;
        tampered.pixel_values[0] = 0.0;
        assert!(matches!(
            validate_qwen38_vision_preprocessed_input(&tampered),
            Err(Qwen38VisionError::InvalidPreprocessingReceipt { .. })
        ));
    }

    #[test]
    fn qwen38_video_preprocessing_records_sampling_and_temporal_patch_order() {
        let frames = vec![
            solid_frame(32, 32, [0, 0, 0]),
            solid_frame(32, 32, [64, 64, 64]),
            solid_frame(32, 32, [128, 128, 128]),
            solid_frame(32, 32, [255, 255, 255]),
        ];
        let output = qwen38_preprocess_video(
            "video-1",
            "video/raw-rgb8-frames",
            frames.as_slice(),
            2.0,
            test_limits(),
        )
        .expect("preprocess video");
        assert_eq!(output.receipt.sampled_frame_indices, vec![0, 1, 2, 3]);
        assert_eq!(output.receipt.grid_thw, [2, 2, 2]);
        assert_eq!(output.receipt.padded_frame_count, 4);
        assert_eq!(&output.pixel_values[0..256], vec![-1.0; 256].as_slice());
        assert!(
            output.pixel_values[256..512]
                .iter()
                .all(|value| (*value - (-127.0 / 255.0)).abs() < 1e-7)
        );
    }

    #[test]
    fn qwen38_preprocessing_refuses_resize_and_bound_violations() {
        let bad_shape = solid_frame(32, 48, [0, 0, 0]);
        assert_eq!(
            qwen38_preprocess_image("bad", "image/raw-rgb8", &bad_shape, test_limits()),
            Err(Qwen38VisionError::ResizeRequired {
                width: 32,
                height: 48,
                factor: 32,
            })
        );

        let bad_aspect = solid_frame(32, 32 * 201, [0, 0, 0]);
        assert_eq!(
            qwen38_preprocess_image("bad-aspect", "image/raw-rgb8", &bad_aspect, test_limits()),
            Err(Qwen38VisionError::AspectRatioExceeded)
        );

        let too_few_frames = vec![solid_frame(32, 32, [0, 0, 0])];
        assert_eq!(
            qwen38_preprocess_video(
                "bad-video",
                "video/raw-rgb8-frames",
                too_few_frames.as_slice(),
                2.0,
                test_limits(),
            ),
            Err(Qwen38VisionError::VideoFramesOutOfBounds {
                minimum: 2,
                maximum: 4,
                actual: 1,
            })
        );

        let mut byte_limited = test_limits();
        byte_limited.maximum_attachment_bytes = 32 * 32 * 3;
        let frames = vec![
            solid_frame(32, 32, [0, 0, 0]),
            solid_frame(32, 32, [0, 0, 0]),
        ];
        assert_eq!(
            qwen38_preprocess_video(
                "oversized-video",
                "video/raw-rgb8-frames",
                frames.as_slice(),
                2.0,
                byte_limited,
            ),
            Err(Qwen38VisionError::AttachmentBytesExceeded {
                maximum: 32 * 32 * 3,
                actual: 32 * 32 * 3 * 2,
            })
        );
    }

    #[test]
    fn qwen38_real_vision_artifact_admits_when_available() {
        let Some(path) = std::env::var_os("PSIONIC_QWEN38_OFFICIAL_MODEL_DIR") else {
            return;
        };
        let report = inspect_qwen38_vision_artifact(path).expect("inspect real vision artifact");
        assert_eq!(report.status, Qwen38VisionArtifactAdmissionStatus::Admitted);
        assert_eq!(report.observed_tensor_count, 333);
        assert_eq!(report.non_vision_tensor_count_in_source_shard, 59);
    }
}
