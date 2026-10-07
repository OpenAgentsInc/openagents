use psionic_core::{DType, QuantizationMode, Shape};
use psionic_runtime::{TassadarExecutorDecodeMode, TassadarTraceAbi, TassadarWasmProfile};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use thiserror::Error;

use crate::{
    tassadar::{
        research_only_workload_capability_matrix, workload_class_for_tassadar_model_id,
        TassadarAttentionGeometryContract, TassadarExecutorAttentionMode, TassadarExecutorFamily,
        TassadarWorkloadCapabilityMatrix, TassadarWorkloadClass,
    },
    ModelDescriptor, TassadarTraceTokenizer, TokenId, TokenSequence, TokenizerBoundary,
    WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
};

/// Stable claim boundary for the first neural executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorTransformerClaimBoundary {
    /// The model currently claims only next-token logits over the tokenized trace domain.
    NextTokenOnly,
    /// Greedy autoregressive decode exists but remains unvalidated for exact executor claims.
    GreedyDecodeUnvalidated,
}

/// Explicit long-trace contract for one learned executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorLongTraceContract {
    /// The family expects one flat growing prefix per supervised window.
    FlatPrefixFullForward,
    /// The family is evaluated and trained through incremental bounded windows.
    IncrementalDecodeWindow,
}

impl TassadarExecutorLongTraceContract {
    /// Returns a stable label for reports and model identity.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FlatPrefixFullForward => "flat_prefix_full_forward",
            Self::IncrementalDecodeWindow => "incremental_decode_window",
        }
    }
}

/// Stable trainable-surface selector for the lookup-style executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorTrainableSurface {
    /// Train the output projection and bias only.
    OutputHeadOnly,
    /// Train the output head plus token embeddings.
    OutputHeadAndTokenEmbeddings,
    /// Train the output head plus token and position embeddings.
    OutputHeadAndEmbeddings,
    /// Train the output head, embeddings, and one small residual mixer.
    OutputHeadEmbeddingsAndSmallLearnedMixer,
}

impl TassadarExecutorTrainableSurface {
    /// Returns a stable label for file names and reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OutputHeadOnly => "output_head_only",
            Self::OutputHeadAndTokenEmbeddings => "output_head_and_token_embeddings",
            Self::OutputHeadAndEmbeddings => "output_head_and_embeddings",
            Self::OutputHeadEmbeddingsAndSmallLearnedMixer => {
                "output_head_embeddings_and_small_learned_mixer"
            }
        }
    }

    /// Returns whether token embeddings are trainable.
    #[must_use]
    pub const fn trains_token_embeddings(self) -> bool {
        matches!(
            self,
            Self::OutputHeadAndTokenEmbeddings
                | Self::OutputHeadAndEmbeddings
                | Self::OutputHeadEmbeddingsAndSmallLearnedMixer
        )
    }

    /// Returns whether position embeddings are trainable.
    #[must_use]
    pub const fn trains_position_embeddings(self) -> bool {
        matches!(
            self,
            Self::OutputHeadAndEmbeddings | Self::OutputHeadEmbeddingsAndSmallLearnedMixer
        )
    }

    /// Returns whether the small learned mixer is active and trainable.
    #[must_use]
    pub const fn trains_small_learned_mixer(self) -> bool {
        matches!(self, Self::OutputHeadEmbeddingsAndSmallLearnedMixer)
    }
}

fn default_trainable_surface() -> TassadarExecutorTrainableSurface {
    TassadarExecutorTrainableSurface::OutputHeadOnly
}

fn trainable_surface_is_output_head_only(surface: &TassadarExecutorTrainableSurface) -> bool {
    *surface == TassadarExecutorTrainableSurface::OutputHeadOnly
}

fn default_long_trace_contract() -> TassadarExecutorLongTraceContract {
    TassadarExecutorLongTraceContract::FlatPrefixFullForward
}

fn long_trace_contract_is_flat_prefix(contract: &TassadarExecutorLongTraceContract) -> bool {
    *contract == TassadarExecutorLongTraceContract::FlatPrefixFullForward
}

/// Explicit config for the first trainable neural executor family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerConfig {
    /// Executor vocabulary size.
    pub vocab_size: usize,
    /// Maximum sequence length admitted by the model.
    pub max_sequence_tokens: usize,
    /// Width of each token and position embedding.
    pub embedding_dim: usize,
    /// Fixed relative-offset lookup heads used to build one hidden state.
    pub context_offsets: Vec<usize>,
    /// Constrained lookup head dimension carried as a geometry claim.
    pub constrained_lookup_head_dim: usize,
    /// Optional prompt-summary embedding buckets for prompt-conditioned learned adapters.
    #[serde(default, skip_serializing_if = "prompt_summary_bucket_count_is_zero")]
    pub prompt_summary_bucket_count: usize,
}

impl TassadarExecutorTransformerConfig {
    /// Returns the canonical small Sudoku-v0 config.
    #[must_use]
    pub fn sudoku_v0(tokenizer: &TassadarTraceTokenizer) -> Self {
        Self {
            vocab_size: tokenizer.vocabulary().len(),
            max_sequence_tokens: 262_144,
            embedding_dim: 16,
            context_offsets: vec![1, 2, 4, 8, 16],
            constrained_lookup_head_dim: 2,
            prompt_summary_bucket_count: 0,
        }
    }

    /// Returns the larger 9x9 Sudoku-class config.
    #[must_use]
    pub fn sudoku_9x9(tokenizer: &TassadarTraceTokenizer) -> Self {
        Self {
            vocab_size: tokenizer.vocabulary().len(),
            max_sequence_tokens: 524_288,
            embedding_dim: 16,
            context_offsets: vec![1, 2, 4, 8, 16, 32],
            constrained_lookup_head_dim: 2,
            prompt_summary_bucket_count: 0,
        }
    }

    /// Returns the canonical bounded Hungarian-v0 config.
    #[must_use]
    pub fn hungarian_v0(tokenizer: &TassadarTraceTokenizer) -> Self {
        Self {
            vocab_size: tokenizer.vocabulary().len(),
            max_sequence_tokens: 262_144,
            embedding_dim: 16,
            context_offsets: vec![1, 2, 4, 8, 16, 32],
            constrained_lookup_head_dim: 2,
            prompt_summary_bucket_count: 0,
        }
    }

    /// Returns the article-sized Hungarian-10x10 config.
    #[must_use]
    pub fn hungarian_10x10(tokenizer: &TassadarTraceTokenizer) -> Self {
        Self {
            vocab_size: tokenizer.vocabulary().len(),
            max_sequence_tokens: 262_144,
            embedding_dim: 32,
            context_offsets: vec![1, 2, 4, 8, 16, 32, 64],
            constrained_lookup_head_dim: 2,
            prompt_summary_bucket_count: 4096,
        }
    }

    /// Returns the hidden-state width emitted by the fixed lookup heads plus position state.
    #[must_use]
    pub fn hidden_width(&self) -> usize {
        let prompt_summary_slots = usize::from(self.prompt_summary_bucket_count > 0);
        self.embedding_dim * (self.context_offsets.len() + 1 + prompt_summary_slots)
    }
}

fn prompt_summary_bucket_count_is_zero(count: &usize) -> bool {
    *count == 0
}

/// Explicit descriptor for the first real neural executor family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerDescriptor {
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
    pub claim_boundary: TassadarExecutorTransformerClaimBoundary,
    /// Explicit long-trace contract for this family.
    #[serde(
        default = "default_long_trace_contract",
        skip_serializing_if = "long_trace_contract_is_flat_prefix"
    )]
    pub long_trace_contract: TassadarExecutorLongTraceContract,
    /// Optional direct SparseTopK decode budget for families that advertise a
    /// sparse baseline directly instead of falling back to reference-linear.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sparse_top_k: Option<u16>,
    /// Active trainable surface carried by the descriptor.
    #[serde(
        default = "default_trainable_surface",
        skip_serializing_if = "trainable_surface_is_output_head_only"
    )]
    pub trainable_surface: TassadarExecutorTrainableSurface,
    /// Model config.
    pub config: TassadarExecutorTransformerConfig,
    /// Weight bundle metadata.
    pub weights: WeightBundleMetadata,
}

impl TassadarExecutorTransformerDescriptor {
    /// Returns a stable digest over the descriptor.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_tassadar_executor_transformer_descriptor|", self)
    }
}

/// Initial deterministic weight bundle for the first neural executor model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerWeightBundle {
    metadata: WeightBundleMetadata,
    token_embeddings: Vec<f32>,
    position_embeddings: Vec<f32>,
    output_projection: Vec<f32>,
    output_bias: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    prompt_summary_embeddings: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    prompt_summary_target_output_bias_rows: Vec<TassadarPromptSummaryTargetOutputBiasRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    relative_target_trace_schema_output_bias: Vec<f32>,
    small_learned_mixer_projection: Vec<f32>,
    small_learned_mixer_bias: Vec<f32>,
    head_offsets: Vec<f32>,
}

/// One prompt-conditioned target-position output-bias override row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarPromptSummaryTargetOutputBiasRow {
    /// Stable prompt-summary bucket selected from the prompt prefix.
    pub prompt_summary_bucket: u32,
    /// Zero-based target position after the prompt boundary.
    pub relative_target_position: u32,
    /// Vocabulary-sized bias row for that prompt and target position.
    pub values: Vec<f32>,
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

fn flatten_prompt_summary_target_output_bias_rows(
    rows: &[TassadarPromptSummaryTargetOutputBiasRow],
) -> (Vec<f32>, Vec<f32>) {
    let mut keys = Vec::with_capacity(rows.len().saturating_mul(2));
    let mut values = Vec::new();
    for row in rows {
        keys.push(row.prompt_summary_bucket as f32);
        keys.push(row.relative_target_position as f32);
        values.extend_from_slice(row.values.as_slice());
    }
    (keys, values)
}

impl TassadarExecutorTransformerWeightBundle {
    fn new(
        config: &TassadarExecutorTransformerConfig,
        trainable_surface: TassadarExecutorTrainableSurface,
    ) -> Self {
        let token_embeddings = seeded_values(
            "token_embeddings",
            config.vocab_size * config.embedding_dim,
            0.125,
        );
        let position_embeddings = seeded_values(
            "position_embeddings",
            config.max_sequence_tokens * config.embedding_dim,
            0.1,
        );
        let output_projection = seeded_values(
            "output_projection",
            config.hidden_width() * config.vocab_size,
            0.08,
        );
        let output_bias = vec![0.0; config.vocab_size];
        let prompt_summary_embeddings =
            vec![0.0; config.prompt_summary_bucket_count * config.embedding_dim];
        let prompt_summary_target_output_bias_rows = Vec::new();
        let relative_target_trace_schema_output_bias = Vec::new();
        let small_learned_mixer_projection =
            vec![0.0; config.hidden_width() * config.hidden_width()];
        let small_learned_mixer_bias = vec![0.0; config.hidden_width()];
        let head_offsets = config
            .context_offsets
            .iter()
            .map(|offset| *offset as f32)
            .collect::<Vec<_>>();
        let prompt_summary_target_output_bias_tensors = (!prompt_summary_target_output_bias_rows
            .is_empty())
        .then(|| {
            flatten_prompt_summary_target_output_bias_rows(&prompt_summary_target_output_bias_rows)
        });

        let mut entries = vec![
            (
                WeightTensorMetadata::new(
                    "token_embeddings",
                    Shape::new(vec![config.vocab_size, config.embedding_dim]),
                    DType::F32,
                ),
                token_embeddings.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "position_embeddings",
                    Shape::new(vec![config.max_sequence_tokens, config.embedding_dim]),
                    DType::F32,
                ),
                position_embeddings.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "output_projection",
                    Shape::new(vec![config.hidden_width(), config.vocab_size]),
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
            (
                WeightTensorMetadata::new(
                    "head_offsets",
                    Shape::new(vec![config.context_offsets.len()]),
                    DType::F32,
                ),
                head_offsets.as_slice(),
            ),
        ];
        if !prompt_summary_embeddings.is_empty() {
            entries.push((
                WeightTensorMetadata::new(
                    "prompt_summary_embeddings",
                    Shape::new(vec![
                        config.prompt_summary_bucket_count,
                        config.embedding_dim,
                    ]),
                    DType::F32,
                ),
                prompt_summary_embeddings.as_slice(),
            ));
        }
        if let Some((keys, values)) = prompt_summary_target_output_bias_tensors.as_ref() {
            entries.extend([
                (
                    WeightTensorMetadata::new(
                        "prompt_summary_target_output_bias_row_keys",
                        Shape::new(vec![prompt_summary_target_output_bias_rows.len(), 2]),
                        DType::F32,
                    ),
                    keys.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        "prompt_summary_target_output_bias_row_values",
                        Shape::new(vec![
                            prompt_summary_target_output_bias_rows.len(),
                            config.vocab_size,
                        ]),
                        DType::F32,
                    ),
                    values.as_slice(),
                ),
            ]);
        }
        if !relative_target_trace_schema_output_bias.is_empty() {
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
        if trainable_surface.trains_small_learned_mixer() {
            entries.extend([
                (
                    WeightTensorMetadata::new(
                        "small_learned_mixer_projection",
                        Shape::new(vec![config.hidden_width(), config.hidden_width()]),
                        DType::F32,
                    ),
                    small_learned_mixer_projection.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        "small_learned_mixer_bias",
                        Shape::new(vec![config.hidden_width()]),
                        DType::F32,
                    ),
                    small_learned_mixer_bias.as_slice(),
                ),
            ]);
        }

        Self {
            metadata: build_metadata(entries.as_slice()),
            token_embeddings,
            position_embeddings,
            output_projection,
            output_bias,
            prompt_summary_embeddings,
            prompt_summary_target_output_bias_rows,
            relative_target_trace_schema_output_bias,
            small_learned_mixer_projection,
            small_learned_mixer_bias,
            head_offsets,
        }
    }

    /// Returns the stable bundle metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightBundleMetadata {
        &self.metadata
    }

    /// Returns mutable token embeddings for later training updates.
    pub fn token_embeddings_mut(&mut self) -> &mut [f32] {
        &mut self.token_embeddings
    }

    /// Returns the current token embeddings.
    #[must_use]
    pub fn token_embeddings(&self) -> &[f32] {
        &self.token_embeddings
    }

    /// Returns mutable position embeddings for later training updates.
    pub fn position_embeddings_mut(&mut self) -> &mut [f32] {
        &mut self.position_embeddings
    }

    /// Returns the current position embeddings.
    #[must_use]
    pub fn position_embeddings(&self) -> &[f32] {
        &self.position_embeddings
    }

    /// Returns mutable output projection weights for later training updates.
    pub fn output_projection_mut(&mut self) -> &mut [f32] {
        &mut self.output_projection
    }

    /// Returns the trained output projection.
    #[must_use]
    pub fn output_projection(&self) -> &[f32] {
        &self.output_projection
    }

    /// Returns mutable output bias weights for later training updates.
    pub fn output_bias_mut(&mut self) -> &mut [f32] {
        &mut self.output_bias
    }

    /// Returns the trained output bias.
    #[must_use]
    pub fn output_bias(&self) -> &[f32] {
        &self.output_bias
    }

    /// Returns mutable prompt-summary embeddings for prompt-conditioned adapters.
    pub fn prompt_summary_embeddings_mut(&mut self) -> &mut [f32] {
        &mut self.prompt_summary_embeddings
    }

    /// Returns the prompt-summary embeddings.
    #[must_use]
    pub fn prompt_summary_embeddings(&self) -> &[f32] {
        &self.prompt_summary_embeddings
    }

    /// Returns mutable prompt-conditioned target-position output-bias rows.
    pub fn prompt_summary_target_output_bias_rows_mut(
        &mut self,
    ) -> &mut Vec<TassadarPromptSummaryTargetOutputBiasRow> {
        &mut self.prompt_summary_target_output_bias_rows
    }

    /// Returns the prompt-conditioned target-position output-bias rows.
    #[must_use]
    pub fn prompt_summary_target_output_bias_rows(
        &self,
    ) -> &[TassadarPromptSummaryTargetOutputBiasRow] {
        &self.prompt_summary_target_output_bias_rows
    }

    /// Returns mutable trace-schema-conditioned output-bias adapter weights.
    pub fn relative_target_trace_schema_output_bias_mut(&mut self) -> &mut [f32] {
        &mut self.relative_target_trace_schema_output_bias
    }

    /// Returns the trace-schema-conditioned output-bias adapter weights.
    #[must_use]
    pub fn relative_target_trace_schema_output_bias(&self) -> &[f32] {
        &self.relative_target_trace_schema_output_bias
    }

    /// Returns mutable residual-mixer projection weights for later training updates.
    pub fn small_learned_mixer_projection_mut(&mut self) -> &mut [f32] {
        &mut self.small_learned_mixer_projection
    }

    /// Returns the residual-mixer projection.
    #[must_use]
    pub fn small_learned_mixer_projection(&self) -> &[f32] {
        &self.small_learned_mixer_projection
    }

    /// Returns mutable residual-mixer bias weights for later training updates.
    pub fn small_learned_mixer_bias_mut(&mut self) -> &mut [f32] {
        &mut self.small_learned_mixer_bias
    }

    /// Returns the residual-mixer bias.
    #[must_use]
    pub fn small_learned_mixer_bias(&self) -> &[f32] {
        &self.small_learned_mixer_bias
    }

    fn refresh_metadata(
        &mut self,
        config: &TassadarExecutorTransformerConfig,
        trainable_surface: TassadarExecutorTrainableSurface,
    ) {
        let mut entries = vec![
            (
                WeightTensorMetadata::new(
                    "token_embeddings",
                    Shape::new(vec![config.vocab_size, config.embedding_dim]),
                    DType::F32,
                ),
                self.token_embeddings.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "position_embeddings",
                    Shape::new(vec![config.max_sequence_tokens, config.embedding_dim]),
                    DType::F32,
                ),
                self.position_embeddings.as_slice(),
            ),
            (
                WeightTensorMetadata::new(
                    "output_projection",
                    Shape::new(vec![config.hidden_width(), config.vocab_size]),
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
            (
                WeightTensorMetadata::new(
                    "head_offsets",
                    Shape::new(vec![config.context_offsets.len()]),
                    DType::F32,
                ),
                self.head_offsets.as_slice(),
            ),
        ];
        if !self.prompt_summary_embeddings.is_empty() {
            entries.push((
                WeightTensorMetadata::new(
                    "prompt_summary_embeddings",
                    Shape::new(vec![
                        config.prompt_summary_bucket_count,
                        config.embedding_dim,
                    ]),
                    DType::F32,
                ),
                self.prompt_summary_embeddings.as_slice(),
            ));
        }
        let prompt_summary_target_output_bias_tensors =
            (!self.prompt_summary_target_output_bias_rows.is_empty()).then(|| {
                flatten_prompt_summary_target_output_bias_rows(
                    &self.prompt_summary_target_output_bias_rows,
                )
            });
        if let Some((keys, values)) = prompt_summary_target_output_bias_tensors.as_ref() {
            entries.extend([
                (
                    WeightTensorMetadata::new(
                        "prompt_summary_target_output_bias_row_keys",
                        Shape::new(vec![self.prompt_summary_target_output_bias_rows.len(), 2]),
                        DType::F32,
                    ),
                    keys.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        "prompt_summary_target_output_bias_row_values",
                        Shape::new(vec![
                            self.prompt_summary_target_output_bias_rows.len(),
                            config.vocab_size,
                        ]),
                        DType::F32,
                    ),
                    values.as_slice(),
                ),
            ]);
        }
        if !self.relative_target_trace_schema_output_bias.is_empty() {
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
        if trainable_surface.trains_small_learned_mixer() {
            entries.extend([
                (
                    WeightTensorMetadata::new(
                        "small_learned_mixer_projection",
                        Shape::new(vec![config.hidden_width(), config.hidden_width()]),
                        DType::F32,
                    ),
                    self.small_learned_mixer_projection.as_slice(),
                ),
                (
                    WeightTensorMetadata::new(
                        "small_learned_mixer_bias",
                        Shape::new(vec![config.hidden_width()]),
                        DType::F32,
                    ),
                    self.small_learned_mixer_bias.as_slice(),
                ),
            ]);
        }
        self.metadata = build_metadata(entries.as_slice());
    }
}

/// Hidden-state and logits emitted by one forward pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerForwardPass {
    /// Lookup-only hidden state before any optional learned mixer.
    pub source_hidden_states: Vec<Vec<f32>>,
    /// Hidden state for each next-token prediction position.
    pub hidden_states: Vec<Vec<f32>>,
    /// Vocabulary logits for each prediction position.
    pub logits: Vec<Vec<f32>>,
    /// Context tokens and positions used to build each hidden state.
    pub step_contexts: Vec<TassadarExecutorTransformerStepContext>,
}

/// One step-local context used to build a hidden state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerStepContext {
    /// Context tokens selected by the lookup heads.
    pub context_tokens: Vec<TokenId>,
    /// Position index used for the position embedding.
    pub position: u32,
    /// Prompt-summary bucket used for prompt-conditioned learned adapters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_summary_bucket: Option<usize>,
}

/// Decode refusal for the neural executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorTransformerDecodeRefusal {
    /// No supported decode path exists for the requested mode.
    NoSupportedDecodeMode,
}

/// Machine-legible decode selection for one requested model decode path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerDecodeSelection {
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
    pub refusal: Option<TassadarExecutorTransformerDecodeRefusal>,
    /// Decode modes surfaced by the descriptor.
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
}

/// One explicit KV point owned by the trained executor model decode state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerKvPoint {
    /// Zero-based token position in the prefix.
    pub position: u32,
    /// Token id stored at this position.
    pub token_id: TokenId,
    /// First key component used by the 2D lookup query.
    pub key_x: i64,
    /// Second key component used by the 2D lookup query.
    pub key_y: i64,
}

/// Typed decode state for linear autoregressive execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerDecodeState {
    /// Prefix tokens visible to the next decode step.
    pub prefix: TokenSequence,
    /// Prompt-token count used to detect bounded early target schema phases.
    #[serde(default)]
    pub initial_prompt_len: usize,
    /// Prompt-summary bucket used for prompt-conditioned learned adapters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_summary_bucket: Option<usize>,
    /// Next decode position.
    pub next_position: usize,
    /// Explicit KV points visible to the next decode step.
    pub kv_points: Vec<TassadarExecutorTransformerKvPoint>,
}

/// Hidden-state and logits emitted for one decode step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerDecodeStep {
    /// Lookup-only hidden state before any optional learned mixer.
    pub source_hidden_state: Vec<f32>,
    /// Hidden state for the next-token prediction.
    pub hidden_state: Vec<f32>,
    /// Vocabulary logits for the next-token prediction.
    pub logits: Vec<f32>,
    /// Context tokens and position used to build the hidden state.
    pub step_context: TassadarExecutorTransformerStepContext,
}

/// First honest neural executor family in Psionic.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarExecutorTransformer {
    descriptor: TassadarExecutorTransformerDescriptor,
    tokenizer: TassadarTraceTokenizer,
    weights: TassadarExecutorTransformerWeightBundle,
}

impl TassadarExecutorTransformer {
    /// Stable model identifier for the first Sudoku-v0 executor family.
    pub const MODEL_ID: &str = "tassadar-executor-transformer-sudoku-v0-v0";
    /// Stable model identifier for the first 9x9 Sudoku-class executor family.
    pub const SUDOKU_9X9_MODEL_ID: &str = "tassadar-executor-transformer-sudoku-9x9-v0";
    /// Stable model identifier for the first Hungarian-v0 executor family.
    pub const HUNGARIAN_V0_MODEL_ID: &str = "tassadar-executor-transformer-hungarian-v0-v0";
    /// Stable model identifier for the first Hungarian-10x10 executor family.
    pub const HUNGARIAN_10X10_MODEL_ID: &str = "tassadar-executor-transformer-hungarian-10x10-v0";
    /// Stable model identifier for the first windowed Sudoku-v0 executor family.
    pub const WINDOWED_MODEL_ID: &str = "tassadar-executor-transformer-sudoku-v0-windowed-v0";
    /// Stable model identifier for the first windowed 9x9 executor family.
    pub const WINDOWED_SUDOKU_9X9_MODEL_ID: &str =
        "tassadar-executor-transformer-sudoku-9x9-windowed-v0";
    /// Stable model identifier for the first windowed Hungarian-v0 executor family.
    pub const WINDOWED_HUNGARIAN_V0_MODEL_ID: &str =
        "tassadar-executor-transformer-hungarian-v0-windowed-v0";
    /// Stable model identifier for the first windowed Hungarian-10x10 executor family.
    pub const WINDOWED_HUNGARIAN_10X10_MODEL_ID: &str =
        "tassadar-executor-transformer-hungarian-10x10-windowed-v0";
    /// Stable model identifier for the first direct sparse Sudoku-v0 executor family.
    pub const SPARSE_MODEL_ID: &str = "tassadar-executor-transformer-sudoku-v0-sparse-v0";
    /// Stable model family label.
    pub const MODEL_FAMILY: &str = "tassadar_executor_transformer";
    /// Stable model family label for explicit windowed replay.
    pub const WINDOWED_MODEL_FAMILY: &str = "tassadar_executor_windowed_transformer";
    /// Stable model family label for the direct sparse baseline.
    pub const SPARSE_MODEL_FAMILY: &str = "tassadar_executor_sparse_transformer";

    /// Creates the canonical small Sudoku-v0 executor transformer.
    #[must_use]
    pub fn sudoku_v0() -> Self {
        Self::sudoku_v0_with_surface(TassadarExecutorTrainableSurface::OutputHeadOnly)
    }

    /// Creates the canonical small Sudoku-v0 executor transformer for one surface.
    #[must_use]
    pub fn sudoku_v0_with_surface(trainable_surface: TassadarExecutorTrainableSurface) -> Self {
        Self::sudoku_v0_with_surface_and_contract(
            trainable_surface,
            TassadarExecutorLongTraceContract::FlatPrefixFullForward,
        )
    }

    /// Creates the canonical small Sudoku-v0 windowed executor transformer for one surface.
    #[must_use]
    pub fn sudoku_v0_windowed_with_surface(
        trainable_surface: TassadarExecutorTrainableSurface,
    ) -> Self {
        Self::sudoku_v0_with_surface_and_contract(
            trainable_surface,
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow,
        )
    }

    /// Creates the canonical small Sudoku-v0 direct sparse-top-k baseline for
    /// one trainable surface.
    #[must_use]
    pub fn sudoku_v0_sparse_with_surface(
        trainable_surface: TassadarExecutorTrainableSurface,
    ) -> Self {
        let tokenizer = TassadarTraceTokenizer::new();
        let config = TassadarExecutorTransformerConfig::sudoku_v0(&tokenizer);
        let weights = TassadarExecutorTransformerWeightBundle::new(&config, trainable_surface);
        let descriptor = TassadarExecutorTransformerDescriptor {
            model: ModelDescriptor::new(Self::SPARSE_MODEL_ID, Self::SPARSE_MODEL_FAMILY, "v0"),
            executor_family: TassadarExecutorFamily::WasmTraceExecutor,
            profile: TassadarWasmProfile::sudoku_v0_search_v1(),
            trace_abi: TassadarTraceAbi::sudoku_v0_search_v1(),
            supported_decode_modes: vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::SparseTopK,
            ],
            attention_mode: TassadarExecutorAttentionMode::HardMaxLookup,
            attention_geometry: TassadarAttentionGeometryContract {
                constrained_lookup_head_dim: Some(config.constrained_lookup_head_dim),
                hull_cache_eligible: false,
            },
            claim_boundary: TassadarExecutorTransformerClaimBoundary::NextTokenOnly,
            long_trace_contract: TassadarExecutorLongTraceContract::FlatPrefixFullForward,
            sparse_top_k: Some(4),
            trainable_surface,
            config,
            weights: weights.metadata().clone(),
        };
        Self {
            descriptor,
            tokenizer,
            weights,
        }
    }

    fn sudoku_v0_with_surface_and_contract(
        trainable_surface: TassadarExecutorTrainableSurface,
        long_trace_contract: TassadarExecutorLongTraceContract,
    ) -> Self {
        let tokenizer = TassadarTraceTokenizer::new();
        let config = TassadarExecutorTransformerConfig::sudoku_v0(&tokenizer);
        let weights = TassadarExecutorTransformerWeightBundle::new(&config, trainable_surface);
        let (model_id, model_family) = match long_trace_contract {
            TassadarExecutorLongTraceContract::FlatPrefixFullForward => {
                (Self::MODEL_ID, Self::MODEL_FAMILY)
            }
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow => {
                (Self::WINDOWED_MODEL_ID, Self::WINDOWED_MODEL_FAMILY)
            }
        };
        let descriptor = TassadarExecutorTransformerDescriptor {
            model: ModelDescriptor::new(model_id, model_family, "v0"),
            executor_family: TassadarExecutorFamily::WasmTraceExecutor,
            profile: TassadarWasmProfile::sudoku_v0_search_v1(),
            trace_abi: TassadarTraceAbi::sudoku_v0_search_v1(),
            supported_decode_modes: vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
            ],
            attention_mode: TassadarExecutorAttentionMode::HardMaxLookup,
            attention_geometry: TassadarAttentionGeometryContract {
                constrained_lookup_head_dim: Some(config.constrained_lookup_head_dim),
                hull_cache_eligible: true,
            },
            claim_boundary: TassadarExecutorTransformerClaimBoundary::NextTokenOnly,
            long_trace_contract,
            sparse_top_k: None,
            trainable_surface,
            config,
            weights: weights.metadata().clone(),
        };
        Self {
            descriptor,
            tokenizer,
            weights,
        }
    }

    /// Creates the first 9x9 Sudoku-class executor transformer.
    #[must_use]
    pub fn sudoku_9x9() -> Self {
        Self::sudoku_9x9_with_surface(TassadarExecutorTrainableSurface::OutputHeadOnly)
    }

    /// Creates the first 9x9 Sudoku-class executor transformer for one surface.
    #[must_use]
    pub fn sudoku_9x9_with_surface(trainable_surface: TassadarExecutorTrainableSurface) -> Self {
        Self::sudoku_9x9_with_surface_and_contract(
            trainable_surface,
            TassadarExecutorLongTraceContract::FlatPrefixFullForward,
        )
    }

    /// Creates the first explicit 9x9 windowed executor transformer for one surface.
    #[must_use]
    pub fn sudoku_9x9_windowed_with_surface(
        trainable_surface: TassadarExecutorTrainableSurface,
    ) -> Self {
        Self::sudoku_9x9_with_surface_and_contract(
            trainable_surface,
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow,
        )
    }

    fn sudoku_9x9_with_surface_and_contract(
        trainable_surface: TassadarExecutorTrainableSurface,
        long_trace_contract: TassadarExecutorLongTraceContract,
    ) -> Self {
        let tokenizer = TassadarTraceTokenizer::new();
        let config = TassadarExecutorTransformerConfig::sudoku_9x9(&tokenizer);
        let weights = TassadarExecutorTransformerWeightBundle::new(&config, trainable_surface);
        let (model_id, model_family) = match long_trace_contract {
            TassadarExecutorLongTraceContract::FlatPrefixFullForward => {
                (Self::SUDOKU_9X9_MODEL_ID, Self::MODEL_FAMILY)
            }
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow => (
                Self::WINDOWED_SUDOKU_9X9_MODEL_ID,
                Self::WINDOWED_MODEL_FAMILY,
            ),
        };
        let descriptor = TassadarExecutorTransformerDescriptor {
            model: ModelDescriptor::new(model_id, model_family, "v0"),
            executor_family: TassadarExecutorFamily::WasmTraceExecutor,
            profile: TassadarWasmProfile::sudoku_9x9_search_v1(),
            trace_abi: TassadarTraceAbi::sudoku_9x9_search_v1(),
            supported_decode_modes: vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
            ],
            attention_mode: TassadarExecutorAttentionMode::HardMaxLookup,
            attention_geometry: TassadarAttentionGeometryContract {
                constrained_lookup_head_dim: Some(config.constrained_lookup_head_dim),
                hull_cache_eligible: true,
            },
            claim_boundary: TassadarExecutorTransformerClaimBoundary::NextTokenOnly,
            long_trace_contract,
            sparse_top_k: None,
            trainable_surface,
            config,
            weights: weights.metadata().clone(),
        };
        Self {
            descriptor,
            tokenizer,
            weights,
        }
    }

    /// Creates the first bounded Hungarian-v0 executor transformer.
    #[must_use]
    pub fn hungarian_v0() -> Self {
        Self::hungarian_v0_with_surface(TassadarExecutorTrainableSurface::OutputHeadOnly)
    }

    /// Creates the first bounded Hungarian-v0 executor transformer for one surface.
    #[must_use]
    pub fn hungarian_v0_with_surface(trainable_surface: TassadarExecutorTrainableSurface) -> Self {
        Self::hungarian_v0_with_surface_and_contract(
            trainable_surface,
            TassadarExecutorLongTraceContract::FlatPrefixFullForward,
        )
    }

    /// Creates the first explicit Hungarian-v0 windowed executor transformer for one surface.
    #[must_use]
    pub fn hungarian_v0_windowed_with_surface(
        trainable_surface: TassadarExecutorTrainableSurface,
    ) -> Self {
        Self::hungarian_v0_with_surface_and_contract(
            trainable_surface,
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow,
        )
    }

    /// Creates the first article-sized Hungarian-10x10 executor transformer.
    #[must_use]
    pub fn hungarian_10x10() -> Self {
        Self::hungarian_10x10_with_surface(TassadarExecutorTrainableSurface::OutputHeadOnly)
    }

    /// Creates the first article-sized Hungarian-10x10 executor transformer for one surface.
    #[must_use]
    pub fn hungarian_10x10_with_surface(
        trainable_surface: TassadarExecutorTrainableSurface,
    ) -> Self {
        Self::hungarian_10x10_with_surface_and_contract(
            trainable_surface,
            TassadarExecutorLongTraceContract::FlatPrefixFullForward,
        )
    }

    /// Creates the first explicit windowed Hungarian-10x10 executor transformer for one surface.
    #[must_use]
    pub fn hungarian_10x10_windowed_with_surface(
        trainable_surface: TassadarExecutorTrainableSurface,
    ) -> Self {
        Self::hungarian_10x10_with_surface_and_contract(
            trainable_surface,
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow,
        )
    }

    fn hungarian_v0_with_surface_and_contract(
        trainable_surface: TassadarExecutorTrainableSurface,
        long_trace_contract: TassadarExecutorLongTraceContract,
    ) -> Self {
        let tokenizer = TassadarTraceTokenizer::new();
        let config = TassadarExecutorTransformerConfig::hungarian_v0(&tokenizer);
        let weights = TassadarExecutorTransformerWeightBundle::new(&config, trainable_surface);
        let (model_id, model_family) = match long_trace_contract {
            TassadarExecutorLongTraceContract::FlatPrefixFullForward => {
                (Self::HUNGARIAN_V0_MODEL_ID, Self::MODEL_FAMILY)
            }
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow => (
                Self::WINDOWED_HUNGARIAN_V0_MODEL_ID,
                Self::WINDOWED_MODEL_FAMILY,
            ),
        };
        let descriptor = TassadarExecutorTransformerDescriptor {
            model: ModelDescriptor::new(model_id, model_family, "v0"),
            executor_family: TassadarExecutorFamily::WasmTraceExecutor,
            profile: TassadarWasmProfile::hungarian_v0_matching_v1(),
            trace_abi: TassadarTraceAbi::hungarian_v0_matching_v1(),
            supported_decode_modes: vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
            ],
            attention_mode: TassadarExecutorAttentionMode::HardMaxLookup,
            attention_geometry: TassadarAttentionGeometryContract {
                constrained_lookup_head_dim: Some(config.constrained_lookup_head_dim),
                hull_cache_eligible: true,
            },
            claim_boundary: TassadarExecutorTransformerClaimBoundary::NextTokenOnly,
            long_trace_contract,
            sparse_top_k: None,
            trainable_surface,
            config,
            weights: weights.metadata().clone(),
        };
        Self {
            descriptor,
            tokenizer,
            weights,
        }
    }

    fn hungarian_10x10_with_surface_and_contract(
        trainable_surface: TassadarExecutorTrainableSurface,
        long_trace_contract: TassadarExecutorLongTraceContract,
    ) -> Self {
        let tokenizer = TassadarTraceTokenizer::new();
        let config = TassadarExecutorTransformerConfig::hungarian_10x10(&tokenizer);
        let weights = TassadarExecutorTransformerWeightBundle::new(&config, trainable_surface);
        let (model_id, model_family) = match long_trace_contract {
            TassadarExecutorLongTraceContract::FlatPrefixFullForward => {
                (Self::HUNGARIAN_10X10_MODEL_ID, Self::MODEL_FAMILY)
            }
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow => (
                Self::WINDOWED_HUNGARIAN_10X10_MODEL_ID,
                Self::WINDOWED_MODEL_FAMILY,
            ),
        };
        let descriptor = TassadarExecutorTransformerDescriptor {
            model: ModelDescriptor::new(model_id, model_family, "v0"),
            executor_family: TassadarExecutorFamily::WasmTraceExecutor,
            profile: TassadarWasmProfile::hungarian_10x10_matching_v1(),
            trace_abi: TassadarTraceAbi::hungarian_10x10_matching_v1(),
            supported_decode_modes: vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
            ],
            attention_mode: TassadarExecutorAttentionMode::HardMaxLookup,
            attention_geometry: TassadarAttentionGeometryContract {
                constrained_lookup_head_dim: Some(config.constrained_lookup_head_dim),
                hull_cache_eligible: true,
            },
            claim_boundary: TassadarExecutorTransformerClaimBoundary::NextTokenOnly,
            long_trace_contract,
            sparse_top_k: None,
            trainable_surface,
            config,
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
    pub fn descriptor(&self) -> &TassadarExecutorTransformerDescriptor {
        &self.descriptor
    }

    /// Returns the tokenizer.
    #[must_use]
    pub fn tokenizer(&self) -> &TassadarTraceTokenizer {
        &self.tokenizer
    }

    /// Returns the mutable weights for later training phases.
    pub fn weights_mut(&mut self) -> &mut TassadarExecutorTransformerWeightBundle {
        &mut self.weights
    }

    /// Returns the stable weights.
    #[must_use]
    pub fn weights(&self) -> &TassadarExecutorTransformerWeightBundle {
        &self.weights
    }

    /// Enables the bounded early trace-schema output-bias adapter when absent.
    pub fn ensure_relative_target_trace_schema_output_bias(&mut self) {
        if !self
            .weights
            .relative_target_trace_schema_output_bias
            .is_empty()
        {
            return;
        }
        self.weights.relative_target_trace_schema_output_bias =
            vec![0.0; TassadarEarlyTraceSchemaPhase::COUNT * self.descriptor.config.vocab_size];
    }

    /// Returns whether prompt-summary embeddings are configured for this model.
    #[must_use]
    pub fn has_prompt_summary_embeddings(&self) -> bool {
        self.descriptor.config.prompt_summary_bucket_count > 0
            && !self.weights.prompt_summary_embeddings.is_empty()
    }

    /// Returns the prompt-summary bucket for one prompt prefix.
    #[must_use]
    pub fn prompt_summary_bucket_for_prompt(&self, prompt: &[TokenId]) -> Option<usize> {
        let bucket_count = self.descriptor.config.prompt_summary_bucket_count;
        if bucket_count == 0 {
            return None;
        }
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_tassadar_prompt_summary_bucket|");
        for token in prompt {
            hasher.update(token.as_u32().to_le_bytes());
        }
        let digest = hasher.finalize();
        let mut bucket_seed = [0_u8; 8];
        bucket_seed.copy_from_slice(&digest[..8]);
        Some((u64::from_le_bytes(bucket_seed) % bucket_count as u64) as usize)
    }

    /// Ensures one prompt-conditioned target-position output-bias row exists.
    pub fn ensure_prompt_summary_target_output_bias_row(
        &mut self,
        prompt_summary_bucket: usize,
        relative_target_position: usize,
    ) {
        if self
            .prompt_summary_target_output_bias_row_index(
                prompt_summary_bucket,
                relative_target_position,
            )
            .is_some()
        {
            return;
        }
        self.weights
            .prompt_summary_target_output_bias_rows_mut()
            .push(TassadarPromptSummaryTargetOutputBiasRow {
                prompt_summary_bucket: prompt_summary_bucket as u32,
                relative_target_position: relative_target_position as u32,
                values: vec![0.0; self.descriptor.config.vocab_size],
            });
    }

    /// Returns the row index for one prompt-conditioned target-position output-bias row.
    #[must_use]
    pub fn prompt_summary_target_output_bias_row_index(
        &self,
        prompt_summary_bucket: usize,
        relative_target_position: usize,
    ) -> Option<usize> {
        self.weights
            .prompt_summary_target_output_bias_rows()
            .iter()
            .position(|row| {
                row.prompt_summary_bucket as usize == prompt_summary_bucket
                    && row.relative_target_position as usize == relative_target_position
            })
    }

    /// Returns whether the bounded early trace-schema adapter has any non-zero trained signal.
    #[must_use]
    pub fn has_relative_target_trace_schema_output_bias_signal(&self) -> bool {
        self.weights
            .relative_target_trace_schema_output_bias
            .iter()
            .any(|value| value.abs() > 1e-6)
    }

    /// Returns the active trainable surface.
    #[must_use]
    pub const fn trainable_surface(&self) -> TassadarExecutorTrainableSurface {
        self.descriptor.trainable_surface
    }

    /// Returns whether the descriptor advertises one decode mode.
    #[must_use]
    pub fn supports_decode_mode(&self, decode_mode: TassadarExecutorDecodeMode) -> bool {
        self.descriptor
            .supported_decode_modes
            .contains(&decode_mode)
    }

    /// Returns the workload capability matrix for this learned executor family.
    #[must_use]
    pub fn workload_capability_matrix(&self) -> TassadarWorkloadCapabilityMatrix {
        let workload_class =
            workload_class_for_tassadar_model_id(self.descriptor.model.model_id.as_str())
                .unwrap_or(TassadarWorkloadClass::SudokuClass);
        let claim_boundary = format!(
            "learned executor family remains {} under the {} contract; served capability publication stays closed until exact executor promotion lands",
            match self.descriptor.claim_boundary {
                TassadarExecutorTransformerClaimBoundary::NextTokenOnly => "next-token-only",
                TassadarExecutorTransformerClaimBoundary::GreedyDecodeUnvalidated => {
                    "greedy-decode-unvalidated"
                }
            },
            self.descriptor.long_trace_contract.label(),
        );
        let detail = format!(
            "learned {} lane stays research-only on `{}` until exact executor promotion and benchmark-backed served publication land",
            workload_class.as_str(),
            self.descriptor.model.model_id,
        );
        research_only_workload_capability_matrix(
            &self.descriptor.model,
            self.descriptor.supported_decode_modes.as_slice(),
            workload_class,
            claim_boundary,
            detail,
        )
    }

    /// Resolves one requested decode mode into an effective path or refusal.
    #[must_use]
    pub fn select_decode_mode(
        &self,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> TassadarExecutorTransformerDecodeSelection {
        let supported_decode_modes = self.descriptor.supported_decode_modes.clone();
        if self.supports_decode_mode(requested_decode_mode) {
            return TassadarExecutorTransformerDecodeSelection {
                requested_decode_mode,
                effective_decode_mode: Some(requested_decode_mode),
                fallback_decode_mode: None,
                refusal: None,
                supported_decode_modes,
            };
        }
        if self.supports_decode_mode(TassadarExecutorDecodeMode::ReferenceLinear) {
            return TassadarExecutorTransformerDecodeSelection {
                requested_decode_mode,
                effective_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
                fallback_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
                refusal: None,
                supported_decode_modes,
            };
        }
        TassadarExecutorTransformerDecodeSelection {
            requested_decode_mode,
            effective_decode_mode: None,
            fallback_decode_mode: None,
            refusal: Some(TassadarExecutorTransformerDecodeRefusal::NoSupportedDecodeMode),
            supported_decode_modes,
        }
    }

    /// Refreshes the descriptor metadata after in-place training updates.
    pub fn refresh_after_training(&mut self) {
        self.weights
            .refresh_metadata(&self.descriptor.config, self.descriptor.trainable_surface);
        self.descriptor.weights = self.weights.metadata().clone();
        self.descriptor.claim_boundary =
            TassadarExecutorTransformerClaimBoundary::GreedyDecodeUnvalidated;
    }

    /// Applies a trained output head onto the deterministic base model.
    pub fn apply_trained_output_head(
        &mut self,
        output_projection: &[f32],
        output_bias: &[f32],
    ) -> Result<(), TassadarExecutorTransformerError> {
        if output_projection.len() != self.weights.output_projection.len() {
            return Err(TassadarExecutorTransformerError::WeightLengthMismatch {
                tensor: String::from("output_projection"),
                expected: self.weights.output_projection.len(),
                actual: output_projection.len(),
            });
        }
        if output_bias.len() != self.weights.output_bias.len() {
            return Err(TassadarExecutorTransformerError::WeightLengthMismatch {
                tensor: String::from("output_bias"),
                expected: self.weights.output_bias.len(),
                actual: output_bias.len(),
            });
        }
        self.weights
            .output_projection
            .copy_from_slice(output_projection);
        self.weights.output_bias.copy_from_slice(output_bias);
        self.refresh_after_training();
        Ok(())
    }

    /// Runs a next-token forward pass over one tokenized executor sequence.
    pub fn forward_logits(
        &self,
        sequence: &TokenSequence,
    ) -> Result<TassadarExecutorTransformerForwardPass, TassadarExecutorTransformerError> {
        if sequence.len() > self.descriptor.config.max_sequence_tokens {
            return Err(TassadarExecutorTransformerError::SequenceTooLong {
                token_count: sequence.len(),
                max_supported: self.descriptor.config.max_sequence_tokens,
            });
        }
        let mut hidden_states = Vec::new();
        let mut source_hidden_states = Vec::new();
        let mut logits = Vec::new();
        let mut step_contexts = Vec::new();
        let prompt_summary_bucket =
            self.infer_initial_prompt_len(sequence.as_slice())
                .and_then(|prompt_len| {
                    self.prompt_summary_bucket_for_prompt(&sequence.as_slice()[..prompt_len])
                });
        for position in 1..sequence.len() {
            let prefix = &sequence.as_slice()[..position];
            let step_context = self.step_context(prefix, position, prompt_summary_bucket)?;
            let source_hidden_state = self.hidden_state_from_step_context(&step_context)?;
            let hidden_state = self.apply_small_learned_mixer(source_hidden_state.as_slice())?;
            let step_logits = self.project_logits(hidden_state.as_slice())?;
            source_hidden_states.push(source_hidden_state);
            hidden_states.push(hidden_state);
            logits.push(step_logits);
            step_contexts.push(step_context);
        }
        Ok(TassadarExecutorTransformerForwardPass {
            source_hidden_states,
            hidden_states,
            logits,
            step_contexts,
        })
    }

    /// Creates a linear decode state from one prompt sequence.
    pub fn start_decode(
        &self,
        prompt: TokenSequence,
    ) -> Result<TassadarExecutorTransformerDecodeState, TassadarExecutorTransformerError> {
        if prompt.len() > self.descriptor.config.max_sequence_tokens {
            return Err(TassadarExecutorTransformerError::SequenceTooLong {
                token_count: prompt.len(),
                max_supported: self.descriptor.config.max_sequence_tokens,
            });
        }
        Ok(TassadarExecutorTransformerDecodeState {
            initial_prompt_len: prompt.len(),
            prompt_summary_bucket: self.prompt_summary_bucket_for_prompt(prompt.as_slice()),
            next_position: prompt.len(),
            kv_points: prompt
                .as_slice()
                .iter()
                .copied()
                .enumerate()
                .map(|(position, token_id)| Self::kv_point(position, token_id))
                .collect::<Vec<_>>(),
            prefix: prompt,
        })
    }

    /// Extends one decode state with an accepted next token.
    pub fn push_decoded_token(
        &self,
        state: &mut TassadarExecutorTransformerDecodeState,
        next_token: TokenId,
    ) -> Result<(), TassadarExecutorTransformerError> {
        if state.next_position >= self.descriptor.config.max_sequence_tokens {
            return Err(TassadarExecutorTransformerError::SequenceTooLong {
                token_count: state.next_position + 1,
                max_supported: self.descriptor.config.max_sequence_tokens,
            });
        }
        state.prefix.push(next_token);
        state
            .kv_points
            .push(Self::kv_point(state.next_position, next_token));
        state.next_position = state.next_position.saturating_add(1);
        Ok(())
    }

    /// Returns next-token logits for the current decode state.
    pub fn next_token_logits(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
    ) -> Result<Vec<f32>, TassadarExecutorTransformerError> {
        self.next_token_logits_for_mode(state, TassadarExecutorDecodeMode::ReferenceLinear)
    }

    /// Returns next-token logits for one requested decode mode.
    pub fn next_token_logits_for_mode(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<Vec<f32>, TassadarExecutorTransformerError> {
        Ok(self
            .decode_step_for_mode(state, requested_decode_mode)?
            .logits)
    }

    /// Greedily chooses the next token for one decode state.
    pub fn greedy_next_token(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
    ) -> Result<TokenId, TassadarExecutorTransformerError> {
        self.greedy_next_token_for_mode(state, TassadarExecutorDecodeMode::ReferenceLinear)
    }

    /// Greedily chooses the next token for one requested decode mode.
    pub fn greedy_next_token_for_mode(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<TokenId, TassadarExecutorTransformerError> {
        let logits = self.next_token_logits_for_mode(state, requested_decode_mode)?;
        let (best_index, _) = logits
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.partial_cmp(right).unwrap())
            .expect("vocabulary logits should be non-empty");
        Ok(TokenId(best_index as u32))
    }

    /// Returns the full decode-step state for one requested decode mode.
    pub fn decode_step_for_mode(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<TassadarExecutorTransformerDecodeStep, TassadarExecutorTransformerError> {
        let selection = self.select_decode_mode(requested_decode_mode);
        let Some(effective_decode_mode) = selection.effective_decode_mode else {
            return Err(TassadarExecutorTransformerError::UnsupportedDecodeMode {
                requested: requested_decode_mode,
                supported: selection.supported_decode_modes,
            });
        };
        let step_context = self.step_context_from_decode_state(state, effective_decode_mode)?;
        let source_hidden_state = self.hidden_state_from_step_context(&step_context)?;
        let hidden_state = self.apply_small_learned_mixer(source_hidden_state.as_slice())?;
        let mut logits = self.project_logits(hidden_state.as_slice())?;
        self.apply_relative_target_trace_schema_output_bias_in_place(
            logits.as_mut_slice(),
            self.relative_target_trace_schema_phase_index(
                state.prefix.as_slice(),
                state.initial_prompt_len,
            ),
        );
        self.apply_prompt_summary_target_output_bias_in_place(
            logits.as_mut_slice(),
            state.prompt_summary_bucket,
            Some(state.prefix.len().saturating_sub(state.initial_prompt_len)),
        );
        Ok(TassadarExecutorTransformerDecodeStep {
            source_hidden_state,
            hidden_state,
            logits,
            step_context,
        })
    }

    /// Returns the full decode-step state for the default reference-linear path.
    pub fn decode_step(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
    ) -> Result<TassadarExecutorTransformerDecodeStep, TassadarExecutorTransformerError> {
        self.decode_step_for_mode(state, TassadarExecutorDecodeMode::ReferenceLinear)
    }

    fn step_context(
        &self,
        prefix: &[TokenId],
        position: usize,
        prompt_summary_bucket: Option<usize>,
    ) -> Result<TassadarExecutorTransformerStepContext, TassadarExecutorTransformerError> {
        if position >= self.descriptor.config.max_sequence_tokens {
            return Err(TassadarExecutorTransformerError::SequenceTooLong {
                token_count: position + 1,
                max_supported: self.descriptor.config.max_sequence_tokens,
            });
        }
        let context_tokens = self
            .descriptor
            .config
            .context_offsets
            .iter()
            .map(|offset| {
                prefix
                    .len()
                    .checked_sub(*offset)
                    .and_then(|index| prefix.get(index).copied())
                    .unwrap_or_else(|| self.tokenizer.vocabulary().bos_id())
            })
            .collect::<Vec<_>>();
        Ok(TassadarExecutorTransformerStepContext {
            context_tokens,
            position: position as u32,
            prompt_summary_bucket,
        })
    }

    fn step_context_from_decode_state(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
        decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<TassadarExecutorTransformerStepContext, TassadarExecutorTransformerError> {
        let config = &self.descriptor.config;
        let context_tokens = config
            .context_offsets
            .iter()
            .map(|offset| {
                if *offset > state.next_position {
                    Ok(self.tokenizer.vocabulary().bos_id())
                } else {
                    let target_position = state.next_position - *offset;
                    self.lookup_token_from_kv(
                        state.kv_points.as_slice(),
                        target_position,
                        decode_mode,
                    )
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TassadarExecutorTransformerStepContext {
            context_tokens,
            position: state.next_position as u32,
            prompt_summary_bucket: state.prompt_summary_bucket,
        })
    }

    fn hidden_state_from_step_context(
        &self,
        step_context: &TassadarExecutorTransformerStepContext,
    ) -> Result<Vec<f32>, TassadarExecutorTransformerError> {
        let config = &self.descriptor.config;
        let mut hidden = Vec::with_capacity(config.hidden_width());
        for token in &step_context.context_tokens {
            hidden.extend_from_slice(self.token_embedding(*token)?);
        }
        hidden.extend_from_slice(self.position_embedding(step_context.position as usize));
        if let Some(prompt_summary_bucket) = step_context.prompt_summary_bucket {
            hidden.extend_from_slice(self.prompt_summary_embedding(prompt_summary_bucket)?);
        }
        Ok(hidden)
    }

    fn apply_small_learned_mixer(
        &self,
        hidden_state: &[f32],
    ) -> Result<Vec<f32>, TassadarExecutorTransformerError> {
        let hidden_width = self.descriptor.config.hidden_width();
        if hidden_state.len() != hidden_width {
            return Err(TassadarExecutorTransformerError::HiddenWidthMismatch {
                expected: hidden_width,
                actual: hidden_state.len(),
            });
        }
        if !self
            .descriptor
            .trainable_surface
            .trains_small_learned_mixer()
        {
            return Ok(hidden_state.to_vec());
        }
        let mut mixed = hidden_state.to_vec();
        for output_index in 0..hidden_width {
            let mut value = self.weights.small_learned_mixer_bias[output_index];
            for (input_index, hidden_value) in hidden_state.iter().enumerate() {
                let weight_index = input_index * hidden_width + output_index;
                value += hidden_value * self.weights.small_learned_mixer_projection[weight_index];
            }
            mixed[output_index] += value;
        }
        Ok(mixed)
    }

    fn lookup_token_from_kv(
        &self,
        kv_points: &[TassadarExecutorTransformerKvPoint],
        target_position: usize,
        decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<TokenId, TassadarExecutorTransformerError> {
        let matched = match decode_mode {
            TassadarExecutorDecodeMode::ReferenceLinear => {
                self.linear_kv_lookup(kv_points, target_position)
            }
            TassadarExecutorDecodeMode::HullCache => {
                self.hull_kv_lookup(kv_points, target_position)
            }
            TassadarExecutorDecodeMode::SparseTopK => {
                self.sparse_top_k_lookup(kv_points, target_position)
            }
        };
        matched
            .map(|point| point.token_id)
            .ok_or(TassadarExecutorTransformerError::KvLookupMiss {
                target_position,
                decode_mode,
                available_points: kv_points.len(),
            })
    }

    fn linear_kv_lookup<'a>(
        &self,
        kv_points: &'a [TassadarExecutorTransformerKvPoint],
        target_position: usize,
    ) -> Option<&'a TassadarExecutorTransformerKvPoint> {
        kv_points
            .iter()
            .max_by_key(|point| Self::lookup_score(point, target_position))
    }

    fn hull_kv_lookup<'a>(
        &self,
        kv_points: &'a [TassadarExecutorTransformerKvPoint],
        target_position: usize,
    ) -> Option<&'a TassadarExecutorTransformerKvPoint> {
        if kv_points.is_empty() {
            return None;
        }
        let mut low = 0_usize;
        let mut high = kv_points.len() - 1;
        while low < high {
            let mid = (low + high) / 2;
            let mid_score = Self::lookup_score(&kv_points[mid], target_position);
            let right_score = Self::lookup_score(&kv_points[mid + 1], target_position);
            if mid_score <= right_score {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        kv_points.get(low)
    }

    fn sparse_top_k_lookup<'a>(
        &self,
        kv_points: &'a [TassadarExecutorTransformerKvPoint],
        target_position: usize,
    ) -> Option<&'a TassadarExecutorTransformerKvPoint> {
        let sparse_top_k = usize::from(self.descriptor.sparse_top_k.unwrap_or(1).max(1));
        let mut ranked = kv_points.iter().collect::<Vec<_>>();
        ranked.sort_by_key(|point| Reverse(Self::lookup_score(point, target_position)));
        ranked.truncate(sparse_top_k.min(ranked.len()));
        ranked
            .into_iter()
            .max_by_key(|point| Self::lookup_score(point, target_position))
    }

    fn lookup_score(point: &TassadarExecutorTransformerKvPoint, target_position: usize) -> i128 {
        let query_x = target_position as i128;
        query_x * i128::from(point.key_x) + i128::from(point.key_y)
    }

    fn kv_point(position: usize, token_id: TokenId) -> TassadarExecutorTransformerKvPoint {
        let position_i64 = position as i64;
        TassadarExecutorTransformerKvPoint {
            position: position as u32,
            token_id,
            key_x: position_i64.saturating_mul(2),
            key_y: -position_i64.saturating_mul(position_i64),
        }
    }

    fn token_embedding(&self, token: TokenId) -> Result<&[f32], TassadarExecutorTransformerError> {
        let index = token.as_u32() as usize;
        if index >= self.descriptor.config.vocab_size {
            return Err(TassadarExecutorTransformerError::UnknownTokenId {
                token_id: token.as_u32(),
                vocab_size: self.descriptor.config.vocab_size,
            });
        }
        let width = self.descriptor.config.embedding_dim;
        let start = index * width;
        Ok(&self.weights.token_embeddings[start..start + width])
    }

    fn prompt_summary_embedding(
        &self,
        prompt_summary_bucket: usize,
    ) -> Result<&[f32], TassadarExecutorTransformerError> {
        let bucket_count = self.descriptor.config.prompt_summary_bucket_count;
        if bucket_count == 0 || self.weights.prompt_summary_embeddings.is_empty() {
            return Ok(&[]);
        }
        if prompt_summary_bucket >= bucket_count {
            return Err(TassadarExecutorTransformerError::WeightLengthMismatch {
                tensor: String::from("prompt_summary_embedding_bucket"),
                expected: bucket_count,
                actual: prompt_summary_bucket,
            });
        }
        let width = self.descriptor.config.embedding_dim;
        let start = prompt_summary_bucket * width;
        Ok(&self.weights.prompt_summary_embeddings[start..start + width])
    }

    fn position_embedding(&self, position: usize) -> &[f32] {
        let width = self.descriptor.config.embedding_dim;
        let clamped = position.min(self.descriptor.config.max_sequence_tokens - 1);
        let start = clamped * width;
        &self.weights.position_embeddings[start..start + width]
    }

    fn infer_initial_prompt_len(&self, sequence: &[TokenId]) -> Option<usize> {
        let trace_token = self.token_id("<trace>");
        sequence
            .iter()
            .position(|token| *token == trace_token)
            .map(|index| index + 1)
    }

    fn project_logits(
        &self,
        hidden_state: &[f32],
    ) -> Result<Vec<f32>, TassadarExecutorTransformerError> {
        let hidden_width = self.descriptor.config.hidden_width();
        if hidden_state.len() != hidden_width {
            return Err(TassadarExecutorTransformerError::HiddenWidthMismatch {
                expected: hidden_width,
                actual: hidden_state.len(),
            });
        }
        let mut logits = vec![0.0; self.descriptor.config.vocab_size];
        for (vocab_index, logit) in logits.iter_mut().enumerate() {
            let column_offset = vocab_index;
            let mut value = self.weights.output_bias[vocab_index];
            for (hidden_index, hidden_value) in hidden_state.iter().enumerate() {
                let weight_index = hidden_index * self.descriptor.config.vocab_size + column_offset;
                value += hidden_value * self.weights.output_projection[weight_index];
            }
            *logit = value;
        }
        Ok(logits)
    }

    /// Applies the bounded trace-schema-conditioned output-bias adapter to one logit slice.
    pub fn apply_relative_target_trace_schema_output_bias_in_place(
        &self,
        logits: &mut [f32],
        trace_schema_phase: Option<usize>,
    ) {
        let Some(trace_schema_phase) = trace_schema_phase else {
            return;
        };
        let vocab_size = self.descriptor.config.vocab_size;
        if logits.len() != vocab_size
            || trace_schema_phase >= TassadarEarlyTraceSchemaPhase::COUNT
            || self
                .weights
                .relative_target_trace_schema_output_bias
                .is_empty()
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

    /// Applies the prompt-conditioned target-position output-bias adapter to one logit slice.
    pub fn apply_prompt_summary_target_output_bias_in_place(
        &self,
        logits: &mut [f32],
        prompt_summary_bucket: Option<usize>,
        relative_target_position: Option<usize>,
    ) {
        let (Some(prompt_summary_bucket), Some(relative_target_position)) =
            (prompt_summary_bucket, relative_target_position)
        else {
            return;
        };
        if logits.len() != self.descriptor.config.vocab_size {
            return;
        }
        let Some(row_index) = self.prompt_summary_target_output_bias_row_index(
            prompt_summary_bucket,
            relative_target_position,
        ) else {
            return;
        };
        let row = &self.weights.prompt_summary_target_output_bias_rows()[row_index].values;
        for (logit, bias) in logits.iter_mut().zip(row.iter()) {
            *logit += *bias;
        }
    }

    /// Returns the bounded early trace-schema phase index for the current decoded prefix.
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
            1 if target_prefix[0] == step_token => {
                Some(TassadarEarlyTraceSchemaPhase::ExpectStepIndex)
            }
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

/// Neural executor forward/decode failure.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutorTransformerError {
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
    /// Internal hidden-state width drifted from the descriptor.
    #[error("hidden width mismatch: expected {expected}, found {actual}")]
    HiddenWidthMismatch {
        /// Expected hidden width.
        expected: usize,
        /// Actual hidden width.
        actual: usize,
    },
    /// One checkpoint or trained output head supplied the wrong tensor length.
    #[error("trained tensor `{tensor}` has wrong length: expected {expected}, found {actual}")]
    WeightLengthMismatch {
        /// Tensor name.
        tensor: String,
        /// Expected length.
        expected: usize,
        /// Actual length.
        actual: usize,
    },
    /// The caller requested a decode mode the model does not advertise.
    #[error("unsupported decode mode `{requested:?}`; supported modes: {supported:?}")]
    UnsupportedDecodeMode {
        /// Requested mode.
        requested: TassadarExecutorDecodeMode,
        /// Supported modes.
        supported: Vec<TassadarExecutorDecodeMode>,
    },
    /// One decode lookup failed to recover the requested prefix position.
    #[error(
        "kv lookup miss for position {target_position} in mode `{decode_mode:?}` over {available_points} points"
    )]
    KvLookupMiss {
        /// Requested prefix position.
        target_position: usize,
        /// Decode mode used for the lookup.
        decode_mode: TassadarExecutorDecodeMode,
        /// Number of visible KV points.
        available_points: usize,
    },
}

fn seeded_values(label: &str, len: usize, scale: f32) -> Vec<f32> {
    (0..len)
        .map(|index| {
            let mut hasher = Sha256::new();
            hasher.update(b"psionic_tassadar_executor_transformer_seed|");
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
        serde_json::to_vec(value).expect("Tassadar executor transformer value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_runtime::{
        tassadar_sudoku_9x9_corpus, tassadar_sudoku_v0_corpus, TassadarCpuReferenceRunner,
        TassadarExecutorDecodeMode,
    };

    use crate::{TassadarTraceTokenizer, TokenSequence, TokenizerBoundary};

    use super::{
        TassadarEarlyTraceSchemaPhase, TassadarExecutorLongTraceContract,
        TassadarExecutorTrainableSurface, TassadarExecutorTransformer,
        TassadarExecutorTransformerClaimBoundary, TassadarExecutorTransformerConfig,
        TassadarExecutorTransformerDecodeRefusal,
    };
    use crate::tassadar::{
        TassadarWorkloadCapabilityRefusalReason, TassadarWorkloadClass,
        TassadarWorkloadSupportPosture,
    };

    #[test]
    fn sudoku_v0_executor_transformer_descriptor_is_explicit_about_geometry_and_scope() {
        let model = TassadarExecutorTransformer::sudoku_v0();
        let descriptor = model.descriptor();

        assert_eq!(
            descriptor.model.model_id,
            TassadarExecutorTransformer::MODEL_ID
        );
        assert_eq!(descriptor.config.constrained_lookup_head_dim, 2);
        assert_eq!(
            descriptor.attention_mode,
            crate::tassadar::TassadarExecutorAttentionMode::HardMaxLookup
        );
        assert_eq!(
            descriptor.claim_boundary,
            TassadarExecutorTransformerClaimBoundary::NextTokenOnly
        );
        assert_eq!(
            descriptor.long_trace_contract,
            TassadarExecutorLongTraceContract::FlatPrefixFullForward
        );
        assert_eq!(
            descriptor.supported_decode_modes,
            vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache
            ]
        );
    }

    #[test]
    fn sudoku_9x9_executor_transformer_descriptor_is_explicit_about_geometry_and_scope() {
        let model = TassadarExecutorTransformer::sudoku_9x9();
        let descriptor = model.descriptor();

        assert_eq!(
            descriptor.model.model_id,
            TassadarExecutorTransformer::SUDOKU_9X9_MODEL_ID
        );
        assert_eq!(
            descriptor.long_trace_contract,
            TassadarExecutorLongTraceContract::FlatPrefixFullForward
        );
        assert_eq!(descriptor.config.constrained_lookup_head_dim, 2);
        assert!(descriptor.config.max_sequence_tokens >= 524_288);
        assert_eq!(
            descriptor.supported_decode_modes,
            vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache
            ]
        );
    }

    #[test]
    fn learned_executor_transformer_workload_matrix_stays_research_only() {
        let model = TassadarExecutorTransformer::sudoku_v0();
        let matrix = model.workload_capability_matrix();
        let row = matrix
            .row(TassadarWorkloadClass::SudokuClass)
            .expect("sudoku workload row");

        assert_eq!(
            row.support_posture,
            TassadarWorkloadSupportPosture::ResearchOnly
        );
        assert_eq!(
            row.refusal_reasons,
            vec![TassadarWorkloadCapabilityRefusalReason::ClaimBoundaryUnvalidated]
        );
        assert!(matrix.validate_publication().is_err());
    }

    #[test]
    fn sudoku_9x9_windowed_executor_transformer_descriptor_is_explicit_about_long_trace_contract() {
        let model = TassadarExecutorTransformer::sudoku_9x9_windowed_with_surface(
            crate::TassadarExecutorTrainableSurface::OutputHeadOnly,
        );
        let descriptor = model.descriptor();

        assert_eq!(
            descriptor.model.model_id,
            TassadarExecutorTransformer::WINDOWED_SUDOKU_9X9_MODEL_ID
        );
        assert_eq!(
            descriptor.model.family,
            TassadarExecutorTransformer::WINDOWED_MODEL_FAMILY
        );
        assert_eq!(
            descriptor.long_trace_contract,
            TassadarExecutorLongTraceContract::IncrementalDecodeWindow
        );
        assert!(descriptor.config.max_sequence_tokens >= 524_288);
    }

    #[test]
    fn sudoku_v0_executor_transformer_emits_logits_over_tokenized_sequences(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let model = TassadarExecutorTransformer::sudoku_v0();
        let case = tassadar_sudoku_v0_corpus()
            .into_iter()
            .next()
            .expect("sudoku corpus should not be empty");
        let execution = TassadarCpuReferenceRunner::for_program(&case.validation_case.program)?
            .execute(&case.validation_case.program)?;
        let sequence =
            tokenizer.tokenize_program_and_execution(&case.validation_case.program, &execution);
        let forward = model.forward_logits(&sequence.sequence)?;

        assert_eq!(forward.logits.len(), sequence.sequence.len() - 1);
        assert_eq!(forward.hidden_states.len(), sequence.sequence.len() - 1);
        assert!(forward
            .logits
            .iter()
            .all(|step| step.len() == model.descriptor().config.vocab_size));
        Ok(())
    }

    #[test]
    fn sudoku_9x9_executor_transformer_emits_logits_over_tokenized_sequences(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let model = TassadarExecutorTransformer::sudoku_9x9();
        let case = tassadar_sudoku_9x9_corpus()
            .into_iter()
            .next()
            .expect("sudoku corpus should not be empty");
        let execution = TassadarCpuReferenceRunner::for_program(&case.validation_case.program)?
            .execute(&case.validation_case.program)?;
        let sequence =
            tokenizer.tokenize_program_and_execution(&case.validation_case.program, &execution);
        let truncated = TokenSequence::new(
            sequence.sequence.as_slice()[..(sequence.prompt_token_count + 8)].to_vec(),
        );
        let forward = model.forward_logits(&truncated)?;

        assert_eq!(forward.logits.len(), truncated.len() - 1);
        assert_eq!(forward.hidden_states.len(), truncated.len() - 1);
        assert!(forward
            .logits
            .iter()
            .all(|step| step.len() == model.descriptor().config.vocab_size));
        Ok(())
    }

    #[test]
    fn sudoku_v0_executor_transformer_can_start_linear_decode(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let model = TassadarExecutorTransformer::sudoku_v0();
        let config = TassadarExecutorTransformerConfig::sudoku_v0(&tokenizer);
        let encoded = tokenizer.encode("<program> <locals>");
        let prompt = TokenSequence::new(
            std::iter::once(tokenizer.vocabulary().bos_id())
                .chain(encoded.as_slice().iter().copied())
                .collect::<Vec<_>>(),
        );
        let state = model.start_decode(prompt)?;
        let next = model.greedy_next_token(&state)?;

        assert!((next.as_u32() as usize) < config.vocab_size);
        Ok(())
    }

    #[test]
    fn sudoku_v0_executor_transformer_surfaces_machine_legible_decode_selection() {
        let model = TassadarExecutorTransformer::sudoku_v0();

        let direct = model.select_decode_mode(TassadarExecutorDecodeMode::HullCache);
        assert_eq!(
            direct.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::HullCache)
        );
        assert_eq!(direct.fallback_decode_mode, None);
        assert_eq!(direct.refusal, None);

        let fallback = model.select_decode_mode(TassadarExecutorDecodeMode::SparseTopK);
        assert_eq!(
            fallback.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );
        assert_eq!(
            fallback.fallback_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );
        assert_eq!(fallback.refusal, None);

        let mut model_without_decode_paths = TassadarExecutorTransformer::sudoku_v0();
        model_without_decode_paths
            .descriptor
            .supported_decode_modes
            .clear();
        let refusal =
            model_without_decode_paths.select_decode_mode(TassadarExecutorDecodeMode::HullCache);
        assert_eq!(refusal.effective_decode_mode, None);
        assert_eq!(
            refusal.refusal,
            Some(TassadarExecutorTransformerDecodeRefusal::NoSupportedDecodeMode)
        );
    }

    #[test]
    fn sudoku_v0_sparse_executor_transformer_surfaces_direct_sparse_decode_selection() {
        let model = TassadarExecutorTransformer::sudoku_v0_sparse_with_surface(
            TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
        );

        let direct = model.select_decode_mode(TassadarExecutorDecodeMode::SparseTopK);
        assert_eq!(
            direct.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::SparseTopK)
        );
        assert_eq!(direct.fallback_decode_mode, None);
        assert_eq!(model.descriptor().sparse_top_k, Some(4));
    }

    #[test]
    fn hull_decode_matches_linear_decode_over_real_model_kv_points(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let model = TassadarExecutorTransformer::sudoku_v0();
        let encoded = tokenizer.encode("<program> <locals> <memory> <trace>");
        let prompt = TokenSequence::new(
            std::iter::once(tokenizer.vocabulary().bos_id())
                .chain(encoded.as_slice().iter().copied())
                .collect::<Vec<_>>(),
        );
        let linear_state = model.start_decode(prompt.clone())?;
        let hull_state = model.start_decode(prompt)?;

        let linear_logits = model.next_token_logits_for_mode(
            &linear_state,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;
        let hull_logits =
            model.next_token_logits_for_mode(&hull_state, TassadarExecutorDecodeMode::HullCache)?;

        assert_eq!(linear_logits, hull_logits);
        assert_eq!(
            model.greedy_next_token_for_mode(
                &linear_state,
                TassadarExecutorDecodeMode::ReferenceLinear
            )?,
            model.greedy_next_token_for_mode(&hull_state, TassadarExecutorDecodeMode::HullCache)?
        );
        Ok(())
    }

    #[test]
    fn sparse_decode_matches_linear_decode_over_real_model_kv_points(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let model = TassadarExecutorTransformer::sudoku_v0_sparse_with_surface(
            TassadarExecutorTrainableSurface::OutputHeadEmbeddingsAndSmallLearnedMixer,
        );
        let encoded = tokenizer.encode("<program> <locals> <memory> <trace>");
        let prompt = TokenSequence::new(
            std::iter::once(tokenizer.vocabulary().bos_id())
                .chain(encoded.as_slice().iter().copied())
                .collect::<Vec<_>>(),
        );
        let linear_state = model.start_decode(prompt.clone())?;
        let sparse_state = model.start_decode(prompt)?;

        let linear_logits = model.next_token_logits_for_mode(
            &linear_state,
            TassadarExecutorDecodeMode::ReferenceLinear,
        )?;
        let sparse_logits = model
            .next_token_logits_for_mode(&sparse_state, TassadarExecutorDecodeMode::SparseTopK)?;

        assert_eq!(linear_logits, sparse_logits);
        assert_eq!(
            model.greedy_next_token_for_mode(
                &linear_state,
                TassadarExecutorDecodeMode::ReferenceLinear
            )?,
            model.greedy_next_token_for_mode(
                &sparse_state,
                TassadarExecutorDecodeMode::SparseTopK
            )?
        );
        Ok(())
    }

    #[test]
    fn applying_a_trained_output_head_reconstructs_the_same_descriptor_digest(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut trained = TassadarExecutorTransformer::sudoku_v0();
        trained.refresh_after_training();
        let mut restored = TassadarExecutorTransformer::sudoku_v0();

        restored.apply_trained_output_head(
            trained.weights().output_projection(),
            trained.weights().output_bias(),
        )?;

        assert_eq!(
            restored.descriptor().stable_digest(),
            trained.descriptor().stable_digest()
        );
        assert_eq!(
            restored.descriptor().weights.digest,
            trained.descriptor().weights.digest
        );
        Ok(())
    }

    #[test]
    fn executor_transformer_trace_schema_phase_recognizes_pc_boundary() {
        let tokenizer = TassadarTraceTokenizer::new();
        let model = TassadarExecutorTransformer::hungarian_10x10();
        let prefix = tokenizer.encode(
            "<bos> <program> <locals> <byte_00> <byte_00> <byte_00> <byte_00> <memory_slots> <byte_00> <byte_00> <byte_00> <byte_00> <initial_memory> <byte_00> <byte_00> <byte_00> <byte_00> <trace> <step> <step_index> <byte_00> <byte_00> <byte_00> <byte_00>",
        );
        let initial_prompt_len = prefix.len() - 6;
        let phase =
            model.relative_target_trace_schema_phase_index(prefix.as_slice(), initial_prompt_len);

        assert_eq!(phase, Some(TassadarEarlyTraceSchemaPhase::ExpectPc.index()));
    }

    #[test]
    fn executor_transformer_trace_schema_bias_targets_structural_boundary() {
        let tokenizer = TassadarTraceTokenizer::new();
        let mut model = TassadarExecutorTransformer::hungarian_10x10();
        model.ensure_relative_target_trace_schema_output_bias();
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

    #[test]
    fn hungarian_10x10_config_exposes_prompt_summary_surface() {
        let model = TassadarExecutorTransformer::hungarian_10x10();

        assert_eq!(model.descriptor().config.prompt_summary_bucket_count, 4096);
        assert_eq!(model.descriptor().config.hidden_width(), 288);
        assert_eq!(model.weights().prompt_summary_embeddings().len(), 4096 * 32);
    }
}
