use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    QWEN38_IMAGE_PROCESSOR_SHA256, QWEN38_VIDEO_PROCESSOR_SHA256,
    QWEN38_VISION_RUNTIME_SCHEMA_VERSION, QWEN38_VISION_SPATIAL_MERGE_SIZE, Qwen38RenderedPrompt,
    Qwen38Tokenizer, Qwen38TokenizerError, Qwen38VisionMediaKind, Qwen38VisionPreprocessedInput,
    Qwen38VisionRuntimeOutput, qwen38_vision_runtime::qwen38_vision_embeddings_sha256,
    validate_qwen38_vision_preprocessed_input,
};

pub const QWEN38_MULTIMODAL_DECODER_PLAN_SCHEMA_VERSION: &str =
    "psionic.qwen38.multimodal_decoder_plan.v1";
pub const QWEN38_VISION_START_TOKEN_ID: u32 = 248_053;
pub const QWEN38_VISION_END_TOKEN_ID: u32 = 248_054;
pub const QWEN38_VISION_PAD_TOKEN_ID: u32 = 248_055;
pub const QWEN38_IMAGE_PAD_TOKEN_ID: u32 = 248_056;
pub const QWEN38_VIDEO_PAD_TOKEN_ID: u32 = 248_057;
pub const QWEN38_DECODER_HIDDEN_SIZE: usize = 5_120;

const IMAGE_PAD_MARKER: &str = "<|image_pad|>";
const VIDEO_PAD_MARKER: &str = "<|video_pad|>";
const VISION_START_MARKER: &str = "<|vision_start|>";
const VISION_END_MARKER: &str = "<|vision_end|>";

#[derive(Clone, Debug, PartialEq)]
pub struct Qwen38DecoderMediaInput {
    pub preprocessing: Qwen38VisionPreprocessedInput,
    pub runtime: Qwen38VisionRuntimeOutput,
    pub source_fps: Option<f64>,
}

impl Qwen38DecoderMediaInput {
    #[must_use]
    pub fn image(
        preprocessing: Qwen38VisionPreprocessedInput,
        runtime: Qwen38VisionRuntimeOutput,
    ) -> Self {
        Self {
            preprocessing,
            runtime,
            source_fps: None,
        }
    }

    #[must_use]
    pub fn video(
        preprocessing: Qwen38VisionPreprocessedInput,
        runtime: Qwen38VisionRuntimeOutput,
        source_fps: f64,
    ) -> Self {
        Self {
            preprocessing,
            runtime,
            source_fps: Some(source_fps),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Qwen38DecoderEmbeddingOverride {
    pub token_index: usize,
    pub media_kind: Qwen38VisionMediaKind,
    pub modality_embedding_index: usize,
    pub embedding: Vec<f32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38MultimodalDecoderPlanReceipt {
    pub schema_version: String,
    pub rendered_prompt_sha256: String,
    pub expanded_prompt_sha256: String,
    pub token_ids_sha256: String,
    pub token_count: usize,
    pub text_token_count: usize,
    pub image_count: usize,
    pub image_token_count: usize,
    pub video_count: usize,
    pub video_token_count: usize,
    pub embedding_override_count: usize,
    pub embedding_width: usize,
    pub mrope_position_delta: i64,
    pub preprocessing_receipt_sha256: Vec<String>,
    pub vision_runtime_output_sha256: Vec<String>,
    pub hidden_fallback_used: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Qwen38MultimodalDecoderPlan {
    expanded_prompt: String,
    token_ids: Vec<u32>,
    mm_token_type_ids: Vec<u8>,
    mrope_position_ids: Vec<[usize; 3]>,
    mrope_position_delta: i64,
    embedding_overrides: Vec<Qwen38DecoderEmbeddingOverride>,
    receipt: Qwen38MultimodalDecoderPlanReceipt,
}

impl Qwen38MultimodalDecoderPlan {
    #[must_use]
    pub fn expanded_prompt(&self) -> &str {
        self.expanded_prompt.as_str()
    }

    #[must_use]
    pub fn token_ids(&self) -> &[u32] {
        self.token_ids.as_slice()
    }

    #[must_use]
    pub fn mm_token_type_ids(&self) -> &[u8] {
        self.mm_token_type_ids.as_slice()
    }

    #[must_use]
    pub fn mrope_position_ids(&self) -> &[[usize; 3]] {
        self.mrope_position_ids.as_slice()
    }

    #[must_use]
    pub fn embedding_overrides(&self) -> &[Qwen38DecoderEmbeddingOverride] {
        self.embedding_overrides.as_slice()
    }

    #[must_use]
    pub const fn receipt(&self) -> &Qwen38MultimodalDecoderPlanReceipt {
        &self.receipt
    }

    pub fn generated_position(
        &self,
        physical_cache_position: usize,
    ) -> Result<[usize; 3], Qwen38MultimodalDecoderPlanError> {
        let position = i64::try_from(physical_cache_position)
            .unwrap_or(i64::MAX)
            .checked_add(self.mrope_position_delta)
            .ok_or(Qwen38MultimodalDecoderPlanError::PositionOverflow)?;
        let position = usize::try_from(position)
            .map_err(|_| Qwen38MultimodalDecoderPlanError::PositionOverflow)?;
        Ok([position; 3])
    }
}

#[derive(Debug, Error)]
pub enum Qwen38MultimodalDecoderPlanError {
    #[error(transparent)]
    Tokenizer(#[from] Qwen38TokenizerError),
    #[error(
        "Qwen3.8 rendered prompt media marker {marker_index} expects {expected:?}, found {actual:?}"
    )]
    MediaOrderMismatch {
        marker_index: usize,
        expected: Qwen38VisionMediaKind,
        actual: Qwen38VisionMediaKind,
    },
    #[error(
        "Qwen3.8 rendered prompt contains {actual} media markers, but {expected} media inputs were supplied"
    )]
    MediaCountMismatch { expected: usize, actual: usize },
    #[error("invalid Qwen3.8 decoder media input {media_index}: {detail}")]
    InvalidMediaInput { media_index: usize, detail: String },
    #[error("Qwen3.8 {media_kind:?} token count mismatch: expected {expected}, found {actual}")]
    ModalityTokenCountMismatch {
        media_kind: Qwen38VisionMediaKind,
        expected: usize,
        actual: usize,
    },
    #[error(
        "Qwen3.8 {media_kind:?} MRoPE group {group_index} has {actual} tokens; expected {expected}"
    )]
    MropeGroupSizeMismatch {
        media_kind: Qwen38VisionMediaKind,
        group_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error("Qwen3.8 MRoPE grid accounting did not consume every {0:?} grid")]
    UnconsumedMropeGrid(Qwen38VisionMediaKind),
    #[error("Qwen3.8 multimodal decoder position arithmetic overflowed")]
    PositionOverflow,
    #[error("failed to serialize Qwen3.8 multimodal decoder evidence: {0}")]
    Serialization(String),
}

pub fn build_qwen38_multimodal_decoder_plan(
    prompt: &Qwen38RenderedPrompt,
    tokenizer: &Qwen38Tokenizer,
    media: &[Qwen38DecoderMediaInput],
) -> Result<Qwen38MultimodalDecoderPlan, Qwen38MultimodalDecoderPlanError> {
    for (media_index, input) in media.iter().enumerate() {
        validate_decoder_media_input(media_index, input)?;
    }
    let expanded_prompt = expand_media_markers(prompt.text.as_str(), media)?;
    let token_ids = tokenizer.encode_text(expanded_prompt.as_str())?;
    build_qwen38_multimodal_decoder_plan_from_token_ids(prompt, expanded_prompt, token_ids, media)
}

pub fn build_qwen38_multimodal_decoder_plan_from_token_ids(
    prompt: &Qwen38RenderedPrompt,
    expanded_prompt: String,
    token_ids: Vec<u32>,
    media: &[Qwen38DecoderMediaInput],
) -> Result<Qwen38MultimodalDecoderPlan, Qwen38MultimodalDecoderPlanError> {
    for (media_index, input) in media.iter().enumerate() {
        validate_decoder_media_input(media_index, input)?;
    }
    let mm_token_type_ids = token_ids
        .iter()
        .map(|token| match *token {
            QWEN38_IMAGE_PAD_TOKEN_ID => 1,
            QWEN38_VIDEO_PAD_TOKEN_ID => 2,
            _ => 0,
        })
        .collect::<Vec<_>>();

    let image_positions = modality_positions(&token_ids, QWEN38_IMAGE_PAD_TOKEN_ID);
    let video_positions = modality_positions(&token_ids, QWEN38_VIDEO_PAD_TOKEN_ID);
    let image_embeddings = modality_embeddings(media, Qwen38VisionMediaKind::Image);
    let video_embeddings = modality_embeddings(media, Qwen38VisionMediaKind::Video);
    validate_modality_count(
        Qwen38VisionMediaKind::Image,
        image_embeddings.len(),
        image_positions.len(),
    )?;
    validate_modality_count(
        Qwen38VisionMediaKind::Video,
        video_embeddings.len(),
        video_positions.len(),
    )?;

    let mut embedding_overrides =
        Vec::with_capacity(image_positions.len().saturating_add(video_positions.len()));
    embedding_overrides.extend(
        image_positions
            .into_iter()
            .zip(image_embeddings)
            .enumerate()
            .map(|(modality_embedding_index, (token_index, embedding))| {
                Qwen38DecoderEmbeddingOverride {
                    token_index,
                    media_kind: Qwen38VisionMediaKind::Image,
                    modality_embedding_index,
                    embedding,
                }
            }),
    );
    embedding_overrides.extend(
        video_positions
            .into_iter()
            .zip(video_embeddings)
            .enumerate()
            .map(|(modality_embedding_index, (token_index, embedding))| {
                Qwen38DecoderEmbeddingOverride {
                    token_index,
                    media_kind: Qwen38VisionMediaKind::Video,
                    modality_embedding_index,
                    embedding,
                }
            }),
    );
    embedding_overrides.sort_by_key(|value| value.token_index);

    let (mrope_position_ids, mrope_position_delta) =
        build_mrope_positions(mm_token_type_ids.as_slice(), media)?;
    let image_count = media
        .iter()
        .filter(|input| input.preprocessing.receipt.media_kind == Qwen38VisionMediaKind::Image)
        .count();
    let video_count = media.len().saturating_sub(image_count);
    let image_token_count = mm_token_type_ids.iter().filter(|kind| **kind == 1).count();
    let video_token_count = mm_token_type_ids.iter().filter(|kind| **kind == 2).count();
    let preprocessing_receipt_sha256 = media
        .iter()
        .map(|input| sha256_json(&input.preprocessing.receipt))
        .collect::<Result<Vec<_>, _>>()?;
    let vision_runtime_output_sha256 = media
        .iter()
        .map(|input| input.runtime.receipt.output_sha256.clone())
        .collect::<Vec<_>>();
    let receipt = Qwen38MultimodalDecoderPlanReceipt {
        schema_version: String::from(QWEN38_MULTIMODAL_DECODER_PLAN_SCHEMA_VERSION),
        rendered_prompt_sha256: sha256_bytes(prompt.text.as_bytes()),
        expanded_prompt_sha256: sha256_bytes(expanded_prompt.as_bytes()),
        token_ids_sha256: sha256_u32(token_ids.as_slice()),
        token_count: token_ids.len(),
        text_token_count: token_ids
            .len()
            .saturating_sub(image_token_count)
            .saturating_sub(video_token_count),
        image_count,
        image_token_count,
        video_count,
        video_token_count,
        embedding_override_count: embedding_overrides.len(),
        embedding_width: QWEN38_DECODER_HIDDEN_SIZE,
        mrope_position_delta,
        preprocessing_receipt_sha256,
        vision_runtime_output_sha256,
        hidden_fallback_used: false,
    };
    Ok(Qwen38MultimodalDecoderPlan {
        expanded_prompt,
        token_ids,
        mm_token_type_ids,
        mrope_position_ids,
        mrope_position_delta,
        embedding_overrides,
        receipt,
    })
}

fn validate_decoder_media_input(
    media_index: usize,
    input: &Qwen38DecoderMediaInput,
) -> Result<(), Qwen38MultimodalDecoderPlanError> {
    let invalid = |detail: String| Qwen38MultimodalDecoderPlanError::InvalidMediaInput {
        media_index,
        detail,
    };
    validate_qwen38_vision_preprocessed_input(&input.preprocessing)
        .map_err(|error| invalid(error.to_string()))?;
    let preprocessing = &input.preprocessing.receipt;
    let runtime = &input.runtime;
    if runtime.receipt.schema_version != QWEN38_VISION_RUNTIME_SCHEMA_VERSION {
        return Err(invalid(String::from(
            "runtime schema version does not match",
        )));
    }
    if runtime.receipt.hidden_fallback_used
        || !runtime.receipt.full_stack_resident
        || runtime.receipt.resident_layer_count != runtime.receipt.expected_layer_count
        || runtime.receipt.expected_layer_count != 27
    {
        return Err(invalid(String::from(
            "vision runtime receipt does not prove full native execution",
        )));
    }
    if runtime.receipt.output_token_count != preprocessing.merged_token_count
        || runtime.embeddings.len() != preprocessing.merged_token_count
        || runtime.receipt.input_patch_count != preprocessing.patch_count
    {
        return Err(invalid(String::from(
            "vision output token count does not match preprocessing",
        )));
    }
    if runtime.receipt.output_width != QWEN38_DECODER_HIDDEN_SIZE
        || runtime
            .embeddings
            .iter()
            .any(|embedding| embedding.len() != QWEN38_DECODER_HIDDEN_SIZE)
    {
        return Err(invalid(format!(
            "vision output width must be {QWEN38_DECODER_HIDDEN_SIZE}",
        )));
    }
    if qwen38_vision_embeddings_sha256(runtime.embeddings.as_slice())
        != runtime.receipt.output_sha256
    {
        return Err(invalid(String::from(
            "vision output digest does not match materialized embeddings",
        )));
    }
    let expected_processor_sha256 = match preprocessing.media_kind {
        Qwen38VisionMediaKind::Image => QWEN38_IMAGE_PROCESSOR_SHA256,
        Qwen38VisionMediaKind::Video => QWEN38_VIDEO_PROCESSOR_SHA256,
    };
    if preprocessing.processor_config_sha256 != expected_processor_sha256 {
        return Err(invalid(String::from(
            "preprocessing processor identity does not match the media kind",
        )));
    }
    match (preprocessing.media_kind, input.source_fps) {
        (Qwen38VisionMediaKind::Image, None) => {}
        (Qwen38VisionMediaKind::Video, Some(fps)) if fps.is_finite() && fps > 0.0 => {}
        (Qwen38VisionMediaKind::Image, Some(_)) => {
            return Err(invalid(String::from(
                "image media must not declare source fps",
            )));
        }
        (Qwen38VisionMediaKind::Video, _) => {
            return Err(invalid(String::from(
                "video media requires a finite positive source fps",
            )));
        }
    }
    Ok(())
}

fn expand_media_markers(
    prompt: &str,
    media: &[Qwen38DecoderMediaInput],
) -> Result<String, Qwen38MultimodalDecoderPlanError> {
    let mut expanded = String::with_capacity(prompt.len());
    let mut remaining = prompt;
    for (marker_index, input) in media.iter().enumerate() {
        let next_image = remaining.find(IMAGE_PAD_MARKER);
        let next_video = remaining.find(VIDEO_PAD_MARKER);
        let (offset, actual, marker) = match (next_image, next_video) {
            (Some(image), Some(video)) if image < video => {
                (image, Qwen38VisionMediaKind::Image, IMAGE_PAD_MARKER)
            }
            (Some(_), Some(video)) => (video, Qwen38VisionMediaKind::Video, VIDEO_PAD_MARKER),
            (Some(image), None) => (image, Qwen38VisionMediaKind::Image, IMAGE_PAD_MARKER),
            (None, Some(video)) => (video, Qwen38VisionMediaKind::Video, VIDEO_PAD_MARKER),
            (None, None) => {
                return Err(Qwen38MultimodalDecoderPlanError::MediaCountMismatch {
                    expected: media.len(),
                    actual: marker_index,
                });
            }
        };
        let expected = input.preprocessing.receipt.media_kind;
        if expected != actual {
            return Err(Qwen38MultimodalDecoderPlanError::MediaOrderMismatch {
                marker_index,
                expected,
                actual,
            });
        }
        expanded.push_str(&remaining[..offset]);
        match actual {
            Qwen38VisionMediaKind::Image => {
                for _ in 0..input.preprocessing.receipt.merged_token_count {
                    expanded.push_str(IMAGE_PAD_MARKER);
                }
            }
            Qwen38VisionMediaKind::Video => expand_video_marker(&mut expanded, input),
        }
        remaining = &remaining[offset + marker.len()..];
    }
    let extra_markers =
        remaining.matches(IMAGE_PAD_MARKER).count() + remaining.matches(VIDEO_PAD_MARKER).count();
    if extra_markers != 0 {
        return Err(Qwen38MultimodalDecoderPlanError::MediaCountMismatch {
            expected: media.len(),
            actual: media.len().saturating_add(extra_markers),
        });
    }
    expanded.push_str(remaining);
    Ok(expanded)
}

fn expand_video_marker(expanded: &mut String, input: &Qwen38DecoderMediaInput) {
    let receipt = &input.preprocessing.receipt;
    let fps = input.source_fps.expect("validated video fps");
    let mut indices = receipt.sampled_frame_indices.clone();
    if indices.len() % 2 != 0 {
        indices.push(*indices.last().expect("validated video frame indices"));
    }
    let frame_token_count = receipt.grid_thw[1].saturating_mul(receipt.grid_thw[2])
        / QWEN38_VISION_SPATIAL_MERGE_SIZE.pow(2);
    for pair in indices.chunks_exact(2) {
        let timestamp = (pair[0] as f64 / fps + pair[1] as f64 / fps) / 2.0;
        expanded.push_str(format!("<{timestamp:.1} seconds>").as_str());
        expanded.push_str(VISION_START_MARKER);
        for _ in 0..frame_token_count {
            expanded.push_str(VIDEO_PAD_MARKER);
        }
        expanded.push_str(VISION_END_MARKER);
    }
}

fn modality_positions(token_ids: &[u32], token_id: u32) -> Vec<usize> {
    token_ids
        .iter()
        .enumerate()
        .filter_map(|(index, token)| (*token == token_id).then_some(index))
        .collect()
}

fn modality_embeddings(
    media: &[Qwen38DecoderMediaInput],
    media_kind: Qwen38VisionMediaKind,
) -> Vec<Vec<f32>> {
    media
        .iter()
        .filter(|input| input.preprocessing.receipt.media_kind == media_kind)
        .flat_map(|input| input.runtime.embeddings.iter().cloned())
        .collect()
}

fn validate_modality_count(
    media_kind: Qwen38VisionMediaKind,
    expected: usize,
    actual: usize,
) -> Result<(), Qwen38MultimodalDecoderPlanError> {
    if expected == actual {
        return Ok(());
    }
    Err(
        Qwen38MultimodalDecoderPlanError::ModalityTokenCountMismatch {
            media_kind,
            expected,
            actual,
        },
    )
}

fn build_mrope_positions(
    mm_token_type_ids: &[u8],
    media: &[Qwen38DecoderMediaInput],
) -> Result<(Vec<[usize; 3]>, i64), Qwen38MultimodalDecoderPlanError> {
    let image_grids = media
        .iter()
        .filter(|input| input.preprocessing.receipt.media_kind == Qwen38VisionMediaKind::Image)
        .map(|input| input.preprocessing.receipt.grid_thw)
        .collect::<Vec<_>>();
    let video_grids = media
        .iter()
        .filter(|input| input.preprocessing.receipt.media_kind == Qwen38VisionMediaKind::Video)
        .flat_map(|input| {
            let [grid_t, grid_h, grid_w] = input.preprocessing.receipt.grid_thw;
            std::iter::repeat_n([1, grid_h, grid_w], grid_t)
        })
        .collect::<Vec<_>>();
    let mut image_grid_index = 0usize;
    let mut video_grid_index = 0usize;
    let mut group_index = [0usize; 3];
    let mut current_pos = 0usize;
    let mut positions = Vec::with_capacity(mm_token_type_ids.len());
    let mut start = 0usize;
    while start < mm_token_type_ids.len() {
        let modality = mm_token_type_ids[start];
        let end = mm_token_type_ids[start..]
            .iter()
            .position(|candidate| *candidate != modality)
            .map_or(mm_token_type_ids.len(), |offset| start + offset);
        let group_len = end - start;
        if modality == 0 {
            for position in current_pos..current_pos.saturating_add(group_len) {
                positions.push([position; 3]);
            }
            current_pos = current_pos
                .checked_add(group_len)
                .ok_or(Qwen38MultimodalDecoderPlanError::PositionOverflow)?;
        } else {
            let (media_kind, grids, grid_index) = if modality == 1 {
                (
                    Qwen38VisionMediaKind::Image,
                    image_grids.as_slice(),
                    &mut image_grid_index,
                )
            } else {
                (
                    Qwen38VisionMediaKind::Video,
                    video_grids.as_slice(),
                    &mut video_grid_index,
                )
            };
            let grid = grids.get(*grid_index).copied().ok_or(
                Qwen38MultimodalDecoderPlanError::MropeGroupSizeMismatch {
                    media_kind,
                    group_index: group_index[modality as usize],
                    expected: 0,
                    actual: group_len,
                },
            )?;
            let [grid_t, grid_h, grid_w] = grid;
            let llm_grid_h = grid_h / QWEN38_VISION_SPATIAL_MERGE_SIZE;
            let llm_grid_w = grid_w / QWEN38_VISION_SPATIAL_MERGE_SIZE;
            let expected = grid_t
                .checked_mul(llm_grid_h)
                .and_then(|count| count.checked_mul(llm_grid_w))
                .ok_or(Qwen38MultimodalDecoderPlanError::PositionOverflow)?;
            if group_len != expected {
                return Err(Qwen38MultimodalDecoderPlanError::MropeGroupSizeMismatch {
                    media_kind,
                    group_index: group_index[modality as usize],
                    expected,
                    actual: group_len,
                });
            }
            for temporal in 0..grid_t {
                for height in 0..llm_grid_h {
                    for width in 0..llm_grid_w {
                        positions.push([
                            current_pos.saturating_add(temporal),
                            current_pos.saturating_add(height),
                            current_pos.saturating_add(width),
                        ]);
                    }
                }
            }
            current_pos = current_pos
                .checked_add(llm_grid_h.max(llm_grid_w))
                .ok_or(Qwen38MultimodalDecoderPlanError::PositionOverflow)?;
            *grid_index += 1;
            group_index[modality as usize] += 1;
        }
        start = end;
    }
    if image_grid_index != image_grids.len() {
        return Err(Qwen38MultimodalDecoderPlanError::UnconsumedMropeGrid(
            Qwen38VisionMediaKind::Image,
        ));
    }
    if video_grid_index != video_grids.len() {
        return Err(Qwen38MultimodalDecoderPlanError::UnconsumedMropeGrid(
            Qwen38VisionMediaKind::Video,
        ));
    }
    let maximum = positions
        .iter()
        .flat_map(|position| position.iter().copied())
        .max()
        .unwrap_or(0);
    let delta = i64::try_from(maximum.saturating_add(1)).unwrap_or(i64::MAX)
        - i64::try_from(mm_token_type_ids.len()).unwrap_or(i64::MAX);
    Ok((positions, delta))
}

fn sha256_u32(values: &[u32]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_json<T: Serialize>(value: &T) -> Result<String, Qwen38MultimodalDecoderPlanError> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(bytes.as_slice()))
        .map_err(|error| Qwen38MultimodalDecoderPlanError::Serialization(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        QWEN38_VISION_PREPROCESSING_SCHEMA_VERSION, Qwen38PromptReceipt,
        Qwen38VisionAdmissionLimits, Qwen38VisionPreprocessedInput,
        Qwen38VisionPreprocessingReceipt, Qwen38VisionRuntimeReceipt,
    };

    fn prompt() -> Qwen38RenderedPrompt {
        Qwen38RenderedPrompt {
            text: String::from("synthetic"),
            receipt: Qwen38PromptReceipt {
                schema_version: String::from("test"),
                model_id: String::from("test"),
                template_id: String::from("test"),
                template_sha256: String::from("test"),
                tokenizer_sha256: String::from("test"),
                thinking_enabled: false,
                reasoning_effort: None,
                preserve_thinking: false,
                add_generation_prompt: false,
                add_vision_id: false,
                tool_count: 0,
                rendered_sha256: String::from("test"),
                prompt_cache_identity: String::from("test"),
            },
        }
    }

    fn media(kind: Qwen38VisionMediaKind, grid_thw: [usize; 3]) -> Qwen38DecoderMediaInput {
        let token_count = grid_thw.iter().product::<usize>() / 4;
        let embeddings = (0..token_count)
            .map(|index| vec![index as f32; QWEN38_DECODER_HIDDEN_SIZE])
            .collect::<Vec<_>>();
        let output_sha256 = qwen38_vision_embeddings_sha256(embeddings.as_slice());
        let patch_count = grid_thw.iter().product::<usize>();
        let pixel_values = vec![0.0_f32; patch_count * 1_536];
        let pixel_values_sha256 = f32_values_sha256(pixel_values.as_slice());
        let mut limits = Qwen38VisionAdmissionLimits::default();
        limits.minimum_image_pixels = 1;
        let receipt = Qwen38VisionPreprocessingReceipt {
            schema_version: String::from(QWEN38_VISION_PREPROCESSING_SCHEMA_VERSION),
            media_kind: kind,
            attachment_id: String::from("fixture"),
            source_mime_type: String::from("fixture/raw"),
            source_sha256: "0".repeat(64),
            source_bytes: 1,
            source_frame_count: if kind == Qwen38VisionMediaKind::Image {
                1
            } else {
                grid_thw[0] * 2
            },
            sampled_frame_indices: if kind == Qwen38VisionMediaKind::Image {
                vec![0]
            } else {
                (0..grid_thw[0] * 2).collect()
            },
            padded_frame_count: grid_thw[0] * 2,
            width: grid_thw[2] * 16,
            height: grid_thw[1] * 16,
            grid_thw,
            patch_vector_size: 1_536,
            patch_count,
            merged_token_count: token_count,
            pixel_values_dtype: String::from("f32"),
            pixel_values_sha256,
            processor_name: if kind == Qwen38VisionMediaKind::Image {
                String::from("Qwen2VLImageProcessor")
            } else {
                String::from("Qwen3VLVideoProcessor")
            },
            processor_config_sha256: String::from(if kind == Qwen38VisionMediaKind::Image {
                QWEN38_IMAGE_PROCESSOR_SHA256
            } else {
                QWEN38_VIDEO_PROCESSOR_SHA256
            }),
            resize_applied: false,
            normalization_mean: [0.5; 3],
            normalization_std: [0.5; 3],
            limits,
        };
        let preprocessing = Qwen38VisionPreprocessedInput {
            pixel_values,
            receipt,
        };
        let runtime = Qwen38VisionRuntimeOutput {
            embeddings,
            receipt: Qwen38VisionRuntimeReceipt {
                schema_version: String::from(QWEN38_VISION_RUNTIME_SCHEMA_VERSION),
                backend: String::from("cpu"),
                execution_mode: String::from("native"),
                execution_engine: String::from("test"),
                fallback_policy: String::from("refuse"),
                source_shard_sha256: String::from("test"),
                image_processor_sha256: String::from(QWEN38_IMAGE_PROCESSOR_SHA256),
                video_processor_sha256: String::from(QWEN38_VIDEO_PROCESSOR_SHA256),
                resident_tensor_count: 333,
                resident_tensor_bytes: 921_460_192,
                resident_layer_count: 27,
                expected_layer_count: 27,
                full_stack_resident: true,
                input_patch_count: preprocessing.receipt.patch_count,
                input_bytes: 0,
                output_token_count: token_count,
                output_width: QWEN38_DECODER_HIDDEN_SIZE,
                output_bytes: 0,
                output_sha256,
                elapsed_ms: 0,
                timeout_ms: 1,
                host_output_materialized: true,
                hidden_fallback_used: false,
            },
        };
        match kind {
            Qwen38VisionMediaKind::Image => Qwen38DecoderMediaInput::image(preprocessing, runtime),
            Qwen38VisionMediaKind::Video => {
                Qwen38DecoderMediaInput::video(preprocessing, runtime, 4.0)
            }
        }
    }

    fn f32_values_sha256(values: &[f32]) -> String {
        let mut hasher = Sha256::new();
        for value in values {
            hasher.update(value.to_le_bytes());
        }
        hex::encode(hasher.finalize())
    }

    #[test]
    fn image_plan_matches_qwen35_mrope_grouping() {
        let media = [media(Qwen38VisionMediaKind::Image, [1, 4, 4])];
        let token_ids = vec![
            10,
            QWEN38_VISION_START_TOKEN_ID,
            QWEN38_IMAGE_PAD_TOKEN_ID,
            QWEN38_IMAGE_PAD_TOKEN_ID,
            QWEN38_IMAGE_PAD_TOKEN_ID,
            QWEN38_IMAGE_PAD_TOKEN_ID,
            QWEN38_VISION_END_TOKEN_ID,
            11,
        ];
        let plan = build_qwen38_multimodal_decoder_plan_from_token_ids(
            &prompt(),
            String::from("synthetic"),
            token_ids,
            &media,
        )
        .expect("image plan");
        assert_eq!(
            plan.mrope_position_ids,
            vec![
                [0, 0, 0],
                [1, 1, 1],
                [2, 2, 2],
                [2, 2, 3],
                [2, 3, 2],
                [2, 3, 3],
                [4, 4, 4],
                [5, 5, 5],
            ]
        );
        assert_eq!(plan.mrope_position_delta, -2);
        assert_eq!(plan.generated_position(8).expect("decode position"), [6; 3]);
        assert_eq!(plan.embedding_overrides.len(), 4);
    }

    #[test]
    fn video_plan_splits_temporal_grid_at_timestamp_text() {
        let media = [media(Qwen38VisionMediaKind::Video, [2, 4, 4])];
        let mut token_ids = vec![10, 20];
        token_ids.extend([QWEN38_VIDEO_PAD_TOKEN_ID; 4]);
        token_ids.extend([21, 22]);
        token_ids.extend([QWEN38_VIDEO_PAD_TOKEN_ID; 4]);
        token_ids.push(23);
        let plan = build_qwen38_multimodal_decoder_plan_from_token_ids(
            &prompt(),
            String::from("synthetic"),
            token_ids,
            &media,
        )
        .expect("video plan");
        assert_eq!(plan.mrope_position_ids[2], [2, 2, 2]);
        assert_eq!(plan.mrope_position_ids[5], [2, 3, 3]);
        assert_eq!(plan.mrope_position_ids[6], [4, 4, 4]);
        assert_eq!(plan.mrope_position_ids[8], [6, 6, 6]);
        assert_eq!(plan.mrope_position_ids[11], [6, 7, 7]);
        assert_eq!(plan.mrope_position_ids[12], [8, 8, 8]);
        assert_eq!(plan.embedding_overrides.len(), 8);
        assert_eq!(plan.receipt.video_token_count, 8);
    }

    #[test]
    fn decoder_plan_rejects_tampered_materialized_embeddings() {
        let mut media = media(Qwen38VisionMediaKind::Image, [1, 4, 4]);
        media.runtime.embeddings[0][0] = 99.0;
        let error = build_qwen38_multimodal_decoder_plan_from_token_ids(
            &prompt(),
            String::from("synthetic"),
            vec![QWEN38_IMAGE_PAD_TOKEN_ID; 4],
            &[media],
        )
        .expect_err("tampered embeddings must fail");
        assert!(error.to_string().contains("digest"));
    }

    #[test]
    fn official_tokenizer_preserves_expanded_media_spans_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::var("PSIONIC_QWEN38_TOKENIZER_PATH")
            .unwrap_or_else(|_| String::from("target/models/qwen/Qwen3.8-27B/tokenizer.json"));
        if !std::path::Path::new(path.as_str()).exists() {
            return Ok(());
        }
        let tokenizer = Qwen38Tokenizer::from_official_file(path)?;
        let mut prompt = prompt();
        prompt.text = String::from(
            "inspect <|vision_start|><|image_pad|><|vision_end|> then <|vision_start|><|video_pad|><|vision_end|>",
        );
        let media = [
            media(Qwen38VisionMediaKind::Image, [1, 4, 4]),
            media(Qwen38VisionMediaKind::Video, [2, 4, 4]),
        ];
        let plan = build_qwen38_multimodal_decoder_plan(&prompt, &tokenizer, &media)?;
        assert_eq!(plan.receipt.image_token_count, 4);
        assert_eq!(plan.receipt.video_token_count, 8);
        assert_eq!(plan.receipt.embedding_override_count, 12);
        assert_eq!(
            plan.token_ids
                .iter()
                .filter(|token| **token == QWEN38_IMAGE_PAD_TOKEN_ID)
                .count(),
            4
        );
        assert_eq!(
            plan.token_ids
                .iter()
                .filter(|token| **token == QWEN38_VIDEO_PAD_TOKEN_ID)
                .count(),
            8
        );
        assert_eq!(plan.mrope_position_ids.len(), plan.token_ids.len());
        Ok(())
    }
}
