use psionic_core::{DType, QuantizationMode, Shape};
use psionic_runtime::{TassadarExecutorDecodeMode, TassadarTraceAbi, TassadarWasmProfile};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelDescriptor, TassadarTraceTokenizer, TokenId, TokenSequence, TokenizerBoundary,
    WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
    tassadar::{
        TassadarAttentionGeometryContract, TassadarExecutorAttentionMode, TassadarExecutorFamily,
    },
};

/// Stable claim boundary for the bounded executor-attention research lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorAttentionClaimBoundary {
    /// The model is only evaluated on bounded prompt/target windows.
    ResearchWindowedDecodeOnly,
    /// Greedy decode exists but is not yet promoted as an exact executor.
    GreedyDecodeUnvalidated,
}

/// Explicit hard-max attention semantics carried by each layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorAttentionSemantics {
    /// Full-prefix causal hard-max attention over 2D heads.
    FullPrefixCausalHardMax2d,
}

/// Whether hull-backed decode is direct, fallback-only, or refused for one layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorAttentionHullPosture {
    /// The layer can use hull-backed decode directly.
    Direct,
    /// The layer falls back to reference-linear decode when hull is requested.
    FallbackToReferenceLinear,
    /// The layer refuses hull-backed decode entirely.
    Refused,
}

/// Per-layer semantics carried by the executor-attention descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionLayerSemantics {
    /// Zero-based layer index.
    pub layer_index: u16,
    /// Explicit attention semantics for the layer.
    pub semantics: TassadarExecutorAttentionSemantics,
    /// Number of 2D heads in the layer.
    pub head_count: u16,
    /// Fixed head dimension.
    pub head_dim: u16,
    /// Decode modes this layer supports directly.
    pub direct_decode_modes: Vec<TassadarExecutorDecodeMode>,
    /// Posture when hull-backed decode is requested.
    pub hull_cache_posture: TassadarExecutorAttentionHullPosture,
}

/// Bounded executor-attention config for the Sudoku-v0 research lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionConfig {
    /// Executor vocabulary size.
    pub vocab_size: usize,
    /// Maximum bounded sequence length admitted by the candidate.
    pub max_sequence_tokens: usize,
    /// Model width used by all layers.
    pub model_width: usize,
    /// Number of 2D attention heads.
    pub head_count: usize,
    /// Head dimension. This is fixed at `2` for the current candidate.
    pub head_dim: usize,
    /// Number of stacked causal-attention layers.
    pub layer_count: usize,
    /// Feed-forward width per layer.
    pub feed_forward_width: usize,
    /// Number of early decoded target positions that may carry a bounded
    /// position-specific output-bias adapter.
    pub relative_target_bias_token_cap: usize,
}

impl TassadarExecutorAttentionConfig {
    /// Returns the canonical bounded Sudoku-v0 research config.
    #[must_use]
    pub fn sudoku_v0(tokenizer: &TassadarTraceTokenizer) -> Self {
        Self {
            vocab_size: tokenizer.vocabulary().len(),
            max_sequence_tokens: 512,
            model_width: 36,
            head_count: 18,
            head_dim: 2,
            layer_count: 7,
            feed_forward_width: 36,
            relative_target_bias_token_cap: 32,
        }
    }
}

/// Descriptor for the bounded executor-attention research family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionDescriptor {
    /// Shared model identity.
    pub model: ModelDescriptor,
    /// Stable executor family.
    pub executor_family: TassadarExecutorFamily,
    /// Bound Wasm profile.
    pub profile: TassadarWasmProfile,
    /// Bound trace ABI.
    pub trace_abi: TassadarTraceAbi,
    /// Decode identities this model can surface honestly right now.
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
    /// Declared attention regime.
    pub attention_mode: TassadarExecutorAttentionMode,
    /// Declared geometry claims.
    pub attention_geometry: TassadarAttentionGeometryContract,
    /// Explicit claim boundary.
    pub claim_boundary: TassadarExecutorAttentionClaimBoundary,
    /// Bounded research config.
    pub config: TassadarExecutorAttentionConfig,
    /// Per-layer semantics visible to eval and research reports.
    pub layer_semantics: Vec<TassadarExecutorAttentionLayerSemantics>,
    /// Weight bundle metadata.
    pub weights: WeightBundleMetadata,
}

impl TassadarExecutorAttentionDescriptor {
    /// Returns a stable digest over the descriptor.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_tassadar_executor_attention_descriptor|", self)
    }
}

/// Programmatic seeded weight bundle for the executor-attention candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionWeightBundle {
    metadata: WeightBundleMetadata,
    token_embeddings: Vec<f32>,
    position_embeddings: Vec<f32>,
    query_projections: Vec<Vec<f32>>,
    key_projections: Vec<Vec<f32>>,
    value_projections: Vec<Vec<f32>>,
    output_projections: Vec<Vec<f32>>,
    feed_forward_in: Vec<Vec<f32>>,
    feed_forward_in_bias: Vec<Vec<f32>>,
    feed_forward_out: Vec<Vec<f32>>,
    output_projection: Vec<f32>,
    output_bias: Vec<f32>,
    relative_target_output_bias: Vec<f32>,
    relative_target_output_projection: Vec<f32>,
    relative_target_transition_output_bias: Vec<f32>,
    relative_target_trace_schema_output_bias: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TassadarEarlyTraceSchemaPhase {
    ExpectStep,
    ExpectStepIndex,
    ExpectStepIndexByte0,
    ExpectStepIndexByte1,
    ExpectStepIndexByte2,
    ExpectStepIndexByte3,
    ExpectPc,
    ExpectPcByte0,
    ExpectPcByte1,
    ExpectPcByte2,
    ExpectPcByte3,
    ExpectNextPc,
    ExpectNextPcByte0,
    ExpectNextPcByte1,
    ExpectNextPcByte2,
    ExpectNextPcByte3,
    ExpectInstruction,
}

impl TassadarEarlyTraceSchemaPhase {
    const COUNT: usize = 17;

    const fn index(self) -> usize {
        match self {
            Self::ExpectStep => 0,
            Self::ExpectStepIndex => 1,
            Self::ExpectStepIndexByte0 => 2,
            Self::ExpectStepIndexByte1 => 3,
            Self::ExpectStepIndexByte2 => 4,
            Self::ExpectStepIndexByte3 => 5,
            Self::ExpectPc => 6,
            Self::ExpectPcByte0 => 7,
            Self::ExpectPcByte1 => 8,
            Self::ExpectPcByte2 => 9,
            Self::ExpectPcByte3 => 10,
            Self::ExpectNextPc => 11,
            Self::ExpectNextPcByte0 => 12,
            Self::ExpectNextPcByte1 => 13,
            Self::ExpectNextPcByte2 => 14,
            Self::ExpectNextPcByte3 => 15,
            Self::ExpectInstruction => 16,
        }
    }
}

impl TassadarExecutorAttentionWeightBundle {
    fn new(config: &TassadarExecutorAttentionConfig) -> Self {
        let token_embeddings = seeded_values(
            "token_embeddings",
            config.vocab_size * config.model_width,
            0.1,
        );
        let position_embeddings = seeded_values(
            "position_embeddings",
            config.max_sequence_tokens * config.model_width,
            0.08,
        );
        let query_projections = (0..config.layer_count)
            .map(|layer| {
                seeded_values(
                    &format!("layers.{layer}.query_projection"),
                    config.model_width * config.model_width,
                    0.05,
                )
            })
            .collect::<Vec<_>>();
        let key_projections = (0..config.layer_count)
            .map(|layer| {
                seeded_values(
                    &format!("layers.{layer}.key_projection"),
                    config.model_width * config.model_width,
                    0.05,
                )
            })
            .collect::<Vec<_>>();
        let value_projections = (0..config.layer_count)
            .map(|layer| {
                seeded_values(
                    &format!("layers.{layer}.value_projection"),
                    config.model_width * config.model_width,
                    0.05,
                )
            })
            .collect::<Vec<_>>();
        let output_projections = (0..config.layer_count)
            .map(|layer| {
                seeded_values(
                    &format!("layers.{layer}.output_projection"),
                    config.model_width * config.model_width,
                    0.05,
                )
            })
            .collect::<Vec<_>>();
        let feed_forward_in = (0..config.layer_count)
            .map(|layer| {
                seeded_values(
                    &format!("layers.{layer}.feed_forward_in"),
                    config.model_width * config.feed_forward_width,
                    0.04,
                )
            })
            .collect::<Vec<_>>();
        let feed_forward_in_bias = vec![vec![0.0; config.feed_forward_width]; config.layer_count];
        let feed_forward_out = (0..config.layer_count)
            .map(|layer| {
                seeded_values(
                    &format!("layers.{layer}.feed_forward_out"),
                    config.feed_forward_width * config.model_width,
                    0.04,
                )
            })
            .collect::<Vec<_>>();
        let output_projection = seeded_values(
            "final_output_projection",
            config.model_width * config.vocab_size,
            0.06,
        );
        let output_bias = vec![0.0; config.vocab_size];
        let relative_target_output_bias =
            vec![0.0; config.relative_target_bias_token_cap * config.vocab_size];
        let relative_target_output_projection =
            vec![
                0.0;
                config.relative_target_bias_token_cap * config.model_width * config.vocab_size
            ];
        let relative_target_transition_output_bias =
            vec![
                0.0;
                config.relative_target_bias_token_cap * config.vocab_size * config.vocab_size
            ];
        let relative_target_trace_schema_output_bias =
            vec![0.0; TassadarEarlyTraceSchemaPhase::COUNT * config.vocab_size];

        let mut entries = vec![
            (
                WeightTensorMetadata::new(
                    "token_embeddings",
                    Shape::new(vec![config.vocab_size, config.model_width]),
                    DType::F32,
                ),
                token_embeddings.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "position_embeddings",
                    Shape::new(vec![config.max_sequence_tokens, config.model_width]),
                    DType::F32,
                ),
                position_embeddings.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "output_projection",
                    Shape::new(vec![config.model_width, config.vocab_size]),
                    DType::F32,
                ),
                output_projection.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "output_bias",
                    Shape::new(vec![config.vocab_size]),
                    DType::F32,
                ),
                output_bias.as_slice(),
            ),
        ];
        if config.relative_target_bias_token_cap > 0 {
            entries.push((
                WeightTensorMetadata::new(
                    "relative_target_output_bias",
                    Shape::new(vec![
                        config.relative_target_bias_token_cap,
                        config.vocab_size,
                    ]),
                    DType::F32,
                ),
                relative_target_output_bias.as_slice(),
            ));
            entries.push((
                WeightTensorMetadata::new(
                    "relative_target_output_projection",
                    Shape::new(vec![
                        config.relative_target_bias_token_cap,
                        config.model_width,
                        config.vocab_size,
                    ]),
                    DType::F32,
                ),
                relative_target_output_projection.as_slice(),
            ));
            entries.push((
                WeightTensorMetadata::new(
                    "relative_target_transition_output_bias",
                    Shape::new(vec![
                        config.relative_target_bias_token_cap,
                        config.vocab_size,
                        config.vocab_size,
                    ]),
                    DType::F32,
                ),
                relative_target_transition_output_bias.as_slice(),
            ));
            entries.push((
                WeightTensorMetadata::new(
                    "relative_target_trace_schema_output_bias",
                    Shape::new(vec![
                        TassadarEarlyTraceSchemaPhase::COUNT,
                        config.vocab_size,
                    ]),
                    DType::F32,
                ),
                relative_target_trace_schema_output_bias.as_slice(),
            ));
        }
        for layer in 0..config.layer_count {
            entries.extend([
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.query_projection"),
                        Shape::new(vec![config.model_width, config.model_width]),
                        DType::F32,
                    ),
                    query_projections[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.key_projection"),
                        Shape::new(vec![config.model_width, config.model_width]),
                        DType::F32,
                    ),
                    key_projections[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.value_projection"),
                        Shape::new(vec![config.model_width, config.model_width]),
                        DType::F32,
                    ),
                    value_projections[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.output_projection"),
                        Shape::new(vec![config.model_width, config.model_width]),
                        DType::F32,
                    ),
                    output_projections[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.feed_forward_in"),
                        Shape::new(vec![config.model_width, config.feed_forward_width]),
                        DType::F32,
                    ),
                    feed_forward_in[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.feed_forward_in_bias"),
                        Shape::new(vec![config.feed_forward_width]),
                        DType::F32,
                    ),
                    feed_forward_in_bias[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.feed_forward_out"),
                        Shape::new(vec![config.feed_forward_width, config.model_width]),
                        DType::F32,
                    ),
                    feed_forward_out[layer].as_slice(),
                ),
            ]);
        }

        Self {
            metadata: build_metadata(entries.as_slice()),
            token_embeddings,
            position_embeddings,
            query_projections,
            key_projections,
            value_projections,
            output_projections,
            feed_forward_in,
            feed_forward_in_bias,
            feed_forward_out,
            output_projection,
            output_bias,
            relative_target_output_bias,
            relative_target_output_projection,
            relative_target_transition_output_bias,
            relative_target_trace_schema_output_bias,
        }
    }

    /// Returns the stable bundle metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightBundleMetadata {
        &self.metadata
    }

    /// Returns the current output projection.
    #[must_use]
    pub fn output_projection(&self) -> &[f32] {
        &self.output_projection
    }

    /// Returns mutable output projection weights for bounded research training.
    pub fn output_projection_mut(&mut self) -> &mut [f32] {
        &mut self.output_projection
    }

    /// Returns the current output bias.
    #[must_use]
    pub fn output_bias(&self) -> &[f32] {
        &self.output_bias
    }

    /// Returns mutable output bias weights for bounded research training.
    pub fn output_bias_mut(&mut self) -> &mut [f32] {
        &mut self.output_bias
    }

    /// Returns the flattened relative-target output-bias adapter tensor.
    #[must_use]
    pub fn relative_target_output_bias(&self) -> &[f32] {
        &self.relative_target_output_bias
    }

    /// Returns mutable relative-target output-bias adapter weights for bounded
    /// research training.
    pub fn relative_target_output_bias_mut(&mut self) -> &mut [f32] {
        &mut self.relative_target_output_bias
    }

    /// Returns the flattened relative-target hidden-state-conditioned output
    /// projection adapter tensor.
    #[must_use]
    pub fn relative_target_output_projection(&self) -> &[f32] {
        &self.relative_target_output_projection
    }

    /// Returns mutable relative-target hidden-state-conditioned output
    /// projection adapter weights for bounded research training.
    pub fn relative_target_output_projection_mut(&mut self) -> &mut [f32] {
        &mut self.relative_target_output_projection
    }

    /// Returns the flattened previous-token-conditioned relative-target output
    /// bias adapter tensor.
    #[must_use]
    pub fn relative_target_transition_output_bias(&self) -> &[f32] {
        &self.relative_target_transition_output_bias
    }

    /// Returns mutable previous-token-conditioned relative-target output bias
    /// adapter weights for bounded research training.
    pub fn relative_target_transition_output_bias_mut(&mut self) -> &mut [f32] {
        &mut self.relative_target_transition_output_bias
    }

    /// Returns the flattened trace-schema-conditioned relative-target output
    /// bias adapter tensor.
    #[must_use]
    pub fn relative_target_trace_schema_output_bias(&self) -> &[f32] {
        &self.relative_target_trace_schema_output_bias
    }

    /// Returns mutable trace-schema-conditioned relative-target output bias
    /// adapter weights for bounded research training.
    pub fn relative_target_trace_schema_output_bias_mut(&mut self) -> &mut [f32] {
        &mut self.relative_target_trace_schema_output_bias
    }

    fn refresh_metadata(&mut self, config: &TassadarExecutorAttentionConfig) {
        let mut entries = vec![
            (
                WeightTensorMetadata::new(
                    "token_embeddings",
                    Shape::new(vec![config.vocab_size, config.model_width]),
                    DType::F32,
                ),
                self.token_embeddings.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "position_embeddings",
                    Shape::new(vec![config.max_sequence_tokens, config.model_width]),
                    DType::F32,
                ),
                self.position_embeddings.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "output_projection",
                    Shape::new(vec![config.model_width, config.vocab_size]),
                    DType::F32,
                ),
                self.output_projection.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "output_bias",
                    Shape::new(vec![config.vocab_size]),
                    DType::F32,
                ),
                self.output_bias.as_slice(),
            ),
        ];
        if config.relative_target_bias_token_cap > 0 {
            entries.push((
                WeightTensorMetadata::new(
                    "relative_target_output_bias",
                    Shape::new(vec![
                        config.relative_target_bias_token_cap,
                        config.vocab_size,
                    ]),
                    DType::F32,
                ),
                self.relative_target_output_bias.as_slice(),
            ));
            entries.push((
                WeightTensorMetadata::new(
                    "relative_target_output_projection",
                    Shape::new(vec![
                        config.relative_target_bias_token_cap,
                        config.model_width,
                        config.vocab_size,
                    ]),
                    DType::F32,
                ),
                self.relative_target_output_projection.as_slice(),
            ));
            entries.push((
                WeightTensorMetadata::new(
                    "relative_target_transition_output_bias",
                    Shape::new(vec![
                        config.relative_target_bias_token_cap,
                        config.vocab_size,
                        config.vocab_size,
                    ]),
                    DType::F32,
                ),
                self.relative_target_transition_output_bias.as_slice(),
            ));
            entries.push((
                WeightTensorMetadata::new(
                    "relative_target_trace_schema_output_bias",
                    Shape::new(vec![
                        TassadarEarlyTraceSchemaPhase::COUNT,
                        config.vocab_size,
                    ]),
                    DType::F32,
                ),
                self.relative_target_trace_schema_output_bias.as_slice(),
            ));
        }
        for layer in 0..config.layer_count {
            entries.extend([
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.query_projection"),
                        Shape::new(vec![config.model_width, config.model_width]),
                        DType::F32,
                    ),
                    self.query_projections[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.key_projection"),
                        Shape::new(vec![config.model_width, config.model_width]),
                        DType::F32,
                    ),
                    self.key_projections[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.value_projection"),
                        Shape::new(vec![config.model_width, config.model_width]),
                        DType::F32,
                    ),
                    self.value_projections[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.output_projection"),
                        Shape::new(vec![config.model_width, config.model_width]),
                        DType::F32,
                    ),
                    self.output_projections[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.feed_forward_in"),
                        Shape::new(vec![config.model_width, config.feed_forward_width]),
                        DType::F32,
                    ),
                    self.feed_forward_in[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.feed_forward_in_bias"),
                        Shape::new(vec![config.feed_forward_width]),
                        DType::F32,
                    ),
                    self.feed_forward_in_bias[layer].as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        format!("layers.{layer}.feed_forward_out"),
                        Shape::new(vec![config.feed_forward_width, config.model_width]),
                        DType::F32,
                    ),
                    self.feed_forward_out[layer].as_slice(),
                ),
            ]);
        }
        self.metadata = build_metadata(entries.as_slice());
    }
}

/// One head-level selected position for a decoded token step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionHeadMatch {
    /// Zero-based head index.
    pub head_index: u16,
    /// Prefix position selected by the head.
    pub selected_position: u32,
}

/// Per-layer step-local attention context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionLayerStepContext {
    /// Zero-based layer index.
    pub layer_index: u16,
    /// Selected prefix positions per head.
    pub head_matches: Vec<TassadarExecutorAttentionHeadMatch>,
}

/// Hidden-state and logits emitted by one bounded forward pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionForwardPass {
    /// Top hidden state for each next-token prediction step.
    pub hidden_states: Vec<Vec<f32>>,
    /// Vocabulary logits for each prediction position.
    pub logits: Vec<Vec<f32>>,
    /// Per-layer attention contexts used to produce each prediction.
    pub layer_contexts: Vec<Vec<TassadarExecutorAttentionLayerStepContext>>,
}

/// Typed refusal when selecting one decode path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorAttentionDecodeRefusal {
    /// No supported decode mode exists at all.
    NoSupportedDecodeMode,
}

/// Machine-readable decode selection for the executor-attention candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionDecodeSelection {
    /// Decode path requested by the caller.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Decode path actually executed by the model when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Exact fallback mode used when the request could not execute directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Typed refusal reason when the request could not execute at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<TassadarExecutorAttentionDecodeRefusal>,
    /// Decode modes surfaced by the descriptor.
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TassadarExecutorAttentionKvPoint {
    position: u32,
    key_x: f32,
    key_y: f32,
    value_x: f32,
    value_y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TassadarExecutorAttentionLayerCache {
    hidden_states: Vec<Vec<f32>>,
    head_kv_points: Vec<Vec<TassadarExecutorAttentionKvPoint>>,
}

impl TassadarExecutorAttentionLayerCache {
    fn new(head_count: usize) -> Self {
        Self {
            hidden_states: Vec::new(),
            head_kv_points: vec![Vec::new(); head_count],
        }
    }
}

/// Decode state for the bounded executor-attention candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorAttentionDecodeState {
    /// Prefix tokens visible to the next decode step.
    pub prefix: TokenSequence,
    /// Prompt token count before any decoded targets were appended.
    pub initial_prompt_len: usize,
    /// Layer caches built over the current prefix.
    layer_caches: Vec<TassadarExecutorAttentionLayerCache>,
    /// Per-layer contexts recorded for the last processed token.
    pub last_step_layer_contexts: Vec<TassadarExecutorAttentionLayerStepContext>,
}

/// Layered causal-attention research candidate for the Sudoku-v0 executor lane.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarExecutorAttentionTransformer {
    descriptor: TassadarExecutorAttentionDescriptor,
    tokenizer: TassadarTraceTokenizer,
    weights: TassadarExecutorAttentionWeightBundle,
}

impl TassadarExecutorAttentionTransformer {
    /// Stable model identifier for the bounded Sudoku-v0 candidate family.
    pub const MODEL_ID: &str = "tassadar-executor-attention-transformer-sudoku-v0-v0";
    /// Stable model family label.
    pub const MODEL_FAMILY: &str = "tassadar_executor_attention_transformer";

    /// Creates the canonical bounded Sudoku-v0 executor-attention candidate.
    #[must_use]
    pub fn sudoku_v0() -> Self {
        let tokenizer = TassadarTraceTokenizer::new();
        let config = TassadarExecutorAttentionConfig::sudoku_v0(&tokenizer);
        let weights = TassadarExecutorAttentionWeightBundle::new(&config);
        let layer_semantics = (0..config.layer_count)
            .map(|layer_index| TassadarExecutorAttentionLayerSemantics {
                layer_index: layer_index as u16,
                semantics: TassadarExecutorAttentionSemantics::FullPrefixCausalHardMax2d,
                head_count: config.head_count as u16,
                head_dim: config.head_dim as u16,
                direct_decode_modes: vec![TassadarExecutorDecodeMode::ReferenceLinear],
                hull_cache_posture: TassadarExecutorAttentionHullPosture::FallbackToReferenceLinear,
            })
            .collect::<Vec<_>>();
        let descriptor = TassadarExecutorAttentionDescriptor {
            model: ModelDescriptor::new(Self::MODEL_ID, Self::MODEL_FAMILY, "v0"),
            executor_family: TassadarExecutorFamily::WasmTraceExecutor,
            profile: TassadarWasmProfile::sudoku_v0_search_v1(),
            trace_abi: TassadarTraceAbi::sudoku_v0_search_v1(),
            supported_decode_modes: vec![TassadarExecutorDecodeMode::ReferenceLinear],
            attention_mode: TassadarExecutorAttentionMode::HardMaxLookup,
            attention_geometry: TassadarAttentionGeometryContract {
                constrained_lookup_head_dim: Some(config.head_dim),
                hull_cache_eligible: false,
            },
            claim_boundary: TassadarExecutorAttentionClaimBoundary::ResearchWindowedDecodeOnly,
            config,
            layer_semantics,
            weights: weights.metadata().clone(),
        };
        Self {
            descriptor,
            tokenizer,
            weights,
        }
    }

    /// Returns the public descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &TassadarExecutorAttentionDescriptor {
        &self.descriptor
    }

    /// Returns the tokenizer.
    #[must_use]
    pub fn tokenizer(&self) -> &TassadarTraceTokenizer {
        &self.tokenizer
    }

    /// Returns the current weight bundle.
    #[must_use]
    pub fn weights(&self) -> &TassadarExecutorAttentionWeightBundle {
        &self.weights
    }

    /// Returns mutable access to the weight bundle for bounded research training.
    pub fn weights_mut(&mut self) -> &mut TassadarExecutorAttentionWeightBundle {
        &mut self.weights
    }

    /// Refreshes descriptor-level weight metadata after a training update.
    pub fn refresh_after_training(&mut self) {
        self.weights.refresh_metadata(&self.descriptor.config);
        self.descriptor.weights = self.weights.metadata().clone();
    }

    /// Returns whether the descriptor advertises one decode mode directly.
    #[must_use]
    pub fn supports_decode_mode(&self, decode_mode: TassadarExecutorDecodeMode) -> bool {
        self.descriptor
            .supported_decode_modes
            .contains(&decode_mode)
    }

    /// Resolves one requested decode mode into an effective path or fallback.
    #[must_use]
    pub fn select_decode_mode(
        &self,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> TassadarExecutorAttentionDecodeSelection {
        let supported_decode_modes = self.descriptor.supported_decode_modes.clone();
        if self.supports_decode_mode(requested_decode_mode) {
            return TassadarExecutorAttentionDecodeSelection {
                requested_decode_mode,
                effective_decode_mode: Some(requested_decode_mode),
                fallback_decode_mode: None,
                refusal: None,
                supported_decode_modes,
            };
        }
        if requested_decode_mode == TassadarExecutorDecodeMode::HullCache
            && self.supports_decode_mode(TassadarExecutorDecodeMode::ReferenceLinear)
        {
            return TassadarExecutorAttentionDecodeSelection {
                requested_decode_mode,
                effective_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
                fallback_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
                refusal: None,
                supported_decode_modes,
            };
        }
        TassadarExecutorAttentionDecodeSelection {
            requested_decode_mode,
            effective_decode_mode: None,
            fallback_decode_mode: None,
            refusal: Some(TassadarExecutorAttentionDecodeRefusal::NoSupportedDecodeMode),
            supported_decode_modes,
        }
    }

    /// Runs a bounded next-token forward pass over one tokenized executor sequence.
    pub fn forward_logits(
        &self,
        sequence: &TokenSequence,
    ) -> Result<TassadarExecutorAttentionForwardPass, TassadarExecutorAttentionError> {
        if sequence.len() > self.descriptor.config.max_sequence_tokens {
            return Err(TassadarExecutorAttentionError::SequenceTooLong {
                token_count: sequence.len(),
                max_supported: self.descriptor.config.max_sequence_tokens,
            });
        }
        if sequence.is_empty() {
            return Err(TassadarExecutorAttentionError::EmptyPrompt);
        }
        let mut state = self.start_decode(TokenSequence::new(vec![sequence.as_slice()[0]]))?;
        let mut hidden_states = Vec::new();
        let mut logits = Vec::new();
        let mut layer_contexts = Vec::new();
        for token in sequence.as_slice().iter().skip(1) {
            hidden_states.push(self.top_hidden_state(&state)?.to_vec());
            logits.push(self.project_logits(self.top_hidden_state(&state)?)?);
            layer_contexts.push(state.last_step_layer_contexts.clone());
            self.push_decoded_token(&mut state, *token)?;
        }
        Ok(TassadarExecutorAttentionForwardPass {
            hidden_states,
            logits,
            layer_contexts,
        })
    }

    /// Creates a decode state from one bounded prompt sequence.
    pub fn start_decode(
        &self,
        prompt: TokenSequence,
    ) -> Result<TassadarExecutorAttentionDecodeState, TassadarExecutorAttentionError> {
        if prompt.is_empty() {
            return Err(TassadarExecutorAttentionError::EmptyPrompt);
        }
        if prompt.len() > self.descriptor.config.max_sequence_tokens {
            return Err(TassadarExecutorAttentionError::SequenceTooLong {
                token_count: prompt.len(),
                max_supported: self.descriptor.config.max_sequence_tokens,
            });
        }
        let mut state = TassadarExecutorAttentionDecodeState {
            prefix: TokenSequence::default(),
            initial_prompt_len: prompt.len(),
            layer_caches: (0..self.descriptor.config.layer_count)
                .map(|_| {
                    TassadarExecutorAttentionLayerCache::new(self.descriptor.config.head_count)
                })
                .collect(),
            last_step_layer_contexts: Vec::new(),
        };
        for token in prompt.as_slice() {
            self.append_token_to_state(
                &mut state,
                *token,
                TassadarExecutorDecodeMode::ReferenceLinear,
            )?;
        }
        Ok(state)
    }

    /// Extends one decode state with an accepted next token.
    pub fn push_decoded_token(
        &self,
        state: &mut TassadarExecutorAttentionDecodeState,
        next_token: TokenId,
    ) -> Result<(), TassadarExecutorAttentionError> {
        self.append_token_to_state(
            state,
            next_token,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )
    }

    /// Returns next-token logits for the current decode state.
    pub fn next_token_logits(
        &self,
        state: &TassadarExecutorAttentionDecodeState,
    ) -> Result<Vec<f32>, TassadarExecutorAttentionError> {
        self.next_token_logits_for_mode(state, TassadarExecutorDecodeMode::ReferenceLinear)
    }

    /// Returns next-token logits for one requested decode mode.
    pub fn next_token_logits_for_mode(
        &self,
        state: &TassadarExecutorAttentionDecodeState,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<Vec<f32>, TassadarExecutorAttentionError> {
        let selection = self.select_decode_mode(requested_decode_mode);
        let Some(effective_decode_mode) = selection.effective_decode_mode else {
            return Err(TassadarExecutorAttentionError::UnsupportedDecodeMode {
                requested: requested_decode_mode,
                supported: selection.supported_decode_modes,
            });
        };
        if effective_decode_mode != TassadarExecutorDecodeMode::ReferenceLinear {
            return Err(TassadarExecutorAttentionError::UnsupportedDecodeMode {
                requested: requested_decode_mode,
                supported: vec![TassadarExecutorDecodeMode::ReferenceLinear],
            });
        }
        let relative_target_index = state.prefix.len().saturating_sub(state.initial_prompt_len);
        let previous_token = state.prefix.as_slice().last().copied();
        let trace_schema_phase = self.relative_target_trace_schema_phase_index(
            state.prefix.as_slice(),
            state.initial_prompt_len,
        );
        self.project_logits_for_relative_target_step(
            self.top_hidden_state(state)?,
            previous_token,
            relative_target_index,
            trace_schema_phase,
        )
    }

    /// Greedily chooses the next token for one decode state.
    pub fn greedy_next_token(
        &self,
        state: &TassadarExecutorAttentionDecodeState,
    ) -> Result<TokenId, TassadarExecutorAttentionError> {
        self.greedy_next_token_for_mode(state, TassadarExecutorDecodeMode::ReferenceLinear)
    }

    /// Greedily chooses the next token for one requested decode mode.
    pub fn greedy_next_token_for_mode(
        &self,
        state: &TassadarExecutorAttentionDecodeState,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<TokenId, TassadarExecutorAttentionError> {
        let logits = self.next_token_logits_for_mode(state, requested_decode_mode)?;
        let (best_index, _) = logits
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.partial_cmp(right).expect("finite logits"))
            .expect("vocabulary logits should be non-empty");
        Ok(TokenId(best_index as u32))
    }

    fn append_token_to_state(
        &self,
        state: &mut TassadarExecutorAttentionDecodeState,
        token: TokenId,
        decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<(), TassadarExecutorAttentionError> {
        let position = state.prefix.len();
        if position >= self.descriptor.config.max_sequence_tokens {
            return Err(TassadarExecutorAttentionError::SequenceTooLong {
                token_count: position + 1,
                max_supported: self.descriptor.config.max_sequence_tokens,
            });
        }
        let mut hidden = self.token_plus_position_embedding(token, position)?;
        let mut layer_contexts = Vec::with_capacity(self.descriptor.config.layer_count);
        for layer_index in 0..self.descriptor.config.layer_count {
            let (next_hidden, context) = self.run_layer(
                layer_index,
                position,
                hidden.as_slice(),
                &mut state.layer_caches[layer_index],
                decode_mode,
            )?;
            hidden = next_hidden;
            layer_contexts.push(context);
        }
        state.prefix.push(token);
        state.last_step_layer_contexts = layer_contexts;
        Ok(())
    }

    fn run_layer(
        &self,
        layer_index: usize,
        position: usize,
        hidden: &[f32],
        cache: &mut TassadarExecutorAttentionLayerCache,
        decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<(Vec<f32>, TassadarExecutorAttentionLayerStepContext), TassadarExecutorAttentionError>
    {
        let config = &self.descriptor.config;
        let queries = self.project_heads(&self.weights.query_projections[layer_index], hidden)?;
        let keys = self.project_heads(&self.weights.key_projections[layer_index], hidden)?;
        let values = self.project_heads(&self.weights.value_projections[layer_index], hidden)?;
        let mut concatenated_attention = vec![0.0; config.model_width];
        let mut head_matches = Vec::with_capacity(config.head_count);

        for head_index in 0..config.head_count {
            let current_point = TassadarExecutorAttentionKvPoint {
                position: position as u32,
                key_x: keys[head_index][0],
                key_y: keys[head_index][1],
                value_x: values[head_index][0],
                value_y: values[head_index][1],
            };
            let selected = self.select_kv_point(
                cache.head_kv_points[head_index].as_slice(),
                &current_point,
                queries[head_index],
                decode_mode,
            )?;
            let offset = head_index * config.head_dim;
            concatenated_attention[offset] = selected.value_x;
            concatenated_attention[offset + 1] = selected.value_y;
            head_matches.push(TassadarExecutorAttentionHeadMatch {
                head_index: head_index as u16,
                selected_position: selected.position,
            });
            cache.head_kv_points[head_index].push(current_point);
        }

        let projected_attention = matvec(
            &self.weights.output_projections[layer_index],
            hidden.len(),
            hidden.len(),
            concatenated_attention.as_slice(),
        )?;
        let mut next_hidden = hidden.to_vec();
        for (index, value) in projected_attention.iter().enumerate() {
            next_hidden[index] += value;
        }

        let mut feed_forward_hidden = matvec(
            &self.weights.feed_forward_in[layer_index],
            config.model_width,
            config.feed_forward_width,
            next_hidden.as_slice(),
        )?;
        for (index, value) in feed_forward_hidden.iter_mut().enumerate() {
            *value = (*value + self.weights.feed_forward_in_bias[layer_index][index]).max(0.0);
        }
        let feed_forward_output = matvec(
            &self.weights.feed_forward_out[layer_index],
            config.feed_forward_width,
            config.model_width,
            feed_forward_hidden.as_slice(),
        )?;
        for (index, value) in feed_forward_output.iter().enumerate() {
            next_hidden[index] += value;
        }
        cache.hidden_states.push(next_hidden.clone());

        Ok((
            next_hidden,
            TassadarExecutorAttentionLayerStepContext {
                layer_index: layer_index as u16,
                head_matches,
            },
        ))
    }

    fn select_kv_point<'a>(
        &self,
        existing_points: &'a [TassadarExecutorAttentionKvPoint],
        current_point: &'a TassadarExecutorAttentionKvPoint,
        query: [f32; 2],
        decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<&'a TassadarExecutorAttentionKvPoint, TassadarExecutorAttentionError> {
        if decode_mode != TassadarExecutorDecodeMode::ReferenceLinear {
            return Err(TassadarExecutorAttentionError::UnsupportedDecodeMode {
                requested: decode_mode,
                supported: vec![TassadarExecutorDecodeMode::ReferenceLinear],
            });
        }
        let mut selected = current_point;
        let mut best_score = attention_score(current_point, query);
        for point in existing_points {
            let score = attention_score(point, query);
            if score > best_score {
                best_score = score;
                selected = point;
            }
        }
        Ok(selected)
    }

    fn token_plus_position_embedding(
        &self,
        token: TokenId,
        position: usize,
    ) -> Result<Vec<f32>, TassadarExecutorAttentionError> {
        let token_embedding = self.token_embedding(token)?;
        let position_embedding = self.position_embedding(position);
        Ok(token_embedding
            .iter()
            .zip(position_embedding.iter())
            .map(|(token_value, position_value)| token_value + position_value)
            .collect())
    }

    fn top_hidden_state<'a>(
        &self,
        state: &'a TassadarExecutorAttentionDecodeState,
    ) -> Result<&'a [f32], TassadarExecutorAttentionError> {
        state
            .layer_caches
            .last()
            .and_then(|layer| layer.hidden_states.last())
            .map(Vec::as_slice)
            .ok_or(TassadarExecutorAttentionError::EmptyDecodeState)
    }

    fn token_embedding(&self, token: TokenId) -> Result<&[f32], TassadarExecutorAttentionError> {
        let index = token.as_u32() as usize;
        if index >= self.descriptor.config.vocab_size {
            return Err(TassadarExecutorAttentionError::UnknownTokenId {
                token_id: token.as_u32(),
                vocab_size: self.descriptor.config.vocab_size,
            });
        }
        let width = self.descriptor.config.model_width;
        let start = index * width;
        Ok(&self.weights.token_embeddings[start..start + width])
    }

    fn position_embedding(&self, position: usize) -> &[f32] {
        let width = self.descriptor.config.model_width;
        let clamped = position.min(self.descriptor.config.max_sequence_tokens - 1);
        let start = clamped * width;
        &self.weights.position_embeddings[start..start + width]
    }

    fn project_heads(
        &self,
        projection: &[f32],
        hidden: &[f32],
    ) -> Result<Vec<[f32; 2]>, TassadarExecutorAttentionError> {
        let output = matvec(
            projection,
            self.descriptor.config.model_width,
            self.descriptor.config.model_width,
            hidden,
        )?;
        Ok(output
            .chunks_exact(self.descriptor.config.head_dim)
            .map(|chunk| [chunk[0], chunk[1]])
            .collect())
    }

    fn project_logits(&self, hidden: &[f32]) -> Result<Vec<f32>, TassadarExecutorAttentionError> {
        self.project_logits_for_relative_target_step(hidden, None, usize::MAX, None)
    }

    fn project_logits_for_relative_target_step(
        &self,
        hidden: &[f32],
        previous_token: Option<TokenId>,
        relative_target_index: usize,
        trace_schema_phase: Option<usize>,
    ) -> Result<Vec<f32>, TassadarExecutorAttentionError> {
        let logits = matvec(
            &self.weights.output_projection,
            self.descriptor.config.model_width,
            self.descriptor.config.vocab_size,
            hidden,
        )?;
        let mut logits = logits
            .into_iter()
            .enumerate()
            .map(|(index, value)| value + self.weights.output_bias[index])
            .collect::<Vec<_>>();
        self.apply_relative_target_output_bias_in_place(
            logits.as_mut_slice(),
            relative_target_index,
        );
        self.apply_relative_target_output_projection_in_place(
            logits.as_mut_slice(),
            hidden,
            relative_target_index,
        );
        self.apply_relative_target_transition_output_bias_in_place(
            logits.as_mut_slice(),
            previous_token,
            relative_target_index,
        );
        self.apply_relative_target_trace_schema_output_bias_in_place(
            logits.as_mut_slice(),
            trace_schema_phase,
        );
        Ok(logits)
    }

    /// Applies the bounded relative-target output-bias adapter to one logit
    /// slice in place.
    pub fn apply_relative_target_output_bias_in_place(
        &self,
        logits: &mut [f32],
        relative_target_index: usize,
    ) {
        let vocab_size = self.descriptor.config.vocab_size;
        if logits.len() != vocab_size
            || relative_target_index >= self.descriptor.config.relative_target_bias_token_cap
        {
            return;
        }
        let start = relative_target_index * vocab_size;
        let bias_slice = &self.weights.relative_target_output_bias[start..start + vocab_size];
        for (logit, bias) in logits.iter_mut().zip(bias_slice.iter()) {
            *logit += *bias;
        }
    }

    /// Applies the bounded relative-target hidden-state-conditioned output
    /// projection adapter to one logit slice in place.
    pub fn apply_relative_target_output_projection_in_place(
        &self,
        logits: &mut [f32],
        hidden: &[f32],
        relative_target_index: usize,
    ) {
        let vocab_size = self.descriptor.config.vocab_size;
        let model_width = self.descriptor.config.model_width;
        if logits.len() != vocab_size
            || hidden.len() != model_width
            || relative_target_index >= self.descriptor.config.relative_target_bias_token_cap
        {
            return;
        }
        let block_len = model_width * vocab_size;
        let start = relative_target_index * block_len;
        let projection = &self.weights.relative_target_output_projection[start..start + block_len];
        for (hidden_index, hidden_value) in hidden.iter().enumerate() {
            let row_start = hidden_index * vocab_size;
            let row = &projection[row_start..row_start + vocab_size];
            for (logit, weight) in logits.iter_mut().zip(row.iter()) {
                *logit += hidden_value * weight;
            }
        }
    }

    /// Applies the bounded previous-token-conditioned relative-target output
    /// bias adapter to one logit slice in place.
    pub fn apply_relative_target_transition_output_bias_in_place(
        &self,
        logits: &mut [f32],
        previous_token: Option<TokenId>,
        relative_target_index: usize,
    ) {
        let vocab_size = self.descriptor.config.vocab_size;
        let Some(previous_token) = previous_token else {
            return;
        };
        let previous_token_index = previous_token.as_u32() as usize;
        if logits.len() != vocab_size
            || previous_token_index >= vocab_size
            || relative_target_index >= self.descriptor.config.relative_target_bias_token_cap
        {
            return;
        }
        let row_start = (relative_target_index * vocab_size + previous_token_index) * vocab_size;
        let row =
            &self.weights.relative_target_transition_output_bias[row_start..row_start + vocab_size];
        for (logit, bias) in logits.iter_mut().zip(row.iter()) {
            *logit += *bias;
        }
    }

    /// Applies the bounded trace-schema-conditioned relative-target output
    /// bias adapter to one logit slice in place.
    pub fn apply_relative_target_trace_schema_output_bias_in_place(
        &self,
        logits: &mut [f32],
        trace_schema_phase: Option<usize>,
    ) {
        let Some(trace_schema_phase) = trace_schema_phase else {
            return;
        };
        let vocab_size = self.descriptor.config.vocab_size;
        if logits.len() != vocab_size || trace_schema_phase >= TassadarEarlyTraceSchemaPhase::COUNT
        {
            return;
        }
        let row_start = trace_schema_phase * vocab_size;
        let row = &self.weights.relative_target_trace_schema_output_bias
            [row_start..row_start + vocab_size];
        for (logit, bias) in logits.iter_mut().zip(row.iter()) {
            *logit += *bias;
        }
    }

    /// Returns whether the bounded relative-target output-bias adapter has any
    /// non-zero trained signal.
    #[must_use]
    pub fn has_relative_target_output_bias_signal(&self) -> bool {
        self.weights
            .relative_target_output_bias
            .iter()
            .any(|value| value.abs() > 1e-6)
    }

    /// Returns whether the bounded relative-target output projection adapter
    /// has any non-zero trained signal.
    #[must_use]
    pub fn has_relative_target_output_projection_signal(&self) -> bool {
        self.weights
            .relative_target_output_projection
            .iter()
            .any(|value| value.abs() > 1e-6)
    }

    /// Returns whether the bounded previous-token-conditioned relative-target
    /// output-bias adapter has any non-zero trained signal.
    #[must_use]
    pub fn has_relative_target_transition_output_bias_signal(&self) -> bool {
        self.weights
            .relative_target_transition_output_bias
            .iter()
            .any(|value| value.abs() > 1e-6)
    }

    /// Returns whether the bounded trace-schema-conditioned relative-target
    /// output-bias adapter has any non-zero trained signal.
    #[must_use]
    pub fn has_relative_target_trace_schema_output_bias_signal(&self) -> bool {
        self.weights
            .relative_target_trace_schema_output_bias
            .iter()
            .any(|value| value.abs() > 1e-6)
    }

    /// Returns the bounded early trace-schema phase index for the current
    /// decoded prefix when one is recognized.
    #[must_use]
    pub fn relative_target_trace_schema_phase_index(
        &self,
        prefix: &[TokenId],
        initial_prompt_len: usize,
    ) -> Option<usize> {
        self.relative_target_trace_schema_phase(prefix, initial_prompt_len)
            .map(TassadarEarlyTraceSchemaPhase::index)
    }

    fn relative_target_trace_schema_phase(
        &self,
        prefix: &[TokenId],
        initial_prompt_len: usize,
    ) -> Option<TassadarEarlyTraceSchemaPhase> {
        if prefix.len() < initial_prompt_len || initial_prompt_len == 0 {
            return None;
        }
        let target_prefix = &prefix[initial_prompt_len..];
        let trace_token = self.token_id("<trace>");
        if target_prefix.is_empty() {
            return (prefix.last().copied() == Some(trace_token))
                .then_some(TassadarEarlyTraceSchemaPhase::ExpectStep);
        }

        let step_token = self.token_id("<step>");
        let step_index_token = self.token_id("<step_index>");
        let pc_token = self.token_id("<pc>");
        let next_pc_token = self.token_id("<next_pc>");
        let (byte_token_start, byte_token_end) = self.byte_token_bounds();
        let is_byte = |token: TokenId| {
            let raw = token.as_u32();
            raw >= byte_token_start && raw <= byte_token_end
        };
        let all_bytes = |tokens: &[TokenId]| tokens.iter().copied().all(is_byte);

        match target_prefix.len() {
            1 if target_prefix[0] == step_token => Some(TassadarEarlyTraceSchemaPhase::ExpectStepIndex),
            2 if target_prefix[0] == step_token && target_prefix[1] == step_index_token => {
                Some(TassadarEarlyTraceSchemaPhase::ExpectStepIndexByte0)
            }
            3 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..3]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectStepIndexByte1)
            }
            4 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..4]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectStepIndexByte2)
            }
            5 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..5]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectStepIndexByte3)
            }
            6 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectPc)
            }
            7 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectPcByte0)
            }
            8 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token
                && all_bytes(&target_prefix[7..8]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectPcByte1)
            }
            9 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token
                && all_bytes(&target_prefix[7..9]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectPcByte2)
            }
            10 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token
                && all_bytes(&target_prefix[7..10]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectPcByte3)
            }
            11 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token
                && all_bytes(&target_prefix[7..11]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectNextPc)
            }
            12 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token
                && all_bytes(&target_prefix[7..11])
                && target_prefix[11] == next_pc_token =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectNextPcByte0)
            }
            13 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token
                && all_bytes(&target_prefix[7..11])
                && target_prefix[11] == next_pc_token
                && all_bytes(&target_prefix[12..13]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectNextPcByte1)
            }
            14 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token
                && all_bytes(&target_prefix[7..11])
                && target_prefix[11] == next_pc_token
                && all_bytes(&target_prefix[12..14]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectNextPcByte2)
            }
            15 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token
                && all_bytes(&target_prefix[7..11])
                && target_prefix[11] == next_pc_token
                && all_bytes(&target_prefix[12..15]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectNextPcByte3)
            }
            16 if target_prefix[0] == step_token
                && target_prefix[1] == step_index_token
                && all_bytes(&target_prefix[2..6])
                && target_prefix[6] == pc_token
                && all_bytes(&target_prefix[7..11])
                && target_prefix[11] == next_pc_token
                && all_bytes(&target_prefix[12..16]) =>
            {
                Some(TassadarEarlyTraceSchemaPhase::ExpectInstruction)
            }
            _ => None,
        }
    }

    fn token_id(&self, token: &str) -> TokenId {
        self.tokenizer.encode(token).as_slice()[0]
    }

    fn byte_token_bounds(&self) -> (u32, u32) {
        let start = self.tokenizer.encode("<byte_00>").as_slice()[0].as_u32();
        let end = self.tokenizer.encode("<byte_ff>").as_slice()[0].as_u32();
        (start, end)
    }
}

/// Bounded executor-attention forward/decode failure.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutorAttentionError {
    /// One caller supplied a token id outside the model vocabulary.
    #[error("unknown token id {token_id}; vocabulary size is {vocab_size}")]
    UnknownTokenId {
        /// Offending token id.
        token_id: u32,
        /// Vocabulary size.
        vocab_size: usize,
    },
    /// One caller supplied a sequence beyond the configured context length.
    #[error("sequence is too long: {token_count} tokens > max {max_supported}")]
    SequenceTooLong {
        /// Requested token count.
        token_count: usize,
        /// Maximum supported token count.
        max_supported: usize,
    },
    /// One caller requested decode from an empty prompt.
    #[error("decode requires at least one prompt token")]
    EmptyPrompt,
    /// The decode state does not yet hold one fully processed token.
    #[error("decode state does not yet contain one hidden state")]
    EmptyDecodeState,
    /// Internal tensor width drifted from the descriptor.
    #[error(
        "matrix width mismatch: expected input {expected_input} / output {expected_output}, found tensor length {actual_len}"
    )]
    MatrixWidthMismatch {
        /// Expected input width.
        expected_input: usize,
        /// Expected output width.
        expected_output: usize,
        /// Actual flattened tensor length.
        actual_len: usize,
    },
    /// Hidden-state width drifted from the descriptor.
    #[error("hidden width mismatch: expected {expected}, found {actual}")]
    HiddenWidthMismatch {
        /// Expected width.
        expected: usize,
        /// Actual width.
        actual: usize,
    },
    /// The caller requested a decode mode the model does not advertise honestly.
    #[error("unsupported decode mode `{requested:?}`; supported modes: {supported:?}")]
    UnsupportedDecodeMode {
        /// Requested mode.
        requested: TassadarExecutorDecodeMode,
        /// Supported modes.
        supported: Vec<TassadarExecutorDecodeMode>,
    },
}

fn attention_score(point: &TassadarExecutorAttentionKvPoint, query: [f32; 2]) -> f32 {
    point.key_x * query[0] + point.key_y * query[1]
}

fn matvec(
    weights: &[f32],
    input_width: usize,
    output_width: usize,
    input: &[f32],
) -> Result<Vec<f32>, TassadarExecutorAttentionError> {
    if weights.len() != input_width * output_width {
        return Err(TassadarExecutorAttentionError::MatrixWidthMismatch {
            expected_input: input_width,
            expected_output: output_width,
            actual_len: weights.len(),
        });
    }
    if input.len() != input_width {
        return Err(TassadarExecutorAttentionError::HiddenWidthMismatch {
            expected: input_width,
            actual: input.len(),
        });
    }
    let mut output = vec![0.0; output_width];
    for (output_index, value) in output.iter_mut().enumerate() {
        let mut sum = 0.0;
        for (input_index, input_value) in input.iter().enumerate() {
            let weight_index = input_index * output_width + output_index;
            sum += input_value * weights[weight_index];
        }
        *value = sum;
    }
    Ok(output)
}

fn seeded_values(label: &str, len: usize, scale: f32) -> Vec<f32> {
    (0..len)
        .map(|index| {
            let mut hasher = Sha256::new();
            hasher.update(b"psionic_tassadar_executor_attention_seed|");
            hasher.update(label.as_bytes());
            hasher.update(index.to_le_bytes());
            let bytes = hasher.finalize();
            let sample = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let centered = (sample as f32 / u32::MAX as f32) * 2.0 - 1.0;
            centered * scale
        })
        .collect()
}

fn build_metadata(entries: &[(WeightTensorMetadata, &[f32])]) -> WeightBundleMetadata {
    let mut ordered = entries.to_vec();
    ordered.sort_by(|(left, _), (right, _)| left.name.cmp(&right.name));

    let mut hasher = Sha256::new();
    for (metadata, values) in &ordered {
        digest_tensor_values(&mut hasher, metadata, values);
    }

    WeightBundleMetadata {
        format: WeightFormat::ProgrammaticFixture,
        source: WeightSource::Fixture,
        quantization: QuantizationMode::None,
        quantization_modes: Vec::new(),
        digest: hex::encode(hasher.finalize()),
        tensors: ordered
            .iter()
            .map(|(metadata, _)| metadata.clone())
            .collect(),
        artifacts: Vec::new(),
    }
}

fn digest_tensor_values(hasher: &mut Sha256, metadata: &WeightTensorMetadata, values: &[f32]) {
    hasher.update(metadata.name.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.dtype).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.quantization).as_bytes());
    hasher.update(b"|");
    for dimension in metadata.shape.dims() {
        hasher.update(dimension.to_be_bytes());
    }
    hasher.update(b"|");
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar executor attention value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_runtime::{TassadarExecutorDecodeMode, tassadar_sudoku_v0_corpus};

    use crate::{TassadarTraceTokenizer, TokenSequence, TokenizerBoundary};

    use super::{
        TassadarEarlyTraceSchemaPhase, TassadarExecutorAttentionClaimBoundary,
        TassadarExecutorAttentionHullPosture, TassadarExecutorAttentionTransformer,
    };

    #[test]
    fn executor_attention_descriptor_is_explicit_about_geometry_and_scope() {
        let model = TassadarExecutorAttentionTransformer::sudoku_v0();
        let descriptor = model.descriptor();

        assert_eq!(
            descriptor.model.model_id,
            TassadarExecutorAttentionTransformer::MODEL_ID
        );
        assert_eq!(descriptor.config.head_dim, 2);
        assert_eq!(descriptor.config.head_count, 18);
        assert_eq!(descriptor.config.layer_count, 7);
        assert_eq!(
            descriptor.claim_boundary,
            TassadarExecutorAttentionClaimBoundary::ResearchWindowedDecodeOnly
        );
        assert_eq!(
            descriptor.layer_semantics.len(),
            descriptor.config.layer_count
        );
        assert!(
            descriptor
                .layer_semantics
                .iter()
                .all(|layer| layer.hull_cache_posture
                    == TassadarExecutorAttentionHullPosture::FallbackToReferenceLinear)
        );
    }

    #[test]
    fn executor_attention_forward_logits_run_on_bounded_sequences()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let model = TassadarExecutorAttentionTransformer::sudoku_v0();
        let case = tassadar_sudoku_v0_corpus()
            .into_iter()
            .next()
            .expect("sudoku corpus should not be empty");
        let execution = psionic_runtime::TassadarCpuReferenceRunner::for_program(
            &case.validation_case.program,
        )?
        .execute(&case.validation_case.program)?;
        let sequence =
            tokenizer.tokenize_program_and_execution(&case.validation_case.program, &execution);
        let truncated = TokenSequence::new(sequence.sequence.as_slice()[..128].to_vec());
        let forward = model.forward_logits(&truncated)?;

        assert_eq!(forward.logits.len(), truncated.len() - 1);
        assert_eq!(forward.layer_contexts.len(), truncated.len() - 1);
        assert!(
            forward
                .logits
                .iter()
                .all(|step| step.len() == model.descriptor().config.vocab_size)
        );
        Ok(())
    }

    #[test]
    fn executor_attention_decode_falls_back_from_hull_to_reference_linear()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let model = TassadarExecutorAttentionTransformer::sudoku_v0();
        let prompt = TokenSequence::new(tokenizer.encode("<program> <locals>").as_slice().to_vec());
        let state = model.start_decode(prompt)?;
        let selection = model.select_decode_mode(TassadarExecutorDecodeMode::HullCache);

        assert_eq!(
            selection.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );
        assert_eq!(
            selection.fallback_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );
        let next =
            model.greedy_next_token_for_mode(&state, TassadarExecutorDecodeMode::HullCache)?;
        assert!((next.as_u32() as usize) < model.descriptor().config.vocab_size);
        Ok(())
    }

    #[test]
    fn executor_attention_transition_bias_targets_previous_token_condition() {
        let tokenizer = TassadarTraceTokenizer::new();
        let mut model = TassadarExecutorAttentionTransformer::sudoku_v0();
        let previous_token = tokenizer.encode("<step>").as_slice()[0];
        let target_token = tokenizer.encode("<step_index>").as_slice()[0];
        let vocab_size = model.descriptor().config.vocab_size;
        let relative_target_index = 1;
        let offset = (relative_target_index * vocab_size + previous_token.as_u32() as usize)
            * vocab_size
            + target_token.as_u32() as usize;
        model
            .weights_mut()
            .relative_target_transition_output_bias_mut()[offset] = 3.5;

        let mut logits = vec![0.0; vocab_size];
        model.apply_relative_target_transition_output_bias_in_place(
            logits.as_mut_slice(),
            Some(previous_token),
            relative_target_index,
        );

        assert_eq!(logits[target_token.as_u32() as usize], 3.5);
        assert!(model.has_relative_target_transition_output_bias_signal());
    }

    #[test]
    fn executor_attention_trace_schema_phase_recognizes_pc_boundary() {
        let tokenizer = TassadarTraceTokenizer::new();
        let model = TassadarExecutorAttentionTransformer::sudoku_v0();
        let prefix = tokenizer.encode(
            "<bos> <program> <locals> <byte_00> <byte_00> <byte_00> <byte_00> <memory_slots> <byte_00> <byte_00> <byte_00> <byte_00> <initial_memory> <byte_00> <byte_00> <byte_00> <byte_00> <trace> <step> <step_index> <byte_00> <byte_00> <byte_00> <byte_00>",
        );
        let initial_prompt_len = prefix.len() - 6;
        let phase = model.relative_target_trace_schema_phase_index(prefix.as_slice(), initial_prompt_len);

        assert_eq!(phase, Some(TassadarEarlyTraceSchemaPhase::ExpectPc.index()));
    }

    #[test]
    fn executor_attention_trace_schema_bias_targets_structural_boundary() {
        let tokenizer = TassadarTraceTokenizer::new();
        let mut model = TassadarExecutorAttentionTransformer::sudoku_v0();
        let target_token = tokenizer.encode("<pc>").as_slice()[0];
        let schema_phase = TassadarEarlyTraceSchemaPhase::ExpectPc.index();
        let vocab_size = model.descriptor().config.vocab_size;
        let offset = schema_phase * vocab_size + target_token.as_u32() as usize;
        model
            .weights_mut()
            .relative_target_trace_schema_output_bias_mut()[offset] = 4.0;

        let mut logits = vec![0.0; vocab_size];
        model.apply_relative_target_trace_schema_output_bias_in_place(
            logits.as_mut_slice(),
            Some(schema_phase),
        );

        assert_eq!(logits[target_token.as_u32() as usize], 4.0);
        assert!(model.has_relative_target_trace_schema_output_bias_signal());
    }
}
