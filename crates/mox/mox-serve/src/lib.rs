//! Served compute product contracts for Mox.

use std::collections::BTreeMap;

use mox_backend_cpu::CpuBackend;
use mox_backend_metal::{EMBEDDINGS_SUPPORTED_OPS, MetalBackend};
use mox_compiler::{CompileError, compile_graph};
pub use mox_core::QuantizationMode;
use mox_core::{DType, Device, Shape, TensorId};
use mox_ir::{Graph, GraphBuilder, GraphError};
pub use mox_models::{
    ActivationFunction, ArtifactWordDecoder, ByteProjectionEmbedder, DecoderAttentionConfig,
    DecoderBlockConfig, DecoderConfig, DecoderFeedForwardConfig, DecoderFixtureWeights,
    DecoderModelDescriptor, DecoderWeightLoader, EmbeddingModelDescriptor, EmbeddingNormalization,
    EmbeddingWeights, FixtureDecoderLoader, FixtureWordTokenizer, ModelDescriptor, ModelLoadError,
    ReferenceWordDecoder, SmokeByteEmbedder, TokenId, TokenSequence, TokenVocabulary,
    TokenizerBoundary, WeightArtifactMetadata, WeightBundleMetadata, WeightFormat, WeightSource,
    WeightTensorMetadata,
};
use mox_runtime::{BackendSelection, DeviceDiscovery, HealthStatus, RuntimeError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "request and response types for served products";

/// Phase-0 embeddings product identifier.
pub const EMBEDDINGS_PRODUCT_ID: &str = "mox.embeddings";

/// Phase-1 text-generation product identifier.
pub const TEXT_GENERATION_PRODUCT_ID: &str = "mox.text_generation";

/// Embeddings request contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Stable client-provided request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Embeddings model descriptor.
    pub model: EmbeddingModelDescriptor,
    /// UTF-8 text inputs to embed.
    pub inputs: Vec<String>,
}

impl EmbeddingRequest {
    /// Creates an embeddings request for the default Mox product.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        model: EmbeddingModelDescriptor,
        inputs: Vec<String>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(EMBEDDINGS_PRODUCT_ID),
            model,
            inputs,
        }
    }
}

/// Individual embeddings vector payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingVector {
    /// Input index in the request.
    pub index: usize,
    /// Embedding values.
    pub values: Vec<f32>,
}

/// Response metadata for embeddings execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingResponseMetadata {
    /// Stable output dimensionality.
    pub dimensions: usize,
    /// Number of returned vectors.
    pub vector_count: usize,
    /// Model identifier used during execution.
    pub model_id: String,
    /// Normalization policy applied by the model.
    pub normalization: EmbeddingNormalization,
}

/// Embeddings response contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Returned embeddings vectors.
    pub embeddings: Vec<EmbeddingVector>,
    /// Metadata describing the outputs.
    pub metadata: EmbeddingResponseMetadata,
}

impl EmbeddingResponse {
    /// Creates an embeddings response from vectors and request metadata.
    #[must_use]
    pub fn new(request: &EmbeddingRequest, embeddings: Vec<EmbeddingVector>) -> Self {
        Self {
            request_id: request.request_id.clone(),
            product_id: request.product_id.clone(),
            metadata: EmbeddingResponseMetadata {
                dimensions: request.model.dimensions,
                vector_count: embeddings.len(),
                model_id: request.model.model.model_id.clone(),
                normalization: request.model.normalization,
            },
            embeddings,
        }
    }
}

/// Minimal embeddings execution interface.
pub trait EmbeddingsExecutor {
    /// Error returned when embedding execution fails.
    type Error;

    /// Executes an embeddings request.
    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error>;
}

/// Stable generation session identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

impl SessionId {
    /// Creates a session identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Prompt input boundary for text generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum GenerationInput {
    /// Prompt text that still needs tokenization.
    Text(String),
    /// Pre-tokenized prompt IDs.
    Tokens(TokenSequence),
}

/// Deterministic decode strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodeStrategy {
    /// Greedy argmax decode.
    Greedy,
}

/// Generation options for the phase-1 reference path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationOptions {
    /// Maximum number of output tokens to emit.
    pub max_output_tokens: usize,
    /// Decode strategy.
    pub decode_strategy: DecodeStrategy,
}

impl GenerationOptions {
    /// Creates greedy-decode options.
    #[must_use]
    pub fn greedy(max_output_tokens: usize) -> Self {
        Self {
            max_output_tokens,
            decode_strategy: DecodeStrategy::Greedy,
        }
    }
}

/// Text-generation request contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Target decoder model descriptor.
    pub model: DecoderModelDescriptor,
    /// Optional session to reuse.
    pub session_id: Option<SessionId>,
    /// Prompt input boundary.
    pub prompt: GenerationInput,
    /// Generation options.
    pub options: GenerationOptions,
    /// Whether to reset the session cache before generation.
    pub reset_session: bool,
}

impl GenerationRequest {
    /// Creates a text prompt request.
    #[must_use]
    pub fn new_text(
        request_id: impl Into<String>,
        model: DecoderModelDescriptor,
        session_id: Option<SessionId>,
        prompt: impl Into<String>,
        options: GenerationOptions,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(TEXT_GENERATION_PRODUCT_ID),
            model,
            session_id,
            prompt: GenerationInput::Text(prompt.into()),
            options,
            reset_session: false,
        }
    }

    /// Creates a pre-tokenized request.
    #[must_use]
    pub fn new_tokens(
        request_id: impl Into<String>,
        model: DecoderModelDescriptor,
        session_id: Option<SessionId>,
        prompt: TokenSequence,
        options: GenerationOptions,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(TEXT_GENERATION_PRODUCT_ID),
            model,
            session_id,
            prompt: GenerationInput::Tokens(prompt),
            options,
            reset_session: false,
        }
    }

    /// Returns a copy that requests a session reset before generation.
    #[must_use]
    pub fn with_reset_session(mut self, reset_session: bool) -> Self {
        self.reset_session = reset_session;
        self
    }
}

/// Generated output payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationOutput {
    /// Output token IDs.
    pub tokens: TokenSequence,
    /// Output text rendering.
    pub text: String,
}

/// Usage counters for generation execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationUsage {
    /// Number of prompt tokens consumed.
    pub input_tokens: usize,
    /// Number of output tokens produced.
    pub output_tokens: usize,
    /// Number of cached KV slots retained after execution.
    pub cache_tokens: usize,
}

/// Terminal reason for a generation response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    /// The model emitted an EOS token.
    EndOfSequence,
    /// The request hit the configured output token cap.
    MaxOutputTokens,
    /// The request hit the context limit.
    ContextLimit,
}

/// Text-generation response contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Model identifier used for execution.
    pub model_id: String,
    /// Optional bound session identifier.
    pub session_id: Option<SessionId>,
    /// Generated payload.
    pub output: GenerationOutput,
    /// Usage counters.
    pub usage: GenerationUsage,
    /// Terminal termination reason.
    pub termination: TerminationReason,
}

impl GenerationResponse {
    /// Creates a generation response.
    #[must_use]
    pub fn new(
        request: &GenerationRequest,
        session_id: Option<SessionId>,
        tokens: TokenSequence,
        text: impl Into<String>,
        input_tokens: usize,
        cache_tokens: usize,
        termination: TerminationReason,
    ) -> Self {
        let output_tokens = tokens.len();
        Self {
            request_id: request.request_id.clone(),
            product_id: request.product_id.clone(),
            model_id: request.model.model.model_id.clone(),
            session_id,
            output: GenerationOutput {
                tokens,
                text: text.into(),
            },
            usage: GenerationUsage {
                input_tokens,
                output_tokens,
                cache_tokens,
            },
            termination,
        }
    }
}

/// Minimal text-generation execution interface.
pub trait TextGenerationExecutor {
    /// Error returned when generation fails.
    type Error;

    /// Executes a text-generation request.
    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error>;
}

/// Trait for loaded generation model handles kept active by the serve layer.
pub trait GenerationModelHandle {
    /// Returns the active model descriptor.
    fn descriptor(&self) -> &DecoderModelDescriptor;
}

impl GenerationModelHandle for ReferenceWordDecoder {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self.descriptor()
    }
}

trait WordDecoderExecutionModel: Clone {
    fn descriptor(&self) -> &DecoderModelDescriptor;
    fn tokenizer(&self) -> &FixtureWordTokenizer;
    fn weights(&self) -> &DecoderFixtureWeights;
}

impl WordDecoderExecutionModel for ReferenceWordDecoder {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self.descriptor()
    }

    fn tokenizer(&self) -> &FixtureWordTokenizer {
        self.tokenizer()
    }

    fn weights(&self) -> &DecoderFixtureWeights {
        self.weights()
    }
}

impl WordDecoderExecutionModel for ArtifactWordDecoder {
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self.descriptor()
    }

    fn tokenizer(&self) -> &FixtureWordTokenizer {
        self.tokenizer()
    }

    fn weights(&self) -> &DecoderFixtureWeights {
        self.weights()
    }
}

/// In-memory registry of loaded generation models.
#[derive(Clone, Debug, Default)]
pub struct InMemoryGenerationModelRegistry<M> {
    models: BTreeMap<String, M>,
}

impl<M> InMemoryGenerationModelRegistry<M>
where
    M: GenerationModelHandle,
{
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            models: BTreeMap::new(),
        }
    }

    /// Loads or replaces a model handle by model ID.
    pub fn load(&mut self, model: M) -> Option<M> {
        let model_id = model.descriptor().model.model_id.clone();
        self.models.insert(model_id, model)
    }

    /// Returns an active model by ID.
    #[must_use]
    pub fn active(&self, model_id: &str) -> Option<&M> {
        self.models.get(model_id)
    }

    /// Returns a mutable active model by ID.
    pub fn active_mut(&mut self, model_id: &str) -> Option<&mut M> {
        self.models.get_mut(model_id)
    }

    /// Unloads an active model.
    pub fn unload(&mut self, model_id: &str) -> Option<M> {
        self.models.remove(model_id)
    }

    /// Returns the number of active models.
    #[must_use]
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Returns whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

/// Single KV cache entry for one token position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KvCacheEntry {
    /// Token position inside the session cache.
    pub position: usize,
    /// Token ID associated with the slot.
    pub token: TokenId,
    /// Key vector.
    pub key: Vec<f32>,
    /// Value vector.
    pub value: Vec<f32>,
}

/// KV cache failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KvCacheError {
    /// A KV append exceeded the configured context limit.
    #[error("session KV cache is full: max_context={max_context}")]
    ContextLimitExceeded {
        /// Maximum cache size.
        max_context: usize,
    },
    /// A KV append used vectors with the wrong width.
    #[error("kv width mismatch: expected={expected} key={actual_key} value={actual_value}")]
    WidthMismatch {
        /// Expected width.
        expected: usize,
        /// Actual key width.
        actual_key: usize,
        /// Actual value width.
        actual_value: usize,
    },
}

/// In-memory per-session KV cache for the phase-1 reference path.
#[derive(Clone, Debug, PartialEq)]
pub struct InMemoryKvCache {
    max_context: usize,
    width: usize,
    entries: Vec<KvCacheEntry>,
}

impl InMemoryKvCache {
    /// Creates an empty in-memory KV cache.
    #[must_use]
    pub fn new(max_context: usize, width: usize) -> Self {
        Self {
            max_context,
            width,
            entries: Vec::new(),
        }
    }

    /// Returns the configured context limit.
    #[must_use]
    pub const fn max_context(&self) -> usize {
        self.max_context
    }

    /// Returns the configured KV vector width.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Returns the number of cached slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the cached entries in token order.
    #[must_use]
    pub fn entries(&self) -> &[KvCacheEntry] {
        &self.entries
    }

    /// Appends a token KV pair to the cache.
    pub fn append(
        &mut self,
        token: TokenId,
        key: Vec<f32>,
        value: Vec<f32>,
    ) -> Result<(), KvCacheError> {
        if self.entries.len() >= self.max_context {
            return Err(KvCacheError::ContextLimitExceeded {
                max_context: self.max_context,
            });
        }
        if key.len() != self.width || value.len() != self.width {
            return Err(KvCacheError::WidthMismatch {
                expected: self.width,
                actual_key: key.len(),
                actual_value: value.len(),
            });
        }

        self.entries.push(KvCacheEntry {
            position: self.entries.len(),
            token,
            key,
            value,
        });
        Ok(())
    }

    /// Clears all cached slots.
    pub fn reset(&mut self) {
        self.entries.clear();
    }
}

/// Session metadata surfaced to higher layers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationSession {
    /// Stable session identifier.
    pub session_id: SessionId,
    /// Bound model identifier.
    pub model_id: String,
    /// Maximum context for the session cache.
    pub max_context: usize,
    /// KV vector width.
    pub kv_width: usize,
    /// Current cached token count.
    pub cached_tokens: usize,
}

/// Session state stored in memory.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationSessionState {
    session: GenerationSession,
    cache: InMemoryKvCache,
}

impl GenerationSessionState {
    /// Returns session metadata.
    #[must_use]
    pub fn session(&self) -> &GenerationSession {
        &self.session
    }

    /// Returns the per-session KV cache.
    #[must_use]
    pub fn cache(&self) -> &InMemoryKvCache {
        &self.cache
    }
}

/// Session store failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionStoreError {
    /// A session identifier was not found.
    #[error("generation session `{0}` was not found")]
    SessionNotFound(String),
    /// The caller attempted to use a session with the wrong model.
    #[error(
        "generation session `{session_id}` expects model `{expected_model}` but got `{actual_model}`"
    )]
    ModelMismatch {
        /// Session identifier.
        session_id: String,
        /// Expected model identifier.
        expected_model: String,
        /// Actual model identifier.
        actual_model: String,
    },
    /// The caller attempted to replace a session cache with incompatible
    /// geometry.
    #[error(
        "generation session `{session_id}` cache geometry mismatch: expected max_context={expected_max_context} width={expected_width}, actual max_context={actual_max_context} width={actual_width}"
    )]
    CacheGeometryMismatch {
        /// Session identifier.
        session_id: String,
        /// Expected max context.
        expected_max_context: usize,
        /// Expected KV width.
        expected_width: usize,
        /// Actual max context.
        actual_max_context: usize,
        /// Actual KV width.
        actual_width: usize,
    },
    /// The session cache rejected an operation.
    #[error(transparent)]
    Cache(#[from] KvCacheError),
}

/// In-memory generation session store for the phase-1 reference path.
#[derive(Clone, Debug, Default)]
pub struct InMemoryGenerationSessionStore {
    next_session: u64,
    sessions: BTreeMap<SessionId, GenerationSessionState>,
}

impl InMemoryGenerationSessionStore {
    /// Creates an empty session store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_session: 0,
            sessions: BTreeMap::new(),
        }
    }

    /// Creates a new session bound to a decoder model.
    pub fn create(&mut self, model: &DecoderModelDescriptor) -> GenerationSession {
        self.next_session += 1;
        let session_id = SessionId::new(format!("sess-{:08}", self.next_session));
        let session = GenerationSession {
            session_id: session_id.clone(),
            model_id: model.model.model_id.clone(),
            max_context: model.config.max_context,
            kv_width: model.config.kv_width(),
            cached_tokens: 0,
        };
        let state = GenerationSessionState {
            session: session.clone(),
            cache: InMemoryKvCache::new(model.config.max_context, model.config.kv_width()),
        };
        self.sessions.insert(session_id, state);
        session
    }

    /// Returns session metadata by ID.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&GenerationSession> {
        self.sessions
            .get(session_id)
            .map(GenerationSessionState::session)
    }

    /// Returns session state by ID.
    pub fn state(
        &self,
        session_id: &SessionId,
    ) -> Result<&GenerationSessionState, SessionStoreError> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))
    }

    /// Returns the session cache by ID.
    pub fn cache(&self, session_id: &SessionId) -> Result<&InMemoryKvCache, SessionStoreError> {
        Ok(self.state(session_id)?.cache())
    }

    /// Appends a KV slot to a session and returns updated metadata.
    pub fn append(
        &mut self,
        session_id: &SessionId,
        model_id: &str,
        token: TokenId,
        key: Vec<f32>,
        value: Vec<f32>,
    ) -> Result<GenerationSession, SessionStoreError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))?;
        if state.session.model_id != model_id {
            return Err(SessionStoreError::ModelMismatch {
                session_id: session_id.as_str().to_string(),
                expected_model: state.session.model_id.clone(),
                actual_model: model_id.to_string(),
            });
        }

        state.cache.append(token, key, value)?;
        state.session.cached_tokens = state.cache.len();
        Ok(state.session.clone())
    }

    /// Resets a session cache in place and returns updated metadata.
    pub fn reset(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, SessionStoreError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))?;
        state.cache.reset();
        state.session.cached_tokens = 0;
        Ok(state.session.clone())
    }

    /// Closes a session and drops its KV cache.
    pub fn close(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, SessionStoreError> {
        self.sessions
            .remove(session_id)
            .map(|state| state.session)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))
    }

    /// Replaces a session cache wholesale and returns updated metadata.
    pub fn replace_cache(
        &mut self,
        session_id: &SessionId,
        model_id: &str,
        cache: InMemoryKvCache,
    ) -> Result<GenerationSession, SessionStoreError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))?;
        if state.session.model_id != model_id {
            return Err(SessionStoreError::ModelMismatch {
                session_id: session_id.as_str().to_string(),
                expected_model: state.session.model_id.clone(),
                actual_model: model_id.to_string(),
            });
        }
        if state.cache.max_context() != cache.max_context() || state.cache.width() != cache.width()
        {
            return Err(SessionStoreError::CacheGeometryMismatch {
                session_id: session_id.as_str().to_string(),
                expected_max_context: state.cache.max_context(),
                expected_width: state.cache.width(),
                actual_max_context: cache.max_context(),
                actual_width: cache.width(),
            });
        }

        state.cache = cache;
        state.session.cached_tokens = state.cache.len();
        Ok(state.session.clone())
    }
}

/// CPU reference text-generation error.
#[derive(Debug, Error)]
pub enum ReferenceTextGenerationError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The requested model is not loaded or does not match the loaded
    /// descriptor.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The request carried no prompt tokens.
    #[error("generation request must contain at least one prompt token")]
    EmptyPrompt,
    /// A supplied token ID does not fit the active vocabulary.
    #[error("invalid token id {token} for vocabulary size {vocab_size}")]
    InvalidToken {
        /// Token value.
        token: u32,
        /// Active vocabulary size.
        vocab_size: usize,
    },
    /// A token step requested an out-of-range position.
    #[error("invalid position {position} for max context {max_context}")]
    InvalidPosition {
        /// Requested position.
        position: usize,
        /// Maximum supported context length.
        max_context: usize,
    },
    /// A context vector had the wrong width.
    #[error("invalid context width: expected {expected}, actual {actual}")]
    InvalidContextWidth {
        /// Expected width.
        expected: usize,
        /// Actual width.
        actual: usize,
    },
    /// The active cache geometry is incompatible with the reference model.
    #[error("unsupported cache geometry: hidden_size={hidden_size} kv_width={kv_width}")]
    UnsupportedCacheGeometry {
        /// Model hidden size.
        hidden_size: usize,
        /// Session KV width.
        kv_width: usize,
    },
    /// Loading or validating a model artifact failed.
    #[error(transparent)]
    Model(#[from] ModelLoadError),
    /// The compiler rejected the reference graph.
    #[error(transparent)]
    Compile(#[from] CompileError),
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// Session operations failed.
    #[error(transparent)]
    Session(#[from] SessionStoreError),
    /// Cache operations failed.
    #[error(transparent)]
    Cache(#[from] KvCacheError),
    /// CPU runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    /// An expected graph output was missing.
    #[error("missing graph output `{0}`")]
    MissingOutput(&'static str),
}

#[derive(Clone, Debug)]
struct GenerationStepOutput {
    hidden: Vec<f32>,
    logits: Vec<f32>,
}

/// Loaded CPU-backed generation model.
#[derive(Clone, Debug)]
struct CpuWordGenerationModel<M> {
    model: M,
    graph: Graph,
    token_input_id: TensorId,
    position_input_id: TensorId,
    context_input_id: TensorId,
    hidden_output_id: TensorId,
    logits_output_id: TensorId,
    plan_digest: String,
}

impl<M> CpuWordGenerationModel<M>
where
    M: WordDecoderExecutionModel,
{
    /// Loads and compiles a decoder model.
    fn new(model: M) -> Result<Self, ReferenceTextGenerationError> {
        let (
            graph,
            token_input_id,
            position_input_id,
            context_input_id,
            hidden_output_id,
            logits_output_id,
        ) = build_generation_graph(&model)?;
        let plan_digest = compile_graph(&graph)?.stable_digest();
        Ok(Self {
            model,
            graph,
            token_input_id,
            position_input_id,
            context_input_id,
            hidden_output_id,
            logits_output_id,
            plan_digest,
        })
    }

    /// Returns the underlying generation model.
    #[must_use]
    fn model(&self) -> &M {
        &self.model
    }

    /// Returns the stable compiled-plan digest.
    #[must_use]
    fn plan_digest(&self) -> &str {
        self.plan_digest.as_str()
    }

    fn execute_step(
        &self,
        backend: &mut CpuBackend,
        token: TokenId,
        position: usize,
        context: &[f32],
    ) -> Result<GenerationStepOutput, ReferenceTextGenerationError> {
        let config = &self.model.descriptor().config;
        if token.as_u32() as usize >= config.vocab_size {
            return Err(ReferenceTextGenerationError::InvalidToken {
                token: token.as_u32(),
                vocab_size: config.vocab_size,
            });
        }
        if position >= config.max_context {
            return Err(ReferenceTextGenerationError::InvalidPosition {
                position,
                max_context: config.max_context,
            });
        }
        if context.len() != config.hidden_size {
            return Err(ReferenceTextGenerationError::InvalidContextWidth {
                expected: config.hidden_size,
                actual: context.len(),
            });
        }

        let mut runtime_inputs = BTreeMap::new();
        runtime_inputs.insert(
            self.token_input_id,
            backend.input_buffer(
                Shape::new(vec![1, config.vocab_size]),
                one_hot(config.vocab_size, token.as_u32() as usize),
            )?,
        );
        runtime_inputs.insert(
            self.position_input_id,
            backend.input_buffer(
                Shape::new(vec![1, config.max_context]),
                one_hot(config.max_context, position),
            )?,
        );
        runtime_inputs.insert(
            self.context_input_id,
            backend.input_buffer(Shape::new(vec![1, config.hidden_size]), context.to_vec())?,
        );

        let result = backend.compile_and_execute(&self.graph, &runtime_inputs)?;
        let hidden = result
            .outputs
            .get(&self.hidden_output_id)
            .ok_or(ReferenceTextGenerationError::MissingOutput("hidden"))?
            .as_f32_slice()
            .to_vec();
        let logits = result
            .outputs
            .get(&self.logits_output_id)
            .ok_or(ReferenceTextGenerationError::MissingOutput("logits"))?
            .as_f32_slice()
            .to_vec();
        Ok(GenerationStepOutput { hidden, logits })
    }
}

impl<M> GenerationModelHandle for CpuWordGenerationModel<M>
where
    M: WordDecoderExecutionModel,
{
    fn descriptor(&self) -> &DecoderModelDescriptor {
        self.model.descriptor()
    }
}

/// Reference-model alias for the phase-1 text-generation path.
type CpuReferenceGenerationModel = CpuWordGenerationModel<ReferenceWordDecoder>;

/// Artifact-backed model alias for the first model-backed text-generation path.
type CpuModelGenerationModel = CpuWordGenerationModel<ArtifactWordDecoder>;

/// CPU-backed deterministic text-generation reference service.
#[derive(Clone, Debug)]
pub struct CpuReferenceTextGenerationService {
    backend: CpuBackend,
    models: InMemoryGenerationModelRegistry<CpuReferenceGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    model_descriptor: DecoderModelDescriptor,
}

impl CpuReferenceTextGenerationService {
    /// Creates a service with the default reference decoder loaded.
    pub fn new() -> Result<Self, ReferenceTextGenerationError> {
        let model = ReferenceWordDecoder::new();
        let model_descriptor = model.descriptor().clone();
        let mut models = InMemoryGenerationModelRegistry::new();
        models.load(CpuReferenceGenerationModel::new(model)?);
        Ok(Self {
            backend: CpuBackend::new(),
            models,
            sessions: InMemoryGenerationSessionStore::new(),
            model_descriptor,
        })
    }

    /// Loads or replaces a reference decoder model.
    pub fn load_model(
        &mut self,
        model: ReferenceWordDecoder,
    ) -> Result<(), ReferenceTextGenerationError> {
        self.model_descriptor = model.descriptor().clone();
        self.models.load(CpuReferenceGenerationModel::new(model)?);
        Ok(())
    }

    /// Returns the default reference model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model_descriptor
    }

    /// Returns the compiled plan digest for a loaded model.
    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        self.models
            .active(model_id)
            .map(CpuReferenceGenerationModel::plan_digest)
    }

    /// Creates a reusable generation session for the provided model ID.
    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        let model = self
            .models
            .active(model_id)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedModel(model_id.to_string()))?;
        Ok(self.sessions.create(model.descriptor()))
    }

    /// Resets an existing session.
    pub fn reset_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.reset(session_id)?)
    }

    /// Closes an existing session.
    pub fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.close(session_id)?)
    }
}

impl TextGenerationExecutor for CpuReferenceTextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        run_generation_request(&mut self.backend, &self.models, &mut self.sessions, request)
    }
}

/// CPU-backed model-backed text-generation service.
#[derive(Clone, Debug)]
pub struct CpuModelTextGenerationService {
    backend: CpuBackend,
    models: InMemoryGenerationModelRegistry<CpuModelGenerationModel>,
    sessions: InMemoryGenerationSessionStore,
    model_descriptor: DecoderModelDescriptor,
}

impl CpuModelTextGenerationService {
    /// Creates a service with the artifact-backed decoder loaded from a local safetensors file.
    pub fn from_safetensors_artifact(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, ReferenceTextGenerationError> {
        let model = ArtifactWordDecoder::from_safetensors_artifact(path)?;
        let model_descriptor = model.descriptor().clone();
        let mut models = InMemoryGenerationModelRegistry::new();
        models.load(CpuModelGenerationModel::new(model)?);
        Ok(Self {
            backend: CpuBackend::new(),
            models,
            sessions: InMemoryGenerationSessionStore::new(),
            model_descriptor,
        })
    }

    /// Loads or replaces an artifact-backed decoder model.
    pub fn load_model(
        &mut self,
        model: ArtifactWordDecoder,
    ) -> Result<(), ReferenceTextGenerationError> {
        self.model_descriptor = model.descriptor().clone();
        self.models.load(CpuModelGenerationModel::new(model)?);
        Ok(())
    }

    /// Returns the loaded model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &DecoderModelDescriptor {
        &self.model_descriptor
    }

    /// Returns the compiled plan digest for the loaded model.
    #[must_use]
    pub fn plan_digest(&self, model_id: &str) -> Option<&str> {
        self.models
            .active(model_id)
            .map(CpuModelGenerationModel::plan_digest)
    }

    /// Creates a reusable generation session for the provided model ID.
    pub fn create_session(
        &mut self,
        model_id: &str,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        let model = self
            .models
            .active(model_id)
            .ok_or_else(|| ReferenceTextGenerationError::UnsupportedModel(model_id.to_string()))?;
        Ok(self.sessions.create(model.descriptor()))
    }

    /// Resets an existing session.
    pub fn reset_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.reset(session_id)?)
    }

    /// Closes an existing session.
    pub fn close_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GenerationSession, ReferenceTextGenerationError> {
        Ok(self.sessions.close(session_id)?)
    }
}

impl TextGenerationExecutor for CpuModelTextGenerationService {
    type Error = ReferenceTextGenerationError;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        run_generation_request(&mut self.backend, &self.models, &mut self.sessions, request)
    }
}

/// Honest CPU product alias for model-backed text generation.
pub type CpuProductTextGenerationService = CpuModelTextGenerationService;

fn run_generation_request<M>(
    backend: &mut CpuBackend,
    models: &InMemoryGenerationModelRegistry<CpuWordGenerationModel<M>>,
    sessions: &mut InMemoryGenerationSessionStore,
    request: &GenerationRequest,
) -> Result<GenerationResponse, ReferenceTextGenerationError>
where
    M: WordDecoderExecutionModel,
{
    if request.product_id != TEXT_GENERATION_PRODUCT_ID {
        return Err(ReferenceTextGenerationError::UnsupportedProduct(
            request.product_id.clone(),
        ));
    }

    let loaded_model = models
        .active(request.model.model.model_id.as_str())
        .ok_or_else(|| {
            ReferenceTextGenerationError::UnsupportedModel(request.model.model.model_id.clone())
        })?
        .clone();
    if loaded_model.descriptor() != &request.model {
        return Err(ReferenceTextGenerationError::UnsupportedModel(
            request.model.model.model_id.clone(),
        ));
    }

    let prompt_tokens = match &request.prompt {
        GenerationInput::Text(text) => loaded_model
            .model()
            .tokenizer()
            .encode_with_special_tokens(text, true, false),
        GenerationInput::Tokens(tokens) => tokens.clone(),
    };
    if prompt_tokens.is_empty() {
        return Err(ReferenceTextGenerationError::EmptyPrompt);
    }

    let hidden_size = loaded_model.descriptor().config.hidden_size;
    let mut cache = if let Some(session_id) = &request.session_id {
        if request.reset_session {
            sessions.reset(session_id)?;
        }
        sessions.cache(session_id)?.clone()
    } else {
        InMemoryKvCache::new(
            loaded_model.descriptor().config.max_context,
            loaded_model.descriptor().config.hidden_size,
        )
    };
    if cache.width() != hidden_size {
        return Err(ReferenceTextGenerationError::UnsupportedCacheGeometry {
            hidden_size,
            kv_width: cache.width(),
        });
    }

    let mut last_logits = Vec::new();
    for token in prompt_tokens.as_slice() {
        let context = mean_cache_value(&cache, hidden_size);
        let step = loaded_model.execute_step(backend, *token, cache.len(), &context)?;
        cache.append(*token, step.hidden.clone(), step.hidden)?;
        last_logits = step.logits;
    }

    let eos_id = loaded_model.model().tokenizer().vocabulary().eos_id();
    let mut generated_tokens = Vec::new();
    let termination = loop {
        if generated_tokens.len() >= request.options.max_output_tokens {
            break TerminationReason::MaxOutputTokens;
        }
        if cache.len() >= cache.max_context() {
            break TerminationReason::ContextLimit;
        }

        let next_token = select_argmax(&last_logits)
            .ok_or(ReferenceTextGenerationError::MissingOutput("next_token"))?;
        if next_token == eos_id {
            break TerminationReason::EndOfSequence;
        }

        generated_tokens.push(next_token);
        let context = mean_cache_value(&cache, hidden_size);
        let step = loaded_model.execute_step(backend, next_token, cache.len(), &context)?;
        cache.append(next_token, step.hidden.clone(), step.hidden)?;
        last_logits = step.logits;
    };

    if let Some(session_id) = &request.session_id {
        sessions.replace_cache(
            session_id,
            request.model.model.model_id.as_str(),
            cache.clone(),
        )?;
    }

    let generated = TokenSequence::new(generated_tokens);
    let text = loaded_model
        .model()
        .tokenizer()
        .decode(generated.as_slice());
    Ok(GenerationResponse::new(
        request,
        request.session_id.clone(),
        generated,
        text,
        prompt_tokens.len(),
        cache.len(),
        termination,
    ))
}

fn build_generation_graph<M>(
    model: &M,
) -> Result<(Graph, TensorId, TensorId, TensorId, TensorId, TensorId), GraphError>
where
    M: WordDecoderExecutionModel,
{
    let descriptor = model.descriptor();
    let config = &descriptor.config;
    let weights = model.weights();

    let mut builder = GraphBuilder::new(Device::cpu());
    let token_input = builder.input(
        "token_one_hot",
        Shape::new(vec![1, config.vocab_size]),
        DType::F32,
    );
    let position_input = builder.input(
        "position_one_hot",
        Shape::new(vec![1, config.max_context]),
        DType::F32,
    );
    let context_input = builder.input(
        "context",
        Shape::new(vec![1, config.hidden_size]),
        DType::F32,
    );
    let token_embedding = builder.constant_f32(
        Shape::new(vec![config.vocab_size, config.hidden_size]),
        weights.token_embedding().to_vec(),
    )?;
    let position_embedding = builder.constant_f32(
        Shape::new(vec![config.max_context, config.hidden_size]),
        weights.position_embedding().to_vec(),
    )?;
    let context_projection = builder.constant_f32(
        Shape::new(vec![config.hidden_size, config.hidden_size]),
        weights.context_projection().to_vec(),
    )?;
    let lm_head = builder.constant_f32(
        Shape::new(vec![config.hidden_size, config.vocab_size]),
        weights.lm_head().to_vec(),
    )?;
    let lm_bias = builder.constant_f32(
        Shape::new(vec![1, config.vocab_size]),
        weights.lm_bias().to_vec(),
    )?;

    let token_hidden = builder.matmul(&token_input, &token_embedding)?;
    let position_hidden = builder.matmul(&position_input, &position_embedding)?;
    let context_hidden = builder.matmul(&context_input, &context_projection)?;
    let hidden = builder.add(&token_hidden, &position_hidden)?;
    let hidden = builder.add(&hidden, &context_hidden)?;
    let logits = builder.matmul(&hidden, &lm_head)?;
    let logits = builder.add(&logits, &lm_bias)?;

    Ok((
        builder.finish(vec![hidden.clone(), logits.clone()]),
        token_input.id(),
        position_input.id(),
        context_input.id(),
        hidden.id(),
        logits.id(),
    ))
}

fn one_hot(width: usize, index: usize) -> Vec<f32> {
    let mut output = vec![0.0; width];
    output[index] = 1.0;
    output
}

fn mean_cache_value(cache: &InMemoryKvCache, width: usize) -> Vec<f32> {
    if cache.is_empty() {
        return vec![0.0; width];
    }

    let mut output = vec![0.0; width];
    for entry in cache.entries() {
        for (accumulator, value) in output.iter_mut().zip(entry.value.iter()) {
            *accumulator += *value;
        }
    }
    let scale = 1.0 / (cache.len() as f32);
    for value in &mut output {
        *value *= scale;
    }
    output
}

fn select_argmax(logits: &[f32]) -> Option<TokenId> {
    logits
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, _)| TokenId(index as u32))
}

/// Smoke embeddings execution error.
#[derive(Debug, Error)]
pub enum SmokeEmbeddingsError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The request targeted the wrong model.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The request carried no inputs.
    #[error("embedding request must contain at least one input")]
    EmptyInputBatch,
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// CPU runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

/// Model-backed embeddings execution error.
#[derive(Debug, Error)]
pub enum ModelEmbeddingsError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The request targeted the wrong model.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The request carried no inputs.
    #[error("embedding request must contain at least one input")]
    EmptyInputBatch,
    /// Loading or validating the model failed.
    #[error(transparent)]
    Model(#[from] ModelLoadError),
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// CPU runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

/// Metal-backed embeddings execution error.
#[derive(Debug, Error)]
pub enum MetalEmbeddingsError {
    /// The request targeted the wrong product.
    #[error("unsupported product id `{0}`")]
    UnsupportedProduct(String),
    /// The request targeted the wrong model.
    #[error("unsupported model `{0}`")]
    UnsupportedModel(String),
    /// The request carried no inputs.
    #[error("embedding request must contain at least one input")]
    EmptyInputBatch,
    /// Metal is not available for the requested product path on this machine.
    #[error("metal backend unavailable ({status:?}): {message}")]
    BackendUnavailable {
        /// Honest backend status.
        status: HealthStatus,
        /// Plain-text reason.
        message: String,
    },
    /// Loading or validating the model failed.
    #[error(transparent)]
    Model(#[from] ModelLoadError),
    /// Graph construction failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// Metal runtime execution failed.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

/// CPU-backed embeddings smoke service.
#[derive(Clone, Debug)]
pub struct SmokeEmbeddingsService {
    backend: CpuBackend,
    model: SmokeByteEmbedder,
    graph: Graph,
    input_shape: Shape,
    input_id: TensorId,
    output_id: TensorId,
}

impl SmokeEmbeddingsService {
    /// Creates a new smoke embeddings service.
    pub fn new() -> Result<Self, SmokeEmbeddingsError> {
        let model = SmokeByteEmbedder::new();
        let input_shape = Shape::new(vec![1, model.input_dimensions()]);
        let (graph, input_id, output_id) = build_embedding_graph(
            Device::cpu(),
            model.input_dimensions(),
            model.descriptor().dimensions,
            model.projection(),
            model.bias(),
            input_shape.clone(),
        )?;
        Ok(Self {
            backend: CpuBackend::new(),
            model,
            graph,
            input_shape,
            input_id,
            output_id,
        })
    }

    /// Returns the smoke model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &EmbeddingModelDescriptor {
        self.model.descriptor()
    }

    fn embed_one(&mut self, input: &str) -> Result<Vec<f32>, SmokeEmbeddingsError> {
        execute_cpu_embedding_graph(
            &mut self.backend,
            &self.graph,
            self.input_id,
            self.output_id,
            &self.input_shape,
            self.model.featurize(input),
        )
        .map_err(SmokeEmbeddingsError::Runtime)
    }
}

impl EmbeddingsExecutor for SmokeEmbeddingsService {
    type Error = SmokeEmbeddingsError;

    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error> {
        if request.product_id != EMBEDDINGS_PRODUCT_ID {
            return Err(SmokeEmbeddingsError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model.model.model_id != self.model.descriptor().model.model_id {
            return Err(SmokeEmbeddingsError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.inputs.is_empty() {
            return Err(SmokeEmbeddingsError::EmptyInputBatch);
        }

        let mut embeddings = Vec::with_capacity(request.inputs.len());
        for (index, input) in request.inputs.iter().enumerate() {
            embeddings.push(EmbeddingVector {
                index,
                values: self.embed_one(input)?,
            });
        }

        Ok(EmbeddingResponse::new(request, embeddings))
    }
}

/// CPU-backed model embeddings service for artifact-loaded embedding families.
#[derive(Clone, Debug)]
pub struct CpuModelEmbeddingsService {
    backend: CpuBackend,
    model: ByteProjectionEmbedder,
    graph: Graph,
    input_shape: Shape,
    input_id: TensorId,
    output_id: TensorId,
}

impl CpuModelEmbeddingsService {
    /// Loads the first model-backed embeddings family from a local safetensors artifact.
    pub fn from_safetensors_artifact(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, ModelEmbeddingsError> {
        let model = ByteProjectionEmbedder::from_safetensors_artifact(path)?;
        let input_shape = Shape::new(vec![1, model.input_dimensions()]);
        let (graph, input_id, output_id) = build_embedding_graph(
            Device::cpu(),
            model.input_dimensions(),
            model.descriptor().dimensions,
            model.weights().projection(),
            model.weights().bias(),
            input_shape.clone(),
        )?;
        Ok(Self {
            backend: CpuBackend::new(),
            model,
            graph,
            input_shape,
            input_id,
            output_id,
        })
    }

    /// Returns the loaded model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &EmbeddingModelDescriptor {
        self.model.descriptor()
    }

    fn embed_one(&mut self, input: &str) -> Result<Vec<f32>, ModelEmbeddingsError> {
        let values = execute_cpu_embedding_graph(
            &mut self.backend,
            &self.graph,
            self.input_id,
            self.output_id,
            &self.input_shape,
            self.model.featurize(input),
        )?;
        Ok(normalize_embedding(
            values,
            self.model.descriptor().normalization,
        ))
    }
}

impl EmbeddingsExecutor for CpuModelEmbeddingsService {
    type Error = ModelEmbeddingsError;

    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error> {
        if request.product_id != EMBEDDINGS_PRODUCT_ID {
            return Err(ModelEmbeddingsError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model != *self.model.descriptor() {
            return Err(ModelEmbeddingsError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.inputs.is_empty() {
            return Err(ModelEmbeddingsError::EmptyInputBatch);
        }

        let mut embeddings = Vec::with_capacity(request.inputs.len());
        for (index, input) in request.inputs.iter().enumerate() {
            embeddings.push(EmbeddingVector {
                index,
                values: self.embed_one(input)?,
            });
        }

        Ok(EmbeddingResponse::new(request, embeddings))
    }
}

/// Honest CPU product alias for model-backed embeddings.
pub type CpuProductEmbeddingsService = CpuModelEmbeddingsService;

/// Metal-backed embeddings service for the supported model-backed product path.
///
/// Text generation remains CPU-only today; this service exists only for the
/// first accelerated `mox.embeddings` milestone.
pub struct MetalModelEmbeddingsService {
    backend: MetalBackend,
    backend_selection: BackendSelection,
    model: ByteProjectionEmbedder,
    graph: Graph,
    input_shape: Shape,
    input_id: TensorId,
    output_id: TensorId,
}

impl MetalModelEmbeddingsService {
    /// Loads the first model-backed embeddings family on Metal when the local
    /// machine exposes a genuinely supported Metal execution device.
    pub fn from_safetensors_artifact(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, MetalEmbeddingsError> {
        let backend = MetalBackend::new();
        let backend_selection = backend
            .backend_selection(EMBEDDINGS_SUPPORTED_OPS)
            .map_err(|error| MetalEmbeddingsError::BackendUnavailable {
                status: backend.health().status,
                message: error.to_string(),
            })?;
        let selected_device = backend_selection
            .selected_device
            .as_ref()
            .map(|device| device.device.clone())
            .ok_or_else(|| MetalEmbeddingsError::BackendUnavailable {
                status: backend.health().status,
                message: String::from("metal backend selected no execution device"),
            })?;

        let model = ByteProjectionEmbedder::from_safetensors_artifact(path)?;
        let input_shape = Shape::new(vec![1, model.input_dimensions()]);
        let (graph, input_id, output_id) = build_embedding_graph(
            selected_device,
            model.input_dimensions(),
            model.descriptor().dimensions,
            model.weights().projection(),
            model.weights().bias(),
            input_shape.clone(),
        )?;
        Ok(Self {
            backend,
            backend_selection,
            model,
            graph,
            input_shape,
            input_id,
            output_id,
        })
    }

    /// Returns the loaded model descriptor.
    #[must_use]
    pub fn model_descriptor(&self) -> &EmbeddingModelDescriptor {
        self.model.descriptor()
    }

    /// Returns truthful backend-selection data for the loaded Metal product.
    #[must_use]
    pub fn backend_selection(&self) -> &BackendSelection {
        &self.backend_selection
    }

    fn embed_one(&mut self, input: &str) -> Result<Vec<f32>, MetalEmbeddingsError> {
        let values = execute_metal_embedding_graph(
            &mut self.backend,
            &self.graph,
            self.input_id,
            self.output_id,
            &self.input_shape,
            self.model.featurize(input),
        )?;
        Ok(normalize_embedding(
            values,
            self.model.descriptor().normalization,
        ))
    }
}

impl EmbeddingsExecutor for MetalModelEmbeddingsService {
    type Error = MetalEmbeddingsError;

    fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, Self::Error> {
        if request.product_id != EMBEDDINGS_PRODUCT_ID {
            return Err(MetalEmbeddingsError::UnsupportedProduct(
                request.product_id.clone(),
            ));
        }
        if request.model != *self.model.descriptor() {
            return Err(MetalEmbeddingsError::UnsupportedModel(
                request.model.model.model_id.clone(),
            ));
        }
        if request.inputs.is_empty() {
            return Err(MetalEmbeddingsError::EmptyInputBatch);
        }

        let mut embeddings = Vec::with_capacity(request.inputs.len());
        for (index, input) in request.inputs.iter().enumerate() {
            embeddings.push(EmbeddingVector {
                index,
                values: self.embed_one(input)?,
            });
        }

        Ok(EmbeddingResponse::new(request, embeddings))
    }
}

/// Honest Metal product alias for model-backed embeddings.
pub type MetalProductEmbeddingsService = MetalModelEmbeddingsService;

fn build_embedding_graph(
    device: Device,
    input_dimensions: usize,
    output_dimensions: usize,
    projection: &[f32],
    bias: &[f32],
    input_shape: Shape,
) -> Result<(Graph, TensorId, TensorId), GraphError> {
    let mut builder = GraphBuilder::new(device);
    let input = builder.input("features", input_shape, DType::F32);
    let weights = builder.constant_f32(
        Shape::new(vec![input_dimensions, output_dimensions]),
        projection.to_vec(),
    )?;
    let bias = builder.constant_f32(Shape::new(vec![1, output_dimensions]), bias.to_vec())?;
    let projected = builder.matmul(&input, &weights)?;
    let shifted = builder.add(&projected, &bias)?;
    let output_id = shifted.id();
    let input_id = input.id();
    let graph = builder.finish(vec![shifted]);
    Ok((graph, input_id, output_id))
}

fn execute_cpu_embedding_graph(
    backend: &mut CpuBackend,
    graph: &Graph,
    input_id: TensorId,
    output_id: TensorId,
    input_shape: &Shape,
    features: Vec<f32>,
) -> Result<Vec<f32>, RuntimeError> {
    let mut runtime_inputs = BTreeMap::new();
    runtime_inputs.insert(
        input_id,
        backend.input_buffer(input_shape.clone(), features)?,
    );
    let result = backend.compile_and_execute(graph, &runtime_inputs)?;
    let Some(output) = result.outputs.get(&output_id) else {
        return Err(RuntimeError::Backend(String::from(
            "missing embedding output",
        )));
    };
    Ok(output.as_f32_slice().to_vec())
}

fn execute_metal_embedding_graph(
    backend: &mut MetalBackend,
    graph: &Graph,
    input_id: TensorId,
    output_id: TensorId,
    input_shape: &Shape,
    features: Vec<f32>,
) -> Result<Vec<f32>, RuntimeError> {
    let mut runtime_inputs = BTreeMap::new();
    runtime_inputs.insert(
        input_id,
        backend.input_buffer(input_shape.clone(), features)?,
    );
    let result = backend.compile_and_execute(graph, &runtime_inputs)?;
    let Some(output) = result.outputs.get(&output_id) else {
        return Err(RuntimeError::Backend(String::from(
            "missing embedding output",
        )));
    };
    output.read_f32()
}

fn normalize_embedding(values: Vec<f32>, normalization: EmbeddingNormalization) -> Vec<f32> {
    if normalization != EmbeddingNormalization::UnitLength {
        return values;
    }
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        return values;
    }
    values.into_iter().map(|value| value / norm).collect()
}

#[cfg(test)]
mod tests {
    use mox_core::{DType, Shape};

    use super::{
        CpuReferenceTextGenerationService, EmbeddingRequest, EmbeddingResponse, EmbeddingVector,
        EmbeddingsExecutor, FixtureWordTokenizer, GenerationOptions, GenerationRequest,
        GenerationResponse, InMemoryGenerationModelRegistry, InMemoryGenerationSessionStore,
        ModelDescriptor, ReferenceTextGenerationError, ReferenceWordDecoder, SessionId,
        SmokeByteEmbedder, SmokeEmbeddingsService, TerminationReason, TextGenerationExecutor,
        WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
    };
    use crate::{DecoderBlockConfig, DecoderConfig, DecoderModelDescriptor};
    use mox_models::{
        ActivationFunction, DecoderAttentionConfig, DecoderFeedForwardConfig, TokenSequence,
    };

    #[test]
    fn embedding_request_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-1",
            sample_embedding_descriptor(),
            vec![String::from("hello world"), String::from("open agents")],
        );

        let encoded = serde_json::to_string_pretty(&request)?;
        let expected = r#"{
  "request_id": "req-1",
  "product_id": "mox.embeddings",
  "model": {
    "model": {
      "model_id": "smoke-byte-embed-v0",
      "family": "smoke",
      "revision": "v0"
    },
    "dimensions": 8,
    "normalization": "None",
    "weights": {
      "format": "ProgrammaticFixture",
      "source": "Fixture",
      "quantization": "none",
      "digest": "30a2fd0264ef45e96101268ae97cfbdffb79540210c88ab834117bc0111c0b00",
      "tensors": [
        {
          "name": "bias",
          "shape": {
            "dims": [
              8
            ]
          },
          "dtype": "F32",
          "quantization": "none"
        },
        {
          "name": "projection",
          "shape": {
            "dims": [
              16,
              8
            ]
          },
          "dtype": "F32",
          "quantization": "none"
        }
      ],
      "artifacts": []
    }
  },
  "inputs": [
    "hello world",
    "open agents"
  ]
}"#;
        assert_eq!(encoded, expected);
        Ok(())
    }

    #[test]
    fn embedding_response_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-2",
            sample_embedding_descriptor(),
            vec![String::from("hi")],
        );
        let response = EmbeddingResponse::new(
            &request,
            vec![EmbeddingVector {
                index: 0,
                values: vec![0.0; 8],
            }],
        );

        let encoded = serde_json::to_string(&response)?;
        let decoded: EmbeddingResponse = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, response);
        Ok(())
    }

    #[test]
    fn smoke_embeddings_service_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let mut service = SmokeEmbeddingsService::new()?;
        let request = EmbeddingRequest::new(
            "req-4",
            service.model_descriptor().clone(),
            vec![String::from("hello world")],
        );

        let first = service.embed(&request)?;
        let second = service.embed(&request)?;
        assert_eq!(first, second);
        assert_eq!(first.metadata.dimensions, 8);
        Ok(())
    }

    #[test]
    fn generation_request_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000001")),
            "hello",
            GenerationOptions::greedy(2),
        );

        let encoded = serde_json::to_string_pretty(&request)?;
        let encoded_again = serde_json::to_string_pretty(&request)?;
        assert_eq!(encoded, encoded_again);
        assert!(encoded.contains("\"product_id\": \"mox.text_generation\""));
        assert!(encoded.contains("\"tokenizer_family\": \"fixture_wordpiece\""));
        assert!(encoded.contains("\"decode_strategy\": \"greedy\""));
        Ok(())
    }

    #[test]
    fn generation_response_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_tokens(
            "gen-2",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000002")),
            TokenSequence::new(vec![FixtureWordTokenizer::HELLO_ID]),
            GenerationOptions::greedy(2),
        );
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![
                FixtureWordTokenizer::OPEN_ID,
                FixtureWordTokenizer::AGENTS_ID,
            ]),
            "open agents",
            1,
            3,
            TerminationReason::EndOfSequence,
        );

        let encoded = serde_json::to_string(&response)?;
        let decoded: GenerationResponse = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, response);
        assert_eq!(decoded.usage.output_tokens, 2);
        Ok(())
    }

    #[test]
    fn generation_sessions_isolate_and_reset_kv_cache() -> Result<(), Box<dyn std::error::Error>> {
        let descriptor = sample_decoder_descriptor();
        let mut store = InMemoryGenerationSessionStore::new();
        let session_a = store.create(&descriptor);
        let session_b = store.create(&descriptor);

        store.append(
            &session_a.session_id,
            descriptor.model.model_id.as_str(),
            FixtureWordTokenizer::HELLO_ID,
            vec![1.0; descriptor.config.kv_width()],
            vec![2.0; descriptor.config.kv_width()],
        )?;
        store.append(
            &session_b.session_id,
            descriptor.model.model_id.as_str(),
            FixtureWordTokenizer::RUSTY_ID,
            vec![3.0; descriptor.config.kv_width()],
            vec![4.0; descriptor.config.kv_width()],
        )?;

        assert_eq!(
            store.cache(&session_a.session_id)?.entries()[0].token,
            FixtureWordTokenizer::HELLO_ID
        );
        assert_eq!(
            store.cache(&session_b.session_id)?.entries()[0].token,
            FixtureWordTokenizer::RUSTY_ID
        );

        let reset = store.reset(&session_a.session_id)?;
        assert_eq!(reset.cached_tokens, 0);
        assert!(store.cache(&session_a.session_id)?.is_empty());
        assert_eq!(store.cache(&session_b.session_id)?.len(), 1);

        let closed = store.close(&session_b.session_id)?;
        assert_eq!(closed.cached_tokens, 1);
        assert!(store.cache(&session_b.session_id).is_err());
        Ok(())
    }

    #[test]
    fn model_registry_tracks_active_generation_models() {
        let mut registry = InMemoryGenerationModelRegistry::new();
        let model = ReferenceWordDecoder::new();

        assert!(registry.load(model).is_none());
        assert_eq!(registry.len(), 1);
        assert!(registry.active(ReferenceWordDecoder::MODEL_ID).is_some());
        assert!(registry.unload(ReferenceWordDecoder::MODEL_ID).is_some());
        assert!(registry.is_empty());
    }

    #[test]
    fn cpu_reference_text_generation_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let request = GenerationRequest::new_text(
            "gen-ref-1",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(4),
        );

        let first = service.generate(&request)?;
        let second = service.generate(&request)?;
        assert_eq!(first, second);
        assert_eq!(
            first.output.tokens.as_slice(),
            &[
                FixtureWordTokenizer::OPEN_ID,
                FixtureWordTokenizer::AGENTS_ID,
            ]
        );
        assert_eq!(first.output.text, "open agents");
        assert_eq!(first.termination, TerminationReason::EndOfSequence);
        assert_eq!(first.usage.input_tokens, 2);
        assert!(
            service
                .plan_digest(ReferenceWordDecoder::MODEL_ID)
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_reuses_and_resets_sessions()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let session = service.create_session(ReferenceWordDecoder::MODEL_ID)?;

        let first_request = GenerationRequest::new_text(
            "gen-ref-session-1",
            service.model_descriptor().clone(),
            Some(session.session_id.clone()),
            "hello",
            GenerationOptions::greedy(4),
        );
        let first = service.generate(&first_request)?;
        assert_eq!(first.output.text, "open agents");
        assert_eq!(first.usage.cache_tokens, 4);

        let second_request = GenerationRequest::new_text(
            "gen-ref-session-2",
            service.model_descriptor().clone(),
            Some(session.session_id.clone()),
            "rusty",
            GenerationOptions::greedy(4),
        );
        let second = service.generate(&second_request)?;
        assert_eq!(second.output.text, "grad");
        assert!(second.usage.cache_tokens > first.usage.cache_tokens);

        let reset_request = GenerationRequest::new_text(
            "gen-ref-session-3",
            service.model_descriptor().clone(),
            Some(session.session_id.clone()),
            "rusty",
            GenerationOptions::greedy(4),
        )
        .with_reset_session(true);
        let reset = service.generate(&reset_request)?;
        assert_eq!(reset.output.text, "grad");
        assert_eq!(reset.usage.cache_tokens, 3);
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_rejects_unknown_session() {
        let mut service = CpuReferenceTextGenerationService::new().expect("service");
        let request = GenerationRequest::new_text(
            "gen-ref-bad-session",
            service.model_descriptor().clone(),
            Some(SessionId::new("sess-missing")),
            "hello",
            GenerationOptions::greedy(2),
        );

        let error = service.generate(&request).expect_err("missing session");
        assert!(matches!(
            error,
            ReferenceTextGenerationError::Session(super::SessionStoreError::SessionNotFound(_))
        ));
    }

    fn sample_decoder_descriptor() -> DecoderModelDescriptor {
        DecoderModelDescriptor::new(
            ModelDescriptor::new("fixture-word-decoder-v0", "fixture_decoder", "v0"),
            DecoderConfig {
                hidden_size: 4,
                layer_count: 1,
                vocab_size: 4,
                max_context: 6,
                block: DecoderBlockConfig {
                    attention: DecoderAttentionConfig {
                        head_count: 2,
                        kv_head_count: 2,
                        head_dim: 2,
                        rotary_dim: 0,
                    },
                    feed_forward: DecoderFeedForwardConfig {
                        intermediate_size: 8,
                        activation: ActivationFunction::Identity,
                    },
                },
            },
            "fixture_wordpiece",
            WeightBundleMetadata {
                format: WeightFormat::ProgrammaticFixture,
                source: WeightSource::Fixture,
                quantization: mox_core::QuantizationMode::None,
                digest: String::from("fixture-digest"),
                tensors: vec![WeightTensorMetadata::new(
                    "lm_head",
                    Shape::new(vec![4, 4]),
                    DType::F32,
                )],
                artifacts: Vec::new(),
            },
        )
    }

    fn sample_embedding_descriptor() -> mox_models::EmbeddingModelDescriptor {
        SmokeByteEmbedder::new().descriptor().clone()
    }
}
