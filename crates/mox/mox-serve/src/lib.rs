//! Served compute product contracts for Mox.

mod conformance;

use std::{
    collections::BTreeMap,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub use conformance::*;
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
    EmbeddingWeights, FixtureDecoderLoader, FixtureWordTokenizer, GgufDecoderAdapter,
    GgufDecoderAdapterLoader, GgufDecoderFamily, GgufDecoderFamilyMetadata,
    GgufDecoderLayerTensorLayout, GgufDecoderTensorLayout, GgufEmbeddingAdapter,
    GgufEmbeddingAdapterLoader, GgufEmbeddingFamily, GgufEmbeddingFamilyMetadata,
    GgufEmbeddingLayerTensorLayout, GgufEmbeddingPooling, GgufEmbeddingTensorLayout,
    GgufPromptTemplateFamily, GgufPromptTemplateRenderer, ModelDescriptor, ModelLoadError,
    PromptMessage, PromptMessageRole, PromptRenderError, ReferenceWordDecoder, RenderedPrompt,
    SmokeByteEmbedder, TokenId, TokenSequence, TokenVocabulary, TokenizerBoundary,
    WeightArtifactMetadata, WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
};
use mox_runtime::{
    BackendSelection, DeviceDiscovery, HealthStatus, KvCacheAccounting, KvCacheDeviceScope,
    KvCachePageLayout, KvCachePolicy, KvCacheSpillPolicy, KvCacheState, LoadedModelResidency,
    RuntimeError,
};
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "request and response types for served products";

/// Phase-0 embeddings product identifier.
pub const EMBEDDINGS_PRODUCT_ID: &str = "mox.embeddings";

/// Phase-1 text-generation product identifier.
pub const TEXT_GENERATION_PRODUCT_ID: &str = "mox.text_generation";

/// Default logical page width for reference-path paged KV state.
pub const DEFAULT_KV_PAGE_TOKENS: usize = 16;

/// Returns the default paged-KV policy for a decoder geometry.
#[must_use]
pub fn default_kv_cache_policy(max_context: usize, width: usize) -> KvCachePolicy {
    let bytes_per_token = width
        .saturating_mul(2)
        .saturating_mul(std::mem::size_of::<f32>());
    KvCachePolicy {
        device_scope: KvCacheDeviceScope::SameDeviceOnly,
        spill_policy: KvCacheSpillPolicy::RefuseNewPages,
        page_layout: KvCachePageLayout::new(
            max_context,
            DEFAULT_KV_PAGE_TOKENS.min(max_context.max(1)),
            bytes_per_token,
        ),
    }
}

/// Returns the default paged-KV policy for a decoder descriptor.
#[must_use]
pub fn default_decoder_kv_cache_policy(model: &DecoderModelDescriptor) -> KvCachePolicy {
    default_kv_cache_policy(model.config.max_context, model.config.kv_width())
}

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

/// Decode strategy for text generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodeStrategy {
    /// Greedy argmax decode.
    Greedy,
    /// Probability sampling with optional temperature, top-k, and top-p controls.
    Sample,
}

/// Generation options for the phase-1 reference path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerationOptions {
    /// Maximum number of output tokens to emit.
    pub max_output_tokens: usize,
    /// Decode strategy.
    pub decode_strategy: DecodeStrategy,
    /// Temperature override for stochastic sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-k sampling cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,
    /// Top-p / nucleus sampling threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Repeat penalty applied to previously seen tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
    /// Presence penalty applied once to previously seen tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// Frequency penalty scaled by prior token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// Deterministic seed for stochastic decode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Explicit stop sequences to truncate from generated text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_sequences: Vec<String>,
}

impl GenerationOptions {
    /// Creates greedy-decode options.
    #[must_use]
    pub fn greedy(max_output_tokens: usize) -> Self {
        Self {
            max_output_tokens,
            decode_strategy: DecodeStrategy::Greedy,
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: None,
            stop_sequences: Vec::new(),
        }
    }

    /// Creates stochastic sampling options with Ollama-aligned default posture.
    #[must_use]
    pub fn sample(max_output_tokens: usize) -> Self {
        Self {
            decode_strategy: DecodeStrategy::Sample,
            ..Self::greedy(max_output_tokens)
        }
    }

    fn effective_temperature(&self) -> f32 {
        self.temperature.unwrap_or(0.8)
    }

    fn effective_top_k(&self) -> Option<usize> {
        self.top_k.or(Some(40))
    }

    fn effective_top_p(&self) -> Option<f32> {
        self.top_p.or(Some(0.9))
    }

    fn effective_repeat_penalty(&self) -> f32 {
        self.repeat_penalty.unwrap_or(1.0)
    }

    fn effective_presence_penalty(&self) -> f32 {
        self.presence_penalty.unwrap_or(0.0)
    }

    fn effective_frequency_penalty(&self) -> f32 {
        self.frequency_penalty.unwrap_or(0.0)
    }
}

/// Text-generation request contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

/// Explicit timing and token metrics for one generation call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GenerationMetrics {
    /// End-to-end generation duration in nanoseconds, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration_ns: Option<u64>,
    /// Model-load or compile duration attributable to this request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration_ns: Option<u64>,
    /// Prompt token count surfaced in the metrics lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<usize>,
    /// Output token count surfaced in the metrics lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<usize>,
    /// Explicit paged-KV accounting for the request, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache: Option<KvCacheAccounting>,
}

/// Whether a request hit a cold or warm model path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationLoadState {
    /// The request consumed a freshly loaded model path.
    Cold,
    /// The request ran against an already-warm model.
    Warm,
}

/// Provenance fields attached to one generation response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationProvenance {
    /// Stable execution-plan digest for the active model graph.
    pub execution_plan_digest: String,
    /// Whether the request took the warm or cold model path.
    pub load_state: GenerationLoadState,
    /// Explicit paged-KV policy for the request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_policy: Option<KvCachePolicy>,
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
    /// Explicit timing and token metrics.
    #[serde(default, skip_serializing_if = "GenerationMetrics::is_empty")]
    pub metrics: GenerationMetrics,
    /// Explicit execution provenance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<GenerationProvenance>,
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
        let usage = GenerationUsage {
            input_tokens,
            output_tokens,
            cache_tokens,
        };
        Self {
            request_id: request.request_id.clone(),
            product_id: request.product_id.clone(),
            model_id: request.model.model.model_id.clone(),
            session_id,
            output: GenerationOutput {
                tokens,
                text: text.into(),
            },
            metrics: GenerationMetrics::from_usage(&usage),
            usage,
            provenance: None,
            termination,
        }
    }

    /// Attaches explicit metrics and provenance to a generation response.
    #[must_use]
    pub fn with_metrics_and_provenance(
        mut self,
        metrics: GenerationMetrics,
        provenance: GenerationProvenance,
    ) -> Self {
        self.metrics = metrics;
        self.provenance = Some(provenance);
        self
    }
}

impl GenerationMetrics {
    #[must_use]
    fn from_usage(usage: &GenerationUsage) -> Self {
        Self {
            total_duration_ns: None,
            load_duration_ns: None,
            prompt_eval_count: Some(usage.input_tokens),
            eval_count: Some(usage.output_tokens),
            kv_cache: None,
        }
    }

    #[must_use]
    fn is_empty(&self) -> bool {
        self.total_duration_ns.is_none()
            && self.load_duration_ns.is_none()
            && self.prompt_eval_count.is_none()
            && self.eval_count.is_none()
            && self.kv_cache.is_none()
    }
}

/// Minimal text-generation execution interface.
pub trait TextGenerationExecutor {
    /// Error returned when generation fails.
    type Error;

    /// Executes a text-generation request.
    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error>;
}

/// Library-first catalog surface for local installed-model inspection.
pub trait LocalModelCatalog {
    /// Returns the local installed-model observation.
    fn list_models(&self) -> ListModelsObservation;

    /// Returns the local model-inspection observation for one model name.
    fn show_model(&self, model: &str) -> ShowObservation;
}

/// Library-first generation surface that also exposes local model lifecycle.
pub trait ManagedTextGenerationRuntime: TextGenerationExecutor {
    /// Returns the current loaded-model observation.
    fn loaded_models(&mut self) -> LoadedModelsObservation;

    /// Refreshes or overrides keepalive for one already-loaded model.
    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, Self::Error>;

    /// Unloads one currently loaded model.
    fn unload_model(&mut self, model_id: &str) -> Result<LoadedModelView, Self::Error>;
}

/// Library-first aggregate runtime boundary over catalog, generation, and embeddings.
///
/// This wrapper is intentionally thin: it forwards to existing reusable Mox
/// surfaces so downstream code can depend on one in-process library API
/// without reaching through multiple crates or speaking Ollama HTTP.
#[derive(Clone, Debug)]
pub struct MoxLocalRuntime<C, G, E> {
    catalog: C,
    generation: G,
    embeddings: E,
}

impl<C, G, E> MoxLocalRuntime<C, G, E> {
    /// Creates a new aggregate local runtime.
    #[must_use]
    pub fn new(catalog: C, generation: G, embeddings: E) -> Self {
        Self {
            catalog,
            generation,
            embeddings,
        }
    }

    /// Returns the underlying catalog surface.
    #[must_use]
    pub fn catalog(&self) -> &C {
        &self.catalog
    }

    /// Returns the underlying generation surface.
    #[must_use]
    pub fn generation(&self) -> &G {
        &self.generation
    }

    /// Returns the underlying embeddings surface.
    #[must_use]
    pub fn embeddings(&self) -> &E {
        &self.embeddings
    }

    /// Returns mutable access to the underlying generation surface.
    pub fn generation_mut(&mut self) -> &mut G {
        &mut self.generation
    }

    /// Returns mutable access to the underlying embeddings surface.
    pub fn embeddings_mut(&mut self) -> &mut E {
        &mut self.embeddings
    }

    /// Consumes the aggregate runtime and returns its component parts.
    #[must_use]
    pub fn into_parts(self) -> (C, G, E) {
        (self.catalog, self.generation, self.embeddings)
    }
}

impl<C, G, E> MoxLocalRuntime<C, G, E>
where
    C: LocalModelCatalog,
    G: ManagedTextGenerationRuntime,
    E: EmbeddingsExecutor,
{
    /// Returns the installed-model observation through the catalog surface.
    #[must_use]
    pub fn list_models(&self) -> ListModelsObservation {
        self.catalog.list_models()
    }

    /// Returns model inspection through the catalog surface.
    #[must_use]
    pub fn show_model(&self, model: &str) -> ShowObservation {
        self.catalog.show_model(model)
    }

    /// Returns the currently loaded-model observation.
    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        self.generation.loaded_models()
    }

    /// Refreshes keepalive for one loaded generation model.
    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, G::Error> {
        self.generation.warm_model(model_id, keep_alive_millis)
    }

    /// Unloads one loaded generation model.
    pub fn unload_model(&mut self, model_id: &str) -> Result<LoadedModelView, G::Error> {
        self.generation.unload_model(model_id)
    }

    /// Executes a text-generation request through the managed generation surface.
    pub fn generate(
        &mut self,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse, G::Error> {
        self.generation.generate(request)
    }

    /// Executes an embeddings request through the configured embeddings surface.
    pub fn embed(&mut self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, E::Error> {
        self.embeddings.embed(request)
    }
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

/// Default local keepalive used by the in-process serve services.
pub const DEFAULT_MODEL_KEEPALIVE_MILLIS: u64 = 300_000;

/// Loaded-model view exposed by the serve layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedModelView {
    /// Comparable loaded-model summary.
    pub summary: LoadedModelSummary,
    /// Explicit residency and keepalive truth.
    pub residency: LoadedModelResidency,
}

#[derive(Clone, Debug)]
struct LoadedGenerationModel<M> {
    model: M,
    residency: LoadedModelResidency,
    has_served_request: bool,
    size_bytes: Option<u64>,
    size_vram_bytes: Option<u64>,
    backend: Option<String>,
    fallback_state: Option<String>,
}

impl<M> LoadedGenerationModel<M>
where
    M: GenerationModelHandle,
{
    fn view(&self) -> LoadedModelView {
        let descriptor = self.model.descriptor();
        let mut summary = LoadedModelSummary::from_decoder_descriptor(
            descriptor.model.model_id.clone(),
            descriptor,
        );
        summary.size_bytes = self.size_bytes;
        summary.size_vram_bytes = self.size_vram_bytes;
        summary.backend = self.backend.clone();
        summary.fallback_state = self.fallback_state.clone();
        LoadedModelView {
            summary,
            residency: self.residency.clone(),
        }
    }
}

/// Loaded-model registry failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LoadedModelRegistryError {
    /// The requested model is not currently loaded.
    #[error("loaded model `{0}` was not found")]
    ModelNotLoaded(String),
}

/// In-memory registry of loaded generation models plus keepalive/lifecycle truth.
#[derive(Clone, Debug, Default)]
pub struct InMemoryGenerationModelRegistry<M> {
    models: BTreeMap<String, LoadedGenerationModel<M>>,
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

    /// Loads or replaces a model handle by model ID using the default keepalive.
    pub fn load(&mut self, model: M) -> Option<M> {
        self.warm_with_metadata(model, 0, DEFAULT_MODEL_KEEPALIVE_MILLIS, None, None, None)
    }

    /// Loads or refreshes a model with explicit keepalive and runtime metadata.
    pub fn warm_with_metadata(
        &mut self,
        model: M,
        now_millis: u64,
        keep_alive_millis: u64,
        size_vram_bytes: Option<u64>,
        backend: Option<String>,
        fallback_state: Option<String>,
    ) -> Option<M> {
        let model_id = model.descriptor().model.model_id.clone();
        let size_bytes = weight_bundle_size_bytes(&model.descriptor().weights);
        self.models
            .insert(
                model_id,
                LoadedGenerationModel {
                    model,
                    residency: LoadedModelResidency::ready(now_millis, keep_alive_millis),
                    has_served_request: false,
                    size_bytes,
                    size_vram_bytes,
                    backend,
                    fallback_state,
                },
            )
            .map(|previous| previous.model)
    }

    /// Refreshes an already-loaded model's keepalive window.
    pub fn warm_loaded(
        &mut self,
        model_id: &str,
        now_millis: u64,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, LoadedModelRegistryError> {
        let entry = self
            .models
            .get_mut(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?;
        entry
            .residency
            .refresh_keep_alive(keep_alive_millis, now_millis);
        Ok(entry.view())
    }

    /// Marks a request as actively using the model.
    pub fn begin_request(
        &mut self,
        model_id: &str,
        now_millis: u64,
    ) -> Result<LoadedModelView, LoadedModelRegistryError> {
        let entry = self
            .models
            .get_mut(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?;
        entry.has_served_request = true;
        entry.residency.begin_request(now_millis);
        Ok(entry.view())
    }

    /// Marks a request as finished and refreshes idle expiry.
    pub fn finish_request(
        &mut self,
        model_id: &str,
        now_millis: u64,
    ) -> Result<LoadedModelView, LoadedModelRegistryError> {
        let entry = self
            .models
            .get_mut(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?;
        entry.residency.finish_request(now_millis);
        Ok(entry.view())
    }

    /// Unloads an active model and returns the final loaded-model view.
    pub fn unload_view(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, LoadedModelRegistryError> {
        let entry = self
            .models
            .remove(model_id)
            .ok_or_else(|| LoadedModelRegistryError::ModelNotLoaded(model_id.to_string()))?;
        Ok(entry.view())
    }

    /// Returns an active model by ID.
    #[must_use]
    pub fn active(&self, model_id: &str) -> Option<&M> {
        self.models.get(model_id).map(|entry| &entry.model)
    }

    /// Returns a mutable active model by ID.
    pub fn active_mut(&mut self, model_id: &str) -> Option<&mut M> {
        self.models.get_mut(model_id).map(|entry| &mut entry.model)
    }

    /// Unloads an active model.
    pub fn unload(&mut self, model_id: &str) -> Option<M> {
        self.models.remove(model_id).map(|entry| entry.model)
    }

    /// Unloads any idle models whose keepalive has expired.
    pub fn expire_idle(&mut self, now_millis: u64) -> Vec<M> {
        let expired = self
            .models
            .iter()
            .filter(|(_, entry)| entry.residency.is_expired(now_millis))
            .map(|(model_id, _)| model_id.clone())
            .collect::<Vec<_>>();
        let mut removed = Vec::with_capacity(expired.len());
        for model_id in expired {
            if let Some(entry) = self.models.remove(model_id.as_str()) {
                removed.push(entry.model);
            }
        }
        removed
    }

    /// Returns loaded-model views in stable `ps` order.
    #[must_use]
    pub fn loaded_model_views(&self) -> Vec<LoadedModelView> {
        let mut views = self
            .models
            .values()
            .map(LoadedGenerationModel::view)
            .collect::<Vec<_>>();
        views.sort_by(|left, right| {
            right
                .residency
                .expires_at_millis
                .unwrap_or(u64::MAX)
                .cmp(&left.residency.expires_at_millis.unwrap_or(u64::MAX))
                .then_with(|| left.summary.model.cmp(&right.summary.model))
        });
        views
    }

    /// Returns the comparable loaded-model observation used by `ps` parity.
    #[must_use]
    pub fn loaded_models_observation(&self) -> LoadedModelsObservation {
        LoadedModelsObservation::new(
            self.loaded_model_views()
                .into_iter()
                .map(|view| view.summary)
                .collect(),
        )
    }

    #[must_use]
    fn load_state(&self, model_id: &str) -> Option<GenerationLoadState> {
        self.models.get(model_id).map(|entry| {
            if entry.has_served_request {
                GenerationLoadState::Warm
            } else {
                GenerationLoadState::Cold
            }
        })
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

fn weight_bundle_size_bytes(weights: &WeightBundleMetadata) -> Option<u64> {
    (!weights.artifacts.is_empty()).then_some(
        weights
            .artifacts
            .iter()
            .map(|artifact| artifact.byte_length)
            .sum(),
    )
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

impl LocalModelCatalog for LocalOllamaCatalogSubject {
    fn list_models(&self) -> ListModelsObservation {
        self.list_models_observation()
    }

    fn show_model(&self, model: &str) -> ShowObservation {
        self.show_model_observation(model)
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
    /// A KV append would require more pages than the cache policy admits.
    #[error(
        "session KV cache refused growth at token {requested_tokens}: max_context={max_context} max_pages={max_pages} spill_policy={spill_policy:?}"
    )]
    PageBudgetExceeded {
        /// Token count the append would have produced.
        requested_tokens: usize,
        /// Maximum cache size in tokens.
        max_context: usize,
        /// Maximum admitted page count.
        max_pages: usize,
        /// Explicit spill/refusal posture.
        spill_policy: KvCacheSpillPolicy,
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

/// In-memory per-session KV cache with an explicit logical page layout.
#[derive(Clone, Debug, PartialEq)]
pub struct InMemoryKvCache {
    max_context: usize,
    width: usize,
    policy: KvCachePolicy,
    entries: Vec<KvCacheEntry>,
}

impl InMemoryKvCache {
    /// Creates an empty in-memory KV cache.
    #[must_use]
    pub fn new(max_context: usize, width: usize) -> Self {
        Self::with_policy(
            max_context,
            width,
            default_kv_cache_policy(max_context, width),
        )
    }

    /// Creates an empty in-memory cache with an explicit paged-KV policy.
    #[must_use]
    pub fn with_policy(max_context: usize, width: usize, policy: KvCachePolicy) -> Self {
        Self {
            max_context,
            width,
            policy,
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

    /// Returns the explicit paged-KV policy for the cache.
    #[must_use]
    pub fn policy(&self) -> &KvCachePolicy {
        &self.policy
    }

    /// Returns the logical page layout for the cache.
    #[must_use]
    pub fn page_layout(&self) -> &KvCachePageLayout {
        &self.policy.page_layout
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

    /// Returns the current paged-KV snapshot for the cache.
    #[must_use]
    pub fn state(&self) -> KvCacheState {
        KvCacheState::paged(self.page_layout(), self.len())
    }

    /// Appends a token KV pair to the cache.
    pub fn append(
        &mut self,
        token: TokenId,
        key: Vec<f32>,
        value: Vec<f32>,
    ) -> Result<(), KvCacheError> {
        if self.entries.len() >= self.max_context {
            return Err(KvCacheError::PageBudgetExceeded {
                requested_tokens: self.entries.len().saturating_add(1),
                max_context: self.max_context,
                max_pages: self.page_layout().max_pages,
                spill_policy: self.policy.spill_policy,
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
    /// Bound model family.
    pub model_family: String,
    /// Bound model revision.
    pub model_revision: String,
    /// Stable weight-bundle digest that owns the session KV state.
    pub weight_bundle_digest: String,
    /// Maximum context for the session cache.
    pub max_context: usize,
    /// KV vector width.
    pub kv_width: usize,
    /// Current cached token count.
    pub cached_tokens: usize,
    /// Explicit paged-KV policy for the session cache.
    pub kv_cache_policy: KvCachePolicy,
    /// Current paged-KV snapshot for the session cache.
    pub kv_cache: KvCacheState,
}

/// Session state stored in memory.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationSessionState {
    session: GenerationSession,
    cache: InMemoryKvCache,
    tokens: Vec<TokenId>,
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

    /// Returns the tokens currently owned by the session.
    #[must_use]
    pub fn tokens(&self) -> &[TokenId] {
        &self.tokens
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
        "generation session `{session_id}` expects model `{expected_model}` revision `{expected_revision}` bundle `{expected_weight_bundle_digest}` but got model `{actual_model}` revision `{actual_revision}` bundle `{actual_weight_bundle_digest}`"
    )]
    ModelMismatch {
        /// Session identifier.
        session_id: String,
        /// Expected model identifier.
        expected_model: String,
        /// Expected model revision.
        expected_revision: String,
        /// Expected weight-bundle digest.
        expected_weight_bundle_digest: String,
        /// Actual model identifier.
        actual_model: String,
        /// Actual model revision.
        actual_revision: String,
        /// Actual weight-bundle digest.
        actual_weight_bundle_digest: String,
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
        let policy = default_decoder_kv_cache_policy(model);
        let session = GenerationSession {
            session_id: session_id.clone(),
            model_id: model.model.model_id.clone(),
            model_family: model.model.family.clone(),
            model_revision: model.model.revision.clone(),
            weight_bundle_digest: model.weights.digest.clone(),
            max_context: model.config.max_context,
            kv_width: model.config.kv_width(),
            cached_tokens: 0,
            kv_cache_policy: policy.clone(),
            kv_cache: KvCacheState::default(),
        };
        let state = GenerationSessionState {
            session: session.clone(),
            cache: InMemoryKvCache::with_policy(
                model.config.max_context,
                model.config.kv_width(),
                policy,
            ),
            tokens: Vec::new(),
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
        model: &DecoderModelDescriptor,
        token: TokenId,
        key: Vec<f32>,
        value: Vec<f32>,
    ) -> Result<GenerationSession, SessionStoreError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))?;
        validate_session_model(state, session_id, model)?;

        state.cache.append(token, key, value)?;
        state.tokens.push(token);
        sync_session_cache_state(&mut state.session, &state.cache);
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
        state.tokens.clear();
        sync_session_cache_state(&mut state.session, &state.cache);
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
        model: &DecoderModelDescriptor,
        cache: InMemoryKvCache,
        tokens: TokenSequence,
    ) -> Result<GenerationSession, SessionStoreError> {
        let state = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionStoreError::SessionNotFound(session_id.as_str().to_string()))?;
        validate_session_model(state, session_id, model)?;
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
        state.tokens = tokens.as_slice().to_vec();
        sync_session_cache_state(&mut state.session, &state.cache);
        Ok(state.session.clone())
    }
}

fn sync_session_cache_state(session: &mut GenerationSession, cache: &InMemoryKvCache) {
    session.cached_tokens = cache.len();
    session.kv_cache_policy = cache.policy().clone();
    session.kv_cache = cache.state();
}

fn validate_session_model(
    state: &GenerationSessionState,
    session_id: &SessionId,
    model: &DecoderModelDescriptor,
) -> Result<(), SessionStoreError> {
    if state.session.model_id != model.model.model_id
        || state.session.model_revision != model.model.revision
        || state.session.weight_bundle_digest != model.weights.digest
    {
        return Err(SessionStoreError::ModelMismatch {
            session_id: session_id.as_str().to_string(),
            expected_model: state.session.model_id.clone(),
            expected_revision: state.session.model_revision.clone(),
            expected_weight_bundle_digest: state.session.weight_bundle_digest.clone(),
            actual_model: model.model.model_id.clone(),
            actual_revision: model.model.revision.clone(),
            actual_weight_bundle_digest: model.weights.digest.clone(),
        });
    }
    Ok(())
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
    /// Loaded-model registry operations failed.
    #[error(transparent)]
    LoadedModelRegistry(#[from] LoadedModelRegistryError),
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
    load_duration_ns: u64,
}

impl<M> CpuWordGenerationModel<M>
where
    M: WordDecoderExecutionModel,
{
    /// Loads and compiles a decoder model.
    fn new(model: M) -> Result<Self, ReferenceTextGenerationError> {
        let load_start = Instant::now();
        let (
            graph,
            token_input_id,
            position_input_id,
            context_input_id,
            hidden_output_id,
            logits_output_id,
        ) = build_generation_graph(&model)?;
        let plan_digest = compile_graph(&graph)?.stable_digest();
        let load_duration_ns = load_start
            .elapsed()
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX);
        Ok(Self {
            model,
            graph,
            token_input_id,
            position_input_id,
            context_input_id,
            hidden_output_id,
            logits_output_id,
            plan_digest,
            load_duration_ns,
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

    #[must_use]
    fn load_duration_ns(&self) -> u64 {
        self.load_duration_ns
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
        models.warm_with_metadata(
            CpuReferenceGenerationModel::new(model)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        );
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
        self.models.warm_with_metadata(
            CpuReferenceGenerationModel::new(model)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        );
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

    /// Refreshes keepalive for an already loaded model.
    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self
            .models
            .warm_loaded(model_id, current_time_millis(), keep_alive_millis)?)
    }

    /// Returns the currently loaded models after applying idle expiry.
    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        self.loaded_models_at(current_time_millis())
    }

    /// Returns the currently loaded models at a caller-provided time.
    #[must_use]
    pub fn loaded_models_at(&mut self, now_millis: u64) -> LoadedModelsObservation {
        self.models.expire_idle(now_millis);
        self.models.loaded_models_observation()
    }

    /// Returns explicit loaded-model residency views at a caller-provided time.
    #[must_use]
    pub fn loaded_model_views_at(&mut self, now_millis: u64) -> Vec<LoadedModelView> {
        self.models.expire_idle(now_millis);
        self.models.loaded_model_views()
    }

    /// Returns explicit loaded-model residency views after applying idle expiry.
    #[must_use]
    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        self.loaded_model_views_at(current_time_millis())
    }

    /// Unloads a currently loaded model explicitly.
    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self.models.unload_view(model_id)?)
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
        run_generation_request(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            request,
        )
    }
}

impl ManagedTextGenerationRuntime for CpuReferenceTextGenerationService {
    fn loaded_models(&mut self) -> LoadedModelsObservation {
        CpuReferenceTextGenerationService::loaded_models(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, Self::Error> {
        CpuReferenceTextGenerationService::warm_model(self, model_id, keep_alive_millis)
    }

    fn unload_model(&mut self, model_id: &str) -> Result<LoadedModelView, Self::Error> {
        CpuReferenceTextGenerationService::unload_model(self, model_id)
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
        models.warm_with_metadata(
            CpuModelGenerationModel::new(model)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        );
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
        self.models.warm_with_metadata(
            CpuModelGenerationModel::new(model)?,
            current_time_millis(),
            DEFAULT_MODEL_KEEPALIVE_MILLIS,
            None,
            Some(String::from("cpu")),
            None,
        );
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

    /// Refreshes keepalive for an already loaded model.
    pub fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self
            .models
            .warm_loaded(model_id, current_time_millis(), keep_alive_millis)?)
    }

    /// Returns the currently loaded models after applying idle expiry.
    #[must_use]
    pub fn loaded_models(&mut self) -> LoadedModelsObservation {
        self.loaded_models_at(current_time_millis())
    }

    /// Returns the currently loaded models at a caller-provided time.
    #[must_use]
    pub fn loaded_models_at(&mut self, now_millis: u64) -> LoadedModelsObservation {
        self.models.expire_idle(now_millis);
        self.models.loaded_models_observation()
    }

    /// Returns explicit loaded-model residency views at a caller-provided time.
    #[must_use]
    pub fn loaded_model_views_at(&mut self, now_millis: u64) -> Vec<LoadedModelView> {
        self.models.expire_idle(now_millis);
        self.models.loaded_model_views()
    }

    /// Returns explicit loaded-model residency views after applying idle expiry.
    #[must_use]
    pub fn loaded_model_views(&mut self) -> Vec<LoadedModelView> {
        self.loaded_model_views_at(current_time_millis())
    }

    /// Unloads a currently loaded model explicitly.
    pub fn unload_model(
        &mut self,
        model_id: &str,
    ) -> Result<LoadedModelView, ReferenceTextGenerationError> {
        Ok(self.models.unload_view(model_id)?)
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
        run_generation_request(
            &mut self.backend,
            &mut self.models,
            &mut self.sessions,
            request,
        )
    }
}

impl ManagedTextGenerationRuntime for CpuModelTextGenerationService {
    fn loaded_models(&mut self) -> LoadedModelsObservation {
        CpuModelTextGenerationService::loaded_models(self)
    }

    fn warm_model(
        &mut self,
        model_id: &str,
        keep_alive_millis: u64,
    ) -> Result<LoadedModelView, Self::Error> {
        CpuModelTextGenerationService::warm_model(self, model_id, keep_alive_millis)
    }

    fn unload_model(&mut self, model_id: &str) -> Result<LoadedModelView, Self::Error> {
        CpuModelTextGenerationService::unload_model(self, model_id)
    }
}

/// Honest CPU product alias for model-backed text generation.
pub type CpuProductTextGenerationService = CpuModelTextGenerationService;

struct GenerationSampler {
    options: GenerationOptions,
    rng: StdRng,
}

impl GenerationSampler {
    fn new(options: &GenerationOptions) -> Self {
        let rng = options
            .seed
            .map_or_else(StdRng::from_os_rng, StdRng::seed_from_u64);
        Self {
            options: options.clone(),
            rng,
        }
    }

    fn select_next_token(&mut self, logits: &[f32], cache: &InMemoryKvCache) -> Option<TokenId> {
        let mut adjusted_logits = logits.to_vec();
        apply_generation_penalties(&mut adjusted_logits, cache, &self.options);
        if self.options.decode_strategy == DecodeStrategy::Greedy
            || self.options.effective_temperature() <= 1e-6
        {
            return select_argmax(&adjusted_logits);
        }
        sample_next_token(&mut self.rng, &adjusted_logits, &self.options)
    }
}

fn run_generation_request<M>(
    backend: &mut CpuBackend,
    models: &mut InMemoryGenerationModelRegistry<CpuWordGenerationModel<M>>,
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

    let model_id = request.model.model.model_id.as_str();
    let load_state = models
        .load_state(model_id)
        .unwrap_or(GenerationLoadState::Warm);
    let request_start = current_time_millis();
    let generation_start = Instant::now();
    models.begin_request(model_id, request_start)?;

    let result = (|| -> Result<GenerationResponse, ReferenceTextGenerationError> {
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
        let mut session_tokens = Vec::new();
        let previous_kv_state = if let Some(session_id) = &request.session_id {
            if request.reset_session {
                sessions.reset(session_id)?;
            }
            let state = sessions.state(session_id)?;
            validate_session_model(state, session_id, loaded_model.descriptor())?;
            session_tokens = state.tokens().to_vec();
            state.cache().state()
        } else {
            KvCacheState::default()
        };
        let mut cache = if let Some(session_id) = &request.session_id {
            sessions.state(session_id)?.cache().clone()
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
        let mut sampler = GenerationSampler::new(&request.options);
        let mut generated_tokens = Vec::new();
        let termination = loop {
            if generated_tokens.len() >= request.options.max_output_tokens {
                break TerminationReason::MaxOutputTokens;
            }
            if cache.len() >= cache.max_context() {
                break TerminationReason::ContextLimit;
            }

            let next_token = sampler
                .select_next_token(&last_logits, &cache)
                .ok_or(ReferenceTextGenerationError::MissingOutput("next_token"))?;
            if next_token == eos_id {
                break TerminationReason::EndOfSequence;
            }

            generated_tokens.push(next_token);
            let context = mean_cache_value(&cache, hidden_size);
            let step = loaded_model.execute_step(backend, next_token, cache.len(), &context)?;
            cache.append(next_token, step.hidden.clone(), step.hidden)?;
            last_logits = step.logits;

            if truncate_generated_text(
                loaded_model.model().tokenizer(),
                &mut generated_tokens,
                &request.options.stop_sequences,
            )
            .is_some()
            {
                break TerminationReason::EndOfSequence;
            }
        };

        let generated = TokenSequence::new(generated_tokens);
        if let Some(session_id) = &request.session_id {
            session_tokens.extend_from_slice(prompt_tokens.as_slice());
            session_tokens.extend_from_slice(generated.as_slice());
            sessions.replace_cache(
                session_id,
                loaded_model.descriptor(),
                cache.clone(),
                TokenSequence::new(session_tokens),
            )?;
        }
        let text = loaded_model
            .model()
            .tokenizer()
            .decode(generated.as_slice());
        let usage = GenerationUsage {
            input_tokens: prompt_tokens.len(),
            output_tokens: generated.len(),
            cache_tokens: cache.len(),
        };
        let kv_cache = KvCacheAccounting::from_states(&previous_kv_state, cache.state());
        let metrics = GenerationMetrics {
            total_duration_ns: Some(
                generation_start
                    .elapsed()
                    .as_nanos()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            load_duration_ns: Some(match load_state {
                GenerationLoadState::Cold => loaded_model.load_duration_ns(),
                GenerationLoadState::Warm => 0,
            }),
            prompt_eval_count: Some(usage.input_tokens),
            eval_count: Some(usage.output_tokens),
            kv_cache: Some(kv_cache),
        };
        let provenance = GenerationProvenance {
            execution_plan_digest: loaded_model.plan_digest().to_string(),
            load_state,
            kv_cache_policy: Some(cache.policy().clone()),
        };
        Ok(GenerationResponse::new(
            request,
            request.session_id.clone(),
            generated,
            text,
            usage.input_tokens,
            usage.cache_tokens,
            termination,
        )
        .with_metrics_and_provenance(metrics, provenance))
    })();

    let _ = models.finish_request(model_id, current_time_millis());
    result
}

fn apply_generation_penalties(
    logits: &mut [f32],
    cache: &InMemoryKvCache,
    options: &GenerationOptions,
) {
    let repeat_penalty = options.effective_repeat_penalty();
    let presence_penalty = options.effective_presence_penalty();
    let frequency_penalty = options.effective_frequency_penalty();
    if (repeat_penalty - 1.0).abs() <= f32::EPSILON
        && presence_penalty.abs() <= f32::EPSILON
        && frequency_penalty.abs() <= f32::EPSILON
    {
        return;
    }

    let mut counts = BTreeMap::<u32, usize>::new();
    for entry in cache.entries() {
        *counts.entry(entry.token.as_u32()).or_default() += 1;
    }

    for (token_id, count) in counts {
        let Some(logit) = logits.get_mut(token_id as usize) else {
            continue;
        };
        if repeat_penalty > 0.0 && (repeat_penalty - 1.0).abs() > f32::EPSILON {
            if *logit >= 0.0 {
                *logit /= repeat_penalty;
            } else {
                *logit *= repeat_penalty;
            }
        }
        if presence_penalty.abs() > f32::EPSILON {
            *logit -= presence_penalty;
        }
        if frequency_penalty.abs() > f32::EPSILON {
            *logit -= frequency_penalty * (count as f32);
        }
    }
}

fn sample_next_token(
    rng: &mut StdRng,
    logits: &[f32],
    options: &GenerationOptions,
) -> Option<TokenId> {
    let temperature = options.effective_temperature();
    if temperature <= 1e-6 {
        return select_argmax(logits);
    }

    let max_logit = logits.iter().copied().max_by(f32::total_cmp)?;
    let mut probabilities = logits
        .iter()
        .enumerate()
        .map(|(index, logit)| (index, ((*logit - max_logit) / temperature).exp()))
        .collect::<Vec<_>>();
    probabilities.sort_by(|left, right| right.1.total_cmp(&left.1));

    if let Some(top_k) = options.effective_top_k() {
        if top_k > 0 && top_k < probabilities.len() {
            probabilities.truncate(top_k);
        }
    }

    if let Some(top_p) = options.effective_top_p() {
        if top_p > 0.0 && top_p < 1.0 {
            let total = probabilities.iter().map(|(_, value)| *value).sum::<f32>();
            let mut cumulative = 0.0;
            let mut truncated = Vec::new();
            for (index, probability) in probabilities {
                cumulative += probability / total.max(f32::EPSILON);
                truncated.push((index, probability));
                if cumulative >= top_p {
                    break;
                }
            }
            probabilities = truncated;
        }
    }

    let total = probabilities.iter().map(|(_, value)| *value).sum::<f32>();
    if total <= 0.0 {
        return select_argmax(logits);
    }

    let mut target = rng.random::<f32>() * total;
    for (index, probability) in &probabilities {
        target -= *probability;
        if target <= 0.0 {
            return Some(TokenId(*index as u32));
        }
    }
    probabilities
        .last()
        .map(|(index, _)| TokenId(*index as u32))
}

fn truncate_generated_text(
    tokenizer: &FixtureWordTokenizer,
    generated_tokens: &mut Vec<TokenId>,
    stop_sequences: &[String],
) -> Option<String> {
    if stop_sequences.is_empty() {
        return None;
    }

    let text = tokenizer.decode(generated_tokens);
    let stop_index = stop_sequences
        .iter()
        .filter(|stop| !stop.is_empty())
        .filter_map(|stop| text.find(stop))
        .min()?;
    let truncated = text[..stop_index].trim_end().to_string();
    *generated_tokens = tokenizer.encode(truncated.as_str()).as_slice().to_vec();
    Some(truncated)
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
    use mox_runtime::{
        KvCacheAccounting, KvCacheDeviceScope, KvCachePageLayout, KvCachePolicy,
        KvCacheSpillPolicy, KvCacheState, LoadedModelState,
    };

    use super::{
        CpuReferenceTextGenerationService, EmbeddingRequest, EmbeddingResponse, EmbeddingVector,
        EmbeddingsExecutor, FixtureWordTokenizer, GenerationLoadState, GenerationModelHandle,
        GenerationOptions, GenerationRequest, GenerationResponse, InMemoryGenerationModelRegistry,
        InMemoryGenerationSessionStore, InMemoryKvCache, KvCacheError, ListModelsObservation,
        LocalModelCatalog, ModelDescriptor, ModelSummary, MoxLocalRuntime,
        ReferenceTextGenerationError, ReferenceWordDecoder, SessionId, ShowObservation,
        SmokeByteEmbedder, SmokeEmbeddingsService, TerminationReason, TextGenerationExecutor,
        TokenId, WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
    };
    use crate::{DecoderBlockConfig, DecoderConfig, DecoderModelDescriptor};
    use mox_models::{
        ActivationFunction, DecoderAttentionConfig, DecoderFeedForwardConfig, TokenSequence,
        assert_prompt_window_case, assert_rendered_prompt_case, golden_prompt_fixture,
        golden_prompt_fixtures, golden_tokenizer_fixture,
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
        assert_eq!(decoded.metrics.prompt_eval_count, Some(1));
        assert_eq!(decoded.metrics.eval_count, Some(2));
        assert_eq!(decoded.metrics.total_duration_ns, None);
        assert_eq!(decoded.metrics.load_duration_ns, None);
        assert_eq!(decoded.provenance, None);
        Ok(())
    }

    #[test]
    fn generation_sampling_options_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let options = GenerationOptions {
            max_output_tokens: 16,
            decode_strategy: super::DecodeStrategy::Sample,
            temperature: Some(0.7),
            top_k: Some(32),
            top_p: Some(0.85),
            repeat_penalty: Some(1.2),
            presence_penalty: Some(0.4),
            frequency_penalty: Some(0.3),
            seed: Some(17),
            stop_sequences: vec![String::from("</end>"), String::from("STOP")],
        };

        let encoded = serde_json::to_value(&options)?;
        assert_eq!(encoded["decode_strategy"], "sample");
        assert!(
            (encoded["temperature"].as_f64().expect("temperature") - 0.7).abs() < 1e-6,
            "temperature should round-trip as 0.7"
        );
        assert_eq!(encoded["top_k"], 32);
        assert!((encoded["top_p"].as_f64().expect("top_p") - 0.85).abs() < 1e-6);
        assert!((encoded["repeat_penalty"].as_f64().expect("repeat_penalty") - 1.2).abs() < 1e-6);
        assert!(
            (encoded["presence_penalty"]
                .as_f64()
                .expect("presence_penalty")
                - 0.4)
                .abs()
                < 1e-6
        );
        assert!(
            (encoded["frequency_penalty"]
                .as_f64()
                .expect("frequency_penalty")
                - 0.3)
                .abs()
                < 1e-6
        );
        assert_eq!(encoded["seed"], 17);
        assert_eq!(
            encoded["stop_sequences"],
            serde_json::json!(["</end>", "STOP"])
        );
        Ok(())
    }

    #[test]
    fn golden_fixture_corpus_is_visible_from_serve() {
        assert_eq!(golden_prompt_fixtures().len(), 4);
        let tokenizer = golden_tokenizer_fixture("qwen2").expect("qwen2 fixture");
        assert_eq!(tokenizer.family, "qwen2");
    }

    #[test]
    fn golden_prompt_render_cases_are_reusable_from_serve() -> Result<(), Box<dyn std::error::Error>>
    {
        let fixture = golden_prompt_fixture("qwen2").expect("qwen2 fixture");
        let variant = fixture.template_variant("qwen2.default").expect("variant");
        let render_case = variant
            .render_case("qwen2.with_system_history")
            .expect("render case");

        assert_rendered_prompt_case(render_case, render_case.expected_rendered)?;
        Ok(())
    }

    #[test]
    fn golden_prompt_window_cases_are_reusable_from_serve() -> Result<(), Box<dyn std::error::Error>>
    {
        let fixture = golden_prompt_fixture("command_r").expect("command-r fixture");
        let variant = fixture
            .template_variant("command_r.default")
            .expect("variant");
        let window_case = fixture
            .window_cases
            .iter()
            .find(|case| case.id == "command_r.system_history_over_small_window")
            .expect("window case");
        let render_case = variant
            .render_case(window_case.render_case_id)
            .expect("render case");

        assert_prompt_window_case(window_case, render_case.expected_rendered)?;
        Ok(())
    }

    #[derive(Clone, Debug)]
    struct TestCatalog {
        list_models: ListModelsObservation,
        show_model: ShowObservation,
    }

    impl LocalModelCatalog for TestCatalog {
        fn list_models(&self) -> ListModelsObservation {
            self.list_models.clone()
        }

        fn show_model(&self, _model: &str) -> ShowObservation {
            self.show_model.clone()
        }
    }

    #[test]
    fn mox_local_runtime_forwards_catalog_lifecycle_generation_and_embeddings()
    -> Result<(), Box<dyn std::error::Error>> {
        let generation = CpuReferenceTextGenerationService::new()?;
        let embeddings = SmokeEmbeddingsService::new()?;
        let decoder_descriptor = generation.model_descriptor().clone();
        let embedding_descriptor = embeddings.model_descriptor().clone();
        let catalog = TestCatalog {
            list_models: ListModelsObservation::new(vec![
                ModelSummary::from_decoder_descriptor(
                    decoder_descriptor.model.model_id.clone(),
                    &decoder_descriptor,
                ),
                ModelSummary::from_embedding_descriptor(
                    embedding_descriptor.model.model_id.clone(),
                    &embedding_descriptor,
                ),
            ]),
            show_model: ShowObservation::from_decoder_descriptor(
                decoder_descriptor.model.model_id.clone(),
                &decoder_descriptor,
            ),
        };
        let mut runtime = MoxLocalRuntime::new(catalog, generation, embeddings);

        let listed = runtime.list_models();
        assert_eq!(listed.models.len(), 2);
        assert_eq!(listed.models[0].name, decoder_descriptor.model.model_id);
        assert_eq!(listed.models[1].name, embedding_descriptor.model.model_id);

        let shown = runtime.show_model(ReferenceWordDecoder::MODEL_ID);
        assert_eq!(shown.model, ReferenceWordDecoder::MODEL_ID);
        assert_eq!(shown.family.as_deref(), Some("fixture_decoder"));

        let loaded = runtime.loaded_models();
        assert_eq!(loaded.models.len(), 1);
        assert_eq!(loaded.models[0].model, ReferenceWordDecoder::MODEL_ID);

        let warmed = runtime.warm_model(ReferenceWordDecoder::MODEL_ID, 0)?;
        assert_eq!(warmed.summary.model, ReferenceWordDecoder::MODEL_ID);
        assert_eq!(warmed.residency.keep_alive_millis, 0);

        let generation_request = GenerationRequest::new_text(
            "runtime-generate-1",
            decoder_descriptor,
            None,
            "hello",
            GenerationOptions::greedy(4),
        );
        let generation_response = runtime.generate(&generation_request)?;
        assert_eq!(generation_response.output.text, "open agents");

        let embedding_request = EmbeddingRequest::new(
            "runtime-embed-1",
            embedding_descriptor,
            vec![String::from("hello world")],
        );
        let embedding_response = runtime.embed(&embedding_request)?;
        assert_eq!(embedding_response.embeddings.len(), 1);
        assert_eq!(embedding_response.metadata.vector_count, 1);

        let unloaded = runtime.unload_model(ReferenceWordDecoder::MODEL_ID)?;
        assert_eq!(unloaded.summary.model, ReferenceWordDecoder::MODEL_ID);
        assert!(runtime.loaded_models().models.is_empty());
        Ok(())
    }

    #[test]
    fn generation_sessions_isolate_and_reset_kv_cache() -> Result<(), Box<dyn std::error::Error>> {
        let descriptor = sample_decoder_descriptor();
        let mut store = InMemoryGenerationSessionStore::new();
        let session_a = store.create(&descriptor);
        let session_b = store.create(&descriptor);

        assert_eq!(session_a.model_family, "fixture_decoder");
        assert_eq!(session_a.model_revision, "v0");
        assert_eq!(session_a.weight_bundle_digest, descriptor.weights.digest);
        assert_eq!(session_a.kv_cache.pages, 0);
        assert_eq!(
            session_a.kv_cache_policy.page_layout.max_context_tokens,
            descriptor.config.max_context
        );

        store.append(
            &session_a.session_id,
            &descriptor,
            FixtureWordTokenizer::HELLO_ID,
            vec![1.0; descriptor.config.kv_width()],
            vec![2.0; descriptor.config.kv_width()],
        )?;
        store.append(
            &session_b.session_id,
            &descriptor,
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
        assert_eq!(
            store.state(&session_a.session_id)?.tokens(),
            &[FixtureWordTokenizer::HELLO_ID]
        );
        assert_eq!(
            store.state(&session_b.session_id)?.tokens(),
            &[FixtureWordTokenizer::RUSTY_ID]
        );
        assert_eq!(
            store
                .state(&session_a.session_id)?
                .session()
                .kv_cache
                .tokens,
            1
        );
        assert_eq!(
            store.state(&session_a.session_id)?.session().kv_cache.pages,
            1
        );
        assert_eq!(
            store.state(&session_a.session_id)?.session().kv_cache.bytes,
            32
        );

        let reset = store.reset(&session_a.session_id)?;
        assert_eq!(reset.cached_tokens, 0);
        assert_eq!(reset.kv_cache.tokens, 0);
        assert_eq!(reset.kv_cache.pages, 0);
        assert!(store.cache(&session_a.session_id)?.is_empty());
        assert!(store.state(&session_a.session_id)?.tokens().is_empty());
        assert_eq!(store.cache(&session_b.session_id)?.len(), 1);

        let closed = store.close(&session_b.session_id)?;
        assert_eq!(closed.cached_tokens, 1);
        assert!(store.cache(&session_b.session_id).is_err());
        Ok(())
    }

    #[test]
    fn generation_sessions_reject_descriptor_drift_even_when_model_id_matches() {
        let descriptor = sample_decoder_descriptor();
        let mut drifted = descriptor.clone();
        drifted.weights.digest = String::from("different-weight-bundle");

        let mut store = InMemoryGenerationSessionStore::new();
        let session = store.create(&descriptor);
        let error = store
            .append(
                &session.session_id,
                &drifted,
                FixtureWordTokenizer::HELLO_ID,
                vec![1.0; descriptor.config.kv_width()],
                vec![2.0; descriptor.config.kv_width()],
            )
            .expect_err("drifted descriptor should fail");

        assert!(matches!(
            error,
            super::SessionStoreError::ModelMismatch {
                expected_model,
                actual_model,
                expected_weight_bundle_digest,
                actual_weight_bundle_digest,
                ..
            } if expected_model == descriptor.model.model_id
                && actual_model == descriptor.model.model_id
                && expected_weight_bundle_digest == descriptor.weights.digest
                && actual_weight_bundle_digest == "different-weight-bundle"
        ));
    }

    #[test]
    fn paged_kv_cache_tracks_growth_refill_and_refusal() -> Result<(), Box<dyn std::error::Error>> {
        let policy = KvCachePolicy {
            device_scope: KvCacheDeviceScope::SameDeviceOnly,
            spill_policy: KvCacheSpillPolicy::RefuseNewPages,
            page_layout: KvCachePageLayout::new(5, 2, 32),
        };
        let mut cache = InMemoryKvCache::with_policy(5, 4, policy.clone());
        assert_eq!(cache.state(), KvCacheState::default());

        cache.append(TokenId(1), vec![0.0; 4], vec![1.0; 4])?;
        let first = cache.state();
        assert_eq!(first.tokens, 1);
        assert_eq!(first.pages, 1);
        assert_eq!(first.bytes, 32);

        cache.append(TokenId(2), vec![0.0; 4], vec![1.0; 4])?;
        let refill = KvCacheAccounting::from_states(&first, cache.state());
        assert_eq!(refill.current.tokens, 2);
        assert_eq!(refill.current.pages, 1);
        assert_eq!(refill.growth.tokens, 1);
        assert_eq!(refill.growth.pages, 0);

        cache.append(TokenId(3), vec![0.0; 4], vec![1.0; 4])?;
        let growth = KvCacheAccounting::from_states(&first, cache.state());
        assert_eq!(growth.current.pages, 2);
        assert_eq!(growth.growth.pages, 1);

        cache.append(TokenId(4), vec![0.0; 4], vec![1.0; 4])?;
        cache.append(TokenId(5), vec![0.0; 4], vec![1.0; 4])?;
        let error = cache
            .append(TokenId(6), vec![0.0; 4], vec![1.0; 4])
            .expect_err("page-budget refusal");
        assert!(matches!(
            error,
            KvCacheError::PageBudgetExceeded {
                requested_tokens: 6,
                max_context: 5,
                max_pages: 3,
                spill_policy: KvCacheSpillPolicy::RefuseNewPages,
            }
        ));

        cache.reset();
        assert_eq!(cache.state(), KvCacheState::default());
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

    #[derive(Clone, Debug)]
    struct TestGenerationHandle {
        descriptor: DecoderModelDescriptor,
    }

    impl GenerationModelHandle for TestGenerationHandle {
        fn descriptor(&self) -> &DecoderModelDescriptor {
            &self.descriptor
        }
    }

    #[test]
    fn model_registry_tracks_keepalive_order_and_idle_expiry() {
        let mut registry = InMemoryGenerationModelRegistry::new();
        let alpha = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("alpha"),
        };
        let beta = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("beta"),
        };

        assert!(
            registry
                .warm_with_metadata(
                    alpha,
                    1_000,
                    5_000,
                    Some(64),
                    Some(String::from("cpu")),
                    None
                )
                .is_none()
        );
        assert!(
            registry
                .warm_with_metadata(
                    beta,
                    2_000,
                    2_000,
                    Some(32),
                    Some(String::from("cpu")),
                    None
                )
                .is_none()
        );

        let views = registry.loaded_model_views();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].summary.model, "alpha");
        assert_eq!(views[0].residency.expires_at_millis, Some(6_000));
        assert_eq!(views[1].summary.model, "beta");
        assert_eq!(views[1].residency.expires_at_millis, Some(4_000));

        let warmed = registry
            .warm_loaded("beta", 3_000, 9_000)
            .expect("warm existing beta");
        assert_eq!(warmed.residency.keep_alive_millis, 9_000);
        assert_eq!(warmed.residency.expires_at_millis, Some(12_000));

        let request_start = registry
            .begin_request("beta", 3_500)
            .expect("begin request");
        assert_eq!(request_start.residency.state, LoadedModelState::Ready);
        assert_eq!(request_start.residency.active_requests, 1);
        assert_eq!(request_start.residency.expires_at_millis, None);

        let during_request = registry.loaded_model_views();
        assert_eq!(during_request[0].summary.model, "beta");

        let request_finish = registry
            .finish_request("beta", 4_000)
            .expect("finish request");
        assert_eq!(request_finish.residency.active_requests, 0);
        assert_eq!(request_finish.residency.expires_at_millis, Some(13_000));

        let expired = registry.expire_idle(6_001);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].descriptor().model.model_id, "alpha");

        let remaining = registry.loaded_model_views();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].summary.model, "beta");
    }

    #[test]
    fn model_registry_zero_keepalive_unloads_when_idle() {
        let mut registry = InMemoryGenerationModelRegistry::new();
        let gamma = TestGenerationHandle {
            descriptor: sample_named_decoder_descriptor("gamma"),
        };

        assert!(
            registry
                .warm_with_metadata(gamma, 10_000, 5_000, None, Some(String::from("cpu")), None)
                .is_none()
        );
        let warmed = registry
            .warm_loaded("gamma", 10_100, 0)
            .expect("warm gamma with zero keepalive");
        assert_eq!(warmed.residency.expires_at_millis, Some(10_100));

        let expired = registry.expire_idle(10_100);
        assert_eq!(expired.len(), 1);
        assert!(registry.is_empty());
    }

    #[test]
    fn seeded_sampling_is_replayable() {
        let options = GenerationOptions {
            max_output_tokens: 4,
            decode_strategy: super::DecodeStrategy::Sample,
            temperature: Some(0.9),
            top_k: Some(3),
            top_p: Some(0.95),
            repeat_penalty: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: Some(42),
            stop_sequences: Vec::new(),
        };
        let cache = super::InMemoryKvCache::new(8, 1);
        let logits = vec![3.0, 2.9, 2.8];
        let mut left = super::GenerationSampler::new(&options);
        let mut right = super::GenerationSampler::new(&options);

        let left_draws = (0..4)
            .map(|_| left.select_next_token(&logits, &cache).expect("sample"))
            .collect::<Vec<_>>();
        let right_draws = (0..4)
            .map(|_| right.select_next_token(&logits, &cache).expect("sample"))
            .collect::<Vec<_>>();

        assert_eq!(left_draws, right_draws);
    }

    #[test]
    fn penalties_shift_token_selection() -> Result<(), Box<dyn std::error::Error>> {
        let options = GenerationOptions {
            max_output_tokens: 4,
            decode_strategy: super::DecodeStrategy::Greedy,
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: Some(2.0),
            presence_penalty: Some(0.5),
            frequency_penalty: Some(0.5),
            seed: None,
            stop_sequences: Vec::new(),
        };
        let mut cache = super::InMemoryKvCache::new(8, 1);
        cache.append(TokenId(1), vec![0.0], vec![0.0])?;
        cache.append(TokenId(1), vec![0.0], vec![0.0])?;

        let mut logits = vec![1.0, 3.0, 2.5];
        super::apply_generation_penalties(&mut logits, &cache, &options);

        assert_eq!(super::select_argmax(&logits), Some(TokenId(2)));
        Ok(())
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
        assert_eq!(first.output, second.output);
        assert_eq!(first.usage, second.usage);
        assert_eq!(first.termination, second.termination);
        assert_eq!(
            first
                .provenance
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str()),
            second
                .provenance
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str())
        );
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
    fn cpu_reference_text_generation_reports_cold_then_warm_provenance()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let request = GenerationRequest::new_text(
            "gen-ref-metrics-1",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(4),
        );
        let expected_plan_digest = service
            .plan_digest(ReferenceWordDecoder::MODEL_ID)
            .expect("plan digest")
            .to_string();

        let first = service.generate(&request)?;
        assert_eq!(
            first.provenance.as_ref().map(|value| value.load_state),
            Some(GenerationLoadState::Cold)
        );
        assert_eq!(
            first
                .provenance
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str()),
            Some(expected_plan_digest.as_str())
        );
        assert_eq!(
            first.metrics.prompt_eval_count,
            Some(first.usage.input_tokens)
        );
        assert_eq!(first.metrics.eval_count, Some(first.usage.output_tokens));
        assert_eq!(
            first
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.current.tokens),
            Some(first.usage.cache_tokens)
        );
        assert_eq!(
            first
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.growth.pages),
            Some(1)
        );
        assert!(first.metrics.total_duration_ns.is_some());
        assert!(first.metrics.load_duration_ns.is_some());
        assert!(
            first
                .provenance
                .as_ref()
                .and_then(|value| value.kv_cache_policy.as_ref())
                .is_some()
        );

        let second = service.generate(&GenerationRequest::new_text(
            "gen-ref-metrics-2",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(4),
        ))?;
        assert_eq!(
            second.provenance.as_ref().map(|value| value.load_state),
            Some(GenerationLoadState::Warm)
        );
        assert_eq!(
            second
                .provenance
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str()),
            Some(expected_plan_digest.as_str())
        );
        assert_eq!(
            second.metrics.prompt_eval_count,
            Some(second.usage.input_tokens)
        );
        assert_eq!(second.metrics.eval_count, Some(second.usage.output_tokens));
        assert_eq!(
            second
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.current.tokens),
            Some(second.usage.cache_tokens)
        );
        assert!(second.metrics.total_duration_ns.is_some());
        assert_eq!(second.metrics.load_duration_ns, Some(0));
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
        assert_eq!(
            first
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.current.pages),
            Some(1)
        );

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
        assert_eq!(
            second
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.growth.tokens),
            Some(3)
        );

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
        assert_eq!(
            reset
                .metrics
                .kv_cache
                .as_ref()
                .map(|value| value.growth.tokens),
            Some(3)
        );
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_truncates_stop_sequences()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let request = GenerationRequest::new_text(
            "gen-ref-stop",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions {
                stop_sequences: vec![String::from("agents")],
                ..GenerationOptions::greedy(4)
            },
        );

        let response = service.generate(&request)?;
        assert_eq!(response.output.text, "open");
        assert_eq!(
            response.output.tokens.as_slice(),
            &[FixtureWordTokenizer::OPEN_ID]
        );
        assert_eq!(response.termination, TerminationReason::EndOfSequence);
        Ok(())
    }

    #[test]
    fn cpu_reference_text_generation_updates_loaded_model_residency()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut service = CpuReferenceTextGenerationService::new()?;
        let warmed = service.warm_model(ReferenceWordDecoder::MODEL_ID, 0)?;
        assert_eq!(warmed.residency.keep_alive_millis, 0);

        let request = GenerationRequest::new_text(
            "gen-ref-lifecycle",
            service.model_descriptor().clone(),
            None,
            "hello",
            GenerationOptions::greedy(2),
        );
        let response = service.generate(&request)?;
        assert_eq!(response.output.text, "open agents");
        assert!(service.loaded_model_views().is_empty());
        assert!(service.loaded_models().models.is_empty());
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
        sample_named_decoder_descriptor("fixture-word-decoder-v0")
    }

    fn sample_named_decoder_descriptor(model_id: &str) -> DecoderModelDescriptor {
        DecoderModelDescriptor::new(
            ModelDescriptor::new(model_id, "fixture_decoder", "v0"),
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
