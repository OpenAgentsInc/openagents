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

/// Stable claim boundary for the first neural executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorTransformerClaimBoundary {
    /// The model currently claims only next-token logits over the tokenized trace domain.
    NextTokenOnly,
    /// Greedy autoregressive decode exists but remains unvalidated for exact executor claims.
    GreedyDecodeUnvalidated,
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

fn trainable_surface_is_output_head_only(
    surface: &TassadarExecutorTrainableSurface,
) -> bool {
    *surface == TassadarExecutorTrainableSurface::OutputHeadOnly
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
        }
    }

    /// Returns the hidden-state width emitted by the fixed lookup heads plus position state.
    #[must_use]
    pub fn hidden_width(&self) -> usize {
        self.embedding_dim * (self.context_offsets.len() + 1)
    }
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
    small_learned_mixer_projection: Vec<f32>,
    small_learned_mixer_bias: Vec<f32>,
    head_offsets: Vec<f32>,
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
        let small_learned_mixer_projection =
            vec![0.0; config.hidden_width() * config.hidden_width()];
        let small_learned_mixer_bias = vec![0.0; config.hidden_width()];
        let head_offsets = config
            .context_offsets
            .iter()
            .map(|offset| *offset as f32)
            .collect::<Vec<_>>();

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
    /// Next decode position.
    pub next_position: usize,
    /// Explicit KV points visible to the next decode step.
    pub kv_points: Vec<TassadarExecutorTransformerKvPoint>,
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
    /// Stable model family label.
    pub const MODEL_FAMILY: &str = "tassadar_executor_transformer";

    /// Creates the canonical small Sudoku-v0 executor transformer.
    #[must_use]
    pub fn sudoku_v0() -> Self {
        Self::sudoku_v0_with_surface(TassadarExecutorTrainableSurface::OutputHeadOnly)
    }

    /// Creates the canonical small Sudoku-v0 executor transformer for one surface.
    #[must_use]
    pub fn sudoku_v0_with_surface(trainable_surface: TassadarExecutorTrainableSurface) -> Self {
        let tokenizer = TassadarTraceTokenizer::new();
        let config = TassadarExecutorTransformerConfig::sudoku_v0(&tokenizer);
        let weights = TassadarExecutorTransformerWeightBundle::new(&config, trainable_surface);
        let descriptor = TassadarExecutorTransformerDescriptor {
            model: ModelDescriptor::new(Self::MODEL_ID, Self::MODEL_FAMILY, "v0"),
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
        let tokenizer = TassadarTraceTokenizer::new();
        let config = TassadarExecutorTransformerConfig::sudoku_9x9(&tokenizer);
        let weights = TassadarExecutorTransformerWeightBundle::new(&config, trainable_surface);
        let descriptor = TassadarExecutorTransformerDescriptor {
            model: ModelDescriptor::new(Self::SUDOKU_9X9_MODEL_ID, Self::MODEL_FAMILY, "v0"),
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
        for position in 1..sequence.len() {
            let prefix = &sequence.as_slice()[..position];
            let step_context = self.step_context(prefix, position)?;
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
        let selection = self.select_decode_mode(requested_decode_mode);
        let Some(effective_decode_mode) = selection.effective_decode_mode else {
            return Err(TassadarExecutorTransformerError::UnsupportedDecodeMode {
                requested: requested_decode_mode,
                supported: selection.supported_decode_modes,
            });
        };
        let hidden_state = self.hidden_state_from_decode_state(state, effective_decode_mode)?;
        self.project_logits(hidden_state.as_slice())
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

    fn hidden_state_from_decode_state(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
        decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<Vec<f32>, TassadarExecutorTransformerError> {
        let step_context = self.step_context_from_decode_state(state, decode_mode)?;
        let hidden = self.hidden_state_from_step_context(&step_context)?;
        self.apply_small_learned_mixer(hidden.as_slice())
    }

    fn step_context(
        &self,
        prefix: &[TokenId],
        position: usize,
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
            TassadarExecutorDecodeMode::SparseTopK => None,
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

    fn position_embedding(&self, position: usize) -> &[f32] {
        let width = self.descriptor.config.embedding_dim;
        let clamped = position.min(self.descriptor.config.max_sequence_tokens - 1);
        let start = clamped * width;
        &self.weights.position_embeddings[start..start + width]
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
        TassadarCpuReferenceRunner, TassadarExecutorDecodeMode, tassadar_sudoku_9x9_corpus,
        tassadar_sudoku_v0_corpus,
    };

    use crate::{TassadarTraceTokenizer, TokenSequence, TokenizerBoundary};

    use super::{
        TassadarExecutorTransformer, TassadarExecutorTransformerClaimBoundary,
        TassadarExecutorTransformerConfig, TassadarExecutorTransformerDecodeRefusal,
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
    fn sudoku_v0_executor_transformer_emits_logits_over_tokenized_sequences()
    -> Result<(), Box<dyn std::error::Error>> {
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
        assert!(
            forward
                .logits
                .iter()
                .all(|step| step.len() == model.descriptor().config.vocab_size)
        );
        Ok(())
    }

    #[test]
    fn sudoku_9x9_executor_transformer_emits_logits_over_tokenized_sequences()
    -> Result<(), Box<dyn std::error::Error>> {
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
        assert!(
            forward
                .logits
                .iter()
                .all(|step| step.len() == model.descriptor().config.vocab_size)
        );
        Ok(())
    }

    #[test]
    fn sudoku_v0_executor_transformer_can_start_linear_decode()
    -> Result<(), Box<dyn std::error::Error>> {
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
    fn hull_decode_matches_linear_decode_over_real_model_kv_points()
    -> Result<(), Box<dyn std::error::Error>> {
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
    fn applying_a_trained_output_head_reconstructs_the_same_descriptor_digest()
    -> Result<(), Box<dyn std::error::Error>> {
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
}
