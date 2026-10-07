// The tensor graph follows the Apache-2.0 Candle Qwen3-VL vision implementation,
// with Qwen3.8-specific admission, timeout, and runtime receipts.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use candle::{D, DType, Device, IndexOp, Tensor};
use candle_nn::{
    Activation, LayerNorm, LayerNormConfig, Linear, Module, VarBuilder, layer_norm, linear,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    QWEN38_VISION_SOURCE_SHARD, QWEN38_VISION_TENSOR_BYTES, QWEN38_VISION_TENSOR_COUNT,
    Qwen38VisionArtifactAdmissionReport, Qwen38VisionArtifactAdmissionStatus,
    Qwen38VisionArtifactError, Qwen38VisionPreprocessedInput, inspect_qwen38_vision_artifact,
    validate_qwen38_vision_preprocessed_input,
};

pub const QWEN38_VISION_RUNTIME_SCHEMA_VERSION: &str = "psionic.qwen38.vision_runtime.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38VisionRuntimeBackend {
    Cpu,
    #[cfg(feature = "qwen38-vision-cuda")]
    Cuda {
        device_ordinal: usize,
    },
}

impl Qwen38VisionRuntimeBackend {
    fn device(self) -> Result<Device, Qwen38VisionRuntimeError> {
        match self {
            Self::Cpu => Ok(Device::Cpu),
            #[cfg(feature = "qwen38-vision-cuda")]
            Self::Cuda { device_ordinal } => Device::new_cuda(device_ordinal).map_err(|error| {
                Qwen38VisionRuntimeError::BackendUnavailable {
                    backend: format!("cuda:{device_ordinal}"),
                    detail: error.to_string(),
                }
            }),
        }
    }

    pub const fn backend_label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            #[cfg(feature = "qwen38-vision-cuda")]
            Self::Cuda { .. } => "cuda",
        }
    }

    pub const fn execution_engine(self) -> &'static str {
        match self {
            Self::Cpu => "psionic_candle_qwen38_vision_cpu",
            #[cfg(feature = "qwen38-vision-cuda")]
            Self::Cuda { .. } => "psionic_candle_qwen38_vision_cuda",
        }
    }

    const fn runtime_dtype(self) -> DType {
        match self {
            Self::Cpu => DType::F32,
            #[cfg(feature = "qwen38-vision-cuda")]
            Self::Cuda { .. } => DType::BF16,
        }
    }

    const fn resident_tensor_bytes(self) -> u64 {
        match self {
            Self::Cpu => QWEN38_VISION_TENSOR_BYTES * 2,
            #[cfg(feature = "qwen38-vision-cuda")]
            Self::Cuda { .. } => QWEN38_VISION_TENSOR_BYTES,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38VisionRuntimeReceipt {
    pub schema_version: String,
    pub backend: String,
    pub execution_mode: String,
    pub execution_engine: String,
    pub fallback_policy: String,
    pub source_shard_sha256: String,
    pub image_processor_sha256: String,
    pub video_processor_sha256: String,
    pub resident_tensor_count: usize,
    pub resident_tensor_bytes: u64,
    pub resident_layer_count: usize,
    pub expected_layer_count: usize,
    pub full_stack_resident: bool,
    pub input_patch_count: usize,
    pub input_bytes: u64,
    pub output_token_count: usize,
    pub output_width: usize,
    pub output_bytes: u64,
    pub output_sha256: String,
    pub elapsed_ms: u64,
    pub timeout_ms: u64,
    pub host_output_materialized: bool,
    pub hidden_fallback_used: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38VisionRuntimeOutput {
    pub embeddings: Vec<Vec<f32>>,
    pub receipt: Qwen38VisionRuntimeReceipt,
}

#[derive(Debug, Error)]
pub enum Qwen38VisionRuntimeError {
    #[error(transparent)]
    Artifact(#[from] Qwen38VisionArtifactError),
    #[error("Qwen3.8 vision artifact admission refused: {0}")]
    ArtifactRefused(String),
    #[error("Qwen3.8 vision backend `{backend}` is unavailable: {detail}")]
    BackendUnavailable { backend: String, detail: String },
    #[error("failed to load Qwen3.8 native vision model: {0}")]
    ModelLoad(String),
    #[error("invalid Qwen3.8 native vision input: {0}")]
    InvalidInput(String),
    #[error("Qwen3.8 native vision execution failed: {0}")]
    Execution(String),
    #[error(
        "Qwen3.8 native vision execution timed out after layer {completed_layers} of {total_layers}"
    )]
    TimedOut {
        completed_layers: usize,
        total_layers: usize,
    },
}

pub struct Qwen38NativeVisionRuntime {
    backend: Qwen38VisionRuntimeBackend,
    device: Device,
    model: VisionModel,
    admission: Qwen38VisionArtifactAdmissionReport,
}

impl Qwen38NativeVisionRuntime {
    pub fn from_official_model_dir(
        model_dir: impl AsRef<Path>,
        backend: Qwen38VisionRuntimeBackend,
    ) -> Result<Self, Qwen38VisionRuntimeError> {
        let model_dir = model_dir.as_ref();
        let admission = inspect_qwen38_vision_artifact(model_dir)?;
        if admission.status != Qwen38VisionArtifactAdmissionStatus::Admitted {
            return Err(Qwen38VisionRuntimeError::ArtifactRefused(
                admission
                    .refusal_detail
                    .clone()
                    .unwrap_or_else(|| String::from("unknown vision artifact refusal")),
            ));
        }
        let device = backend.device()?;
        let shard_path = model_dir.join(QWEN38_VISION_SOURCE_SHARD);
        let weights = [shard_path.as_path()];
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&weights, backend.runtime_dtype(), &device)
        }
        .map_err(|error| Qwen38VisionRuntimeError::ModelLoad(error.to_string()))?;
        let model = VisionModel::new(&VisionConfig::official(), vb.pp("model").pp("visual"))
            .map_err(|error| Qwen38VisionRuntimeError::ModelLoad(error.to_string()))?;
        Ok(Self {
            backend,
            device,
            model,
            admission,
        })
    }

    pub const fn backend(&self) -> Qwen38VisionRuntimeBackend {
        self.backend
    }

    pub fn artifact_admission(&self) -> &Qwen38VisionArtifactAdmissionReport {
        &self.admission
    }

    pub fn encode(
        &self,
        input: &Qwen38VisionPreprocessedInput,
    ) -> Result<Qwen38VisionRuntimeOutput, Qwen38VisionRuntimeError> {
        validate_qwen38_vision_preprocessed_input(input)
            .map_err(|error| Qwen38VisionRuntimeError::InvalidInput(error.to_string()))?;
        let patch_count = input.receipt.patch_count;
        let patch_vector_size = self.model.config.patch_vector_size();
        if input.receipt.patch_vector_size != patch_vector_size {
            return Err(Qwen38VisionRuntimeError::InvalidInput(format!(
                "patch vector width {} does not match admitted width {patch_vector_size}",
                input.receipt.patch_vector_size
            )));
        }
        let expected_values = patch_count.checked_mul(patch_vector_size).ok_or_else(|| {
            Qwen38VisionRuntimeError::InvalidInput(String::from("input tensor size overflow"))
        })?;
        if input.pixel_values.len() != expected_values {
            return Err(Qwen38VisionRuntimeError::InvalidInput(format!(
                "pixel_values has {} values; expected {expected_values}",
                input.pixel_values.len()
            )));
        }
        let [grid_t, grid_h, grid_w] = input.receipt.grid_thw;
        let grid_patch_count = grid_t
            .checked_mul(grid_h)
            .and_then(|count| count.checked_mul(grid_w))
            .ok_or_else(|| {
                Qwen38VisionRuntimeError::InvalidInput(String::from("grid size overflow"))
            })?;
        if grid_patch_count != patch_count {
            return Err(Qwen38VisionRuntimeError::InvalidInput(format!(
                "grid patch count {grid_patch_count} does not match receipt count {patch_count}"
            )));
        }
        if grid_h % self.model.config.spatial_merge_size != 0
            || grid_w % self.model.config.spatial_merge_size != 0
        {
            return Err(Qwen38VisionRuntimeError::InvalidInput(String::from(
                "grid dimensions are not divisible by the spatial merge size",
            )));
        }

        let started = Instant::now();
        let deadline = started
            .checked_add(Duration::from_millis(input.receipt.limits.timeout_ms))
            .ok_or_else(|| {
                Qwen38VisionRuntimeError::InvalidInput(String::from("timeout deadline overflow"))
            })?;
        let pixel_values = Tensor::from_vec(
            input.pixel_values.clone(),
            (patch_count, patch_vector_size),
            &self.device,
        )
        .map_err(|error| Qwen38VisionRuntimeError::Execution(error.to_string()))?;
        let output = self
            .model
            .forward(&pixel_values, [grid_t, grid_h, grid_w], deadline)?;
        self.device
            .synchronize()
            .map_err(|error| Qwen38VisionRuntimeError::Execution(error.to_string()))?;
        if Instant::now() >= deadline {
            return Err(Qwen38VisionRuntimeError::TimedOut {
                completed_layers: self.model.config.depth,
                total_layers: self.model.config.depth,
            });
        }
        let embeddings = output
            .to_dtype(DType::F32)
            .and_then(|output| output.to_vec2::<f32>())
            .map_err(|error| Qwen38VisionRuntimeError::Execution(error.to_string()))?;
        let output_token_count = embeddings.len();
        let output_width = embeddings.first().map_or(0, Vec::len);
        let output_sha256 = qwen38_vision_embeddings_sha256(embeddings.as_slice());
        let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        Ok(Qwen38VisionRuntimeOutput {
            embeddings,
            receipt: Qwen38VisionRuntimeReceipt {
                schema_version: String::from(QWEN38_VISION_RUNTIME_SCHEMA_VERSION),
                backend: String::from(self.backend.backend_label()),
                execution_mode: String::from("native"),
                execution_engine: String::from(self.backend.execution_engine()),
                fallback_policy: String::from("refuse"),
                source_shard_sha256: self.admission.source_shard_sha256.clone(),
                image_processor_sha256: self.admission.image_processor_sha256.clone(),
                video_processor_sha256: self.admission.video_processor_sha256.clone(),
                resident_tensor_count: QWEN38_VISION_TENSOR_COUNT,
                resident_tensor_bytes: self.backend.resident_tensor_bytes(),
                resident_layer_count: self.model.config.depth,
                expected_layer_count: 27,
                full_stack_resident: self.model.config.depth == 27,
                input_patch_count: patch_count,
                input_bytes: (input.pixel_values.len() as u64).saturating_mul(4),
                output_token_count,
                output_width,
                output_bytes: (output_token_count as u64)
                    .saturating_mul(output_width as u64)
                    .saturating_mul(4),
                output_sha256,
                elapsed_ms,
                timeout_ms: input.receipt.limits.timeout_ms,
                host_output_materialized: true,
                hidden_fallback_used: false,
            },
        })
    }
}

#[derive(Clone, Debug)]
struct VisionConfig {
    depth: usize,
    hidden_size: usize,
    out_hidden_size: usize,
    hidden_act: Activation,
    intermediate_size: usize,
    num_heads: usize,
    in_channels: usize,
    patch_size: usize,
    spatial_merge_size: usize,
    temporal_patch_size: usize,
    num_position_embeddings: usize,
}

impl VisionConfig {
    fn official() -> Self {
        Self {
            depth: 27,
            hidden_size: 1_152,
            out_hidden_size: 5_120,
            hidden_act: Activation::GeluPytorchTanh,
            intermediate_size: 4_304,
            num_heads: 16,
            in_channels: 3,
            patch_size: 16,
            spatial_merge_size: 2,
            temporal_patch_size: 2,
            num_position_embeddings: 2_304,
        }
    }

    fn patch_vector_size(&self) -> usize {
        self.in_channels * self.temporal_patch_size * self.patch_size * self.patch_size
    }
}

struct PatchEmbed {
    projection: Linear,
}

impl PatchEmbed {
    fn new(config: &VisionConfig, vb: VarBuilder<'_>) -> candle::Result<Self> {
        let vb = vb.pp("proj");
        let weight = vb
            .get(
                (
                    config.hidden_size,
                    config.in_channels,
                    config.temporal_patch_size,
                    config.patch_size,
                    config.patch_size,
                ),
                "weight",
            )?
            .flatten_from(1)?;
        let bias = vb.get(config.hidden_size, "bias")?;
        Ok(Self {
            projection: Linear::new(weight, Some(bias)),
        })
    }

    fn forward(&self, input: &Tensor) -> candle::Result<Tensor> {
        self.projection.forward(input)
    }
}

struct VisionMlp {
    fc1: Linear,
    fc2: Linear,
    activation: Activation,
}

impl VisionMlp {
    fn new(config: &VisionConfig, vb: VarBuilder<'_>) -> candle::Result<Self> {
        Ok(Self {
            fc1: linear(
                config.hidden_size,
                config.intermediate_size,
                vb.pp("linear_fc1"),
            )?,
            fc2: linear(
                config.intermediate_size,
                config.hidden_size,
                vb.pp("linear_fc2"),
            )?,
            activation: config.hidden_act,
        })
    }

    fn forward(&self, input: &Tensor) -> candle::Result<Tensor> {
        self.fc2
            .forward(&self.activation.forward(&self.fc1.forward(input)?)?)
    }
}

fn rotate_half(input: &Tensor) -> candle::Result<Tensor> {
    let width = input.dim(D::Minus1)?;
    let first = input.narrow(D::Minus1, 0, width / 2)?;
    let second = input.narrow(D::Minus1, width / 2, width - width / 2)?;
    Tensor::cat(&[&second.neg()?, &first], D::Minus1)
}

fn apply_rotary(
    query: &Tensor,
    key: &Tensor,
    cosine: &Tensor,
    sine: &Tensor,
) -> candle::Result<(Tensor, Tensor)> {
    let cosine = cosine.unsqueeze(D::Minus2)?;
    let sine = sine.unsqueeze(D::Minus2)?;
    let query = (query.broadcast_mul(&cosine)? + rotate_half(query)?.broadcast_mul(&sine)?)?;
    let key = (key.broadcast_mul(&cosine)? + rotate_half(key)?.broadcast_mul(&sine)?)?;
    Ok((query, key))
}

struct VisionAttention {
    qkv: Linear,
    projection: Linear,
    num_heads: usize,
    head_dim: usize,
}

impl VisionAttention {
    fn new(config: &VisionConfig, vb: VarBuilder<'_>) -> candle::Result<Self> {
        Ok(Self {
            qkv: linear(config.hidden_size, config.hidden_size * 3, vb.pp("qkv"))?,
            projection: linear(config.hidden_size, config.hidden_size, vb.pp("proj"))?,
            num_heads: config.num_heads,
            head_dim: config.hidden_size / config.num_heads,
        })
    }

    fn forward(
        &self,
        input: &Tensor,
        cumulative_sequence_lengths: &[usize],
        cosine: &Tensor,
        sine: &Tensor,
    ) -> candle::Result<Tensor> {
        let sequence_length = input.dim(0)?;
        let qkv = self
            .qkv
            .forward(input)?
            .reshape((sequence_length, 3, self.num_heads, self.head_dim))?
            .permute((1, 0, 2, 3))?;
        let mut query = qkv.i(0)?.squeeze(0)?.to_dtype(DType::F32)?;
        let mut key = qkv.i(1)?.squeeze(0)?.to_dtype(DType::F32)?;
        let value = qkv.i(2)?.squeeze(0)?;
        (query, key) = apply_rotary(&query, &key, cosine, sine)?;
        query = query.to_dtype(input.dtype())?;
        key = key.to_dtype(input.dtype())?;

        let mut chunks = Vec::new();
        for window in cumulative_sequence_lengths.windows(2) {
            let start = window[0];
            let length = window[1] - start;
            let query = query
                .narrow(0, start, length)?
                .transpose(0, 1)?
                .contiguous()?;
            let key = key
                .narrow(0, start, length)?
                .transpose(0, 1)?
                .contiguous()?;
            let value = value
                .narrow(0, start, length)?
                .transpose(0, 1)?
                .contiguous()?;
            let weights = (query
                .unsqueeze(0)?
                .matmul(&key.unsqueeze(0)?.transpose(2, 3)?)?
                / (self.head_dim as f64).sqrt())?;
            let weights = candle_nn::ops::softmax_last_dim(&weights.to_dtype(DType::F32)?)?
                .to_dtype(input.dtype())?;
            let output = weights
                .matmul(&value.unsqueeze(0)?)?
                .squeeze(0)?
                .transpose(0, 1)?
                .reshape((length, self.num_heads * self.head_dim))?
                .to_dtype(input.dtype())?;
            chunks.push(output);
        }
        self.projection.forward(&Tensor::cat(chunks.as_slice(), 0)?)
    }
}

struct VisionBlock {
    norm1: LayerNorm,
    norm2: LayerNorm,
    attention: VisionAttention,
    mlp: VisionMlp,
}

impl VisionBlock {
    fn new(config: &VisionConfig, vb: VarBuilder<'_>) -> candle::Result<Self> {
        let norm_config = LayerNormConfig {
            eps: 1e-6,
            ..Default::default()
        };
        Ok(Self {
            norm1: layer_norm(config.hidden_size, norm_config, vb.pp("norm1"))?,
            norm2: layer_norm(config.hidden_size, norm_config, vb.pp("norm2"))?,
            attention: VisionAttention::new(config, vb.pp("attn"))?,
            mlp: VisionMlp::new(config, vb.pp("mlp"))?,
        })
    }

    fn forward(
        &self,
        input: &Tensor,
        cumulative_sequence_lengths: &[usize],
        cosine: &Tensor,
        sine: &Tensor,
    ) -> candle::Result<Tensor> {
        let attention = self.attention.forward(
            &self.norm1.forward(input)?,
            cumulative_sequence_lengths,
            cosine,
            sine,
        )?;
        let hidden = (input + attention)?;
        let mlp = self.mlp.forward(&self.norm2.forward(&hidden)?)?;
        hidden + mlp
    }
}

struct PatchMerger {
    norm: LayerNorm,
    fc1: Linear,
    fc2: Linear,
    spatial_merge_unit: usize,
    merged_hidden_size: usize,
}

impl PatchMerger {
    fn new(config: &VisionConfig, vb: VarBuilder<'_>) -> candle::Result<Self> {
        let merged_hidden_size = config.hidden_size * config.spatial_merge_size.pow(2);
        Ok(Self {
            norm: layer_norm(
                config.hidden_size,
                LayerNormConfig {
                    eps: 1e-6,
                    ..Default::default()
                },
                vb.pp("norm"),
            )?,
            fc1: linear(merged_hidden_size, merged_hidden_size, vb.pp("linear_fc1"))?,
            fc2: linear(
                merged_hidden_size,
                config.out_hidden_size,
                vb.pp("linear_fc2"),
            )?,
            spatial_merge_unit: config.spatial_merge_size.pow(2),
            merged_hidden_size,
        })
    }

    fn forward(&self, input: &Tensor) -> candle::Result<Tensor> {
        let sequence_length = input.dim(0)?;
        if sequence_length % self.spatial_merge_unit != 0 {
            candle::bail!(
                "vision sequence length {sequence_length} is not divisible by merge unit {}",
                self.spatial_merge_unit
            );
        }
        let merged = self.norm.forward(input)?.reshape((
            sequence_length / self.spatial_merge_unit,
            self.merged_hidden_size,
        ))?;
        self.fc2.forward(&self.fc1.forward(&merged)?.gelu_erf()?)
    }
}

struct VisionModel {
    config: VisionConfig,
    patch_embed: PatchEmbed,
    position_embeddings: Tensor,
    blocks: Vec<VisionBlock>,
    merger: PatchMerger,
    position_grid_side: usize,
}

impl VisionModel {
    fn new(config: &VisionConfig, vb: VarBuilder<'_>) -> candle::Result<Self> {
        let patch_embed = PatchEmbed::new(config, vb.pp("patch_embed"))?;
        let position_embeddings = vb.pp("pos_embed").get(
            (config.num_position_embeddings, config.hidden_size),
            "weight",
        )?;
        let mut blocks = Vec::with_capacity(config.depth);
        for layer in 0..config.depth {
            blocks.push(VisionBlock::new(config, vb.pp(format!("blocks.{layer}")))?);
        }
        let merger = PatchMerger::new(config, vb.pp("merger"))?;
        let position_grid_side = (config.num_position_embeddings as f64).sqrt() as usize;
        if position_grid_side * position_grid_side != config.num_position_embeddings {
            candle::bail!(
                "vision position count {} is not a square",
                config.num_position_embeddings
            );
        }
        Ok(Self {
            config: config.clone(),
            patch_embed,
            position_embeddings,
            blocks,
            merger,
            position_grid_side,
        })
    }

    fn forward(
        &self,
        input: &Tensor,
        grid: [usize; 3],
        deadline: Instant,
    ) -> Result<Tensor, Qwen38VisionRuntimeError> {
        ensure_deadline(deadline, 0, self.config.depth)?;
        let input = input
            .to_dtype(self.position_embeddings.dtype())
            .and_then(|input| self.patch_embed.forward(&input))
            .map_err(execution_error)?;
        let position = self.position_embedding(grid).map_err(execution_error)?;
        let mut hidden = (input + position).map_err(execution_error)?;
        let (cosine, sine) = self.rotary_embeddings(grid).map_err(execution_error)?;
        let cumulative_sequence_lengths = cumulative_sequence_lengths(grid);
        for (layer, block) in self.blocks.iter().enumerate() {
            ensure_deadline(deadline, layer, self.config.depth)?;
            hidden = block
                .forward(
                    &hidden,
                    cumulative_sequence_lengths.as_slice(),
                    &cosine,
                    &sine,
                )
                .map_err(execution_error)?;
            ensure_deadline(deadline, layer + 1, self.config.depth)?;
        }
        self.merger.forward(&hidden).map_err(execution_error)
    }

    fn position_embedding(&self, grid: [usize; 3]) -> candle::Result<Tensor> {
        let [temporal, height, width] = grid;
        let merge = self.config.spatial_merge_size;
        let mut indices = [Vec::<i64>::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut weights = [Vec::<f32>::new(), Vec::new(), Vec::new(), Vec::new()];
        for _ in 0..temporal {
            for block_row in 0..(height / merge) {
                for block_column in 0..(width / merge) {
                    for merge_row in 0..merge {
                        for merge_column in 0..merge {
                            let row = block_row * merge + merge_row;
                            let column = block_column * merge + merge_column;
                            let row_source =
                                interpolation_source(row, height, self.position_grid_side);
                            let column_source =
                                interpolation_source(column, width, self.position_grid_side);
                            let row_floor = row_source.floor() as usize;
                            let column_floor = column_source.floor() as usize;
                            let row_ceil = (row_floor + 1).min(self.position_grid_side - 1);
                            let column_ceil = (column_floor + 1).min(self.position_grid_side - 1);
                            let row_delta = row_source - row_floor as f32;
                            let column_delta = column_source - column_floor as f32;
                            let taps = [
                                (
                                    row_floor,
                                    column_floor,
                                    (1.0 - row_delta) * (1.0 - column_delta),
                                ),
                                (row_floor, column_ceil, (1.0 - row_delta) * column_delta),
                                (row_ceil, column_floor, row_delta * (1.0 - column_delta)),
                                (row_ceil, column_ceil, row_delta * column_delta),
                            ];
                            for (tap, (tap_row, tap_column, tap_weight)) in
                                taps.into_iter().enumerate()
                            {
                                indices[tap]
                                    .push((tap_row * self.position_grid_side + tap_column) as i64);
                                weights[tap].push(tap_weight);
                            }
                        }
                    }
                }
            }
        }
        let token_count = temporal * height * width;
        let mut output = Tensor::zeros(
            (token_count, self.config.hidden_size),
            self.position_embeddings.dtype(),
            self.position_embeddings.device(),
        )?;
        for tap in 0..4 {
            let indices = Tensor::from_vec(
                std::mem::take(&mut indices[tap]),
                (token_count,),
                self.position_embeddings.device(),
            )?;
            let weights = Tensor::from_vec(
                std::mem::take(&mut weights[tap]),
                (token_count, 1),
                self.position_embeddings.device(),
            )?
            .to_dtype(self.position_embeddings.dtype())?;
            let values = self.position_embeddings.index_select(&indices, 0)?;
            output = (output + values.broadcast_mul(&weights)?)?;
        }
        Ok(output)
    }

    fn rotary_embeddings(&self, grid: [usize; 3]) -> candle::Result<(Tensor, Tensor)> {
        let [temporal, height, width] = grid;
        let merge = self.config.spatial_merge_size;
        let head_dim = self.config.hidden_size / self.config.num_heads;
        let rotary_dim = head_dim / 2;
        let inverse_frequency = (0..rotary_dim)
            .step_by(2)
            .map(|index| 1.0 / 10_000f32.powf(index as f32 / rotary_dim as f32))
            .collect::<Vec<_>>();
        let mut cosine = Vec::with_capacity(temporal * height * width * head_dim);
        let mut sine = Vec::with_capacity(cosine.capacity());
        for _ in 0..temporal {
            for block_row in 0..(height / merge) {
                for block_column in 0..(width / merge) {
                    for merge_row in 0..merge {
                        for merge_column in 0..merge {
                            let row = (block_row * merge + merge_row) as f32;
                            let column = (block_column * merge + merge_column) as f32;
                            let mut frequencies = Vec::with_capacity(rotary_dim);
                            frequencies
                                .extend(inverse_frequency.iter().map(|frequency| row * frequency));
                            frequencies.extend(
                                inverse_frequency.iter().map(|frequency| column * frequency),
                            );
                            for _ in 0..2 {
                                cosine.extend(frequencies.iter().map(|value| value.cos()));
                                sine.extend(frequencies.iter().map(|value| value.sin()));
                            }
                        }
                    }
                }
            }
        }
        let token_count = temporal * height * width;
        Ok((
            Tensor::from_vec(
                cosine,
                (token_count, head_dim),
                self.position_embeddings.device(),
            )?,
            Tensor::from_vec(
                sine,
                (token_count, head_dim),
                self.position_embeddings.device(),
            )?,
        ))
    }
}

fn interpolation_source(index: usize, size: usize, source_side: usize) -> f32 {
    if size <= 1 {
        0.0
    } else {
        index as f32 * (source_side - 1) as f32 / (size - 1) as f32
    }
}

fn cumulative_sequence_lengths(grid: [usize; 3]) -> Vec<usize> {
    let [temporal, height, width] = grid;
    let frame_tokens = height * width;
    (0..=temporal).map(|frame| frame * frame_tokens).collect()
}

fn ensure_deadline(
    deadline: Instant,
    completed_layers: usize,
    total_layers: usize,
) -> Result<(), Qwen38VisionRuntimeError> {
    if Instant::now() >= deadline {
        return Err(Qwen38VisionRuntimeError::TimedOut {
            completed_layers,
            total_layers,
        });
    }
    Ok(())
}

fn execution_error(error: candle::Error) -> Qwen38VisionRuntimeError {
    Qwen38VisionRuntimeError::Execution(error.to_string())
}

pub(crate) fn qwen38_vision_embeddings_sha256(embeddings: &[Vec<f32>]) -> String {
    let mut hasher = Sha256::new();
    hasher.update((embeddings.len() as u64).to_le_bytes());
    for embedding in embeddings {
        hasher.update((embedding.len() as u64).to_le_bytes());
        for value in embedding {
            hasher.update(value.to_le_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    #[cfg(feature = "qwen38-vision-cuda")]
    use crate::{Qwen38RgbFrame, Qwen38VisionAdmissionLimits, qwen38_preprocess_image};

    fn tiny_config() -> VisionConfig {
        VisionConfig {
            depth: 1,
            hidden_size: 8,
            out_hidden_size: 6,
            hidden_act: Activation::GeluPytorchTanh,
            intermediate_size: 16,
            num_heads: 2,
            in_channels: 3,
            patch_size: 2,
            spatial_merge_size: 2,
            temporal_patch_size: 2,
            num_position_embeddings: 4,
        }
    }

    fn insert_zeros(
        tensors: &mut HashMap<String, Tensor>,
        name: impl Into<String>,
        shape: impl Into<candle::Shape>,
    ) {
        tensors.insert(
            name.into(),
            Tensor::zeros(shape, DType::F32, &Device::Cpu).expect("zero tensor"),
        );
    }

    fn tiny_weights(config: &VisionConfig) -> HashMap<String, Tensor> {
        let mut tensors = HashMap::new();
        insert_zeros(
            &mut tensors,
            "patch_embed.proj.weight",
            (
                config.hidden_size,
                config.in_channels,
                config.temporal_patch_size,
                config.patch_size,
                config.patch_size,
            ),
        );
        insert_zeros(&mut tensors, "patch_embed.proj.bias", config.hidden_size);
        insert_zeros(
            &mut tensors,
            "pos_embed.weight",
            (config.num_position_embeddings, config.hidden_size),
        );
        for layer in 0..config.depth {
            let prefix = format!("blocks.{layer}");
            for norm in ["norm1", "norm2"] {
                tensors.insert(
                    format!("{prefix}.{norm}.weight"),
                    Tensor::ones(config.hidden_size, DType::F32, &Device::Cpu)
                        .expect("norm weight"),
                );
                insert_zeros(
                    &mut tensors,
                    format!("{prefix}.{norm}.bias"),
                    config.hidden_size,
                );
            }
            insert_zeros(
                &mut tensors,
                format!("{prefix}.attn.qkv.weight"),
                (config.hidden_size * 3, config.hidden_size),
            );
            insert_zeros(
                &mut tensors,
                format!("{prefix}.attn.qkv.bias"),
                config.hidden_size * 3,
            );
            insert_zeros(
                &mut tensors,
                format!("{prefix}.attn.proj.weight"),
                (config.hidden_size, config.hidden_size),
            );
            insert_zeros(
                &mut tensors,
                format!("{prefix}.attn.proj.bias"),
                config.hidden_size,
            );
            insert_zeros(
                &mut tensors,
                format!("{prefix}.mlp.linear_fc1.weight"),
                (config.intermediate_size, config.hidden_size),
            );
            insert_zeros(
                &mut tensors,
                format!("{prefix}.mlp.linear_fc1.bias"),
                config.intermediate_size,
            );
            insert_zeros(
                &mut tensors,
                format!("{prefix}.mlp.linear_fc2.weight"),
                (config.hidden_size, config.intermediate_size),
            );
            insert_zeros(
                &mut tensors,
                format!("{prefix}.mlp.linear_fc2.bias"),
                config.hidden_size,
            );
        }
        tensors.insert(
            String::from("merger.norm.weight"),
            Tensor::ones(config.hidden_size, DType::F32, &Device::Cpu).expect("merger norm"),
        );
        insert_zeros(&mut tensors, "merger.norm.bias", config.hidden_size);
        let merged = config.hidden_size * config.spatial_merge_size.pow(2);
        insert_zeros(&mut tensors, "merger.linear_fc1.weight", (merged, merged));
        insert_zeros(&mut tensors, "merger.linear_fc1.bias", merged);
        insert_zeros(
            &mut tensors,
            "merger.linear_fc2.weight",
            (config.out_hidden_size, merged),
        );
        insert_zeros(
            &mut tensors,
            "merger.linear_fc2.bias",
            config.out_hidden_size,
        );
        tensors
    }

    #[test]
    fn qwen38_native_vision_tiny_graph_runs_full_stack() {
        let config = tiny_config();
        let vb = VarBuilder::from_tensors(tiny_weights(&config), DType::F32, &Device::Cpu);
        let model = VisionModel::new(&config, vb).expect("tiny vision model");
        let input = Tensor::zeros((4, config.patch_vector_size()), DType::F32, &Device::Cpu)
            .expect("tiny input");
        let output = model
            .forward(&input, [1, 2, 2], Instant::now() + Duration::from_secs(1))
            .expect("tiny forward");
        assert_eq!(output.dims(), &[1, 6]);
        assert_eq!(
            output.to_vec2::<f32>().expect("output values"),
            vec![vec![0.0; 6]]
        );
    }

    #[test]
    fn qwen38_cpu_vision_runtime_uses_supported_f32_residency() {
        assert_eq!(Qwen38VisionRuntimeBackend::Cpu.runtime_dtype(), DType::F32);
        assert_eq!(
            Qwen38VisionRuntimeBackend::Cpu.resident_tensor_bytes(),
            QWEN38_VISION_TENSOR_BYTES * 2
        );
    }

    #[test]
    fn qwen38_native_vision_timeout_refuses_before_execution() {
        let config = tiny_config();
        let vb = VarBuilder::from_tensors(tiny_weights(&config), DType::F32, &Device::Cpu);
        let model = VisionModel::new(&config, vb).expect("tiny vision model");
        let input = Tensor::zeros((4, config.patch_vector_size()), DType::F32, &Device::Cpu)
            .expect("tiny input");
        let error = model
            .forward(&input, [1, 2, 2], Instant::now())
            .expect_err("expired deadline must refuse");
        assert!(matches!(
            error,
            Qwen38VisionRuntimeError::TimedOut {
                completed_layers: 0,
                total_layers: 1,
            }
        ));
    }

    #[cfg(feature = "qwen38-vision-cuda")]
    #[test]
    fn qwen38_native_vision_real_cuda_runs_when_available() {
        let Some(model_dir) = std::env::var_os("PSIONIC_QWEN38_OFFICIAL_MODEL_DIR") else {
            return;
        };
        let rgb8 = (0..(256 * 256))
            .flat_map(|pixel| {
                let x = pixel % 256;
                let y = pixel / 256;
                [x as u8, y as u8, ((x + y) / 2) as u8]
            })
            .collect::<Vec<_>>();
        let frame = Qwen38RgbFrame::new(256, 256, rgb8).expect("reference image");
        let mut limits = Qwen38VisionAdmissionLimits::default();
        limits.timeout_ms = 120_000;
        let input =
            qwen38_preprocess_image("qwen38-gradient-256", "image/raw-rgb8", &frame, limits)
                .expect("preprocess reference image");
        let runtime = Qwen38NativeVisionRuntime::from_official_model_dir(
            model_dir,
            Qwen38VisionRuntimeBackend::Cuda { device_ordinal: 0 },
        )
        .expect("load native CUDA vision runtime");
        let output = runtime.encode(&input).expect("encode reference image");
        assert_eq!(output.embeddings.len(), 64);
        assert_eq!(output.embeddings[0].len(), 5_120);
        assert_eq!(output.receipt.backend, "cuda");
        assert_eq!(output.receipt.resident_layer_count, 27);
        assert!(output.receipt.full_stack_resident);
        assert!(!output.receipt.hidden_fallback_used);
        println!(
            "{}",
            serde_json::to_string_pretty(&output.receipt).expect("serialize receipt")
        );
    }
}
