//! Model abstractions for Rustygrad.

use std::collections::BTreeMap;

use rustygrad_core::{DType, Shape};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "reusable model definitions and metadata";

/// Embedding vector normalization policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingNormalization {
    /// Return raw vectors without normalization.
    None,
    /// Normalize each vector to unit length.
    UnitLength,
}

/// Shared model descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    /// Stable model identifier.
    pub model_id: String,
    /// Model family label such as `smoke`, `llama`, or `bert`.
    pub family: String,
    /// Revision string or version tag.
    pub revision: String,
}

impl ModelDescriptor {
    /// Creates a model descriptor.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        family: impl Into<String>,
        revision: impl Into<String>,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            family: family.into(),
            revision: revision.into(),
        }
    }
}

/// Embeddings-specific model descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingModelDescriptor {
    /// Shared model metadata.
    pub model: ModelDescriptor,
    /// Stable vector dimension.
    pub dimensions: usize,
    /// Normalization policy applied to results.
    pub normalization: EmbeddingNormalization,
}

impl EmbeddingModelDescriptor {
    /// Creates an embeddings model descriptor.
    #[must_use]
    pub fn new(
        model: ModelDescriptor,
        dimensions: usize,
        normalization: EmbeddingNormalization,
    ) -> Self {
        Self {
            model,
            dimensions,
            normalization,
        }
    }
}

/// Token identifier used by tokenizer-neutral generation APIs.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct TokenId(pub u32);

impl TokenId {
    /// Returns the underlying token value.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Ordered token sequence.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSequence {
    tokens: Vec<TokenId>,
}

impl TokenSequence {
    /// Creates a token sequence.
    #[must_use]
    pub fn new(tokens: impl Into<Vec<TokenId>>) -> Self {
        Self {
            tokens: tokens.into(),
        }
    }

    /// Returns the ordered tokens.
    #[must_use]
    pub fn as_slice(&self) -> &[TokenId] {
        &self.tokens
    }

    /// Returns the token count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Returns whether the sequence is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Pushes a token onto the sequence.
    pub fn push(&mut self, token: TokenId) {
        self.tokens.push(token);
    }
}

impl From<Vec<TokenId>> for TokenSequence {
    fn from(tokens: Vec<TokenId>) -> Self {
        Self::new(tokens)
    }
}

/// Stable token vocabulary metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenVocabulary {
    tokens: Vec<String>,
    pad_id: TokenId,
    bos_id: TokenId,
    eos_id: TokenId,
    unknown_id: TokenId,
}

impl TokenVocabulary {
    /// Creates a vocabulary.
    #[must_use]
    pub fn new(
        tokens: Vec<String>,
        pad_id: TokenId,
        bos_id: TokenId,
        eos_id: TokenId,
        unknown_id: TokenId,
    ) -> Self {
        Self {
            tokens,
            pad_id,
            bos_id,
            eos_id,
            unknown_id,
        }
    }

    /// Returns the vocabulary size.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Returns whether the vocabulary is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Returns the token string for an ID.
    #[must_use]
    pub fn token(&self, id: TokenId) -> Option<&str> {
        self.tokens.get(id.as_u32() as usize).map(String::as_str)
    }

    /// Returns all token strings in stable ID order.
    #[must_use]
    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    /// Returns the pad token ID.
    #[must_use]
    pub const fn pad_id(&self) -> TokenId {
        self.pad_id
    }

    /// Returns the beginning-of-sequence token ID.
    #[must_use]
    pub const fn bos_id(&self) -> TokenId {
        self.bos_id
    }

    /// Returns the end-of-sequence token ID.
    #[must_use]
    pub const fn eos_id(&self) -> TokenId {
        self.eos_id
    }

    /// Returns the unknown token ID.
    #[must_use]
    pub const fn unknown_id(&self) -> TokenId {
        self.unknown_id
    }
}

/// Tokenizer-neutral boundary for generation models.
pub trait TokenizerBoundary {
    /// Encodes text into model token IDs.
    fn encode(&self, text: &str) -> TokenSequence;

    /// Decodes token IDs into text.
    fn decode(&self, tokens: &[TokenId]) -> String;

    /// Returns the stable vocabulary metadata.
    fn vocabulary(&self) -> &TokenVocabulary;
}

/// Deterministic whitespace tokenizer used by the phase-1 reference decoder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureWordTokenizer {
    vocabulary: TokenVocabulary,
    lookup: BTreeMap<String, TokenId>,
}

impl Default for FixtureWordTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl FixtureWordTokenizer {
    /// Pad token ID.
    pub const PAD_ID: TokenId = TokenId(0);
    /// Begin-of-sequence token ID.
    pub const BOS_ID: TokenId = TokenId(1);
    /// End-of-sequence token ID.
    pub const EOS_ID: TokenId = TokenId(2);
    /// Unknown token ID.
    pub const UNKNOWN_ID: TokenId = TokenId(3);
    /// `hello` token ID.
    pub const HELLO_ID: TokenId = TokenId(4);
    /// `open` token ID.
    pub const OPEN_ID: TokenId = TokenId(5);
    /// `agents` token ID.
    pub const AGENTS_ID: TokenId = TokenId(6);
    /// `rusty` token ID.
    pub const RUSTY_ID: TokenId = TokenId(7);
    /// `grad` token ID.
    pub const GRAD_ID: TokenId = TokenId(8);
    /// `world` token ID.
    pub const WORLD_ID: TokenId = TokenId(9);

    /// Creates the default reference tokenizer.
    #[must_use]
    pub fn new() -> Self {
        let tokens = vec![
            String::from("<pad>"),
            String::from("<bos>"),
            String::from("<eos>"),
            String::from("<unk>"),
            String::from("hello"),
            String::from("open"),
            String::from("agents"),
            String::from("rusty"),
            String::from("grad"),
            String::from("world"),
        ];
        let lookup = tokens
            .iter()
            .enumerate()
            .map(|(index, token)| (token.clone(), TokenId(index as u32)))
            .collect();
        Self {
            vocabulary: TokenVocabulary::new(
                tokens,
                Self::PAD_ID,
                Self::BOS_ID,
                Self::EOS_ID,
                Self::UNKNOWN_ID,
            ),
            lookup,
        }
    }

    /// Encodes text and optionally prepends/appends BOS/EOS tokens.
    #[must_use]
    pub fn encode_with_special_tokens(
        &self,
        text: &str,
        add_bos: bool,
        add_eos: bool,
    ) -> TokenSequence {
        let mut tokens = Vec::new();
        if add_bos {
            tokens.push(Self::BOS_ID);
        }
        for piece in text.split_whitespace() {
            let normalized = normalize_piece(piece);
            if normalized.is_empty() {
                continue;
            }
            tokens.push(
                self.lookup
                    .get(normalized.as_str())
                    .copied()
                    .unwrap_or(Self::UNKNOWN_ID),
            );
        }
        if add_eos {
            tokens.push(Self::EOS_ID);
        }
        TokenSequence::new(tokens)
    }
}

impl TokenizerBoundary for FixtureWordTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        self.encode_with_special_tokens(text, false, false)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        tokens
            .iter()
            .filter_map(|token| self.vocabulary.token(*token))
            .filter(|token| !matches!(*token, "<pad>" | "<bos>" | "<eos>"))
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        &self.vocabulary
    }
}

/// Activation function used by a decoder feed-forward block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationFunction {
    /// Identity activation, useful for deterministic fixture paths.
    Identity,
    /// ReLU activation.
    Relu,
}

/// Attention configuration for a decoder block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderAttentionConfig {
    /// Number of query heads.
    pub head_count: usize,
    /// Number of KV heads.
    pub kv_head_count: usize,
    /// Width of each head.
    pub head_dim: usize,
    /// Rotary dimension reserved for future RoPE support.
    pub rotary_dim: usize,
}

/// Feed-forward configuration for a decoder block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderFeedForwardConfig {
    /// Hidden expansion size.
    pub intermediate_size: usize,
    /// Activation used inside the block.
    pub activation: ActivationFunction,
}

/// Reusable decoder block configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderBlockConfig {
    /// Attention sub-block configuration.
    pub attention: DecoderAttentionConfig,
    /// Feed-forward sub-block configuration.
    pub feed_forward: DecoderFeedForwardConfig,
}

/// Decoder-style transformer configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderConfig {
    /// Model hidden width.
    pub hidden_size: usize,
    /// Number of decoder layers.
    pub layer_count: usize,
    /// Vocabulary size.
    pub vocab_size: usize,
    /// Maximum supported context length.
    pub max_context: usize,
    /// Shared block configuration.
    pub block: DecoderBlockConfig,
}

impl DecoderConfig {
    /// Returns the total KV width per position.
    #[must_use]
    pub fn kv_width(&self) -> usize {
        self.block.attention.kv_head_count * self.block.attention.head_dim
    }
}

/// Supported weight bundle encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeightFormat {
    /// Programmatic fixture weights generated in-memory.
    ProgrammaticFixture,
    /// Future safetensors import boundary.
    SafeTensors,
}

/// Weight source authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeightSource {
    /// Phase-1 reference weights generated by Rustygrad itself.
    Fixture,
    /// Future external artifact source.
    ExternalArtifact,
}

/// Stable metadata for a single tensor in a weight bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightTensorMetadata {
    /// Stable tensor name.
    pub name: String,
    /// Logical tensor shape.
    pub shape: Shape,
    /// Scalar dtype.
    pub dtype: DType,
}

impl WeightTensorMetadata {
    /// Creates tensor metadata.
    #[must_use]
    pub fn new(name: impl Into<String>, shape: Shape, dtype: DType) -> Self {
        Self {
            name: name.into(),
            shape,
            dtype,
        }
    }

    /// Returns the tensor element count.
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.shape.element_count()
    }
}

/// Stable metadata for a complete weight bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightBundleMetadata {
    /// Bundle format.
    pub format: WeightFormat,
    /// Source authority.
    pub source: WeightSource,
    /// Stable digest over tensor metadata and values.
    pub digest: String,
    /// Ordered tensor metadata.
    pub tensors: Vec<WeightTensorMetadata>,
}

/// Decoder-specific model descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderModelDescriptor {
    /// Shared model metadata.
    pub model: ModelDescriptor,
    /// Decoder configuration.
    pub config: DecoderConfig,
    /// Tokenizer family label.
    pub tokenizer_family: String,
    /// Weight bundle metadata.
    pub weights: WeightBundleMetadata,
}

impl DecoderModelDescriptor {
    /// Creates a decoder model descriptor.
    #[must_use]
    pub fn new(
        model: ModelDescriptor,
        config: DecoderConfig,
        tokenizer_family: impl Into<String>,
        weights: WeightBundleMetadata,
    ) -> Self {
        Self {
            model,
            config,
            tokenizer_family: tokenizer_family.into(),
            weights,
        }
    }
}

/// Programmatic fixture weights for the phase-1 reference decoder.
#[derive(Clone, Debug, PartialEq)]
pub struct DecoderFixtureWeights {
    metadata: WeightBundleMetadata,
    token_embedding: Vec<f32>,
    position_embedding: Vec<f32>,
    context_projection: Vec<f32>,
    lm_head: Vec<f32>,
    lm_bias: Vec<f32>,
}

impl DecoderFixtureWeights {
    /// Returns the stable weight metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightBundleMetadata {
        &self.metadata
    }

    /// Returns the token embedding matrix.
    #[must_use]
    pub fn token_embedding(&self) -> &[f32] {
        &self.token_embedding
    }

    /// Returns the position embedding matrix.
    #[must_use]
    pub fn position_embedding(&self) -> &[f32] {
        &self.position_embedding
    }

    /// Returns the context projection matrix.
    #[must_use]
    pub fn context_projection(&self) -> &[f32] {
        &self.context_projection
    }

    /// Returns the LM head projection matrix.
    #[must_use]
    pub fn lm_head(&self) -> &[f32] {
        &self.lm_head
    }

    /// Returns the LM head bias vector.
    #[must_use]
    pub fn lm_bias(&self) -> &[f32] {
        &self.lm_bias
    }
}

/// Model loading failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelLoadError {
    /// The requested model identifier is unsupported.
    #[error("unsupported decoder model `{0}`")]
    UnsupportedModel(String),
    /// The supplied descriptor does not match the reference fixture config.
    #[error("unsupported decoder fixture config `{0}`")]
    UnsupportedConfig(String),
}

/// Loader boundary for decoder weight bundles.
pub trait DecoderWeightLoader {
    /// Loader error type.
    type Error;

    /// Loads weights for the provided descriptor.
    fn load(
        &self,
        descriptor: &DecoderModelDescriptor,
    ) -> Result<DecoderFixtureWeights, Self::Error>;
}

/// Programmatic loader for the phase-1 reference decoder weights.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixtureDecoderLoader;

impl DecoderWeightLoader for FixtureDecoderLoader {
    type Error = ModelLoadError;

    fn load(
        &self,
        descriptor: &DecoderModelDescriptor,
    ) -> Result<DecoderFixtureWeights, Self::Error> {
        if descriptor.model.model_id != ReferenceWordDecoder::MODEL_ID {
            return Err(ModelLoadError::UnsupportedModel(
                descriptor.model.model_id.clone(),
            ));
        }

        let expected_config =
            reference_decoder_config(FixtureWordTokenizer::new().vocabulary().len());
        if descriptor.config != expected_config {
            return Err(ModelLoadError::UnsupportedConfig(format!(
                "expected hidden_size={} layer_count={} vocab_size={} max_context={}",
                expected_config.hidden_size,
                expected_config.layer_count,
                expected_config.vocab_size,
                expected_config.max_context
            )));
        }

        Ok(build_reference_weights(&expected_config))
    }
}

/// Deterministic decoder fixture used for the phase-1 text-generation path.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceWordDecoder {
    descriptor: DecoderModelDescriptor,
    tokenizer: FixtureWordTokenizer,
    weights: DecoderFixtureWeights,
}

impl Default for ReferenceWordDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceWordDecoder {
    /// Stable model identifier for the phase-1 reference path.
    pub const MODEL_ID: &str = "fixture-word-decoder-v0";

    /// Creates the default reference decoder fixture.
    #[must_use]
    pub fn new() -> Self {
        let tokenizer = FixtureWordTokenizer::new();
        let config = reference_decoder_config(tokenizer.vocabulary().len());
        let weights = build_reference_weights(&config);
        let descriptor = DecoderModelDescriptor::new(
            ModelDescriptor::new(Self::MODEL_ID, "fixture_decoder", "v0"),
            config,
            "fixture_wordpiece",
            weights.metadata().clone(),
        );
        Self {
            descriptor,
            tokenizer,
            weights,
        }
    }

    /// Returns the public decoder descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &DecoderModelDescriptor {
        &self.descriptor
    }

    /// Returns the fixture tokenizer.
    #[must_use]
    pub fn tokenizer(&self) -> &FixtureWordTokenizer {
        &self.tokenizer
    }

    /// Returns the programmatic fixture weights.
    #[must_use]
    pub fn weights(&self) -> &DecoderFixtureWeights {
        &self.weights
    }
}

/// Deterministic embeddings smoke model used for the phase-0 end-to-end flow.
#[derive(Clone, Debug, PartialEq)]
pub struct SmokeByteEmbedder {
    descriptor: EmbeddingModelDescriptor,
    input_dimensions: usize,
    projection: Vec<f32>,
    bias: Vec<f32>,
}

impl Default for SmokeByteEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl SmokeByteEmbedder {
    /// Stable smoke model identifier.
    pub const MODEL_ID: &str = "smoke-byte-embed-v0";

    /// Creates the default smoke embeddings model.
    #[must_use]
    pub fn new() -> Self {
        let input_dimensions = 16;
        let dimensions = 8;
        let descriptor = EmbeddingModelDescriptor::new(
            ModelDescriptor::new(Self::MODEL_ID, "smoke", "v0"),
            dimensions,
            EmbeddingNormalization::None,
        );
        let projection = (0..input_dimensions)
            .flat_map(|row| {
                (0..dimensions).map(move |column| {
                    let seed = ((row + 3) * (column + 5)) % 17;
                    ((seed as f32) - 8.0) / 8.0
                })
            })
            .collect();
        let bias = (0..dimensions)
            .map(|column| {
                let seed = ((column + 1) * 3) % 7;
                ((seed as f32) - 3.0) / 10.0
            })
            .collect();

        Self {
            descriptor,
            input_dimensions,
            projection,
            bias,
        }
    }

    /// Returns the public model descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &EmbeddingModelDescriptor {
        &self.descriptor
    }

    /// Returns the fixed input feature dimension.
    #[must_use]
    pub const fn input_dimensions(&self) -> usize {
        self.input_dimensions
    }

    /// Returns the projection matrix in row-major form.
    #[must_use]
    pub fn projection(&self) -> &[f32] {
        &self.projection
    }

    /// Returns the bias vector.
    #[must_use]
    pub fn bias(&self) -> &[f32] {
        &self.bias
    }

    /// Converts input text into a deterministic feature vector.
    #[must_use]
    pub fn featurize(&self, input: &str) -> Vec<f32> {
        let mut buckets = vec![0.0; self.input_dimensions];
        let bytes = input.as_bytes();
        if bytes.is_empty() {
            return buckets;
        }

        for (index, byte) in bytes.iter().enumerate() {
            let bucket = (usize::from(*byte) + index) % self.input_dimensions;
            buckets[bucket] += f32::from(*byte) / 255.0;
        }

        let scale = 1.0 / (bytes.len() as f32);
        for value in &mut buckets {
            *value *= scale;
        }

        buckets
    }
}

fn normalize_piece(piece: &str) -> String {
    piece
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == '<' || *character == '>'
        })
        .collect::<String>()
        .to_ascii_lowercase()
}

fn reference_decoder_config(vocab_size: usize) -> DecoderConfig {
    DecoderConfig {
        hidden_size: vocab_size,
        layer_count: 1,
        vocab_size,
        max_context: 8,
        block: DecoderBlockConfig {
            attention: DecoderAttentionConfig {
                head_count: 2,
                kv_head_count: 2,
                head_dim: vocab_size / 2,
                rotary_dim: 0,
            },
            feed_forward: DecoderFeedForwardConfig {
                intermediate_size: vocab_size * 2,
                activation: ActivationFunction::Identity,
            },
        },
    }
}

fn build_reference_weights(config: &DecoderConfig) -> DecoderFixtureWeights {
    let token_embedding = build_token_embedding(config.hidden_size, config.vocab_size);
    let position_embedding = build_position_embedding(config.max_context, config.hidden_size);
    let context_projection = build_context_projection(config.hidden_size);
    let lm_head = build_lm_head(config.hidden_size, config.vocab_size);
    let lm_bias = vec![0.0; config.vocab_size];
    let metadata = build_weight_bundle_metadata(
        config,
        &token_embedding,
        &position_embedding,
        &context_projection,
        &lm_head,
        &lm_bias,
    );

    DecoderFixtureWeights {
        metadata,
        token_embedding,
        position_embedding,
        context_projection,
        lm_head,
        lm_bias,
    }
}

fn build_token_embedding(hidden_size: usize, vocab_size: usize) -> Vec<f32> {
    let mut output = vec![0.0; vocab_size * hidden_size];
    for token_index in 0..vocab_size {
        output[(token_index * hidden_size) + token_index] = 1.0;
    }
    output
}

fn build_position_embedding(max_context: usize, hidden_size: usize) -> Vec<f32> {
    (0..max_context)
        .flat_map(|position| {
            (0..hidden_size).map(move |column| {
                let seed = ((position + 1) * (column + 3)) % 11;
                ((seed as f32) - 5.0) / 200.0
            })
        })
        .collect()
}

fn build_context_projection(hidden_size: usize) -> Vec<f32> {
    let mut output = vec![0.0; hidden_size * hidden_size];
    for row in 0..hidden_size {
        for column in 0..hidden_size {
            let value = if row == column {
                0.05
            } else if row.abs_diff(column) == 1 {
                0.01
            } else {
                0.0
            };
            output[(row * hidden_size) + column] = value;
        }
    }
    output
}

fn build_lm_head(hidden_size: usize, vocab_size: usize) -> Vec<f32> {
    let mut output = vec![-0.25; hidden_size * vocab_size];
    for token_index in 0..hidden_size {
        output[(token_index * vocab_size) + next_token_for_row(token_index)] = 1.25;
    }
    output
}

fn next_token_for_row(token_index: usize) -> usize {
    match token_index {
        1 => FixtureWordTokenizer::HELLO_ID.as_u32() as usize,
        4 => FixtureWordTokenizer::OPEN_ID.as_u32() as usize,
        5 => FixtureWordTokenizer::AGENTS_ID.as_u32() as usize,
        6 => FixtureWordTokenizer::EOS_ID.as_u32() as usize,
        7 => FixtureWordTokenizer::GRAD_ID.as_u32() as usize,
        8 => FixtureWordTokenizer::EOS_ID.as_u32() as usize,
        9 => FixtureWordTokenizer::EOS_ID.as_u32() as usize,
        _ => FixtureWordTokenizer::EOS_ID.as_u32() as usize,
    }
}

fn build_weight_bundle_metadata(
    config: &DecoderConfig,
    token_embedding: &[f32],
    position_embedding: &[f32],
    context_projection: &[f32],
    lm_head: &[f32],
    lm_bias: &[f32],
) -> WeightBundleMetadata {
    let tensors = vec![
        WeightTensorMetadata::new(
            "token_embedding",
            Shape::new(vec![config.vocab_size, config.hidden_size]),
            DType::F32,
        ),
        WeightTensorMetadata::new(
            "position_embedding",
            Shape::new(vec![config.max_context, config.hidden_size]),
            DType::F32,
        ),
        WeightTensorMetadata::new(
            "context_projection",
            Shape::new(vec![config.hidden_size, config.hidden_size]),
            DType::F32,
        ),
        WeightTensorMetadata::new(
            "lm_head",
            Shape::new(vec![config.hidden_size, config.vocab_size]),
            DType::F32,
        ),
        WeightTensorMetadata::new("lm_bias", Shape::new(vec![config.vocab_size]), DType::F32),
    ];

    let mut hasher = Sha256::new();
    digest_tensor(&mut hasher, &tensors[0], token_embedding);
    digest_tensor(&mut hasher, &tensors[1], position_embedding);
    digest_tensor(&mut hasher, &tensors[2], context_projection);
    digest_tensor(&mut hasher, &tensors[3], lm_head);
    digest_tensor(&mut hasher, &tensors[4], lm_bias);

    WeightBundleMetadata {
        format: WeightFormat::ProgrammaticFixture,
        source: WeightSource::Fixture,
        digest: hex::encode(hasher.finalize()),
        tensors,
    }
}

fn digest_tensor(hasher: &mut Sha256, metadata: &WeightTensorMetadata, values: &[f32]) {
    hasher.update(metadata.name.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.dtype).as_bytes());
    hasher.update(b"|");
    for dim in metadata.shape.dims() {
        hasher.update(dim.to_string().as_bytes());
        hasher.update(b",");
    }
    hasher.update(b"|");
    for value in values {
        hasher.update(value.to_bits().to_be_bytes());
    }
    hasher.update(b"\n");
}

#[cfg(test)]
mod tests {
    use super::{
        DecoderWeightLoader, FixtureDecoderLoader, FixtureWordTokenizer, ReferenceWordDecoder,
        SmokeByteEmbedder, TokenizerBoundary, WeightFormat, WeightSource,
    };

    #[test]
    fn smoke_featurize_is_deterministic() {
        let model = SmokeByteEmbedder::new();
        let first = model.featurize("hello world");
        let second = model.featurize("hello world");
        assert_eq!(first, second);
    }

    #[test]
    fn smoke_model_exposes_stable_dimensions() {
        let model = SmokeByteEmbedder::new();
        assert_eq!(model.input_dimensions(), 16);
        assert_eq!(model.descriptor().dimensions, 8);
        assert_eq!(
            model.descriptor().model.model_id,
            SmokeByteEmbedder::MODEL_ID
        );
    }

    #[test]
    fn fixture_tokenizer_encodes_and_decodes_known_tokens() {
        let tokenizer = FixtureWordTokenizer::new();
        let encoded = tokenizer.encode("hello open agents");
        assert_eq!(
            encoded.as_slice(),
            &[
                FixtureWordTokenizer::HELLO_ID,
                FixtureWordTokenizer::OPEN_ID,
                FixtureWordTokenizer::AGENTS_ID
            ]
        );
        assert_eq!(tokenizer.decode(encoded.as_slice()), "hello open agents");
    }

    #[test]
    fn reference_decoder_descriptor_is_stable() {
        let model = ReferenceWordDecoder::new();
        assert_eq!(
            model.descriptor().model.model_id,
            ReferenceWordDecoder::MODEL_ID
        );
        assert_eq!(model.descriptor().config.hidden_size, 10);
        assert_eq!(model.descriptor().config.layer_count, 1);
        assert_eq!(model.descriptor().config.max_context, 8);
        assert_eq!(
            model.descriptor().weights.format,
            WeightFormat::ProgrammaticFixture
        );
        assert_eq!(model.descriptor().weights.source, WeightSource::Fixture);
        assert_eq!(model.descriptor().weights.tensors.len(), 5);
    }

    #[test]
    fn fixture_decoder_loader_is_deterministic() -> Result<(), super::ModelLoadError> {
        let model = ReferenceWordDecoder::new();
        let loader = FixtureDecoderLoader;
        let first = loader.load(model.descriptor())?;
        let second = loader.load(model.descriptor())?;

        assert_eq!(first.metadata(), second.metadata());
        assert_eq!(first.token_embedding(), second.token_embedding());
        assert_eq!(first.lm_head(), second.lm_head());
        assert_eq!(first.metadata().digest, model.descriptor().weights.digest);
        Ok(())
    }

    #[test]
    fn reference_decoder_loads_consistently() {
        let first = ReferenceWordDecoder::new();
        let second = ReferenceWordDecoder::new();

        assert_eq!(first.descriptor(), second.descriptor());
        assert_eq!(first.weights().metadata(), second.weights().metadata());
        assert_eq!(
            first.tokenizer().vocabulary(),
            second.tokenizer().vocabulary()
        );
    }
}
