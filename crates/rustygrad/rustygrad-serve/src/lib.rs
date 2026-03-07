//! Served compute product contracts for Rustygrad.

use std::collections::BTreeMap;

use rustygrad_backend_cpu::CpuBackend;
use rustygrad_core::{DType, Device, Shape, TensorId};
use rustygrad_ir::{Graph, GraphBuilder, GraphError};
pub use rustygrad_models::{
    ActivationFunction, DecoderAttentionConfig, DecoderBlockConfig, DecoderConfig,
    DecoderFeedForwardConfig, DecoderFixtureWeights, DecoderModelDescriptor, DecoderWeightLoader,
    EmbeddingModelDescriptor, EmbeddingNormalization, FixtureDecoderLoader, FixtureWordTokenizer,
    ModelDescriptor, ModelLoadError, ReferenceWordDecoder, SmokeByteEmbedder, TokenId,
    TokenSequence, TokenVocabulary, TokenizerBoundary, WeightBundleMetadata, WeightFormat,
    WeightSource, WeightTensorMetadata,
};
use rustygrad_runtime::RuntimeError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "request and response types for served products";

/// Phase-0 embeddings product identifier.
pub const EMBEDDINGS_PRODUCT_ID: &str = "rustygrad.embeddings";

/// Phase-1 text-generation product identifier.
pub const TEXT_GENERATION_PRODUCT_ID: &str = "rustygrad.text_generation";

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
    /// Creates an embeddings request for the default Rustygrad product.
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
    #[error("generation session `{session_id}` expects model `{expected_model}` but got `{actual_model}`")]
    ModelMismatch {
        /// Session identifier.
        session_id: String,
        /// Expected model identifier.
        expected_model: String,
        /// Actual model identifier.
        actual_model: String,
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
        let (graph, input_id, output_id) = build_smoke_graph(&model, input_shape.clone())?;
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
        let mut runtime_inputs = BTreeMap::new();
        runtime_inputs.insert(
            self.input_id,
            self.backend
                .input_buffer(self.input_shape.clone(), self.model.featurize(input))?,
        );
        let result = self
            .backend
            .compile_and_execute(&self.graph, &runtime_inputs)?;
        let Some(output) = result.outputs.get(&self.output_id) else {
            return Err(SmokeEmbeddingsError::Runtime(RuntimeError::Backend(
                String::from("missing smoke embedding output"),
            )));
        };
        Ok(output.as_f32_slice().to_vec())
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

fn build_smoke_graph(
    model: &SmokeByteEmbedder,
    input_shape: Shape,
) -> Result<(Graph, TensorId, TensorId), GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("features", input_shape, DType::F32);
    let weights = builder.constant_f32(
        Shape::new(vec![
            model.input_dimensions(),
            model.descriptor().dimensions,
        ]),
        model.projection().to_vec(),
    )?;
    let bias = builder.constant_f32(
        Shape::new(vec![1, model.descriptor().dimensions]),
        model.bias().to_vec(),
    )?;
    let projected = builder.matmul(&input, &weights)?;
    let shifted = builder.add(&projected, &bias)?;
    let output_id = shifted.id();
    let input_id = input.id();
    let graph = builder.finish(vec![shifted]);
    Ok((graph, input_id, output_id))
}

#[cfg(test)]
mod tests {
    use rustygrad_core::{DType, Shape};

    use super::{
        EmbeddingRequest, EmbeddingResponse, EmbeddingVector, EmbeddingsExecutor,
        FixtureWordTokenizer, GenerationOptions, GenerationRequest, GenerationResponse,
        InMemoryGenerationModelRegistry, InMemoryGenerationSessionStore, ModelDescriptor,
        ReferenceWordDecoder, SessionId, SmokeEmbeddingsService, TerminationReason,
        WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
    };
    use crate::{DecoderBlockConfig, DecoderConfig, DecoderModelDescriptor};
    use rustygrad_models::{
        ActivationFunction, DecoderAttentionConfig, DecoderFeedForwardConfig, TokenSequence,
    };

    #[test]
    fn embedding_request_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-1",
            rustygrad_models::EmbeddingModelDescriptor::new(
                ModelDescriptor::new("smoke-byte-embed-v0", "smoke", "v0"),
                8,
                rustygrad_models::EmbeddingNormalization::UnitLength,
            ),
            vec![String::from("hello world"), String::from("open agents")],
        );

        let encoded = serde_json::to_string_pretty(&request)?;
        let expected = r#"{
  "request_id": "req-1",
  "product_id": "rustygrad.embeddings",
  "model": {
    "model": {
      "model_id": "smoke-byte-embed-v0",
      "family": "smoke",
      "revision": "v0"
    },
    "dimensions": 8,
    "normalization": "UnitLength"
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
            rustygrad_models::EmbeddingModelDescriptor::new(
                ModelDescriptor::new("smoke-byte-embed-v0", "smoke", "v0"),
                4,
                rustygrad_models::EmbeddingNormalization::None,
            ),
            vec![String::from("hi")],
        );
        let response = EmbeddingResponse::new(
            &request,
            vec![EmbeddingVector {
                index: 0,
                values: vec![0.0, 1.0, 2.0, 3.0],
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
        assert!(encoded.contains("\"product_id\": \"rustygrad.text_generation\""));
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
                digest: String::from("fixture-digest"),
                tensors: vec![WeightTensorMetadata::new(
                    "lm_head",
                    Shape::new(vec![4, 4]),
                    DType::F32,
                )],
            },
        )
    }
}
