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
            max_sequence_tokens: 131_072,
            embedding_dim: 16,
            context_offsets: vec![1, 2, 4, 8, 16],
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
    head_offsets: Vec<f32>,
}

impl TassadarExecutorTransformerWeightBundle {
    fn new(config: &TassadarExecutorTransformerConfig) -> Self {
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
        let head_offsets = config
            .context_offsets
            .iter()
            .map(|offset| *offset as f32)
            .collect::<Vec<_>>();

        let entries = vec![
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

        Self {
            metadata: build_metadata(entries.as_slice()),
            token_embeddings,
            position_embeddings,
            output_projection,
            output_bias,
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

    /// Returns mutable output projection weights for later training updates.
    pub fn output_projection_mut(&mut self) -> &mut [f32] {
        &mut self.output_projection
    }

    /// Returns mutable output bias weights for later training updates.
    pub fn output_bias_mut(&mut self) -> &mut [f32] {
        &mut self.output_bias
    }
}

/// Hidden-state and logits emitted by one forward pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerForwardPass {
    /// Hidden state for each next-token prediction position.
    pub hidden_states: Vec<Vec<f32>>,
    /// Vocabulary logits for each prediction position.
    pub logits: Vec<Vec<f32>>,
}

/// Typed decode state for linear autoregressive execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTransformerDecodeState {
    /// Prefix tokens visible to the next decode step.
    pub prefix: TokenSequence,
    /// Next decode position.
    pub next_position: usize,
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
    /// Stable model family label.
    pub const MODEL_FAMILY: &str = "tassadar_executor_transformer";

    /// Creates the canonical small Sudoku-v0 executor transformer.
    #[must_use]
    pub fn sudoku_v0() -> Self {
        let tokenizer = TassadarTraceTokenizer::new();
        let config = TassadarExecutorTransformerConfig::sudoku_v0(&tokenizer);
        let weights = TassadarExecutorTransformerWeightBundle::new(&config);
        let descriptor = TassadarExecutorTransformerDescriptor {
            model: ModelDescriptor::new(Self::MODEL_ID, Self::MODEL_FAMILY, "v0"),
            executor_family: TassadarExecutorFamily::WasmTraceExecutor,
            profile: TassadarWasmProfile::sudoku_v0_search_v1(),
            trace_abi: TassadarTraceAbi::sudoku_v0_search_v1(),
            supported_decode_modes: vec![TassadarExecutorDecodeMode::ReferenceLinear],
            attention_mode: TassadarExecutorAttentionMode::HardMaxLookup,
            attention_geometry: TassadarAttentionGeometryContract {
                constrained_lookup_head_dim: Some(config.constrained_lookup_head_dim),
                hull_cache_eligible: true,
            },
            claim_boundary: TassadarExecutorTransformerClaimBoundary::NextTokenOnly,
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
        let mut logits = Vec::new();
        for position in 1..sequence.len() {
            let prefix = &sequence.as_slice()[..position];
            let hidden_state = self.hidden_state(prefix, position)?;
            let step_logits = self.project_logits(hidden_state.as_slice())?;
            hidden_states.push(hidden_state);
            logits.push(step_logits);
        }
        Ok(TassadarExecutorTransformerForwardPass {
            hidden_states,
            logits,
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
            prefix: prompt,
        })
    }

    /// Returns next-token logits for the current decode state.
    pub fn next_token_logits(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
    ) -> Result<Vec<f32>, TassadarExecutorTransformerError> {
        let hidden_state = self.hidden_state(state.prefix.as_slice(), state.next_position)?;
        self.project_logits(hidden_state.as_slice())
    }

    /// Greedily chooses the next token for one decode state.
    pub fn greedy_next_token(
        &self,
        state: &TassadarExecutorTransformerDecodeState,
    ) -> Result<TokenId, TassadarExecutorTransformerError> {
        let logits = self.next_token_logits(state)?;
        let (best_index, _) = logits
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.partial_cmp(right).unwrap())
            .expect("vocabulary logits should be non-empty");
        Ok(TokenId(best_index as u32))
    }

    fn hidden_state(
        &self,
        prefix: &[TokenId],
        position: usize,
    ) -> Result<Vec<f32>, TassadarExecutorTransformerError> {
        if position >= self.descriptor.config.max_sequence_tokens {
            return Err(TassadarExecutorTransformerError::SequenceTooLong {
                token_count: position + 1,
                max_supported: self.descriptor.config.max_sequence_tokens,
            });
        }

        let config = &self.descriptor.config;
        let mut hidden = Vec::with_capacity(config.hidden_width());
        for offset in &config.context_offsets {
            let token = prefix
                .len()
                .checked_sub(*offset)
                .and_then(|index| prefix.get(index).copied())
                .unwrap_or_else(|| self.tokenizer.vocabulary().bos_id());
            hidden.extend_from_slice(self.token_embedding(token)?);
        }
        hidden.extend_from_slice(self.position_embedding(position));
        Ok(hidden)
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
    use psionic_runtime::{TassadarCpuReferenceRunner, tassadar_sudoku_v0_corpus};

    use crate::{TassadarTraceTokenizer, TokenSequence, TokenizerBoundary};

    use super::{
        TassadarExecutorTransformer, TassadarExecutorTransformerClaimBoundary,
        TassadarExecutorTransformerConfig,
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
            vec![psionic_runtime::TassadarExecutorDecodeMode::ReferenceLinear]
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
}
