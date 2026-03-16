//! Model abstractions for Psionic.

mod fixtures;
mod harmony;
mod runtime_tokenizer;
mod sharding;
mod tassadar;

use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt, fs,
    mem::size_of,
    path::{Path, PathBuf},
};

use psionic_catalog::{
    BlobError, BlobReadPath, LocalBlob, LocalBlobKind, LocalBlobMetadata, LocalBlobOpenOptions,
    OllamaAdapterPolicy, OllamaCatalogSurface, OllamaLicenseFacts, OllamaManifest,
    OllamaProvenanceFacts, OllamaProvenanceKind, PagedBlobRange,
};
use psionic_core::{DType, QuantizationMode, QuantizedBlockLayout, Shape};
use safetensors::{Dtype as SafeTensorsDType, SafeTensors, serialize_to_file, tensor::TensorView};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use fixtures::*;
pub use harmony::*;
pub use runtime_tokenizer::*;
pub use sharding::*;
pub use tassadar::*;

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

/// How one model entered Psionic-owned descriptor/runtime space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelIngressSurface {
    /// Programmatic fixture defined inside Psionic itself.
    Fixture,
    /// Direct external artifact import such as a local GGUF or safetensors path.
    DirectArtifactImport,
    /// Direct digest-addressed import from an Ollama blob store.
    OllamaCompatBlobImport,
    /// Import from a resolved Ollama manifest and its compatibility metadata.
    OllamaCompatManifestImport,
    /// Future Psionic-native model bundle path.
    PsionicNativeBundle,
}

/// Request/inspection surface currently exposing the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelServingSurface {
    /// An Ollama-compatible migration surface.
    OllamaCompatMigration,
    /// A Psionic-owned native surface.
    PsionicNative,
}

/// Runtime format ownership for an executed model path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRuntimeSurface {
    /// The active execution/runtime format is owned by Psionic itself.
    PsionicNative,
}

/// Explicit boundary between compatibility/migration inputs and Psionic-native execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInteropBoundary {
    /// Catalog surface when the model arrived through a catalog layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_surface: Option<OllamaCatalogSurface>,
    /// How the model entered Psionic-owned descriptor/runtime space.
    pub ingress_surface: ModelIngressSurface,
    /// Request/inspection surface exposing the model.
    pub serving_surface: ModelServingSurface,
    /// Runtime format ownership for the executed model path.
    pub runtime_surface: ModelRuntimeSurface,
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
    /// Stable model-side artifact identity inputs used by serving/evidence layers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_identity: Option<ServedModelArtifactMetadata>,
    /// Stable provenance and license facts for the backing artifact when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_governance: Option<ModelArtifactGovernance>,
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
            artifact_identity: None,
            artifact_governance: None,
        }
    }

    /// Attaches stable serving-identity metadata.
    #[must_use]
    pub fn with_artifact_identity(
        mut self,
        artifact_identity: ServedModelArtifactMetadata,
    ) -> Self {
        self.artifact_identity = Some(artifact_identity);
        self
    }

    /// Attaches provenance and license facts for the backing artifact.
    #[must_use]
    pub fn with_artifact_governance(
        mut self,
        artifact_governance: ModelArtifactGovernance,
    ) -> Self {
        self.artifact_governance = Some(artifact_governance);
        self
    }

    /// Returns the explicit compatibility/native boundary for this loaded model path.
    #[must_use]
    pub fn interop_boundary(&self) -> ModelInteropBoundary {
        ModelInteropBoundary {
            catalog_surface: infer_catalog_surface(self.artifact_governance.as_ref()),
            ingress_surface: infer_model_ingress_surface(
                &self.weights,
                self.artifact_governance.as_ref(),
            ),
            serving_surface: ModelServingSurface::PsionicNative,
            runtime_surface: ModelRuntimeSurface::PsionicNative,
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

/// Explicit overflow posture when a prompt would exceed the available context budget.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextOverflowPolicy {
    /// Refuse the request instead of truncating the prompt.
    #[default]
    Refuse,
    /// Truncate the oldest prompt tokens while preserving an explicit prefix when requested.
    TruncateOldest,
}

/// Explicit context-window budget for one prompt evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWindowBudget {
    /// Maximum context tokens the model can hold.
    pub max_context_tokens: usize,
    /// Tokens already occupied by prior session/KV state.
    pub existing_context_tokens: usize,
    /// Tokens reserved for requested generation output.
    pub reserved_output_tokens: usize,
    /// Remaining prompt-token budget after existing context and output reservation.
    pub available_prompt_tokens: usize,
}

impl ContextWindowBudget {
    /// Builds a context-window budget from max context, existing context, and reserved output.
    #[must_use]
    pub fn new(
        max_context_tokens: usize,
        existing_context_tokens: usize,
        reserved_output_tokens: usize,
    ) -> Self {
        let available_prompt_tokens = max_context_tokens
            .saturating_sub(existing_context_tokens.saturating_add(reserved_output_tokens));
        Self {
            max_context_tokens,
            existing_context_tokens,
            reserved_output_tokens,
            available_prompt_tokens,
        }
    }
}

/// Explicit accounting for how one prompt fit or overflowed the available context budget.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWindowAccounting {
    /// Derived context-window budget.
    pub budget: ContextWindowBudget,
    /// Prompt tokens before truncation.
    pub input_prompt_tokens: usize,
    /// Prompt tokens retained for evaluation.
    pub retained_prompt_tokens: usize,
    /// Prompt tokens truncated from the front.
    pub truncated_prompt_tokens: usize,
    /// Number of prefix tokens the caller requested to preserve during truncation.
    pub preserved_prefix_tokens: usize,
}

impl ContextWindowAccounting {
    /// Returns whether the untruncated prompt overflowed the available budget.
    #[must_use]
    pub fn overflowed(&self) -> bool {
        self.input_prompt_tokens > self.budget.available_prompt_tokens
    }
}

/// Context-window application failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextWindowError {
    /// The prompt exceeded the available context budget and truncation was not allowed.
    #[error("input exceeds maximum context length and cannot be truncated further")]
    CannotTruncateFurther {
        /// Maximum context tokens.
        max_context_tokens: usize,
        /// Existing occupied context tokens.
        existing_context_tokens: usize,
        /// Reserved output tokens.
        reserved_output_tokens: usize,
        /// Input prompt tokens.
        input_prompt_tokens: usize,
        /// Available prompt-token budget.
        available_prompt_tokens: usize,
        /// Selected overflow policy.
        policy: ContextOverflowPolicy,
    },
    /// The prompt still did not fit after applying truncation rules.
    #[error("input after truncation exceeds maximum context length")]
    ExceedsAfterTruncation {
        /// Maximum context tokens.
        max_context_tokens: usize,
        /// Existing occupied context tokens.
        existing_context_tokens: usize,
        /// Reserved output tokens.
        reserved_output_tokens: usize,
        /// Input prompt tokens.
        input_prompt_tokens: usize,
        /// Available prompt-token budget.
        available_prompt_tokens: usize,
        /// Number of prefix tokens the caller required preserving.
        preserved_prefix_tokens: usize,
        /// Prompt tokens retained by the attempted truncation.
        retained_prompt_tokens: usize,
        /// Selected overflow policy.
        policy: ContextOverflowPolicy,
    },
}

/// Applies context budgeting and optional front truncation to a prompt token sequence.
pub fn apply_context_window(
    prompt_tokens: &TokenSequence,
    max_context_tokens: usize,
    existing_context_tokens: usize,
    reserved_output_tokens: usize,
    overflow_policy: ContextOverflowPolicy,
    preserve_prefix_tokens: usize,
) -> Result<(TokenSequence, ContextWindowAccounting), ContextWindowError> {
    let budget = ContextWindowBudget::new(
        max_context_tokens,
        existing_context_tokens,
        reserved_output_tokens,
    );
    let input_prompt_tokens = prompt_tokens.len();
    if input_prompt_tokens <= budget.available_prompt_tokens {
        return Ok((
            prompt_tokens.clone(),
            ContextWindowAccounting {
                budget,
                input_prompt_tokens,
                retained_prompt_tokens: input_prompt_tokens,
                truncated_prompt_tokens: 0,
                preserved_prefix_tokens: preserve_prefix_tokens.min(input_prompt_tokens),
            },
        ));
    }

    let preserved_prefix_tokens = preserve_prefix_tokens.min(input_prompt_tokens);
    if overflow_policy == ContextOverflowPolicy::Refuse {
        return Err(ContextWindowError::CannotTruncateFurther {
            max_context_tokens,
            existing_context_tokens,
            reserved_output_tokens,
            input_prompt_tokens,
            available_prompt_tokens: budget.available_prompt_tokens,
            policy: overflow_policy,
        });
    }

    if budget.available_prompt_tokens == 0
        || budget.available_prompt_tokens <= preserved_prefix_tokens
    {
        return Err(ContextWindowError::ExceedsAfterTruncation {
            max_context_tokens,
            existing_context_tokens,
            reserved_output_tokens,
            input_prompt_tokens,
            available_prompt_tokens: budget.available_prompt_tokens,
            preserved_prefix_tokens,
            retained_prompt_tokens: preserved_prefix_tokens,
            policy: overflow_policy,
        });
    }

    let tail_tokens = budget
        .available_prompt_tokens
        .saturating_sub(preserved_prefix_tokens);
    let suffix_start = input_prompt_tokens
        .saturating_sub(tail_tokens)
        .max(preserved_prefix_tokens);
    let mut retained = prompt_tokens.as_slice()[..preserved_prefix_tokens].to_vec();
    retained.extend_from_slice(&prompt_tokens.as_slice()[suffix_start..]);
    let retained = TokenSequence::new(retained);
    if retained.len() > budget.available_prompt_tokens || retained.len() <= preserved_prefix_tokens
    {
        return Err(ContextWindowError::ExceedsAfterTruncation {
            max_context_tokens,
            existing_context_tokens,
            reserved_output_tokens,
            input_prompt_tokens,
            available_prompt_tokens: budget.available_prompt_tokens,
            preserved_prefix_tokens,
            retained_prompt_tokens: retained.len(),
            policy: overflow_policy,
        });
    }

    Ok((
        retained.clone(),
        ContextWindowAccounting {
            budget,
            input_prompt_tokens,
            retained_prompt_tokens: retained.len(),
            truncated_prompt_tokens: input_prompt_tokens.saturating_sub(retained.len()),
            preserved_prefix_tokens,
        },
    ))
}

/// Activation function used by a decoder feed-forward block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationFunction {
    /// Identity activation, useful for deterministic fixture paths.
    Identity,
    /// ReLU activation.
    Relu,
    /// SiLU / SwiGLU-style activation used by the first supported GGUF decoder families.
    Silu,
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
    /// GGUF artifact-backed weights.
    Gguf,
}

impl WeightFormat {
    /// Returns the stable identity label used in provider/runtime evidence.
    #[must_use]
    pub const fn identity_label(self) -> &'static str {
        match self {
            Self::ProgrammaticFixture => "programmatic_fixture",
            Self::SafeTensors => "safetensors",
            Self::Gguf => "gguf",
        }
    }
}

/// Weight source authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeightSource {
    /// Phase-1 reference weights generated by Psionic itself.
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
    /// Explicit local storage posture when the artifact came from blob-backed paging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<WeightArtifactStorageMetadata>,
}

/// Blob family used to back a paged artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightArtifactBlobKind {
    /// Standalone GGUF file discovered directly on disk.
    GgufFile,
    /// Ollama-managed blob discovered by digest.
    OllamaBlob,
}

impl WeightArtifactBlobKind {
    fn from_local_blob_kind(kind: LocalBlobKind) -> Self {
        match kind {
            LocalBlobKind::GgufFile => Self::GgufFile,
            LocalBlobKind::OllamaBlob => Self::OllamaBlob,
        }
    }
}

/// Actual local read path used for a paged artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightArtifactReadPath {
    /// Artifact bytes are exposed through a memory map.
    MemoryMapped,
    /// Artifact bytes are exposed from a buffered in-memory copy.
    Buffered,
}

impl WeightArtifactReadPath {
    fn from_blob_read_path(read_path: BlobReadPath) -> Self {
        match read_path {
            BlobReadPath::MemoryMapped => Self::MemoryMapped,
            BlobReadPath::Buffered => Self::Buffered,
        }
    }
}

/// Stable paging and read-path metadata for a blob-backed artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightArtifactStorageMetadata {
    /// Local blob family that backed the artifact.
    pub blob_kind: WeightArtifactBlobKind,
    /// Actual local read path used for the artifact.
    pub read_path: WeightArtifactReadPath,
    /// Logical page size used for paged tensor slices.
    pub page_size: usize,
    /// Explicit fallback reason when mmap was requested but not used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

impl WeightArtifactMetadata {
    /// Creates artifact metadata.
    #[must_use]
    pub fn new(name: impl Into<String>, byte_length: u64, sha256: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            byte_length,
            sha256: sha256.into(),
            storage: None,
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

    fn for_blob(blob: &LocalBlobMetadata) -> Self {
        Self {
            name: blob.name.clone(),
            byte_length: blob.byte_length,
            sha256: blob
                .sha256
                .strip_prefix("sha256:")
                .unwrap_or(blob.sha256.as_str())
                .to_string(),
            storage: Some(WeightArtifactStorageMetadata {
                blob_kind: WeightArtifactBlobKind::from_local_blob_kind(blob.kind),
                read_path: WeightArtifactReadPath::from_blob_read_path(blob.read_path),
                page_size: blob.page_size,
                fallback_reason: blob.fallback_reason.clone(),
            }),
        }
    }

    fn integrity_label(&self) -> Cow<'_, str> {
        if self.sha256.contains(':') {
            Cow::Borrowed(self.sha256.as_str())
        } else {
            Cow::Owned(format!("sha256:{}", self.sha256))
        }
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
    /// Stable GGML block layout when the tensor keeps quantized block storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantized_layout: Option<QuantizedBlockLayout>,
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
            quantized_layout: None,
        }
    }

    /// Returns a copy tagged with an explicit quantization mode.
    #[must_use]
    pub fn with_quantization(mut self, quantization: QuantizationMode) -> Self {
        self.quantization = quantization;
        self
    }

    /// Returns a copy tagged with an explicit quantized block layout.
    #[must_use]
    pub fn with_quantized_layout(mut self, quantized_layout: QuantizedBlockLayout) -> Self {
        self.quantized_layout = Some(quantized_layout);
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
    /// All quantization modes observed across the logical model weights.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quantization_modes: Vec<QuantizationMode>,
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

    /// Returns the primary external model-blob digest when the bundle is artifact-backed.
    #[must_use]
    pub fn primary_artifact_digest(&self) -> Option<&str> {
        self.artifacts
            .first()
            .map(|artifact| artifact.sha256.as_str())
    }
}

/// Stable model-side artifact identity inputs reused across serving and receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServedModelArtifactMetadata {
    /// Primary model-blob digest when the model was loaded from an external artifact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_blob_digest: Option<String>,
    /// Stable tokenizer digest when tokenization participates in serving behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer_digest: Option<String>,
    /// Stable chat-template digest when prompt rendering participates in serving behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_digest: Option<String>,
    /// Stable digest over model-default generation behavior such as BOS/EOS and default stops.
    pub generation_defaults_digest: String,
}

impl ServedModelArtifactMetadata {
    /// Creates serving identity metadata from explicit digests.
    #[must_use]
    pub fn new(
        model_blob_digest: Option<String>,
        tokenizer_digest: Option<String>,
        chat_template_digest: Option<String>,
        generation_defaults_digest: impl Into<String>,
    ) -> Self {
        Self {
            model_blob_digest,
            tokenizer_digest,
            chat_template_digest,
            generation_defaults_digest: generation_defaults_digest.into(),
        }
    }
}

/// Provenance class for one backing artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactProvenanceKind {
    /// Programmatic fixture owned by Psionic itself.
    Fixture,
    /// Direct local file path supplied by the caller.
    LocalPath,
    /// Raw blob discovered inside an Ollama store but not tied to a resolved manifest.
    OllamaBlob,
    /// Resolved local Ollama manifest without an explicit remote alias.
    OllamaManifest,
    /// Resolved local Ollama manifest that also declares an upstream remote alias.
    OllamaRemoteAlias,
}

/// Stable provenance facts for one backing artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelArtifactProvenance {
    /// Provenance class for the artifact.
    pub kind: ModelArtifactProvenanceKind,
    /// Human-readable source label such as a file path, blob name, or canonical model name.
    pub source: String,
    /// Stable manifest digest when provenance came from a resolved Ollama manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_sha256: Option<String>,
    /// Declared remote host when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_host: Option<String>,
    /// Declared remote model when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_model: Option<String>,
    /// Declared base model when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_model: Option<String>,
}

impl ModelArtifactProvenance {
    /// Returns fixture provenance for one model identifier.
    #[must_use]
    pub fn fixture(model_id: impl Into<String>) -> Self {
        Self {
            kind: ModelArtifactProvenanceKind::Fixture,
            source: model_id.into(),
            manifest_sha256: None,
            remote_host: None,
            remote_model: None,
            base_model: None,
        }
    }

    fn local_path(path: &Path) -> Self {
        Self {
            kind: ModelArtifactProvenanceKind::LocalPath,
            source: path.display().to_string(),
            manifest_sha256: None,
            remote_host: None,
            remote_model: None,
            base_model: None,
        }
    }

    fn ollama_blob(blob: &LocalBlobMetadata) -> Self {
        Self {
            kind: ModelArtifactProvenanceKind::OllamaBlob,
            source: blob.name.clone(),
            manifest_sha256: None,
            remote_host: None,
            remote_model: None,
            base_model: None,
        }
    }

    fn from_ollama_provenance(facts: OllamaProvenanceFacts) -> Self {
        Self {
            kind: match facts.kind {
                OllamaProvenanceKind::LocalManifest => ModelArtifactProvenanceKind::OllamaManifest,
                OllamaProvenanceKind::RemoteAlias => ModelArtifactProvenanceKind::OllamaRemoteAlias,
            },
            source: facts.canonical_name,
            manifest_sha256: Some(facts.manifest_sha256),
            remote_host: facts.remote_host,
            remote_model: facts.remote_model,
            base_model: facts.base_model,
        }
    }
}

/// One declared license payload for the backing artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelArtifactLicenseEntry {
    /// Stable digest over the license text.
    pub sha256: String,
    /// Exact declared license text.
    pub text: String,
}

/// Stable license facts for the backing artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelArtifactLicenseFacts {
    /// Whether any license text was declared.
    pub declared: bool,
    /// Declared licenses in source order.
    pub entries: Vec<ModelArtifactLicenseEntry>,
}

impl ModelArtifactLicenseFacts {
    /// Returns the declared license digests in source order.
    #[must_use]
    pub fn digests(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.sha256.clone())
            .collect()
    }
}

/// Stable provenance and license facts for a backing artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelArtifactGovernance {
    /// Stable provenance facts.
    pub provenance: ModelArtifactProvenance,
    /// Stable declared license facts.
    pub licenses: ModelArtifactLicenseFacts,
}

impl ModelArtifactGovernance {
    /// Returns governance facts for a fixture-owned model.
    #[must_use]
    pub fn fixture(model_id: impl Into<String>) -> Self {
        Self {
            provenance: ModelArtifactProvenance::fixture(model_id),
            licenses: ModelArtifactLicenseFacts {
                declared: false,
                entries: Vec::new(),
            },
        }
    }

    fn local_path(path: &Path) -> Self {
        Self {
            provenance: ModelArtifactProvenance::local_path(path),
            licenses: ModelArtifactLicenseFacts {
                declared: false,
                entries: Vec::new(),
            },
        }
    }

    fn ollama_blob(blob: &LocalBlobMetadata) -> Self {
        Self {
            provenance: ModelArtifactProvenance::ollama_blob(blob),
            licenses: ModelArtifactLicenseFacts {
                declared: false,
                entries: Vec::new(),
            },
        }
    }

    fn from_ollama_manifest(
        manifest: &OllamaManifest,
        config: Option<&psionic_catalog::OllamaModelConfig>,
        licenses: OllamaLicenseFacts,
    ) -> Self {
        Self {
            provenance: ModelArtifactProvenance::from_ollama_provenance(
                manifest.provenance_facts(config),
            ),
            licenses: ModelArtifactLicenseFacts {
                declared: licenses.declared,
                entries: licenses
                    .entries
                    .into_iter()
                    .map(|entry| ModelArtifactLicenseEntry {
                        sha256: entry.sha256,
                        text: entry.text,
                    })
                    .collect(),
            },
        }
    }

    /// Returns the explicit descriptor-ingress class implied by this governance record.
    #[must_use]
    pub const fn ingress_surface(&self) -> ModelIngressSurface {
        match self.provenance.kind {
            ModelArtifactProvenanceKind::Fixture => ModelIngressSurface::Fixture,
            ModelArtifactProvenanceKind::LocalPath => ModelIngressSurface::DirectArtifactImport,
            ModelArtifactProvenanceKind::OllamaBlob => ModelIngressSurface::OllamaCompatBlobImport,
            ModelArtifactProvenanceKind::OllamaManifest
            | ModelArtifactProvenanceKind::OllamaRemoteAlias => {
                ModelIngressSurface::OllamaCompatManifestImport
            }
        }
    }

    /// Returns the explicit catalog role when this governance record came through Ollama discovery.
    #[must_use]
    pub const fn catalog_surface(&self) -> Option<OllamaCatalogSurface> {
        match self.provenance.kind {
            ModelArtifactProvenanceKind::OllamaManifest
            | ModelArtifactProvenanceKind::OllamaRemoteAlias => {
                Some(OllamaCatalogSurface::OllamaCompatMigration)
            }
            ModelArtifactProvenanceKind::Fixture
            | ModelArtifactProvenanceKind::LocalPath
            | ModelArtifactProvenanceKind::OllamaBlob => None,
        }
    }
}

fn infer_model_ingress_surface(
    weights: &WeightBundleMetadata,
    artifact_governance: Option<&ModelArtifactGovernance>,
) -> ModelIngressSurface {
    artifact_governance
        .map(ModelArtifactGovernance::ingress_surface)
        .unwrap_or_else(|| {
            if weights.source == WeightSource::Fixture
                || weights.format == WeightFormat::ProgrammaticFixture
            {
                ModelIngressSurface::Fixture
            } else if weights.is_artifact_backed() {
                ModelIngressSurface::DirectArtifactImport
            } else {
                ModelIngressSurface::PsionicNativeBundle
            }
        })
}

fn infer_catalog_surface(
    artifact_governance: Option<&ModelArtifactGovernance>,
) -> Option<OllamaCatalogSurface> {
    artifact_governance.and_then(ModelArtifactGovernance::catalog_surface)
}

/// Supported GGUF file versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GgufVersion {
    /// GGUF v1.
    V1,
    /// GGUF v2.
    V2,
    /// GGUF v3.
    V3,
}

impl GgufVersion {
    fn read_count(self, reader: &mut GgufBytesReader<'_>) -> Result<usize, ModelLoadError> {
        let raw = match self {
            Self::V1 => u64::from(reader.read_u32()?),
            Self::V2 | Self::V3 => reader.read_u64()?,
        };
        usize::try_from(raw).map_err(|_| {
            artifact_format_error(
                "gguf",
                format!("count `{raw}` does not fit into usize on this platform"),
            )
        })
    }
}

/// A GGUF metadata value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GgufMetadataValue {
    /// Unsigned 8-bit integer.
    U8(u8),
    /// Signed 8-bit integer.
    I8(i8),
    /// Unsigned 16-bit integer.
    U16(u16),
    /// Signed 16-bit integer.
    I16(i16),
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Signed 32-bit integer.
    I32(i32),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Signed 64-bit integer.
    I64(i64),
    /// 32-bit float.
    F32(f32),
    /// 64-bit float.
    F64(f64),
    /// Boolean.
    Bool(bool),
    /// UTF-8 string.
    String(String),
    /// Homogeneous array value.
    Array(Vec<GgufMetadataValue>),
}

impl GgufMetadataValue {
    /// Returns this value as a string when applicable.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            Self::U8(_)
            | Self::I8(_)
            | Self::U16(_)
            | Self::I16(_)
            | Self::U32(_)
            | Self::I32(_)
            | Self::U64(_)
            | Self::I64(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::Array(_) => None,
        }
    }

    /// Returns this value as an array when applicable.
    #[must_use]
    pub fn as_array(&self) -> Option<&[GgufMetadataValue]> {
        match self {
            Self::Array(values) => Some(values.as_slice()),
            Self::U8(_)
            | Self::I8(_)
            | Self::U16(_)
            | Self::I16(_)
            | Self::U32(_)
            | Self::I32(_)
            | Self::U64(_)
            | Self::I64(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::String(_) => None,
        }
    }

    /// Returns this value as a non-negative integer when possible.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U8(value) => Some(u64::from(*value)),
            Self::U16(value) => Some(u64::from(*value)),
            Self::U32(value) => Some(u64::from(*value)),
            Self::U64(value) => Some(*value),
            Self::I8(value) if *value >= 0 => Some(*value as u64),
            Self::I16(value) if *value >= 0 => Some(*value as u64),
            Self::I32(value) if *value >= 0 => Some(*value as u64),
            Self::I64(value) if *value >= 0 => Some(*value as u64),
            Self::F32(_)
            | Self::F64(_)
            | Self::Bool(_)
            | Self::String(_)
            | Self::Array(_)
            | Self::I8(_)
            | Self::I16(_)
            | Self::I32(_)
            | Self::I64(_) => None,
        }
    }

    /// Returns this value as a signed integer when possible.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::U8(value) => Some(i64::from(*value)),
            Self::I8(value) => Some(i64::from(*value)),
            Self::U16(value) => Some(i64::from(*value)),
            Self::I16(value) => Some(i64::from(*value)),
            Self::U32(value) => Some(i64::from(*value)),
            Self::I32(value) => Some(i64::from(*value)),
            Self::U64(value) => i64::try_from(*value).ok(),
            Self::I64(value) => Some(*value),
            Self::F32(_) | Self::F64(_) | Self::Bool(_) | Self::String(_) | Self::Array(_) => None,
        }
    }

    /// Returns this value as a `f32` when possible.
    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::U8(value) => Some(f32::from(*value)),
            Self::I8(value) => Some(f32::from(*value)),
            Self::U16(value) => Some(f32::from(*value)),
            Self::I16(value) => Some(f32::from(*value)),
            Self::U32(value) => Some(*value as f32),
            Self::I32(value) => Some(*value as f32),
            Self::U64(value) => Some(*value as f32),
            Self::I64(value) => Some(*value as f32),
            Self::F32(value) => Some(*value),
            Self::F64(value) => Some(*value as f32),
            Self::Bool(_) | Self::String(_) | Self::Array(_) => None,
        }
    }

    /// Returns this value as a boolean when applicable.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            Self::U8(_)
            | Self::I8(_)
            | Self::U16(_)
            | Self::I16(_)
            | Self::U32(_)
            | Self::I32(_)
            | Self::U64(_)
            | Self::I64(_)
            | Self::F32(_)
            | Self::F64(_)
            | Self::String(_)
            | Self::Array(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GgufMetadataValueType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    Bool,
    String,
    Array,
    U64,
    I64,
    F64,
}

impl GgufMetadataValueType {
    fn from_u32(value: u32) -> Result<Self, ModelLoadError> {
        let value_type = match value {
            0 => Self::U8,
            1 => Self::I8,
            2 => Self::U16,
            3 => Self::I16,
            4 => Self::U32,
            5 => Self::I32,
            6 => Self::F32,
            7 => Self::Bool,
            8 => Self::String,
            9 => Self::Array,
            10 => Self::U64,
            11 => Self::I64,
            12 => Self::F64,
            _ => {
                return Err(artifact_format_error(
                    "gguf",
                    format!("unsupported metadata value type `{value}`"),
                ));
            }
        };
        Ok(value_type)
    }

    fn read_value(
        self,
        reader: &mut GgufBytesReader<'_>,
        version: GgufVersion,
    ) -> Result<GgufMetadataValue, ModelLoadError> {
        let value = match self {
            Self::U8 => GgufMetadataValue::U8(reader.read_u8()?),
            Self::I8 => GgufMetadataValue::I8(reader.read_i8()?),
            Self::U16 => GgufMetadataValue::U16(reader.read_u16()?),
            Self::I16 => GgufMetadataValue::I16(reader.read_i16()?),
            Self::U32 => GgufMetadataValue::U32(reader.read_u32()?),
            Self::I32 => GgufMetadataValue::I32(reader.read_i32()?),
            Self::U64 => GgufMetadataValue::U64(reader.read_u64()?),
            Self::I64 => GgufMetadataValue::I64(reader.read_i64()?),
            Self::F32 => GgufMetadataValue::F32(reader.read_f32()?),
            Self::F64 => GgufMetadataValue::F64(reader.read_f64()?),
            Self::Bool => match reader.read_u8()? {
                0 => GgufMetadataValue::Bool(false),
                1 => GgufMetadataValue::Bool(true),
                other => {
                    return Err(artifact_format_error(
                        "gguf",
                        format!("invalid boolean metadata value `{other}`"),
                    ));
                }
            },
            Self::String => GgufMetadataValue::String(read_gguf_string(reader, version)?),
            Self::Array => {
                let element_type = GgufMetadataValueType::from_u32(reader.read_u32()?)?;
                let length = version.read_count(reader)?;
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(element_type.read_value(reader, version)?);
                }
                GgufMetadataValue::Array(values)
            }
        };
        Ok(value)
    }
}

/// GGUF tensor type descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GgufTensorType {
    /// 32-bit float tensor.
    F32,
    /// 16-bit float tensor.
    F16,
    /// 16-bit bfloat tensor.
    BF16,
    /// GGML MXFP4 tensor.
    MXFP4,
    /// GGML Q4_0 tensor.
    Q4_0,
    /// GGML Q4_1 tensor.
    Q4_1,
    /// GGML Q5_0 tensor.
    Q5_0,
    /// GGML Q5_1 tensor.
    Q5_1,
    /// GGML Q8_0 tensor.
    Q8_0,
    /// GGML Q8_1 tensor.
    Q8_1,
    /// GGML Q2_K tensor.
    Q2K,
    /// GGML Q3_K tensor.
    Q3K,
    /// GGML Q4_K tensor.
    Q4K,
    /// GGML Q5_K tensor.
    Q5K,
    /// GGML Q6_K tensor.
    Q6K,
    /// GGML Q8_K tensor.
    Q8K,
    /// Unknown tensor type code.
    Unknown(u32),
}

impl GgufTensorType {
    fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::F32,
            1 => Self::F16,
            2 => Self::Q4_0,
            3 => Self::Q4_1,
            6 => Self::Q5_0,
            7 => Self::Q5_1,
            8 => Self::Q8_0,
            9 => Self::Q8_1,
            10 => Self::Q2K,
            11 => Self::Q3K,
            12 => Self::Q4K,
            13 => Self::Q5K,
            14 => Self::Q6K,
            15 => Self::Q8K,
            30 => Self::BF16,
            39 => Self::MXFP4,
            other => Self::Unknown(other),
        }
    }

    fn dense_dtype(self) -> Option<DType> {
        match self {
            Self::F32 => Some(DType::F32),
            Self::F16 => Some(DType::F16),
            Self::BF16 => Some(DType::BF16),
            Self::MXFP4
            | Self::Q4_0
            | Self::Q4_1
            | Self::Q5_0
            | Self::Q5_1
            | Self::Q8_0
            | Self::Q8_1
            | Self::Q2K
            | Self::Q3K
            | Self::Q4K
            | Self::Q5K
            | Self::Q6K
            | Self::Q8K
            | Self::Unknown(_) => None,
        }
    }

    fn quantization_mode(self) -> Option<QuantizationMode> {
        match self {
            Self::MXFP4 => Some(QuantizationMode::GgmlMxfp4),
            Self::Q4_0 => Some(QuantizationMode::GgmlQ4_0),
            Self::Q4_1 => Some(QuantizationMode::GgmlQ4_1),
            Self::Q8_0 => Some(QuantizationMode::GgmlQ8_0),
            Self::F32
            | Self::F16
            | Self::BF16
            | Self::Q5_0
            | Self::Q5_1
            | Self::Q8_1
            | Self::Q2K
            | Self::Q3K
            | Self::Q4K
            | Self::Q5K
            | Self::Q6K
            | Self::Q8K
            | Self::Unknown(_) => None,
        }
    }
}

impl fmt::Display for GgufTensorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::F32 => f.write_str("f32"),
            Self::F16 => f.write_str("f16"),
            Self::BF16 => f.write_str("bf16"),
            Self::MXFP4 => f.write_str("mxfp4"),
            Self::Q4_0 => f.write_str("q4_0"),
            Self::Q4_1 => f.write_str("q4_1"),
            Self::Q5_0 => f.write_str("q5_0"),
            Self::Q5_1 => f.write_str("q5_1"),
            Self::Q8_0 => f.write_str("q8_0"),
            Self::Q8_1 => f.write_str("q8_1"),
            Self::Q2K => f.write_str("q2_k"),
            Self::Q3K => f.write_str("q3_k"),
            Self::Q4K => f.write_str("q4_k"),
            Self::Q5K => f.write_str("q5_k"),
            Self::Q6K => f.write_str("q6_k"),
            Self::Q8K => f.write_str("q8_k"),
            Self::Unknown(value) => write!(f, "unknown({value})"),
        }
    }
}

/// GGUF tensor metadata entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufTensorInfo {
    /// Stable tensor name.
    pub name: String,
    /// Logical tensor shape.
    pub shape: Shape,
    /// Raw GGUF tensor type.
    pub tensor_type: GgufTensorType,
    /// Byte offset from the start of the GGUF tensor data section.
    pub offset: u64,
}

impl GgufTensorInfo {
    fn byte_len(&self) -> Result<usize, ModelLoadError> {
        if let Some(dtype) = self.tensor_type.dense_dtype() {
            return self
                .shape
                .element_count()
                .checked_mul(dtype.element_size_bytes())
                .ok_or_else(|| {
                    artifact_format_error(
                        "gguf",
                        format!(
                            "tensor `{}` byte length overflow for shape {:?}",
                            self.name,
                            self.shape.dims()
                        ),
                    )
                });
        }

        if let Some(quantization) = self.tensor_type.quantization_mode() {
            return quantization.ggml_block_layout(&self.shape).map_or_else(
                || {
                    Err(ModelLoadError::InvalidQuantizedTensorShape {
                        quantization,
                        shape: self.shape.dims().to_vec(),
                    })
                },
                |layout| Ok(layout.byte_len()),
            );
        }

        Err(ModelLoadError::UnsupportedGgufTensorType {
            name: self.name.clone(),
            tensor_type: self.tensor_type,
        })
    }

    fn weight_metadata(&self) -> Result<WeightTensorMetadata, ModelLoadError> {
        if let Some(dtype) = self.tensor_type.dense_dtype() {
            return Ok(WeightTensorMetadata::new(
                self.name.clone(),
                self.shape.clone(),
                dtype,
            ));
        }

        if let Some(quantization) = self.tensor_type.quantization_mode() {
            let layout = quantization.ggml_block_layout(&self.shape).ok_or_else(|| {
                ModelLoadError::InvalidQuantizedTensorShape {
                    quantization,
                    shape: self.shape.dims().to_vec(),
                }
            })?;
            return Ok(WeightTensorMetadata::new(
                self.name.clone(),
                self.shape.clone(),
                DType::F32,
            )
            .with_quantization(quantization)
            .with_quantized_layout(layout));
        }

        Err(ModelLoadError::UnsupportedGgufTensorType {
            name: self.name.clone(),
            tensor_type: self.tensor_type,
        })
    }
}

/// Reusable GGUF metadata and tensor table parsed from an artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GgufContent {
    version: GgufVersion,
    alignment: u64,
    metadata: BTreeMap<String, GgufMetadataValue>,
    tensor_infos: BTreeMap<String, GgufTensorInfo>,
    tensor_data_offset: u64,
}

impl GgufContent {
    /// Parses GGUF metadata and tensor descriptors from raw bytes.
    pub fn read(bytes: &[u8]) -> Result<Self, ModelLoadError> {
        const DEFAULT_ALIGNMENT: u64 = 32;

        let mut reader = GgufBytesReader::new(bytes);
        let magic = reader.read_u32()?;
        let version = reader.read_u32()?;
        let version = match (magic, version) {
            (0x4655_4747 | 0x4747_5546, 1) => GgufVersion::V1,
            (0x4655_4747 | 0x4747_5546, 2) => GgufVersion::V2,
            (0x4655_4747 | 0x4747_5546, 3) => GgufVersion::V3,
            _ => {
                return Err(artifact_format_error(
                    "gguf",
                    format!("unsupported magic/version 0x{magic:08x}/{version}"),
                ));
            }
        };

        let tensor_count = version.read_count(&mut reader)?;
        let metadata_kv_count = version.read_count(&mut reader)?;

        let mut metadata = BTreeMap::new();
        for _ in 0..metadata_kv_count {
            let key = read_gguf_string(&mut reader, version)?;
            let value_type = GgufMetadataValueType::from_u32(reader.read_u32()?)?;
            let value = value_type.read_value(&mut reader, version)?;
            if metadata.insert(key.clone(), value).is_some() {
                return Err(artifact_format_error(
                    "gguf",
                    format!("duplicate metadata key `{key}`"),
                ));
            }
        }

        let mut tensor_infos = BTreeMap::new();
        for _ in 0..tensor_count {
            let name = read_gguf_string(&mut reader, version)?;
            let dimension_count = reader.read_u32()?;
            let mut dimensions = Vec::with_capacity(dimension_count as usize);
            for _ in 0..dimension_count {
                let dimension = match version {
                    GgufVersion::V1 => u64::from(reader.read_u32()?),
                    GgufVersion::V2 | GgufVersion::V3 => reader.read_u64()?,
                };
                dimensions.push(usize::try_from(dimension).map_err(|_| {
                    artifact_format_error(
                        "gguf",
                        format!("tensor `{name}` dimension `{dimension}` does not fit into usize"),
                    )
                })?);
            }
            dimensions.reverse();

            let tensor_info = GgufTensorInfo {
                name: name.clone(),
                shape: Shape::new(dimensions),
                tensor_type: GgufTensorType::from_u32(reader.read_u32()?),
                offset: reader.read_u64()?,
            };

            if tensor_infos.insert(name.clone(), tensor_info).is_some() {
                return Err(artifact_format_error(
                    "gguf",
                    format!("duplicate tensor entry `{name}`"),
                ));
            }
        }

        let alignment = metadata
            .get("general.alignment")
            .and_then(GgufMetadataValue::as_u64)
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_ALIGNMENT);
        let tensor_data_offset = align_offset(reader.position() as u64, alignment);

        Ok(Self {
            version,
            alignment,
            metadata,
            tensor_infos,
            tensor_data_offset,
        })
    }

    /// Reads GGUF metadata and tensor descriptors from a local file.
    pub fn read_path(path: &Path) -> Result<Self, ModelLoadError> {
        let bytes = fs::read(path).map_err(|error| ModelLoadError::ArtifactRead {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        Self::read(&bytes)
    }

    /// Returns the parsed GGUF version.
    #[must_use]
    pub const fn version(&self) -> GgufVersion {
        self.version
    }

    /// Returns the effective tensor-data alignment.
    #[must_use]
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    /// Returns parsed GGUF metadata.
    #[must_use]
    pub fn metadata(&self) -> &BTreeMap<String, GgufMetadataValue> {
        &self.metadata
    }

    /// Returns parsed GGUF tensor descriptors in stable name order.
    pub fn tensor_infos(&self) -> impl Iterator<Item = &GgufTensorInfo> {
        self.tensor_infos.values()
    }

    /// Returns a parsed GGUF tensor descriptor by stable name.
    #[must_use]
    pub fn tensor_info(&self, name: &str) -> Option<&GgufTensorInfo> {
        self.tensor_infos.get(name)
    }

    /// Returns the byte offset of the aligned tensor-data section.
    #[must_use]
    pub const fn tensor_data_offset(&self) -> u64 {
        self.tensor_data_offset
    }

    /// Returns the exact serialized bytes for a tensor.
    pub fn tensor_bytes<'a>(
        &self,
        artifact_bytes: &'a [u8],
        name: &str,
    ) -> Result<&'a [u8], ModelLoadError> {
        let (start, byte_len) = self.tensor_byte_range(name)?;
        let end = start.checked_add(byte_len).ok_or_else(|| {
            artifact_format_error(
                "gguf",
                format!("tensor `{name}` byte range overflows usize"),
            )
        })?;
        artifact_bytes.get(start..end).ok_or_else(|| {
            artifact_format_error(
                "gguf",
                format!(
                    "tensor `{name}` byte range [{start}, {end}) is out of bounds for artifact length {}",
                    artifact_bytes.len()
                ),
            )
        })
    }

    /// Returns the exact byte range for a tensor inside the GGUF artifact.
    pub fn tensor_byte_range(&self, name: &str) -> Result<(usize, usize), ModelLoadError> {
        let tensor = self
            .tensor_info(name)
            .ok_or_else(|| ModelLoadError::MissingTensor(String::from(name)))?;
        let byte_len = tensor.byte_len()?;
        let start = usize::try_from(self.tensor_data_offset)
            .map_err(|_| artifact_format_error("gguf", "tensor data offset does not fit usize"))?
            .checked_add(usize::try_from(tensor.offset).map_err(|_| {
                artifact_format_error("gguf", format!("tensor `{name}` offset does not fit usize"))
            })?)
            .ok_or_else(|| {
                artifact_format_error(
                    "gguf",
                    format!("tensor `{name}` start offset overflows usize"),
                )
            })?;
        Ok((start, byte_len))
    }

    /// Loads a single tensor from GGUF bytes into Psionic loader storage.
    pub fn load_tensor(
        &self,
        artifact_bytes: &[u8],
        name: &str,
    ) -> Result<LoadedWeightTensor, ModelLoadError> {
        let tensor = self
            .tensor_info(name)
            .ok_or_else(|| ModelLoadError::MissingTensor(String::from(name)))?;
        let data = self.tensor_bytes(artifact_bytes, name)?;

        if let Some(dtype) = tensor.tensor_type.dense_dtype() {
            let values = match tensor.tensor_type {
                GgufTensorType::F32 => decode_f32_values("gguf", name, data)?,
                GgufTensorType::F16 => decode_f16_values("gguf", name, data)?,
                GgufTensorType::BF16 => decode_bf16_values("gguf", name, data)?,
                GgufTensorType::MXFP4
                | GgufTensorType::Q4_0
                | GgufTensorType::Q4_1
                | GgufTensorType::Q5_0
                | GgufTensorType::Q5_1
                | GgufTensorType::Q8_0
                | GgufTensorType::Q8_1
                | GgufTensorType::Q2K
                | GgufTensorType::Q3K
                | GgufTensorType::Q4K
                | GgufTensorType::Q5K
                | GgufTensorType::Q6K
                | GgufTensorType::Q8K
                | GgufTensorType::Unknown(_) => unreachable!("dense dtype already filtered"),
            };
            return Ok(LoadedWeightTensor::new(
                WeightTensorMetadata::new(name, tensor.shape.clone(), dtype),
                values,
            ));
        }

        if let Some(quantization) = tensor.tensor_type.quantization_mode() {
            return LoadedWeightTensor::from_ggml_blocks(
                name,
                tensor.shape.clone(),
                quantization,
                data.to_vec(),
            );
        }

        Err(ModelLoadError::UnsupportedGgufTensorType {
            name: String::from(name),
            tensor_type: tensor.tensor_type,
        })
    }

    /// Loads tokenizer metadata for supported GGUF tokenizer families.
    pub fn load_tokenizer(&self) -> Result<GgufTokenizerMetadata, ModelLoadError> {
        let model_name = self
            .metadata
            .get("tokenizer.ggml.model")
            .and_then(GgufMetadataValue::as_str)
            .ok_or_else(|| ModelLoadError::MissingTokenizerMetadata {
                key: String::from("tokenizer.ggml.model"),
            })?;
        let model = GgufTokenizerModel::from_gguf_name(model_name)?;
        let tokens = read_tokenizer_string_array(&self.metadata, "tokenizer.ggml.tokens")?;
        let scores = read_optional_tokenizer_f32_array(&self.metadata, "tokenizer.ggml.scores")?;
        if !scores.is_empty() && scores.len() != tokens.len() {
            return Err(ModelLoadError::InvalidTokenizerMetadata {
                key: String::from("tokenizer.ggml.scores"),
                message: format!(
                    "expected {} score entries to match tokenizer vocabulary, got {}",
                    tokens.len(),
                    scores.len()
                ),
            });
        }
        let token_types =
            read_optional_tokenizer_i32_array(&self.metadata, "tokenizer.ggml.token_type")?;
        if !token_types.is_empty() && token_types.len() != tokens.len() {
            return Err(ModelLoadError::InvalidTokenizerMetadata {
                key: String::from("tokenizer.ggml.token_type"),
                message: format!(
                    "expected {} token-type entries to match tokenizer vocabulary, got {}",
                    tokens.len(),
                    token_types.len()
                ),
            });
        }

        let merges = match model {
            GgufTokenizerModel::SentencePiece | GgufTokenizerModel::BertWordPiece => {
                read_optional_tokenizer_string_array(&self.metadata, "tokenizer.ggml.merges")?
            }
            GgufTokenizerModel::Gpt2Bpe => {
                read_tokenizer_string_array(&self.metadata, "tokenizer.ggml.merges")?
            }
        };
        let bos_token_id =
            read_optional_tokenizer_id(&self.metadata, "tokenizer.ggml.bos_token_id")?;
        let eos_token_id =
            read_optional_tokenizer_id(&self.metadata, "tokenizer.ggml.eos_token_id")?;
        let extra_eos_ids =
            read_optional_tokenizer_id_array(&self.metadata, "tokenizer.ggml.eos_token_ids")?;
        let pad_token_id =
            match read_optional_tokenizer_id(&self.metadata, "tokenizer.ggml.padding_token_id")? {
                Some(id) => Some(id),
                None => read_optional_tokenizer_id(&self.metadata, "tokenizer.ggml.pad_token_id")?,
            };
        let unknown_token_id =
            read_optional_tokenizer_id(&self.metadata, "tokenizer.ggml.unknown_token_id")?.or(
                read_optional_tokenizer_id(&self.metadata, "tokenizer.ggml.unk_token_id")?,
            );
        let add_bos = self
            .metadata
            .get("tokenizer.ggml.add_bos_token")
            .and_then(GgufMetadataValue::as_bool)
            .unwrap_or(true);
        let add_eos = self
            .metadata
            .get("tokenizer.ggml.add_eos_token")
            .and_then(GgufMetadataValue::as_bool)
            .unwrap_or(matches!(model, GgufTokenizerModel::BertWordPiece));
        let pretokenizer = match model {
            GgufTokenizerModel::SentencePiece | GgufTokenizerModel::BertWordPiece => None,
            GgufTokenizerModel::Gpt2Bpe => Some(
                self.metadata
                    .get("tokenizer.ggml.pre")
                    .and_then(GgufMetadataValue::as_str)
                    .map_or(
                        GgufTokenizerPretokenizer::Default,
                        GgufTokenizerPretokenizer::from_gguf_name,
                    ),
            ),
        };
        let token_type_count =
            read_optional_gguf_usize(&self.metadata, "tokenizer.ggml.token_type_count")?;

        validate_tokenizer_id("tokenizer.ggml.bos_token_id", bos_token_id, tokens.len())?;
        validate_tokenizer_id(
            "tokenizer.ggml.padding_token_id",
            pad_token_id,
            tokens.len(),
        )?;
        validate_tokenizer_id(
            "tokenizer.ggml.unknown_token_id",
            unknown_token_id,
            tokens.len(),
        )?;

        let mut eos_token_ids = Vec::new();
        if let Some(id) = eos_token_id {
            eos_token_ids.push(id);
        }
        for id in extra_eos_ids {
            if !eos_token_ids.contains(&id) {
                eos_token_ids.push(id);
            }
        }
        for id in &eos_token_ids {
            validate_tokenizer_id("tokenizer.ggml.eos_token_ids", Some(*id), tokens.len())?;
        }

        let digest = digest_gguf_tokenizer(
            model,
            tokens.as_slice(),
            scores.as_slice(),
            token_types.as_slice(),
            merges.as_slice(),
            bos_token_id,
            eos_token_ids.as_slice(),
            pad_token_id,
            unknown_token_id,
            add_bos,
            add_eos,
            pretokenizer.as_ref(),
            token_type_count,
        );

        Ok(GgufTokenizerMetadata {
            model,
            vocabulary: GgufTokenizerVocabulary {
                tokens,
                bos_token_id,
                eos_token_ids,
                pad_token_id,
                unknown_token_id,
            },
            scores,
            token_types,
            merges,
            add_bos,
            add_eos,
            pretokenizer,
            token_type_count,
            digest,
        })
    }

    /// Loads default and named chat-template metadata from GGUF.
    pub fn load_chat_templates(&self) -> Result<GgufChatTemplateMetadata, ModelLoadError> {
        let default_template =
            read_optional_gguf_string(&self.metadata, "tokenizer.chat_template")?;
        let declared_names =
            read_optional_gguf_string_array(&self.metadata, "tokenizer.chat_templates")?;

        let mut named_templates = BTreeMap::new();
        for name in declared_names {
            let key = format!("tokenizer.chat_template.{name}");
            let value = read_optional_gguf_string(&self.metadata, &key)?.ok_or_else(|| {
                artifact_format_error(
                    "gguf",
                    format!(
                        "missing named chat template `{key}` declared in tokenizer.chat_templates"
                    ),
                )
            })?;
            named_templates.insert(name, value);
        }
        for key in self.metadata.keys() {
            let Some(name) = key.strip_prefix("tokenizer.chat_template.") else {
                continue;
            };
            if name.is_empty() || named_templates.contains_key(name) {
                continue;
            }
            if let Some(value) = read_optional_gguf_string(&self.metadata, key)? {
                named_templates.insert(name.to_string(), value);
            }
        }

        Ok(GgufChatTemplateMetadata::new(
            default_template,
            named_templates,
        ))
    }
}

/// Paged tensor bytes backed by a local model blob.
#[derive(Clone, Debug)]
pub struct PagedTensorStorage {
    metadata: WeightTensorMetadata,
    bytes: PagedBlobRange,
}

impl PagedTensorStorage {
    fn new(metadata: WeightTensorMetadata, bytes: PagedBlobRange) -> Self {
        Self { metadata, bytes }
    }

    /// Returns the stable tensor metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightTensorMetadata {
        &self.metadata
    }

    /// Returns the metadata for the underlying blob.
    #[must_use]
    pub fn blob_metadata(&self) -> &LocalBlobMetadata {
        self.bytes.blob_metadata()
    }

    /// Returns the starting byte offset inside the blob.
    #[must_use]
    pub fn blob_offset(&self) -> usize {
        self.bytes.offset()
    }

    /// Returns the tensor byte length.
    #[must_use]
    pub fn byte_length(&self) -> usize {
        self.bytes.len()
    }

    /// Returns the logical page size.
    #[must_use]
    pub fn page_size(&self) -> usize {
        self.bytes.page_size()
    }

    /// Returns the total page count for the tensor bytes.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.bytes.page_count()
    }

    /// Returns the full tensor bytes.
    pub fn bytes(&self) -> Result<&[u8], ModelLoadError> {
        self.bytes.bytes().map_err(ModelLoadError::from)
    }

    /// Returns a single page inside the tensor byte range.
    pub fn page(&self, page_index: usize) -> Result<&[u8], ModelLoadError> {
        self.bytes.page(page_index).map_err(ModelLoadError::from)
    }

    /// Returns a validated byte slice inside the paged tensor bytes.
    pub fn read_range(&self, offset: usize, len: usize) -> Result<&[u8], ModelLoadError> {
        self.bytes
            .read_range(offset, len)
            .map_err(ModelLoadError::from)
    }

    /// Materializes the paged bytes into a loaded tensor.
    pub fn load(&self) -> Result<LoadedWeightTensor, ModelLoadError> {
        let bytes = self.bytes()?;
        if self.metadata.quantization == QuantizationMode::None {
            let values = match self.metadata.dtype {
                DType::F32 => decode_f32_values("gguf", &self.metadata.name, bytes)?,
                DType::F16 => decode_f16_values("gguf", &self.metadata.name, bytes)?,
                DType::BF16 => decode_bf16_values("gguf", &self.metadata.name, bytes)?,
                other => {
                    return Err(ModelLoadError::UnsupportedTensorDType {
                        name: self.metadata.name.clone(),
                        dtype: format!("{other:?}"),
                    });
                }
            };
            return Ok(LoadedWeightTensor::new(self.metadata.clone(), values));
        }

        LoadedWeightTensor::from_ggml_blocks(
            self.metadata.name.clone(),
            self.metadata.shape.clone(),
            self.metadata.quantization,
            bytes.to_vec(),
        )
    }
}

/// Parsed GGUF metadata backed by a local blob that supports paged tensor reads.
#[derive(Clone, Debug)]
pub struct GgufBlobArtifact {
    content: GgufContent,
    blob: LocalBlob,
    artifact: WeightArtifactMetadata,
}

impl GgufBlobArtifact {
    /// Opens a GGUF artifact directly from a local file path.
    pub fn open_path(
        path: impl AsRef<Path>,
        options: LocalBlobOpenOptions,
    ) -> Result<Self, ModelLoadError> {
        let blob = LocalBlob::open_path(path, LocalBlobKind::GgufFile, options)?;
        Self::from_blob(blob)
    }

    /// Opens an Ollama-managed GGUF blob from the provided models root and digest.
    pub fn open_ollama_blob(
        models_root: impl AsRef<Path>,
        digest: &str,
        options: LocalBlobOpenOptions,
    ) -> Result<Self, ModelLoadError> {
        let blob = LocalBlob::open_ollama_blob(models_root, digest, options)?;
        Self::from_blob(blob)
    }

    /// Opens the primary GGUF model layer from a resolved local Ollama manifest.
    pub fn open_ollama_manifest(
        manifest: &OllamaManifest,
        options: LocalBlobOpenOptions,
    ) -> Result<Self, ModelLoadError> {
        let adapter_policy = manifest.adapter_policy_status();
        if !adapter_policy.supported {
            return Err(ModelLoadError::UnsupportedOllamaAdapterPolicy {
                model: manifest.name.canonical_name(),
                adapter_layers: adapter_policy.adapter_layer_count,
                policy: adapter_policy.policy,
            });
        }
        let layer =
            manifest
                .primary_model_layer()
                .ok_or_else(|| ModelLoadError::ArtifactFormat {
                    format: String::from("gguf"),
                    message: String::from("ollama manifest does not carry a primary model layer"),
                })?;
        let integrity = manifest.verify_integrity(options.clone());
        if let Some(diagnostic) = integrity.diagnostics.iter().find(|diagnostic| {
            diagnostic.layer_kind == Some(psionic_catalog::OllamaLayerKind::Model)
        }) {
            return Err(ModelLoadError::ArtifactRead {
                path: layer.blob_path.display().to_string(),
                message: format!(
                    "ollama manifest integrity verification failed: {}",
                    diagnostic.message
                ),
            });
        }
        let blob = layer
            .open_blob(options)
            .map_err(|error| ModelLoadError::ArtifactRead {
                path: layer.blob_path.display().to_string(),
                message: error.to_string(),
            })?;
        Self::from_blob(blob)
    }

    fn from_blob(blob: LocalBlob) -> Result<Self, ModelLoadError> {
        let content = GgufContent::read(blob.bytes())?;
        let artifact = WeightArtifactMetadata::for_blob(blob.metadata());
        Ok(Self {
            content,
            blob,
            artifact,
        })
    }

    /// Returns the parsed GGUF content.
    #[must_use]
    pub fn content(&self) -> &GgufContent {
        &self.content
    }

    /// Returns the opened blob metadata.
    #[must_use]
    pub fn blob_metadata(&self) -> &LocalBlobMetadata {
        self.blob.metadata()
    }

    /// Returns artifact metadata suitable for weight-bundle evidence.
    #[must_use]
    pub fn artifact_metadata(&self) -> &WeightArtifactMetadata {
        &self.artifact
    }

    /// Returns paged tensor storage for the named tensor.
    pub fn paged_tensor(&self, name: &str) -> Result<PagedTensorStorage, ModelLoadError> {
        let tensor = self
            .content
            .tensor_info(name)
            .ok_or_else(|| ModelLoadError::MissingTensor(String::from(name)))?;
        let (offset, byte_length) = self.content.tensor_byte_range(name)?;
        let bytes = self.blob.paged_range(offset, byte_length)?;
        Ok(PagedTensorStorage::new(tensor.weight_metadata()?, bytes))
    }

    /// Loads the named tensor through the paged blob path.
    pub fn load_tensor(&self, name: &str) -> Result<LoadedWeightTensor, ModelLoadError> {
        self.paged_tensor(name)?.load()
    }
}

/// Supported GGUF tokenizer model families Psionic can reconstruct from metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GgufTokenizerModel {
    /// SentencePiece-style tokenizer using GGML `llama` metadata.
    SentencePiece,
    /// GPT-2 style byte-level BPE tokenizer using GGML `gpt2` metadata.
    Gpt2Bpe,
    /// BERT-style wordpiece tokenizer using GGML `bert` metadata.
    BertWordPiece,
}

impl GgufTokenizerModel {
    fn from_gguf_name(value: &str) -> Result<Self, ModelLoadError> {
        match value {
            "llama" => Ok(Self::SentencePiece),
            "gpt2" => Ok(Self::Gpt2Bpe),
            "bert" => Ok(Self::BertWordPiece),
            other => Err(ModelLoadError::UnsupportedTokenizerModel {
                model: other.to_string(),
            }),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SentencePiece => "sentencepiece",
            Self::Gpt2Bpe => "gpt2_bpe",
            Self::BertWordPiece => "bert_wordpiece",
        }
    }
}

/// GGUF GPT-style BPE pretokenizer family extracted from `tokenizer.ggml.pre`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GgufTokenizerPretokenizer {
    /// Default GGML/Ollama GPT-style pretokenizer.
    Default,
    /// Llama-style fallback pretokenizer family.
    Llama,
    /// Qwen2 tokenizer family.
    Qwen2,
    /// Refact tokenizer family.
    Refact,
    /// Tekken tokenizer family.
    Tekken,
    /// Any nonstandard pretokenizer string preserved verbatim.
    Custom(String),
}

impl GgufTokenizerPretokenizer {
    fn from_gguf_name(value: &str) -> Self {
        match value {
            "default" => Self::Default,
            "llama-bpe" | "llama" => Self::Llama,
            "qwen2" => Self::Qwen2,
            "refact" => Self::Refact,
            "tekken" => Self::Tekken,
            other => Self::Custom(other.to_string()),
        }
    }

    fn digest_label(&self) -> Cow<'_, str> {
        match self {
            Self::Default => Cow::Borrowed("default"),
            Self::Llama => Cow::Borrowed("llama"),
            Self::Qwen2 => Cow::Borrowed("qwen2"),
            Self::Refact => Cow::Borrowed("refact"),
            Self::Tekken => Cow::Borrowed("tekken"),
            Self::Custom(value) => Cow::Owned(format!("custom:{value}")),
        }
    }
}

/// GGUF tokenizer vocabulary reconstructed from model metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufTokenizerVocabulary {
    tokens: Vec<String>,
    bos_token_id: Option<TokenId>,
    eos_token_ids: Vec<TokenId>,
    pad_token_id: Option<TokenId>,
    unknown_token_id: Option<TokenId>,
}

impl GgufTokenizerVocabulary {
    /// Returns the tokenizer vocabulary size.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Returns whether the vocabulary is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Returns all tokens in stable ID order.
    #[must_use]
    pub fn tokens(&self) -> &[String] {
        self.tokens.as_slice()
    }

    /// Returns a token string by token ID.
    #[must_use]
    pub fn token(&self, id: TokenId) -> Option<&str> {
        self.tokens.get(id.as_u32() as usize).map(String::as_str)
    }

    /// Returns the token ID for an exact token string.
    #[must_use]
    pub fn token_id(&self, token: &str) -> Option<TokenId> {
        self.tokens
            .iter()
            .position(|value| value == token)
            .map(|index| TokenId(index as u32))
    }

    /// Returns the configured BOS token ID.
    #[must_use]
    pub const fn bos_token_id(&self) -> Option<TokenId> {
        self.bos_token_id
    }

    /// Returns the configured EOS token IDs in stable order.
    #[must_use]
    pub fn eos_token_ids(&self) -> &[TokenId] {
        self.eos_token_ids.as_slice()
    }

    /// Returns the configured padding token ID.
    #[must_use]
    pub const fn pad_token_id(&self) -> Option<TokenId> {
        self.pad_token_id
    }

    /// Returns the configured unknown token ID.
    #[must_use]
    pub const fn unknown_token_id(&self) -> Option<TokenId> {
        self.unknown_token_id
    }
}

/// Reusable GGUF tokenizer metadata reconstructed from a GGUF artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GgufTokenizerMetadata {
    /// Tokenizer model family.
    pub model: GgufTokenizerModel,
    /// Vocabulary and special-token IDs.
    pub vocabulary: GgufTokenizerVocabulary,
    /// Token scores when the GGUF metadata provides them.
    pub scores: Vec<f32>,
    /// Raw token-type IDs preserved in tokenizer order.
    pub token_types: Vec<i32>,
    /// BPE merge rules in stable GGUF order.
    pub merges: Vec<String>,
    /// Whether callers should prepend BOS by default.
    pub add_bos: bool,
    /// Whether callers should append EOS by default.
    pub add_eos: bool,
    /// GPT-style BPE pretokenizer family when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pretokenizer: Option<GgufTokenizerPretokenizer>,
    /// Token-type vocabulary width when the tokenizer declares it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type_count: Option<usize>,
    /// Stable digest over tokenizer metadata relevant to serving behavior.
    pub digest: String,
}

impl GgufTokenizerMetadata {
    /// Returns the stable tokenizer digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        self.digest.as_str()
    }
}

/// Reusable GGUF chat-template metadata reconstructed from a GGUF artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufChatTemplateMetadata {
    default_template: Option<String>,
    named_templates: BTreeMap<String, String>,
    digest: String,
}

impl GgufChatTemplateMetadata {
    fn new(default_template: Option<String>, named_templates: BTreeMap<String, String>) -> Self {
        let digest = digest_gguf_chat_templates(default_template.as_deref(), &named_templates);
        Self {
            default_template,
            named_templates,
            digest,
        }
    }

    /// Returns the default chat template when present.
    #[must_use]
    pub fn default_template(&self) -> Option<&str> {
        self.default_template.as_deref()
    }

    /// Returns all named templates in stable key order.
    #[must_use]
    pub fn named_templates(&self) -> &BTreeMap<String, String> {
        &self.named_templates
    }

    /// Returns a template by name, or the default template when `name` is `None`.
    #[must_use]
    pub fn template(&self, name: Option<&str>) -> Option<&str> {
        match name {
            Some(name) => self.named_templates.get(name).map(String::as_str),
            None => self.default_template(),
        }
    }

    /// Returns whether the artifact carries no chat templates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.default_template.is_none() && self.named_templates.is_empty()
    }

    /// Returns the stable digest over all carried chat templates.
    #[must_use]
    pub fn digest(&self) -> &str {
        self.digest.as_str()
    }
}

/// Prompt message role used by the supported GGUF prompt-rendering surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptMessageRole {
    /// System instruction.
    System,
    /// Developer instruction.
    Developer,
    /// End-user input.
    User,
    /// Assistant output already present in history.
    Assistant,
    /// Tool result message.
    Tool,
}

impl PromptMessageRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Developer => "developer",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

/// Prompt message consumed by the supported GGUF prompt renderer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptMessage {
    /// Message role.
    pub role: PromptMessageRole,
    /// Message content.
    pub content: String,
    /// Author name when the role carries one, such as a named tool result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_name: Option<String>,
    /// Explicit recipient when the message targets a specific tool or assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    /// Explicit Harmony channel when the message already carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Explicit content-type suffix when the message needs one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Assistant reasoning content that should map to the Harmony analysis channel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

impl PromptMessage {
    /// Creates a prompt message.
    #[must_use]
    pub fn new(role: PromptMessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            author_name: None,
            recipient: None,
            channel: None,
            content_type: None,
            reasoning_content: None,
        }
    }

    /// Attaches an explicit author name.
    #[must_use]
    pub fn with_author_name(mut self, author_name: impl Into<String>) -> Self {
        self.author_name = Some(author_name.into());
        self
    }

    /// Attaches an explicit recipient.
    #[must_use]
    pub fn with_recipient(mut self, recipient: impl Into<String>) -> Self {
        self.recipient = Some(recipient.into());
        self
    }

    /// Attaches an explicit Harmony channel.
    #[must_use]
    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    /// Attaches an explicit content type suffix.
    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Attaches assistant reasoning content for GPT-OSS / Harmony prompts.
    #[must_use]
    pub fn with_reasoning_content(mut self, reasoning_content: impl Into<String>) -> Self {
        self.reasoning_content = Some(reasoning_content.into());
        self
    }
}

/// Supported GGUF prompt-template families with explicit render compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GgufPromptTemplateFamily {
    /// Phi-3 template family.
    Phi3,
    /// Qwen2 template family.
    Qwen2,
    /// Command-R template family.
    CommandR,
    /// GPT-OSS template family rendered through Harmony semantics.
    GptOss,
}

impl GgufPromptTemplateFamily {
    fn stop_sequences(self) -> &'static [&'static str] {
        match self {
            Self::Phi3 => &["<|end|>", "<|system|>", "<|user|>", "<|assistant|>"],
            Self::Qwen2 => &[],
            Self::CommandR => &["<|START_OF_TURN_TOKEN|>", "<|END_OF_TURN_TOKEN|>"],
            Self::GptOss => &[],
        }
    }
}

/// Rendered prompt plus explicit template truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedPrompt {
    /// Fully rendered prompt text.
    pub text: String,
    /// Selected template name when a named GGUF template was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_name: Option<String>,
    /// Stable digest of the selected raw template.
    pub template_digest: String,
    /// Supported template family that produced the prompt.
    pub family: GgufPromptTemplateFamily,
    /// Explicit stop defaults associated with the template family.
    pub stop_sequences: Vec<String>,
}

/// Prompt-render failure for the supported GGUF prompt surface.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PromptRenderError {
    /// The requested template name was not present in GGUF metadata.
    #[error("missing gguf chat template `{name}`")]
    MissingTemplateName {
        /// Requested template name.
        name: String,
    },
    /// The GGUF artifact carries no default chat template.
    #[error("gguf artifact does not carry a default chat template")]
    MissingDefaultTemplate,
    /// The template digest is not one of the supported prompt families yet.
    #[error("unsupported gguf prompt template digest `{digest}`")]
    UnsupportedTemplateDigest {
        /// Stable digest of the selected raw template.
        digest: String,
    },
    /// The prompt history cannot be rendered honestly for the selected family.
    #[error("invalid prompt conversation: {message}")]
    InvalidConversation {
        /// Validation failure summary.
        message: String,
    },
    /// The GPT-OSS / Harmony render path failed.
    #[error("failed to render gpt-oss harmony prompt: {message}")]
    HarmonyRendering {
        /// Lower-level failure summary.
        message: String,
    },
}

/// Supported GGUF prompt renderer over extracted tokenizer and chat-template metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct GgufPromptTemplateRenderer {
    tokenizer: GgufTokenizerMetadata,
    chat_templates: GgufChatTemplateMetadata,
}

impl GgufPromptTemplateRenderer {
    /// Creates a renderer from GGUF tokenizer and chat-template metadata.
    #[must_use]
    pub fn new(tokenizer: GgufTokenizerMetadata, chat_templates: GgufChatTemplateMetadata) -> Self {
        Self {
            tokenizer,
            chat_templates,
        }
    }

    /// Renders a prompt for a supported chat template.
    pub fn render(
        &self,
        template_name: Option<&str>,
        messages: &[PromptMessage],
        add_generation_prompt: bool,
    ) -> Result<RenderedPrompt, PromptRenderError> {
        self.render_with_options(
            template_name,
            messages,
            add_generation_prompt,
            &PromptRenderOptions::default(),
        )
    }

    /// Renders a prompt for a supported chat template with explicit family options.
    pub fn render_with_options(
        &self,
        template_name: Option<&str>,
        messages: &[PromptMessage],
        add_generation_prompt: bool,
        options: &PromptRenderOptions,
    ) -> Result<RenderedPrompt, PromptRenderError> {
        let template_name_owned = template_name.map(str::to_string);
        let raw_template = match template_name {
            Some(name) => self.chat_templates.template(Some(name)).ok_or_else(|| {
                PromptRenderError::MissingTemplateName {
                    name: name.to_string(),
                }
            })?,
            None => self
                .chat_templates
                .default_template()
                .ok_or(PromptRenderError::MissingDefaultTemplate)?,
        };
        let template_digest = digest_chat_template(raw_template);
        let family =
            supported_prompt_template_family(template_digest.as_str()).ok_or_else(|| {
                PromptRenderError::UnsupportedTemplateDigest {
                    digest: template_digest.clone(),
                }
            })?;
        let rendered = match family {
            GgufPromptTemplateFamily::Phi3 => self.render_phi3(messages),
            GgufPromptTemplateFamily::Qwen2 => self.render_qwen2(messages, add_generation_prompt),
            GgufPromptTemplateFamily::CommandR => {
                self.render_command_r(messages, add_generation_prompt)
            }
            GgufPromptTemplateFamily::GptOss => {
                render_gpt_oss_harmony_prompt(messages, add_generation_prompt, Some(options))
                    .map_err(|error| PromptRenderError::HarmonyRendering {
                        message: error.to_string(),
                    })
            }
        }?;

        Ok(RenderedPrompt {
            text: rendered,
            template_name: template_name_owned,
            template_digest,
            family,
            stop_sequences: family
                .stop_sequences()
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        })
    }

    fn render_phi3(&self, messages: &[PromptMessage]) -> Result<String, PromptRenderError> {
        let mut rendered = self.bos_token().unwrap_or_default().to_string();
        for message in messages {
            match message.role {
                PromptMessageRole::User => {
                    rendered.push_str("<|user|>\n");
                    rendered.push_str(message.content.as_str());
                    rendered.push_str("<|end|>\n<|assistant|>\n");
                }
                PromptMessageRole::Assistant => {
                    rendered.push_str(message.content.as_str());
                    rendered.push_str("<|end|>\n");
                }
                PromptMessageRole::System
                | PromptMessageRole::Developer
                | PromptMessageRole::Tool => {}
            }
        }
        Ok(rendered)
    }

    fn render_qwen2(
        &self,
        messages: &[PromptMessage],
        add_generation_prompt: bool,
    ) -> Result<String, PromptRenderError> {
        if messages.is_empty() {
            return Err(PromptRenderError::InvalidConversation {
                message: String::from("qwen2 prompt rendering requires at least one message"),
            });
        }

        let mut rendered = String::new();
        if messages[0].role != PromptMessageRole::System {
            rendered.push_str("<|im_start|>system\nYou are a helpful assistant<|im_end|>\n");
        }
        for message in messages {
            rendered.push_str("<|im_start|>");
            rendered.push_str(message.role.as_str());
            rendered.push('\n');
            rendered.push_str(message.content.as_str());
            rendered.push_str("<|im_end|>\n");
        }
        if add_generation_prompt {
            rendered.push_str("<|im_start|>assistant\n");
        }
        Ok(rendered)
    }

    fn render_command_r(
        &self,
        messages: &[PromptMessage],
        add_generation_prompt: bool,
    ) -> Result<String, PromptRenderError> {
        if messages.is_empty() {
            return Err(PromptRenderError::InvalidConversation {
                message: String::from("command-r prompt rendering requires at least one message"),
            });
        }

        let mut rendered = self.bos_token().unwrap_or_default().to_string();
        let mut loop_messages = messages;
        if let Some(first) = messages.first() {
            if first.role == PromptMessageRole::System {
                rendered.push_str("<|START_OF_TURN_TOKEN|><|SYSTEM_TOKEN|>");
                rendered.push_str(first.content.as_str());
                rendered.push_str("<|END_OF_TURN_TOKEN|>");
                loop_messages = &messages[1..];
            }
        }

        for (index, message) in loop_messages.iter().enumerate() {
            let expected_role = if index % 2 == 0 {
                PromptMessageRole::User
            } else {
                PromptMessageRole::Assistant
            };
            if message.role != expected_role {
                return Err(PromptRenderError::InvalidConversation {
                    message: String::from(
                        "command-r messages must alternate user/assistant after an optional leading system message",
                    ),
                });
            }

            match message.role {
                PromptMessageRole::User => {
                    rendered.push_str("<|START_OF_TURN_TOKEN|><|USER_TOKEN|>");
                    rendered.push_str(message.content.trim());
                    rendered.push_str("<|END_OF_TURN_TOKEN|>");
                }
                PromptMessageRole::Assistant => {
                    rendered.push_str("<|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>");
                    rendered.push_str(message.content.trim());
                    rendered.push_str("<|END_OF_TURN_TOKEN|>");
                }
                PromptMessageRole::System
                | PromptMessageRole::Developer
                | PromptMessageRole::Tool => {
                    return Err(PromptRenderError::InvalidConversation {
                        message: String::from(
                            "command-r only supports user and assistant turns after the optional leading system message",
                        ),
                    });
                }
            }
        }
        if add_generation_prompt {
            rendered.push_str("<|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>");
        }
        Ok(rendered)
    }

    fn bos_token(&self) -> Option<&str> {
        self.tokenizer
            .vocabulary
            .bos_token_id()
            .and_then(|token_id| self.tokenizer.vocabulary.token(token_id))
    }
}

fn supported_prompt_template_family(digest: &str) -> Option<GgufPromptTemplateFamily> {
    match digest {
        "268b6082ceb7176dc6ed80557a2f7837f9f0339592fbee677d405a553af15f88" => {
            Some(GgufPromptTemplateFamily::Phi3)
        }
        "af9c0233881b083b52ff773580215222b5440ac3d0beeeca99b76329b048f8db" => {
            Some(GgufPromptTemplateFamily::Qwen2)
        }
        "9db2cf47ce03bfd0aab6ec59942503714fa0372f09f7e1d54cbcd71a1110b863" => {
            Some(GgufPromptTemplateFamily::CommandR)
        }
        "a4c9919cbbd4acdd51ccffe22da049264b1b73e59055fa58811a99efbd7c8146" => {
            Some(GgufPromptTemplateFamily::GptOss)
        }
        _ => None,
    }
}

/// First-launch GGUF decoder family classification used by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GgufDecoderFamily {
    /// Llama-family decoder behavior.
    Llama,
    /// Qwen-family decoder behavior backed by the `qwen2` GGUF architecture today.
    Qwen,
    /// Mistral-family decoder behavior, including legacy Mistral models carried through `llama` GGUF metadata.
    Mistral,
    /// GPT-OSS / OpenAI-MoE decoder behavior.
    GptOss,
}

impl GgufDecoderFamily {
    fn as_str(self) -> &'static str {
        match self {
            Self::Llama => "llama",
            Self::Qwen => "qwen",
            Self::Mistral => "mistral",
            Self::GptOss => "gpt_oss",
        }
    }
}

/// Family-specific GGUF decoder metadata kept outside the generic decoder descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GgufDecoderFamilyMetadata {
    /// Launch-family label used by higher layers.
    pub family: GgufDecoderFamily,
    /// Raw GGUF `general.architecture` label.
    pub architecture: String,
    /// Human-readable model name when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Effective RoPE frequency base.
    pub rope_theta: f32,
    /// Effective RoPE scaling factor when the artifact declares scaled context behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rope_scaling_factor: Option<f32>,
    /// Original training context length for scaled RoPE when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rope_original_context_length: Option<usize>,
    /// Effective RMSNorm epsilon.
    pub rms_norm_epsilon: f32,
    /// Sliding-window attention bound when the artifact declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sliding_window: Option<usize>,
    /// Per-head key/query width when the artifact declares it explicitly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_key_length: Option<usize>,
    /// Per-head value width when the artifact declares it explicitly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_value_length: Option<usize>,
    /// Whether the artifact uses tied token embedding / LM head weights.
    pub tie_word_embeddings: bool,
    /// Whether the adapter expects explicit Q/K/V bias tensors.
    pub attention_qkv_biases: bool,
    /// Mixture-of-experts expert count when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expert_count: Option<usize>,
    /// Routed expert count per token when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expert_used_count: Option<usize>,
    /// Expert feed-forward width when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expert_feed_forward_length: Option<usize>,
}

/// Family-specific tensor naming for one GGUF decoder layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufDecoderLayerTensorLayout {
    /// Zero-based decoder layer index.
    pub layer_index: usize,
    /// Attention input norm tensor.
    pub attention_norm: String,
    /// Attention query weight tensor.
    pub attention_query_weight: String,
    /// Attention query bias tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_query_bias: Option<String>,
    /// Attention key weight tensor.
    pub attention_key_weight: String,
    /// Attention key bias tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_key_bias: Option<String>,
    /// Attention value weight tensor.
    pub attention_value_weight: String,
    /// Attention value bias tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_value_bias: Option<String>,
    /// Attention output projection tensor.
    pub attention_output_weight: String,
    /// Attention output projection bias when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_output_bias: Option<String>,
    /// Post-attention norm tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_post_norm: Option<String>,
    /// Attention sinks tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_sinks_weight: Option<String>,
    /// Feed-forward gate projection tensor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_gate_weight: Option<String>,
    /// Feed-forward down projection tensor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_down_weight: Option<String>,
    /// Feed-forward up projection tensor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_up_weight: Option<String>,
    /// Feed-forward norm tensor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_norm: Option<String>,
    /// Router tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_router_weight: Option<String>,
    /// Router bias when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_router_bias: Option<String>,
    /// Expert gate tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_gate_experts_weight: Option<String>,
    /// Expert gate bias when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_gate_experts_bias: Option<String>,
    /// Expert up tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_up_experts_weight: Option<String>,
    /// Expert up bias when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_up_experts_bias: Option<String>,
    /// Expert down tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_down_experts_weight: Option<String>,
    /// Expert down bias when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_down_experts_bias: Option<String>,
}

/// Reusable tensor-name layout for a GGUF-backed decoder family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufDecoderTensorLayout {
    /// Token embedding tensor.
    pub token_embedding: String,
    /// Final norm tensor.
    pub output_norm: String,
    /// LM-head tensor when it is stored separately.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Decoder-layer tensor layouts in stable layer order.
    pub layers: Vec<GgufDecoderLayerTensorLayout>,
}

/// First-launch GGUF embedding family classification used by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GgufEmbeddingFamily {
    /// BERT-style embedding encoder behavior.
    Bert,
    /// Nomic BERT-style embedding encoder behavior.
    #[serde(rename = "nomic-bert")]
    NomicBert,
}

impl GgufEmbeddingFamily {
    fn as_str(self) -> &'static str {
        match self {
            Self::Bert => "bert",
            Self::NomicBert => "nomic-bert",
        }
    }
}

/// Embedding pooling mode reconstructed from GGUF metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GgufEmbeddingPooling {
    /// No sequence pooling was requested.
    None,
    /// Mean-pool token states over the sequence.
    Mean,
    /// Pool from the CLS token state.
    Cls,
    /// Pool from the final token state.
    Last,
    /// Pool from a rank/classification head.
    Rank,
}

impl GgufEmbeddingPooling {
    fn from_gguf_value(value: usize, key: &str) -> Result<Self, ModelLoadError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Mean),
            2 => Ok(Self::Cls),
            3 => Ok(Self::Last),
            4 => Ok(Self::Rank),
            other => Err(ModelLoadError::InvalidGgufMetadata {
                key: key.to_string(),
                message: format!("unsupported pooling type `{other}`"),
            }),
        }
    }
}

/// Family-specific GGUF embeddings metadata kept outside the generic embedding descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GgufEmbeddingFamilyMetadata {
    /// Launch-family label used by higher layers.
    pub family: GgufEmbeddingFamily,
    /// Raw GGUF `general.architecture` label.
    pub architecture: String,
    /// Human-readable model name when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Maximum supported context length.
    pub max_context: usize,
    /// Encoder layer count.
    pub layer_count: usize,
    /// Attention head count.
    pub attention_head_count: usize,
    /// Attention KV head count when it differs from query heads.
    pub attention_kv_head_count: usize,
    /// Effective layer norm epsilon.
    pub layer_norm_epsilon: f32,
    /// Effective RoPE frequency base when the family uses rotary positions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rope_theta: Option<f32>,
    /// Pooling mode applied to per-token outputs.
    pub pooling: GgufEmbeddingPooling,
    /// Output normalization policy.
    pub normalization: EmbeddingNormalization,
    /// Whether the family uses explicit absolute position embeddings.
    pub uses_position_embeddings: bool,
    /// Whether the family stores fused QKV projection weights.
    pub uses_fused_qkv: bool,
}

/// Family-specific tensor naming for one GGUF embedding layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufEmbeddingLayerTensorLayout {
    /// Zero-based encoder layer index.
    pub layer_index: usize,
    /// Attention output norm tensor.
    pub attention_output_norm: String,
    /// Separate attention query weight tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_query_weight: Option<String>,
    /// Separate attention query bias tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_query_bias: Option<String>,
    /// Separate attention query norm tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_query_norm: Option<String>,
    /// Separate attention key weight tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_key_weight: Option<String>,
    /// Separate attention key bias tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_key_bias: Option<String>,
    /// Separate attention key norm tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_key_norm: Option<String>,
    /// Separate attention value weight tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_value_weight: Option<String>,
    /// Separate attention value bias tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_value_bias: Option<String>,
    /// Fused attention QKV weight tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_qkv_weight: Option<String>,
    /// Fused attention QKV bias tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_qkv_bias: Option<String>,
    /// Attention output projection tensor.
    pub attention_output_weight: String,
    /// Attention output bias tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_output_bias: Option<String>,
    /// Feed-forward gate tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_gate_weight: Option<String>,
    /// Feed-forward gate bias when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_gate_bias: Option<String>,
    /// Feed-forward up tensor.
    pub feed_forward_up_weight: String,
    /// Feed-forward up bias when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_up_bias: Option<String>,
    /// Feed-forward down tensor.
    pub feed_forward_down_weight: String,
    /// Feed-forward down bias when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_down_bias: Option<String>,
    /// Feed-forward router tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_router_weight: Option<String>,
    /// Feed-forward router bias when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_router_bias: Option<String>,
    /// Expert up projections when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_up_experts_weight: Option<String>,
    /// Expert down projections when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_forward_down_experts_weight: Option<String>,
    /// Final MLP output norm tensor.
    pub layer_output_norm: String,
}

/// Reusable tensor-name layout for a GGUF-backed embeddings family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufEmbeddingTensorLayout {
    /// Token embedding tensor.
    pub token_embedding: String,
    /// Token-type embedding tensor.
    pub token_type_embedding: String,
    /// Token embedding normalization tensor.
    pub token_embedding_norm: String,
    /// Position embedding tensor when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_embedding: Option<String>,
    /// Encoder-layer tensor layouts in stable layer order.
    pub layers: Vec<GgufEmbeddingLayerTensorLayout>,
}

/// Backend-neutral quantized GGML/GGUF block storage preserved by the loader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantizedTensorStorage {
    quantization: QuantizationMode,
    layout: QuantizedBlockLayout,
    bytes: Vec<u8>,
    digest: String,
}

impl QuantizedTensorStorage {
    /// Creates quantized storage from validated GGML/GGUF block bytes.
    pub fn from_ggml_blocks(
        quantization: QuantizationMode,
        shape: &Shape,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, ModelLoadError> {
        let bytes = bytes.into();
        let Some(layout) = quantization.ggml_block_layout(shape) else {
            return Err(ModelLoadError::InvalidQuantizedTensorShape {
                quantization,
                shape: shape.dims().to_vec(),
            });
        };
        if bytes.len() != layout.byte_len() {
            return Err(ModelLoadError::InvalidQuantizedTensorByteLength {
                quantization,
                expected: layout.byte_len(),
                actual: bytes.len(),
            });
        }

        let mut hasher = Sha256::new();
        hasher.update(format!("{quantization:?}").as_bytes());
        hasher.update(b"|");
        hasher.update((layout.elements_per_block as u64).to_be_bytes());
        hasher.update((layout.bytes_per_block as u64).to_be_bytes());
        hasher.update((layout.block_count as u64).to_be_bytes());
        hasher.update(b"|");
        hasher.update(&bytes);

        Ok(Self {
            quantization,
            layout,
            bytes,
            digest: hex::encode(hasher.finalize()),
        })
    }

    /// Returns the quantization family for the storage.
    #[must_use]
    pub const fn quantization(&self) -> QuantizationMode {
        self.quantization
    }

    /// Returns the stable block layout.
    #[must_use]
    pub const fn layout(&self) -> QuantizedBlockLayout {
        self.layout
    }

    /// Returns the serialized block bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    /// Returns the stable storage digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        self.digest.as_str()
    }

    /// Dequantizes the stored GGML/GGUF blocks into logical `f32` values.
    pub fn dequantize_values(&self) -> Result<Vec<f32>, ModelLoadError> {
        decode_ggml_quantized_values(self.quantization, self.layout, self.bytes())
    }
}

/// Explicit tensor storage returned by a loader.
#[derive(Clone, Debug, PartialEq)]
pub enum WeightTensorStorage {
    /// Dense `f32` values already materialized on the host.
    DequantizedF32(Vec<f32>),
    /// Quantized GGML/GGUF blocks preserved by the loader.
    QuantizedBlocks(QuantizedTensorStorage),
}

/// Loaded tensor values for a weight bundle.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedWeightTensor {
    metadata: WeightTensorMetadata,
    storage: WeightTensorStorage,
}

impl LoadedWeightTensor {
    /// Creates a loaded tensor payload.
    #[must_use]
    pub fn new(metadata: WeightTensorMetadata, values: Vec<f32>) -> Self {
        Self {
            metadata,
            storage: WeightTensorStorage::DequantizedF32(values),
        }
    }

    /// Creates a tensor payload that preserves quantized GGML/GGUF block storage.
    pub fn from_ggml_blocks(
        name: impl Into<String>,
        shape: Shape,
        quantization: QuantizationMode,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, ModelLoadError> {
        let storage = QuantizedTensorStorage::from_ggml_blocks(quantization, &shape, bytes)?;
        let metadata = WeightTensorMetadata::new(name, shape, DType::F32)
            .with_quantization(quantization)
            .with_quantized_layout(storage.layout());
        Ok(Self {
            metadata,
            storage: WeightTensorStorage::QuantizedBlocks(storage),
        })
    }

    /// Returns the stable tensor metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightTensorMetadata {
        &self.metadata
    }

    /// Returns the explicit storage representation.
    #[must_use]
    pub fn storage(&self) -> &WeightTensorStorage {
        &self.storage
    }

    /// Returns the logical tensor values as `f32`.
    pub fn values(&self) -> Result<Cow<'_, [f32]>, ModelLoadError> {
        match &self.storage {
            WeightTensorStorage::DequantizedF32(values) => Ok(Cow::Borrowed(values.as_slice())),
            WeightTensorStorage::QuantizedBlocks(storage) => {
                Ok(Cow::Owned(storage.dequantize_values()?))
            }
        }
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
            let (dtype, quantization, loaded_tensor) = match tensor.dtype() {
                SafeTensorsDType::F32 => (
                    DType::F32,
                    QuantizationMode::None,
                    LoadedWeightTensor::new(
                        WeightTensorMetadata::new(
                            name,
                            Shape::new(tensor.shape().to_vec()),
                            DType::F32,
                        ),
                        decode_f32_values("safetensors", name, tensor.data())?,
                    ),
                ),
                SafeTensorsDType::I8 => (
                    DType::I8,
                    QuantizationMode::Int8Symmetric,
                    LoadedWeightTensor::new(
                        WeightTensorMetadata::new(
                            name,
                            Shape::new(tensor.shape().to_vec()),
                            DType::I8,
                        )
                        .with_quantization(QuantizationMode::Int8Symmetric),
                        decode_int8_symmetric_values(name, tensor.data(), &tensors)?,
                    ),
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
            let tensor_metadata = loaded_tensor.metadata().clone();
            debug_assert_eq!(tensor_metadata.dtype, dtype);
            debug_assert_eq!(tensor_metadata.quantization, quantization);
            digest_loaded_tensor(&mut hasher, &loaded_tensor)?;
            metadata.push(tensor_metadata);
            loaded.insert(name.to_string(), loaded_tensor);
        }

        Ok(LoadedWeightBundle::new(
            WeightBundleMetadata {
                format: WeightFormat::SafeTensors,
                source: WeightSource::ExternalArtifact,
                quantization: bundle_quantization,
                quantization_modes: (bundle_quantization != QuantizationMode::None)
                    .then_some(vec![bundle_quantization])
                    .unwrap_or_default(),
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

/// GGUF-backed local weight bundle loader.
#[derive(Clone, Copy, Debug, Default)]
pub struct GgufWeightBundleLoader;

impl GgufWeightBundleLoader {
    fn describe_artifact(
        &self,
        artifact: &GgufBlobArtifact,
    ) -> Result<WeightBundleMetadata, ModelLoadError> {
        let content = artifact.content();
        let mut metadata = Vec::with_capacity(content.tensor_infos.len());
        let mut quantized_bytes = Vec::new();
        let mut hasher = Sha256::new();
        hasher.update(artifact.artifact_metadata().integrity_label().as_bytes());
        hasher.update(b"\n");

        for tensor_info in content.tensor_infos() {
            let tensor_metadata = tensor_info.weight_metadata()?;
            let (_, byte_length) = content.tensor_byte_range(&tensor_info.name)?;
            if tensor_metadata.quantization != QuantizationMode::None {
                track_quantized_bytes(
                    &mut quantized_bytes,
                    tensor_metadata.quantization,
                    byte_length,
                );
            }
            digest_tensor_metadata(&mut hasher, &tensor_metadata);
            metadata.push(tensor_metadata);
        }

        Ok(WeightBundleMetadata {
            format: WeightFormat::Gguf,
            source: WeightSource::ExternalArtifact,
            quantization: dominant_quantization_mode(&quantized_bytes),
            quantization_modes: quantized_bytes
                .iter()
                .filter_map(|(mode, _)| (*mode != QuantizationMode::None).then_some(*mode))
                .collect(),
            digest: hex::encode(hasher.finalize()),
            tensors: metadata,
            artifacts: vec![artifact.artifact_metadata().clone()],
        })
    }

    fn load_artifact(
        &self,
        artifact: &GgufBlobArtifact,
    ) -> Result<LoadedWeightBundle, ModelLoadError> {
        let content = artifact.content();
        let mut metadata = Vec::with_capacity(content.tensor_infos.len());
        let mut loaded = BTreeMap::new();
        let mut hasher = Sha256::new();
        let mut quantized_bytes = Vec::new();

        for tensor_info in content.tensor_infos() {
            let tensor = artifact.load_tensor(&tensor_info.name)?;
            if let WeightTensorStorage::QuantizedBlocks(storage) = tensor.storage() {
                track_quantized_bytes(
                    &mut quantized_bytes,
                    storage.quantization(),
                    storage.bytes().len(),
                );
            }
            let tensor_metadata = tensor.metadata().clone();
            digest_loaded_tensor(&mut hasher, &tensor)?;
            metadata.push(tensor_metadata);
            loaded.insert(tensor_info.name.clone(), tensor);
        }

        Ok(LoadedWeightBundle::new(
            WeightBundleMetadata {
                format: WeightFormat::Gguf,
                source: WeightSource::ExternalArtifact,
                quantization: dominant_quantization_mode(&quantized_bytes),
                quantization_modes: quantization_modes_from_counts(&quantized_bytes),
                digest: hex::encode(hasher.finalize()),
                tensors: metadata,
                artifacts: vec![artifact.artifact_metadata().clone()],
            },
            loaded,
        ))
    }

    /// Loads a GGUF-backed bundle from a paged local blob artifact.
    pub fn load_blob_artifact(
        &self,
        artifact: &GgufBlobArtifact,
    ) -> Result<LoadedWeightBundle, ModelLoadError> {
        self.load_artifact(artifact)
    }

    /// Loads an Ollama-managed GGUF blob from the provided models root and digest.
    pub fn load_ollama_blob(
        &self,
        models_root: impl AsRef<Path>,
        digest: &str,
        options: LocalBlobOpenOptions,
    ) -> Result<LoadedWeightBundle, ModelLoadError> {
        let artifact = GgufBlobArtifact::open_ollama_blob(models_root, digest, options)?;
        self.load_artifact(&artifact)
    }

    /// Loads a GGUF-backed bundle from a resolved local Ollama manifest.
    pub fn load_ollama_manifest(
        &self,
        manifest: &OllamaManifest,
        options: LocalBlobOpenOptions,
    ) -> Result<LoadedWeightBundle, ModelLoadError> {
        let artifact = GgufBlobArtifact::open_ollama_manifest(manifest, options)?;
        self.load_artifact(&artifact)
    }
}

impl LocalWeightBundleLoader for GgufWeightBundleLoader {
    type Error = ModelLoadError;

    fn load_path(&self, path: &Path) -> Result<LoadedWeightBundle, Self::Error> {
        let artifact = GgufBlobArtifact::open_path(path, LocalBlobOpenOptions::default())?;
        self.load_artifact(&artifact)
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
    /// Stable model-side artifact identity inputs used by serving/evidence layers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_identity: Option<ServedModelArtifactMetadata>,
    /// Stable provenance and license facts for the backing artifact when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_governance: Option<ModelArtifactGovernance>,
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
            artifact_identity: None,
            artifact_governance: None,
        }
    }

    /// Attaches stable serving-identity metadata.
    #[must_use]
    pub fn with_artifact_identity(
        mut self,
        artifact_identity: ServedModelArtifactMetadata,
    ) -> Self {
        self.artifact_identity = Some(artifact_identity);
        self
    }

    /// Attaches provenance and license facts for the backing artifact.
    #[must_use]
    pub fn with_artifact_governance(
        mut self,
        artifact_governance: ModelArtifactGovernance,
    ) -> Self {
        self.artifact_governance = Some(artifact_governance);
        self
    }

    /// Returns the explicit compatibility/native boundary for this loaded model path.
    #[must_use]
    pub fn interop_boundary(&self) -> ModelInteropBoundary {
        ModelInteropBoundary {
            catalog_surface: infer_catalog_surface(self.artifact_governance.as_ref()),
            ingress_surface: infer_model_ingress_surface(
                &self.weights,
                self.artifact_governance.as_ref(),
            ),
            serving_surface: ModelServingSurface::PsionicNative,
            runtime_surface: ModelRuntimeSurface::PsionicNative,
        }
    }
}

/// Reusable GGUF-backed embeddings-family adapter for supported first-launch model families.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GgufEmbeddingAdapter {
    descriptor: EmbeddingModelDescriptor,
    family_metadata: GgufEmbeddingFamilyMetadata,
    tokenizer: GgufTokenizerMetadata,
    tensor_layout: GgufEmbeddingTensorLayout,
}

impl GgufEmbeddingAdapter {
    /// Returns the generic embeddings descriptor for higher-layer request contracts.
    #[must_use]
    pub fn descriptor(&self) -> &EmbeddingModelDescriptor {
        &self.descriptor
    }

    /// Returns the family-specific GGUF metadata.
    #[must_use]
    pub fn family_metadata(&self) -> &GgufEmbeddingFamilyMetadata {
        &self.family_metadata
    }

    /// Returns the tokenizer metadata loaded from GGUF.
    #[must_use]
    pub fn tokenizer(&self) -> &GgufTokenizerMetadata {
        &self.tokenizer
    }

    /// Returns the reusable GGUF tensor-name layout for the embeddings family.
    #[must_use]
    pub fn tensor_layout(&self) -> &GgufEmbeddingTensorLayout {
        &self.tensor_layout
    }
}

/// GGUF-backed embeddings-family adapter loader.
#[derive(Clone, Copy, Debug, Default)]
pub struct GgufEmbeddingAdapterLoader;

impl GgufEmbeddingAdapterLoader {
    fn load_blob_artifact_with_governance(
        &self,
        artifact: &GgufBlobArtifact,
        artifact_governance: ModelArtifactGovernance,
    ) -> Result<GgufEmbeddingAdapter, ModelLoadError> {
        let content = artifact.content();
        let metadata = content.metadata();
        let architecture = read_required_gguf_string(metadata, "general.architecture")?;
        let family = classify_gguf_embedding_family(architecture.as_str())?;
        validate_supported_embedding_family_features(metadata, family, architecture.as_str())?;

        let tokenizer = content.load_tokenizer()?;
        let bundle = GgufWeightBundleLoader.describe_artifact(artifact)?;
        let family_metadata =
            build_gguf_embedding_family_metadata(metadata, content, family, architecture)?;
        let descriptor = build_gguf_embedding_descriptor(
            artifact.artifact_metadata(),
            &bundle,
            &family_metadata,
            &tokenizer,
            content,
        )?
        .with_artifact_governance(artifact_governance);
        let tensor_layout =
            build_gguf_embedding_tensor_layout(content, &family_metadata, &tokenizer)?;

        Ok(GgufEmbeddingAdapter {
            descriptor,
            family_metadata,
            tokenizer,
            tensor_layout,
        })
    }

    /// Loads an embeddings-family adapter from an already-open GGUF blob artifact.
    pub fn load_blob_artifact(
        &self,
        artifact: &GgufBlobArtifact,
    ) -> Result<GgufEmbeddingAdapter, ModelLoadError> {
        let artifact_governance = match artifact.blob_metadata().kind {
            LocalBlobKind::GgufFile => {
                ModelArtifactGovernance::local_path(&artifact.blob_metadata().path)
            }
            LocalBlobKind::OllamaBlob => {
                ModelArtifactGovernance::ollama_blob(artifact.blob_metadata())
            }
        };
        self.load_blob_artifact_with_governance(artifact, artifact_governance)
    }

    /// Loads an embeddings-family adapter from a local GGUF path.
    pub fn load_path(&self, path: &Path) -> Result<GgufEmbeddingAdapter, ModelLoadError> {
        let artifact = GgufBlobArtifact::open_path(path, LocalBlobOpenOptions::default())?;
        self.load_blob_artifact(&artifact)
    }

    /// Loads an embeddings-family adapter from an Ollama-managed GGUF blob.
    pub fn load_ollama_blob(
        &self,
        models_root: impl AsRef<Path>,
        digest: &str,
        options: LocalBlobOpenOptions,
    ) -> Result<GgufEmbeddingAdapter, ModelLoadError> {
        let artifact = GgufBlobArtifact::open_ollama_blob(models_root, digest, options)?;
        self.load_blob_artifact(&artifact)
    }

    /// Loads an embeddings-family adapter from a resolved Ollama manifest.
    pub fn load_ollama_manifest(
        &self,
        manifest: &OllamaManifest,
        options: LocalBlobOpenOptions,
    ) -> Result<GgufEmbeddingAdapter, ModelLoadError> {
        let config = manifest
            .load_config(
                LocalBlobOpenOptions::default()
                    .with_read_preference(psionic_catalog::BlobReadPreference::PreferBuffered),
            )
            .map_err(|error| ModelLoadError::ArtifactRead {
                path: manifest.manifest_path.display().to_string(),
                message: error.to_string(),
            })?;
        let licenses = manifest
            .load_license_facts(
                LocalBlobOpenOptions::default()
                    .with_read_preference(psionic_catalog::BlobReadPreference::PreferBuffered),
            )
            .map_err(|error| ModelLoadError::ArtifactRead {
                path: manifest.manifest_path.display().to_string(),
                message: error.to_string(),
            })?;
        let artifact = GgufBlobArtifact::open_ollama_manifest(manifest, options)?;
        self.load_blob_artifact_with_governance(
            &artifact,
            ModelArtifactGovernance::from_ollama_manifest(manifest, config.as_ref(), licenses),
        )
    }
}

/// Reusable GGUF-backed decoder-family adapter for supported first-launch model families.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GgufDecoderAdapter {
    descriptor: DecoderModelDescriptor,
    family_metadata: GgufDecoderFamilyMetadata,
    tokenizer: GgufTokenizerMetadata,
    chat_templates: GgufChatTemplateMetadata,
    tensor_layout: GgufDecoderTensorLayout,
}

impl GgufDecoderAdapter {
    /// Returns the generic decoder descriptor for higher-layer request contracts.
    #[must_use]
    pub fn descriptor(&self) -> &DecoderModelDescriptor {
        &self.descriptor
    }

    /// Returns the family-specific GGUF metadata.
    #[must_use]
    pub fn family_metadata(&self) -> &GgufDecoderFamilyMetadata {
        &self.family_metadata
    }

    /// Returns the tokenizer metadata loaded from GGUF.
    #[must_use]
    pub fn tokenizer(&self) -> &GgufTokenizerMetadata {
        &self.tokenizer
    }

    /// Returns the chat-template metadata loaded from GGUF.
    #[must_use]
    pub fn chat_templates(&self) -> &GgufChatTemplateMetadata {
        &self.chat_templates
    }

    /// Creates a reusable prompt renderer over the adapter's tokenizer and chat templates.
    #[must_use]
    pub fn prompt_renderer(&self) -> GgufPromptTemplateRenderer {
        GgufPromptTemplateRenderer::new(self.tokenizer.clone(), self.chat_templates.clone())
    }

    /// Renders a prompt through the adapter's supported chat-template families.
    pub fn render_prompt(
        &self,
        template_name: Option<&str>,
        messages: &[PromptMessage],
        add_generation_prompt: bool,
    ) -> Result<RenderedPrompt, PromptRenderError> {
        self.render_prompt_with_options(
            template_name,
            messages,
            add_generation_prompt,
            &PromptRenderOptions::default(),
        )
    }

    /// Renders a prompt through the adapter's supported chat-template families with explicit options.
    pub fn render_prompt_with_options(
        &self,
        template_name: Option<&str>,
        messages: &[PromptMessage],
        add_generation_prompt: bool,
        options: &PromptRenderOptions,
    ) -> Result<RenderedPrompt, PromptRenderError> {
        self.prompt_renderer().render_with_options(
            template_name,
            messages,
            add_generation_prompt,
            options,
        )
    }

    /// Returns the reusable GGUF tensor-name layout for the decoder family.
    #[must_use]
    pub fn tensor_layout(&self) -> &GgufDecoderTensorLayout {
        &self.tensor_layout
    }
}

/// GGUF-backed decoder-family adapter loader.
#[derive(Clone, Copy, Debug, Default)]
pub struct GgufDecoderAdapterLoader;

impl GgufDecoderAdapterLoader {
    fn load_blob_artifact_with_governance(
        &self,
        artifact: &GgufBlobArtifact,
        artifact_governance: ModelArtifactGovernance,
    ) -> Result<GgufDecoderAdapter, ModelLoadError> {
        let content = artifact.content();
        let metadata = content.metadata();
        let architecture = read_required_gguf_string(metadata, "general.architecture")?;
        let family = classify_gguf_decoder_family(metadata, architecture.as_str())?;
        validate_supported_decoder_family_features(metadata, family, architecture.as_str())?;

        let tokenizer = content.load_tokenizer()?;
        let chat_templates = content.load_chat_templates()?;
        let bundle = GgufWeightBundleLoader.describe_artifact(artifact)?;
        let family_metadata =
            build_gguf_decoder_family_metadata(metadata, content, &family, architecture)?;
        let descriptor = build_gguf_decoder_descriptor(
            artifact.artifact_metadata(),
            &bundle,
            &family_metadata,
            &tokenizer,
            &chat_templates,
            content,
        )?
        .with_artifact_governance(artifact_governance);
        let tensor_layout =
            build_gguf_decoder_tensor_layout(content, &family_metadata, &descriptor.config)?;

        Ok(GgufDecoderAdapter {
            descriptor,
            family_metadata,
            tokenizer,
            chat_templates,
            tensor_layout,
        })
    }

    /// Loads a decoder-family adapter from an already-open GGUF blob artifact.
    pub fn load_blob_artifact(
        &self,
        artifact: &GgufBlobArtifact,
    ) -> Result<GgufDecoderAdapter, ModelLoadError> {
        let artifact_governance = match artifact.blob_metadata().kind {
            LocalBlobKind::GgufFile => {
                ModelArtifactGovernance::local_path(&artifact.blob_metadata().path)
            }
            LocalBlobKind::OllamaBlob => {
                ModelArtifactGovernance::ollama_blob(artifact.blob_metadata())
            }
        };
        self.load_blob_artifact_with_governance(artifact, artifact_governance)
    }

    /// Loads a decoder-family adapter from a local GGUF path.
    pub fn load_path(&self, path: &Path) -> Result<GgufDecoderAdapter, ModelLoadError> {
        let artifact = GgufBlobArtifact::open_path(path, LocalBlobOpenOptions::default())?;
        self.load_blob_artifact(&artifact)
    }

    /// Loads a decoder-family adapter from an Ollama-managed GGUF blob.
    pub fn load_ollama_blob(
        &self,
        models_root: impl AsRef<Path>,
        digest: &str,
        options: LocalBlobOpenOptions,
    ) -> Result<GgufDecoderAdapter, ModelLoadError> {
        let artifact = GgufBlobArtifact::open_ollama_blob(models_root, digest, options)?;
        self.load_blob_artifact(&artifact)
    }

    /// Loads a decoder-family adapter from a resolved Ollama manifest.
    pub fn load_ollama_manifest(
        &self,
        manifest: &OllamaManifest,
        options: LocalBlobOpenOptions,
    ) -> Result<GgufDecoderAdapter, ModelLoadError> {
        let config = manifest
            .load_config(
                LocalBlobOpenOptions::default()
                    .with_read_preference(psionic_catalog::BlobReadPreference::PreferBuffered),
            )
            .map_err(|error| ModelLoadError::ArtifactRead {
                path: manifest.manifest_path.display().to_string(),
                message: error.to_string(),
            })?;
        let licenses = manifest
            .load_license_facts(
                LocalBlobOpenOptions::default()
                    .with_read_preference(psionic_catalog::BlobReadPreference::PreferBuffered),
            )
            .map_err(|error| ModelLoadError::ArtifactRead {
                path: manifest.manifest_path.display().to_string(),
                message: error.to_string(),
            })?;
        let artifact = GgufBlobArtifact::open_ollama_manifest(manifest, options)?;
        self.load_blob_artifact_with_governance(
            &artifact,
            ModelArtifactGovernance::from_ollama_manifest(manifest, config.as_ref(), licenses),
        )
    }
}

fn classify_gguf_embedding_family(
    architecture: &str,
) -> Result<GgufEmbeddingFamily, ModelLoadError> {
    match architecture {
        "bert" => Ok(GgufEmbeddingFamily::Bert),
        "nomic-bert" | "nomic-bert-moe" => Ok(GgufEmbeddingFamily::NomicBert),
        other => Err(ModelLoadError::UnsupportedGgufEmbeddingArchitecture {
            architecture: other.to_string(),
        }),
    }
}

fn validate_supported_embedding_family_features(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    family: GgufEmbeddingFamily,
    architecture: &str,
) -> Result<(), ModelLoadError> {
    let attention_causal_key = format!("{architecture}.attention.causal");
    if read_optional_gguf_bool(metadata, attention_causal_key.as_str())?.unwrap_or(false) {
        return Err(ModelLoadError::InvalidGgufMetadata {
            key: attention_causal_key,
            message: String::from("embedding families must be non-causal"),
        });
    }

    if matches!(family, GgufEmbeddingFamily::NomicBert)
        && (architecture.ends_with("-moe")
            || read_optional_gguf_usize(metadata, format!("{architecture}.expert_count").as_str())?
                .unwrap_or(0)
                > 0
            || read_optional_gguf_usize(
                metadata,
                format!("{architecture}.moe_every_n_layers").as_str(),
            )?
            .unwrap_or(0)
                > 0)
    {
        return Err(ModelLoadError::UnsupportedGgufEmbeddingFamilyFeature {
            family: family.as_str().to_string(),
            feature: String::from("mixture_of_experts"),
        });
    }

    Ok(())
}

fn build_gguf_embedding_family_metadata(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    content: &GgufContent,
    family: GgufEmbeddingFamily,
    architecture: String,
) -> Result<GgufEmbeddingFamilyMetadata, ModelLoadError> {
    let max_context =
        read_required_gguf_usize(metadata, format!("{architecture}.context_length").as_str())?;
    let layer_count =
        read_required_gguf_usize(metadata, format!("{architecture}.block_count").as_str())?;
    let attention_head_count = read_required_gguf_usize(
        metadata,
        format!("{architecture}.attention.head_count").as_str(),
    )?;
    let pooling_key = format!("{architecture}.pooling_type");
    let pooling = GgufEmbeddingPooling::from_gguf_value(
        read_optional_gguf_usize(metadata, pooling_key.as_str())?.unwrap_or(0),
        pooling_key.as_str(),
    )?;
    let normalization = if read_optional_gguf_bool(
        metadata,
        format!("{architecture}.normalize_embeddings").as_str(),
    )?
    .unwrap_or(matches!(family, GgufEmbeddingFamily::Bert))
    {
        EmbeddingNormalization::UnitLength
    } else {
        EmbeddingNormalization::None
    };

    Ok(GgufEmbeddingFamilyMetadata {
        family,
        architecture: architecture.clone(),
        display_name: read_optional_gguf_string(metadata, "general.name")?,
        max_context,
        layer_count,
        attention_head_count,
        attention_kv_head_count: read_optional_gguf_usize(
            metadata,
            format!("{architecture}.attention.head_count_kv").as_str(),
        )?
        .unwrap_or(attention_head_count),
        layer_norm_epsilon: read_required_gguf_f32(
            metadata,
            format!("{architecture}.attention.layer_norm_epsilon").as_str(),
        )?,
        rope_theta: read_optional_gguf_f32(
            metadata,
            format!("{architecture}.rope.freq_base").as_str(),
        )?,
        pooling,
        normalization,
        uses_position_embeddings: matches!(family, GgufEmbeddingFamily::Bert)
            || content.tensor_info("position_embd.weight").is_some(),
        uses_fused_qkv: matches!(family, GgufEmbeddingFamily::NomicBert),
    })
}

fn build_gguf_embedding_descriptor(
    artifact: &WeightArtifactMetadata,
    bundle: &WeightBundleMetadata,
    family_metadata: &GgufEmbeddingFamilyMetadata,
    tokenizer: &GgufTokenizerMetadata,
    content: &GgufContent,
) -> Result<EmbeddingModelDescriptor, ModelLoadError> {
    let metadata = content.metadata();
    let architecture = family_metadata.architecture.as_str();
    let hidden_size = read_required_gguf_usize(
        metadata,
        format!("{architecture}.embedding_length").as_str(),
    )?;
    let head_count = family_metadata.attention_head_count;
    if head_count == 0 || hidden_size % head_count != 0 {
        return Err(ModelLoadError::InvalidGgufMetadata {
            key: format!("{architecture}.attention.head_count"),
            message: format!(
                "embedding length {hidden_size} is not divisible by attention head count {head_count}"
            ),
        });
    }

    let token_embedding = required_tensor_info(content, "token_embd.weight")?;
    let (vocab_size, token_hidden_size) = tensor_matrix_shape(token_embedding)?;
    if token_hidden_size != hidden_size {
        return Err(artifact_format_error(
            "gguf",
            format!(
                "token_embd.weight hidden size {token_hidden_size} does not match metadata embedding length {hidden_size}"
            ),
        ));
    }

    let token_embedding_norm = required_tensor_info(content, "token_embd_norm.weight")?;
    if tensor_vector_shape(token_embedding_norm)? != hidden_size {
        return Err(artifact_format_error(
            "gguf",
            format!(
                "token_embd_norm.weight width {} does not match metadata embedding length {hidden_size}",
                tensor_vector_shape(token_embedding_norm)?
            ),
        ));
    }

    let token_type_embedding = required_tensor_info(content, "token_types.weight")?;
    let (token_type_rows, token_type_hidden_size) = tensor_matrix_shape(token_type_embedding)?;
    if token_type_hidden_size != hidden_size {
        return Err(artifact_format_error(
            "gguf",
            format!(
                "token_types.weight hidden size {token_type_hidden_size} does not match metadata embedding length {hidden_size}"
            ),
        ));
    }
    if let Some(token_type_count) = tokenizer.token_type_count {
        if token_type_rows != token_type_count {
            return Err(ModelLoadError::InvalidTokenizerMetadata {
                key: String::from("tokenizer.ggml.token_type_count"),
                message: format!(
                    "declared token type count {token_type_count} does not match token_types.weight rows {token_type_rows}"
                ),
            });
        }
    }

    if family_metadata.uses_position_embeddings {
        let position_embedding = required_tensor_info(content, "position_embd.weight")?;
        let (position_rows, position_hidden_size) = tensor_matrix_shape(position_embedding)?;
        if position_rows != family_metadata.max_context || position_hidden_size != hidden_size {
            return Err(artifact_format_error(
                "gguf",
                format!(
                    "position_embd.weight shape [{position_rows}, {position_hidden_size}] does not match expected [{}, {hidden_size}]",
                    family_metadata.max_context
                ),
            ));
        }
    }

    if tokenizer.vocabulary.len() != vocab_size {
        return Err(ModelLoadError::InvalidTokenizerMetadata {
            key: String::from("tokenizer.ggml.tokens"),
            message: format!(
                "tokenizer vocabulary length {} does not match token_embd.weight rows {vocab_size}",
                tokenizer.vocabulary.len()
            ),
        });
    }

    let model_id = build_gguf_model_id(
        artifact,
        family_metadata.display_name.as_deref(),
        family_metadata.family.as_str(),
    );
    let revision = artifact.integrity_label().into_owned();
    Ok(EmbeddingModelDescriptor::new(
        ModelDescriptor::new(model_id, family_metadata.family.as_str(), revision),
        hidden_size,
        family_metadata.normalization,
        bundle.clone(),
    )
    .with_artifact_identity(ServedModelArtifactMetadata::new(
        Some(artifact.integrity_label().into_owned()),
        Some(tokenizer.digest().to_string()),
        None,
        digest_generation_defaults(tokenizer.add_bos, tokenizer.add_eos, &[]),
    )))
}

fn build_gguf_embedding_tensor_layout(
    content: &GgufContent,
    family_metadata: &GgufEmbeddingFamilyMetadata,
    tokenizer: &GgufTokenizerMetadata,
) -> Result<GgufEmbeddingTensorLayout, ModelLoadError> {
    let hidden_size = tensor_matrix_shape(required_tensor_info(content, "token_embd.weight")?)?.1;
    let metadata = content.metadata();
    let architecture = family_metadata.architecture.as_str();
    let intermediate_size = read_required_gguf_usize(
        metadata,
        format!("{architecture}.feed_forward_length").as_str(),
    )?;
    let head_dim = hidden_size / family_metadata.attention_head_count;
    let kv_hidden_size = family_metadata.attention_kv_head_count * head_dim;

    let token_type_embedding = required_tensor_info(content, "token_types.weight")?;
    let (token_type_rows, token_type_hidden_size) = tensor_matrix_shape(token_type_embedding)?;
    if token_type_hidden_size != hidden_size {
        return Err(artifact_format_error(
            "gguf",
            format!(
                "token_types.weight hidden size {token_type_hidden_size} does not match token embedding width {hidden_size}"
            ),
        ));
    }
    if let Some(token_type_count) = tokenizer.token_type_count {
        if token_type_rows != token_type_count {
            return Err(ModelLoadError::InvalidTokenizerMetadata {
                key: String::from("tokenizer.ggml.token_type_count"),
                message: format!(
                    "declared token type count {token_type_count} does not match token_types.weight rows {token_type_rows}"
                ),
            });
        }
    }

    let position_embedding = if family_metadata.uses_position_embeddings {
        Some(
            required_tensor_info(content, "position_embd.weight")?
                .name
                .clone(),
        )
    } else {
        optional_tensor_name(content, "position_embd.weight")
    };

    let mut layers = Vec::with_capacity(family_metadata.layer_count);
    for layer_index in 0..family_metadata.layer_count {
        let prefix = format!("blk.{layer_index}");
        let attention_output_norm =
            required_tensor_info(content, &format!("{prefix}.attn_output_norm.weight"))?;
        if tensor_vector_shape(attention_output_norm)? != hidden_size {
            return Err(artifact_format_error(
                "gguf",
                format!(
                    "{} width {} does not match hidden size {hidden_size}",
                    attention_output_norm.name,
                    tensor_vector_shape(attention_output_norm)?
                ),
            ));
        }
        let layer_output_norm =
            required_tensor_info(content, &format!("{prefix}.layer_output_norm.weight"))?;
        if tensor_vector_shape(layer_output_norm)? != hidden_size {
            return Err(artifact_format_error(
                "gguf",
                format!(
                    "{} width {} does not match hidden size {hidden_size}",
                    layer_output_norm.name,
                    tensor_vector_shape(layer_output_norm)?
                ),
            ));
        }

        let (
            attention_query_weight,
            attention_query_bias,
            attention_query_norm,
            attention_key_weight,
            attention_key_bias,
            attention_key_norm,
            attention_value_weight,
            attention_value_bias,
            attention_qkv_weight,
            attention_qkv_bias,
        ) = if family_metadata.uses_fused_qkv {
            let qkv_weight = required_tensor_info(content, &format!("{prefix}.attn_qkv.weight"))?;
            let (rows, columns) = tensor_matrix_shape(qkv_weight)?;
            if rows != hidden_size * 3 || columns != hidden_size {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{rows}, {columns}] does not match expected [{}, {hidden_size}]",
                        qkv_weight.name,
                        hidden_size * 3
                    ),
                ));
            }
            (
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(qkv_weight.name.clone()),
                optional_tensor_name(content, &format!("{prefix}.attn_qkv.bias")),
            )
        } else {
            let query_weight = required_tensor_info(content, &format!("{prefix}.attn_q.weight"))?;
            let (query_rows, query_columns) = tensor_matrix_shape(query_weight)?;
            if query_rows != hidden_size || query_columns != hidden_size {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{query_rows}, {query_columns}] does not match expected [{hidden_size}, {hidden_size}]",
                        query_weight.name
                    ),
                ));
            }
            let key_weight = required_tensor_info(content, &format!("{prefix}.attn_k.weight"))?;
            let (key_rows, key_columns) = tensor_matrix_shape(key_weight)?;
            if key_rows != kv_hidden_size || key_columns != hidden_size {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{key_rows}, {key_columns}] does not match expected [{kv_hidden_size}, {hidden_size}]",
                        key_weight.name
                    ),
                ));
            }
            let value_weight = required_tensor_info(content, &format!("{prefix}.attn_v.weight"))?;
            let (value_rows, value_columns) = tensor_matrix_shape(value_weight)?;
            if value_rows != kv_hidden_size || value_columns != hidden_size {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{value_rows}, {value_columns}] does not match expected [{kv_hidden_size}, {hidden_size}]",
                        value_weight.name
                    ),
                ));
            }
            (
                Some(query_weight.name.clone()),
                optional_tensor_name(content, &format!("{prefix}.attn_q.bias")),
                optional_tensor_name(content, &format!("{prefix}.attn_q_norm.weight")),
                Some(key_weight.name.clone()),
                optional_tensor_name(content, &format!("{prefix}.attn_k.bias")),
                optional_tensor_name(content, &format!("{prefix}.attn_k_norm.weight")),
                Some(value_weight.name.clone()),
                optional_tensor_name(content, &format!("{prefix}.attn_v.bias")),
                None,
                None,
            )
        };

        let attention_output =
            required_tensor_info(content, &format!("{prefix}.attn_output.weight"))?;
        let (attention_output_rows, attention_output_columns) =
            tensor_matrix_shape(attention_output)?;
        if attention_output_rows != hidden_size || attention_output_columns != hidden_size {
            return Err(artifact_format_error(
                "gguf",
                format!(
                    "{} shape [{attention_output_rows}, {attention_output_columns}] does not match expected [{hidden_size}, {hidden_size}]",
                    attention_output.name
                ),
            ));
        }

        let feed_forward_up = required_tensor_info(content, &format!("{prefix}.ffn_up.weight"))?;
        let (feed_forward_up_rows, feed_forward_up_columns) = tensor_matrix_shape(feed_forward_up)?;
        if feed_forward_up_rows != intermediate_size || feed_forward_up_columns != hidden_size {
            return Err(artifact_format_error(
                "gguf",
                format!(
                    "{} shape [{feed_forward_up_rows}, {feed_forward_up_columns}] does not match expected [{intermediate_size}, {hidden_size}]",
                    feed_forward_up.name
                ),
            ));
        }
        let feed_forward_down =
            required_tensor_info(content, &format!("{prefix}.ffn_down.weight"))?;
        let (feed_forward_down_rows, feed_forward_down_columns) =
            tensor_matrix_shape(feed_forward_down)?;
        if feed_forward_down_rows != hidden_size || feed_forward_down_columns != intermediate_size {
            return Err(artifact_format_error(
                "gguf",
                format!(
                    "{} shape [{feed_forward_down_rows}, {feed_forward_down_columns}] does not match expected [{hidden_size}, {intermediate_size}]",
                    feed_forward_down.name
                ),
            ));
        }

        let feed_forward_gate_weight = match family_metadata.family {
            GgufEmbeddingFamily::Bert => {
                optional_tensor_name(content, &format!("{prefix}.ffn_gate.weight"))
            }
            GgufEmbeddingFamily::NomicBert => {
                let feed_forward_gate =
                    required_tensor_info(content, &format!("{prefix}.ffn_gate.weight"))?;
                let (feed_forward_gate_rows, feed_forward_gate_columns) =
                    tensor_matrix_shape(feed_forward_gate)?;
                if feed_forward_gate_rows != intermediate_size
                    || feed_forward_gate_columns != hidden_size
                {
                    return Err(artifact_format_error(
                        "gguf",
                        format!(
                            "{} shape [{feed_forward_gate_rows}, {feed_forward_gate_columns}] does not match expected [{intermediate_size}, {hidden_size}]",
                            feed_forward_gate.name
                        ),
                    ));
                }
                Some(feed_forward_gate.name.clone())
            }
        };

        layers.push(GgufEmbeddingLayerTensorLayout {
            layer_index,
            attention_output_norm: attention_output_norm.name.clone(),
            attention_query_weight,
            attention_query_bias,
            attention_query_norm,
            attention_key_weight,
            attention_key_bias,
            attention_key_norm,
            attention_value_weight,
            attention_value_bias,
            attention_qkv_weight,
            attention_qkv_bias,
            attention_output_weight: attention_output.name.clone(),
            attention_output_bias: optional_tensor_name(
                content,
                &format!("{prefix}.attn_output.bias"),
            ),
            feed_forward_gate_weight,
            feed_forward_gate_bias: optional_tensor_name(
                content,
                &format!("{prefix}.ffn_gate.bias"),
            ),
            feed_forward_up_weight: feed_forward_up.name.clone(),
            feed_forward_up_bias: optional_tensor_name(content, &format!("{prefix}.ffn_up.bias")),
            feed_forward_down_weight: feed_forward_down.name.clone(),
            feed_forward_down_bias: optional_tensor_name(
                content,
                &format!("{prefix}.ffn_down.bias"),
            ),
            feed_forward_router_weight: optional_tensor_name(
                content,
                &format!("{prefix}.ffn_gate_inp.weight"),
            ),
            feed_forward_router_bias: optional_tensor_name(
                content,
                &format!("{prefix}.ffn_gate_inp.bias"),
            ),
            feed_forward_up_experts_weight: optional_tensor_name(
                content,
                &format!("{prefix}.ffn_up_exps.weight"),
            ),
            feed_forward_down_experts_weight: optional_tensor_name(
                content,
                &format!("{prefix}.ffn_down_exps.weight"),
            ),
            layer_output_norm: layer_output_norm.name.clone(),
        });
    }

    Ok(GgufEmbeddingTensorLayout {
        token_embedding: String::from("token_embd.weight"),
        token_type_embedding: String::from("token_types.weight"),
        token_embedding_norm: String::from("token_embd_norm.weight"),
        position_embedding,
        layers,
    })
}

fn classify_gguf_decoder_family(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    architecture: &str,
) -> Result<GgufDecoderFamily, ModelLoadError> {
    match architecture {
        "llama" => {
            let sliding_window =
                read_optional_gguf_usize(metadata, "llama.attention.sliding_window")?;
            let name = read_optional_gguf_string(metadata, "general.name")?;
            if sliding_window.is_some()
                || name
                    .as_deref()
                    .is_some_and(|value| value.to_ascii_lowercase().contains("mistral"))
            {
                Ok(GgufDecoderFamily::Mistral)
            } else {
                Ok(GgufDecoderFamily::Llama)
            }
        }
        "mistral" | "mistral3" => Ok(GgufDecoderFamily::Mistral),
        "qwen2" => Ok(GgufDecoderFamily::Qwen),
        "gpt-oss" => Ok(GgufDecoderFamily::GptOss),
        other => Err(ModelLoadError::UnsupportedGgufArchitecture {
            architecture: other.to_string(),
        }),
    }
}

fn validate_supported_decoder_family_features(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    family: GgufDecoderFamily,
    architecture: &str,
) -> Result<(), ModelLoadError> {
    let expert_count_key = format!("{architecture}.expert_count");
    let expert_count = read_optional_gguf_usize(metadata, expert_count_key.as_str())?.unwrap_or(0);
    if expert_count > 0 && !matches!(family, GgufDecoderFamily::GptOss) {
        return Err(ModelLoadError::UnsupportedGgufDecoderFamilyFeature {
            family: family.as_str().to_string(),
            feature: String::from("mixture_of_experts"),
        });
    }
    Ok(())
}

fn build_gguf_decoder_family_metadata(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    content: &GgufContent,
    family: &GgufDecoderFamily,
    architecture: String,
) -> Result<GgufDecoderFamilyMetadata, ModelLoadError> {
    let rope_theta_key = format!("{architecture}.rope.freq_base");
    let rope_scaling_factor_key = format!("{architecture}.rope.scaling.factor");
    let rope_original_context_key = format!("{architecture}.rope.scaling.original_context_length");
    let rms_norm_key = format!("{architecture}.attention.layer_norm_rms_epsilon");
    let sliding_window_key = format!("{architecture}.attention.sliding_window");
    let attention_key_length_key = format!("{architecture}.attention.key_length");
    let attention_value_length_key = format!("{architecture}.attention.value_length");
    let tie_word_embeddings_key = format!("{architecture}.tie_word_embeddings");
    let expert_count_key = format!("{architecture}.expert_count");
    let expert_used_count_key = format!("{architecture}.expert_used_count");
    let expert_feed_forward_length_key = format!("{architecture}.expert_feed_forward_length");

    Ok(GgufDecoderFamilyMetadata {
        family: *family,
        architecture,
        display_name: read_optional_gguf_string(metadata, "general.name")?,
        rope_theta: read_optional_gguf_f32(metadata, rope_theta_key.as_str())?.unwrap_or(10_000.0),
        rope_scaling_factor: read_optional_gguf_f32(metadata, rope_scaling_factor_key.as_str())?,
        rope_original_context_length: read_optional_gguf_usize(
            metadata,
            rope_original_context_key.as_str(),
        )?,
        rms_norm_epsilon: read_required_gguf_f32(metadata, rms_norm_key.as_str())?,
        sliding_window: read_optional_gguf_usize(metadata, sliding_window_key.as_str())?,
        attention_key_length: read_optional_gguf_usize(
            metadata,
            attention_key_length_key.as_str(),
        )?,
        attention_value_length: read_optional_gguf_usize(
            metadata,
            attention_value_length_key.as_str(),
        )?,
        tie_word_embeddings: read_optional_gguf_bool(metadata, tie_word_embeddings_key.as_str())?
            .unwrap_or(false)
            || content.tensor_info("output.weight").is_none(),
        attention_qkv_biases: matches!(family, GgufDecoderFamily::Qwen | GgufDecoderFamily::GptOss),
        expert_count: read_optional_gguf_usize(metadata, expert_count_key.as_str())?,
        expert_used_count: read_optional_gguf_usize(metadata, expert_used_count_key.as_str())?,
        expert_feed_forward_length: read_optional_gguf_usize(
            metadata,
            expert_feed_forward_length_key.as_str(),
        )?,
    })
}

fn build_gguf_decoder_descriptor(
    artifact: &WeightArtifactMetadata,
    bundle: &WeightBundleMetadata,
    family_metadata: &GgufDecoderFamilyMetadata,
    tokenizer: &GgufTokenizerMetadata,
    chat_templates: &GgufChatTemplateMetadata,
    content: &GgufContent,
) -> Result<DecoderModelDescriptor, ModelLoadError> {
    let metadata = content.metadata();
    let architecture = family_metadata.architecture.as_str();
    let hidden_size = read_required_gguf_usize(
        metadata,
        format!("{architecture}.embedding_length").as_str(),
    )?;
    let layer_count =
        read_required_gguf_usize(metadata, format!("{architecture}.block_count").as_str())?;
    let max_context =
        read_required_gguf_usize(metadata, format!("{architecture}.context_length").as_str())?;
    let intermediate_size =
        family_metadata
            .expert_feed_forward_length
            .unwrap_or(read_required_gguf_usize(
                metadata,
                format!("{architecture}.feed_forward_length").as_str(),
            )?);
    let head_count = read_required_gguf_usize(
        metadata,
        format!("{architecture}.attention.head_count").as_str(),
    )?;
    let kv_head_count = read_optional_gguf_usize(
        metadata,
        format!("{architecture}.attention.head_count_kv").as_str(),
    )?
    .unwrap_or(head_count);
    let head_dim = if let Some(key_length) = family_metadata.attention_key_length {
        key_length
    } else if head_count == 0 || hidden_size % head_count != 0 {
        return Err(ModelLoadError::InvalidGgufMetadata {
            key: format!("{architecture}.attention.head_count"),
            message: format!(
                "hidden size {hidden_size} is not divisible by attention head count {head_count}"
            ),
        });
    } else {
        hidden_size / head_count
    };
    let value_length = family_metadata.attention_value_length.unwrap_or(head_dim);
    if value_length != head_dim {
        return Err(ModelLoadError::UnsupportedGgufDecoderFamilyFeature {
            family: family_metadata.family.as_str().to_string(),
            feature: String::from("distinct_value_head_width"),
        });
    }
    let rotary_dim = read_optional_gguf_usize(
        metadata,
        format!("{architecture}.rope.dimension_count").as_str(),
    )?
    .unwrap_or(head_dim);

    let token_embedding = required_tensor_info(content, "token_embd.weight")?;
    let (vocab_size, token_hidden_size) = tensor_matrix_shape(token_embedding)?;
    if token_hidden_size != hidden_size {
        return Err(artifact_format_error(
            "gguf",
            format!(
                "token_embd.weight hidden size {token_hidden_size} does not match metadata embedding length {hidden_size}"
            ),
        ));
    }
    let output_norm = required_tensor_info(content, "output_norm.weight")?;
    let output_norm_width = tensor_vector_shape(output_norm)?;
    if output_norm_width != hidden_size {
        return Err(artifact_format_error(
            "gguf",
            format!(
                "output_norm.weight width {output_norm_width} does not match metadata embedding length {hidden_size}"
            ),
        ));
    }
    if tokenizer.vocabulary.len() != vocab_size {
        return Err(ModelLoadError::InvalidTokenizerMetadata {
            key: String::from("tokenizer.ggml.tokens"),
            message: format!(
                "tokenizer vocabulary length {} does not match token_embd.weight rows {vocab_size}",
                tokenizer.vocabulary.len()
            ),
        });
    }

    let config = DecoderConfig {
        hidden_size,
        layer_count,
        vocab_size,
        max_context,
        block: DecoderBlockConfig {
            attention: DecoderAttentionConfig {
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
            },
            feed_forward: DecoderFeedForwardConfig {
                intermediate_size,
                activation: ActivationFunction::Silu,
            },
        },
    };
    let model_id = build_gguf_model_id(
        artifact,
        family_metadata.display_name.as_deref(),
        family_metadata.family.as_str(),
    );
    let revision = artifact.integrity_label().into_owned();

    let stop_sequences = default_chat_template_stop_sequences(chat_templates);
    Ok(DecoderModelDescriptor::new(
        ModelDescriptor::new(model_id, family_metadata.family.as_str(), revision),
        config,
        gguf_tokenizer_family_label(tokenizer),
        bundle.clone(),
    )
    .with_artifact_identity(ServedModelArtifactMetadata::new(
        Some(artifact.integrity_label().into_owned()),
        Some(tokenizer.digest().to_string()),
        (!chat_templates.is_empty()).then(|| chat_templates.digest().to_string()),
        digest_generation_defaults(tokenizer.add_bos, tokenizer.add_eos, &stop_sequences),
    )))
}

fn build_gguf_decoder_tensor_layout(
    content: &GgufContent,
    family_metadata: &GgufDecoderFamilyMetadata,
    config: &DecoderConfig,
) -> Result<GgufDecoderTensorLayout, ModelLoadError> {
    required_tensor_info(content, "token_embd.weight")?;
    required_tensor_info(content, "output_norm.weight")?;
    let output = match content.tensor_info("output.weight") {
        Some(_) => Some(String::from("output.weight")),
        None if family_metadata.tie_word_embeddings => None,
        None => return Err(ModelLoadError::MissingTensor(String::from("output.weight"))),
    };

    let mut layers = Vec::with_capacity(config.layer_count);
    for layer_index in 0..config.layer_count {
        let prefix = format!("blk.{layer_index}");
        let query_bias = if family_metadata.attention_qkv_biases {
            Some(
                required_tensor_info(content, &format!("{prefix}.attn_q.bias"))?
                    .name
                    .clone(),
            )
        } else {
            None
        };
        let key_bias = if family_metadata.attention_qkv_biases {
            Some(
                required_tensor_info(content, &format!("{prefix}.attn_k.bias"))?
                    .name
                    .clone(),
            )
        } else {
            None
        };
        let value_bias = if family_metadata.attention_qkv_biases {
            Some(
                required_tensor_info(content, &format!("{prefix}.attn_v.bias"))?
                    .name
                    .clone(),
            )
        } else {
            None
        };
        if matches!(family_metadata.family, GgufDecoderFamily::GptOss) {
            let query_width = config
                .block
                .attention
                .head_count
                .saturating_mul(config.block.attention.head_dim);
            let kv_width = config.kv_width();
            let expert_count = family_metadata.expert_count.ok_or_else(|| {
                ModelLoadError::MissingGgufMetadata {
                    key: format!("{}.expert_count", family_metadata.architecture),
                }
            })?;
            let expert_width = family_metadata
                .expert_feed_forward_length
                .unwrap_or(config.block.feed_forward.intermediate_size);
            let query_weight = required_tensor_info(content, &format!("{prefix}.attn_q.weight"))?;
            let (query_rows, query_columns) = tensor_matrix_shape(query_weight)?;
            if query_rows != query_width || query_columns != config.hidden_size {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{query_rows}, {query_columns}] does not match expected [{query_width}, {}]",
                        query_weight.name, config.hidden_size
                    ),
                ));
            }
            let key_weight = required_tensor_info(content, &format!("{prefix}.attn_k.weight"))?;
            let (key_rows, key_columns) = tensor_matrix_shape(key_weight)?;
            if key_rows != kv_width || key_columns != config.hidden_size {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{key_rows}, {key_columns}] does not match expected [{kv_width}, {}]",
                        key_weight.name, config.hidden_size
                    ),
                ));
            }
            let value_weight = required_tensor_info(content, &format!("{prefix}.attn_v.weight"))?;
            let (value_rows, value_columns) = tensor_matrix_shape(value_weight)?;
            if value_rows != kv_width || value_columns != config.hidden_size {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{value_rows}, {value_columns}] does not match expected [{kv_width}, {}]",
                        value_weight.name, config.hidden_size
                    ),
                ));
            }
            let output_weight =
                required_tensor_info(content, &format!("{prefix}.attn_output.weight"))?;
            let (output_rows, output_columns) = tensor_matrix_shape(output_weight)?;
            if output_rows != config.hidden_size || output_columns != query_width {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{output_rows}, {output_columns}] does not match expected [{}, {query_width}]",
                        output_weight.name, config.hidden_size
                    ),
                ));
            }
            let router_weight =
                required_tensor_info(content, &format!("{prefix}.ffn_gate_inp.weight"))?;
            let (router_rows, router_columns) = tensor_matrix_shape(router_weight)?;
            if router_rows != expert_count || router_columns != config.hidden_size {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{router_rows}, {router_columns}] does not match expected [{expert_count}, {}]",
                        router_weight.name, config.hidden_size
                    ),
                ));
            }
            let gate_experts =
                required_tensor_info(content, &format!("{prefix}.ffn_gate_exps.weight"))?;
            let (gate_expert_count, gate_rows, gate_columns) = tensor_rank3_shape(gate_experts)?;
            if gate_expert_count != expert_count
                || gate_rows != expert_width
                || gate_columns != config.hidden_size
            {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{gate_expert_count}, {gate_rows}, {gate_columns}] does not match expected [{expert_count}, {expert_width}, {}]",
                        gate_experts.name, config.hidden_size
                    ),
                ));
            }
            let up_experts =
                required_tensor_info(content, &format!("{prefix}.ffn_up_exps.weight"))?;
            let (up_expert_count, up_rows, up_columns) = tensor_rank3_shape(up_experts)?;
            if up_expert_count != expert_count
                || up_rows != expert_width
                || up_columns != config.hidden_size
            {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{up_expert_count}, {up_rows}, {up_columns}] does not match expected [{expert_count}, {expert_width}, {}]",
                        up_experts.name, config.hidden_size
                    ),
                ));
            }
            let down_experts =
                required_tensor_info(content, &format!("{prefix}.ffn_down_exps.weight"))?;
            let (down_expert_count, down_rows, down_columns) = tensor_rank3_shape(down_experts)?;
            if down_expert_count != expert_count
                || down_rows != config.hidden_size
                || down_columns != expert_width
            {
                return Err(artifact_format_error(
                    "gguf",
                    format!(
                        "{} shape [{down_expert_count}, {down_rows}, {down_columns}] does not match expected [{expert_count}, {}, {expert_width}]",
                        down_experts.name, config.hidden_size
                    ),
                ));
            }

            layers.push(GgufDecoderLayerTensorLayout {
                layer_index,
                attention_norm: required_tensor_info(
                    content,
                    &format!("{prefix}.attn_norm.weight"),
                )?
                .name
                .clone(),
                attention_query_weight: query_weight.name.clone(),
                attention_query_bias: query_bias,
                attention_key_weight: key_weight.name.clone(),
                attention_key_bias: key_bias,
                attention_value_weight: value_weight.name.clone(),
                attention_value_bias: value_bias,
                attention_output_weight: output_weight.name.clone(),
                attention_output_bias: optional_tensor_name(
                    content,
                    &format!("{prefix}.attn_output.bias"),
                ),
                attention_post_norm: optional_tensor_name(
                    content,
                    &format!("{prefix}.post_attention_norm.weight"),
                ),
                attention_sinks_weight: optional_tensor_name(
                    content,
                    &format!("{prefix}.attn_sinks.weight"),
                ),
                feed_forward_gate_weight: None,
                feed_forward_down_weight: None,
                feed_forward_up_weight: None,
                feed_forward_norm: optional_tensor_name(
                    content,
                    &format!("{prefix}.post_attention_norm.weight"),
                ),
                feed_forward_router_weight: Some(router_weight.name.clone()),
                feed_forward_router_bias: optional_tensor_name(
                    content,
                    &format!("{prefix}.ffn_gate_inp.bias"),
                ),
                feed_forward_gate_experts_weight: Some(gate_experts.name.clone()),
                feed_forward_gate_experts_bias: optional_tensor_name(
                    content,
                    &format!("{prefix}.ffn_gate_exps.bias"),
                ),
                feed_forward_up_experts_weight: Some(up_experts.name.clone()),
                feed_forward_up_experts_bias: optional_tensor_name(
                    content,
                    &format!("{prefix}.ffn_up_exps.bias"),
                ),
                feed_forward_down_experts_weight: Some(down_experts.name.clone()),
                feed_forward_down_experts_bias: optional_tensor_name(
                    content,
                    &format!("{prefix}.ffn_down_exps.bias"),
                ),
            });
            continue;
        }

        layers.push(GgufDecoderLayerTensorLayout {
            layer_index,
            attention_norm: required_tensor_info(content, &format!("{prefix}.attn_norm.weight"))?
                .name
                .clone(),
            attention_query_weight: required_tensor_info(
                content,
                &format!("{prefix}.attn_q.weight"),
            )?
            .name
            .clone(),
            attention_query_bias: query_bias,
            attention_key_weight: required_tensor_info(
                content,
                &format!("{prefix}.attn_k.weight"),
            )?
            .name
            .clone(),
            attention_key_bias: key_bias,
            attention_value_weight: required_tensor_info(
                content,
                &format!("{prefix}.attn_v.weight"),
            )?
            .name
            .clone(),
            attention_value_bias: value_bias,
            attention_output_weight: required_tensor_info(
                content,
                &format!("{prefix}.attn_output.weight"),
            )?
            .name
            .clone(),
            attention_output_bias: None,
            attention_post_norm: None,
            attention_sinks_weight: None,
            feed_forward_gate_weight: Some(
                required_tensor_info(content, &format!("{prefix}.ffn_gate.weight"))?
                    .name
                    .clone(),
            ),
            feed_forward_down_weight: Some(
                required_tensor_info(content, &format!("{prefix}.ffn_down.weight"))?
                    .name
                    .clone(),
            ),
            feed_forward_up_weight: Some(
                required_tensor_info(content, &format!("{prefix}.ffn_up.weight"))?
                    .name
                    .clone(),
            ),
            feed_forward_norm: Some(
                required_tensor_info(content, &format!("{prefix}.ffn_norm.weight"))?
                    .name
                    .clone(),
            ),
            feed_forward_router_weight: None,
            feed_forward_router_bias: None,
            feed_forward_gate_experts_weight: None,
            feed_forward_gate_experts_bias: None,
            feed_forward_up_experts_weight: None,
            feed_forward_up_experts_bias: None,
            feed_forward_down_experts_weight: None,
            feed_forward_down_experts_bias: None,
        });
    }

    Ok(GgufDecoderTensorLayout {
        token_embedding: String::from("token_embd.weight"),
        output_norm: String::from("output_norm.weight"),
        output,
        layers,
    })
}

fn gguf_tokenizer_family_label(tokenizer: &GgufTokenizerMetadata) -> String {
    match tokenizer.model {
        GgufTokenizerModel::SentencePiece => String::from("sentencepiece"),
        GgufTokenizerModel::Gpt2Bpe => tokenizer.pretokenizer.as_ref().map_or_else(
            || String::from("gpt2_bpe"),
            |pretokenizer| format!("gpt2_bpe:{}", pretokenizer.digest_label()),
        ),
        GgufTokenizerModel::BertWordPiece => String::from("bert_wordpiece"),
    }
}

fn required_tensor_info<'a>(
    content: &'a GgufContent,
    name: &str,
) -> Result<&'a GgufTensorInfo, ModelLoadError> {
    content
        .tensor_info(name)
        .ok_or_else(|| ModelLoadError::MissingTensor(name.to_string()))
}

fn optional_tensor_name(content: &GgufContent, name: &str) -> Option<String> {
    content.tensor_info(name).map(|tensor| tensor.name.clone())
}

fn tensor_matrix_shape(tensor: &GgufTensorInfo) -> Result<(usize, usize), ModelLoadError> {
    match tensor.shape.dims() {
        [rows, columns] => Ok((*rows, *columns)),
        actual => Err(artifact_format_error(
            "gguf",
            format!(
                "tensor `{}` expected rank-2 shape, got {actual:?}",
                tensor.name
            ),
        )),
    }
}

fn tensor_rank3_shape(tensor: &GgufTensorInfo) -> Result<(usize, usize, usize), ModelLoadError> {
    match tensor.shape.dims() {
        [outer, rows, columns] => Ok((*outer, *rows, *columns)),
        actual => Err(artifact_format_error(
            "gguf",
            format!(
                "tensor `{}` expected rank-3 shape, got {actual:?}",
                tensor.name
            ),
        )),
    }
}

fn tensor_vector_shape(tensor: &GgufTensorInfo) -> Result<usize, ModelLoadError> {
    match tensor.shape.dims() {
        [width] => Ok(*width),
        actual => Err(artifact_format_error(
            "gguf",
            format!(
                "tensor `{}` expected rank-1 shape, got {actual:?}",
                tensor.name
            ),
        )),
    }
}

fn build_gguf_model_id(
    artifact: &WeightArtifactMetadata,
    display_name: Option<&str>,
    family_label: &str,
) -> String {
    let base = display_name
        .filter(|value| !value.trim().is_empty())
        .map(str::trim)
        .or_else(|| artifact.name.strip_suffix(".gguf"))
        .unwrap_or(family_label);
    let normalized = normalize_model_id_component(base);
    format!("{normalized}@{}", artifact.integrity_label())
}

fn normalize_model_id_component(value: &str) -> String {
    let mut normalized = String::new();
    let mut pending_dash = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_dash && !normalized.is_empty() {
                normalized.push('-');
            }
            normalized.push(character.to_ascii_lowercase());
            pending_dash = false;
        } else {
            pending_dash = true;
        }
    }
    if normalized.is_empty() {
        String::from("gguf-model")
    } else {
        normalized
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
    /// A required GGUF metadata key was missing.
    #[error("missing gguf metadata `{key}`")]
    MissingGgufMetadata {
        /// Missing metadata key.
        key: String,
    },
    /// A GGUF metadata key had the wrong type or an invalid value.
    #[error("invalid gguf metadata `{key}`: {message}")]
    InvalidGgufMetadata {
        /// Metadata key that failed validation.
        key: String,
        /// Validation failure summary.
        message: String,
    },
    /// The GGUF architecture is not one of the supported first-launch decoder families.
    #[error("unsupported gguf decoder architecture `{architecture}`")]
    UnsupportedGgufArchitecture {
        /// Raw architecture string from `general.architecture`.
        architecture: String,
    },
    /// The GGUF architecture is not one of the supported first-launch embedding families.
    #[error("unsupported gguf embedding architecture `{architecture}`")]
    UnsupportedGgufEmbeddingArchitecture {
        /// Raw architecture string from `general.architecture`.
        architecture: String,
    },
    /// The GGUF decoder artifact uses a family feature that the first-launch adapters do not support yet.
    #[error("unsupported gguf decoder family feature for `{family}`: {feature}")]
    UnsupportedGgufDecoderFamilyFeature {
        /// Launch-family label.
        family: String,
        /// Unsupported feature summary.
        feature: String,
    },
    /// The GGUF embedding artifact uses a family feature that the first-launch adapters do not support yet.
    #[error("unsupported gguf embedding family feature for `{family}`: {feature}")]
    UnsupportedGgufEmbeddingFamilyFeature {
        /// Launch-family label.
        family: String,
        /// Unsupported feature summary.
        feature: String,
    },
    /// The Ollama manifest carries adapter layers that Psionic does not support at the replacement boundary yet.
    #[error(
        "unsupported ollama adapter policy for `{model}`: policy=`{policy}` adapter_layers={adapter_layers}"
    )]
    UnsupportedOllamaAdapterPolicy {
        /// Canonical Ollama model name.
        model: String,
        /// Number of adapter layers carried by the manifest.
        adapter_layers: usize,
        /// Current Psionic adapter policy.
        policy: OllamaAdapterPolicy,
    },
    /// Opening or paging a local blob failed.
    #[error(transparent)]
    Blob(#[from] BlobError),
    /// GGUF tokenizer metadata is missing a required key.
    #[error("missing tokenizer metadata `{key}` in gguf artifact")]
    MissingTokenizerMetadata {
        /// Required GGUF metadata key.
        key: String,
    },
    /// GGUF tokenizer metadata had an invalid value or shape.
    #[error("invalid tokenizer metadata `{key}`: {message}")]
    InvalidTokenizerMetadata {
        /// GGUF metadata key that failed validation.
        key: String,
        /// Validation failure summary.
        message: String,
    },
    /// The GGUF tokenizer model family is not supported yet.
    #[error("unsupported gguf tokenizer model `{model}`")]
    UnsupportedTokenizerModel {
        /// Raw tokenizer model string from GGUF metadata.
        model: String,
    },
    /// A tensor dtype in the artifact bundle is not supported yet.
    #[error("unsupported tensor dtype `{dtype}` for `{name}`")]
    UnsupportedTensorDType {
        /// Tensor name.
        name: String,
        /// Dtype label.
        dtype: String,
    },
    /// A GGUF tensor type is known but not yet supported by the Psionic loader.
    #[error("unsupported gguf tensor type `{tensor_type}` for `{name}`")]
    UnsupportedGgufTensorType {
        /// Tensor name.
        name: String,
        /// GGUF tensor type label.
        tensor_type: GgufTensorType,
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
    /// A GGML/GGUF quantized tensor was requested with a shape that is invalid
    /// for row-wise block quantization.
    #[error(
        "quantized tensor mode `{quantization:?}` requires a non-scalar shape whose last dimension is block-aligned, got {shape:?}"
    )]
    InvalidQuantizedTensorShape {
        /// Quantization family that requires block alignment.
        quantization: QuantizationMode,
        /// Logical tensor shape requested.
        shape: Vec<usize>,
    },
    /// The serialized byte length for a quantized tensor does not match its block layout.
    #[error(
        "quantized tensor mode `{quantization:?}` expected {expected} bytes from its block layout, got {actual}"
    )]
    InvalidQuantizedTensorByteLength {
        /// Quantization family that was being decoded.
        quantization: QuantizationMode,
        /// Expected serialized byte length.
        expected: usize,
        /// Actual serialized byte length.
        actual: usize,
    },
    /// The quantized tensor mode is not backed by a GGML/GGUF block decoder.
    #[error("quantized tensor mode `{quantization:?}` does not use GGML/GGUF block storage")]
    UnsupportedQuantizedTensorMode {
        /// Quantization family requested for GGML/GGUF block decode.
        quantization: QuantizationMode,
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
        )
        .with_artifact_identity(ServedModelArtifactMetadata::new(
            weights
                .metadata()
                .primary_artifact_digest()
                .map(str::to_string),
            Some(digest_fixture_tokenizer(&tokenizer)),
            None,
            digest_generation_defaults(false, false, &[]),
        ));
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
        )
        .with_artifact_identity(ServedModelArtifactMetadata::new(
            bundle
                .metadata()
                .primary_artifact_digest()
                .map(str::to_string),
            Some(digest_fixture_tokenizer(&tokenizer)),
            None,
            digest_generation_defaults(false, false, &[]),
        ))
        .with_artifact_governance(ModelArtifactGovernance::local_path(path.as_ref()));
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
        )
        .with_artifact_identity(ServedModelArtifactMetadata::new(
            weights
                .metadata()
                .primary_artifact_digest()
                .map(str::to_string),
            None,
            None,
            digest_generation_defaults(false, false, &[]),
        ));

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
        )
        .with_artifact_identity(ServedModelArtifactMetadata::new(
            weights
                .metadata()
                .primary_artifact_digest()
                .map(str::to_string),
            None,
            None,
            digest_generation_defaults(false, false, &[]),
        ))
        .with_artifact_governance(ModelArtifactGovernance::local_path(path.as_ref()));
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
    digest_tensor_values(&mut hasher, &tensors[0], bias);
    digest_tensor_values(&mut hasher, &tensors[1], projection);
    WeightBundleMetadata {
        format: WeightFormat::ProgrammaticFixture,
        source: WeightSource::Fixture,
        quantization: QuantizationMode::None,
        quantization_modes: Vec::new(),
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
        digest_tensor_values(&mut hasher, metadata, values);
    }

    WeightBundleMetadata {
        format: WeightFormat::ProgrammaticFixture,
        source: WeightSource::Fixture,
        quantization: QuantizationMode::None,
        quantization_modes: Vec::new(),
        digest: hex::encode(hasher.finalize()),
        tensors,
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

fn digest_quantized_tensor(
    hasher: &mut Sha256,
    metadata: &WeightTensorMetadata,
    storage: &QuantizedTensorStorage,
) {
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
    let layout = storage.layout();
    hasher.update((layout.elements_per_block as u64).to_be_bytes());
    hasher.update((layout.bytes_per_block as u64).to_be_bytes());
    hasher.update((layout.block_count as u64).to_be_bytes());
    hasher.update(b"|");
    hasher.update(storage.bytes());
    hasher.update(b"\n");
}

fn digest_tensor_metadata(hasher: &mut Sha256, metadata: &WeightTensorMetadata) {
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
    if let Some(layout) = metadata.quantized_layout {
        hasher.update((layout.elements_per_block as u64).to_be_bytes());
        hasher.update((layout.bytes_per_block as u64).to_be_bytes());
        hasher.update((layout.block_count as u64).to_be_bytes());
    }
    hasher.update(b"\n");
}

fn digest_loaded_tensor(
    hasher: &mut Sha256,
    tensor: &LoadedWeightTensor,
) -> Result<(), ModelLoadError> {
    match tensor.storage() {
        WeightTensorStorage::DequantizedF32(values) => {
            digest_tensor_values(hasher, tensor.metadata(), values);
        }
        WeightTensorStorage::QuantizedBlocks(storage) => {
            digest_quantized_tensor(hasher, tensor.metadata(), storage);
        }
    }
    Ok(())
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
    Ok(tensor.values()?.into_owned())
}

fn artifact_format_error(format: &str, message: impl Into<String>) -> ModelLoadError {
    ModelLoadError::ArtifactFormat {
        format: String::from(format),
        message: message.into(),
    }
}

fn track_quantized_bytes(
    counts: &mut Vec<(QuantizationMode, usize)>,
    quantization: QuantizationMode,
    bytes: usize,
) {
    if let Some((_, total_bytes)) = counts
        .iter_mut()
        .find(|(candidate, _)| *candidate == quantization)
    {
        *total_bytes += bytes;
    } else {
        counts.push((quantization, bytes));
    }
}

fn dominant_quantization_mode(counts: &[(QuantizationMode, usize)]) -> QuantizationMode {
    counts
        .iter()
        .max_by_key(|(quantization, bytes)| (*bytes, quantization_priority(*quantization)))
        .map_or(QuantizationMode::None, |(quantization, _)| *quantization)
}

fn quantization_priority(quantization: QuantizationMode) -> u8 {
    match quantization {
        QuantizationMode::None => 0,
        QuantizationMode::Int8Symmetric => 1,
        QuantizationMode::GgmlQ4_0 => 2,
        QuantizationMode::GgmlQ4_1 => 3,
        QuantizationMode::GgmlQ8_0 => 4,
        QuantizationMode::GgmlMxfp4 => 5,
    }
}

fn quantization_modes_from_counts(counts: &[(QuantizationMode, usize)]) -> Vec<QuantizationMode> {
    let mut modes = counts.iter().map(|(mode, _)| *mode).collect::<Vec<_>>();
    modes.sort_by_key(|mode| quantization_priority(*mode));
    modes.dedup();
    modes
}

fn read_tokenizer_string_array(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Vec<String>, ModelLoadError> {
    let Some(value) = metadata.get(key) else {
        return Err(ModelLoadError::MissingTokenizerMetadata {
            key: key.to_string(),
        });
    };
    read_tokenizer_string_values(key, value)
}

fn read_optional_tokenizer_string_array(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Vec<String>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(Vec::new()),
        |value| read_tokenizer_string_values(key, value),
    )
}

fn read_optional_gguf_string(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Option<String>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(None),
        |value| {
            value
                .as_str()
                .map(|value| Some(value.to_string()))
                .ok_or_else(|| {
                    artifact_format_error("gguf", format!("metadata key `{key}` must be a string"))
                })
        },
    )
}

fn read_required_gguf_string(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<String, ModelLoadError> {
    read_optional_gguf_string(metadata, key)?.ok_or_else(|| ModelLoadError::MissingGgufMetadata {
        key: key.to_string(),
    })
}

fn read_optional_gguf_bool(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Option<bool>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(None),
        |value| {
            value
                .as_bool()
                .ok_or_else(|| ModelLoadError::InvalidGgufMetadata {
                    key: key.to_string(),
                    message: String::from("expected a boolean value"),
                })
                .map(Some)
        },
    )
}

fn read_optional_gguf_usize(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Option<usize>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(None),
        |value| {
            let Some(raw) = value.as_u64() else {
                return Err(ModelLoadError::InvalidGgufMetadata {
                    key: key.to_string(),
                    message: String::from("expected a non-negative integer value"),
                });
            };
            usize::try_from(raw)
                .map(Some)
                .map_err(|_| ModelLoadError::InvalidGgufMetadata {
                    key: key.to_string(),
                    message: format!("value `{raw}` does not fit usize"),
                })
        },
    )
}

fn read_required_gguf_usize(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<usize, ModelLoadError> {
    read_optional_gguf_usize(metadata, key)?.ok_or_else(|| ModelLoadError::MissingGgufMetadata {
        key: key.to_string(),
    })
}

fn read_optional_gguf_f32(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Option<f32>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(None),
        |value| {
            value
                .as_f32()
                .ok_or_else(|| ModelLoadError::InvalidGgufMetadata {
                    key: key.to_string(),
                    message: String::from("expected a numeric value"),
                })
                .map(Some)
        },
    )
}

fn read_required_gguf_f32(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<f32, ModelLoadError> {
    read_optional_gguf_f32(metadata, key)?.ok_or_else(|| ModelLoadError::MissingGgufMetadata {
        key: key.to_string(),
    })
}

fn read_optional_gguf_string_array(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Vec<String>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(Vec::new()),
        |value| {
            let Some(values) = value.as_array() else {
                return Err(artifact_format_error(
                    "gguf",
                    format!("metadata key `{key}` must be an array of strings"),
                ));
            };
            values
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    entry.as_str().map(str::to_string).ok_or_else(|| {
                        artifact_format_error(
                            "gguf",
                            format!(
                                "metadata key `{key}` expected string element at index {index}"
                            ),
                        )
                    })
                })
                .collect()
        },
    )
}

fn read_tokenizer_string_values(
    key: &str,
    value: &GgufMetadataValue,
) -> Result<Vec<String>, ModelLoadError> {
    let Some(values) = value.as_array() else {
        return Err(ModelLoadError::InvalidTokenizerMetadata {
            key: key.to_string(),
            message: String::from("expected an array of strings"),
        });
    };
    values
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            entry.as_str().map(String::from).ok_or_else(|| {
                ModelLoadError::InvalidTokenizerMetadata {
                    key: key.to_string(),
                    message: format!("expected string element at index {index}"),
                }
            })
        })
        .collect()
}

fn read_optional_tokenizer_f32_array(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Vec<f32>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(Vec::new()),
        |value| {
            let Some(values) = value.as_array() else {
                return Err(ModelLoadError::InvalidTokenizerMetadata {
                    key: key.to_string(),
                    message: String::from("expected an array of numeric scores"),
                });
            };
            values
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    entry
                        .as_f32()
                        .ok_or_else(|| ModelLoadError::InvalidTokenizerMetadata {
                            key: key.to_string(),
                            message: format!("expected numeric score at index {index}"),
                        })
                })
                .collect()
        },
    )
}

fn read_optional_tokenizer_i32_array(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Vec<i32>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(Vec::new()),
        |value| {
            let Some(values) = value.as_array() else {
                return Err(ModelLoadError::InvalidTokenizerMetadata {
                    key: key.to_string(),
                    message: String::from("expected an array of integer token types"),
                });
            };
            values
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let Some(raw) = entry.as_i64() else {
                        return Err(ModelLoadError::InvalidTokenizerMetadata {
                            key: key.to_string(),
                            message: format!("expected integer token type at index {index}"),
                        });
                    };
                    i32::try_from(raw).map_err(|_| ModelLoadError::InvalidTokenizerMetadata {
                        key: key.to_string(),
                        message: format!("token type `{raw}` at index {index} does not fit i32"),
                    })
                })
                .collect()
        },
    )
}

fn read_optional_tokenizer_id(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Option<TokenId>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(None),
        |value| {
            let Some(raw) = value.as_u64() else {
                return Err(ModelLoadError::InvalidTokenizerMetadata {
                    key: key.to_string(),
                    message: String::from("expected a non-negative integer token id"),
                });
            };
            let id = u32::try_from(raw).map_err(|_| ModelLoadError::InvalidTokenizerMetadata {
                key: key.to_string(),
                message: format!("token id `{raw}` does not fit u32"),
            })?;
            Ok(Some(TokenId(id)))
        },
    )
}

fn read_optional_tokenizer_id_array(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Vec<TokenId>, ModelLoadError> {
    metadata.get(key).map_or_else(
        || Ok(Vec::new()),
        |value| {
            let Some(values) = value.as_array() else {
                return Err(ModelLoadError::InvalidTokenizerMetadata {
                    key: key.to_string(),
                    message: String::from("expected an array of token ids"),
                });
            };
            values
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let Some(raw) = entry.as_u64() else {
                        return Err(ModelLoadError::InvalidTokenizerMetadata {
                            key: key.to_string(),
                            message: format!(
                                "expected non-negative integer token id at index {index}"
                            ),
                        });
                    };
                    let id = u32::try_from(raw).map_err(|_| {
                        ModelLoadError::InvalidTokenizerMetadata {
                            key: key.to_string(),
                            message: format!("token id `{raw}` at index {index} does not fit u32"),
                        }
                    })?;
                    Ok(TokenId(id))
                })
                .collect()
        },
    )
}

fn validate_tokenizer_id(
    key: &str,
    token_id: Option<TokenId>,
    vocabulary_len: usize,
) -> Result<(), ModelLoadError> {
    if let Some(token_id) = token_id {
        if token_id.as_u32() as usize >= vocabulary_len {
            return Err(ModelLoadError::InvalidTokenizerMetadata {
                key: key.to_string(),
                message: format!(
                    "token id {} is out of range for vocabulary size {vocabulary_len}",
                    token_id.as_u32()
                ),
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn digest_gguf_tokenizer(
    model: GgufTokenizerModel,
    tokens: &[String],
    scores: &[f32],
    token_types: &[i32],
    merges: &[String],
    bos_token_id: Option<TokenId>,
    eos_token_ids: &[TokenId],
    pad_token_id: Option<TokenId>,
    unknown_token_id: Option<TokenId>,
    add_bos: bool,
    add_eos: bool,
    pretokenizer: Option<&GgufTokenizerPretokenizer>,
    token_type_count: Option<usize>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(model.as_str().as_bytes());
    hasher.update(b"\n");
    update_optional_token_id(&mut hasher, bos_token_id);
    update_optional_token_id(&mut hasher, pad_token_id);
    update_optional_token_id(&mut hasher, unknown_token_id);
    hasher.update([u8::from(add_bos), u8::from(add_eos)]);
    hasher.update(b"\n");
    match pretokenizer {
        Some(pretokenizer) => hasher.update(pretokenizer.digest_label().as_bytes()),
        None => hasher.update(b"none"),
    }
    hasher.update(b"\n");
    match token_type_count {
        Some(token_type_count) => hasher.update(token_type_count.to_be_bytes()),
        None => hasher.update(b"none"),
    }
    hasher.update(b"\n");
    for token in tokens {
        update_digest_string(&mut hasher, token);
    }
    hasher.update(b"\n");
    for score in scores {
        hasher.update(score.to_bits().to_be_bytes());
    }
    hasher.update(b"\n");
    for token_type in token_types {
        hasher.update(token_type.to_be_bytes());
    }
    hasher.update(b"\n");
    for merge in merges {
        update_digest_string(&mut hasher, merge);
    }
    hasher.update(b"\n");
    for token_id in eos_token_ids {
        hasher.update(token_id.as_u32().to_be_bytes());
    }
    hex::encode(hasher.finalize())
}

fn digest_gguf_chat_templates(
    default_template: Option<&str>,
    named_templates: &BTreeMap<String, String>,
) -> String {
    let mut hasher = Sha256::new();
    match default_template {
        Some(template) => {
            hasher.update([1]);
            update_digest_string(&mut hasher, template);
        }
        None => hasher.update([0]),
    }
    hasher.update(b"\n");
    for (name, template) in named_templates {
        update_digest_string(&mut hasher, name);
        update_digest_string(&mut hasher, template);
    }
    hex::encode(hasher.finalize())
}

/// Computes a stable digest over default generation behavior.
#[must_use]
pub fn digest_generation_defaults(
    add_bos: bool,
    add_eos: bool,
    stop_sequences: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update([u8::from(add_bos), u8::from(add_eos)]);
    hasher.update(b"\n");
    for stop_sequence in stop_sequences {
        update_digest_string(&mut hasher, stop_sequence);
    }
    hex::encode(hasher.finalize())
}

fn digest_fixture_tokenizer(tokenizer: &FixtureWordTokenizer) -> String {
    let mut hasher = Sha256::new();
    for token in tokenizer.vocabulary().tokens() {
        update_digest_string(&mut hasher, token);
    }
    hasher.update(tokenizer.vocabulary().pad_id().as_u32().to_be_bytes());
    hasher.update(tokenizer.vocabulary().bos_id().as_u32().to_be_bytes());
    hasher.update(tokenizer.vocabulary().eos_id().as_u32().to_be_bytes());
    hasher.update(tokenizer.vocabulary().unknown_id().as_u32().to_be_bytes());
    hex::encode(hasher.finalize())
}

fn default_chat_template_stop_sequences(chat_templates: &GgufChatTemplateMetadata) -> Vec<String> {
    let Some(default_template) = chat_templates.default_template() else {
        return Vec::new();
    };
    let template_digest = digest_chat_template(default_template);
    supported_prompt_template_family(template_digest.as_str())
        .map(|family| {
            family
                .stop_sequences()
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn update_optional_token_id(hasher: &mut Sha256, token_id: Option<TokenId>) {
    match token_id {
        Some(token_id) => {
            hasher.update([1]);
            hasher.update(token_id.as_u32().to_be_bytes());
        }
        None => hasher.update([0]),
    }
}

fn update_digest_string(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

struct GgufBytesReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> GgufBytesReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn position(&self) -> usize {
        self.offset
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], ModelLoadError> {
        let end = self.offset.checked_add(len).ok_or_else(|| {
            artifact_format_error("gguf", "reader offset overflow while parsing artifact")
        })?;
        let bytes = self.bytes.get(self.offset..end).ok_or_else(|| {
            artifact_format_error(
                "gguf",
                format!(
                    "unexpected end of file while parsing gguf at byte {}",
                    self.offset
                ),
            )
        })?;
        self.offset = end;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, ModelLoadError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_i8(&mut self) -> Result<i8, ModelLoadError> {
        Ok(i8::from_le_bytes([self.read_u8()?]))
    }

    fn read_u16(&mut self) -> Result<u16, ModelLoadError> {
        let mut bytes = [0_u8; 2];
        bytes.copy_from_slice(self.read_exact(2)?);
        Ok(u16::from_le_bytes(bytes))
    }

    fn read_i16(&mut self) -> Result<i16, ModelLoadError> {
        let mut bytes = [0_u8; 2];
        bytes.copy_from_slice(self.read_exact(2)?);
        Ok(i16::from_le_bytes(bytes))
    }

    fn read_u32(&mut self) -> Result<u32, ModelLoadError> {
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(self.read_exact(4)?);
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_i32(&mut self) -> Result<i32, ModelLoadError> {
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(self.read_exact(4)?);
        Ok(i32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self) -> Result<u64, ModelLoadError> {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(self.read_exact(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_i64(&mut self) -> Result<i64, ModelLoadError> {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(self.read_exact(8)?);
        Ok(i64::from_le_bytes(bytes))
    }

    fn read_f32(&mut self) -> Result<f32, ModelLoadError> {
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(self.read_exact(4)?);
        Ok(f32::from_le_bytes(bytes))
    }

    fn read_f64(&mut self) -> Result<f64, ModelLoadError> {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(self.read_exact(8)?);
        Ok(f64::from_le_bytes(bytes))
    }
}

fn read_gguf_string(
    reader: &mut GgufBytesReader<'_>,
    version: GgufVersion,
) -> Result<String, ModelLoadError> {
    let raw_len = match version {
        GgufVersion::V1 => u64::from(reader.read_u32()?),
        GgufVersion::V2 | GgufVersion::V3 => reader.read_u64()?,
    };
    let len = usize::try_from(raw_len).map_err(|_| {
        artifact_format_error(
            "gguf",
            format!("string length `{raw_len}` does not fit into usize"),
        )
    })?;
    let mut bytes = reader.read_exact(len)?.to_vec();
    while bytes.last() == Some(&0) {
        bytes.pop();
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn align_offset(position: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    position.div_ceil(alignment) * alignment
}

fn decode_ggml_quantized_values(
    quantization: QuantizationMode,
    layout: QuantizedBlockLayout,
    bytes: &[u8],
) -> Result<Vec<f32>, ModelLoadError> {
    match quantization {
        QuantizationMode::GgmlMxfp4 => decode_mxfp4_blocks(layout, bytes),
        QuantizationMode::GgmlQ4_0 => decode_q4_0_blocks(layout, bytes),
        QuantizationMode::GgmlQ4_1 => decode_q4_1_blocks(layout, bytes),
        QuantizationMode::GgmlQ8_0 => decode_q8_0_blocks(layout, bytes),
        QuantizationMode::None | QuantizationMode::Int8Symmetric => {
            Err(ModelLoadError::UnsupportedQuantizedTensorMode { quantization })
        }
    }
}

fn decode_mxfp4_blocks(
    layout: QuantizedBlockLayout,
    bytes: &[u8],
) -> Result<Vec<f32>, ModelLoadError> {
    const KVALUES: [i8; 16] = [0, 1, 2, 3, 4, 6, 8, 12, 0, -1, -2, -3, -4, -6, -8, -12];
    let half_block = layout.elements_per_block / 2;
    decode_fixed_width_blocks(layout, bytes, 17, |block, output| {
        let scale = decode_e8m0_to_fp32_half(block[0]) * 0.5;
        let start = output.len();
        output.resize(start + (half_block * 2), 0.0);
        for (j, packed) in block[1..17].iter().enumerate() {
            let low = usize::from(packed & 0x0f);
            let high = usize::from((packed >> 4) & 0x0f);
            output[start + j] = f32::from(KVALUES[low]) * scale;
            output[start + j + half_block] = f32::from(KVALUES[high]) * scale;
        }
    })
}

fn decode_q8_0_blocks(
    layout: QuantizedBlockLayout,
    bytes: &[u8],
) -> Result<Vec<f32>, ModelLoadError> {
    decode_fixed_width_blocks(layout, bytes, 34, |block, output| {
        let scale = decode_f16([block[0], block[1]]);
        for quantized in &block[2..34] {
            output.push(f32::from(i8::from_le_bytes([*quantized])) * scale);
        }
    })
}

fn decode_q4_0_blocks(
    layout: QuantizedBlockLayout,
    bytes: &[u8],
) -> Result<Vec<f32>, ModelLoadError> {
    let half_block = layout.elements_per_block / 2;
    decode_fixed_width_blocks(layout, bytes, 18, |block, output| {
        let scale = decode_f16([block[0], block[1]]);
        let start = output.len();
        output.resize(start + (half_block * 2), 0.0);
        for (j, packed) in block[2..18].iter().enumerate() {
            let low = (packed & 0x0f) as i8 - 8;
            let high = ((packed >> 4) & 0x0f) as i8 - 8;
            output[start + j] = f32::from(low) * scale;
            output[start + j + half_block] = f32::from(high) * scale;
        }
    })
}

fn decode_q4_1_blocks(
    layout: QuantizedBlockLayout,
    bytes: &[u8],
) -> Result<Vec<f32>, ModelLoadError> {
    let half_block = layout.elements_per_block / 2;
    decode_fixed_width_blocks(layout, bytes, 20, |block, output| {
        let scale = decode_f16([block[0], block[1]]);
        let minimum = decode_f16([block[2], block[3]]);
        let start = output.len();
        output.resize(start + (half_block * 2), 0.0);
        for (j, packed) in block[4..20].iter().enumerate() {
            let low = f32::from((packed & 0x0f) as i8) * scale + minimum;
            let high = f32::from(((packed >> 4) & 0x0f) as i8) * scale + minimum;
            output[start + j] = low;
            output[start + j + half_block] = high;
        }
    })
}

fn decode_fixed_width_blocks(
    layout: QuantizedBlockLayout,
    bytes: &[u8],
    expected_bytes_per_block: usize,
    mut decode_block: impl FnMut(&[u8], &mut Vec<f32>),
) -> Result<Vec<f32>, ModelLoadError> {
    if layout.bytes_per_block != expected_bytes_per_block || bytes.len() != layout.byte_len() {
        return Err(ModelLoadError::InvalidQuantizedTensorByteLength {
            quantization: quantization_from_block_bytes(expected_bytes_per_block),
            expected: layout.byte_len(),
            actual: bytes.len(),
        });
    }

    let mut values = Vec::with_capacity(layout.element_count());
    for block in bytes.chunks_exact(expected_bytes_per_block) {
        decode_block(block, &mut values);
    }
    Ok(values)
}

fn quantization_from_block_bytes(bytes_per_block: usize) -> QuantizationMode {
    match bytes_per_block {
        17 => QuantizationMode::GgmlMxfp4,
        18 => QuantizationMode::GgmlQ4_0,
        20 => QuantizationMode::GgmlQ4_1,
        34 => QuantizationMode::GgmlQ8_0,
        _ => QuantizationMode::None,
    }
}

fn decode_e8m0_to_fp32_half(value: u8) -> f32 {
    let bits = if value == 0 {
        0x0040_0000_u32
    } else {
        u32::from(value) << 23
    };
    f32::from_bits(bits)
}

fn decode_f16(bytes: [u8; 2]) -> f32 {
    let bits = u16::from_le_bytes(bytes);
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let mantissa = bits & 0x03ff;

    let f32_bits = match (exponent, mantissa) {
        (0, 0) => sign,
        (0, mantissa) => {
            let mut mantissa = u32::from(mantissa);
            let mut exponent = 113_u32;
            while mantissa & 0x0400 == 0 {
                mantissa <<= 1;
                exponent -= 1;
            }
            mantissa &= 0x03ff;
            sign | (exponent << 23) | (mantissa << 13)
        }
        (0x1f, mantissa) => sign | 0x7f80_0000 | (u32::from(mantissa) << 13),
        (exponent, mantissa) => {
            sign | ((u32::from(exponent) + 112) << 23) | (u32::from(mantissa) << 13)
        }
    };

    f32::from_bits(f32_bits)
}

fn decode_f32_values(format: &str, name: &str, data: &[u8]) -> Result<Vec<f32>, ModelLoadError> {
    let chunks = data.chunks_exact(size_of::<f32>());
    if !chunks.remainder().is_empty() {
        return Err(artifact_format_error(
            format,
            format!("tensor `{name}` byte length is not a multiple of 4"),
        ));
    }
    Ok(chunks
        .map(|chunk| {
            let mut bytes = [0_u8; size_of::<f32>()];
            bytes.copy_from_slice(chunk);
            f32::from_le_bytes(bytes)
        })
        .collect())
}

fn decode_f16_values(format: &str, name: &str, data: &[u8]) -> Result<Vec<f32>, ModelLoadError> {
    let chunks = data.chunks_exact(size_of::<u16>());
    if !chunks.remainder().is_empty() {
        return Err(artifact_format_error(
            format,
            format!("tensor `{name}` byte length is not a multiple of 2"),
        ));
    }
    Ok(chunks
        .map(|chunk| {
            let mut bytes = [0_u8; size_of::<u16>()];
            bytes.copy_from_slice(chunk);
            decode_f16(bytes)
        })
        .collect())
}

fn decode_bf16_values(format: &str, name: &str, data: &[u8]) -> Result<Vec<f32>, ModelLoadError> {
    let chunks = data.chunks_exact(size_of::<u16>());
    if !chunks.remainder().is_empty() {
        return Err(artifact_format_error(
            format,
            format!("tensor `{name}` byte length is not a multiple of 2"),
        ));
    }
    Ok(chunks
        .map(|chunk| {
            let mut bytes = [0_u8; size_of::<u16>()];
            bytes.copy_from_slice(chunk);
            decode_bf16(bytes)
        })
        .collect())
}

fn decode_bf16(bytes: [u8; 2]) -> f32 {
    f32::from_bits(u32::from(u16::from_le_bytes(bytes)) << 16)
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
    let scale = decode_f32_values("safetensors", name, scale_tensor.data())?
        .into_iter()
        .next()
        .ok_or_else(|| ModelLoadError::MissingTensorScale(String::from(name)))?;
    Ok(data
        .iter()
        .map(|byte| f32::from(i8::from_le_bytes([*byte])) * scale)
        .collect())
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::unwrap_used
)]
mod tests {
    use std::{collections::BTreeMap, path::Path};

    use psionic_catalog::{BlobReadPreference, OllamaCatalogSurface, OllamaModelCatalog};
    use psionic_core::{DType, QuantizationMode, QuantizedBlockLayout, Shape};
    use safetensors::{Dtype as SafeTensorsDType, serialize_to_file, tensor::TensorView};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::{
        ActivationFunction, ByteProjectionEmbedder, ContextOverflowPolicy, ContextWindowError,
        DecoderModelDescriptor, DecoderWeightLoader, FixtureDecoderLoader, FixtureWordTokenizer,
        GgufBlobArtifact, GgufContent, GgufDecoderAdapterLoader, GgufDecoderFamily,
        GgufEmbeddingAdapterLoader, GgufEmbeddingFamily, GgufEmbeddingPooling, GgufMetadataValue,
        GgufPromptTemplateFamily, GgufPromptTemplateRenderer, GgufTensorType,
        GgufTokenizerMetadata, GgufTokenizerModel, GgufTokenizerPretokenizer, GgufVersion,
        GgufWeightBundleLoader, GptOssHarmonyParseOptions, GptOssHarmonyParseSource,
        GptOssHarmonyStreamParser, LoadedWeightTensor, LocalBlobOpenOptions,
        LocalWeightBundleLoader, ParsedReasoningResponse, PromptMessage, PromptMessageRole,
        PromptRenderOptions, QuantizedTensorStorage, ReasoningParser, ReasoningResponsePartKind,
        ReferenceWordDecoder, SafeTensorsDecoderLoader, SafeTensorsWeightBundleLoader,
        SmokeByteEmbedder, TokenId, TokenSequence, TokenizerBoundary, WeightArtifactBlobKind,
        WeightArtifactReadPath, WeightFormat, WeightSource, WeightTensorStorage,
        apply_context_window, apply_special_token_defaults, assert_prompt_template_fixture_matches,
        assert_prompt_window_case, assert_rendered_prompt_case, assert_tokenizer_fixture_matches,
        digest_chat_template, golden_prompt_fixture, golden_prompt_fixtures,
        golden_tokenizer_fixture, golden_tokenizer_fixtures, parse_gpt_oss_harmony_text,
        parse_gpt_oss_harmony_tokens, parse_reasoning_response_text_for_decoder_family,
        reasoning_parser_for_decoder_family,
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
    fn context_window_refuses_overflow_when_truncation_is_disabled() {
        let prompt = TokenSequence::new(vec![
            FixtureWordTokenizer::BOS_ID,
            FixtureWordTokenizer::HELLO_ID,
            FixtureWordTokenizer::WORLD_ID,
        ]);
        let error = apply_context_window(&prompt, 8, 0, 6, ContextOverflowPolicy::Refuse, 1)
            .expect_err("overflow should refuse");

        assert!(matches!(
            error,
            ContextWindowError::CannotTruncateFurther {
                max_context_tokens: 8,
                existing_context_tokens: 0,
                reserved_output_tokens: 6,
                input_prompt_tokens: 3,
                available_prompt_tokens: 2,
                policy: ContextOverflowPolicy::Refuse,
            }
        ));
    }

    #[test]
    fn context_window_truncates_oldest_tokens_while_preserving_prefix() {
        let prompt = TokenSequence::new(vec![
            FixtureWordTokenizer::BOS_ID,
            FixtureWordTokenizer::HELLO_ID,
            FixtureWordTokenizer::OPEN_ID,
            FixtureWordTokenizer::AGENTS_ID,
        ]);
        let (retained, accounting) =
            apply_context_window(&prompt, 8, 0, 6, ContextOverflowPolicy::TruncateOldest, 1)
                .expect("truncate oldest");

        assert_eq!(
            retained.as_slice(),
            &[
                FixtureWordTokenizer::BOS_ID,
                FixtureWordTokenizer::AGENTS_ID
            ]
        );
        assert_eq!(accounting.budget.available_prompt_tokens, 2);
        assert_eq!(accounting.input_prompt_tokens, 4);
        assert_eq!(accounting.retained_prompt_tokens, 2);
        assert_eq!(accounting.truncated_prompt_tokens, 2);
        assert_eq!(accounting.preserved_prefix_tokens, 1);
        assert!(accounting.overflowed());
    }

    #[test]
    fn context_window_rejects_truncation_when_only_preserved_prefix_would_remain() {
        let prompt = TokenSequence::new(vec![
            FixtureWordTokenizer::BOS_ID,
            FixtureWordTokenizer::HELLO_ID,
            FixtureWordTokenizer::WORLD_ID,
        ]);
        let error =
            apply_context_window(&prompt, 8, 0, 7, ContextOverflowPolicy::TruncateOldest, 1)
                .expect_err("prefix-only truncation should refuse");

        assert!(matches!(
            error,
            ContextWindowError::ExceedsAfterTruncation {
                max_context_tokens: 8,
                existing_context_tokens: 0,
                reserved_output_tokens: 7,
                input_prompt_tokens: 3,
                available_prompt_tokens: 1,
                preserved_prefix_tokens: 1,
                retained_prompt_tokens: 1,
                policy: ContextOverflowPolicy::TruncateOldest,
            }
        ));
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
        assert_eq!(tensor.values()?.len(), model.descriptor().config.vocab_size);
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
        assert_eq!(tensor.values()?.as_ref(), &[0.5, -0.5, 1.0, -1.0]);
        Ok(())
    }

    #[test]
    fn gguf_content_reads_metadata_and_tensor_infos() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("sample_v2.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V2,
            &[
                (
                    String::from("general.architecture"),
                    GgufMetadataValue::String(String::from("llama")),
                ),
                (
                    String::from("general.alignment"),
                    GgufMetadataValue::U32(64),
                ),
                (
                    String::from("tokenizer.ggml.tokens"),
                    GgufMetadataValue::Array(vec![
                        GgufMetadataValue::String(String::from("hello")),
                        GgufMetadataValue::String(String::from("world")),
                    ]),
                ),
            ],
            &[TestGgufTensor::new(
                "dense",
                vec![2, 2],
                GgufTensorType::F32,
                super::encode_f32_bytes(&[1.0, 2.0, 3.0, 4.0]),
            )],
        )?;

        let content = GgufContent::read_path(&path)?;
        let bytes = std::fs::read(&path)?;

        assert_eq!(content.version(), GgufVersion::V2);
        assert_eq!(content.alignment(), 64);
        assert_eq!(content.tensor_data_offset() % 64, 0);
        assert_eq!(
            content
                .metadata()
                .get("general.architecture")
                .and_then(GgufMetadataValue::as_str),
            Some("llama")
        );

        let token_values = content
            .metadata()
            .get("tokenizer.ggml.tokens")
            .and_then(GgufMetadataValue::as_array)
            .ok_or("missing tokenizer tokens")?;
        assert_eq!(
            token_values,
            &[
                GgufMetadataValue::String(String::from("hello")),
                GgufMetadataValue::String(String::from("world")),
            ]
        );

        let tensor = content.tensor_info("dense").ok_or("missing dense tensor")?;
        assert_eq!(tensor.shape, Shape::new(vec![2, 2]));
        assert_eq!(tensor.tensor_type, GgufTensorType::F32);
        assert_eq!(content.tensor_bytes(&bytes, "dense")?.len(), 16);
        Ok(())
    }

    #[test]
    fn gguf_content_loads_sentencepiece_tokenizer_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("sentencepiece.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[
                (
                    String::from("tokenizer.ggml.model"),
                    GgufMetadataValue::String(String::from("llama")),
                ),
                (
                    String::from("tokenizer.ggml.tokens"),
                    GgufMetadataValue::Array(vec![
                        GgufMetadataValue::String(String::from("<unk>")),
                        GgufMetadataValue::String(String::from("<s>")),
                        GgufMetadataValue::String(String::from("</s>")),
                        GgufMetadataValue::String(String::from("hello")),
                        GgufMetadataValue::String(String::from("▁world")),
                    ]),
                ),
                (
                    String::from("tokenizer.ggml.scores"),
                    GgufMetadataValue::Array(vec![
                        GgufMetadataValue::F32(0.0),
                        GgufMetadataValue::F32(0.0),
                        GgufMetadataValue::F32(0.0),
                        GgufMetadataValue::F32(-0.25),
                        GgufMetadataValue::F32(-0.5),
                    ]),
                ),
                (
                    String::from("tokenizer.ggml.token_type"),
                    GgufMetadataValue::Array(vec![
                        GgufMetadataValue::I32(2),
                        GgufMetadataValue::I32(3),
                        GgufMetadataValue::I32(3),
                        GgufMetadataValue::I32(1),
                        GgufMetadataValue::I32(1),
                    ]),
                ),
                (
                    String::from("tokenizer.ggml.bos_token_id"),
                    GgufMetadataValue::U32(1),
                ),
                (
                    String::from("tokenizer.ggml.eos_token_id"),
                    GgufMetadataValue::U32(2),
                ),
                (
                    String::from("tokenizer.ggml.unknown_token_id"),
                    GgufMetadataValue::U32(0),
                ),
                (
                    String::from("tokenizer.ggml.add_bos_token"),
                    GgufMetadataValue::Bool(true),
                ),
                (
                    String::from("tokenizer.ggml.add_eos_token"),
                    GgufMetadataValue::Bool(false),
                ),
            ],
            &[],
        )?;

        let content = GgufContent::read_path(&path)?;
        let tokenizer = content.load_tokenizer()?;

        assert_eq!(tokenizer.model, GgufTokenizerModel::SentencePiece);
        assert_eq!(tokenizer.vocabulary.len(), 5);
        assert_eq!(tokenizer.vocabulary.token_id("hello"), Some(TokenId(3)));
        assert_eq!(tokenizer.vocabulary.token(TokenId(4)), Some("▁world"));
        assert_eq!(tokenizer.vocabulary.bos_token_id(), Some(TokenId(1)));
        assert_eq!(tokenizer.vocabulary.eos_token_ids(), &[TokenId(2)]);
        assert_eq!(tokenizer.vocabulary.unknown_token_id(), Some(TokenId(0)));
        assert_eq!(tokenizer.scores, vec![0.0, 0.0, 0.0, -0.25, -0.5]);
        assert_eq!(tokenizer.token_types, vec![2, 3, 3, 1, 1]);
        assert!(tokenizer.merges.is_empty());
        assert_eq!(tokenizer.pretokenizer, None);
        assert!(tokenizer.add_bos);
        assert!(!tokenizer.add_eos);
        assert!(!tokenizer.digest().is_empty());
        assert_eq!(tokenizer, content.load_tokenizer()?);
        Ok(())
    }

    #[test]
    fn gguf_content_loads_gpt_style_bpe_tokenizer_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("gpt2.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[
                (
                    String::from("tokenizer.ggml.model"),
                    GgufMetadataValue::String(String::from("gpt2")),
                ),
                (
                    String::from("tokenizer.ggml.pre"),
                    GgufMetadataValue::String(String::from("qwen2")),
                ),
                (
                    String::from("tokenizer.ggml.tokens"),
                    GgufMetadataValue::Array(vec![
                        GgufMetadataValue::String(String::from("<|bos|>")),
                        GgufMetadataValue::String(String::from("<|eos|>")),
                        GgufMetadataValue::String(String::from("h")),
                        GgufMetadataValue::String(String::from("e")),
                        GgufMetadataValue::String(String::from("l")),
                        GgufMetadataValue::String(String::from("o")),
                        GgufMetadataValue::String(String::from("he")),
                        GgufMetadataValue::String(String::from("ll")),
                        GgufMetadataValue::String(String::from("hello")),
                    ]),
                ),
                (
                    String::from("tokenizer.ggml.merges"),
                    GgufMetadataValue::Array(vec![
                        GgufMetadataValue::String(String::from("h e")),
                        GgufMetadataValue::String(String::from("l l")),
                        GgufMetadataValue::String(String::from("he ll")),
                        GgufMetadataValue::String(String::from("hell o")),
                    ]),
                ),
                (
                    String::from("tokenizer.ggml.bos_token_id"),
                    GgufMetadataValue::U32(0),
                ),
                (
                    String::from("tokenizer.ggml.eos_token_id"),
                    GgufMetadataValue::U32(1),
                ),
                (
                    String::from("tokenizer.ggml.eos_token_ids"),
                    GgufMetadataValue::Array(vec![
                        GgufMetadataValue::U32(1),
                        GgufMetadataValue::U32(1),
                        GgufMetadataValue::U32(8),
                    ]),
                ),
                (
                    String::from("tokenizer.ggml.add_bos_token"),
                    GgufMetadataValue::Bool(false),
                ),
                (
                    String::from("tokenizer.ggml.add_eos_token"),
                    GgufMetadataValue::Bool(false),
                ),
            ],
            &[],
        )?;

        let content = GgufContent::read_path(&path)?;
        let tokenizer = content.load_tokenizer()?;

        assert_eq!(tokenizer.model, GgufTokenizerModel::Gpt2Bpe);
        assert_eq!(
            tokenizer.pretokenizer,
            Some(GgufTokenizerPretokenizer::Qwen2)
        );
        assert_eq!(
            tokenizer.merges,
            vec![
                String::from("h e"),
                String::from("l l"),
                String::from("he ll"),
                String::from("hell o"),
            ]
        );
        assert_eq!(tokenizer.vocabulary.bos_token_id(), Some(TokenId(0)));
        assert_eq!(
            tokenizer.vocabulary.eos_token_ids(),
            &[TokenId(1), TokenId(8)]
        );
        assert!(!tokenizer.add_bos);
        assert!(!tokenizer.add_eos);
        Ok(())
    }

    #[test]
    fn gguf_content_loads_bert_wordpiece_tokenizer_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("bert_tokenizer.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &bert_wordpiece_tokenizer_metadata_entries(2),
            &[],
        )?;

        let content = GgufContent::read_path(&path)?;
        let tokenizer = content.load_tokenizer()?;

        assert_eq!(tokenizer.model, GgufTokenizerModel::BertWordPiece);
        assert_eq!(tokenizer.vocabulary.len(), 6);
        assert_eq!(tokenizer.vocabulary.bos_token_id(), Some(TokenId(1)));
        assert_eq!(tokenizer.vocabulary.eos_token_ids(), &[TokenId(2)]);
        assert_eq!(tokenizer.vocabulary.pad_token_id(), Some(TokenId(0)));
        assert_eq!(tokenizer.vocabulary.unknown_token_id(), Some(TokenId(3)));
        assert_eq!(tokenizer.token_type_count, Some(2));
        assert!(tokenizer.merges.is_empty());
        assert_eq!(tokenizer.pretokenizer, None);
        assert!(tokenizer.add_bos);
        assert!(tokenizer.add_eos);
        Ok(())
    }

    #[test]
    fn gguf_content_rejects_unsupported_tokenizer_model() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempdir()?;
        let path = temp.path().join("unsupported_tokenizer.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[
                (
                    String::from("tokenizer.ggml.model"),
                    GgufMetadataValue::String(String::from("rwkv")),
                ),
                (
                    String::from("tokenizer.ggml.tokens"),
                    GgufMetadataValue::Array(vec![GgufMetadataValue::String(String::from(
                        "hello",
                    ))]),
                ),
            ],
            &[],
        )?;

        let content = GgufContent::read_path(&path)?;
        let error = content
            .load_tokenizer()
            .expect_err("unsupported tokenizer model should fail");
        assert!(matches!(
            error,
            super::ModelLoadError::UnsupportedTokenizerModel { model } if model == "rwkv"
        ));
        Ok(())
    }

    #[test]
    fn gguf_content_rejects_out_of_range_tokenizer_ids() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("bad_tokenizer_ids.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[
                (
                    String::from("tokenizer.ggml.model"),
                    GgufMetadataValue::String(String::from("llama")),
                ),
                (
                    String::from("tokenizer.ggml.tokens"),
                    GgufMetadataValue::Array(vec![
                        GgufMetadataValue::String(String::from("<unk>")),
                        GgufMetadataValue::String(String::from("hello")),
                    ]),
                ),
                (
                    String::from("tokenizer.ggml.bos_token_id"),
                    GgufMetadataValue::U32(4),
                ),
            ],
            &[],
        )?;

        let content = GgufContent::read_path(&path)?;
        let error = content
            .load_tokenizer()
            .expect_err("out-of-range token ids should fail");
        assert!(matches!(
            error,
            super::ModelLoadError::InvalidTokenizerMetadata { key, .. }
                if key == "tokenizer.ggml.bos_token_id"
        ));
        Ok(())
    }

    #[test]
    fn gguf_content_loads_chat_template_metadata_and_named_variants()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("chat_templates.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[
                (
                    String::from("tokenizer.chat_template"),
                    GgufMetadataValue::String(String::from("{{ prompt }}")),
                ),
                (
                    String::from("tokenizer.chat_templates"),
                    GgufMetadataValue::Array(vec![
                        GgufMetadataValue::String(String::from("tool_use")),
                        GgufMetadataValue::String(String::from("rag")),
                    ]),
                ),
                (
                    String::from("tokenizer.chat_template.tool_use"),
                    GgufMetadataValue::String(String::from("{{ tool_prompt }}")),
                ),
                (
                    String::from("tokenizer.chat_template.rag"),
                    GgufMetadataValue::String(String::from("{{ rag_prompt }}")),
                ),
            ],
            &[],
        )?;

        let content = GgufContent::read_path(&path)?;
        let templates = content.load_chat_templates()?;

        assert_eq!(templates.default_template(), Some("{{ prompt }}"));
        assert_eq!(
            templates.template(Some("tool_use")),
            Some("{{ tool_prompt }}")
        );
        assert_eq!(templates.template(Some("rag")), Some("{{ rag_prompt }}"));
        assert_eq!(templates.named_templates().len(), 2);
        assert!(!templates.is_empty());
        assert_eq!(templates, content.load_chat_templates()?);
        assert!(!templates.digest().is_empty());
        Ok(())
    }

    #[test]
    fn golden_tokenizer_fixtures_are_available_and_reviewable() {
        let ids = golden_tokenizer_fixtures()
            .iter()
            .map(|fixture| fixture.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["llama_spm", "qwen2", "gpt_oss_20b"]);

        let gpt_oss = golden_tokenizer_fixture("gpt_oss_20b").expect("gpt-oss fixture");
        assert_eq!(gpt_oss.pretokenizer, Some("gpt-4o"));
        assert_eq!(gpt_oss.sample_tokens.len(), 3);
    }

    #[test]
    fn golden_prompt_fixtures_hash_to_expected_digests() -> Result<(), Box<dyn std::error::Error>> {
        let prompt_fixtures = golden_prompt_fixtures();
        assert_eq!(prompt_fixtures.len(), 4);

        for fixture in prompt_fixtures {
            for variant in fixture.template_variants {
                assert!(!variant.template_digest.is_empty());
                assert!(!variant.template_excerpt.is_empty());
                if let Some(raw_template) = variant.raw_template {
                    assert_prompt_template_fixture_matches(variant, raw_template)?;
                    assert_eq!(digest_chat_template(raw_template), variant.template_digest);
                }
            }
        }

        let command_r = golden_prompt_fixture("command_r").expect("command-r fixture");
        assert!(command_r.template_variant("command_r.tool_use").is_some());
        assert!(command_r.template_variant("command_r.rag").is_some());

        let gpt_oss = golden_prompt_fixture("gpt_oss").expect("gpt-oss fixture");
        assert_eq!(gpt_oss.template_variants[0].render_cases.len(), 2);
        Ok(())
    }

    #[test]
    fn golden_prompt_window_cases_reference_real_render_cases()
    -> Result<(), Box<dyn std::error::Error>> {
        for fixture in golden_prompt_fixtures() {
            for window_case in fixture.window_cases {
                let variant = fixture
                    .template_variant(window_case.template_variant_id)
                    .expect("variant");
                let render_case = variant
                    .render_case(window_case.render_case_id)
                    .expect("render case");
                assert_rendered_prompt_case(render_case, render_case.expected_rendered)?;
                assert_prompt_window_case(window_case, render_case.expected_rendered)?;
            }
        }
        Ok(())
    }

    #[test]
    fn gguf_prompt_template_renderer_matches_phi3_fixture_render_case()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = golden_prompt_fixture("phi3").expect("phi3 fixture");
        let variant = fixture.template_variant("phi3.default").expect("variant");
        let render_case = variant.render_case("phi3.multi_turn").expect("render case");
        let renderer = GgufPromptTemplateRenderer::new(
            phi3_prompt_tokenizer_metadata(),
            super::GgufChatTemplateMetadata::new(
                variant.raw_template.map(String::from),
                BTreeMap::new(),
            ),
        );

        let rendered = renderer.render(
            None,
            prompt_messages_from_fixture(render_case.messages).as_slice(),
            render_case.add_generation_prompt,
        )?;

        assert_eq!(rendered.family, GgufPromptTemplateFamily::Phi3);
        assert_eq!(rendered.text, render_case.expected_rendered);
        assert_eq!(
            rendered.stop_sequences,
            variant
                .stop_sequences
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn gguf_prompt_template_renderer_matches_command_r_fixture_render_case()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = golden_prompt_fixture("command_r").expect("command-r fixture");
        let variant = fixture
            .template_variant("command_r.default")
            .expect("variant");
        let render_case = variant
            .render_case("command_r.with_system_history")
            .expect("render case");
        let renderer = GgufPromptTemplateRenderer::new(
            command_r_prompt_tokenizer_metadata(),
            super::GgufChatTemplateMetadata::new(
                variant.raw_template.map(String::from),
                BTreeMap::new(),
            ),
        );

        let rendered = renderer.render(
            None,
            prompt_messages_from_fixture(render_case.messages).as_slice(),
            render_case.add_generation_prompt,
        )?;

        assert_eq!(rendered.family, GgufPromptTemplateFamily::CommandR);
        assert_eq!(rendered.text, render_case.expected_rendered);
        Ok(())
    }

    #[test]
    fn gguf_decoder_adapter_render_prompt_matches_qwen2_fixture()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("tiny_qwen2.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &qwen2_decoder_metadata(
                "Tiny Qwen2",
                golden_prompt_fixture("qwen2")
                    .and_then(|fixture| fixture.template_variant("qwen2.default"))
                    .and_then(|variant| variant.raw_template),
            ),
            &decoder_family_tensors(1, true, false),
        )?;

        let adapter = GgufDecoderAdapterLoader.load_path(&path)?;
        let fixture = golden_prompt_fixture("qwen2").expect("qwen2 fixture");
        let render_case = fixture
            .template_variant("qwen2.default")
            .and_then(|variant| variant.render_case("qwen2.with_system_history"))
            .expect("render case");

        let rendered = adapter.render_prompt(
            None,
            prompt_messages_from_fixture(render_case.messages).as_slice(),
            render_case.add_generation_prompt,
        )?;

        assert_eq!(rendered.family, GgufPromptTemplateFamily::Qwen2);
        assert_eq!(rendered.text, render_case.expected_rendered);
        Ok(())
    }

    #[test]
    fn gguf_prompt_template_renderer_matches_gpt_oss_fixture_render_case()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = golden_prompt_fixture("gpt_oss").expect("gpt-oss fixture");
        let variant = fixture
            .template_variant("gpt_oss.default")
            .expect("variant");
        let render_case = variant
            .render_case("gpt_oss.reasoning_with_developer")
            .expect("render case");
        let content = GgufContent::read_path(Path::new(fixture.source_path))?;
        let renderer = GgufPromptTemplateRenderer::new(
            content.load_tokenizer()?,
            content.load_chat_templates()?,
        );

        let rendered = renderer.render_with_options(
            None,
            prompt_messages_from_fixture(render_case.messages).as_slice(),
            render_case.add_generation_prompt,
            &prompt_render_options_from_fixture(render_case.harmony_context),
        )?;

        assert_eq!(rendered.family, GgufPromptTemplateFamily::GptOss);
        assert_eq!(rendered.text, render_case.expected_rendered);
        Ok(())
    }

    #[test]
    fn gpt_oss_harmony_parser_parses_final_only_output() -> Result<(), Box<dyn std::error::Error>> {
        let output = "<|channel|>final<|message|>323";
        let encoding = openai_harmony::load_harmony_encoding(
            openai_harmony::HarmonyEncodingName::HarmonyGptOss,
        )?;
        let tokens = encoding
            .tokenizer()
            .encode_with_special_tokens(output)
            .into_iter()
            .map(TokenId)
            .collect::<Vec<_>>();
        let parsed = parse_gpt_oss_harmony_tokens(
            &tokens,
            GptOssHarmonyParseOptions {
                role_hint: Some(PromptMessageRole::Assistant),
                strict: true,
            },
        )?;

        assert_eq!(parsed.source, GptOssHarmonyParseSource::Tokens);
        assert_eq!(
            parsed.messages,
            vec![PromptMessage::new(PromptMessageRole::Assistant, "323").with_channel("final")]
        );
        Ok(())
    }

    #[test]
    fn gpt_oss_harmony_parser_parses_analysis_then_final_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let text = "<|channel|>analysis<|message|>working<|end|><|start|>assistant<|channel|>final<|message|>323";
        let parsed = parse_gpt_oss_harmony_text(
            text,
            GptOssHarmonyParseOptions {
                role_hint: Some(PromptMessageRole::Assistant),
                strict: true,
            },
        )?;

        assert_eq!(
            parsed.messages,
            vec![
                PromptMessage::new(PromptMessageRole::Assistant, "working")
                    .with_channel("analysis"),
                PromptMessage::new(PromptMessageRole::Assistant, "323").with_channel("final"),
            ]
        );
        Ok(())
    }

    #[test]
    fn gpt_oss_harmony_parser_parses_tool_call_forms() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_gpt_oss_harmony_text(
            "<|start|>assistant<|channel|>commentary to=functions.get_weather<|constrain|>json<|message|>{\"latitude\":48.8566,\"longitude\":2.3522}<|call|>",
            GptOssHarmonyParseOptions::default(),
        )?;

        assert_eq!(
            parsed.messages,
            vec![
                PromptMessage::new(
                    PromptMessageRole::Assistant,
                    "{\"latitude\":48.8566,\"longitude\":2.3522}",
                )
                .with_recipient("functions.get_weather")
                .with_channel("commentary")
                .with_content_type("<|constrain|>json"),
            ]
        );
        Ok(())
    }

    #[test]
    fn gpt_oss_harmony_reasoning_response_separates_final_reasoning_and_tool_calls()
    -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_gpt_oss_harmony_text(
            concat!(
                "<|channel|>analysis<|message|>thinking<|end|>",
                "<|start|>assistant<|channel|>final<|message|>323<|end|>",
                "<|start|>assistant<|channel|>commentary to=functions.get_weather<|constrain|>json<|message|>{\"city\":\"Paris\"}<|call|>",
            ),
            GptOssHarmonyParseOptions {
                role_hint: Some(PromptMessageRole::Assistant),
                strict: true,
            },
        )?;

        let typed = parsed.reasoning_response();
        assert_eq!(typed.parser, ReasoningParser::GptOssHarmony);
        assert_eq!(typed.final_content.as_deref(), Some("323"));
        assert_eq!(typed.reasoning_content.as_deref(), Some("thinking"));
        assert_eq!(
            typed.parts.iter().map(|part| part.kind).collect::<Vec<_>>(),
            vec![
                ReasoningResponsePartKind::Reasoning,
                ReasoningResponsePartKind::Final,
                ReasoningResponsePartKind::ToolCall
            ]
        );
        assert_eq!(
            typed
                .parts
                .iter()
                .find(|part| part.kind == ReasoningResponsePartKind::ToolCall)
                .and_then(|part| part.recipient.as_deref()),
            Some("functions.get_weather")
        );
        Ok(())
    }

    #[test]
    fn gpt_oss_harmony_reasoning_response_can_suppress_reasoning()
    -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_gpt_oss_harmony_text(
            "<|channel|>analysis<|message|>thinking<|end|><|start|>assistant<|channel|>final<|message|>323",
            GptOssHarmonyParseOptions {
                role_hint: Some(PromptMessageRole::Assistant),
                strict: true,
            },
        )?;

        let suppressed: ParsedReasoningResponse = parsed.reasoning_response().suppress_reasoning();
        assert_eq!(suppressed.reasoning_content, None);
        assert_eq!(suppressed.final_content.as_deref(), Some("323"));
        assert!(
            suppressed
                .parts
                .iter()
                .all(|part| part.kind != ReasoningResponsePartKind::Reasoning)
        );
        Ok(())
    }

    #[test]
    fn reasoning_parser_registry_tracks_decoder_family() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            reasoning_parser_for_decoder_family(GgufDecoderFamily::GptOss),
            Some(ReasoningParser::GptOssHarmony)
        );
        assert_eq!(
            reasoning_parser_for_decoder_family(GgufDecoderFamily::Llama),
            None
        );

        let parsed = parse_reasoning_response_text_for_decoder_family(
            GgufDecoderFamily::GptOss,
            "<|channel|>analysis<|message|>thinking<|end|><|start|>assistant<|channel|>final<|message|>323",
            GptOssHarmonyParseOptions {
                role_hint: Some(PromptMessageRole::Assistant),
                strict: true,
            },
        )?
        .expect("gpt-oss family should parse");
        assert_eq!(parsed.final_content.as_deref(), Some("323"));
        assert_eq!(parsed.reasoning_content.as_deref(), Some("thinking"));

        let unsupported = parse_reasoning_response_text_for_decoder_family(
            GgufDecoderFamily::Llama,
            "world",
            GptOssHarmonyParseOptions::default(),
        )?;
        assert_eq!(unsupported, None);
        Ok(())
    }

    #[test]
    fn gpt_oss_harmony_stream_parser_tracks_partial_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let text = "<|channel|>analysis<|message|>thinking<|end|><|start|>assistant<|channel|>final<|message|>323";
        let tokens = openai_harmony::load_harmony_encoding(
            openai_harmony::HarmonyEncodingName::HarmonyGptOss,
        )?
        .tokenizer()
        .encode_with_special_tokens(text);
        let mut parser = GptOssHarmonyStreamParser::new(GptOssHarmonyParseOptions {
            role_hint: Some(PromptMessageRole::Assistant),
            strict: true,
        })?;

        for token in tokens {
            parser.process_token(TokenId(token))?;
        }
        parser.process_eos()?;

        assert_eq!(parser.current_role(), None);
        assert_eq!(
            parser.messages(),
            vec![
                PromptMessage::new(PromptMessageRole::Assistant, "thinking")
                    .with_channel("analysis"),
                PromptMessage::new(PromptMessageRole::Assistant, "323").with_channel("final"),
            ]
        );
        Ok(())
    }

    #[test]
    fn gguf_prompt_template_renderer_rejects_unsupported_template_digest() {
        let renderer = GgufPromptTemplateRenderer::new(
            command_r_prompt_tokenizer_metadata(),
            super::GgufChatTemplateMetadata::new(
                Some(String::from("{{ prompt }}")),
                BTreeMap::new(),
            ),
        );

        let error = renderer
            .render(
                None,
                &[PromptMessage::new(PromptMessageRole::User, "hello")],
                true,
            )
            .expect_err("unknown template digest should be rejected");

        assert!(matches!(
            error,
            super::PromptRenderError::UnsupportedTemplateDigest { .. }
        ));
    }

    #[test]
    fn apply_special_token_defaults_handles_bos_and_eos_paths() {
        let with_both = build_test_tokenizer_metadata(true, true);
        let without_both = build_test_tokenizer_metadata(false, false);

        let tokens = [TokenId(4), TokenId(5)];
        assert_eq!(
            apply_special_token_defaults(&with_both, &tokens).as_slice(),
            &[TokenId(1), TokenId(4), TokenId(5), TokenId(2)]
        );
        assert_eq!(
            apply_special_token_defaults(&without_both, &tokens).as_slice(),
            &[TokenId(4), TokenId(5)]
        );
    }

    #[test]
    fn tokenizer_fixture_assertion_helper_reports_expected_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = golden_tokenizer_fixture("llama_spm").expect("llama fixture");
        let metadata = GgufTokenizerMetadata {
            model: GgufTokenizerModel::SentencePiece,
            vocabulary: super::GgufTokenizerVocabulary {
                tokens: vec![
                    String::from("<unk>"),
                    String::from("<s>"),
                    String::from("</s>"),
                    String::from("<0x00>"),
                    String::from("<0x01>"),
                    String::from("<0x02>"),
                ],
                bos_token_id: Some(TokenId(1)),
                eos_token_ids: vec![TokenId(2)],
                pad_token_id: None,
                unknown_token_id: Some(TokenId(0)),
            },
            scores: Vec::new(),
            token_types: Vec::new(),
            merges: Vec::new(),
            add_bos: true,
            add_eos: false,
            pretokenizer: None,
            token_type_count: None,
            digest: String::from("fixture"),
        };

        let error = assert_tokenizer_fixture_matches(fixture, &metadata)
            .expect_err("full fixture should reject truncated vocabulary");
        assert!(error.contains("vocabulary length mismatch"));
        Ok(())
    }

    #[test]
    fn gguf_weight_bundle_loader_loads_dense_half_and_quantized_tensors()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("bundle.gguf");
        let q8_bytes = std::iter::once(0x00)
            .chain(std::iter::once(0x40))
            .chain((1_i8..=32).map(|value| value.to_le_bytes()[0]))
            .collect::<Vec<_>>();
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[
                (
                    String::from("general.architecture"),
                    GgufMetadataValue::String(String::from("llama")),
                ),
                (
                    String::from("general.alignment"),
                    GgufMetadataValue::U32(32),
                ),
            ],
            &[
                TestGgufTensor::new(
                    "dense_f32",
                    vec![2],
                    GgufTensorType::F32,
                    super::encode_f32_bytes(&[1.0, -2.0]),
                ),
                TestGgufTensor::new(
                    "dense_f16",
                    vec![2],
                    GgufTensorType::F16,
                    vec![0x00, 0x3c, 0x00, 0x38],
                ),
                TestGgufTensor::new("quantized", vec![32], GgufTensorType::Q8_0, q8_bytes),
            ],
        )?;

        let bundle = GgufWeightBundleLoader.load_path(&path)?;
        assert_eq!(bundle.metadata().format, WeightFormat::Gguf);
        assert_eq!(bundle.metadata().source, WeightSource::ExternalArtifact);
        assert_eq!(bundle.metadata().quantization, QuantizationMode::GgmlQ8_0);
        assert_eq!(
            bundle.metadata().quantization_modes,
            vec![QuantizationMode::GgmlQ8_0]
        );
        assert_eq!(bundle.metadata().artifacts[0].name, "bundle.gguf");

        let dense_f32 = bundle.tensor("dense_f32").ok_or("missing dense_f32")?;
        assert_eq!(dense_f32.metadata().dtype, DType::F32);
        assert_eq!(dense_f32.values()?.as_ref(), &[1.0, -2.0]);

        let dense_f16 = bundle.tensor("dense_f16").ok_or("missing dense_f16")?;
        assert_eq!(dense_f16.metadata().dtype, DType::F16);
        assert_eq!(dense_f16.values()?.as_ref(), &[1.0, 0.5]);

        let quantized = bundle.tensor("quantized").ok_or("missing quantized")?;
        assert!(matches!(
            quantized.storage(),
            WeightTensorStorage::QuantizedBlocks(_)
        ));
        let expected = (1..=32).map(|value| value as f32 * 2.0).collect::<Vec<_>>();
        assert_eq!(quantized.values()?.as_ref(), expected.as_slice());
        Ok(())
    }

    #[test]
    fn gguf_weight_bundle_loader_rejects_unsupported_tensor_types()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("unsupported.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[],
            &[TestGgufTensor::new(
                "unsupported",
                vec![32],
                GgufTensorType::Q6K,
                Vec::new(),
            )],
        )?;

        let error = GgufWeightBundleLoader
            .load_path(&path)
            .expect_err("q6_k should remain unsupported in PSI-110");
        assert!(matches!(
            error,
            super::ModelLoadError::UnsupportedGgufTensorType {
                name,
                tensor_type: GgufTensorType::Q6K
            } if name == "unsupported"
        ));
        Ok(())
    }

    #[test]
    fn gguf_blob_artifact_pages_tensor_bytes_and_reports_buffered_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("paged.gguf");
        let dense_bytes = super::encode_f32_bytes(&[1.0, 2.0, 3.0, 4.0]);
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[(
                String::from("general.alignment"),
                GgufMetadataValue::U32(32),
            )],
            &[TestGgufTensor::new(
                "dense",
                vec![4],
                GgufTensorType::F32,
                dense_bytes.clone(),
            )],
        )?;

        let artifact = GgufBlobArtifact::open_path(
            &path,
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::PreferBuffered)
                .with_page_size(8),
        )?;

        let storage = artifact
            .artifact_metadata()
            .storage
            .as_ref()
            .ok_or("missing artifact storage metadata")?;
        assert_eq!(storage.blob_kind, WeightArtifactBlobKind::GgufFile);
        assert_eq!(storage.read_path, WeightArtifactReadPath::Buffered);
        assert_eq!(storage.page_size, 8);

        let paged = artifact.paged_tensor("dense")?;
        assert_eq!(paged.metadata().shape, Shape::new(vec![4]));
        assert_eq!(paged.blob_offset() % 32, 0);
        assert_eq!(paged.byte_length(), dense_bytes.len());
        assert_eq!(paged.page_count(), 2);
        assert_eq!(paged.page(0)?, &dense_bytes[..8]);
        assert_eq!(paged.page(1)?, &dense_bytes[8..]);
        assert_eq!(paged.read_range(4, 8)?, &dense_bytes[4..12]);
        assert_eq!(paged.read_range(4, 8)?, &dense_bytes[4..12]);

        let loaded = paged.load()?;
        assert_eq!(loaded.values()?.as_ref(), &[1.0, 2.0, 3.0, 4.0]);
        Ok(())
    }

    #[test]
    fn gguf_blob_artifact_supports_memory_mapped_open_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("mapped.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[],
            &[TestGgufTensor::new(
                "dense",
                vec![1],
                GgufTensorType::F32,
                super::encode_f32_bytes(&[7.0]),
            )],
        )?;

        let artifact = GgufBlobArtifact::open_path(
            &path,
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::RequireMemoryMap),
        )?;
        let storage = artifact
            .artifact_metadata()
            .storage
            .as_ref()
            .ok_or("missing artifact storage metadata")?;

        assert_eq!(storage.read_path, WeightArtifactReadPath::MemoryMapped);
        assert_eq!(artifact.load_tensor("dense")?.values()?.as_ref(), &[7.0]);
        Ok(())
    }

    #[test]
    fn gguf_weight_bundle_loader_loads_ollama_blob_with_storage_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bytes = build_test_gguf(
            GgufVersion::V3,
            &[],
            &[TestGgufTensor::new(
                "dense",
                vec![2],
                GgufTensorType::F32,
                super::encode_f32_bytes(&[3.0, -1.0]),
            )],
        )?;
        let digest = hex::encode(Sha256::digest(bytes.as_slice()));
        let blob_path = temp.path().join("blobs").join(format!("sha256-{digest}"));
        std::fs::create_dir_all(blob_path.parent().ok_or("missing parent")?)?;
        std::fs::write(&blob_path, &bytes)?;

        let bundle = GgufWeightBundleLoader.load_ollama_blob(
            temp.path(),
            &format!("sha256:{digest}"),
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::PreferBuffered)
                .with_page_size(16),
        )?;

        let artifact = &bundle.metadata().artifacts[0];
        let storage = artifact
            .storage
            .as_ref()
            .ok_or("missing artifact storage metadata")?;
        assert_eq!(artifact.sha256, digest);
        assert_eq!(storage.blob_kind, WeightArtifactBlobKind::OllamaBlob);
        assert_eq!(storage.read_path, WeightArtifactReadPath::Buffered);
        assert_eq!(storage.page_size, 16);
        assert_eq!(
            bundle
                .tensor("dense")
                .ok_or("missing dense")?
                .values()?
                .as_ref(),
            &[3.0, -1.0]
        );
        Ok(())
    }

    #[test]
    fn gguf_weight_bundle_loader_loads_from_resolved_ollama_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bytes = build_test_gguf(
            GgufVersion::V3,
            &[],
            &[TestGgufTensor::new(
                "dense",
                vec![2],
                GgufTensorType::F32,
                super::encode_f32_bytes(&[4.0, -2.0]),
            )],
        )?;
        let digest = hex::encode(Sha256::digest(bytes.as_slice()));
        let blob_path = temp.path().join("blobs").join(format!("sha256-{digest}"));
        std::fs::create_dir_all(blob_path.parent().ok_or("missing parent")?)?;
        std::fs::write(&blob_path, &bytes)?;

        let manifest_path = temp
            .path()
            .join("manifests/registry.ollama.ai/library/qwen2/latest");
        std::fs::create_dir_all(manifest_path.parent().ok_or("missing parent")?)?;
        std::fs::write(
            &manifest_path,
            format!(
                r#"{{"schemaVersion":2,"mediaType":"application/vnd.docker.distribution.manifest.v2+json","layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{digest}","size":{}}}]}}"#,
                bytes.len()
            ),
        )?;

        let manifest = OllamaModelCatalog::new(temp.path()).resolve_model("qwen2")?;
        let bundle = GgufWeightBundleLoader.load_ollama_manifest(
            &manifest,
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::PreferBuffered)
                .with_page_size(16),
        )?;

        let artifact = &bundle.metadata().artifacts[0];
        let storage = artifact
            .storage
            .as_ref()
            .ok_or("missing artifact storage metadata")?;
        assert_eq!(artifact.sha256, digest);
        assert_eq!(storage.blob_kind, WeightArtifactBlobKind::OllamaBlob);
        assert_eq!(storage.read_path, WeightArtifactReadPath::Buffered);
        assert_eq!(
            bundle
                .tensor("dense")
                .ok_or("missing dense")?
                .values()?
                .as_ref(),
            &[4.0, -2.0]
        );
        Ok(())
    }

    #[test]
    fn gguf_weight_bundle_loader_rejects_corrupt_primary_model_blob_from_ollama_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bytes = build_test_gguf(
            GgufVersion::V3,
            &[],
            &[TestGgufTensor::new(
                "dense",
                vec![2],
                GgufTensorType::F32,
                super::encode_f32_bytes(&[4.0, -2.0]),
            )],
        )?;
        let digest = hex::encode(Sha256::digest(bytes.as_slice()));
        let blob_path = temp.path().join("blobs").join(format!("sha256-{digest}"));
        std::fs::create_dir_all(blob_path.parent().ok_or("missing parent")?)?;
        std::fs::write(&blob_path, b"corrupt-gguf")?;

        let manifest_path = temp
            .path()
            .join("manifests/registry.ollama.ai/library/qwen2/latest");
        std::fs::create_dir_all(manifest_path.parent().ok_or("missing parent")?)?;
        std::fs::write(
            &manifest_path,
            format!(
                r#"{{"schemaVersion":2,"mediaType":"application/vnd.docker.distribution.manifest.v2+json","layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{digest}","size":{}}}]}}"#,
                bytes.len()
            ),
        )?;

        let manifest = OllamaModelCatalog::new(temp.path()).resolve_model("qwen2")?;
        let error = GgufWeightBundleLoader
            .load_ollama_manifest(
                &manifest,
                LocalBlobOpenOptions::default()
                    .with_read_preference(BlobReadPreference::PreferBuffered)
                    .with_page_size(16),
            )
            .expect_err("corrupt primary blob should fail integrity verification");

        assert!(matches!(
            error,
            super::ModelLoadError::ArtifactRead { ref message, .. }
                if message.contains("integrity verification failed")
                    && message.contains("blob digest mismatch")
        ));
        Ok(())
    }

    #[test]
    fn gguf_weight_bundle_loader_refuses_adapter_bearing_ollama_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bytes = build_test_gguf(
            GgufVersion::V3,
            &[],
            &[TestGgufTensor::new(
                "dense",
                vec![2],
                GgufTensorType::F32,
                super::encode_f32_bytes(&[1.0, 2.0]),
            )],
        )?;
        let digest = hex::encode(Sha256::digest(bytes.as_slice()));
        let blob_path = temp.path().join("blobs").join(format!("sha256-{digest}"));
        std::fs::create_dir_all(blob_path.parent().ok_or("missing parent")?)?;
        std::fs::write(&blob_path, &bytes)?;

        let adapter_digest = hex::encode(Sha256::digest(b"adapter-gguf"));
        let adapter_path = temp
            .path()
            .join("blobs")
            .join(format!("sha256-{adapter_digest}"));
        std::fs::write(&adapter_path, b"adapter-gguf")?;

        let manifest_path = temp
            .path()
            .join("manifests/registry.ollama.ai/library/qwen2-adapter/latest");
        std::fs::create_dir_all(manifest_path.parent().ok_or("missing parent")?)?;
        std::fs::write(
            &manifest_path,
            format!(
                r#"{{"schemaVersion":2,"mediaType":"application/vnd.docker.distribution.manifest.v2+json","layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{digest}","size":{}}},{{"mediaType":"application/vnd.ollama.image.adapter","digest":"sha256:{adapter_digest}","size":12}}]}}"#,
                bytes.len()
            ),
        )?;

        let manifest = OllamaModelCatalog::new(temp.path()).resolve_model("qwen2-adapter")?;
        let error = GgufWeightBundleLoader
            .load_ollama_manifest(
                &manifest,
                LocalBlobOpenOptions::default()
                    .with_read_preference(BlobReadPreference::PreferBuffered)
                    .with_page_size(16),
            )
            .expect_err("adapter-bearing manifest should be refused explicitly");

        assert!(matches!(
            error,
            super::ModelLoadError::UnsupportedOllamaAdapterPolicy {
                ref model,
                adapter_layers,
                policy: psionic_catalog::OllamaAdapterPolicy::RefuseManifestWithAdapters,
            } if model == "registry.ollama.ai/library/qwen2-adapter:latest" && adapter_layers == 1
        ));
        Ok(())
    }

    #[test]
    fn gguf_blob_artifact_reports_missing_and_corrupt_blob_failures()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let missing = temp.path().join("missing.gguf");
        let error = GgufBlobArtifact::open_path(
            &missing,
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::PreferBuffered),
        )
        .expect_err("missing blob should fail");
        assert!(matches!(
            error,
            super::ModelLoadError::Blob(psionic_catalog::BlobError::MissingFile { .. })
        ));

        let corrupt = temp.path().join("corrupt.gguf");
        std::fs::write(&corrupt, b"not-a-gguf")?;
        let error = GgufBlobArtifact::open_path(
            &corrupt,
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::PreferBuffered),
        )
        .expect_err("corrupt blob should fail");
        assert!(matches!(
            error,
            super::ModelLoadError::ArtifactFormat { format, .. } if format == "gguf"
        ));
        Ok(())
    }

    #[test]
    fn gguf_decoder_adapter_loader_maps_llama_family_and_layout()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("tiny_llama.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &llama_decoder_metadata("Tiny Llama", None),
            &decoder_family_tensors(2, false, true),
        )?;

        let adapter = GgufDecoderAdapterLoader.load_path(&path)?;
        assert_eq!(adapter.descriptor().model.family, "llama");
        assert_eq!(adapter.descriptor().config.hidden_size, 4);
        assert_eq!(adapter.descriptor().config.layer_count, 2);
        assert_eq!(adapter.descriptor().config.vocab_size, 6);
        assert_eq!(adapter.descriptor().config.max_context, 64);
        assert_eq!(adapter.descriptor().config.block.attention.head_count, 2);
        assert_eq!(adapter.descriptor().config.block.attention.kv_head_count, 1);
        assert_eq!(adapter.descriptor().config.block.attention.head_dim, 2);
        assert_eq!(adapter.descriptor().config.block.attention.rotary_dim, 2);
        assert_eq!(
            adapter.descriptor().config.block.feed_forward.activation,
            ActivationFunction::Silu
        );
        assert_eq!(adapter.descriptor().tokenizer_family, "sentencepiece");
        assert_eq!(adapter.family_metadata().family, GgufDecoderFamily::Llama);
        assert_eq!(adapter.family_metadata().architecture, "llama");
        assert_eq!(
            adapter.family_metadata().display_name.as_deref(),
            Some("Tiny Llama")
        );
        assert_eq!(adapter.family_metadata().rope_theta, 10000.0);
        assert_eq!(adapter.family_metadata().rms_norm_epsilon, 1e-5);
        assert_eq!(adapter.family_metadata().sliding_window, None);
        assert!(!adapter.family_metadata().tie_word_embeddings);
        assert!(!adapter.family_metadata().attention_qkv_biases);
        assert_eq!(
            adapter.tensor_layout().output.as_deref(),
            Some("output.weight")
        );
        assert_eq!(adapter.tensor_layout().layers.len(), 2);
        assert_eq!(
            adapter.tensor_layout().layers[0].attention_query_weight,
            "blk.0.attn_q.weight"
        );
        assert!(
            adapter.tensor_layout().layers[0]
                .attention_query_bias
                .is_none()
        );
        assert!(adapter.chat_templates().is_empty());
        let governance = adapter
            .descriptor()
            .artifact_governance
            .as_ref()
            .expect("local path governance");
        assert_eq!(
            governance.provenance.kind,
            super::ModelArtifactProvenanceKind::LocalPath
        );
        assert_eq!(governance.provenance.source, path.display().to_string());
        assert!(!governance.licenses.declared);
        let boundary = adapter.descriptor().interop_boundary();
        assert_eq!(boundary.catalog_surface, None);
        assert_eq!(
            boundary.ingress_surface,
            super::ModelIngressSurface::DirectArtifactImport
        );
        assert_eq!(
            boundary.serving_surface,
            super::ModelServingSurface::PsionicNative
        );
        assert_eq!(
            boundary.runtime_surface,
            super::ModelRuntimeSurface::PsionicNative
        );
        Ok(())
    }

    #[test]
    fn gguf_decoder_adapter_loader_maps_qwen_family_with_biases_and_tied_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("tiny_qwen2.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &qwen2_decoder_metadata("Tiny Qwen2", Some("{{ prompt }}")),
            &decoder_family_tensors(1, true, false),
        )?;

        let adapter = GgufDecoderAdapterLoader.load_path(&path)?;
        assert_eq!(adapter.descriptor().model.family, "qwen");
        assert_eq!(adapter.descriptor().tokenizer_family, "gpt2_bpe:qwen2");
        assert_eq!(adapter.family_metadata().family, GgufDecoderFamily::Qwen);
        assert_eq!(adapter.family_metadata().architecture, "qwen2");
        assert_eq!(adapter.family_metadata().sliding_window, Some(32));
        assert!(adapter.family_metadata().tie_word_embeddings);
        assert!(adapter.family_metadata().attention_qkv_biases);
        assert!(adapter.tensor_layout().output.is_none());
        assert_eq!(
            adapter.tensor_layout().layers[0]
                .attention_query_bias
                .as_deref(),
            Some("blk.0.attn_q.bias")
        );
        assert_eq!(
            adapter.chat_templates().default_template(),
            Some("{{ prompt }}")
        );
        Ok(())
    }

    #[test]
    fn gguf_decoder_adapter_loader_classifies_sliding_window_llama_as_mistral()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("tiny_mistral.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &llama_decoder_metadata("Tiny Mistral", Some(16)),
            &decoder_family_tensors(2, false, true),
        )?;

        let adapter = GgufDecoderAdapterLoader.load_path(&path)?;
        assert_eq!(adapter.descriptor().model.family, "mistral");
        assert_eq!(adapter.family_metadata().family, GgufDecoderFamily::Mistral);
        assert_eq!(adapter.family_metadata().architecture, "llama");
        assert_eq!(adapter.family_metadata().sliding_window, Some(16));
        Ok(())
    }

    #[test]
    fn gguf_decoder_adapter_loader_rejects_moe_llama_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("moe_llama.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &[
                (
                    String::from("general.architecture"),
                    GgufMetadataValue::String(String::from("llama")),
                ),
                (
                    String::from("llama.expert_count"),
                    GgufMetadataValue::U32(8),
                ),
            ],
            &[],
        )?;

        let error = GgufDecoderAdapterLoader
            .load_path(&path)
            .expect_err("moe llama should remain unsupported");
        assert!(matches!(
            error,
            super::ModelLoadError::UnsupportedGgufDecoderFamilyFeature { family, feature }
                if family == "llama" && feature == "mixture_of_experts"
        ));
        Ok(())
    }

    #[test]
    fn gguf_decoder_adapter_loader_maps_gpt_oss_family_and_layout()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("tiny_gpt_oss.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &gpt_oss_decoder_metadata("Tiny GPT-OSS", Some("{{ prompt }}"), 64),
            &gpt_oss_decoder_tensors(64),
        )?;

        let adapter = GgufDecoderAdapterLoader.load_path(&path)?;
        assert_eq!(adapter.descriptor().model.family, "gpt_oss");
        assert_eq!(
            adapter.descriptor().tokenizer_family,
            "gpt2_bpe:custom:gpt-4o"
        );
        assert_eq!(adapter.family_metadata().family, GgufDecoderFamily::GptOss);
        assert_eq!(adapter.family_metadata().architecture, "gpt-oss");
        assert_eq!(adapter.family_metadata().attention_key_length, Some(16));
        assert_eq!(adapter.family_metadata().attention_value_length, Some(16));
        assert_eq!(adapter.family_metadata().expert_count, Some(3));
        assert_eq!(adapter.family_metadata().expert_used_count, Some(2));
        assert_eq!(
            adapter.family_metadata().expert_feed_forward_length,
            Some(64)
        );
        assert_eq!(adapter.family_metadata().rope_scaling_factor, Some(32.0));
        assert_eq!(
            adapter.family_metadata().rope_original_context_length,
            Some(4096)
        );
        assert!(adapter.family_metadata().attention_qkv_biases);
        assert!(!adapter.family_metadata().tie_word_embeddings);
        assert_eq!(adapter.descriptor().config.hidden_size, 32);
        assert_eq!(adapter.descriptor().config.layer_count, 1);
        assert_eq!(adapter.descriptor().config.block.attention.head_count, 4);
        assert_eq!(adapter.descriptor().config.block.attention.kv_head_count, 1);
        assert_eq!(adapter.descriptor().config.block.attention.head_dim, 16);
        assert_eq!(
            adapter
                .descriptor()
                .config
                .block
                .feed_forward
                .intermediate_size,
            64
        );
        assert_eq!(
            adapter.descriptor().weights.quantization,
            QuantizationMode::GgmlMxfp4
        );
        assert_eq!(
            adapter.descriptor().weights.quantization_modes,
            vec![QuantizationMode::GgmlQ8_0, QuantizationMode::GgmlMxfp4]
        );
        assert_eq!(
            adapter.chat_templates().default_template(),
            Some("{{ prompt }}")
        );

        let layer = &adapter.tensor_layout().layers[0];
        assert_eq!(
            layer.attention_post_norm.as_deref(),
            Some("blk.0.post_attention_norm.weight")
        );
        assert_eq!(
            layer.attention_sinks_weight.as_deref(),
            Some("blk.0.attn_sinks.weight")
        );
        assert_eq!(
            layer.feed_forward_router_weight.as_deref(),
            Some("blk.0.ffn_gate_inp.weight")
        );
        assert_eq!(
            layer.feed_forward_gate_experts_weight.as_deref(),
            Some("blk.0.ffn_gate_exps.weight")
        );
        assert_eq!(
            layer.feed_forward_up_experts_weight.as_deref(),
            Some("blk.0.ffn_up_exps.weight")
        );
        assert_eq!(
            layer.feed_forward_down_experts_weight.as_deref(),
            Some("blk.0.ffn_down_exps.weight")
        );
        assert!(layer.feed_forward_gate_weight.is_none());
        assert!(layer.feed_forward_up_weight.is_none());
        assert!(layer.feed_forward_down_weight.is_none());
        Ok(())
    }

    #[test]
    fn gguf_decoder_adapter_loader_loads_ollama_manifest_with_governance()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let model_path = temp.path().join("tiny_qwen2.gguf");
        write_test_gguf(
            &model_path,
            GgufVersion::V3,
            &qwen2_decoder_metadata("Tiny Qwen2", Some("{{ prompt }}")),
            &decoder_family_tensors(1, true, false),
        )?;
        let model_bytes = std::fs::read(&model_path)?;
        let model_digest = hex::encode(Sha256::digest(model_bytes.as_slice()));
        let blob_path = temp
            .path()
            .join("blobs")
            .join(format!("sha256-{model_digest}"));
        std::fs::create_dir_all(blob_path.parent().ok_or("missing parent")?)?;
        std::fs::write(&blob_path, &model_bytes)?;

        let config_bytes = br#"{
            "model_format":"gguf",
            "model_family":"qwen2",
            "remote_host":"cloud.example",
            "remote_model":"team/qwen2-licensed",
            "base_name":"qwen2-base"
        }"#;
        let config_digest = hex::encode(Sha256::digest(config_bytes));
        let config_path = temp
            .path()
            .join("blobs")
            .join(format!("sha256-{config_digest}"));
        std::fs::write(&config_path, config_bytes)?;

        let license_bytes = b"Apache-2.0";
        let license_digest = hex::encode(Sha256::digest(license_bytes));
        let license_path = temp
            .path()
            .join("blobs")
            .join(format!("sha256-{license_digest}"));
        std::fs::write(&license_path, license_bytes)?;

        let manifest_path = temp
            .path()
            .join("manifests/registry.ollama.ai/library/qwen2/latest");
        std::fs::create_dir_all(manifest_path.parent().ok_or("missing parent")?)?;
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "config": {
                    "mediaType": "application/vnd.docker.container.image.v1+json",
                    "digest": format!("sha256:{config_digest}"),
                    "size": config_bytes.len()
                },
                "layers": [
                    {
                        "mediaType": "application/vnd.ollama.image.model",
                        "digest": format!("sha256:{model_digest}"),
                        "size": model_bytes.len()
                    },
                    {
                        "mediaType": "application/vnd.ollama.image.license",
                        "digest": format!("sha256:{license_digest}"),
                        "size": license_bytes.len()
                    }
                ]
            }))?,
        )?;

        let manifest = OllamaModelCatalog::new(temp.path()).resolve_model("qwen2")?;
        let adapter = GgufDecoderAdapterLoader.load_ollama_manifest(
            &manifest,
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::PreferBuffered),
        )?;

        let governance = adapter
            .descriptor()
            .artifact_governance
            .as_ref()
            .expect("manifest governance");
        assert_eq!(
            governance.provenance.kind,
            super::ModelArtifactProvenanceKind::OllamaRemoteAlias
        );
        assert_eq!(
            governance.provenance.source,
            "registry.ollama.ai/library/qwen2:latest"
        );
        assert_eq!(
            governance.provenance.remote_host.as_deref(),
            Some("cloud.example")
        );
        assert_eq!(
            governance.provenance.remote_model.as_deref(),
            Some("team/qwen2-licensed")
        );
        assert_eq!(governance.licenses.digests(), vec![license_digest]);
        let boundary = adapter.descriptor().interop_boundary();
        assert_eq!(
            boundary.catalog_surface,
            Some(OllamaCatalogSurface::OllamaCompatMigration)
        );
        assert_eq!(
            boundary.ingress_surface,
            super::ModelIngressSurface::OllamaCompatManifestImport
        );
        assert_eq!(
            boundary.serving_surface,
            super::ModelServingSurface::PsionicNative
        );
        assert_eq!(
            boundary.runtime_surface,
            super::ModelRuntimeSurface::PsionicNative
        );
        Ok(())
    }

    #[test]
    fn gguf_embedding_adapter_loader_maps_bert_family_and_layout()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("bert_embed.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &bert_embedding_metadata("bert-bge"),
            &bert_embedding_tensors(2),
        )?;

        let adapter = GgufEmbeddingAdapterLoader.load_path(&path)?;
        assert_eq!(adapter.descriptor().model.family, "bert");
        assert_eq!(adapter.descriptor().dimensions, 4);
        assert_eq!(
            adapter.descriptor().normalization,
            super::EmbeddingNormalization::UnitLength
        );
        assert_eq!(adapter.family_metadata().family, GgufEmbeddingFamily::Bert);
        assert_eq!(adapter.family_metadata().architecture, "bert");
        assert_eq!(adapter.family_metadata().max_context, 64);
        assert_eq!(adapter.family_metadata().layer_count, 2);
        assert_eq!(
            adapter.family_metadata().pooling,
            GgufEmbeddingPooling::Mean
        );
        assert!(adapter.family_metadata().uses_position_embeddings);
        assert!(!adapter.family_metadata().uses_fused_qkv);
        assert_eq!(adapter.tokenizer().model, GgufTokenizerModel::BertWordPiece);
        assert_eq!(adapter.tokenizer().token_type_count, Some(2));
        assert_eq!(
            adapter.tensor_layout().position_embedding.as_deref(),
            Some("position_embd.weight")
        );
        assert_eq!(adapter.tensor_layout().layers.len(), 2);
        assert_eq!(
            adapter.tensor_layout().layers[0]
                .attention_query_weight
                .as_deref(),
            Some("blk.0.attn_q.weight")
        );
        assert_eq!(
            adapter.tensor_layout().layers[0]
                .attention_query_bias
                .as_deref(),
            Some("blk.0.attn_q.bias")
        );
        assert!(
            adapter.tensor_layout().layers[0]
                .attention_qkv_weight
                .is_none()
        );
        assert!(
            adapter.tensor_layout().layers[0]
                .feed_forward_gate_weight
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn gguf_embedding_adapter_loader_maps_nomic_bert_family_and_layout()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("nomic_bert_embed.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &nomic_bert_embedding_metadata("nomic-embed", "nomic-bert"),
            &nomic_bert_embedding_tensors(1),
        )?;

        let adapter = GgufEmbeddingAdapterLoader.load_path(&path)?;
        assert_eq!(adapter.descriptor().model.family, "nomic-bert");
        assert_eq!(
            adapter.descriptor().normalization,
            super::EmbeddingNormalization::None
        );
        assert_eq!(
            adapter.family_metadata().family,
            GgufEmbeddingFamily::NomicBert
        );
        assert_eq!(adapter.family_metadata().architecture, "nomic-bert");
        assert_eq!(adapter.family_metadata().pooling, GgufEmbeddingPooling::Cls);
        assert_eq!(adapter.family_metadata().rope_theta, Some(10_000.0));
        assert!(!adapter.family_metadata().uses_position_embeddings);
        assert!(adapter.family_metadata().uses_fused_qkv);
        assert!(adapter.tensor_layout().position_embedding.is_none());
        assert_eq!(
            adapter.tensor_layout().layers[0]
                .attention_qkv_weight
                .as_deref(),
            Some("blk.0.attn_qkv.weight")
        );
        assert_eq!(
            adapter.tensor_layout().layers[0]
                .attention_qkv_bias
                .as_deref(),
            Some("blk.0.attn_qkv.bias")
        );
        assert!(
            adapter.tensor_layout().layers[0]
                .attention_query_weight
                .is_none()
        );
        assert_eq!(
            adapter.tensor_layout().layers[0]
                .feed_forward_gate_weight
                .as_deref(),
            Some("blk.0.ffn_gate.weight")
        );
        Ok(())
    }

    #[test]
    fn gguf_embedding_adapter_loader_rejects_moe_nomic_bert_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("nomic_bert_moe.gguf");
        write_test_gguf(
            &path,
            GgufVersion::V3,
            &{
                let mut metadata = nomic_bert_embedding_metadata("nomic-moe", "nomic-bert-moe");
                metadata.push((
                    String::from("nomic-bert-moe.expert_count"),
                    GgufMetadataValue::U32(8),
                ));
                metadata.push((
                    String::from("nomic-bert-moe.moe_every_n_layers"),
                    GgufMetadataValue::U32(2),
                ));
                metadata
            },
            &[],
        )?;

        let error = GgufEmbeddingAdapterLoader
            .load_path(&path)
            .expect_err("nomic bert moe should remain unsupported");
        assert!(matches!(
            error,
            super::ModelLoadError::UnsupportedGgufEmbeddingFamilyFeature { family, feature }
                if family == "nomic-bert" && feature == "mixture_of_experts"
        ));
        Ok(())
    }

    fn build_test_tokenizer_metadata(add_bos: bool, add_eos: bool) -> GgufTokenizerMetadata {
        GgufTokenizerMetadata {
            model: GgufTokenizerModel::SentencePiece,
            vocabulary: super::GgufTokenizerVocabulary {
                tokens: vec![
                    String::from("<unk>"),
                    String::from("<s>"),
                    String::from("</s>"),
                    String::from("hello"),
                    String::from("world"),
                    String::from("psionic"),
                ],
                bos_token_id: Some(TokenId(1)),
                eos_token_ids: vec![TokenId(2)],
                pad_token_id: None,
                unknown_token_id: Some(TokenId(0)),
            },
            scores: Vec::new(),
            token_types: Vec::new(),
            merges: Vec::new(),
            add_bos,
            add_eos,
            pretokenizer: None,
            token_type_count: None,
            digest: String::from("fixture"),
        }
    }

    fn phi3_prompt_tokenizer_metadata() -> GgufTokenizerMetadata {
        GgufTokenizerMetadata {
            model: GgufTokenizerModel::SentencePiece,
            vocabulary: super::GgufTokenizerVocabulary {
                tokens: vec![
                    String::from("<unk>"),
                    String::from("<s>"),
                    String::from("<|end|>"),
                    String::from("<|user|>"),
                    String::from("<|assistant|>"),
                ],
                bos_token_id: Some(TokenId(1)),
                eos_token_ids: vec![TokenId(2)],
                pad_token_id: None,
                unknown_token_id: Some(TokenId(0)),
            },
            scores: Vec::new(),
            token_types: Vec::new(),
            merges: Vec::new(),
            add_bos: true,
            add_eos: false,
            pretokenizer: None,
            token_type_count: None,
            digest: String::from("phi3-fixture"),
        }
    }

    fn command_r_prompt_tokenizer_metadata() -> GgufTokenizerMetadata {
        GgufTokenizerMetadata {
            model: GgufTokenizerModel::SentencePiece,
            vocabulary: super::GgufTokenizerVocabulary {
                tokens: vec![
                    String::from("<unk>"),
                    String::from("<BOS_TOKEN>"),
                    String::from("<EOS_TOKEN>"),
                    String::from("<|START_OF_TURN_TOKEN|>"),
                    String::from("<|END_OF_TURN_TOKEN|>"),
                ],
                bos_token_id: Some(TokenId(1)),
                eos_token_ids: vec![TokenId(2)],
                pad_token_id: None,
                unknown_token_id: Some(TokenId(0)),
            },
            scores: Vec::new(),
            token_types: Vec::new(),
            merges: Vec::new(),
            add_bos: true,
            add_eos: false,
            pretokenizer: None,
            token_type_count: None,
            digest: String::from("command-r-fixture"),
        }
    }

    fn prompt_messages_from_fixture(messages: &[super::GoldenPromptMessage]) -> Vec<PromptMessage> {
        messages
            .iter()
            .map(|message| {
                let role = match message.role {
                    super::GoldenPromptRole::System => PromptMessageRole::System,
                    super::GoldenPromptRole::Developer => PromptMessageRole::Developer,
                    super::GoldenPromptRole::User => PromptMessageRole::User,
                    super::GoldenPromptRole::Assistant => PromptMessageRole::Assistant,
                    super::GoldenPromptRole::Tool => PromptMessageRole::Tool,
                };
                PromptMessage::new(role, message.content)
            })
            .collect()
    }

    fn prompt_render_options_from_fixture(
        context: Option<super::GoldenPromptHarmonyContext>,
    ) -> PromptRenderOptions {
        let Some(context) = context else {
            return PromptRenderOptions::default();
        };
        PromptRenderOptions {
            gpt_oss_harmony: Some(super::GptOssHarmonyRenderContext {
                model_identity: None,
                reasoning_effort: context.reasoning_effort,
                tool_namespaces: Vec::new(),
                conversation_start_date: context.conversation_start_date.map(str::to_string),
                knowledge_cutoff: context.knowledge_cutoff.map(str::to_string),
                channel_config: Some(super::PromptChannelConfig::require(
                    context.valid_channels.iter().copied(),
                )),
            }),
        }
    }

    fn llama_decoder_metadata(
        name: &str,
        sliding_window: Option<u32>,
    ) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = vec![
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(String::from("llama")),
            ),
            (
                String::from("general.name"),
                GgufMetadataValue::String(name.to_string()),
            ),
            (
                String::from("llama.context_length"),
                GgufMetadataValue::U32(64),
            ),
            (
                String::from("llama.embedding_length"),
                GgufMetadataValue::U32(4),
            ),
            (
                String::from("llama.feed_forward_length"),
                GgufMetadataValue::U32(8),
            ),
            (String::from("llama.block_count"), GgufMetadataValue::U32(2)),
            (
                String::from("llama.attention.head_count"),
                GgufMetadataValue::U32(2),
            ),
            (
                String::from("llama.attention.head_count_kv"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("llama.attention.layer_norm_rms_epsilon"),
                GgufMetadataValue::F32(1e-5),
            ),
            (
                String::from("llama.rope.dimension_count"),
                GgufMetadataValue::U32(2),
            ),
            (
                String::from("llama.rope.freq_base"),
                GgufMetadataValue::F32(10_000.0),
            ),
        ];
        if let Some(sliding_window) = sliding_window {
            metadata.push((
                String::from("llama.attention.sliding_window"),
                GgufMetadataValue::U32(sliding_window),
            ));
        }
        metadata.extend(sentencepiece_tokenizer_metadata_entries());
        metadata
    }

    fn qwen2_decoder_metadata(
        name: &str,
        chat_template: Option<&str>,
    ) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = vec![
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(String::from("qwen2")),
            ),
            (
                String::from("general.name"),
                GgufMetadataValue::String(name.to_string()),
            ),
            (
                String::from("qwen2.context_length"),
                GgufMetadataValue::U32(128),
            ),
            (
                String::from("qwen2.embedding_length"),
                GgufMetadataValue::U32(4),
            ),
            (
                String::from("qwen2.feed_forward_length"),
                GgufMetadataValue::U32(8),
            ),
            (String::from("qwen2.block_count"), GgufMetadataValue::U32(1)),
            (
                String::from("qwen2.attention.head_count"),
                GgufMetadataValue::U32(2),
            ),
            (
                String::from("qwen2.attention.head_count_kv"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("qwen2.attention.layer_norm_rms_epsilon"),
                GgufMetadataValue::F32(1e-6),
            ),
            (
                String::from("qwen2.rope.freq_base"),
                GgufMetadataValue::F32(1_000_000.0),
            ),
            (
                String::from("qwen2.attention.sliding_window"),
                GgufMetadataValue::U32(32),
            ),
        ];
        metadata.extend(qwen2_tokenizer_metadata_entries());
        if let Some(chat_template) = chat_template {
            metadata.push((
                String::from("tokenizer.chat_template"),
                GgufMetadataValue::String(chat_template.to_string()),
            ));
        }
        metadata
    }

    fn gpt_oss_decoder_metadata(
        name: &str,
        chat_template: Option<&str>,
        expert_feed_forward_length: u32,
    ) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = vec![
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(String::from("gpt-oss")),
            ),
            (
                String::from("general.name"),
                GgufMetadataValue::String(name.to_string()),
            ),
            (
                String::from("gpt-oss.context_length"),
                GgufMetadataValue::U32(128),
            ),
            (
                String::from("gpt-oss.embedding_length"),
                GgufMetadataValue::U32(32),
            ),
            (
                String::from("gpt-oss.feed_forward_length"),
                GgufMetadataValue::U32(32),
            ),
            (
                String::from("gpt-oss.expert_feed_forward_length"),
                GgufMetadataValue::U32(expert_feed_forward_length),
            ),
            (
                String::from("gpt-oss.block_count"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("gpt-oss.attention.head_count"),
                GgufMetadataValue::U32(4),
            ),
            (
                String::from("gpt-oss.attention.head_count_kv"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("gpt-oss.attention.key_length"),
                GgufMetadataValue::U32(16),
            ),
            (
                String::from("gpt-oss.attention.value_length"),
                GgufMetadataValue::U32(16),
            ),
            (
                String::from("gpt-oss.attention.layer_norm_rms_epsilon"),
                GgufMetadataValue::F32(1e-5),
            ),
            (
                String::from("gpt-oss.rope.dimension_count"),
                GgufMetadataValue::U32(16),
            ),
            (
                String::from("gpt-oss.rope.freq_base"),
                GgufMetadataValue::F32(10_000.0),
            ),
            (
                String::from("gpt-oss.rope.scaling.factor"),
                GgufMetadataValue::F32(32.0),
            ),
            (
                String::from("gpt-oss.rope.scaling.original_context_length"),
                GgufMetadataValue::U32(4096),
            ),
            (
                String::from("gpt-oss.expert_count"),
                GgufMetadataValue::U32(3),
            ),
            (
                String::from("gpt-oss.expert_used_count"),
                GgufMetadataValue::U32(2),
            ),
        ];
        metadata.extend(gpt_oss_tokenizer_metadata_entries());
        if let Some(chat_template) = chat_template {
            metadata.push((
                String::from("tokenizer.chat_template"),
                GgufMetadataValue::String(chat_template.to_string()),
            ));
        }
        metadata
    }

    fn sentencepiece_tokenizer_metadata_entries() -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("llama")),
            ),
            (
                String::from("tokenizer.ggml.tokens"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("<unk>")),
                    GgufMetadataValue::String(String::from("<s>")),
                    GgufMetadataValue::String(String::from("</s>")),
                    GgufMetadataValue::String(String::from("hello")),
                    GgufMetadataValue::String(String::from("world")),
                    GgufMetadataValue::String(String::from("psionic")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.bos_token_id"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("tokenizer.ggml.eos_token_id"),
                GgufMetadataValue::U32(2),
            ),
            (
                String::from("tokenizer.ggml.unknown_token_id"),
                GgufMetadataValue::U32(0),
            ),
            (
                String::from("tokenizer.ggml.add_bos_token"),
                GgufMetadataValue::Bool(true),
            ),
            (
                String::from("tokenizer.ggml.add_eos_token"),
                GgufMetadataValue::Bool(false),
            ),
        ]
    }

    fn qwen2_tokenizer_metadata_entries() -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("gpt2")),
            ),
            (
                String::from("tokenizer.ggml.pre"),
                GgufMetadataValue::String(String::from("qwen2")),
            ),
            (
                String::from("tokenizer.ggml.tokens"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("<|bos|>")),
                    GgufMetadataValue::String(String::from("<|eos|>")),
                    GgufMetadataValue::String(String::from("h")),
                    GgufMetadataValue::String(String::from("e")),
                    GgufMetadataValue::String(String::from("l")),
                    GgufMetadataValue::String(String::from("o")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.merges"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("h e")),
                    GgufMetadataValue::String(String::from("he l")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.bos_token_id"),
                GgufMetadataValue::U32(0),
            ),
            (
                String::from("tokenizer.ggml.eos_token_id"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("tokenizer.ggml.add_bos_token"),
                GgufMetadataValue::Bool(false),
            ),
            (
                String::from("tokenizer.ggml.add_eos_token"),
                GgufMetadataValue::Bool(false),
            ),
        ]
    }

    fn gpt_oss_tokenizer_metadata_entries() -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("gpt2")),
            ),
            (
                String::from("tokenizer.ggml.pre"),
                GgufMetadataValue::String(String::from("gpt-4o")),
            ),
            (
                String::from("tokenizer.ggml.tokens"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("<|start|>")),
                    GgufMetadataValue::String(String::from("<|end|>")),
                    GgufMetadataValue::String(String::from("hello")),
                    GgufMetadataValue::String(String::from("world")),
                    GgufMetadataValue::String(String::from("psionic")),
                    GgufMetadataValue::String(String::from("gpt-oss")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.merges"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("hello world")),
                    GgufMetadataValue::String(String::from("psionic gpt-oss")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.bos_token_id"),
                GgufMetadataValue::U32(0),
            ),
            (
                String::from("tokenizer.ggml.eos_token_id"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("tokenizer.ggml.unknown_token_id"),
                GgufMetadataValue::U32(0),
            ),
            (
                String::from("tokenizer.ggml.padding_token_id"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("tokenizer.ggml.add_bos_token"),
                GgufMetadataValue::Bool(false),
            ),
            (
                String::from("tokenizer.ggml.add_eos_token"),
                GgufMetadataValue::Bool(false),
            ),
        ]
    }

    fn bert_wordpiece_tokenizer_metadata_entries(
        token_type_count: u32,
    ) -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("bert")),
            ),
            (
                String::from("tokenizer.ggml.tokens"),
                GgufMetadataValue::Array(vec![
                    GgufMetadataValue::String(String::from("[PAD]")),
                    GgufMetadataValue::String(String::from("[CLS]")),
                    GgufMetadataValue::String(String::from("[SEP]")),
                    GgufMetadataValue::String(String::from("[UNK]")),
                    GgufMetadataValue::String(String::from("hello")),
                    GgufMetadataValue::String(String::from("world")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.padding_token_id"),
                GgufMetadataValue::U32(0),
            ),
            (
                String::from("tokenizer.ggml.bos_token_id"),
                GgufMetadataValue::U32(1),
            ),
            (
                String::from("tokenizer.ggml.eos_token_id"),
                GgufMetadataValue::U32(2),
            ),
            (
                String::from("tokenizer.ggml.unknown_token_id"),
                GgufMetadataValue::U32(3),
            ),
            (
                String::from("tokenizer.ggml.token_type_count"),
                GgufMetadataValue::U32(token_type_count),
            ),
        ]
    }

    fn bert_embedding_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = vec![
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(String::from("bert")),
            ),
            (
                String::from("general.name"),
                GgufMetadataValue::String(name.to_string()),
            ),
            (
                String::from("bert.context_length"),
                GgufMetadataValue::U32(64),
            ),
            (
                String::from("bert.embedding_length"),
                GgufMetadataValue::U32(4),
            ),
            (
                String::from("bert.feed_forward_length"),
                GgufMetadataValue::U32(8),
            ),
            (String::from("bert.block_count"), GgufMetadataValue::U32(2)),
            (
                String::from("bert.attention.head_count"),
                GgufMetadataValue::U32(2),
            ),
            (
                String::from("bert.attention.layer_norm_epsilon"),
                GgufMetadataValue::F32(1e-12),
            ),
            (
                String::from("bert.attention.causal"),
                GgufMetadataValue::Bool(false),
            ),
            (String::from("bert.pooling_type"), GgufMetadataValue::U32(1)),
            (
                String::from("bert.normalize_embeddings"),
                GgufMetadataValue::Bool(true),
            ),
        ];
        metadata.extend(bert_wordpiece_tokenizer_metadata_entries(2));
        metadata
    }

    fn nomic_bert_embedding_metadata(
        name: &str,
        architecture: &str,
    ) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = vec![
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(architecture.to_string()),
            ),
            (
                String::from("general.name"),
                GgufMetadataValue::String(name.to_string()),
            ),
            (
                format!("{architecture}.context_length"),
                GgufMetadataValue::U32(128),
            ),
            (
                format!("{architecture}.embedding_length"),
                GgufMetadataValue::U32(4),
            ),
            (
                format!("{architecture}.feed_forward_length"),
                GgufMetadataValue::U32(8),
            ),
            (
                format!("{architecture}.block_count"),
                GgufMetadataValue::U32(1),
            ),
            (
                format!("{architecture}.attention.head_count"),
                GgufMetadataValue::U32(2),
            ),
            (
                format!("{architecture}.attention.layer_norm_epsilon"),
                GgufMetadataValue::F32(1e-5),
            ),
            (
                format!("{architecture}.attention.causal"),
                GgufMetadataValue::Bool(false),
            ),
            (
                format!("{architecture}.pooling_type"),
                GgufMetadataValue::U32(2),
            ),
            (
                format!("{architecture}.rope.freq_base"),
                GgufMetadataValue::F32(10_000.0),
            ),
        ];
        metadata.extend(bert_wordpiece_tokenizer_metadata_entries(2));
        metadata
    }

    fn decoder_family_tensors(
        layer_count: usize,
        include_qkv_bias: bool,
        include_output: bool,
    ) -> Vec<TestGgufTensor> {
        let mut tensors = vec![
            dense_f32_tensor("token_embd.weight", vec![6, 4]),
            dense_f32_tensor("output_norm.weight", vec![4]),
        ];
        if include_output {
            tensors.push(dense_f32_tensor("output.weight", vec![6, 4]));
        }

        for layer_index in 0..layer_count {
            let prefix = format!("blk.{layer_index}");
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_norm.weight"),
                vec![4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_q.weight"),
                vec![4, 4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_k.weight"),
                vec![2, 4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_v.weight"),
                vec![2, 4],
            ));
            if include_qkv_bias {
                tensors.push(dense_f32_tensor(&format!("{prefix}.attn_q.bias"), vec![4]));
                tensors.push(dense_f32_tensor(&format!("{prefix}.attn_k.bias"), vec![2]));
                tensors.push(dense_f32_tensor(&format!("{prefix}.attn_v.bias"), vec![2]));
            }
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_output.weight"),
                vec![4, 4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_gate.weight"),
                vec![8, 4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_down.weight"),
                vec![4, 8],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_up.weight"),
                vec![8, 4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_norm.weight"),
                vec![4],
            ));
        }

        tensors
    }

    fn gpt_oss_decoder_tensors(expert_width: usize) -> Vec<TestGgufTensor> {
        let expert_blocks = 3 * expert_width;
        vec![
            TestGgufTensor::new(
                "token_embd.weight",
                vec![6, 32],
                GgufTensorType::Q8_0,
                repeated_q8_0_bytes(6),
            ),
            dense_f32_tensor("output_norm.weight", vec![32]),
            TestGgufTensor::new(
                "output.weight",
                vec![6, 32],
                GgufTensorType::Q8_0,
                repeated_q8_0_bytes(6),
            ),
            dense_f32_tensor("blk.0.attn_norm.weight", vec![32]),
            dense_f32_tensor("blk.0.attn_q.weight", vec![64, 32]),
            dense_f32_tensor("blk.0.attn_q.bias", vec![64]),
            dense_f32_tensor("blk.0.attn_k.weight", vec![16, 32]),
            dense_f32_tensor("blk.0.attn_k.bias", vec![16]),
            dense_f32_tensor("blk.0.attn_v.weight", vec![16, 32]),
            dense_f32_tensor("blk.0.attn_v.bias", vec![16]),
            dense_f32_tensor("blk.0.attn_output.weight", vec![32, 64]),
            dense_f32_tensor("blk.0.attn_output.bias", vec![32]),
            dense_f32_tensor("blk.0.post_attention_norm.weight", vec![32]),
            dense_f32_tensor("blk.0.attn_sinks.weight", vec![16]),
            dense_f32_tensor("blk.0.ffn_gate_inp.weight", vec![3, 32]),
            dense_f32_tensor("blk.0.ffn_gate_inp.bias", vec![3]),
            TestGgufTensor::new(
                "blk.0.ffn_gate_exps.weight",
                vec![3, expert_width, 32],
                GgufTensorType::MXFP4,
                repeated_mxfp4_bytes(expert_blocks),
            ),
            dense_f32_tensor("blk.0.ffn_gate_exps.bias", vec![3, expert_width]),
            TestGgufTensor::new(
                "blk.0.ffn_up_exps.weight",
                vec![3, expert_width, 32],
                GgufTensorType::MXFP4,
                repeated_mxfp4_bytes(expert_blocks),
            ),
            dense_f32_tensor("blk.0.ffn_up_exps.bias", vec![3, expert_width]),
            TestGgufTensor::new(
                "blk.0.ffn_down_exps.weight",
                vec![3, 32, expert_width],
                GgufTensorType::MXFP4,
                repeated_mxfp4_bytes(expert_blocks),
            ),
            dense_f32_tensor("blk.0.ffn_down_exps.bias", vec![3, 32]),
        ]
    }

    fn bert_embedding_tensors(layer_count: usize) -> Vec<TestGgufTensor> {
        let mut tensors = vec![
            dense_f32_tensor("token_embd.weight", vec![6, 4]),
            dense_f32_tensor("token_types.weight", vec![2, 4]),
            dense_f32_tensor("token_embd_norm.weight", vec![4]),
            dense_f32_tensor("position_embd.weight", vec![64, 4]),
        ];

        for layer_index in 0..layer_count {
            let prefix = format!("blk.{layer_index}");
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_q.weight"),
                vec![4, 4],
            ));
            tensors.push(dense_f32_tensor(&format!("{prefix}.attn_q.bias"), vec![4]));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_k.weight"),
                vec![4, 4],
            ));
            tensors.push(dense_f32_tensor(&format!("{prefix}.attn_k.bias"), vec![4]));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_v.weight"),
                vec![4, 4],
            ));
            tensors.push(dense_f32_tensor(&format!("{prefix}.attn_v.bias"), vec![4]));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_output.weight"),
                vec![4, 4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_output.bias"),
                vec![4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_output_norm.weight"),
                vec![4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_up.weight"),
                vec![8, 4],
            ));
            tensors.push(dense_f32_tensor(&format!("{prefix}.ffn_up.bias"), vec![8]));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_down.weight"),
                vec![4, 8],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_down.bias"),
                vec![4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.layer_output_norm.weight"),
                vec![4],
            ));
        }

        tensors
    }

    fn nomic_bert_embedding_tensors(layer_count: usize) -> Vec<TestGgufTensor> {
        let mut tensors = vec![
            dense_f32_tensor("token_embd.weight", vec![6, 4]),
            dense_f32_tensor("token_types.weight", vec![2, 4]),
            dense_f32_tensor("token_embd_norm.weight", vec![4]),
        ];

        for layer_index in 0..layer_count {
            let prefix = format!("blk.{layer_index}");
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_qkv.weight"),
                vec![12, 4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_qkv.bias"),
                vec![12],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_output.weight"),
                vec![4, 4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_output.bias"),
                vec![4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.attn_output_norm.weight"),
                vec![4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_gate.weight"),
                vec![8, 4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_gate.bias"),
                vec![8],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_up.weight"),
                vec![8, 4],
            ));
            tensors.push(dense_f32_tensor(&format!("{prefix}.ffn_up.bias"), vec![8]));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_down.weight"),
                vec![4, 8],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.ffn_down.bias"),
                vec![4],
            ));
            tensors.push(dense_f32_tensor(
                &format!("{prefix}.layer_output_norm.weight"),
                vec![4],
            ));
        }

        tensors
    }

    fn dense_f32_tensor(name: &str, shape: Vec<usize>) -> TestGgufTensor {
        let element_count = shape.iter().product::<usize>();
        TestGgufTensor::new(
            name,
            shape,
            GgufTensorType::F32,
            super::encode_f32_bytes(&vec![0.0; element_count]),
        )
    }

    fn repeated_q8_0_bytes(block_count: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(block_count * 34);
        for _ in 0..block_count {
            bytes.extend([0x00, 0x3c]);
            bytes.extend([0_u8; 32]);
        }
        bytes
    }

    fn repeated_mxfp4_bytes(block_count: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(block_count * 17);
        for _ in 0..block_count {
            bytes.push(128_u8);
            bytes.extend([0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]);
            bytes.extend([0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]);
        }
        bytes
    }

    #[test]
    fn ggml_q8_0_block_decode_matches_reference() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = std::iter::once(0x00)
            .chain(std::iter::once(0x40))
            .chain((1_i8..=32).map(|value| value.to_le_bytes()[0]))
            .collect::<Vec<_>>();
        let storage = QuantizedTensorStorage::from_ggml_blocks(
            QuantizationMode::GgmlQ8_0,
            &Shape::new(vec![32]),
            bytes,
        )?;

        let expected = (1..=32).map(|value| value as f32 * 2.0).collect::<Vec<_>>();
        assert_eq!(storage.layout(), QuantizedBlockLayout::new(32, 34, 1));
        assert_eq!(storage.dequantize_values()?, expected);
        Ok(())
    }

    #[test]
    fn ggml_q4_0_block_decode_matches_reference() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = [0x00_u8, 0x40]
            .into_iter()
            .chain(
                [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]
                    .into_iter()
                    .cycle()
                    .take(16),
            )
            .collect::<Vec<_>>();
        let tensor = LoadedWeightTensor::from_ggml_blocks(
            "weight",
            Shape::new(vec![32]),
            QuantizationMode::GgmlQ4_0,
            bytes,
        )?;

        let low_half = [-16.0, -12.0, -8.0, -4.0, 0.0, 4.0, 8.0, 12.0]
            .into_iter()
            .cycle()
            .take(16);
        let high_half = [-14.0, -10.0, -6.0, -2.0, 2.0, 6.0, 10.0, 14.0]
            .into_iter()
            .cycle()
            .take(16);
        let expected = low_half.chain(high_half).collect::<Vec<_>>();
        assert_eq!(
            tensor.metadata().quantized_layout,
            Some(QuantizedBlockLayout::new(32, 18, 1))
        );
        assert!(matches!(
            tensor.storage(),
            WeightTensorStorage::QuantizedBlocks(_)
        ));
        assert_eq!(tensor.values()?.as_ref(), expected.as_slice());
        Ok(())
    }

    #[test]
    fn ggml_q4_1_block_decode_matches_reference() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = [0x00_u8, 0x40, 0x00, 0xbc]
            .into_iter()
            .chain(
                [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]
                    .into_iter()
                    .cycle()
                    .take(16),
            )
            .collect::<Vec<_>>();
        let storage = QuantizedTensorStorage::from_ggml_blocks(
            QuantizationMode::GgmlQ4_1,
            &Shape::new(vec![32]),
            bytes,
        )?;

        let low_half = [-1.0, 3.0, 7.0, 11.0, 15.0, 19.0, 23.0, 27.0]
            .into_iter()
            .cycle()
            .take(16);
        let high_half = [1.0, 5.0, 9.0, 13.0, 17.0, 21.0, 25.0, 29.0]
            .into_iter()
            .cycle()
            .take(16);
        let expected = low_half.chain(high_half).collect::<Vec<_>>();
        assert_eq!(storage.dequantize_values()?, expected);
        Ok(())
    }

    #[test]
    fn ggml_mxfp4_block_decode_matches_reference() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = std::iter::once(128_u8)
            .chain(
                [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]
                    .into_iter()
                    .cycle()
                    .take(16),
            )
            .collect::<Vec<_>>();
        let storage = QuantizedTensorStorage::from_ggml_blocks(
            QuantizationMode::GgmlMxfp4,
            &Shape::new(vec![32]),
            bytes,
        )?;

        let low_half = [0.0, 2.0, 4.0, 8.0, 0.0, -2.0, -4.0, -8.0]
            .into_iter()
            .cycle()
            .take(16);
        let high_half = [1.0, 3.0, 6.0, 12.0, -1.0, -3.0, -6.0, -12.0]
            .into_iter()
            .cycle()
            .take(16);
        let expected = low_half.chain(high_half).collect::<Vec<_>>();
        assert_eq!(storage.dequantize_values()?, expected);
        Ok(())
    }

    #[test]
    fn ggml_quantized_storage_is_digest_stable_across_reloads()
    -> Result<(), Box<dyn std::error::Error>> {
        let bytes = [0x00_u8, 0x40]
            .into_iter()
            .chain((1_i8..=32).map(|value| value.to_le_bytes()[0]))
            .collect::<Vec<_>>();
        let first = QuantizedTensorStorage::from_ggml_blocks(
            QuantizationMode::GgmlQ8_0,
            &Shape::new(vec![32]),
            bytes.clone(),
        )?;
        let second = QuantizedTensorStorage::from_ggml_blocks(
            QuantizationMode::GgmlQ8_0,
            &Shape::new(vec![32]),
            bytes,
        )?;

        assert_eq!(first.layout(), second.layout());
        assert_eq!(first.digest(), second.digest());
        Ok(())
    }

    #[test]
    fn ggml_quantized_storage_rejects_shapes_without_block_aligned_last_dim() {
        let bytes = std::iter::once(0x00)
            .chain(std::iter::once(0x40))
            .chain((1_i8..=32).map(|value| value.to_le_bytes()[0]))
            .collect::<Vec<_>>();

        let error = QuantizedTensorStorage::from_ggml_blocks(
            QuantizationMode::GgmlQ8_0,
            &Shape::new(vec![2, 16]),
            bytes,
        )
        .expect_err("shape should be rejected");

        assert!(matches!(
            error,
            super::ModelLoadError::InvalidQuantizedTensorShape {
                quantization: QuantizationMode::GgmlQ8_0,
                shape
            } if shape == vec![2, 16]
        ));
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
        let governance = model
            .descriptor()
            .artifact_governance
            .as_ref()
            .expect("local path governance");
        assert_eq!(
            governance.provenance.kind,
            super::ModelArtifactProvenanceKind::LocalPath
        );
        assert_eq!(governance.provenance.source, path.display().to_string());
        assert!(!governance.licenses.declared);
        let boundary = model.descriptor().interop_boundary();
        assert_eq!(boundary.catalog_surface, None);
        assert_eq!(
            boundary.ingress_surface,
            super::ModelIngressSurface::DirectArtifactImport
        );
        assert_eq!(
            boundary.serving_surface,
            super::ModelServingSurface::PsionicNative
        );
        assert_eq!(
            boundary.runtime_surface,
            super::ModelRuntimeSurface::PsionicNative
        );
        Ok(())
    }

    #[test]
    fn fixture_decoder_interop_boundary_is_native() {
        let model = ReferenceWordDecoder::new();
        let boundary = model.descriptor().interop_boundary();

        assert_eq!(boundary.catalog_surface, None);
        assert_eq!(
            boundary.ingress_surface,
            super::ModelIngressSurface::Fixture
        );
        assert_eq!(
            boundary.serving_surface,
            super::ModelServingSurface::PsionicNative
        );
        assert_eq!(
            boundary.runtime_surface,
            super::ModelRuntimeSurface::PsionicNative
        );
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
        let governance = model
            .descriptor()
            .artifact_governance
            .as_ref()
            .expect("local path governance");
        assert_eq!(
            governance.provenance.kind,
            super::ModelArtifactProvenanceKind::LocalPath
        );
        assert_eq!(governance.provenance.source, path.display().to_string());
        assert!(!governance.licenses.declared);
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

    #[derive(Clone, Debug)]
    struct TestGgufTensor {
        name: String,
        shape: Vec<usize>,
        tensor_type: GgufTensorType,
        bytes: Vec<u8>,
    }

    impl TestGgufTensor {
        fn new(
            name: impl Into<String>,
            shape: Vec<usize>,
            tensor_type: GgufTensorType,
            bytes: Vec<u8>,
        ) -> Self {
            Self {
                name: name.into(),
                shape,
                tensor_type,
                bytes,
            }
        }
    }

    fn write_test_gguf(
        path: &Path,
        version: GgufVersion,
        metadata: &[(String, GgufMetadataValue)],
        tensors: &[TestGgufTensor],
    ) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(path, build_test_gguf(version, metadata, tensors)?)?;
        Ok(())
    }

    fn build_test_gguf(
        version: GgufVersion,
        metadata: &[(String, GgufMetadataValue)],
        tensors: &[TestGgufTensor],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let alignment = metadata
            .iter()
            .find(|(key, _)| key == "general.alignment")
            .and_then(|(_, value)| value.as_u64())
            .unwrap_or(32);
        let alignment = alignment.max(1);

        let mut bytes = Vec::new();
        bytes.extend(b"GGUF");
        push_u32(
            &mut bytes,
            match version {
                GgufVersion::V1 => 1,
                GgufVersion::V2 => 2,
                GgufVersion::V3 => 3,
            },
        );
        push_count(&mut bytes, version, tensors.len())?;
        push_count(&mut bytes, version, metadata.len())?;

        for (key, value) in metadata {
            push_gguf_string(&mut bytes, version, key)?;
            push_u32(&mut bytes, gguf_metadata_value_type(value));
            push_gguf_value(&mut bytes, version, value)?;
        }

        let mut next_offset = 0_usize;
        let mut tensor_offsets = Vec::with_capacity(tensors.len());
        for tensor in tensors {
            tensor_offsets.push(next_offset);
            next_offset = align_usize(next_offset + tensor.bytes.len(), alignment as usize);
        }

        for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
            push_gguf_string(&mut bytes, version, &tensor.name)?;
            push_u32(
                &mut bytes,
                u32::try_from(tensor.shape.len()).map_err(|_| "tensor rank does not fit in u32")?,
            );
            for dimension in tensor.shape.iter().rev() {
                push_u64(
                    &mut bytes,
                    u64::try_from(*dimension)
                        .map_err(|_| "tensor dimension does not fit in u64")?,
                );
            }
            push_u32(&mut bytes, gguf_tensor_type_code(tensor.tensor_type));
            push_u64(
                &mut bytes,
                u64::try_from(*offset).map_err(|_| "tensor offset does not fit in u64")?,
            );
        }

        let tensor_data_offset = super::align_offset(bytes.len() as u64, alignment);
        bytes.resize(tensor_data_offset as usize, 0);

        for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
            let start = tensor_data_offset as usize + offset;
            if bytes.len() < start {
                bytes.resize(start, 0);
            }
            bytes.extend_from_slice(&tensor.bytes);
            bytes.resize(align_usize(bytes.len(), alignment as usize), 0);
        }

        Ok(bytes)
    }

    fn push_count(
        bytes: &mut Vec<u8>,
        version: GgufVersion,
        value: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match version {
            GgufVersion::V1 => push_u32(bytes, u32::try_from(value)?),
            GgufVersion::V2 | GgufVersion::V3 => push_u64(bytes, u64::try_from(value)?),
        }
        Ok(())
    }

    fn push_gguf_string(
        bytes: &mut Vec<u8>,
        version: GgufVersion,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match version {
            GgufVersion::V1 => push_u32(bytes, u32::try_from(value.len())?),
            GgufVersion::V2 | GgufVersion::V3 => push_u64(bytes, u64::try_from(value.len())?),
        }
        bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn push_gguf_value(
        bytes: &mut Vec<u8>,
        version: GgufVersion,
        value: &GgufMetadataValue,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match value {
            GgufMetadataValue::U8(value) => bytes.push(*value),
            GgufMetadataValue::I8(value) => bytes.push(value.to_le_bytes()[0]),
            GgufMetadataValue::U16(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I16(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::U32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::U64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::I64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::F32(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::F64(value) => bytes.extend(value.to_le_bytes()),
            GgufMetadataValue::Bool(value) => bytes.push(u8::from(*value)),
            GgufMetadataValue::String(value) => push_gguf_string(bytes, version, value)?,
            GgufMetadataValue::Array(values) => {
                let value_type = values.first().map_or(4, gguf_metadata_value_type);
                push_u32(bytes, value_type);
                push_count(bytes, version, values.len())?;
                for value in values {
                    push_gguf_value(bytes, version, value)?;
                }
            }
        }
        Ok(())
    }

    fn gguf_metadata_value_type(value: &GgufMetadataValue) -> u32 {
        match value {
            GgufMetadataValue::U8(_) => 0,
            GgufMetadataValue::I8(_) => 1,
            GgufMetadataValue::U16(_) => 2,
            GgufMetadataValue::I16(_) => 3,
            GgufMetadataValue::U32(_) => 4,
            GgufMetadataValue::I32(_) => 5,
            GgufMetadataValue::F32(_) => 6,
            GgufMetadataValue::Bool(_) => 7,
            GgufMetadataValue::String(_) => 8,
            GgufMetadataValue::Array(_) => 9,
            GgufMetadataValue::U64(_) => 10,
            GgufMetadataValue::I64(_) => 11,
            GgufMetadataValue::F64(_) => 12,
        }
    }

    fn gguf_tensor_type_code(tensor_type: GgufTensorType) -> u32 {
        match tensor_type {
            GgufTensorType::F32 => 0,
            GgufTensorType::F16 => 1,
            GgufTensorType::MXFP4 => 39,
            GgufTensorType::Q4_0 => 2,
            GgufTensorType::Q4_1 => 3,
            GgufTensorType::Q5_0 => 6,
            GgufTensorType::Q5_1 => 7,
            GgufTensorType::Q8_0 => 8,
            GgufTensorType::Q8_1 => 9,
            GgufTensorType::Q2K => 10,
            GgufTensorType::Q3K => 11,
            GgufTensorType::Q4K => 12,
            GgufTensorType::Q5K => 13,
            GgufTensorType::Q6K => 14,
            GgufTensorType::Q8K => 15,
            GgufTensorType::BF16 => 30,
            GgufTensorType::Unknown(value) => value,
        }
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend(value.to_le_bytes());
    }

    fn align_usize(value: usize, alignment: usize) -> usize {
        super::align_offset(value as u64, alignment as u64) as usize
    }
}
