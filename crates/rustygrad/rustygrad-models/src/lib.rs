//! Model abstractions for Rustygrad.

use std::{
    collections::BTreeMap,
    fs,
    mem::size_of,
    path::{Path, PathBuf},
};

use rustygrad_core::{DType, QuantizationMode, Shape};
use safetensors::{Dtype as SafeTensorsDType, SafeTensors, serialize_to_file, tensor::TensorView};
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
    /// Weight bundle metadata for the embedding model.
    pub weights: WeightBundleMetadata,
}

impl EmbeddingModelDescriptor {
    /// Creates an embeddings model descriptor.
    #[must_use]
    pub fn new(
        model: ModelDescriptor,
        dimensions: usize,
        normalization: EmbeddingNormalization,
        weights: WeightBundleMetadata,
    ) -> Self {
        Self {
            model,
            dimensions,
            normalization,
            weights,
        }
    }
}

/// Loaded embedding weights.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddingWeights {
    metadata: WeightBundleMetadata,
    projection: Vec<f32>,
    bias: Vec<f32>,
}

impl EmbeddingWeights {
    /// Returns the stable weight metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightBundleMetadata {
        &self.metadata
    }

    /// Returns the projection matrix in row-major order.
    #[must_use]
    pub fn projection(&self) -> &[f32] {
        &self.projection
    }

    /// Returns the bias vector.
    #[must_use]
    pub fn bias(&self) -> &[f32] {
        &self.bias
    }

    fn from_loaded_bundle(
        bundle: LoadedWeightBundle,
        input_dimensions: usize,
        dimensions: usize,
    ) -> Result<Self, ModelLoadError> {
        Ok(Self {
            metadata: bundle.metadata().clone(),
            projection: load_tensor_values(&bundle, "projection", &[input_dimensions, dimensions])?,
            bias: load_tensor_values(&bundle, "bias", &[dimensions])?,
        })
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
#[serde(transparent)]
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
    /// External artifact source.
    ExternalArtifact,
}

/// Stable metadata for an external artifact backing a weight bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightArtifactMetadata {
    /// Stable artifact file name.
    pub name: String,
    /// Artifact size in bytes.
    pub byte_length: u64,
    /// Stable SHA-256 digest of the artifact bytes.
    pub sha256: String,
}

impl WeightArtifactMetadata {
    /// Creates artifact metadata.
    #[must_use]
    pub fn new(name: impl Into<String>, byte_length: u64, sha256: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            byte_length,
            sha256: sha256.into(),
        }
    }

    fn for_path(path: &Path, bytes: &[u8]) -> Self {
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| path.display().to_string());
        Self::new(name, bytes.len() as u64, hex::encode(Sha256::digest(bytes)))
    }
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
    /// Storage quantization mode for the tensor.
    pub quantization: QuantizationMode,
}

impl WeightTensorMetadata {
    /// Creates tensor metadata.
    #[must_use]
    pub fn new(name: impl Into<String>, shape: Shape, dtype: DType) -> Self {
        Self {
            name: name.into(),
            shape,
            dtype,
            quantization: QuantizationMode::None,
        }
    }

    /// Returns a copy tagged with an explicit quantization mode.
    #[must_use]
    pub fn with_quantization(mut self, quantization: QuantizationMode) -> Self {
        self.quantization = quantization;
        self
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
    /// Dominant quantization mode for logical model weights.
    pub quantization: QuantizationMode,
    /// Stable digest over tensor metadata and values.
    pub digest: String,
    /// Ordered tensor metadata.
    pub tensors: Vec<WeightTensorMetadata>,
    /// Backing artifacts, if the bundle was loaded from external files.
    pub artifacts: Vec<WeightArtifactMetadata>,
}

impl WeightBundleMetadata {
    /// Returns whether this bundle was loaded from an external artifact.
    #[must_use]
    pub fn is_artifact_backed(&self) -> bool {
        !self.artifacts.is_empty()
    }
}

/// Loaded tensor values for a weight bundle.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedWeightTensor {
    metadata: WeightTensorMetadata,
    values: Vec<f32>,
}

impl LoadedWeightTensor {
    /// Creates a loaded tensor payload.
    #[must_use]
    pub fn new(metadata: WeightTensorMetadata, values: Vec<f32>) -> Self {
        Self { metadata, values }
    }

    /// Returns the stable tensor metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightTensorMetadata {
        &self.metadata
    }

    /// Returns the tensor values as `f32`.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

/// Loaded external weight bundle with named tensor values.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedWeightBundle {
    metadata: WeightBundleMetadata,
    tensors: BTreeMap<String, LoadedWeightTensor>,
}

impl LoadedWeightBundle {
    /// Creates a loaded weight bundle.
    #[must_use]
    pub fn new(
        metadata: WeightBundleMetadata,
        tensors: BTreeMap<String, LoadedWeightTensor>,
    ) -> Self {
        Self { metadata, tensors }
    }

    /// Returns the stable bundle metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightBundleMetadata {
        &self.metadata
    }

    /// Returns a loaded tensor by stable name.
    #[must_use]
    pub fn tensor(&self, name: &str) -> Option<&LoadedWeightTensor> {
        self.tensors.get(name)
    }
}

/// Loader boundary for local external weight bundles.
pub trait LocalWeightBundleLoader {
    /// Loader error type.
    type Error;

    /// Loads a local artifact-backed bundle from the provided path.
    fn load_path(&self, path: &Path) -> Result<LoadedWeightBundle, Self::Error>;
}

/// Safetensors-backed local weight bundle loader.
#[derive(Clone, Copy, Debug, Default)]
pub struct SafeTensorsWeightBundleLoader;

impl SafeTensorsWeightBundleLoader {
    fn load_bytes(
        &self,
        bytes: &[u8],
        artifact: WeightArtifactMetadata,
    ) -> Result<LoadedWeightBundle, ModelLoadError> {
        let tensors =
            SafeTensors::deserialize(bytes).map_err(|error| ModelLoadError::ArtifactFormat {
                format: String::from("safetensors"),
                message: error.to_string(),
            })?;
        let mut names = tensors.names();
        names.sort_unstable();

        let mut metadata = Vec::with_capacity(names.len());
        let mut loaded = BTreeMap::new();
        let mut hasher = Sha256::new();
        let mut bundle_quantization = QuantizationMode::None;
        for name in names {
            if name.ends_with("__scale") {
                continue;
            }
            let tensor = tensors
                .tensor(name)
                .map_err(|error| ModelLoadError::ArtifactFormat {
                    format: String::from("safetensors"),
                    message: error.to_string(),
                })?;
            let (dtype, quantization, values) = match tensor.dtype() {
                SafeTensorsDType::F32 => (
                    DType::F32,
                    QuantizationMode::None,
                    decode_f32_values(name, tensor.data())?,
                ),
                SafeTensorsDType::I8 => (
                    DType::I8,
                    QuantizationMode::Int8Symmetric,
                    decode_int8_symmetric_values(name, tensor.data(), &tensors)?,
                ),
                other => {
                    return Err(ModelLoadError::UnsupportedTensorDType {
                        name: name.to_string(),
                        dtype: other.to_string(),
                    });
                }
            };
            if quantization != QuantizationMode::None {
                bundle_quantization = quantization;
            }
            let shape = Shape::new(tensor.shape().to_vec());
            let tensor_metadata =
                WeightTensorMetadata::new(name, shape, dtype).with_quantization(quantization);
            digest_tensor(&mut hasher, &tensor_metadata, &values);
            metadata.push(tensor_metadata.clone());
            loaded.insert(
                name.to_string(),
                LoadedWeightTensor::new(tensor_metadata, values),
            );
        }

        Ok(LoadedWeightBundle::new(
            WeightBundleMetadata {
                format: WeightFormat::SafeTensors,
                source: WeightSource::ExternalArtifact,
                quantization: bundle_quantization,
                digest: hex::encode(hasher.finalize()),
                tensors: metadata,
                artifacts: vec![artifact],
            },
            loaded,
        ))
    }
}

impl LocalWeightBundleLoader for SafeTensorsWeightBundleLoader {
    type Error = ModelLoadError;

    fn load_path(&self, path: &Path) -> Result<LoadedWeightBundle, Self::Error> {
        let bytes = fs::read(path).map_err(|error| ModelLoadError::ArtifactRead {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        self.load_bytes(&bytes, WeightArtifactMetadata::for_path(path, &bytes))
    }
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

/// Loaded decoder weights.
#[derive(Clone, Debug, PartialEq)]
pub struct DecoderWeights {
    metadata: WeightBundleMetadata,
    token_embedding: Vec<f32>,
    position_embedding: Vec<f32>,
    context_projection: Vec<f32>,
    lm_head: Vec<f32>,
    lm_bias: Vec<f32>,
}

impl DecoderWeights {
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

    fn from_loaded_bundle(
        descriptor: &DecoderModelDescriptor,
        bundle: LoadedWeightBundle,
    ) -> Result<Self, ModelLoadError> {
        validate_loaded_bundle(descriptor, bundle.metadata())?;
        Ok(Self {
            metadata: bundle.metadata().clone(),
            token_embedding: load_tensor_values(
                &bundle,
                "token_embedding",
                &[descriptor.config.vocab_size, descriptor.config.hidden_size],
            )?,
            position_embedding: load_tensor_values(
                &bundle,
                "position_embedding",
                &[descriptor.config.max_context, descriptor.config.hidden_size],
            )?,
            context_projection: load_tensor_values(
                &bundle,
                "context_projection",
                &[descriptor.config.hidden_size, descriptor.config.hidden_size],
            )?,
            lm_head: load_tensor_values(
                &bundle,
                "lm_head",
                &[descriptor.config.hidden_size, descriptor.config.vocab_size],
            )?,
            lm_bias: load_tensor_values(&bundle, "lm_bias", &[descriptor.config.vocab_size])?,
        })
    }
}

/// Backward-compatible alias for phase-1 fixture naming.
pub type DecoderFixtureWeights = DecoderWeights;

/// Model loading failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelLoadError {
    /// The requested model identifier is unsupported.
    #[error("unsupported decoder model `{0}`")]
    UnsupportedModel(String),
    /// The supplied descriptor does not match the reference fixture config.
    #[error("unsupported decoder fixture config `{0}`")]
    UnsupportedConfig(String),
    /// Reading an artifact file failed.
    #[error("failed to read artifact `{path}`: {message}")]
    ArtifactRead {
        /// Artifact path attempted by the loader.
        path: String,
        /// Read failure summary.
        message: String,
    },
    /// Writing an artifact file failed.
    #[error("failed to write artifact `{path}`: {message}")]
    ArtifactWrite {
        /// Artifact path attempted by the writer.
        path: String,
        /// Write failure summary.
        message: String,
    },
    /// Parsing an artifact file failed.
    #[error("failed to parse {format} artifact: {message}")]
    ArtifactFormat {
        /// Artifact format label.
        format: String,
        /// Parse failure summary.
        message: String,
    },
    /// A tensor dtype in the artifact bundle is not supported yet.
    #[error("unsupported tensor dtype `{dtype}` for `{name}`")]
    UnsupportedTensorDType {
        /// Tensor name.
        name: String,
        /// Dtype label.
        dtype: String,
    },
    /// A required tensor is missing from the artifact bundle.
    #[error("missing required tensor `{0}` in artifact bundle")]
    MissingTensor(String),
    /// A quantized tensor is missing its scale tensor.
    #[error("missing scale tensor `{0}__scale` for quantized tensor")]
    MissingTensorScale(String),
    /// A tensor shape in the artifact bundle does not match the descriptor.
    #[error("tensor `{name}` has shape {actual:?}, expected {expected:?}")]
    InvalidTensorShape {
        /// Tensor name.
        name: String,
        /// Expected dimensions.
        expected: Vec<usize>,
        /// Actual dimensions.
        actual: Vec<usize>,
    },
    /// A quantized tensor scale tensor has an invalid shape.
    #[error("scale tensor for `{name}` has shape {actual:?}, expected scalar or [1]")]
    InvalidTensorScaleShape {
        /// Logical tensor name.
        name: String,
        /// Actual dimensions from the scale tensor.
        actual: Vec<usize>,
    },
    /// Loaded bundle metadata does not match the requested descriptor.
    #[error("weight bundle digest mismatch: expected `{expected}`, actual `{actual}`")]
    WeightDigestMismatch {
        /// Expected digest from the descriptor.
        expected: String,
        /// Actual digest from the artifact bundle.
        actual: String,
    },
    /// Loaded bundle tensor metadata does not match the requested descriptor.
    #[error("weight tensor metadata mismatch for `{name}`")]
    WeightTensorMetadataMismatch {
        /// Tensor name.
        name: String,
    },
}

/// Loader boundary for decoder weight bundles.
pub trait DecoderWeightLoader {
    /// Loader error type.
    type Error;

    /// Loads weights for the provided descriptor.
    fn load(&self, descriptor: &DecoderModelDescriptor) -> Result<DecoderWeights, Self::Error>;
}

/// Programmatic loader for the phase-1 reference decoder weights.
#[derive(Clone, Copy, Debug, Default)]
pub struct FixtureDecoderLoader;

impl DecoderWeightLoader for FixtureDecoderLoader {
    type Error = ModelLoadError;

    fn load(&self, descriptor: &DecoderModelDescriptor) -> Result<DecoderWeights, Self::Error> {
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

/// Artifact-backed decoder loader for local safetensors bundles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafeTensorsDecoderLoader {
    path: PathBuf,
}

impl SafeTensorsDecoderLoader {
    /// Creates a safetensors decoder loader for the provided local path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the configured local artifact path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl DecoderWeightLoader for SafeTensorsDecoderLoader {
    type Error = ModelLoadError;

    fn load(&self, descriptor: &DecoderModelDescriptor) -> Result<DecoderWeights, Self::Error> {
        let bundle = SafeTensorsWeightBundleLoader.load_path(self.path())?;
        DecoderWeights::from_loaded_bundle(descriptor, bundle)
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

/// Artifact-backed wordpiece decoder family built on the phase-1 tiny architecture.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtifactWordDecoder {
    descriptor: DecoderModelDescriptor,
    tokenizer: FixtureWordTokenizer,
    weights: DecoderWeights,
}

impl ArtifactWordDecoder {
    /// Stable model identifier for the first supported model-backed generation path.
    pub const MODEL_ID: &str = "wordpiece-decoder-v1";
    /// Stable model family for the first supported model-backed generation path.
    pub const MODEL_FAMILY: &str = "wordpiece_decoder";

    /// Writes the default local safetensors artifact for the model family.
    pub fn write_default_safetensors_artifact(path: &Path) -> Result<(), ModelLoadError> {
        let reference = ReferenceWordDecoder::new();
        let token_embedding_bytes = encode_f32_bytes(reference.weights().token_embedding());
        let position_embedding_bytes = encode_f32_bytes(reference.weights().position_embedding());
        let context_projection_bytes = encode_f32_bytes(reference.weights().context_projection());
        let lm_head_bytes = encode_f32_bytes(reference.weights().lm_head());
        let lm_bias_bytes = encode_f32_bytes(reference.weights().lm_bias());

        let config = &reference.descriptor().config;
        let tensors = vec![
            (
                "token_embedding",
                TensorView::new(
                    SafeTensorsDType::F32,
                    vec![config.vocab_size, config.hidden_size],
                    token_embedding_bytes.as_slice(),
                )
                .map_err(|error| ModelLoadError::ArtifactFormat {
                    format: String::from("safetensors"),
                    message: error.to_string(),
                })?,
            ),
            (
                "position_embedding",
                TensorView::new(
                    SafeTensorsDType::F32,
                    vec![config.max_context, config.hidden_size],
                    position_embedding_bytes.as_slice(),
                )
                .map_err(|error| ModelLoadError::ArtifactFormat {
                    format: String::from("safetensors"),
                    message: error.to_string(),
                })?,
            ),
            (
                "context_projection",
                TensorView::new(
                    SafeTensorsDType::F32,
                    vec![config.hidden_size, config.hidden_size],
                    context_projection_bytes.as_slice(),
                )
                .map_err(|error| ModelLoadError::ArtifactFormat {
                    format: String::from("safetensors"),
                    message: error.to_string(),
                })?,
            ),
            (
                "lm_head",
                TensorView::new(
                    SafeTensorsDType::F32,
                    vec![config.hidden_size, config.vocab_size],
                    lm_head_bytes.as_slice(),
                )
                .map_err(|error| ModelLoadError::ArtifactFormat {
                    format: String::from("safetensors"),
                    message: error.to_string(),
                })?,
            ),
            (
                "lm_bias",
                TensorView::new(
                    SafeTensorsDType::F32,
                    vec![config.vocab_size],
                    lm_bias_bytes.as_slice(),
                )
                .map_err(|error| ModelLoadError::ArtifactFormat {
                    format: String::from("safetensors"),
                    message: error.to_string(),
                })?,
            ),
        ];
        serialize_to_file(tensors, None, path).map_err(|error| ModelLoadError::ArtifactWrite {
            path: path.display().to_string(),
            message: error.to_string(),
        })
    }

    /// Loads the decoder family from a local safetensors artifact.
    pub fn from_safetensors_artifact(path: impl AsRef<Path>) -> Result<Self, ModelLoadError> {
        let tokenizer = FixtureWordTokenizer::new();
        let config = reference_decoder_config(tokenizer.vocabulary().len());
        let bundle = SafeTensorsWeightBundleLoader.load_path(path.as_ref())?;
        let descriptor = DecoderModelDescriptor::new(
            ModelDescriptor::new(Self::MODEL_ID, Self::MODEL_FAMILY, "v1"),
            config,
            "fixture_wordpiece",
            bundle.metadata().clone(),
        );
        let weights = SafeTensorsDecoderLoader::new(path.as_ref()).load(&descriptor)?;
        Ok(Self {
            descriptor,
            tokenizer,
            weights,
        })
    }

    /// Returns the public decoder descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &DecoderModelDescriptor {
        &self.descriptor
    }

    /// Returns the tokenizer used by the decoder.
    #[must_use]
    pub fn tokenizer(&self) -> &FixtureWordTokenizer {
        &self.tokenizer
    }

    /// Returns the loaded decoder weights.
    #[must_use]
    pub fn weights(&self) -> &DecoderWeights {
        &self.weights
    }
}

/// Deterministic embeddings smoke model used for the phase-0 end-to-end flow.
#[derive(Clone, Debug, PartialEq)]
pub struct SmokeByteEmbedder {
    descriptor: EmbeddingModelDescriptor,
    input_dimensions: usize,
    weights: EmbeddingWeights,
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
        let (projection, bias) = build_byte_projection_parameters(input_dimensions, dimensions);
        let weights =
            build_embedding_fixture_weights(input_dimensions, dimensions, &projection, &bias);
        let descriptor = EmbeddingModelDescriptor::new(
            ModelDescriptor::new(Self::MODEL_ID, "smoke", "v0"),
            dimensions,
            EmbeddingNormalization::None,
            weights.metadata().clone(),
        );

        Self {
            descriptor,
            input_dimensions,
            weights,
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
        self.weights.projection()
    }

    /// Returns the bias vector.
    #[must_use]
    pub fn bias(&self) -> &[f32] {
        self.weights.bias()
    }

    /// Returns the underlying fixture weights.
    #[must_use]
    pub fn weights(&self) -> &EmbeddingWeights {
        &self.weights
    }

    /// Converts input text into a deterministic feature vector.
    #[must_use]
    pub fn featurize(&self, input: &str) -> Vec<f32> {
        byte_projection_features(self.input_dimensions, input)
    }
}

/// Artifact-backed byte-projection embedding model family.
#[derive(Clone, Debug, PartialEq)]
pub struct ByteProjectionEmbedder {
    descriptor: EmbeddingModelDescriptor,
    input_dimensions: usize,
    weights: EmbeddingWeights,
}

impl ByteProjectionEmbedder {
    /// Stable model identifier for the first supported model-backed embeddings path.
    pub const MODEL_ID: &str = "byte-projection-embed-v1";
    /// Stable model family for the first supported model-backed embeddings path.
    pub const MODEL_FAMILY: &str = "byte_projection";

    /// Writes the default local safetensors artifact for the model family.
    pub fn write_default_safetensors_artifact(path: &Path) -> Result<(), ModelLoadError> {
        let input_dimensions = 16;
        let dimensions = 8;
        let (projection, bias) = build_byte_projection_parameters(input_dimensions, dimensions);
        let projection_bytes = encode_f32_bytes(&projection);
        let bias_bytes = encode_f32_bytes(&bias);
        let tensors = vec![
            (
                "projection",
                TensorView::new(
                    SafeTensorsDType::F32,
                    vec![input_dimensions, dimensions],
                    projection_bytes.as_slice(),
                )
                .map_err(|error| ModelLoadError::ArtifactFormat {
                    format: String::from("safetensors"),
                    message: error.to_string(),
                })?,
            ),
            (
                "bias",
                TensorView::new(
                    SafeTensorsDType::F32,
                    vec![dimensions],
                    bias_bytes.as_slice(),
                )
                .map_err(|error| ModelLoadError::ArtifactFormat {
                    format: String::from("safetensors"),
                    message: error.to_string(),
                })?,
            ),
        ];
        serialize_to_file(tensors, None, path).map_err(|error| ModelLoadError::ArtifactWrite {
            path: path.display().to_string(),
            message: error.to_string(),
        })
    }

    /// Loads the model family from a local safetensors artifact.
    pub fn from_safetensors_artifact(path: impl AsRef<Path>) -> Result<Self, ModelLoadError> {
        let input_dimensions = 16;
        let dimensions = 8;
        let bundle = SafeTensorsWeightBundleLoader.load_path(path.as_ref())?;
        let weights = EmbeddingWeights::from_loaded_bundle(bundle, input_dimensions, dimensions)?;
        let descriptor = EmbeddingModelDescriptor::new(
            ModelDescriptor::new(Self::MODEL_ID, Self::MODEL_FAMILY, "v1"),
            dimensions,
            EmbeddingNormalization::UnitLength,
            weights.metadata().clone(),
        );
        Ok(Self {
            descriptor,
            input_dimensions,
            weights,
        })
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

    /// Returns the loaded embedding weights.
    #[must_use]
    pub fn weights(&self) -> &EmbeddingWeights {
        &self.weights
    }

    /// Converts input text into a deterministic feature vector.
    #[must_use]
    pub fn featurize(&self, input: &str) -> Vec<f32> {
        byte_projection_features(self.input_dimensions, input)
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

fn build_byte_projection_parameters(
    input_dimensions: usize,
    dimensions: usize,
) -> (Vec<f32>, Vec<f32>) {
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
    (projection, bias)
}

fn build_embedding_fixture_weights(
    input_dimensions: usize,
    dimensions: usize,
    projection: &[f32],
    bias: &[f32],
) -> EmbeddingWeights {
    let metadata =
        build_embedding_weight_bundle_metadata(input_dimensions, dimensions, projection, bias);
    EmbeddingWeights {
        metadata,
        projection: projection.to_vec(),
        bias: bias.to_vec(),
    }
}

fn build_embedding_weight_bundle_metadata(
    input_dimensions: usize,
    dimensions: usize,
    projection: &[f32],
    bias: &[f32],
) -> WeightBundleMetadata {
    let tensors = vec![
        WeightTensorMetadata::new("bias", Shape::new(vec![dimensions]), DType::F32),
        WeightTensorMetadata::new(
            "projection",
            Shape::new(vec![input_dimensions, dimensions]),
            DType::F32,
        ),
    ];

    let mut hasher = Sha256::new();
    digest_tensor(&mut hasher, &tensors[0], bias);
    digest_tensor(&mut hasher, &tensors[1], projection);
    WeightBundleMetadata {
        format: WeightFormat::ProgrammaticFixture,
        source: WeightSource::Fixture,
        quantization: QuantizationMode::None,
        digest: hex::encode(hasher.finalize()),
        tensors,
        artifacts: Vec::new(),
    }
}

fn byte_projection_features(input_dimensions: usize, input: &str) -> Vec<f32> {
    let mut buckets = vec![0.0; input_dimensions];
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return buckets;
    }

    for (index, byte) in bytes.iter().enumerate() {
        let bucket = (usize::from(*byte) + index) % input_dimensions;
        buckets[bucket] += f32::from(*byte) / 255.0;
    }

    let scale = 1.0 / (bytes.len() as f32);
    for value in &mut buckets {
        *value *= scale;
    }

    buckets
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
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

fn build_reference_weights(config: &DecoderConfig) -> DecoderWeights {
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

    DecoderWeights {
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
    let mut entries = vec![
        (
            WeightTensorMetadata::new(
                "token_embedding",
                Shape::new(vec![config.vocab_size, config.hidden_size]),
                DType::F32,
            ),
            token_embedding,
        ),
        (
            WeightTensorMetadata::new(
                "position_embedding",
                Shape::new(vec![config.max_context, config.hidden_size]),
                DType::F32,
            ),
            position_embedding,
        ),
        (
            WeightTensorMetadata::new(
                "context_projection",
                Shape::new(vec![config.hidden_size, config.hidden_size]),
                DType::F32,
            ),
            context_projection,
        ),
        (
            WeightTensorMetadata::new(
                "lm_head",
                Shape::new(vec![config.hidden_size, config.vocab_size]),
                DType::F32,
            ),
            lm_head,
        ),
        (
            WeightTensorMetadata::new("lm_bias", Shape::new(vec![config.vocab_size]), DType::F32),
            lm_bias,
        ),
    ];
    entries.sort_by(|(left, _), (right, _)| left.name.cmp(&right.name));
    let tensors = entries
        .iter()
        .map(|(metadata, _)| metadata.clone())
        .collect::<Vec<_>>();

    let mut hasher = Sha256::new();
    for (metadata, values) in &entries {
        digest_tensor(&mut hasher, metadata, values);
    }

    WeightBundleMetadata {
        format: WeightFormat::ProgrammaticFixture,
        source: WeightSource::Fixture,
        quantization: QuantizationMode::None,
        digest: hex::encode(hasher.finalize()),
        tensors,
        artifacts: Vec::new(),
    }
}

fn digest_tensor(hasher: &mut Sha256, metadata: &WeightTensorMetadata, values: &[f32]) {
    hasher.update(metadata.name.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.dtype).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.quantization).as_bytes());
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

fn validate_loaded_bundle(
    descriptor: &DecoderModelDescriptor,
    loaded: &WeightBundleMetadata,
) -> Result<(), ModelLoadError> {
    if descriptor.weights.digest != loaded.digest {
        return Err(ModelLoadError::WeightDigestMismatch {
            expected: descriptor.weights.digest.clone(),
            actual: loaded.digest.clone(),
        });
    }

    for expected in &descriptor.weights.tensors {
        let Some(actual) = loaded
            .tensors
            .iter()
            .find(|candidate| candidate.name == expected.name)
        else {
            return Err(ModelLoadError::MissingTensor(expected.name.clone()));
        };
        if expected != actual {
            return Err(ModelLoadError::WeightTensorMetadataMismatch {
                name: expected.name.clone(),
            });
        }
    }
    Ok(())
}

fn load_tensor_values(
    bundle: &LoadedWeightBundle,
    name: &str,
    expected_shape: &[usize],
) -> Result<Vec<f32>, ModelLoadError> {
    let Some(tensor) = bundle.tensor(name) else {
        return Err(ModelLoadError::MissingTensor(String::from(name)));
    };
    let actual_shape = tensor.metadata().shape.dims().to_vec();
    if actual_shape != expected_shape {
        return Err(ModelLoadError::InvalidTensorShape {
            name: String::from(name),
            expected: expected_shape.to_vec(),
            actual: actual_shape,
        });
    }
    Ok(tensor.values().to_vec())
}

fn decode_f32_values(name: &str, data: &[u8]) -> Result<Vec<f32>, ModelLoadError> {
    let chunks = data.chunks_exact(size_of::<f32>());
    if !chunks.remainder().is_empty() {
        return Err(ModelLoadError::ArtifactFormat {
            format: String::from("safetensors"),
            message: format!("tensor `{name}` byte length is not a multiple of 4"),
        });
    }
    Ok(chunks
        .map(|chunk| {
            let mut bytes = [0_u8; size_of::<f32>()];
            bytes.copy_from_slice(chunk);
            f32::from_le_bytes(bytes)
        })
        .collect())
}

fn decode_int8_symmetric_values(
    name: &str,
    data: &[u8],
    tensors: &SafeTensors<'_>,
) -> Result<Vec<f32>, ModelLoadError> {
    let scale_name = format!("{name}__scale");
    let scale_tensor = tensors
        .tensor(&scale_name)
        .map_err(|_| ModelLoadError::MissingTensorScale(String::from(name)))?;
    let scale_shape = scale_tensor.shape().to_vec();
    if !(scale_shape.is_empty() || scale_shape == [1]) {
        return Err(ModelLoadError::InvalidTensorScaleShape {
            name: String::from(name),
            actual: scale_shape,
        });
    }
    if scale_tensor.dtype() != SafeTensorsDType::F32 {
        return Err(ModelLoadError::UnsupportedTensorDType {
            name: scale_name,
            dtype: scale_tensor.dtype().to_string(),
        });
    }
    let scale = decode_f32_values(name, scale_tensor.data())?
        .into_iter()
        .next()
        .ok_or_else(|| ModelLoadError::MissingTensorScale(String::from(name)))?;
    Ok(data
        .iter()
        .map(|byte| f32::from(i8::from_le_bytes([*byte])) * scale)
        .collect())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::Path};

    use rustygrad_core::{DType, QuantizationMode};
    use safetensors::{Dtype as SafeTensorsDType, serialize_to_file, tensor::TensorView};
    use tempfile::tempdir;

    use super::{
        ByteProjectionEmbedder, DecoderModelDescriptor, DecoderWeightLoader, FixtureDecoderLoader,
        FixtureWordTokenizer, LocalWeightBundleLoader, ReferenceWordDecoder,
        SafeTensorsDecoderLoader, SafeTensorsWeightBundleLoader, SmokeByteEmbedder,
        TokenizerBoundary, WeightFormat, WeightSource,
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
        assert_eq!(
            model.descriptor().weights.quantization,
            QuantizationMode::None
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
        assert_eq!(
            model.descriptor().weights.quantization,
            QuantizationMode::None
        );
        assert_eq!(model.descriptor().weights.tensors.len(), 5);
        assert!(model.descriptor().weights.artifacts.is_empty());
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

    #[test]
    fn safetensors_bundle_loader_reports_external_artifact_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = ReferenceWordDecoder::new();
        let temp = tempdir()?;
        let path = temp.path().join("reference_decoder.safetensors");
        write_reference_decoder_bundle(model.descriptor(), &path)?;

        let bundle = SafeTensorsWeightBundleLoader.load_path(&path)?;
        assert_eq!(bundle.metadata().format, WeightFormat::SafeTensors);
        assert_eq!(bundle.metadata().source, WeightSource::ExternalArtifact);
        assert_eq!(bundle.metadata().artifacts.len(), 1);
        assert_eq!(
            bundle.metadata().artifacts[0].name,
            "reference_decoder.safetensors"
        );
        assert_eq!(
            bundle.metadata().tensors,
            model.descriptor().weights.tensors
        );
        assert_eq!(bundle.metadata().digest, model.descriptor().weights.digest);

        let Some(tensor) = bundle.tensor("lm_bias") else {
            return Err("missing lm_bias tensor".into());
        };
        assert_eq!(tensor.values().len(), model.descriptor().config.vocab_size);
        Ok(())
    }

    #[test]
    fn safetensors_decoder_loader_reads_reference_weights() -> Result<(), Box<dyn std::error::Error>>
    {
        let model = ReferenceWordDecoder::new();
        let temp = tempdir()?;
        let path = temp.path().join("reference_decoder.safetensors");
        write_reference_decoder_bundle(model.descriptor(), &path)?;

        let mut descriptor = model.descriptor().clone();
        descriptor.weights = SafeTensorsWeightBundleLoader
            .load_path(&path)?
            .metadata()
            .clone();

        let loader = SafeTensorsDecoderLoader::new(&path);
        let weights = loader.load(&descriptor)?;
        assert_eq!(weights.metadata(), &descriptor.weights);
        assert_eq!(weights.lm_head(), model.weights().lm_head());
        assert_eq!(weights.token_embedding(), model.weights().token_embedding());
        Ok(())
    }

    #[test]
    fn safetensors_loader_reports_and_dequantizes_int8_weights()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("quantized.safetensors");
        let tensors = BTreeMap::from([
            (
                String::from("projection"),
                quantized_tensor_view(vec![2, 2], &[0.5, -0.5, 1.0, -1.0], 0.5)?,
            ),
            (
                String::from("projection__scale"),
                tensor_view(vec![1], &[0.5])?,
            ),
        ]);
        serialize_to_file(tensors, None, &path)?;

        let bundle = SafeTensorsWeightBundleLoader.load_path(&path)?;
        assert_eq!(
            bundle.metadata().quantization,
            QuantizationMode::Int8Symmetric
        );
        let Some(tensor) = bundle.tensor("projection") else {
            return Err("missing projection tensor".into());
        };
        assert_eq!(tensor.metadata().dtype, DType::I8);
        assert_eq!(
            tensor.metadata().quantization,
            QuantizationMode::Int8Symmetric
        );
        assert_eq!(tensor.values(), &[0.5, -0.5, 1.0, -1.0]);
        Ok(())
    }

    #[test]
    fn byte_projection_embedder_loads_from_artifact() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("byte_projection.safetensors");
        ByteProjectionEmbedder::write_default_safetensors_artifact(&path)?;
        let model = ByteProjectionEmbedder::from_safetensors_artifact(&path)?;

        assert_eq!(
            model.descriptor().model.model_id,
            ByteProjectionEmbedder::MODEL_ID
        );
        assert_eq!(
            model.descriptor().model.family,
            ByteProjectionEmbedder::MODEL_FAMILY
        );
        assert_eq!(model.descriptor().weights.format, WeightFormat::SafeTensors);
        assert_eq!(
            model.descriptor().weights.source,
            WeightSource::ExternalArtifact
        );
        assert_eq!(
            model.descriptor().weights.artifacts[0].name,
            "byte_projection.safetensors"
        );
        assert_eq!(
            model.descriptor().normalization,
            super::EmbeddingNormalization::UnitLength
        );
        Ok(())
    }

    #[test]
    fn artifact_word_decoder_loads_from_artifact() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("wordpiece_decoder.safetensors");
        super::ArtifactWordDecoder::write_default_safetensors_artifact(&path)?;
        let model = super::ArtifactWordDecoder::from_safetensors_artifact(&path)?;

        assert_eq!(
            model.descriptor().model.model_id,
            super::ArtifactWordDecoder::MODEL_ID
        );
        assert_eq!(
            model.descriptor().model.family,
            super::ArtifactWordDecoder::MODEL_FAMILY
        );
        assert_eq!(model.descriptor().weights.format, WeightFormat::SafeTensors);
        assert_eq!(
            model.descriptor().weights.source,
            WeightSource::ExternalArtifact
        );
        assert_eq!(
            model.descriptor().weights.artifacts[0].name,
            "wordpiece_decoder.safetensors"
        );
        assert_eq!(
            model.tokenizer().decode(&[FixtureWordTokenizer::OPEN_ID]),
            "open"
        );
        Ok(())
    }

    fn write_reference_decoder_bundle(
        descriptor: &DecoderModelDescriptor,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let reference = ReferenceWordDecoder::new();
        let mut tensors = BTreeMap::new();
        tensors.insert(
            String::from("context_projection"),
            tensor_view(
                vec![descriptor.config.hidden_size, descriptor.config.hidden_size],
                reference.weights().context_projection(),
            )?,
        );
        tensors.insert(
            String::from("lm_bias"),
            tensor_view(
                vec![descriptor.config.vocab_size],
                reference.weights().lm_bias(),
            )?,
        );
        tensors.insert(
            String::from("lm_head"),
            tensor_view(
                vec![descriptor.config.hidden_size, descriptor.config.vocab_size],
                reference.weights().lm_head(),
            )?,
        );
        tensors.insert(
            String::from("position_embedding"),
            tensor_view(
                vec![descriptor.config.max_context, descriptor.config.hidden_size],
                reference.weights().position_embedding(),
            )?,
        );
        tensors.insert(
            String::from("token_embedding"),
            tensor_view(
                vec![descriptor.config.vocab_size, descriptor.config.hidden_size],
                reference.weights().token_embedding(),
            )?,
        );
        serialize_to_file(tensors, None, path)?;
        Ok(())
    }

    fn tensor_view(
        shape: Vec<usize>,
        values: &[f32],
    ) -> Result<TensorView<'static>, Box<dyn std::error::Error>> {
        let bytes = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let leaked = Box::leak(bytes);
        Ok(TensorView::new(SafeTensorsDType::F32, shape, leaked)?)
    }

    fn quantized_tensor_view(
        shape: Vec<usize>,
        values: &[f32],
        scale: f32,
    ) -> Result<TensorView<'static>, Box<dyn std::error::Error>> {
        let bytes = values
            .iter()
            .map(|value| ((*value / scale).round() as i8).to_le_bytes()[0])
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let leaked = Box::leak(bytes);
        Ok(TensorView::new(SafeTensorsDType::I8, shape, leaked)?)
    }
}
