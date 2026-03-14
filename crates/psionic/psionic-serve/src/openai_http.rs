use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    convert::Infallible,
    env,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use psionic_catalog::{BlobIntegrityPolicy, LocalBlobOpenOptions};
use psionic_models::{
    GgufBlobArtifact, GgufDecoderFamily, GgufPromptTemplateRenderer, GptOssHarmonyParseOptions,
    GptOssHarmonyParsedOutput, GptOssHarmonyRenderContext, GptOssTokenizer,
    ParsedReasoningResponse, PromptChannelConfig, PromptMessage, PromptMessageRole,
    PromptReasoningEffort, PromptRenderOptions, ReasoningParser, parse_gpt_oss_harmony_text,
    parse_reasoning_response_text_for_decoder_family, reasoning_parser_for_decoder_family,
    render_gpt_oss_harmony_prompt,
};
use psionic_router::{
    FleetRouter, ResponseConversationRef, ResponseStateCapability, ResponseStateError,
    ResponseStateRecord, ResponseStateRetentionPolicy, ResponseStateStore, RouteSelection,
    RouteSelectionStrategy, RoutedModelInventory, RoutedWarmState, RoutedWorkerInventory,
    RoutingEndpoint, RoutingError, RoutingRequest, RoutingTarget,
};
use psionic_runtime::{
    ExecutionCapabilityProfile, GenerationSchedulerPolicy, GenerationSchedulerRequestReceipt,
    PrefixCacheControl, PrefixCacheRefusalReason, PrefixCacheState, StructuredGrammarSyntax,
    StructuredOutputCapability, StructuredOutputExecutionReport, StructuredOutputMatcher,
    StructuredOutputParser, StructuredOutputRequest, StructuredOutputValue,
    StructuredTaggedVariant, local_structured_output_capabilities, local_structured_output_parsers,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
};
use tokio_stream::iter;

use crate::{
    CpuGgufTextGenerationService, CpuModelEmbeddingsService, CudaGgufGptOssTextGenerationService,
    CudaGptOssTextGenerationError, DecodeStrategy, DecoderModelDescriptor, EmbeddingMetrics,
    EmbeddingNormalization, EmbeddingProvenance, EmbeddingRequest, EmbeddingResponse,
    EmbeddingsExecutor, GenerationMetrics, GenerationOptions, GenerationRequest,
    GgufDecoderAdapterLoader, GptOssPerformanceMetrics, MetalGgufGptOssTextGenerationService,
    MetalGptOssTextGenerationError, ModelEmbeddingsError, PromptRenderError,
    ReferenceTextGenerationError, TerminationReason, TextGenerationExecutor, TokenSequence,
    continuous_batch_text_generation_execution_profile, default_embeddings_execution_profile,
    default_generation_scheduler_policy,
};

const DEFAULT_MAX_TOKENS: usize = 256;
const HARMONY_RETURN_STOP: &str = "<|return|>";
const HARMONY_CALL_STOP: &str = "<|call|>";
const CPU_SERVER_RESIDENCY_MODE: &str = "cpu_only";
const CPU_SERVER_HYBRID_OFFLOAD_MODE: &str = "unsupported";
const CPU_SERVER_FALLBACK_POLICY: &str = "refuse";
const CPU_SERVER_PERFORMANCE_CLASS: &str = "portable_cpu_degraded";
const CPU_SERVER_LOAD_STATUS: &str = "loaded";
const CPU_SERVER_WARM_CONTROL: &str = "not_implemented";
const CPU_SERVER_UNLOAD_CONTROL: &str = "not_implemented";
const CPU_SERVER_MEMORY_PRESSURE_REPORTING: &str = "not_implemented";
const OPENAI_COMPAT_WORKER_ID: &str = "local_cpu_0";

fn structured_output_parser_labels() -> Vec<&'static str> {
    local_structured_output_parsers()
        .into_iter()
        .map(StructuredOutputParser::label)
        .collect()
}

fn unsupported_structured_output_capabilities(detail: &str) -> Vec<StructuredOutputCapability> {
    local_structured_output_capabilities()
        .into_iter()
        .map(|capability| StructuredOutputCapability::unsupported(capability.kind, detail))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ToolCallingSupportLevel {
    Fallback,
    Unsupported,
}

impl ToolCallingSupportLevel {
    #[cfg(test)]
    fn label(self) -> &'static str {
        match self {
            Self::Fallback => "fallback",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ToolCallingCapability {
    support_level: ToolCallingSupportLevel,
    supported_modes: Vec<&'static str>,
    parser: &'static str,
    argument_validation: &'static str,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ResponseContinuationMode {
    #[default]
    AppendTurn,
    ContinueLastAssistant,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct PsionicResponseStateRequest {
    #[serde(
        default = "default_response_state_store",
        skip_serializing_if = "is_true"
    )]
    store: bool,
    #[serde(default)]
    continuation: ResponseContinuationMode,
    #[serde(default)]
    invalidate_references: bool,
}

impl Default for PsionicResponseStateRequest {
    fn default() -> Self {
        Self {
            store: true,
            continuation: ResponseContinuationMode::AppendTurn,
            invalidate_references: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ResponseStateReceipt {
    storage: String,
    retention_scope: String,
    cache_behavior: String,
    stored: bool,
    continuation: ResponseContinuationMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation_id: Option<String>,
    replayed_prompt_messages: usize,
    input_messages_appended: usize,
    assistant_messages_recorded: usize,
    max_responses: usize,
    max_conversations: usize,
    max_items_per_conversation: usize,
    conversation_item_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    invalidated_references: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolChoiceMode {
    None,
    Auto,
    Required,
    Named,
}

#[derive(Clone, Debug)]
struct ToolCallingContract {
    tools: BTreeMap<String, ToolDefinitionRequest>,
    mode: ToolChoiceMode,
    named_tool: Option<String>,
}

#[derive(Clone, Debug)]
struct ToolCallOutcome {
    content: Option<String>,
    tool_calls: Vec<ResolvedToolCall>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResolvedReasoningRequest {
    parser: ReasoningParser,
    mode: PsionicReasoningMode,
}

#[derive(Clone, Debug)]
struct ResolvedToolCall {
    id: String,
    name: String,
    arguments: serde_json::Value,
}

impl ResolvedToolCall {
    fn raw_arguments(&self) -> Result<String, OpenAiCompatHttpError> {
        serde_json::to_string(&self.arguments).map_err(|error| {
            OpenAiCompatHttpError::Internal(format!(
                "failed to serialize validated tool arguments for `{}`: {error}",
                self.name
            ))
        })
    }

    fn into_chat_tool_call(self) -> Result<ChatCompletionToolCall, OpenAiCompatHttpError> {
        let raw_arguments = self.raw_arguments()?;
        Ok(ChatCompletionToolCall {
            id: self.id,
            kind: "function",
            function: ChatCompletionToolCallFunction {
                name: self.name,
                arguments: raw_arguments,
            },
        })
    }

    fn into_psionic_tool_call(self) -> Result<PsionicToolCall, OpenAiCompatHttpError> {
        let raw_arguments = self.raw_arguments()?;
        Ok(PsionicToolCall {
            id: self.id,
            name: self.name,
            arguments: self.arguments,
            raw_arguments,
        })
    }
}

fn gpt_oss_local_blob_open_options() -> LocalBlobOpenOptions {
    LocalBlobOpenOptions::default().with_integrity_policy(BlobIntegrityPolicy::LocalUnverifiedLabel)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GptOssOpenAiCompatBackend {
    Auto,
    Cpu,
    Cuda,
    Metal,
}

impl GptOssOpenAiCompatBackend {
    fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Metal => "metal",
        }
    }

    fn resolve(self) -> Self {
        match self {
            Self::Auto => {
                if cfg!(target_os = "macos") {
                    Self::Metal
                } else {
                    Self::Cuda
                }
            }
            backend => backend,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GptOssMetalExecutionMode {
    Auto,
    Native,
    ProxyLlamaCpp,
}

impl GptOssMetalExecutionMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Native => "native",
            Self::ProxyLlamaCpp => "proxy",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GptOssOpenAiCompatExecutionSummary {
    backend_label: &'static str,
    execution_mode_label: &'static str,
    execution_engine_label: &'static str,
}

impl GptOssOpenAiCompatExecutionSummary {
    const fn native(backend_label: &'static str) -> Self {
        Self {
            backend_label,
            execution_mode_label: "native",
            execution_engine_label: "psionic",
        }
    }

    const fn metal_proxy() -> Self {
        Self {
            backend_label: "metal",
            execution_mode_label: "proxy",
            execution_engine_label: "llama.cpp",
        }
    }

    fn uses_proxy(self) -> bool {
        matches!(self.execution_engine_label, "llama.cpp")
    }
}

fn resolve_execution_summary(
    backend: GptOssOpenAiCompatBackend,
    metal_mode: GptOssMetalExecutionMode,
    legacy_proxy_enabled: bool,
) -> Result<GptOssOpenAiCompatExecutionSummary, OpenAiCompatServerError> {
    match backend {
        GptOssOpenAiCompatBackend::Metal => match metal_mode {
            GptOssMetalExecutionMode::Auto => Ok(if legacy_proxy_enabled {
                GptOssOpenAiCompatExecutionSummary::metal_proxy()
            } else {
                GptOssOpenAiCompatExecutionSummary::native("metal")
            }),
            GptOssMetalExecutionMode::Native => {
                if legacy_proxy_enabled {
                    Err(OpenAiCompatServerError::Config(String::from(
                        "requested `--metal-mode native` while legacy PSIONIC_METAL_PROXY_LLAMA_CPP is enabled",
                    )))
                } else {
                    Ok(GptOssOpenAiCompatExecutionSummary::native("metal"))
                }
            }
            GptOssMetalExecutionMode::ProxyLlamaCpp => {
                Ok(GptOssOpenAiCompatExecutionSummary::metal_proxy())
            }
        },
        GptOssOpenAiCompatBackend::Cpu => {
            if matches!(metal_mode, GptOssMetalExecutionMode::Auto) {
                Ok(GptOssOpenAiCompatExecutionSummary::native("cpu"))
            } else {
                Err(OpenAiCompatServerError::Config(format!(
                    "requested `--metal-mode {}` but resolved backend is cpu",
                    metal_mode.label()
                )))
            }
        }
        GptOssOpenAiCompatBackend::Cuda => {
            if matches!(metal_mode, GptOssMetalExecutionMode::Auto) {
                Ok(GptOssOpenAiCompatExecutionSummary::native("cuda"))
            } else {
                Err(OpenAiCompatServerError::Config(format!(
                    "requested `--metal-mode {}` but resolved backend is cuda",
                    metal_mode.label()
                )))
            }
        }
        GptOssOpenAiCompatBackend::Auto => Err(OpenAiCompatServerError::Config(String::from(
            "auto backend must be resolved before execution mode selection",
        ))),
    }
}

#[derive(Clone, Debug)]
pub struct GptOssOpenAiCompatConfig {
    pub model_path: PathBuf,
    pub host: String,
    pub port: u16,
    pub backend: GptOssOpenAiCompatBackend,
    pub context_length: Option<usize>,
    pub gpu_layers: Option<i32>,
    pub metal_mode: GptOssMetalExecutionMode,
    pub reasoning_budget: u8,
    pub webui_enabled: bool,
}

impl GptOssOpenAiCompatConfig {
    #[must_use]
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            host: String::from("127.0.0.1"),
            port: 8080,
            backend: GptOssOpenAiCompatBackend::Auto,
            context_length: None,
            gpu_layers: None,
            metal_mode: GptOssMetalExecutionMode::Auto,
            reasoning_budget: 0,
            webui_enabled: false,
        }
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, OpenAiCompatServerError> {
        let host = self.host.parse::<IpAddr>().map_err(|error| {
            OpenAiCompatServerError::Config(format!("invalid host `{}`: {error}", self.host))
        })?;
        Ok(SocketAddr::new(host, self.port))
    }
}

#[derive(Clone)]
pub struct GptOssOpenAiCompatServer {
    state: Arc<GptOssOpenAiCompatState>,
}

#[derive(Clone)]
pub struct GptOssCudaOpenAiCompatServer {
    inner: GptOssOpenAiCompatServer,
}

struct GptOssOpenAiCompatState {
    worker: Option<GptOssWorker>,
    proxy: Option<Arc<LlamaCppProxyState>>,
    backend_label: &'static str,
    execution_mode_label: &'static str,
    execution_engine_label: &'static str,
    descriptor: DecoderModelDescriptor,
    tokenizer: GptOssTokenizer,
    prompt_options: PromptRenderOptions,
    prompt_token_cache: Mutex<PromptTokenCache>,
    default_model_name: String,
    accepted_model_names: BTreeSet<String>,
    include_psionic_fields: bool,
    request_counter: AtomicU64,
}

struct LlamaCppProxyState {
    base_url: String,
    client: reqwest::Client,
    child: Mutex<Option<Child>>,
}

impl Drop for LlamaCppProxyState {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.lock().ok().and_then(|mut child| child.take()) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Clone, Debug)]
struct PromptTokenCacheEntry {
    request_key: String,
    tokens: TokenSequence,
}

#[derive(Clone, Debug)]
struct PromptTokenCache {
    entries: VecDeque<PromptTokenCacheEntry>,
    capacity: usize,
}

impl PromptTokenCache {
    const DEFAULT_CAPACITY: usize = 16;

    fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    fn lookup(&mut self, request_key: &str) -> Option<TokenSequence> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.request_key == request_key)?;
        let entry = self.entries.remove(index)?;
        let tokens = entry.tokens.clone();
        self.entries.push_front(entry);
        Some(tokens)
    }

    fn record(&mut self, request_key: String, tokens: TokenSequence) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.request_key == request_key)
        {
            self.entries.remove(index);
        }
        self.entries.push_front(PromptTokenCacheEntry {
            request_key,
            tokens,
        });
        while self.entries.len() > self.capacity {
            self.entries.pop_back();
        }
    }
}

impl GptOssOpenAiCompatServer {
    pub fn from_config(config: &GptOssOpenAiCompatConfig) -> Result<Self, OpenAiCompatServerError> {
        let artifact =
            GgufBlobArtifact::open_path(&config.model_path, gpt_oss_local_blob_open_options())
                .map_err(|error| OpenAiCompatServerError::Config(error.to_string()))?;
        let adapter = GgufDecoderAdapterLoader
            .load_blob_artifact(&artifact)
            .map_err(|error| OpenAiCompatServerError::Config(error.to_string()))?;
        let descriptor = adapter.descriptor().clone();
        let tokenizer = GptOssTokenizer::from_gguf(adapter.tokenizer())
            .map_err(|error| OpenAiCompatServerError::Config(error.to_string()))?;
        let default_model_name =
            default_model_name(&config.model_path, descriptor.model.model_id.as_str());
        let accepted_model_names =
            accepted_model_names(&config.model_path, descriptor.model.model_id.as_str());
        let prompt_options = PromptRenderOptions {
            gpt_oss_harmony: Some(GptOssHarmonyRenderContext {
                reasoning_effort: Some(reasoning_effort(config.reasoning_budget)),
                channel_config: Some(PromptChannelConfig::default()),
                ..Default::default()
            }),
        };
        let include_psionic_fields = env::var("PSIONIC_OPENAI_INCLUDE_DEBUG_FIELDS")
            .ok()
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        let backend = config.backend.resolve();
        let execution_summary =
            resolve_execution_summary(backend, config.metal_mode, metal_proxy_llama_cpp_enabled())?;
        let proxy = if execution_summary.uses_proxy() {
            Some(Arc::new(LlamaCppProxyState::spawn(config)?))
        } else {
            None
        };
        Ok(Self {
            state: Arc::new(GptOssOpenAiCompatState {
                worker: if proxy.is_some() {
                    None
                } else {
                    Some(GptOssWorker::spawn(config.model_path.clone(), backend)?)
                },
                proxy,
                backend_label: execution_summary.backend_label,
                execution_mode_label: execution_summary.execution_mode_label,
                execution_engine_label: execution_summary.execution_engine_label,
                descriptor,
                tokenizer,
                prompt_options,
                prompt_token_cache: Mutex::new(PromptTokenCache::new(
                    PromptTokenCache::DEFAULT_CAPACITY,
                )),
                default_model_name,
                accepted_model_names,
                include_psionic_fields,
                request_counter: AtomicU64::new(1),
            }),
        })
    }

    #[must_use]
    pub fn backend_label(&self) -> &'static str {
        self.state.backend_label
    }

    #[must_use]
    pub fn execution_mode_label(&self) -> &'static str {
        self.state.execution_mode_label
    }

    #[must_use]
    pub fn execution_engine_label(&self) -> &'static str {
        self.state.execution_engine_label
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(health))
            .route("/v1/models", get(list_models))
            .route("/v1/chat/completions", post(chat_completions))
            .with_state(Arc::clone(&self.state))
    }

    pub async fn serve(&self, listener: TcpListener) -> Result<(), OpenAiCompatServerError> {
        axum::serve(listener, self.router())
            .await
            .map_err(OpenAiCompatServerError::Io)
    }
}

impl GptOssCudaOpenAiCompatServer {
    pub fn from_config(config: &GptOssOpenAiCompatConfig) -> Result<Self, OpenAiCompatServerError> {
        let mut config = config.clone();
        config.backend = GptOssOpenAiCompatBackend::Cuda;
        Ok(Self {
            inner: GptOssOpenAiCompatServer::from_config(&config)?,
        })
    }

    pub fn router(&self) -> Router {
        self.inner.router()
    }

    pub async fn serve(&self, listener: TcpListener) -> Result<(), OpenAiCompatServerError> {
        self.inner.serve(listener).await
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenAiCompatBackend {
    Cpu,
}

impl OpenAiCompatBackend {
    fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
        }
    }
}

#[derive(Clone, Debug)]
pub struct OpenAiCompatConfig {
    pub model_paths: Vec<PathBuf>,
    pub host: String,
    pub port: u16,
    pub backend: OpenAiCompatBackend,
    pub reasoning_budget: u8,
}

impl OpenAiCompatConfig {
    #[must_use]
    pub fn new(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_paths: vec![model_path.into()],
            host: String::from("127.0.0.1"),
            port: 8080,
            backend: OpenAiCompatBackend::Cpu,
            reasoning_budget: 0,
        }
    }

    pub fn add_model_path(&mut self, model_path: impl Into<PathBuf>) {
        self.model_paths.push(model_path.into());
    }

    pub fn socket_addr(&self) -> Result<SocketAddr, OpenAiCompatServerError> {
        let host = self.host.parse::<IpAddr>().map_err(|error| {
            OpenAiCompatServerError::Config(format!("invalid host `{}`: {error}", self.host))
        })?;
        Ok(SocketAddr::new(host, self.port))
    }
}

#[derive(Clone)]
pub struct OpenAiCompatServer {
    state: Arc<OpenAiCompatState>,
}

struct OpenAiCompatState {
    workers: BTreeMap<String, OpenAiCompatWorker>,
    router: FleetRouter,
    backend_label: &'static str,
    execution_mode_label: &'static str,
    execution_engine_label: &'static str,
    default_model_key: String,
    default_model_name: String,
    models_by_key: BTreeMap<String, OpenAiCompatLoadedModel>,
    include_psionic_fields: bool,
    request_counter: AtomicU64,
    conversation_counter: AtomicU64,
    response_state_capability: ResponseStateCapability,
    response_state: Mutex<ResponseStateStore>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenAiCompatRuntimeKind {
    GgufDecoder,
    SafetensorsEmbeddings,
}

#[derive(Clone, Debug)]
struct OpenAiCompatModelLoadPlan {
    path: PathBuf,
    runtime_kind: OpenAiCompatRuntimeKind,
}

#[derive(Clone)]
struct OpenAiCompatLoadedModel {
    model_key: String,
    canonical_name: String,
    supported_endpoints: Vec<RoutingEndpoint>,
    kind: OpenAiCompatLoadedModelKind,
}

#[derive(Clone)]
enum OpenAiCompatLoadedModelKind {
    Decoder(OpenAiCompatLoadedDecoderModel),
    Embeddings(OpenAiCompatLoadedEmbeddingsModel),
}

#[derive(Clone)]
struct OpenAiCompatLoadedDecoderModel {
    descriptor: DecoderModelDescriptor,
    family: GgufDecoderFamily,
    prompt_renderer: Option<GgufPromptTemplateRenderer>,
    prompt_options: PromptRenderOptions,
    execution_profile: ExecutionCapabilityProfile,
    scheduler_policy: GenerationSchedulerPolicy,
}

#[derive(Clone)]
struct OpenAiCompatLoadedEmbeddingsModel {
    descriptor: psionic_models::EmbeddingModelDescriptor,
    execution_profile: ExecutionCapabilityProfile,
}

impl OpenAiCompatLoadedModel {
    fn decoder(&self) -> Option<&OpenAiCompatLoadedDecoderModel> {
        match &self.kind {
            OpenAiCompatLoadedModelKind::Decoder(model) => Some(model),
            OpenAiCompatLoadedModelKind::Embeddings(_) => None,
        }
    }

    fn embeddings(&self) -> Option<&OpenAiCompatLoadedEmbeddingsModel> {
        match &self.kind {
            OpenAiCompatLoadedModelKind::Decoder(_) => None,
            OpenAiCompatLoadedModelKind::Embeddings(model) => Some(model),
        }
    }

    fn execution_profile(&self) -> &ExecutionCapabilityProfile {
        match &self.kind {
            OpenAiCompatLoadedModelKind::Decoder(model) => &model.execution_profile,
            OpenAiCompatLoadedModelKind::Embeddings(model) => &model.execution_profile,
        }
    }

    fn scheduler_policy(&self) -> Option<&GenerationSchedulerPolicy> {
        self.decoder().map(|model| &model.scheduler_policy)
    }

    fn structured_output_labels(&self) -> Option<Vec<&'static str>> {
        self.decoder().map(|_| structured_output_parser_labels())
    }

    fn structured_output_capabilities(&self) -> Vec<StructuredOutputCapability> {
        if self.decoder().is_some() {
            local_structured_output_capabilities()
        } else {
            unsupported_structured_output_capabilities(
                "structured outputs are unavailable on embeddings-only models",
            )
        }
    }

    fn tool_calling_capability(&self) -> ToolCallingCapability {
        if self.decoder().is_some() {
            ToolCallingCapability {
                support_level: ToolCallingSupportLevel::Fallback,
                supported_modes: vec!["none", "auto", "required", "named"],
                parser: "tagged_json_schema",
                argument_validation: "json_schema_subset",
            }
        } else {
            ToolCallingCapability {
                support_level: ToolCallingSupportLevel::Unsupported,
                supported_modes: vec!["none"],
                parser: "not_available",
                argument_validation: "not_available",
            }
        }
    }

    fn family_label(&self) -> &str {
        match &self.kind {
            OpenAiCompatLoadedModelKind::Decoder(model) => model.descriptor.model.family.as_str(),
            OpenAiCompatLoadedModelKind::Embeddings(model) => {
                model.descriptor.model.family.as_str()
            }
        }
    }

    fn embedding_dimensions(&self) -> Option<usize> {
        self.embeddings().map(|model| model.descriptor.dimensions)
    }

    fn embedding_normalization(&self) -> Option<EmbeddingNormalization> {
        self.embeddings()
            .map(|model| model.descriptor.normalization)
    }
}

#[derive(Clone)]
struct OpenAiCompatWorker {
    sender: mpsc::UnboundedSender<OpenAiCompatWorkerCommand>,
}

enum OpenAiCompatWorkerCommand {
    Generate {
        model_key: String,
        request: GenerationRequest,
        reply: oneshot::Sender<Result<crate::GenerationResponse, ReferenceTextGenerationError>>,
    },
    Embed {
        model_key: String,
        request: EmbeddingRequest,
        reply: oneshot::Sender<Result<EmbeddingResponse, ModelEmbeddingsError>>,
    },
}

impl OpenAiCompatServer {
    pub fn from_config(config: &OpenAiCompatConfig) -> Result<Self, OpenAiCompatServerError> {
        Self::from_config_with_response_state_store(
            config,
            ResponseStateStore::in_memory(ResponseStateRetentionPolicy::default()),
        )
    }

    pub fn from_config_with_response_state_store(
        config: &OpenAiCompatConfig,
        response_state: ResponseStateStore,
    ) -> Result<Self, OpenAiCompatServerError> {
        if config.model_paths.is_empty() {
            return Err(OpenAiCompatServerError::Config(String::from(
                "generic OpenAI server requires at least one `--model` path",
            )));
        }
        if !matches!(config.backend, OpenAiCompatBackend::Cpu) {
            return Err(OpenAiCompatServerError::Config(String::from(
                "generic OpenAI server currently supports only the cpu backend",
            )));
        }

        let include_psionic_fields = env::var("PSIONIC_OPENAI_INCLUDE_DEBUG_FIELDS")
            .ok()
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        let mut models_by_key = BTreeMap::new();
        let mut routed_models = Vec::new();
        let mut default_model_key = None;
        let mut default_canonical_model_name = None;
        let mut load_plans = Vec::new();

        for model_path in &config.model_paths {
            let decoder_attempt = load_generic_decoder_model(model_path, config.reasoning_budget);
            let embeddings_attempt = load_generic_embeddings_model(model_path);
            let (loaded_model, accepted_names, load_plan) = match (
                decoder_attempt,
                embeddings_attempt,
            ) {
                (Ok(result), _) => result,
                (Err(_), Ok(result)) => result,
                (Err(decoder_error), Err(embeddings_error)) => {
                    return Err(OpenAiCompatServerError::Config(format!(
                        "unsupported generic model artifact `{}`: decoder load failed: {decoder_error}; embeddings load failed: {embeddings_error}",
                        model_path.display()
                    )));
                }
            };
            if models_by_key
                .insert(loaded_model.model_key.clone(), loaded_model.clone())
                .is_some()
            {
                return Err(OpenAiCompatServerError::Config(format!(
                    "duplicate loaded model id `{}`",
                    loaded_model.model_key
                )));
            }
            routed_models.push(routed_inventory_for_loaded_model(
                &loaded_model,
                accepted_names.into_iter().collect(),
            ));
            if default_model_key.is_none() {
                default_model_key = Some(loaded_model.model_key.clone());
                default_canonical_model_name = Some(loaded_model.canonical_name.clone());
            }
            load_plans.push(load_plan);
        }

        let worker = OpenAiCompatWorker::spawn(load_plans)?;
        let default_model_key = default_model_key.expect("validated non-empty model list");
        let response_state_capability = response_state.capability();
        let router = FleetRouter::new(
            default_model_key.clone(),
            vec![
                RoutedWorkerInventory::new(
                    OPENAI_COMPAT_WORKER_ID,
                    config.backend.label(),
                    "native",
                    "psionic",
                )
                .with_model_entries(routed_models),
            ],
        )
        .map_err(|error| OpenAiCompatServerError::Config(error.to_string()))?;
        let mut workers = BTreeMap::new();
        workers.insert(String::from(OPENAI_COMPAT_WORKER_ID), worker);
        Ok(Self {
            state: Arc::new(OpenAiCompatState {
                workers,
                router,
                backend_label: config.backend.label(),
                execution_mode_label: "native",
                execution_engine_label: "psionic",
                default_model_key,
                default_model_name: default_canonical_model_name
                    .expect("validated non-empty model list"),
                models_by_key,
                include_psionic_fields,
                request_counter: AtomicU64::new(1),
                conversation_counter: AtomicU64::new(1),
                response_state_capability,
                response_state: Mutex::new(response_state),
            }),
        })
    }

    #[must_use]
    pub fn backend_label(&self) -> &'static str {
        self.state.backend_label
    }

    #[must_use]
    pub fn execution_mode_label(&self) -> &'static str {
        self.state.execution_mode_label
    }

    #[must_use]
    pub fn execution_engine_label(&self) -> &'static str {
        self.state.execution_engine_label
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(generic_health))
            .route("/v1/models", get(generic_list_models))
            .route("/v1/chat/completions", post(generic_chat_completions))
            .route("/v1/responses", post(generic_responses))
            .route("/v1/embeddings", post(generic_embeddings))
            .with_state(Arc::clone(&self.state))
    }

    pub async fn serve(&self, listener: TcpListener) -> Result<(), OpenAiCompatServerError> {
        axum::serve(listener, self.router())
            .await
            .map_err(OpenAiCompatServerError::Io)
    }
}

impl OpenAiCompatWorker {
    fn spawn(load_plans: Vec<OpenAiCompatModelLoadPlan>) -> Result<Self, OpenAiCompatServerError> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name(String::from("psionic-openai-cpu-worker"))
            .spawn(move || {
                let mut generation_services = BTreeMap::new();
                let mut embeddings_services = BTreeMap::new();
                for load_plan in &load_plans {
                    match load_plan.runtime_kind {
                        OpenAiCompatRuntimeKind::GgufDecoder => {
                            match CpuGgufTextGenerationService::from_gguf_path(&load_plan.path) {
                                Ok(service) => {
                                    let model_key =
                                        service.model_descriptor().model.model_id.clone();
                                    generation_services.insert(model_key, service);
                                }
                                Err(error) => {
                                    let _ = ready_tx.send(Err::<(), String>(error.to_string()));
                                    return;
                                }
                            }
                        }
                        OpenAiCompatRuntimeKind::SafetensorsEmbeddings => {
                            match CpuModelEmbeddingsService::from_safetensors_artifact(
                                &load_plan.path,
                            ) {
                                Ok(service) => {
                                    let model_key =
                                        service.model_descriptor().model.model_id.clone();
                                    embeddings_services.insert(model_key, service);
                                }
                                Err(error) => {
                                    let _ = ready_tx.send(Err::<(), String>(error.to_string()));
                                    return;
                                }
                            }
                        }
                    }
                }
                let _ = ready_tx.send(Ok::<(), String>(()));
                let mut pending_commands = VecDeque::new();
                loop {
                    let Some(command) = pending_commands
                        .pop_front()
                        .or_else(|| receiver.blocking_recv())
                    else {
                        break;
                    };
                    pending_commands.push_back(command);
                    while let Ok(command) = receiver.try_recv() {
                        pending_commands.push_back(command);
                    }

                    let Some(model_key) = pending_commands.front().map(|command| match command {
                        OpenAiCompatWorkerCommand::Generate { model_key, .. } => model_key.clone(),
                        OpenAiCompatWorkerCommand::Embed { model_key, .. } => model_key.clone(),
                    }) else {
                        continue;
                    };
                    if matches!(
                        pending_commands.front(),
                        Some(OpenAiCompatWorkerCommand::Embed { .. })
                    ) {
                        let Some(OpenAiCompatWorkerCommand::Embed {
                            model_key,
                            request,
                            reply,
                        }) = pending_commands.pop_front()
                        else {
                            continue;
                        };
                        let Some(service) = embeddings_services.get_mut(model_key.as_str()) else {
                            let _ = reply.send(Err(ModelEmbeddingsError::UnsupportedModel(
                                model_key.clone(),
                            )));
                            continue;
                        };
                        let _ = reply.send(service.embed(&request));
                        continue;
                    }
                    let mut selected = Vec::new();
                    let mut remaining = VecDeque::new();
                    while let Some(command) = pending_commands.pop_front() {
                        match command {
                            OpenAiCompatWorkerCommand::Generate {
                                model_key: command_model_key,
                                request,
                                reply,
                            } if command_model_key == model_key => {
                                selected.push((request, reply));
                            }
                            OpenAiCompatWorkerCommand::Embed {
                                model_key: command_model_key,
                                request,
                                reply,
                            } if command_model_key == model_key => {
                                remaining.push_back(OpenAiCompatWorkerCommand::Embed {
                                    model_key: command_model_key,
                                    request,
                                    reply,
                                });
                            }
                            other => remaining.push_back(other),
                        }
                    }
                    pending_commands = remaining;

                    let Some(service) = generation_services.get_mut(model_key.as_str()) else {
                        for (_, reply) in selected {
                            let _ = reply.send(Err(
                                ReferenceTextGenerationError::UnsupportedModel(model_key.clone()),
                            ));
                        }
                        continue;
                    };
                    let requests = selected
                        .iter()
                        .map(|(request, _)| request.clone())
                        .collect::<Vec<_>>();
                    let results = service.generate_continuous_batch(requests);
                    for ((_, reply), result) in selected.into_iter().zip(results.responses) {
                        let _ = reply.send(result);
                    }
                }
            })?;
        match ready_rx.recv().map_err(|error| {
            OpenAiCompatServerError::Config(format!(
                "failed to receive generic OpenAI worker readiness: {error}"
            ))
        })? {
            Ok(()) => Ok(Self { sender }),
            Err(message) => Err(OpenAiCompatServerError::Config(message)),
        }
    }

    async fn generate(
        &self,
        model_key: String,
        request: GenerationRequest,
    ) -> Result<crate::GenerationResponse, ReferenceTextGenerationError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.sender
            .send(OpenAiCompatWorkerCommand::Generate {
                model_key,
                request,
                reply: reply_tx,
            })
            .map_err(|_| {
                ReferenceTextGenerationError::Runtime(psionic_runtime::RuntimeError::Backend(
                    String::from("generic OpenAI worker is no longer available"),
                ))
            })?;
        reply_rx.await.map_err(|_| {
            ReferenceTextGenerationError::Runtime(psionic_runtime::RuntimeError::Backend(
                String::from("generic OpenAI worker dropped the response channel"),
            ))
        })?
    }

    async fn embed(
        &self,
        model_key: String,
        request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, ModelEmbeddingsError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.sender
            .send(OpenAiCompatWorkerCommand::Embed {
                model_key,
                request,
                reply: reply_tx,
            })
            .map_err(|_| {
                ModelEmbeddingsError::Runtime(psionic_runtime::RuntimeError::Backend(String::from(
                    "generic OpenAI worker is no longer available",
                )))
            })?;
        reply_rx.await.map_err(|_| {
            ModelEmbeddingsError::Runtime(psionic_runtime::RuntimeError::Backend(String::from(
                "generic OpenAI worker dropped the response channel",
            )))
        })?
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OpenAiCompatServerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Config(String),
}

#[derive(Debug, thiserror::Error)]
pub enum GptOssOpenAiCompatGenerationError {
    #[error("{backend} backend unavailable ({status:?}): {message}")]
    BackendUnavailable {
        backend: &'static str,
        status: psionic_runtime::HealthStatus,
        message: String,
    },
    #[error(transparent)]
    Generation(#[from] ReferenceTextGenerationError),
}

impl From<CudaGptOssTextGenerationError> for GptOssOpenAiCompatGenerationError {
    fn from(value: CudaGptOssTextGenerationError) -> Self {
        match value {
            CudaGptOssTextGenerationError::BackendUnavailable { status, message } => {
                Self::BackendUnavailable {
                    backend: "cuda",
                    status,
                    message,
                }
            }
            CudaGptOssTextGenerationError::Generation(error) => Self::Generation(error),
        }
    }
}

impl From<MetalGptOssTextGenerationError> for GptOssOpenAiCompatGenerationError {
    fn from(value: MetalGptOssTextGenerationError) -> Self {
        match value {
            MetalGptOssTextGenerationError::BackendUnavailable { status, message } => {
                Self::BackendUnavailable {
                    backend: "metal",
                    status,
                    message,
                }
            }
            MetalGptOssTextGenerationError::Generation(error) => Self::Generation(error),
        }
    }
}

#[derive(Clone)]
struct GptOssWorker {
    sender: mpsc::UnboundedSender<GptOssWorkerCommand>,
}

enum GptOssWorkerCommand {
    Generate {
        request: GenerationRequest,
        reply:
            oneshot::Sender<Result<crate::GenerationResponse, GptOssOpenAiCompatGenerationError>>,
    },
}

impl GptOssWorker {
    fn spawn(
        model_path: PathBuf,
        backend: GptOssOpenAiCompatBackend,
    ) -> Result<Self, OpenAiCompatServerError> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name(format!("psionic-gpt-oss-{}-worker", backend.label()))
            .spawn(move || {
                let ready = match backend {
                    GptOssOpenAiCompatBackend::Cpu => {
                        Err(String::from("cpu GPT-OSS OpenAI server is not implemented"))
                    }
                    GptOssOpenAiCompatBackend::Cuda => {
                        match CudaGgufGptOssTextGenerationService::from_gguf_path(&model_path) {
                            Ok(mut service) => {
                                let _ = ready_tx.send(Ok::<(), String>(()));
                                while let Some(command) = receiver.blocking_recv() {
                                    match command {
                                        GptOssWorkerCommand::Generate { request, reply } => {
                                            let _ = reply.send(
                                                service.generate(&request).map_err(Into::into),
                                            );
                                        }
                                    }
                                }
                                return;
                            }
                            Err(error) => Err(error.to_string()),
                        }
                    }
                    GptOssOpenAiCompatBackend::Metal => {
                        match MetalGgufGptOssTextGenerationService::from_gguf_path(&model_path) {
                            Ok(mut service) => {
                                let _ = ready_tx.send(Ok::<(), String>(()));
                                while let Some(command) = receiver.blocking_recv() {
                                    match command {
                                        GptOssWorkerCommand::Generate { request, reply } => {
                                            let _ = reply.send(
                                                service.generate(&request).map_err(Into::into),
                                            );
                                        }
                                    }
                                }
                                return;
                            }
                            Err(error) => Err(error.to_string()),
                        }
                    }
                    GptOssOpenAiCompatBackend::Auto => Err(String::from(
                        "auto backend must be resolved before worker spawn",
                    )),
                };
                let _ = ready_tx.send(ready);
            })?;
        match ready_rx.recv().map_err(|error| {
            OpenAiCompatServerError::Config(format!(
                "failed to receive GPT-OSS {} worker readiness: {error}",
                backend.label()
            ))
        })? {
            Ok(()) => Ok(Self { sender }),
            Err(message) => Err(OpenAiCompatServerError::Config(message)),
        }
    }

    async fn generate(
        &self,
        request: GenerationRequest,
    ) -> Result<crate::GenerationResponse, GptOssOpenAiCompatGenerationError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.sender
            .send(GptOssWorkerCommand::Generate {
                request,
                reply: reply_tx,
            })
            .map_err(|_| GptOssOpenAiCompatGenerationError::BackendUnavailable {
                backend: "worker",
                status: psionic_runtime::HealthStatus::Offline,
                message: String::from("gpt-oss worker is no longer available"),
            })?;
        reply_rx
            .await
            .map_err(|_| GptOssOpenAiCompatGenerationError::BackendUnavailable {
                backend: "worker",
                status: psionic_runtime::HealthStatus::Offline,
                message: String::from("gpt-oss worker dropped the response channel"),
            })?
    }
}

fn metal_proxy_llama_cpp_enabled() -> bool {
    env::var("PSIONIC_METAL_PROXY_LLAMA_CPP")
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

impl LlamaCppProxyState {
    fn spawn(config: &GptOssOpenAiCompatConfig) -> Result<Self, OpenAiCompatServerError> {
        let llama_bin = env::var("PSIONIC_LLAMA_SERVER_BIN").unwrap_or_else(|_| {
            if cfg!(target_os = "macos") {
                String::from("/Users/christopherdavid/code/llama.cpp/build/bin/llama-server")
            } else {
                String::from("/home/christopherdavid/code/llama.cpp/build/bin/llama-server")
            }
        });
        let internal_port = reserve_local_port()?;
        let host = "127.0.0.1";
        let mut command = Command::new(&llama_bin);
        let ctx = config
            .context_length
            .unwrap_or(if cfg!(target_os = "macos") {
                1024
            } else {
                4096
            });
        let gpu_layers =
            config
                .gpu_layers
                .unwrap_or(if cfg!(target_os = "macos") { 4 } else { 999 });
        let batch_size = env::var("PSIONIC_LLAMA_BATCH_SIZE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(if cfg!(target_os = "macos") { 64 } else { 2048 });
        let ubatch_size = env::var("PSIONIC_LLAMA_UBATCH_SIZE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(if cfg!(target_os = "macos") { 64 } else { 512 });
        command
            .arg("-m")
            .arg(&config.model_path)
            .arg("--host")
            .arg(host)
            .arg("--port")
            .arg(internal_port.to_string())
            .arg("-c")
            .arg(ctx.to_string())
            .arg("-b")
            .arg(batch_size.to_string())
            .arg("-ub")
            .arg(ubatch_size.to_string())
            .arg("-ngl")
            .arg(gpu_layers.to_string())
            .arg("--reasoning-budget")
            .arg(config.reasoning_budget.to_string())
            .arg("--no-webui")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if cfg!(target_os = "macos")
            && env::var("PSIONIC_LLAMA_DISABLE_CPU_MOE")
                .ok()
                .map(|value| !matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(true)
        {
            command.arg("--cpu-moe");
        }
        let child = command.spawn().map_err(|error| {
            OpenAiCompatServerError::Config(format!(
                "failed to spawn llama.cpp proxy backend `{llama_bin}`: {error}"
            ))
        })?;
        let base_url = format!("http://{host}:{internal_port}");
        wait_for_upstream_ready(base_url.as_str(), config.model_path.as_path())?;
        Ok(Self {
            base_url,
            client: reqwest::Client::new(),
            child: Mutex::new(Some(child)),
        })
    }
}

fn reserve_local_port() -> Result<u16, OpenAiCompatServerError> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|error| {
        OpenAiCompatServerError::Config(format!("failed to reserve local proxy port: {error}"))
    })?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|error| {
            OpenAiCompatServerError::Config(format!("failed to query reserved proxy port: {error}"))
        })
}

fn wait_for_upstream_ready(
    base_url: &str,
    model_path: &Path,
) -> Result<(), OpenAiCompatServerError> {
    const HEALTH_TIMEOUT: Duration = Duration::from_secs(1);
    const CHAT_TIMEOUT: Duration = Duration::from_secs(10);

    let health_url = format!("{base_url}/health");
    let chat_url = format!("{base_url}/v1/chat/completions");
    let model_name = model_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            OpenAiCompatServerError::Config(format!(
                "failed to derive proxy model name from {}",
                model_path.display()
            ))
        })?;
    let health_client = reqwest::blocking::Client::builder()
        .timeout(HEALTH_TIMEOUT)
        .build()
        .map_err(|error| {
            OpenAiCompatServerError::Config(format!(
                "failed to build llama.cpp proxy health client: {error}"
            ))
        })?;
    let chat_client = reqwest::blocking::Client::builder()
        .timeout(CHAT_TIMEOUT)
        .build()
        .map_err(|error| {
            OpenAiCompatServerError::Config(format!(
                "failed to build llama.cpp proxy chat client: {error}"
            ))
        })?;
    let probe = serde_json::json!({
        "model": model_name,
        "messages": [
            {
                "role": "user",
                "content": "Say hello."
            }
        ],
        "max_tokens": 1,
        "temperature": 0
    });
    for _ in 0..300 {
        let health_ready = matches!(
            health_client.get(health_url.as_str()).send(),
            Ok(response) if response.status().is_success()
        );
        if health_ready {
            match chat_client.post(chat_url.as_str()).json(&probe).send() {
                Ok(response) if response.status().is_success() => return Ok(()),
                Ok(response) if response.status() != reqwest::StatusCode::SERVICE_UNAVAILABLE => {
                    return Err(OpenAiCompatServerError::Config(format!(
                        "llama.cpp proxy readiness probe failed with status {}",
                        response.status()
                    )));
                }
                Ok(_) | Err(_) => {}
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err(OpenAiCompatServerError::Config(format!(
        "llama.cpp proxy backend did not become ready for chat completions: {chat_url}"
    )))
}

#[derive(Debug, thiserror::Error)]
enum OpenAiCompatHttpError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Internal(String),
    #[error(transparent)]
    PromptRender(Box<PromptRenderError>),
    #[error(transparent)]
    Embeddings(Box<ModelEmbeddingsError>),
    #[error(transparent)]
    Generation(Box<GptOssOpenAiCompatGenerationError>),
}

impl From<PromptRenderError> for OpenAiCompatHttpError {
    fn from(value: PromptRenderError) -> Self {
        Self::PromptRender(Box::new(value))
    }
}

impl From<GptOssOpenAiCompatGenerationError> for OpenAiCompatHttpError {
    fn from(value: GptOssOpenAiCompatGenerationError) -> Self {
        Self::Generation(Box::new(value))
    }
}

impl From<ModelEmbeddingsError> for OpenAiCompatHttpError {
    fn from(value: ModelEmbeddingsError) -> Self {
        Self::Embeddings(Box::new(value))
    }
}

impl IntoResponse for OpenAiCompatHttpError {
    fn into_response(self) -> Response {
        let (status, kind) = match &self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
            Self::PromptRender(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
            Self::Embeddings(error) => (
                StatusCode::from_u16(error.diagnostic().status)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                "embeddings_error",
            ),
            Self::Generation(error) => match error.as_ref() {
                GptOssOpenAiCompatGenerationError::BackendUnavailable { .. } => {
                    (StatusCode::SERVICE_UNAVAILABLE, "backend_unavailable")
                }
                GptOssOpenAiCompatGenerationError::Generation(error) => (
                    StatusCode::from_u16(error.diagnostic().status)
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    "generation_error",
                ),
            },
        };
        (
            status,
            Json(OpenAiErrorEnvelope {
                error: OpenAiErrorBody {
                    message: self.to_string(),
                    kind: String::from(kind),
                },
            }),
        )
            .into_response()
    }
}

#[derive(Clone, Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    backend: &'static str,
    execution_mode: &'static str,
    execution_engine: &'static str,
    model: String,
}

async fn health(State(state): State<Arc<GptOssOpenAiCompatState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        backend: state.backend_label,
        execution_mode: state.execution_mode_label,
        execution_engine: state.execution_engine_label,
        model: state.default_model_name.clone(),
    })
}

#[derive(Clone, Debug, Serialize)]
struct ModelsResponse {
    data: Vec<ModelCard>,
}

#[derive(Clone, Debug, Serialize)]
struct ModelCard {
    id: String,
    object: &'static str,
    owned_by: &'static str,
    psionic_supported_endpoints: Vec<&'static str>,
    psionic_model_family: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_served_backend: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_residency_mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_hybrid_offload: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_fallback_policy: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_performance_class: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_structured_outputs: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_structured_output_capabilities: Option<Vec<StructuredOutputCapability>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_tool_calling: Option<ToolCallingCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_response_state: Option<ResponseStateCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_execution_profile: Option<ExecutionCapabilityProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_scheduler_policy: Option<GenerationSchedulerPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_embedding_dimensions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_embedding_normalization: Option<EmbeddingNormalization>,
}

async fn list_models(State(state): State<Arc<GptOssOpenAiCompatState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        data: vec![ModelCard {
            id: state.default_model_name.clone(),
            object: "model",
            owned_by: "psionic",
            psionic_supported_endpoints: vec![RoutingEndpoint::ChatCompletions.path()],
            psionic_model_family: state.descriptor.model.family.clone(),
            psionic_served_backend: None,
            psionic_residency_mode: None,
            psionic_hybrid_offload: None,
            psionic_fallback_policy: None,
            psionic_performance_class: None,
            psionic_structured_outputs: None,
            psionic_structured_output_capabilities: None,
            psionic_tool_calling: None,
            psionic_response_state: None,
            psionic_execution_profile: None,
            psionic_scheduler_policy: None,
            psionic_embedding_dimensions: None,
            psionic_embedding_normalization: None,
        }],
    })
}

#[derive(Clone, Debug, Serialize)]
struct GenericHealthResponse {
    status: &'static str,
    backend: &'static str,
    execution_mode: &'static str,
    execution_engine: &'static str,
    default_model: String,
    model_count: usize,
    residency_mode: &'static str,
    hybrid_offload: &'static str,
    fallback_policy: &'static str,
    performance_class: &'static str,
    load_status: &'static str,
    warm_control: &'static str,
    unload_control: &'static str,
    memory_pressure_reporting: &'static str,
    default_model_supported_endpoints: Vec<&'static str>,
    supported_endpoints: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    structured_output_fallbacks: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    structured_output_capabilities: Option<Vec<StructuredOutputCapability>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calling: Option<ToolCallingCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_state: Option<ResponseStateCapability>,
    execution_profile: ExecutionCapabilityProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduler_policy: Option<GenerationSchedulerPolicy>,
}

async fn generic_health(
    State(state): State<Arc<OpenAiCompatState>>,
) -> Json<GenericHealthResponse> {
    let default_model = state
        .models_by_key
        .get(&state.default_model_key)
        .expect("default model should exist");
    Json(GenericHealthResponse {
        status: "ok",
        backend: state.backend_label,
        execution_mode: state.execution_mode_label,
        execution_engine: state.execution_engine_label,
        default_model: state.default_model_name.clone(),
        model_count: state.models_by_key.len(),
        residency_mode: CPU_SERVER_RESIDENCY_MODE,
        hybrid_offload: CPU_SERVER_HYBRID_OFFLOAD_MODE,
        fallback_policy: CPU_SERVER_FALLBACK_POLICY,
        performance_class: CPU_SERVER_PERFORMANCE_CLASS,
        load_status: CPU_SERVER_LOAD_STATUS,
        warm_control: CPU_SERVER_WARM_CONTROL,
        unload_control: CPU_SERVER_UNLOAD_CONTROL,
        memory_pressure_reporting: CPU_SERVER_MEMORY_PRESSURE_REPORTING,
        default_model_supported_endpoints: model_endpoint_paths(default_model),
        supported_endpoints: union_supported_endpoint_paths(state.as_ref()),
        structured_output_fallbacks: default_model.structured_output_labels(),
        structured_output_capabilities: Some(default_model.structured_output_capabilities()),
        tool_calling: Some(default_model.tool_calling_capability()),
        response_state: default_model
            .decoder()
            .map(|_| state.response_state_capability.clone()),
        execution_profile: default_model.execution_profile().clone(),
        scheduler_policy: default_model.scheduler_policy().cloned(),
    })
}

async fn generic_list_models(State(state): State<Arc<OpenAiCompatState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        data: state
            .models_by_key
            .values()
            .map(|model| ModelCard {
                id: model.canonical_name.clone(),
                object: "model",
                owned_by: "psionic",
                psionic_supported_endpoints: model_endpoint_paths(model),
                psionic_model_family: model.family_label().to_string(),
                psionic_served_backend: Some("cpu"),
                psionic_residency_mode: Some(CPU_SERVER_RESIDENCY_MODE),
                psionic_hybrid_offload: Some(CPU_SERVER_HYBRID_OFFLOAD_MODE),
                psionic_fallback_policy: Some(CPU_SERVER_FALLBACK_POLICY),
                psionic_performance_class: Some(CPU_SERVER_PERFORMANCE_CLASS),
                psionic_structured_outputs: model.structured_output_labels(),
                psionic_structured_output_capabilities: Some(
                    model.structured_output_capabilities(),
                ),
                psionic_tool_calling: Some(model.tool_calling_capability()),
                psionic_response_state: model
                    .decoder()
                    .map(|_| state.response_state_capability.clone()),
                psionic_execution_profile: Some(model.execution_profile().clone()),
                psionic_scheduler_policy: model.scheduler_policy().cloned(),
                psionic_embedding_dimensions: model.embedding_dimensions(),
                psionic_embedding_normalization: model.embedding_normalization(),
            })
            .collect(),
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ChatCompletionRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    messages: Vec<ChatCompletionMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stop: Option<StopSequences>,
    #[serde(default)]
    stream: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolDefinitionEnvelope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoiceRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response_format: Option<ChatCompletionResponseFormatRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_grammar: Option<PsionicGrammarRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_structured_output: Option<StructuredOutputRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_reasoning: Option<PsionicReasoningRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_prefix_cache: Option<PrefixCacheControl>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum PsionicReasoningMode {
    #[default]
    Separate,
    Suppress,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct PsionicReasoningRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parser: Option<ReasoningParser>,
    #[serde(default)]
    mode: PsionicReasoningMode,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ToolDefinitionEnvelope {
    #[serde(rename = "type")]
    kind: String,
    function: ToolDefinitionRequest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ToolDefinitionRequest {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parameters: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum ToolChoiceRequest {
    Mode(String),
    Named(NamedToolChoiceRequest),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct NamedToolChoiceRequest {
    #[serde(rename = "type")]
    kind: String,
    function: NamedToolChoiceFunction,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct NamedToolChoiceFunction {
    name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct ChatCompletionMessage {
    role: String,
    content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum StopSequences {
    One(String),
    Many(Vec<String>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ChatCompletionResponseFormatRequest {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    json_schema: Option<ChatCompletionJsonSchemaRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    schema: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ChatCompletionJsonSchemaRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PsionicGrammarRequest {
    grammar: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    syntax: Option<StructuredGrammarSyntax>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EmbeddingsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    input: EmbeddingsInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    encoding_format: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum EmbeddingsInput {
    One(String),
    Many(Vec<String>),
}

impl EmbeddingsInput {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ResponsesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conversation: Option<String>,
    input: ResponsesInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stop: Option<StopSequences>,
    #[serde(default)]
    stream: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolDefinitionEnvelope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoiceRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_response_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_structured_output: Option<StructuredOutputRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_reasoning: Option<PsionicReasoningRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_response_state: Option<PsionicResponseStateRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_prefix_cache: Option<PrefixCacheControl>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum ResponsesInput {
    Text(String),
    Messages(Vec<ChatCompletionMessage>),
}

impl StopSequences {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

fn structured_output_from_chat_request(
    request: &ChatCompletionRequest,
) -> Result<Option<StructuredOutputRequest>, OpenAiCompatHttpError> {
    let surfaces = usize::from(request.response_format.is_some())
        + usize::from(request.psionic_grammar.is_some())
        + usize::from(request.psionic_structured_output.is_some());
    if surfaces > 1 {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "structured output accepts exactly one of `psionic_structured_output`, `response_format`, or `psionic_grammar`",
        )));
    }

    if let Some(structured_output) = request.psionic_structured_output.clone() {
        return validate_structured_output_request(structured_output).map(Some);
    }

    if let Some(grammar) = &request.psionic_grammar {
        if grammar.grammar.trim().is_empty() {
            return Err(OpenAiCompatHttpError::BadRequest(String::from(
                "`psionic_grammar.grammar` must not be empty",
            )));
        }
        return validate_structured_output_request(StructuredOutputRequest::Grammar {
            syntax: grammar.syntax.unwrap_or(StructuredGrammarSyntax::Gbnf),
            grammar: grammar.grammar.clone(),
        })
        .map(Some);
    }

    let Some(response_format) = &request.response_format else {
        return Ok(None);
    };
    match response_format.kind.as_str() {
        "json_object" => {
            if let Some(schema) = response_format.schema.as_ref() {
                validate_structured_output_request(StructuredOutputRequest::JsonSchema {
                    name: None,
                    schema: schema.clone(),
                })
                .map(Some)
            } else {
                validate_structured_output_request(StructuredOutputRequest::JsonObject).map(Some)
            }
        }
        "json_schema" => {
            let Some(schema) = response_format.json_schema.as_ref() else {
                return Err(OpenAiCompatHttpError::BadRequest(String::from(
                    "`response_format.type = json_schema` requires a `json_schema` object",
                )));
            };
            validate_structured_output_request(StructuredOutputRequest::JsonSchema {
                name: schema.name.clone(),
                schema: schema.schema.clone(),
            })
            .map(Some)
        }
        other => Err(OpenAiCompatHttpError::BadRequest(format!(
            "unsupported `response_format.type` `{other}` for local structured output fallback"
        ))),
    }
}

fn structured_output_from_responses_request(
    request: &ResponsesRequest,
) -> Result<Option<StructuredOutputRequest>, OpenAiCompatHttpError> {
    let Some(structured_output) = request.psionic_structured_output.clone() else {
        return Ok(None);
    };
    validate_structured_output_request(structured_output).map(Some)
}

fn validate_structured_output_request(
    structured_output: StructuredOutputRequest,
) -> Result<StructuredOutputRequest, OpenAiCompatHttpError> {
    StructuredOutputMatcher::compile(structured_output.clone())
        .map_err(|error| OpenAiCompatHttpError::BadRequest(error.to_string()))?;
    Ok(structured_output)
}

fn reasoning_request_for_family(
    request: Option<&PsionicReasoningRequest>,
    family: GgufDecoderFamily,
) -> Result<Option<ResolvedReasoningRequest>, OpenAiCompatHttpError> {
    let Some(request) = request else {
        return Ok(None);
    };
    let Some(family_parser) = reasoning_parser_for_decoder_family(family) else {
        return Err(OpenAiCompatHttpError::BadRequest(format!(
            "model family `{}` does not expose a Psionic reasoning parser",
            decoder_family_label(family)
        )));
    };
    if let Some(parser) = request.parser
        && parser != family_parser
    {
        return Err(OpenAiCompatHttpError::BadRequest(format!(
            "requested reasoning parser `{}` does not match the `{}` parser for model family `{}`",
            parser.label(),
            family_parser.label(),
            decoder_family_label(family)
        )));
    }
    Ok(Some(ResolvedReasoningRequest {
        parser: request.parser.unwrap_or(family_parser),
        mode: request.mode,
    }))
}

fn decoder_family_label(family: GgufDecoderFamily) -> &'static str {
    match family {
        GgufDecoderFamily::Llama => "llama",
        GgufDecoderFamily::Qwen => "qwen",
        GgufDecoderFamily::Mistral => "mistral",
        GgufDecoderFamily::GptOss => "gpt_oss",
    }
}

fn default_response_state_store() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn resolved_response_state_request(
    request: &ResponsesRequest,
) -> Result<PsionicResponseStateRequest, OpenAiCompatHttpError> {
    if request.previous_response_id.is_some() && request.conversation.is_some() {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "`conversation` and `previous_response_id` are mutually exclusive on `/v1/responses`",
        )));
    }
    Ok(request.psionic_response_state.clone().unwrap_or_default())
}

fn next_conversation_id(state: &OpenAiCompatState) -> String {
    let next = state.conversation_counter.fetch_add(1, Ordering::Relaxed);
    format!("psionic-conv-{next}")
}

fn current_response_state_capability(state: &OpenAiCompatState) -> ResponseStateCapability {
    state.response_state_capability.clone()
}

fn response_state_error_into_http(error: ResponseStateError) -> OpenAiCompatHttpError {
    match error {
        ResponseStateError::UnknownResponseState { response_id } => {
            OpenAiCompatHttpError::BadRequest(format!(
                "response state `{response_id}` is unknown or expired"
            ))
        }
        ResponseStateError::UnknownConversationState { conversation_id } => {
            OpenAiCompatHttpError::BadRequest(format!(
                "conversation state `{conversation_id}` is unknown or expired"
            ))
        }
        ResponseStateError::ConversationTooLarge {
            max_items_per_conversation,
            ..
        } => OpenAiCompatHttpError::BadRequest(format!(
            "stateful response exceeds the bounded conversation-state limit of {max_items_per_conversation} prompt messages"
        )),
        ResponseStateError::IoRead { .. }
        | ResponseStateError::IoWrite { .. }
        | ResponseStateError::Deserialize { .. }
        | ResponseStateError::Serialize { .. } => OpenAiCompatHttpError::Internal(format!(
            "generic response-state backend failed: {error}"
        )),
    }
}

fn parse_reasoning_response_for_family(
    family: GgufDecoderFamily,
    text: &str,
) -> Result<Option<ParsedReasoningResponse>, OpenAiCompatHttpError> {
    parse_reasoning_response_text_for_decoder_family(
        family,
        text,
        GptOssHarmonyParseOptions {
            role_hint: Some(PromptMessageRole::Assistant),
            strict: false,
        },
    )
    .map_err(|error| OpenAiCompatHttpError::BadRequest(error.to_string()))
}

fn surfaced_reasoning_response(
    parsed: Option<&ParsedReasoningResponse>,
    request: Option<&ResolvedReasoningRequest>,
    include_debug_fields: bool,
) -> Option<ParsedReasoningResponse> {
    let parsed = parsed?;
    if let Some(request) = request {
        return Some(match request.mode {
            PsionicReasoningMode::Separate => parsed.clone(),
            PsionicReasoningMode::Suppress => parsed.suppress_reasoning(),
        });
    }
    include_debug_fields.then(|| parsed.clone())
}

fn tool_contract_from_chat_request(
    request: &ChatCompletionRequest,
    structured_output_requested: bool,
) -> Result<Option<ToolCallingContract>, OpenAiCompatHttpError> {
    validate_tool_contract(
        request.tools.as_slice(),
        request.tool_choice.as_ref(),
        structured_output_requested,
    )
}

fn tool_contract_from_responses_request(
    request: &ResponsesRequest,
    structured_output_requested: bool,
) -> Result<Option<ToolCallingContract>, OpenAiCompatHttpError> {
    validate_tool_contract(
        request.tools.as_slice(),
        request.tool_choice.as_ref(),
        structured_output_requested,
    )
}

fn validate_tool_contract(
    tools: &[ToolDefinitionEnvelope],
    tool_choice: Option<&ToolChoiceRequest>,
    structured_output_requested: bool,
) -> Result<Option<ToolCallingContract>, OpenAiCompatHttpError> {
    if tools.is_empty() {
        if tool_choice.is_some() {
            return Err(OpenAiCompatHttpError::BadRequest(String::from(
                "`tool_choice` requires at least one declared tool",
            )));
        }
        return Ok(None);
    }

    let mut tool_map = BTreeMap::new();
    for tool in tools {
        if tool.kind != "function" {
            return Err(OpenAiCompatHttpError::BadRequest(format!(
                "unsupported tool type `{}`; only `function` is supported",
                tool.kind
            )));
        }
        if tool.function.name.trim().is_empty() {
            return Err(OpenAiCompatHttpError::BadRequest(String::from(
                "tool function names must not be empty",
            )));
        }
        if tool_map
            .insert(tool.function.name.clone(), tool.function.clone())
            .is_some()
        {
            return Err(OpenAiCompatHttpError::BadRequest(format!(
                "duplicate tool definition `{}`",
                tool.function.name
            )));
        }
        let _ = normalized_tool_parameters_schema(&tool.function)?;
    }

    let (mode, named_tool) = match tool_choice {
        None => (ToolChoiceMode::Auto, None),
        Some(ToolChoiceRequest::Mode(value)) => match value.as_str() {
            "none" => (ToolChoiceMode::None, None),
            "auto" => (ToolChoiceMode::Auto, None),
            "required" => (ToolChoiceMode::Required, None),
            other => {
                return Err(OpenAiCompatHttpError::BadRequest(format!(
                    "unsupported `tool_choice` mode `{other}`"
                )));
            }
        },
        Some(ToolChoiceRequest::Named(named)) => {
            if named.kind != "function" {
                return Err(OpenAiCompatHttpError::BadRequest(format!(
                    "unsupported named tool choice type `{}`",
                    named.kind
                )));
            }
            if !tool_map.contains_key(named.function.name.as_str()) {
                return Err(OpenAiCompatHttpError::BadRequest(format!(
                    "named tool choice `{}` does not match a declared tool",
                    named.function.name
                )));
            }
            (ToolChoiceMode::Named, Some(named.function.name.clone()))
        }
    };

    if structured_output_requested && !matches!(mode, ToolChoiceMode::None) {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "tool-calling modes cannot be combined with `psionic_structured_output`, `response_format`, or `psionic_grammar` on the same request",
        )));
    }

    Ok(Some(ToolCallingContract {
        tools: tool_map,
        mode,
        named_tool,
    }))
}

fn normalized_tool_parameters_schema(
    tool: &ToolDefinitionRequest,
) -> Result<serde_json::Value, OpenAiCompatHttpError> {
    let mut schema = match tool.parameters.clone() {
        Some(serde_json::Value::Object(map)) => map,
        Some(_) => {
            return Err(OpenAiCompatHttpError::BadRequest(format!(
                "tool `{}` parameters must be a JSON object schema",
                tool.name
            )));
        }
        None => serde_json::Map::new(),
    };
    match schema.get("type") {
        Some(serde_json::Value::String(kind)) if kind == "object" => {}
        Some(_) => {
            return Err(OpenAiCompatHttpError::BadRequest(format!(
                "tool `{}` parameters must describe an object schema",
                tool.name
            )));
        }
        None => {
            schema.insert(
                String::from("type"),
                serde_json::Value::String(String::from("object")),
            );
        }
    }
    if !schema.contains_key("properties") {
        schema.insert(
            String::from("properties"),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    }
    if !schema.contains_key("additionalProperties") {
        schema.insert(
            String::from("additionalProperties"),
            serde_json::Value::Bool(false),
        );
    }
    Ok(serde_json::Value::Object(schema))
}

fn tool_prompt_message(contract: &ToolCallingContract) -> PromptMessage {
    let mut lines = vec![String::from(
        "When tools are enabled, respond with exactly one JSON object that matches the declared Psionic tool contract.",
    )];
    match contract.mode {
        ToolChoiceMode::None => lines.push(String::from(
            "Tool use is disabled for this request. Answer normally.",
        )),
        ToolChoiceMode::Auto => lines.push(String::from(
            "Use `{ \"kind\": \"message\", \"content\": \"...\" }` for a normal answer, or `{ \"kind\": \"tool:<name>\", ...tool arguments... }` to call exactly one tool.",
        )),
        ToolChoiceMode::Required => lines.push(String::from(
            "You must call exactly one tool using `{ \"kind\": \"tool:<name>\", ...tool arguments... }`.",
        )),
        ToolChoiceMode::Named => lines.push(format!(
            "You must call exactly one tool using `{{ \"kind\": \"tool:{}\", ...tool arguments... }}`.",
            contract.named_tool.as_deref().unwrap_or_default()
        )),
    }
    lines.push(String::from("Declared tools:"));
    for tool in contract.tools.values() {
        let schema =
            normalized_tool_parameters_schema(tool).unwrap_or_else(|_| serde_json::json!({}));
        lines.push(format!(
            "- {}: {} | schema={}",
            tool.name,
            tool.description
                .clone()
                .unwrap_or_else(|| String::from("no description")),
            schema
        ));
    }
    PromptMessage::new(PromptMessageRole::Developer, lines.join("\n"))
}

fn apply_tool_contract_to_prompt_messages(
    mut messages: Vec<PromptMessage>,
    contract: Option<&ToolCallingContract>,
) -> Vec<PromptMessage> {
    if let Some(contract) = contract
        && !matches!(contract.mode, ToolChoiceMode::None)
    {
        messages.insert(0, tool_prompt_message(contract));
    }
    messages
}

fn structured_output_from_tool_contract(
    contract: Option<&ToolCallingContract>,
) -> Result<Option<StructuredOutputRequest>, OpenAiCompatHttpError> {
    let Some(contract) = contract else {
        return Ok(None);
    };
    if matches!(contract.mode, ToolChoiceMode::None) {
        return Ok(None);
    }

    let mut variants = Vec::new();
    if matches!(contract.mode, ToolChoiceMode::Auto) {
        variants.push(StructuredTaggedVariant {
            tag: String::from("message"),
            schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "minLength": 1 }
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        });
    }

    match contract.mode {
        ToolChoiceMode::Named => {
            let name = contract.named_tool.as_ref().ok_or_else(|| {
                OpenAiCompatHttpError::Internal(String::from(
                    "named tool mode is missing the selected tool",
                ))
            })?;
            let tool = contract.tools.get(name).ok_or_else(|| {
                OpenAiCompatHttpError::Internal(format!(
                    "named tool `{name}` is missing from the validated tool map"
                ))
            })?;
            variants.push(StructuredTaggedVariant {
                tag: tool_variant_tag(name),
                schema: normalized_tool_parameters_schema(tool)?,
            });
        }
        ToolChoiceMode::Auto | ToolChoiceMode::Required => {
            for (name, tool) in &contract.tools {
                variants.push(StructuredTaggedVariant {
                    tag: tool_variant_tag(name),
                    schema: normalized_tool_parameters_schema(tool)?,
                });
            }
        }
        ToolChoiceMode::None => {}
    }

    validate_structured_output_request(StructuredOutputRequest::TaggedStructure {
        name: Some(String::from("psionic_tool_call")),
        discriminator: String::from("kind"),
        variants,
    })
    .map(Some)
}

fn tool_variant_tag(name: &str) -> String {
    format!("tool:{name}")
}

fn tool_call_outcome_from_response(
    request_id: &str,
    response: &crate::GenerationResponse,
    contract: Option<&ToolCallingContract>,
) -> Result<Option<ToolCallOutcome>, OpenAiCompatHttpError> {
    let Some(contract) = contract else {
        return Ok(None);
    };
    if matches!(contract.mode, ToolChoiceMode::None) {
        return Ok(None);
    }

    let Some(StructuredOutputValue::TaggedStructure {
        discriminator,
        tag,
        value,
    }) = response.output.structured.clone()
    else {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "tool-calling request completed without a machine-readable tool envelope",
        )));
    };
    if discriminator != "kind" {
        return Err(OpenAiCompatHttpError::BadRequest(format!(
            "unexpected tool envelope discriminator `{discriminator}`"
        )));
    }

    if tag == "message" {
        let content = value
            .get("content")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                OpenAiCompatHttpError::BadRequest(String::from(
                    "tool auto-mode message envelope is missing string `content`",
                ))
            })?
            .to_string();
        return Ok(Some(ToolCallOutcome {
            content: Some(content),
            tool_calls: Vec::new(),
        }));
    }

    let Some(tool_name) = tag.strip_prefix("tool:") else {
        return Err(OpenAiCompatHttpError::BadRequest(format!(
            "unexpected tool envelope tag `{tag}`"
        )));
    };
    let tool = contract.tools.get(tool_name).ok_or_else(|| {
        OpenAiCompatHttpError::BadRequest(format!("model selected undeclared tool `{tool_name}`"))
    })?;
    let mut arguments = match value {
        serde_json::Value::Object(map) => map,
        _ => {
            return Err(OpenAiCompatHttpError::BadRequest(format!(
                "tool envelope for `{tool_name}` must be a JSON object"
            )));
        }
    };
    let _ = arguments.remove("kind");
    let arguments = serde_json::Value::Object(arguments);
    validate_tool_arguments(tool, &arguments)?;
    Ok(Some(ToolCallOutcome {
        content: None,
        tool_calls: vec![ResolvedToolCall {
            id: format!("{request_id}-tool-0"),
            name: tool_name.to_string(),
            arguments,
        }],
    }))
}

fn validate_tool_arguments(
    tool: &ToolDefinitionRequest,
    arguments: &serde_json::Value,
) -> Result<(), OpenAiCompatHttpError> {
    let schema = normalized_tool_parameters_schema(tool)?;
    let matcher = StructuredOutputMatcher::compile(StructuredOutputRequest::JsonSchema {
        name: Some(format!("tool:{} arguments", tool.name)),
        schema,
    })
    .map_err(|error| OpenAiCompatHttpError::BadRequest(error.to_string()))?;
    let raw = serde_json::to_string(arguments).map_err(|error| {
        OpenAiCompatHttpError::BadRequest(format!(
            "failed to serialize arguments for tool `{}`: {error}",
            tool.name
        ))
    })?;
    matcher
        .materialize(raw.as_str())
        .map_err(|error| OpenAiCompatHttpError::BadRequest(error.to_string()))?
        .ok_or_else(|| {
            OpenAiCompatHttpError::BadRequest(format!(
                "arguments for tool `{}` did not satisfy the declared schema",
                tool.name
            ))
        })?;
    Ok(())
}

async fn chat_completions(
    State(state): State<Arc<GptOssOpenAiCompatState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    match handle_chat_completions(state, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn handle_chat_completions(
    state: Arc<GptOssOpenAiCompatState>,
    request: ChatCompletionRequest,
) -> Result<Response, OpenAiCompatHttpError> {
    let reasoning_request = reasoning_request_for_family(
        request.psionic_reasoning.as_ref(),
        GgufDecoderFamily::GptOss,
    )?;
    let tool_contract = tool_contract_from_chat_request(&request, false)?;
    if tool_contract
        .as_ref()
        .is_some_and(|contract| !matches!(contract.mode, ToolChoiceMode::None))
    {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "tool-calling modes are only available on the generic Psionic server today",
        )));
    }
    if structured_output_from_chat_request(&request)?.is_some() {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "structured output fallback is only available on `psionic-openai-server` today",
        )));
    }
    if state.proxy.is_some() && reasoning_request.is_some() {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "psionic reasoning separation is unavailable while the GPT-OSS endpoint is proxying through llama.cpp",
        )));
    }
    if let Some(proxy) = state.proxy.as_ref() {
        return proxy_chat_completions(state.as_ref(), proxy, &request).await;
    }
    validate_requested_model(request.model.as_deref(), &state.accepted_model_names)?;
    let prompt_messages = chat_messages_to_prompt_messages(&request.messages)?;
    let request_prompt_key = prompt_request_cache_key(prompt_messages.as_slice());
    let request_id = next_request_id(&state);
    let response_model_name = request
        .model
        .clone()
        .unwrap_or_else(|| state.default_model_name.clone());
    let options = generation_options_from_chat_request(&request);
    let prompt_tokens = {
        let mut cache = state.prompt_token_cache.lock().map_err(|_| {
            OpenAiCompatHttpError::from(GptOssOpenAiCompatGenerationError::Generation(
                ReferenceTextGenerationError::Runtime(psionic_runtime::RuntimeError::Backend(
                    String::from("openai prompt token cache is poisoned"),
                )),
            ))
        })?;
        if let Some(tokens) = cache.lookup(request_prompt_key.as_str()) {
            tokens
        } else {
            let rendered = render_gpt_oss_harmony_prompt(
                prompt_messages.as_slice(),
                true,
                Some(&state.prompt_options),
            )
            .map_err(|error| {
                OpenAiCompatHttpError::from(PromptRenderError::HarmonyRendering {
                    message: error.to_string(),
                })
            })?;
            let tokens = state.tokenizer.encode_with_defaults(rendered.as_str());
            cache.record(request_prompt_key, tokens.clone());
            tokens
        }
    };
    let generation_request = GenerationRequest::new_tokens(
        request_id.clone(),
        state.descriptor.clone(),
        None,
        prompt_tokens,
        options,
    )
    .with_prefix_cache_control(request.psionic_prefix_cache.clone().unwrap_or_default());

    let worker = state.worker.as_ref().ok_or_else(|| {
        OpenAiCompatHttpError::from(GptOssOpenAiCompatGenerationError::BackendUnavailable {
            backend: state.backend_label,
            status: psionic_runtime::HealthStatus::Offline,
            message: String::from("gpt-oss native worker is not available"),
        })
    })?;
    let response = worker.generate(generation_request).await?;
    let parsed = parse_gpt_oss_harmony_text(
        response.output.text.as_str(),
        GptOssHarmonyParseOptions {
            role_hint: Some(PromptMessageRole::Assistant),
            strict: false,
        },
    )
    .ok();
    let parsed_reasoning = parsed
        .as_ref()
        .map(GptOssHarmonyParsedOutput::reasoning_response);
    let choice = completion_choice(
        &response,
        parsed_reasoning.as_ref(),
        reasoning_request.as_ref(),
    );
    if request.stream {
        let terminal_chunk = completion_terminal_chunk(
            request_id.as_str(),
            &response_model_name,
            response.termination,
            Some(choice.finish_reason),
            unix_timestamp_secs(),
        );
        let delta_chunk = serialize_event_data(&completion_delta_chunk(
            request_id.as_str(),
            response_model_name.as_str(),
            choice.content.clone(),
            choice.reasoning_content.clone(),
            (!choice.tool_calls.is_empty()).then_some(choice.tool_calls.clone()),
            unix_timestamp_secs(),
        ))?;
        let terminal_chunk = serialize_event_data(&terminal_chunk)?;
        let events = vec![
            Ok::<_, Infallible>(Event::default().data(delta_chunk)),
            Ok::<_, Infallible>(Event::default().data(terminal_chunk)),
            Ok::<_, Infallible>(Event::default().data("[DONE]")),
        ];
        let mut response = Sse::new(iter(events)).into_response();
        insert_execution_headers(response.headers_mut(), state.as_ref());
        return Ok(response);
    }

    let psionic_harmony = if state.include_psionic_fields {
        parsed
    } else {
        None
    };
    let full_choice = choice.into_full_choice();
    let mut response = Json(ChatCompletionResponse {
        id: request_id,
        object: "chat.completion",
        created: unix_timestamp_secs(),
        model: response_model_name,
        choices: vec![full_choice],
        usage: ChatCompletionUsage {
            prompt_tokens: response.usage.input_tokens,
            completion_tokens: response.usage.output_tokens,
            total_tokens: response.usage.input_tokens + response.usage.output_tokens,
        },
        psionic_metrics: state
            .include_psionic_fields
            .then(|| response.metrics.clone()),
        psionic_harmony,
        psionic_reasoning: surfaced_reasoning_response(
            parsed_reasoning.as_ref(),
            reasoning_request.as_ref(),
            state.include_psionic_fields,
        ),
        psionic_perf: state
            .include_psionic_fields
            .then(|| response.metrics.gpt_oss_perf.clone())
            .flatten(),
        psionic_output_text: state
            .include_psionic_fields
            .then(|| response.output.text.clone()),
        psionic_output_tokens: state.include_psionic_fields.then(|| {
            response
                .output
                .tokens
                .as_slice()
                .iter()
                .map(|token| token.as_u32())
                .collect()
        }),
        psionic_structured_output: None,
        psionic_structured_value: None,
        psionic_tool_calls: None,
        psionic_scheduler: None,
    })
    .into_response();
    insert_execution_headers(response.headers_mut(), state.as_ref());
    Ok(response)
}

async fn generic_chat_completions(
    State(state): State<Arc<OpenAiCompatState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    match handle_generic_chat_completions(state, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn handle_generic_chat_completions(
    state: Arc<OpenAiCompatState>,
    request: ChatCompletionRequest,
) -> Result<Response, OpenAiCompatHttpError> {
    let structured_output = structured_output_from_chat_request(&request)?;
    let tool_contract = tool_contract_from_chat_request(&request, structured_output.is_some())?;
    let route = resolve_generic_model_for_endpoint(
        state.as_ref(),
        request.model.as_deref(),
        RoutingEndpoint::ChatCompletions,
        {
            let mut route_request = RoutingRequest::new(RoutingEndpoint::ChatCompletions);
            if structured_output.is_some() {
                route_request = route_request.require_structured_outputs();
            }
            if tool_contract.is_some() {
                route_request = route_request.require_tool_calling();
            }
            route_request
        },
    )?;
    let loaded_model = route.loaded_model;
    let model = loaded_model.decoder().ok_or_else(|| {
        OpenAiCompatHttpError::Internal(format!(
            "loaded model `{}` is missing decoder metadata",
            loaded_model.model_key
        ))
    })?;
    let reasoning_request =
        reasoning_request_for_family(request.psionic_reasoning.as_ref(), model.family)?;
    let prompt_messages = apply_tool_contract_to_prompt_messages(
        chat_messages_to_prompt_messages_for_family(&request.messages, model.family)?,
        tool_contract.as_ref(),
    );
    let rendered = render_prompt_for_model(loaded_model, prompt_messages.as_slice())?;
    let request_id = next_generic_request_id(&state, "psionic-chatcmpl");
    let response_model_name = request
        .model
        .clone()
        .unwrap_or_else(|| loaded_model.canonical_name.clone());
    let options = generation_options_from_chat_request_for_family(
        &request,
        model.family,
        rendered.stop_sequences.as_slice(),
    );
    let mut options = options;
    options.structured_output =
        structured_output_from_tool_contract(tool_contract.as_ref())?.or(structured_output);
    let generation_request = GenerationRequest::new_text(
        request_id.clone(),
        model.descriptor.clone(),
        None,
        rendered.text,
        options,
    )
    .with_prefix_cache_control(request.psionic_prefix_cache.clone().unwrap_or_default());

    let response = worker_for_route(state.as_ref(), &route.selection)?
        .generate(route.selection.model_key.clone(), generation_request)
        .await
        .map_err(|error| {
            OpenAiCompatHttpError::from(GptOssOpenAiCompatGenerationError::Generation(error))
        })?;
    let parsed_reasoning = if reasoning_parser_for_decoder_family(model.family).is_some() {
        parse_reasoning_response_for_family(model.family, response.output.text.as_str())
            .ok()
            .flatten()
    } else {
        None
    };
    let parsed =
        if state.include_psionic_fields && matches!(model.family, GgufDecoderFamily::GptOss) {
            parse_gpt_oss_harmony_text(
                response.output.text.as_str(),
                GptOssHarmonyParseOptions {
                    role_hint: Some(PromptMessageRole::Assistant),
                    strict: false,
                },
            )
            .ok()
        } else {
            None
        };
    let tool_outcome =
        tool_call_outcome_from_response(request_id.as_str(), &response, tool_contract.as_ref())?;
    let choice = completion_choice_for_family(
        model.family,
        &response,
        parsed_reasoning.as_ref(),
        reasoning_request.as_ref(),
        tool_outcome.as_ref(),
    )?;
    let psionic_tool_calls = tool_outcome
        .as_ref()
        .map(|outcome| {
            outcome
                .tool_calls
                .clone()
                .into_iter()
                .map(ResolvedToolCall::into_psionic_tool_call)
                .collect::<Result<Vec<_>, OpenAiCompatHttpError>>()
        })
        .transpose()?
        .filter(|tool_calls| !tool_calls.is_empty());
    let structured_output_report = response
        .provenance
        .as_ref()
        .and_then(|value| value.structured_output.clone());
    let structured_output_value = response.output.structured.clone();
    let scheduler_receipt = response
        .provenance
        .as_ref()
        .and_then(|value| value.scheduler.clone());
    let prefix_cache_state = response
        .provenance
        .as_ref()
        .and_then(|value| value.prefix_cache_state);
    let prefix_cache_refusal_reason = response
        .provenance
        .as_ref()
        .and_then(|value| value.prefix_cache_refusal_reason);
    let prefix_tokens_reused = response.metrics.prefix_tokens_reused;
    let prefill_decode_mode = scheduler_receipt
        .as_ref()
        .and_then(|receipt| receipt.prefill_decode_mode)
        .or_else(|| {
            response
                .provenance
                .as_ref()
                .and_then(|value| value.delivery_proof.as_ref())
                .and_then(|proof| proof.prefill_decode_handoff.as_ref())
                .map(|handoff| handoff.mode)
        });
    let time_to_first_token_ns = response.metrics.time_to_first_token_ns;
    let inter_token_latency_ns = response.metrics.inter_token_latency_ns;
    if request.stream {
        let terminal_chunk = completion_terminal_chunk(
            request_id.as_str(),
            &response_model_name,
            response.termination,
            Some(choice.finish_reason),
            unix_timestamp_secs(),
        );
        let delta_chunk = serialize_event_data(&completion_delta_chunk(
            request_id.as_str(),
            response_model_name.as_str(),
            choice.content.clone(),
            choice.reasoning_content.clone(),
            (!choice.tool_calls.is_empty()).then_some(choice.tool_calls.clone()),
            unix_timestamp_secs(),
        ))?;
        let terminal_chunk = serialize_event_data(&terminal_chunk)?;
        let events = vec![
            Ok::<_, Infallible>(Event::default().data(delta_chunk)),
            Ok::<_, Infallible>(Event::default().data(terminal_chunk)),
            Ok::<_, Infallible>(Event::default().data("[DONE]")),
        ];
        let mut response = Sse::new(iter(events)).into_response();
        insert_generic_execution_headers(
            response.headers_mut(),
            state.as_ref(),
            &route.selection,
            structured_output_report.as_ref(),
            scheduler_receipt.as_ref(),
            prefill_decode_mode,
            time_to_first_token_ns,
            inter_token_latency_ns,
            prefix_cache_state,
            prefix_cache_refusal_reason,
            prefix_tokens_reused,
        );
        return Ok(response);
    }

    let psionic_harmony = if state.include_psionic_fields {
        parsed
    } else {
        None
    };
    let body = ChatCompletionResponse {
        id: request_id,
        object: "chat.completion",
        created: unix_timestamp_secs(),
        model: response_model_name,
        choices: vec![choice.into_full_choice()],
        usage: ChatCompletionUsage {
            prompt_tokens: response.usage.input_tokens,
            completion_tokens: response.usage.output_tokens,
            total_tokens: response.usage.input_tokens + response.usage.output_tokens,
        },
        psionic_metrics: state
            .include_psionic_fields
            .then(|| response.metrics.clone()),
        psionic_harmony,
        psionic_reasoning: surfaced_reasoning_response(
            parsed_reasoning.as_ref(),
            reasoning_request.as_ref(),
            state.include_psionic_fields,
        ),
        psionic_perf: state
            .include_psionic_fields
            .then(|| response.metrics.gpt_oss_perf.clone())
            .flatten(),
        psionic_output_text: state
            .include_psionic_fields
            .then(|| response.output.text.clone()),
        psionic_output_tokens: state.include_psionic_fields.then(|| {
            response
                .output
                .tokens
                .as_slice()
                .iter()
                .map(|token| token.as_u32())
                .collect()
        }),
        psionic_structured_output: response
            .provenance
            .as_ref()
            .and_then(|value| value.structured_output.clone()),
        psionic_structured_value: structured_output_value,
        psionic_tool_calls,
        psionic_scheduler: state
            .include_psionic_fields
            .then(|| scheduler_receipt.clone())
            .flatten(),
    };
    let mut response = Json(body).into_response();
    insert_generic_execution_headers(
        response.headers_mut(),
        state.as_ref(),
        &route.selection,
        structured_output_report.as_ref(),
        scheduler_receipt.as_ref(),
        prefill_decode_mode,
        time_to_first_token_ns,
        inter_token_latency_ns,
        prefix_cache_state,
        prefix_cache_refusal_reason,
        prefix_tokens_reused,
    );
    Ok(response)
}

async fn generic_responses(
    State(state): State<Arc<OpenAiCompatState>>,
    Json(request): Json<ResponsesRequest>,
) -> Response {
    match handle_generic_responses(state, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn handle_generic_responses(
    state: Arc<OpenAiCompatState>,
    request: ResponsesRequest,
) -> Result<Response, OpenAiCompatHttpError> {
    if request.stream {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "streaming `/v1/responses` is not implemented on the generic Psionic server yet",
        )));
    }
    let response_state_request = resolved_response_state_request(&request)?;
    if matches!(
        response_state_request.continuation,
        ResponseContinuationMode::ContinueLastAssistant
    ) {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "`psionic_response_state.continuation = continue_last_assistant` is not available on the current prompt-replay `/v1/responses` runtime",
        )));
    }
    let response_state_context = state
        .response_state
        .lock()
        .map_err(|_| {
            OpenAiCompatHttpError::Internal(String::from(
                "generic response-state store is poisoned",
            ))
        })?
        .load_context(
            request.previous_response_id.as_deref(),
            request.conversation.as_deref(),
        )
        .map_err(response_state_error_into_http)?;
    let structured_output = structured_output_from_responses_request(&request)?;
    let tool_contract =
        tool_contract_from_responses_request(&request, structured_output.is_some())?;
    let route_request = {
        let mut route_request =
            RoutingRequest::new(RoutingEndpoint::Responses).require_response_state();
        if structured_output.is_some() {
            route_request = route_request.require_structured_outputs();
        }
        if tool_contract.is_some() {
            route_request = route_request.require_tool_calling();
        }
        if let Some(worker_id) = response_state_context.worker_id.as_deref() {
            route_request = route_request.prefer_worker(worker_id.to_string());
        }
        route_request
    };
    let route = match (
        request.model.as_deref(),
        response_state_context.model_key.as_deref(),
    ) {
        (Some(requested), _) => resolve_generic_model_for_endpoint(
            state.as_ref(),
            Some(requested),
            RoutingEndpoint::Responses,
            route_request.clone(),
        )?,
        (None, Some(model_key)) => resolve_generic_model_key_for_endpoint(
            state.as_ref(),
            model_key,
            RoutingEndpoint::Responses,
            route_request.clone(),
        )?,
        (None, None) => resolve_generic_model_for_endpoint(
            state.as_ref(),
            None,
            RoutingEndpoint::Responses,
            route_request,
        )?,
    };
    let loaded_model = route.loaded_model;
    if let Some(expected_model_key) = response_state_context.model_key.as_deref()
        && loaded_model.model_key != expected_model_key
    {
        return Err(OpenAiCompatHttpError::BadRequest(format!(
            "stateful `/v1/responses` continuation must stay on model `{}`",
            loaded_model.canonical_name
        )));
    }
    let model = loaded_model.decoder().ok_or_else(|| {
        OpenAiCompatHttpError::Internal(format!(
            "loaded model `{}` is missing decoder metadata",
            loaded_model.model_key
        ))
    })?;
    if !response_state_context.prompt_history.is_empty()
        && let Some(instructions) = request.instructions.as_deref()
        && leading_response_instructions(response_state_context.prompt_history.as_slice())
            != Some(instructions)
    {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "stateful `/v1/responses` continuation cannot change `instructions`; omit it or repeat the original value exactly",
        )));
    }
    let reasoning_request =
        reasoning_request_for_family(request.psionic_reasoning.as_ref(), model.family)?;
    let appended_prompt_messages = response_input_to_prompt_messages_with_options(
        &request,
        model.family,
        response_state_context.prompt_history.is_empty(),
        false,
    )?;
    let mut prompt_history = response_state_context.prompt_history.clone();
    prompt_history.extend(appended_prompt_messages.clone());
    let prompt_messages =
        apply_tool_contract_to_prompt_messages(prompt_history.clone(), tool_contract.as_ref());
    let rendered = render_prompt_for_model(loaded_model, prompt_messages.as_slice())?;
    let request_id = next_generic_request_id(&state, "psionic-resp");
    let response_model_name = request
        .model
        .clone()
        .unwrap_or_else(|| loaded_model.canonical_name.clone());
    let mut options = generation_options_from_responses_request(
        &request,
        model.family,
        rendered.stop_sequences.as_slice(),
    );
    options.structured_output =
        structured_output_from_tool_contract(tool_contract.as_ref())?.or(structured_output);
    let generation_request = GenerationRequest::new_text(
        request_id.clone(),
        model.descriptor.clone(),
        None,
        rendered.text,
        options,
    )
    .with_prefix_cache_control(request.psionic_prefix_cache.clone().unwrap_or_default());

    let response = worker_for_route(state.as_ref(), &route.selection)?
        .generate(route.selection.model_key.clone(), generation_request)
        .await
        .map_err(|error| {
            OpenAiCompatHttpError::from(GptOssOpenAiCompatGenerationError::Generation(error))
        })?;
    let parsed_reasoning = if reasoning_parser_for_decoder_family(model.family).is_some() {
        parse_reasoning_response_for_family(model.family, response.output.text.as_str())
            .ok()
            .flatten()
    } else {
        None
    };
    let parsed =
        if state.include_psionic_fields && matches!(model.family, GgufDecoderFamily::GptOss) {
            parse_gpt_oss_harmony_text(
                response.output.text.as_str(),
                GptOssHarmonyParseOptions {
                    role_hint: Some(PromptMessageRole::Assistant),
                    strict: false,
                },
            )
            .ok()
        } else {
            None
        };
    let tool_outcome =
        tool_call_outcome_from_response(request_id.as_str(), &response, tool_contract.as_ref())?;
    let choice = completion_choice_for_family(
        model.family,
        &response,
        parsed_reasoning.as_ref(),
        reasoning_request.as_ref(),
        tool_outcome.as_ref(),
    )?;
    let content = choice.content.clone().unwrap_or_default();
    let psionic_tool_calls = tool_outcome
        .as_ref()
        .map(|outcome| {
            outcome
                .tool_calls
                .clone()
                .into_iter()
                .map(ResolvedToolCall::into_psionic_tool_call)
                .collect::<Result<Vec<_>, OpenAiCompatHttpError>>()
        })
        .transpose()?
        .filter(|tool_calls| !tool_calls.is_empty());
    let structured_output_report = response
        .provenance
        .as_ref()
        .and_then(|value| value.structured_output.clone());
    let structured_output_value = response.output.structured.clone();
    let scheduler_receipt = response
        .provenance
        .as_ref()
        .and_then(|value| value.scheduler.clone());
    let prefix_cache_state = response
        .provenance
        .as_ref()
        .and_then(|value| value.prefix_cache_state);
    let prefix_cache_refusal_reason = response
        .provenance
        .as_ref()
        .and_then(|value| value.prefix_cache_refusal_reason);
    let prefix_tokens_reused = response.metrics.prefix_tokens_reused;
    let prefill_decode_mode = scheduler_receipt
        .as_ref()
        .and_then(|receipt| receipt.prefill_decode_mode)
        .or_else(|| {
            response
                .provenance
                .as_ref()
                .and_then(|value| value.delivery_proof.as_ref())
                .and_then(|proof| proof.prefill_decode_handoff.as_ref())
                .map(|handoff| handoff.mode)
        });
    let time_to_first_token_ns = response.metrics.time_to_first_token_ns;
    let inter_token_latency_ns = response.metrics.inter_token_latency_ns;
    let assistant_history = assistant_history_from_response(
        model.family,
        response.output.text.as_str(),
        parsed.as_ref(),
    );
    let response_state_capability = current_response_state_capability(state.as_ref());
    let assigned_conversation_id = response_state_request.store.then(|| {
        if response_state_request.invalidate_references
            || response_state_context.conversation_id.is_none()
        {
            next_conversation_id(state.as_ref())
        } else {
            response_state_context
                .conversation_id
                .clone()
                .expect("checked conversation presence above")
        }
    });
    let mut stored_prompt_history = prompt_history.clone();
    stored_prompt_history.extend(assistant_history.clone());
    let (conversation, invalidated_references) = {
        let mut response_state = state.response_state.lock().map_err(|_| {
            OpenAiCompatHttpError::Internal(String::from(
                "generic response-state store is poisoned",
            ))
        })?;
        let conversation = if response_state_request.store {
            response_state
                .record_response(ResponseStateRecord {
                    response_id: request_id.clone(),
                    model_key: loaded_model.model_key.clone(),
                    worker_id: route.selection.worker_id.clone(),
                    conversation_id: assigned_conversation_id.clone(),
                    prompt_history: stored_prompt_history.clone(),
                })
                .map_err(response_state_error_into_http)?
        } else {
            None
        };
        let invalidated = if response_state_request.invalidate_references {
            let invalidated_conversation_id = response_state_context
                .conversation_id
                .as_deref()
                .filter(|candidate| Some(*candidate) != assigned_conversation_id.as_deref());
            response_state
                .invalidate_references(
                    request.previous_response_id.as_deref(),
                    invalidated_conversation_id,
                )
                .map_err(response_state_error_into_http)?
        } else {
            Vec::new()
        };
        (conversation, invalidated)
    };
    let body = ResponsesResponse {
        id: request_id.clone(),
        object: "response",
        created_at: unix_timestamp_secs(),
        status: "completed",
        model: response_model_name,
        output: responses_output_items(request_id.as_str(), &choice),
        output_text: content,
        usage: ResponsesUsage {
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            total_tokens: response.usage.input_tokens + response.usage.output_tokens,
        },
        previous_response_id: response_state_context.previous_response_id.clone(),
        conversation,
        psionic_metrics: state
            .include_psionic_fields
            .then(|| response.metrics.clone()),
        psionic_harmony: state.include_psionic_fields.then_some(parsed).flatten(),
        psionic_reasoning: surfaced_reasoning_response(
            parsed_reasoning.as_ref(),
            reasoning_request.as_ref(),
            state.include_psionic_fields,
        ),
        psionic_response_state: Some(ResponseStateReceipt {
            storage: response_state_capability.storage.clone(),
            retention_scope: response_state_capability.retention_scope.clone(),
            cache_behavior: response_state_capability.cache_behavior.clone(),
            stored: response_state_request.store,
            continuation: response_state_request.continuation,
            previous_response_id: response_state_context.previous_response_id.clone(),
            conversation_id: assigned_conversation_id.clone(),
            replayed_prompt_messages: response_state_context.replayed_prompt_messages,
            input_messages_appended: appended_prompt_messages.len(),
            assistant_messages_recorded: if response_state_request.store {
                assistant_history.len()
            } else {
                0
            },
            max_responses: response_state_capability.max_responses,
            max_conversations: response_state_capability.max_conversations,
            max_items_per_conversation: response_state_capability.max_items_per_conversation,
            conversation_item_count: if response_state_request.store {
                stored_prompt_history.len()
            } else {
                response_state_context.conversation_item_count
            },
            invalidated_references,
        }),
        psionic_perf: state
            .include_psionic_fields
            .then(|| response.metrics.gpt_oss_perf.clone())
            .flatten(),
        psionic_output_tokens: state.include_psionic_fields.then(|| {
            response
                .output
                .tokens
                .as_slice()
                .iter()
                .map(|token| token.as_u32())
                .collect()
        }),
        psionic_structured_output: response
            .provenance
            .as_ref()
            .and_then(|value| value.structured_output.clone()),
        psionic_structured_value: structured_output_value,
        psionic_tool_calls,
        psionic_scheduler: state
            .include_psionic_fields
            .then(|| scheduler_receipt.clone())
            .flatten(),
    };
    let mut response = Json(body).into_response();
    insert_generic_execution_headers(
        response.headers_mut(),
        state.as_ref(),
        &route.selection,
        structured_output_report.as_ref(),
        scheduler_receipt.as_ref(),
        prefill_decode_mode,
        time_to_first_token_ns,
        inter_token_latency_ns,
        prefix_cache_state,
        prefix_cache_refusal_reason,
        prefix_tokens_reused,
    );
    Ok(response)
}

async fn generic_embeddings(
    State(state): State<Arc<OpenAiCompatState>>,
    Json(request): Json<EmbeddingsRequest>,
) -> Response {
    match handle_generic_embeddings(state, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn handle_generic_embeddings(
    state: Arc<OpenAiCompatState>,
    request: EmbeddingsRequest,
) -> Result<Response, OpenAiCompatHttpError> {
    if let Some(encoding_format) = request.encoding_format.as_deref()
        && encoding_format != "float"
    {
        return Err(OpenAiCompatHttpError::BadRequest(format!(
            "unsupported `encoding_format` `{encoding_format}` for `/v1/embeddings`; only `float` is supported"
        )));
    }
    let loaded_model = resolve_generic_model_for_endpoint(
        state.as_ref(),
        request.model.as_deref(),
        RoutingEndpoint::Embeddings,
        RoutingRequest::new(RoutingEndpoint::Embeddings),
    )?;
    let route = loaded_model;
    let loaded_model = route.loaded_model;
    let model = loaded_model.embeddings().ok_or_else(|| {
        OpenAiCompatHttpError::Internal(format!(
            "loaded model `{}` is missing embeddings metadata",
            loaded_model.model_key
        ))
    })?;
    let request_id = next_generic_request_id(&state, "psionic-embed");
    let response_model_name = request
        .model
        .clone()
        .unwrap_or_else(|| loaded_model.canonical_name.clone());
    let embedding_request = if let Some(dimensions) = request.dimensions {
        EmbeddingRequest::new(
            request_id.clone(),
            model.descriptor.clone(),
            request.input.into_vec(),
        )
        .with_output_dimensions(dimensions)
    } else {
        EmbeddingRequest::new(
            request_id.clone(),
            model.descriptor.clone(),
            request.input.into_vec(),
        )
    };
    let response = worker_for_route(state.as_ref(), &route.selection)?
        .embed(route.selection.model_key.clone(), embedding_request)
        .await?;
    let body = EmbeddingsResponse {
        object: "list",
        data: response
            .embeddings
            .iter()
            .map(|embedding| EmbeddingsResponseData {
                object: "embedding",
                index: embedding.index,
                embedding: embedding.values.clone(),
            })
            .collect(),
        model: response_model_name,
        usage: response
            .metrics
            .prompt_eval_count
            .map(|prompt_tokens| EmbeddingsUsage {
                prompt_tokens,
                total_tokens: prompt_tokens,
            }),
        psionic_metrics: state
            .include_psionic_fields
            .then(|| response.metrics.clone()),
        psionic_provenance: state
            .include_psionic_fields
            .then(|| response.provenance.clone())
            .flatten(),
    };
    let mut response = Json(body).into_response();
    insert_generic_execution_headers(
        response.headers_mut(),
        state.as_ref(),
        &route.selection,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    Ok(response)
}

async fn proxy_chat_completions(
    state: &GptOssOpenAiCompatState,
    proxy: &LlamaCppProxyState,
    request: &ChatCompletionRequest,
) -> Result<Response, OpenAiCompatHttpError> {
    let upstream = proxy
        .client
        .post(format!("{}/v1/chat/completions", proxy.base_url))
        .json(request)
        .send()
        .await
        .map_err(|error| {
            OpenAiCompatHttpError::from(GptOssOpenAiCompatGenerationError::BackendUnavailable {
                backend: "metal-proxy",
                status: psionic_runtime::HealthStatus::Offline,
                message: format!("llama.cpp proxy request failed: {error}"),
            })
        })?;
    let status = upstream.status();
    let content_type = upstream
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .cloned();
    let body = upstream.bytes().await.map_err(|error| {
        OpenAiCompatHttpError::from(GptOssOpenAiCompatGenerationError::BackendUnavailable {
            backend: "metal-proxy",
            status: psionic_runtime::HealthStatus::Offline,
            message: format!("llama.cpp proxy response read failed: {error}"),
        })
    })?;
    let mut response = Response::builder().status(status);
    if let Some(content_type) = content_type {
        response = response.header(axum::http::header::CONTENT_TYPE, content_type);
    }
    let mut response = response
        .body(axum::body::Body::from(body))
        .map_err(|error| OpenAiCompatHttpError::BadRequest(error.to_string()))?;
    insert_execution_headers(response.headers_mut(), state);
    Ok(response)
}

fn insert_execution_headers(headers: &mut HeaderMap, state: &GptOssOpenAiCompatState) {
    headers.insert(
        HeaderName::from_static("x-psionic-backend"),
        HeaderValue::from_static(state.backend_label),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-execution-mode"),
        HeaderValue::from_static(state.execution_mode_label),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-execution-engine"),
        HeaderValue::from_static(state.execution_engine_label),
    );
}

#[derive(Clone, Debug, Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: &'static str,
    created: u64,
    model: String,
    choices: Vec<ChatCompletionChoice>,
    usage: ChatCompletionUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_metrics: Option<GenerationMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_harmony: Option<GptOssHarmonyParsedOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_reasoning: Option<ParsedReasoningResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_perf: Option<GptOssPerformanceMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_output_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_output_tokens: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_structured_output: Option<StructuredOutputExecutionReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_structured_value: Option<StructuredOutputValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_tool_calls: Option<Vec<PsionicToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_scheduler: Option<GenerationSchedulerRequestReceipt>,
}

#[derive(Clone, Debug, Serialize)]
struct ResponsesResponse {
    id: String,
    object: &'static str,
    created_at: u64,
    status: &'static str,
    model: String,
    output: Vec<ResponsesOutputItem>,
    output_text: String,
    usage: ResponsesUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation: Option<ResponseConversationRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_metrics: Option<GenerationMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_harmony: Option<GptOssHarmonyParsedOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_reasoning: Option<ParsedReasoningResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_response_state: Option<ResponseStateReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_perf: Option<GptOssPerformanceMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_output_tokens: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_structured_output: Option<StructuredOutputExecutionReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_structured_value: Option<StructuredOutputValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_tool_calls: Option<Vec<PsionicToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_scheduler: Option<GenerationSchedulerRequestReceipt>,
}

#[derive(Clone, Debug, Serialize)]
struct ResponsesOutputItem {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    status: &'static str,
    role: &'static str,
    content: Vec<ResponsesOutputContent>,
}

#[derive(Clone, Debug, Serialize)]
struct ResponsesOutputContent {
    #[serde(rename = "type")]
    kind: &'static str,
    text: String,
}

#[derive(Clone, Debug, Serialize)]
struct ResponsesUsage {
    input_tokens: usize,
    output_tokens: usize,
    total_tokens: usize,
}

#[derive(Clone, Debug, Serialize)]
struct ChatCompletionChoice {
    index: usize,
    message: ChatCompletionResponseMessage,
    finish_reason: &'static str,
}

#[derive(Clone, Debug, Serialize)]
struct ChatCompletionResponseMessage {
    role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ChatCompletionToolCall>>,
}

#[derive(Clone, Debug, Serialize)]
struct ChatCompletionToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: ChatCompletionToolCallFunction,
}

#[derive(Clone, Debug, Serialize)]
struct ChatCompletionToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Clone, Debug, Serialize)]
struct ChatCompletionUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[derive(Clone, Debug, Serialize)]
struct EmbeddingsResponse {
    object: &'static str,
    data: Vec<EmbeddingsResponseData>,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<EmbeddingsUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_metrics: Option<EmbeddingMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_provenance: Option<EmbeddingProvenance>,
}

#[derive(Clone, Debug, Serialize)]
struct EmbeddingsResponseData {
    object: &'static str,
    index: usize,
    embedding: Vec<f32>,
}

#[derive(Clone, Debug, Serialize)]
struct EmbeddingsUsage {
    prompt_tokens: usize,
    total_tokens: usize,
}

#[derive(Clone, Debug, Serialize)]
struct ChatCompletionChunk {
    id: String,
    object: &'static str,
    created: u64,
    model: String,
    choices: Vec<ChatCompletionChunkChoice>,
}

#[derive(Clone, Debug, Serialize)]
struct ChatCompletionChunkChoice {
    index: usize,
    delta: ChatCompletionChunkDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize, Default)]
struct ChatCompletionChunkDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ChatCompletionToolCall>>,
}

#[derive(Clone, Debug)]
struct ParsedCompletionChoice {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Vec<ChatCompletionToolCall>,
    finish_reason: &'static str,
}

impl ParsedCompletionChoice {
    fn into_full_choice(self) -> ChatCompletionChoice {
        ChatCompletionChoice {
            index: 0,
            message: ChatCompletionResponseMessage {
                role: "assistant",
                content: self.content,
                reasoning_content: self.reasoning_content,
                tool_calls: (!self.tool_calls.is_empty()).then_some(self.tool_calls),
            },
            finish_reason: self.finish_reason,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct PsionicToolCall {
    id: String,
    name: String,
    arguments: serde_json::Value,
    raw_arguments: String,
}

fn completion_choice(
    response: &crate::GenerationResponse,
    parsed_reasoning: Option<&ParsedReasoningResponse>,
    reasoning_request: Option<&ResolvedReasoningRequest>,
) -> ParsedCompletionChoice {
    let content = parsed_reasoning
        .and_then(|parsed| parsed.final_content.clone())
        .unwrap_or_else(|| response.output.text.clone());
    ParsedCompletionChoice {
        content: Some(content),
        reasoning_content: reasoning_request.and_then(|request| match request.mode {
            PsionicReasoningMode::Separate => {
                parsed_reasoning.and_then(|parsed| parsed.reasoning_content.clone())
            }
            PsionicReasoningMode::Suppress => None,
        }),
        tool_calls: Vec::new(),
        finish_reason: finish_reason(response.termination),
    }
}

fn completion_terminal_chunk(
    request_id: &str,
    model: &str,
    termination: TerminationReason,
    finish_reason_override: Option<&'static str>,
    created: u64,
) -> ChatCompletionChunk {
    ChatCompletionChunk {
        id: request_id.to_string(),
        object: "chat.completion.chunk",
        created,
        model: model.to_string(),
        choices: vec![ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionChunkDelta::default(),
            finish_reason: Some(finish_reason_override.unwrap_or(finish_reason(termination))),
        }],
    }
}

fn completion_delta_chunk(
    request_id: &str,
    model: &str,
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ChatCompletionToolCall>>,
    created: u64,
) -> ChatCompletionChunk {
    ChatCompletionChunk {
        id: request_id.to_string(),
        object: "chat.completion.chunk",
        created,
        model: model.to_string(),
        choices: vec![ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionChunkDelta {
                role: Some("assistant"),
                content,
                reasoning_content,
                tool_calls,
            },
            finish_reason: None,
        }],
    }
}

fn responses_output_items(
    request_id: &str,
    choice: &ParsedCompletionChoice,
) -> Vec<ResponsesOutputItem> {
    let mut content_items = Vec::new();
    if let Some(reasoning) = choice.reasoning_content.clone() {
        content_items.push(ResponsesOutputContent {
            kind: "reasoning_text",
            text: reasoning,
        });
    }
    if let Some(content) = choice.content.clone()
        && !content.is_empty()
    {
        content_items.push(ResponsesOutputContent {
            kind: "output_text",
            text: content,
        });
    }
    if content_items.is_empty() {
        return Vec::new();
    }
    vec![ResponsesOutputItem {
        id: format!("{request_id}-msg-0"),
        kind: "message",
        status: "completed",
        role: "assistant",
        content: content_items,
    }]
}

fn serialize_event_data(value: &impl Serialize) -> Result<String, OpenAiCompatHttpError> {
    serde_json::to_string(value).map_err(|error| {
        OpenAiCompatHttpError::Internal(format!("failed to serialize OpenAI stream event: {error}"))
    })
}

fn finish_reason(termination: TerminationReason) -> &'static str {
    match termination {
        TerminationReason::EndOfSequence => "stop",
        TerminationReason::MaxOutputTokens | TerminationReason::ContextLimit => "length",
        TerminationReason::Cancelled
        | TerminationReason::Disconnected
        | TerminationReason::Error => "stop",
    }
}

fn next_request_id(state: &GptOssOpenAiCompatState) -> String {
    let next = state.request_counter.fetch_add(1, Ordering::Relaxed);
    format!("psionic-chatcmpl-{next}")
}

fn next_generic_request_id(state: &OpenAiCompatState, prefix: &str) -> String {
    let next = state.request_counter.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{next}")
}

fn insert_generic_execution_headers(
    headers: &mut HeaderMap,
    state: &OpenAiCompatState,
    route_selection: &RouteSelection,
    structured_output: Option<&StructuredOutputExecutionReport>,
    scheduler: Option<&GenerationSchedulerRequestReceipt>,
    prefill_decode_mode: Option<psionic_runtime::PrefillDecodeExecutionMode>,
    time_to_first_token_ns: Option<u64>,
    inter_token_latency_ns: Option<u64>,
    prefix_cache_state: Option<PrefixCacheState>,
    prefix_cache_refusal_reason: Option<PrefixCacheRefusalReason>,
    prefix_tokens_reused: Option<usize>,
) {
    headers.insert(
        HeaderName::from_static("x-psionic-backend"),
        HeaderValue::from_static(state.backend_label),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-execution-mode"),
        HeaderValue::from_static(state.execution_mode_label),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-execution-engine"),
        HeaderValue::from_static(state.execution_engine_label),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-residency-mode"),
        HeaderValue::from_static(CPU_SERVER_RESIDENCY_MODE),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-hybrid-offload"),
        HeaderValue::from_static(CPU_SERVER_HYBRID_OFFLOAD_MODE),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-fallback-policy"),
        HeaderValue::from_static(CPU_SERVER_FALLBACK_POLICY),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-performance-class"),
        HeaderValue::from_static(CPU_SERVER_PERFORMANCE_CLASS),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-route-worker"),
        HeaderValue::from_str(route_selection.worker_id.as_str())
            .unwrap_or_else(|_| HeaderValue::from_static("invalid")),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-route-strategy"),
        HeaderValue::from_static(match route_selection.metrics.strategy {
            RouteSelectionStrategy::FirstReady => "first_ready",
            RouteSelectionStrategy::CacheAware => "cache_aware",
            RouteSelectionStrategy::WarmAware => "warm_aware",
            RouteSelectionStrategy::PowerOfTwoLeastLoaded => "power_of_two_least_loaded",
        }),
    );
    insert_usize_header(
        headers,
        "x-psionic-route-eligible-workers",
        route_selection.metrics.eligible_workers,
    );
    insert_usize_header(
        headers,
        "x-psionic-route-warm-workers",
        route_selection.metrics.warm_workers,
    );
    insert_usize_header(
        headers,
        "x-psionic-route-cache-matches",
        route_selection.metrics.cache_matches,
    );
    insert_usize_header(
        headers,
        "x-psionic-route-sampled-workers",
        route_selection.metrics.sampled_workers,
    );
    insert_usize_header(
        headers,
        "x-psionic-route-active-requests",
        route_selection.metrics.selected_active_requests,
    );
    if let Some(fallback_reason) = route_selection.metrics.fallback_reason.as_deref()
        && let Ok(value) = HeaderValue::from_str(fallback_reason)
    {
        headers.insert(HeaderName::from_static("x-psionic-route-fallback"), value);
    }
    if let Some(scheduler) = scheduler {
        headers.insert(
            HeaderName::from_static("x-psionic-batch-posture"),
            HeaderValue::from_static(match scheduler.batch_posture {
                psionic_runtime::BatchExecutionPosture::SingleRequestOnly => "single_request_only",
                psionic_runtime::BatchExecutionPosture::CallerStaticBatch => "caller_static_batch",
                psionic_runtime::BatchExecutionPosture::SchedulerStaticBatch => {
                    "scheduler_static_batch"
                }
                psionic_runtime::BatchExecutionPosture::ContinuousBatch => "continuous_batch",
            }),
        );
        headers.insert(
            HeaderName::from_static("x-psionic-scheduling-class"),
            HeaderValue::from_static(match scheduler.scheduling_class {
                psionic_runtime::GenerationSchedulingClass::Prefill => "prefill",
                psionic_runtime::GenerationSchedulingClass::Decode => "decode",
                psionic_runtime::GenerationSchedulingClass::MixedPrefillDecode => {
                    "mixed_prefill_decode"
                }
                psionic_runtime::GenerationSchedulingClass::FallbackSingleRequest => {
                    "fallback_single_request"
                }
            }),
        );
    }
    if let Some(prefill_decode_mode) = prefill_decode_mode {
        headers.insert(
            HeaderName::from_static("x-psionic-prefill-decode-mode"),
            HeaderValue::from_static(prefill_decode_mode.as_str()),
        );
    }
    if let Some(time_to_first_token_ns) = time_to_first_token_ns
        && let Ok(value) = HeaderValue::from_str(time_to_first_token_ns.to_string().as_str())
    {
        headers.insert(HeaderName::from_static("x-psionic-ttft-ns"), value);
    }
    if let Some(inter_token_latency_ns) = inter_token_latency_ns
        && let Ok(value) = HeaderValue::from_str(inter_token_latency_ns.to_string().as_str())
    {
        headers.insert(HeaderName::from_static("x-psionic-itl-ns"), value);
    }
    if let Some(prefix_cache_state) = prefix_cache_state {
        headers.insert(
            HeaderName::from_static("x-psionic-prefix-cache-state"),
            HeaderValue::from_static(match prefix_cache_state {
                PrefixCacheState::None => "none",
                PrefixCacheState::Hit => "hit",
                PrefixCacheState::Miss => "miss",
                PrefixCacheState::Bypassed => "bypassed",
                PrefixCacheState::Rebuilt => "rebuilt",
            }),
        );
    }
    if let Some(prefix_cache_refusal_reason) = prefix_cache_refusal_reason {
        headers.insert(
            HeaderName::from_static("x-psionic-prefix-cache-refusal"),
            HeaderValue::from_static(match prefix_cache_refusal_reason {
                PrefixCacheRefusalReason::RequestOptOut => "request_opt_out",
                PrefixCacheRefusalReason::ForcedInvalidation => "forced_invalidation",
                PrefixCacheRefusalReason::TenantBoundary => "tenant_boundary",
                PrefixCacheRefusalReason::SamplerBoundary => "sampler_boundary",
                PrefixCacheRefusalReason::SessionBoundState => "session_bound_state",
            }),
        );
    }
    if let Some(prefix_tokens_reused) = prefix_tokens_reused {
        if let Ok(value) = HeaderValue::from_str(prefix_tokens_reused.to_string().as_str()) {
            headers.insert(
                HeaderName::from_static("x-psionic-prefix-cache-reused-tokens"),
                value,
            );
        }
    }
    insert_structured_output_headers(headers, structured_output);
}

fn insert_usize_header(headers: &mut HeaderMap, name: &'static str, value: usize) {
    if let Ok(value) = HeaderValue::from_str(value.to_string().as_str()) {
        headers.insert(HeaderName::from_static(name), value);
    }
}

fn insert_structured_output_headers(
    headers: &mut HeaderMap,
    structured_output: Option<&StructuredOutputExecutionReport>,
) {
    let Some(structured_output) = structured_output else {
        return;
    };
    headers.insert(
        HeaderName::from_static("x-psionic-structured-output-mode"),
        HeaderValue::from_static(structured_output.mode.label()),
    );
    headers.insert(
        HeaderName::from_static("x-psionic-structured-output-parser"),
        HeaderValue::from_static(structured_output.parser.label()),
    );
}

#[derive(Clone, Debug)]
struct GenericRenderedPrompt {
    text: String,
    stop_sequences: Vec<String>,
}

struct ResolvedGenericRoute<'a> {
    selection: RouteSelection,
    loaded_model: &'a OpenAiCompatLoadedModel,
}

#[cfg(test)]
fn resolve_generic_model<'a>(
    state: &'a OpenAiCompatState,
    requested: Option<&str>,
) -> Result<&'a OpenAiCompatLoadedModel, OpenAiCompatHttpError> {
    Ok(resolve_generic_route(
        state,
        match requested {
            Some(requested) => RoutingTarget::RequestedModel(requested.to_string()),
            None => RoutingTarget::Default,
        },
        None,
    )?
    .loaded_model)
}

fn resolve_generic_route<'a>(
    state: &'a OpenAiCompatState,
    target: RoutingTarget,
    request: Option<RoutingRequest>,
) -> Result<ResolvedGenericRoute<'a>, OpenAiCompatHttpError> {
    let request = match request {
        Some(mut request) => {
            request.target = target;
            request
        }
        None => {
            let mut request = RoutingRequest::new(RoutingEndpoint::ChatCompletions);
            request.target = target;
            request
        }
    };
    let selection = state
        .router
        .resolve(&request)
        .map_err(openai_http_error_from_routing)?;
    let loaded_model = state
        .models_by_key
        .get(selection.model_key.as_str())
        .ok_or_else(|| {
            OpenAiCompatHttpError::Internal(format!(
                "loaded model `{}` selected by router is missing",
                selection.model_key
            ))
        })?;
    Ok(ResolvedGenericRoute {
        selection,
        loaded_model,
    })
}

fn resolve_generic_model_for_endpoint<'a>(
    state: &'a OpenAiCompatState,
    requested: Option<&str>,
    endpoint: RoutingEndpoint,
    request: RoutingRequest,
) -> Result<ResolvedGenericRoute<'a>, OpenAiCompatHttpError> {
    let route = resolve_generic_route(
        state,
        match requested {
            Some(requested) => RoutingTarget::RequestedModel(requested.to_string()),
            None => RoutingTarget::Default,
        },
        Some(request),
    )?;
    if route.loaded_model.supported_endpoints.contains(&endpoint) {
        Ok(route)
    } else {
        Err(OpenAiCompatHttpError::BadRequest(format!(
            "model `{}` does not support `{}`; supported endpoints: {}",
            requested.unwrap_or(route.loaded_model.canonical_name.as_str()),
            endpoint.path(),
            model_endpoint_paths(route.loaded_model).join(", ")
        )))
    }
}

fn resolve_generic_model_key_for_endpoint<'a>(
    state: &'a OpenAiCompatState,
    model_key: &str,
    endpoint: RoutingEndpoint,
    request: RoutingRequest,
) -> Result<ResolvedGenericRoute<'a>, OpenAiCompatHttpError> {
    let route = resolve_generic_route(
        state,
        RoutingTarget::ModelKey(model_key.to_string()),
        Some(request.with_model_key(model_key.to_string())),
    )?;
    if route.loaded_model.supported_endpoints.contains(&endpoint) {
        Ok(route)
    } else {
        Err(OpenAiCompatHttpError::BadRequest(format!(
            "model `{}` does not support `{}`; supported endpoints: {}",
            route.loaded_model.canonical_name,
            endpoint.path(),
            model_endpoint_paths(route.loaded_model).join(", ")
        )))
    }
}

fn openai_http_error_from_routing(error: RoutingError) -> OpenAiCompatHttpError {
    match error {
        RoutingError::UnknownRequestedModel { requested } => OpenAiCompatHttpError::BadRequest(
            format!("requested model `{requested}` is not loaded"),
        ),
        RoutingError::UnknownModelKey { model_key } => OpenAiCompatHttpError::BadRequest(format!(
            "requested model key `{model_key}` is not loaded"
        )),
        RoutingError::NoEligibleRoute { reason, .. } => OpenAiCompatHttpError::BadRequest(reason),
        RoutingError::EmptyWorkerInventory
        | RoutingError::DuplicateWorkerId { .. }
        | RoutingError::UnknownDefaultModel { .. }
        | RoutingError::InconsistentInventory { .. } => {
            OpenAiCompatHttpError::Internal(error.to_string())
        }
    }
}

fn worker_for_route<'a>(
    state: &'a OpenAiCompatState,
    selection: &RouteSelection,
) -> Result<&'a OpenAiCompatWorker, OpenAiCompatHttpError> {
    state
        .workers
        .get(selection.worker_id.as_str())
        .ok_or_else(|| {
            OpenAiCompatHttpError::Internal(format!(
                "worker `{}` selected by router is missing",
                selection.worker_id
            ))
        })
}

fn model_endpoint_paths(model: &OpenAiCompatLoadedModel) -> Vec<&'static str> {
    model
        .supported_endpoints
        .iter()
        .map(|endpoint| endpoint.path())
        .collect()
}

fn union_supported_endpoint_paths(state: &OpenAiCompatState) -> Vec<&'static str> {
    let mut endpoints = BTreeSet::new();
    for model in state.models_by_key.values() {
        for endpoint in &model.supported_endpoints {
            endpoints.insert(endpoint.path());
        }
    }
    endpoints.into_iter().collect()
}

fn routed_inventory_for_loaded_model(
    model: &OpenAiCompatLoadedModel,
    accepted_names: Vec<String>,
) -> RoutedModelInventory {
    let mut inventory = RoutedModelInventory::new(
        model.model_key.clone(),
        model.canonical_name.clone(),
        model.family_label().to_string(),
        model.execution_profile().clone(),
    );
    for alias in accepted_names {
        inventory = inventory.with_alias(alias);
    }
    for endpoint in &model.supported_endpoints {
        inventory = inventory.with_supported_endpoint(*endpoint);
    }
    if let Some(policy) = model.scheduler_policy() {
        inventory = inventory.with_scheduler_policy(policy.clone());
    }
    inventory = inventory.with_warm_state(RoutedWarmState::Warm);
    if model.decoder().is_some() {
        inventory = inventory
            .with_structured_outputs()
            .with_tool_calling()
            .with_response_state();
    }
    inventory
}

fn prompt_options_for_family(
    family: GgufDecoderFamily,
    reasoning_budget: u8,
) -> PromptRenderOptions {
    if matches!(family, GgufDecoderFamily::GptOss) {
        PromptRenderOptions {
            gpt_oss_harmony: Some(GptOssHarmonyRenderContext {
                reasoning_effort: Some(reasoning_effort(reasoning_budget)),
                channel_config: Some(PromptChannelConfig::default()),
                ..Default::default()
            }),
        }
    } else {
        PromptRenderOptions::default()
    }
}

fn load_generic_decoder_model(
    model_path: &Path,
    reasoning_budget: u8,
) -> Result<
    (
        OpenAiCompatLoadedModel,
        BTreeSet<String>,
        OpenAiCompatModelLoadPlan,
    ),
    String,
> {
    let artifact = GgufBlobArtifact::open_path(model_path, gpt_oss_local_blob_open_options())
        .map_err(|error| error.to_string())?;
    let adapter = GgufDecoderAdapterLoader
        .load_blob_artifact(&artifact)
        .map_err(|error| error.to_string())?;
    let descriptor = adapter.descriptor().clone();
    let family = adapter.family_metadata().family;
    let loaded_model = OpenAiCompatLoadedModel {
        model_key: descriptor.model.model_id.clone(),
        canonical_name: default_model_name(model_path, descriptor.model.model_id.as_str()),
        supported_endpoints: vec![RoutingEndpoint::ChatCompletions, RoutingEndpoint::Responses],
        kind: OpenAiCompatLoadedModelKind::Decoder(OpenAiCompatLoadedDecoderModel {
            descriptor: descriptor.clone(),
            family,
            prompt_renderer: (!matches!(family, GgufDecoderFamily::GptOss))
                .then(|| adapter.prompt_renderer()),
            prompt_options: prompt_options_for_family(family, reasoning_budget),
            execution_profile: continuous_batch_text_generation_execution_profile(),
            scheduler_policy: default_generation_scheduler_policy(),
        }),
    };
    Ok((
        loaded_model,
        accepted_model_names(model_path, descriptor.model.model_id.as_str()),
        OpenAiCompatModelLoadPlan {
            path: model_path.to_path_buf(),
            runtime_kind: OpenAiCompatRuntimeKind::GgufDecoder,
        },
    ))
}

fn load_generic_embeddings_model(
    model_path: &Path,
) -> Result<
    (
        OpenAiCompatLoadedModel,
        BTreeSet<String>,
        OpenAiCompatModelLoadPlan,
    ),
    String,
> {
    let service = CpuModelEmbeddingsService::from_safetensors_artifact(model_path)
        .map_err(|error| error.to_string())?;
    let descriptor = service.model_descriptor().clone();
    let loaded_model = OpenAiCompatLoadedModel {
        model_key: descriptor.model.model_id.clone(),
        canonical_name: default_model_name(model_path, descriptor.model.model_id.as_str()),
        supported_endpoints: vec![RoutingEndpoint::Embeddings],
        kind: OpenAiCompatLoadedModelKind::Embeddings(OpenAiCompatLoadedEmbeddingsModel {
            descriptor: descriptor.clone(),
            execution_profile: default_embeddings_execution_profile(),
        }),
    };
    Ok((
        loaded_model,
        accepted_model_names(model_path, descriptor.model.model_id.as_str()),
        OpenAiCompatModelLoadPlan {
            path: model_path.to_path_buf(),
            runtime_kind: OpenAiCompatRuntimeKind::SafetensorsEmbeddings,
        },
    ))
}

fn render_prompt_for_model(
    model: &OpenAiCompatLoadedModel,
    messages: &[PromptMessage],
) -> Result<GenericRenderedPrompt, OpenAiCompatHttpError> {
    let decoder = model.decoder().ok_or_else(|| {
        OpenAiCompatHttpError::BadRequest(format!(
            "model `{}` does not support text-generation prompts",
            model.canonical_name
        ))
    })?;
    if matches!(decoder.family, GgufDecoderFamily::GptOss) {
        let text = render_gpt_oss_harmony_prompt(messages, true, Some(&decoder.prompt_options))
            .map_err(|error| {
                OpenAiCompatHttpError::from(PromptRenderError::HarmonyRendering {
                    message: error.to_string(),
                })
            })?;
        return Ok(GenericRenderedPrompt {
            text,
            stop_sequences: vec![
                String::from(HARMONY_RETURN_STOP),
                String::from(HARMONY_CALL_STOP),
            ],
        });
    }
    let renderer = decoder.prompt_renderer.as_ref().ok_or_else(|| {
        OpenAiCompatHttpError::Internal(format!(
            "model `{}` is missing a generic prompt renderer",
            model.model_key
        ))
    })?;
    let rendered = match renderer.render_with_options(None, messages, true, &decoder.prompt_options)
    {
        Ok(rendered) => rendered,
        Err(PromptRenderError::MissingDefaultTemplate)
            if messages
                .iter()
                .all(|message| message.role != PromptMessageRole::Tool) =>
        {
            return Ok(GenericRenderedPrompt {
                text: fallback_prompt_text(messages),
                stop_sequences: Vec::new(),
            });
        }
        Err(error) => return Err(error.into()),
    };
    Ok(GenericRenderedPrompt {
        text: rendered.text,
        stop_sequences: rendered.stop_sequences,
    })
}

fn fallback_prompt_text(messages: &[PromptMessage]) -> String {
    if messages.len() == 1 && messages[0].role == PromptMessageRole::User {
        return messages[0].content.clone();
    }
    messages
        .iter()
        .map(|message| {
            let role = match message.role {
                PromptMessageRole::System => "system",
                PromptMessageRole::Developer => "developer",
                PromptMessageRole::User => "user",
                PromptMessageRole::Assistant => "assistant",
                PromptMessageRole::Tool => "tool",
            };
            format!("{role}:\n{}", message.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn completion_choice_for_family(
    family: GgufDecoderFamily,
    response: &crate::GenerationResponse,
    parsed_reasoning: Option<&ParsedReasoningResponse>,
    reasoning_request: Option<&ResolvedReasoningRequest>,
    tool_outcome: Option<&ToolCallOutcome>,
) -> Result<ParsedCompletionChoice, OpenAiCompatHttpError> {
    if let Some(tool_outcome) = tool_outcome {
        let tool_calls = tool_outcome
            .tool_calls
            .clone()
            .into_iter()
            .map(ResolvedToolCall::into_chat_tool_call)
            .collect::<Result<Vec<_>, OpenAiCompatHttpError>>()?;
        return Ok(ParsedCompletionChoice {
            content: tool_outcome.content.clone(),
            reasoning_content: reasoning_request.and_then(|request| match request.mode {
                PsionicReasoningMode::Separate => {
                    parsed_reasoning.and_then(|parsed| parsed.reasoning_content.clone())
                }
                PsionicReasoningMode::Suppress => None,
            }),
            finish_reason: if tool_calls.is_empty() {
                finish_reason(response.termination)
            } else {
                "tool_calls"
            },
            tool_calls,
        });
    }
    if matches!(family, GgufDecoderFamily::GptOss) {
        return Ok(completion_choice(
            response,
            parsed_reasoning,
            reasoning_request,
        ));
    }
    Ok(ParsedCompletionChoice {
        content: Some(response.output.text.clone()),
        reasoning_content: None,
        tool_calls: Vec::new(),
        finish_reason: finish_reason(response.termination),
    })
}

fn prompt_request_cache_key(messages: &[PromptMessage]) -> String {
    let mut hasher = Sha256::new();
    for message in messages {
        hasher.update(prompt_message_role_cache_key(message.role).as_bytes());
        hasher.update([0xff]);
        hasher.update(message.content.as_bytes());
        hasher.update([0xff]);
        if let Some(name) = message.author_name.as_deref() {
            hasher.update(name.as_bytes());
        }
        hasher.update([0x00]);
    }
    format!("{:x}", hasher.finalize())
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn generation_options_from_chat_request(request: &ChatCompletionRequest) -> GenerationOptions {
    generation_options_from_chat_request_for_family(request, GgufDecoderFamily::GptOss, &[])
}

fn generation_options_from_chat_request_for_family(
    request: &ChatCompletionRequest,
    family: GgufDecoderFamily,
    default_stop_sequences: &[String],
) -> GenerationOptions {
    let max_output_tokens = request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    let temperature = request.temperature.unwrap_or(0.0);
    let mut options = if temperature <= f32::EPSILON {
        GenerationOptions::greedy(max_output_tokens)
    } else {
        let mut options = GenerationOptions::sample(max_output_tokens);
        options.temperature = Some(temperature);
        options
    };
    options.decode_strategy = if temperature <= f32::EPSILON {
        DecodeStrategy::Greedy
    } else {
        DecodeStrategy::Sample
    };
    if let Some(stop) = &request.stop {
        options.stop_sequences.extend(stop.clone().into_vec());
    }
    for stop in default_stop_sequences {
        if !options.stop_sequences.iter().any(|value| value == stop) {
            options.stop_sequences.push(stop.clone());
        }
    }
    if matches!(family, GgufDecoderFamily::GptOss) {
        ensure_harmony_stop_sequences(&mut options.stop_sequences);
    }
    options
}

fn generation_options_from_responses_request(
    request: &ResponsesRequest,
    family: GgufDecoderFamily,
    default_stop_sequences: &[String],
) -> GenerationOptions {
    let max_output_tokens = request.max_output_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    let temperature = request.temperature.unwrap_or(0.0);
    let mut options = if temperature <= f32::EPSILON {
        GenerationOptions::greedy(max_output_tokens)
    } else {
        let mut options = GenerationOptions::sample(max_output_tokens);
        options.temperature = Some(temperature);
        options
    };
    options.decode_strategy = if temperature <= f32::EPSILON {
        DecodeStrategy::Greedy
    } else {
        DecodeStrategy::Sample
    };
    if let Some(stop) = &request.stop {
        options.stop_sequences.extend(stop.clone().into_vec());
    }
    for stop in default_stop_sequences {
        if !options.stop_sequences.iter().any(|value| value == stop) {
            options.stop_sequences.push(stop.clone());
        }
    }
    if matches!(family, GgufDecoderFamily::GptOss) {
        ensure_harmony_stop_sequences(&mut options.stop_sequences);
    }
    options
}

fn response_input_to_prompt_messages_with_options(
    request: &ResponsesRequest,
    family: GgufDecoderFamily,
    include_instructions: bool,
    allow_empty_input: bool,
) -> Result<Vec<PromptMessage>, OpenAiCompatHttpError> {
    let mut messages = Vec::new();
    if include_instructions && let Some(instructions) = request.instructions.as_ref() {
        messages.push(ChatCompletionMessage {
            role: String::from("developer"),
            content: instructions.clone(),
            name: None,
        });
    }
    match &request.input {
        ResponsesInput::Text(text) => {
            if allow_empty_input && text.is_empty() {
            } else {
                messages.push(ChatCompletionMessage {
                    role: String::from("user"),
                    content: text.clone(),
                    name: None,
                });
            }
        }
        ResponsesInput::Messages(input_messages) => {
            if allow_empty_input && input_messages.is_empty() {
            } else {
                messages.extend(input_messages.clone());
            }
        }
    }
    chat_messages_to_prompt_messages_for_family(messages.as_slice(), family)
}

fn assistant_history_from_response(
    family: GgufDecoderFamily,
    raw_output: &str,
    parsed_harmony: Option<&GptOssHarmonyParsedOutput>,
) -> Vec<PromptMessage> {
    if matches!(family, GgufDecoderFamily::GptOss)
        && let Some(parsed_harmony) = parsed_harmony
        && !parsed_harmony.messages.is_empty()
    {
        return parsed_harmony.messages.clone();
    }
    vec![PromptMessage::new(PromptMessageRole::Assistant, raw_output)]
}

fn leading_response_instructions(prompt_history: &[PromptMessage]) -> Option<&str> {
    prompt_history
        .first()
        .filter(|message| {
            message.role == PromptMessageRole::Developer
                && message.author_name.is_none()
                && message.recipient.is_none()
                && message.channel.is_none()
                && message.content_type.is_none()
        })
        .map(|message| message.content.as_str())
}

fn ensure_harmony_stop_sequences(stop_sequences: &mut Vec<String>) {
    for stop in [HARMONY_RETURN_STOP, HARMONY_CALL_STOP] {
        if !stop_sequences.iter().any(|value| value == stop) {
            stop_sequences.push(String::from(stop));
        }
    }
}

fn chat_messages_to_prompt_messages(
    messages: &[ChatCompletionMessage],
) -> Result<Vec<PromptMessage>, OpenAiCompatHttpError> {
    chat_messages_to_prompt_messages_for_family(messages, GgufDecoderFamily::GptOss)
}

fn chat_messages_to_prompt_messages_for_family(
    messages: &[ChatCompletionMessage],
    family: GgufDecoderFamily,
) -> Result<Vec<PromptMessage>, OpenAiCompatHttpError> {
    if matches!(family, GgufDecoderFamily::GptOss) {
        return chat_messages_to_prompt_messages_gpt_oss(messages);
    }
    chat_messages_to_prompt_messages_generic(messages)
}

fn chat_messages_to_prompt_messages_gpt_oss(
    messages: &[ChatCompletionMessage],
) -> Result<Vec<PromptMessage>, OpenAiCompatHttpError> {
    if messages.is_empty() {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "chat completions require at least one message",
        )));
    }
    let mut prompt_messages = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let role = match message.role.as_str() {
            "system" => PromptMessageRole::System,
            "developer" => PromptMessageRole::Developer,
            "user" => PromptMessageRole::User,
            "assistant" => PromptMessageRole::Assistant,
            "tool" => PromptMessageRole::Tool,
            other => {
                return Err(OpenAiCompatHttpError::BadRequest(format!(
                    "unsupported chat message role `{other}`"
                )));
            }
        };
        // Mirror the GPT-OSS llama.cpp OpenAI template so native and proxy
        // backends tokenize the same public request contract.
        let normalized_role = match (index, role) {
            (0, PromptMessageRole::System | PromptMessageRole::Developer) => {
                PromptMessageRole::Developer
            }
            (_, PromptMessageRole::System | PromptMessageRole::Developer) => continue,
            _ => role,
        };
        let mut prompt = PromptMessage::new(normalized_role, message.content.clone());
        if normalized_role == PromptMessageRole::Tool {
            let Some(name) = message.name.as_ref() else {
                return Err(OpenAiCompatHttpError::BadRequest(String::from(
                    "tool messages require a `name` field",
                )));
            };
            prompt = prompt.with_author_name(name.clone());
        }
        prompt_messages.push(prompt);
    }
    Ok(prompt_messages)
}

fn chat_messages_to_prompt_messages_generic(
    messages: &[ChatCompletionMessage],
) -> Result<Vec<PromptMessage>, OpenAiCompatHttpError> {
    if messages.is_empty() {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "chat completions require at least one message",
        )));
    }
    let mut prompt_messages = Vec::new();
    for message in messages {
        let role = match message.role.as_str() {
            "system" => PromptMessageRole::System,
            "developer" => PromptMessageRole::Developer,
            "user" => PromptMessageRole::User,
            "assistant" => PromptMessageRole::Assistant,
            "tool" => PromptMessageRole::Tool,
            other => {
                return Err(OpenAiCompatHttpError::BadRequest(format!(
                    "unsupported chat message role `{other}`"
                )));
            }
        };
        let mut prompt = PromptMessage::new(role, message.content.clone());
        if role == PromptMessageRole::Tool {
            let Some(name) = message.name.as_ref() else {
                return Err(OpenAiCompatHttpError::BadRequest(String::from(
                    "tool messages require a `name` field",
                )));
            };
            prompt = prompt.with_author_name(name.clone());
        }
        prompt_messages.push(prompt);
    }
    Ok(prompt_messages)
}

fn prompt_message_role_cache_key(role: PromptMessageRole) -> &'static str {
    match role {
        PromptMessageRole::System => "system",
        PromptMessageRole::Developer => "developer",
        PromptMessageRole::User => "user",
        PromptMessageRole::Assistant => "assistant",
        PromptMessageRole::Tool => "tool",
    }
}

fn validate_requested_model(
    requested: Option<&str>,
    accepted_model_names: &BTreeSet<String>,
) -> Result<(), OpenAiCompatHttpError> {
    let Some(requested) = requested else {
        return Ok(());
    };
    if accepted_model_names.contains(requested) {
        return Ok(());
    }
    Err(OpenAiCompatHttpError::BadRequest(format!(
        "requested model `{requested}` is not loaded"
    )))
}

fn default_model_name(path: &Path, model_id: &str) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(String::from)
        .unwrap_or_else(|| model_id.to_string())
}

fn accepted_model_names(path: &Path, model_id: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    names.insert(model_id.to_string());
    if let Some(file_name) = path.file_name().and_then(|value| value.to_str()) {
        names.insert(file_name.to_string());
    }
    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
        names.insert(stem.to_string());
    }
    names
}

fn reasoning_effort(reasoning_budget: u8) -> PromptReasoningEffort {
    match reasoning_budget {
        0 => PromptReasoningEffort::Low,
        1 => PromptReasoningEffort::Medium,
        _ => PromptReasoningEffort::High,
    }
}

#[derive(Clone, Debug, Serialize)]
struct OpenAiErrorEnvelope {
    error: OpenAiErrorBody,
}

#[derive(Clone, Debug, Serialize)]
struct OpenAiErrorBody {
    message: String,
    #[serde(rename = "type")]
    kind: String,
}

#[cfg(test)]
mod tests {
    use super::{
        CPU_SERVER_FALLBACK_POLICY, CPU_SERVER_HYBRID_OFFLOAD_MODE, CPU_SERVER_RESIDENCY_MODE,
        ChatCompletionJsonSchemaRequest, ChatCompletionMessage, ChatCompletionRequest,
        ChatCompletionResponseFormatRequest, EmbeddingsInput, EmbeddingsRequest,
        GptOssMetalExecutionMode, GptOssOpenAiCompatBackend, HARMONY_CALL_STOP,
        HARMONY_RETURN_STOP, NamedToolChoiceFunction, NamedToolChoiceRequest, OpenAiCompatConfig,
        OpenAiCompatServer, PromptTokenCache, PsionicGrammarRequest, PsionicReasoningMode,
        PsionicReasoningRequest, PsionicResponseStateRequest, ResolvedReasoningRequest,
        ResponseContinuationMode, ResponsesInput, ResponsesRequest, ToolChoiceRequest,
        ToolDefinitionEnvelope, ToolDefinitionRequest, chat_messages_to_prompt_messages,
        chat_messages_to_prompt_messages_for_family, completion_choice,
        ensure_harmony_stop_sequences, generation_options_from_chat_request,
        generation_options_from_chat_request_for_family, generic_embeddings, generic_health,
        generic_list_models, handle_generic_chat_completions, handle_generic_responses,
        prompt_request_cache_key, render_prompt_for_model, resolve_execution_summary,
        resolve_generic_model, responses_output_items, surfaced_reasoning_response,
    };
    use crate::{
        GenerationMetrics, GenerationOutput, GenerationRequest, GenerationResponse,
        GenerationUsage, TerminationReason,
    };
    use axum::{
        Json,
        body::to_bytes,
        extract::State,
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Response},
    };
    use psionic_models::{
        ByteProjectionEmbedder, GgufDecoderFamily, GgufMetadataValue, GgufTensorType,
        GptOssHarmonyParseOptions, GptOssHarmonyRenderContext, PromptChannelConfig, PromptMessage,
        PromptMessageRole, PromptReasoningEffort, PromptRenderOptions, ReasoningParser, TokenId,
        TokenSequence, parse_gpt_oss_harmony_text, render_gpt_oss_harmony_prompt,
    };
    use psionic_router::{ResponseStateRetentionPolicy, ResponseStateStore};
    use psionic_runtime::{
        BatchExecutionPosture, PrefixCacheControl, PrefixCacheMode, QueueDiscipline,
        StructuredGrammarSyntax, StructuredOutputRequest, StructuredTaggedVariant,
    };

    #[test]
    fn chat_messages_map_to_prompt_messages() {
        let prompt = chat_messages_to_prompt_messages(&[
            ChatCompletionMessage {
                role: String::from("system"),
                content: String::from("sys"),
                name: None,
            },
            ChatCompletionMessage {
                role: String::from("tool"),
                content: String::from("{\"ok\":true}"),
                name: Some(String::from("functions.lookup_weather")),
            },
        ])
        .expect("prompt messages");

        assert_eq!(prompt[0].role, PromptMessageRole::Developer);
        assert_eq!(prompt[1].role, PromptMessageRole::Tool);
        assert_eq!(
            prompt[1].author_name.as_deref(),
            Some("functions.lookup_weather")
        );
    }

    #[test]
    fn chat_messages_ignore_non_initial_instruction_turns_for_gpt_oss_parity() {
        let prompt = chat_messages_to_prompt_messages(&[
            ChatCompletionMessage {
                role: String::from("system"),
                content: String::from("first instruction"),
                name: None,
            },
            ChatCompletionMessage {
                role: String::from("developer"),
                content: String::from("ignored instruction"),
                name: None,
            },
            ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            },
        ])
        .expect("prompt messages");

        assert_eq!(prompt.len(), 2);
        assert_eq!(prompt[0].role, PromptMessageRole::Developer);
        assert_eq!(prompt[0].content, "first instruction");
        assert_eq!(prompt[1].role, PromptMessageRole::User);
        assert_eq!(prompt[1].content, "hello");
    }

    #[test]
    fn rendered_prompt_matches_llama_cpp_gpt_oss_openai_contract() {
        let prompt_messages = chat_messages_to_prompt_messages(&[
            ChatCompletionMessage {
                role: String::from("system"),
                content: String::from(
                    "You are ChatGPT, a large language model trained by OpenAI.\nKnowledge cutoff: 2024-06\nCurrent date: 2026-03-09\n\nReasoning: low\n\n# Valid channels: analysis, final. Channel must be included for every message.",
                ),
                name: None,
            },
            ChatCompletionMessage {
                role: String::from("developer"),
                content: String::from("Be concise. Output exactly one sentence."),
                name: None,
            },
            ChatCompletionMessage {
                role: String::from("user"),
                content: String::from(
                    "Reply with exactly this sentence and nothing else: HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.",
                ),
                name: None,
            },
        ])
        .expect("prompt messages");
        let prompt_options = PromptRenderOptions {
            gpt_oss_harmony: Some(GptOssHarmonyRenderContext {
                reasoning_effort: Some(PromptReasoningEffort::Low),
                conversation_start_date: Some(String::from("2026-03-09")),
                knowledge_cutoff: Some(String::from("2024-06")),
                channel_config: Some(PromptChannelConfig::default()),
                ..Default::default()
            }),
        };

        let rendered =
            render_gpt_oss_harmony_prompt(prompt_messages.as_slice(), true, Some(&prompt_options))
                .expect("rendered prompt");

        assert_eq!(
            rendered,
            concat!(
                "<|start|>system<|message|>",
                "You are ChatGPT, a large language model trained by OpenAI.\n",
                "Knowledge cutoff: 2024-06\n",
                "Current date: 2026-03-09\n\n",
                "Reasoning: low\n\n",
                "# Valid channels: analysis, commentary, final. Channel must be included for every message.",
                "<|end|>",
                "<|start|>developer<|message|>",
                "# Instructions\n\n",
                "You are ChatGPT, a large language model trained by OpenAI.\n",
                "Knowledge cutoff: 2024-06\n",
                "Current date: 2026-03-09\n\n",
                "Reasoning: low\n\n",
                "# Valid channels: analysis, final. Channel must be included for every message.",
                "<|end|>",
                "<|start|>user<|message|>",
                "Reply with exactly this sentence and nothing else: HTTPS protects users by encrypting traffic, preventing tampering, and confirming they are connected to the right website.",
                "<|end|>",
                "<|start|>assistant",
            )
        );
    }

    #[test]
    fn generation_options_force_harmony_stop_sequences() {
        let options = generation_options_from_chat_request(&ChatCompletionRequest {
            model: None,
            messages: vec![ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hi"),
                name: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(64),
            stop: None,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
            response_format: None,
            psionic_grammar: None,
            psionic_structured_output: None,
            psionic_reasoning: None,
            psionic_prefix_cache: None,
        });

        assert!(
            options
                .stop_sequences
                .iter()
                .any(|value| value == HARMONY_RETURN_STOP)
        );
        assert!(
            options
                .stop_sequences
                .iter()
                .any(|value| value == HARMONY_CALL_STOP)
        );
    }

    #[test]
    fn auto_metal_mode_resolves_to_native_without_legacy_proxy() {
        let summary = resolve_execution_summary(
            GptOssOpenAiCompatBackend::Metal,
            GptOssMetalExecutionMode::Auto,
            false,
        )
        .expect("metal summary");

        assert_eq!(summary.backend_label, "metal");
        assert_eq!(summary.execution_mode_label, "native");
        assert_eq!(summary.execution_engine_label, "psionic");
    }

    #[test]
    fn explicit_native_metal_mode_rejects_legacy_proxy_env() {
        let error = resolve_execution_summary(
            GptOssOpenAiCompatBackend::Metal,
            GptOssMetalExecutionMode::Native,
            true,
        )
        .expect_err("native metal should reject legacy proxy env");

        assert!(error.to_string().contains("PSIONIC_METAL_PROXY_LLAMA_CPP"));
    }

    #[test]
    fn explicit_metal_mode_is_rejected_when_backend_is_not_metal() {
        let error = resolve_execution_summary(
            GptOssOpenAiCompatBackend::Cuda,
            GptOssMetalExecutionMode::ProxyLlamaCpp,
            false,
        )
        .expect_err("non-metal backend should reject explicit metal mode");

        assert!(error.to_string().contains("resolved backend is cuda"));
    }

    #[test]
    fn gpt_oss_completion_choice_can_surface_reasoning_contracts()
    -> Result<(), Box<dyn std::error::Error>> {
        let raw = "<|channel|>analysis<|message|>thinking<|end|><|start|>assistant<|channel|>final<|message|>323";
        let parsed = parse_gpt_oss_harmony_text(
            raw,
            GptOssHarmonyParseOptions {
                role_hint: Some(PromptMessageRole::Assistant),
                strict: false,
            },
        )?
        .reasoning_response();
        let response = test_generation_response(raw);
        let reasoning_request = ResolvedReasoningRequest {
            parser: ReasoningParser::GptOssHarmony,
            mode: PsionicReasoningMode::Separate,
        };

        let choice = completion_choice(&response, Some(&parsed), Some(&reasoning_request));
        let serialized_choice = serde_json::to_value(choice.clone().into_full_choice())?;

        assert_eq!(choice.content.as_deref(), Some("323"));
        assert_eq!(choice.reasoning_content.as_deref(), Some("thinking"));
        assert_eq!(
            serialized_choice["message"]["reasoning_content"],
            serde_json::json!("thinking")
        );
        let surfaced = surfaced_reasoning_response(Some(&parsed), Some(&reasoning_request), false)
            .expect("typed reasoning should surface");
        assert_eq!(surfaced.final_content.as_deref(), Some("323"));
        assert_eq!(surfaced.reasoning_content.as_deref(), Some("thinking"));
        Ok(())
    }

    #[test]
    fn responses_output_items_keep_reasoning_and_final_text_in_order() {
        let items = responses_output_items(
            "resp-1",
            &super::ParsedCompletionChoice {
                content: Some(String::from("323")),
                reasoning_content: Some(String::from("thinking")),
                tool_calls: Vec::new(),
                finish_reason: "stop",
            },
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content.len(), 2);
        assert_eq!(items[0].content[0].kind, "reasoning_text");
        assert_eq!(items[0].content[0].text, "thinking");
        assert_eq!(items[0].content[1].kind, "output_text");
        assert_eq!(items[0].content[1].text, "323");
    }

    #[test]
    fn ensure_harmony_stop_sequences_is_idempotent() {
        let mut stops = vec![String::from(HARMONY_RETURN_STOP)];
        ensure_harmony_stop_sequences(&mut stops);
        ensure_harmony_stop_sequences(&mut stops);

        assert_eq!(
            stops
                .iter()
                .filter(|value| value.as_str() == HARMONY_RETURN_STOP)
                .count(),
            1
        );
        assert_eq!(
            stops
                .iter()
                .filter(|value| value.as_str() == HARMONY_CALL_STOP)
                .count(),
            1
        );
    }

    #[test]
    fn prompt_token_cache_is_lru() {
        let mut cache = PromptTokenCache::new(2);
        cache.record(
            String::from("key-one"),
            TokenSequence::new(vec![TokenId(1), TokenId(2)]),
        );
        cache.record(
            String::from("key-two"),
            TokenSequence::new(vec![TokenId(3)]),
        );

        assert_eq!(
            cache.lookup("key-one").expect("cached prompt").as_slice(),
            &[TokenId(1), TokenId(2)]
        );

        cache.record(
            String::from("key-three"),
            TokenSequence::new(vec![TokenId(4)]),
        );

        assert!(cache.lookup("key-two").is_none());
        assert_eq!(
            cache.lookup("key-three").expect("cached prompt").as_slice(),
            &[TokenId(4)]
        );
    }

    #[test]
    fn prompt_request_cache_key_is_stable_for_identical_messages() {
        let messages = vec![PromptMessage::new(PromptMessageRole::User, "hello")];

        assert_eq!(
            prompt_request_cache_key(messages.as_slice()),
            prompt_request_cache_key(messages.as_slice())
        );
    }

    #[test]
    fn prompt_request_cache_key_uses_normalized_prompt_messages() {
        let first = chat_messages_to_prompt_messages(&[
            ChatCompletionMessage {
                role: String::from("system"),
                content: String::from("first instruction"),
                name: None,
            },
            ChatCompletionMessage {
                role: String::from("developer"),
                content: String::from("ignored instruction"),
                name: None,
            },
            ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            },
        ])
        .expect("first normalized prompt");
        let second = chat_messages_to_prompt_messages(&[
            ChatCompletionMessage {
                role: String::from("system"),
                content: String::from("first instruction"),
                name: None,
            },
            ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            },
        ])
        .expect("second normalized prompt");

        assert_eq!(
            prompt_request_cache_key(first.as_slice()),
            prompt_request_cache_key(second.as_slice())
        );
    }

    #[test]
    fn generic_server_routes_multiple_dense_model_families()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let llama_path = temp.path().join("tiny-llama.gguf");
        let qwen_path = temp.path().join("tiny-qwen.gguf");
        write_test_gguf(
            &llama_path,
            dense_llama_metadata("tiny server llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;
        write_test_gguf(
            &qwen_path,
            dense_qwen_metadata("tiny server qwen").as_slice(),
            dense_decoder_tensors(true, 2, 3).as_slice(),
        )?;

        let mut config = OpenAiCompatConfig::new(&llama_path);
        config.add_model_path(&qwen_path);
        let server = OpenAiCompatServer::from_config(&config)?;

        let llama_model = resolve_generic_model(server.state.as_ref(), Some("tiny-llama"))
            .expect("llama model should resolve");
        let qwen_model = resolve_generic_model(server.state.as_ref(), Some("tiny-qwen"))
            .expect("qwen model should resolve");
        let llama_decoder = llama_model.decoder().expect("llama decoder model");
        let qwen_decoder = qwen_model.decoder().expect("qwen decoder model");

        assert_eq!(llama_decoder.family, GgufDecoderFamily::Llama);
        assert_eq!(qwen_decoder.family, GgufDecoderFamily::Qwen);
        assert_eq!(server.state.models_by_key.len(), 2);
        let health = tokio::runtime::Runtime::new()?
            .block_on(generic_health(State(std::sync::Arc::clone(&server.state))));
        assert_eq!(health.0.residency_mode, CPU_SERVER_RESIDENCY_MODE);
        assert_eq!(health.0.fallback_policy, CPU_SERVER_FALLBACK_POLICY);
        assert_eq!(health.0.hybrid_offload, CPU_SERVER_HYBRID_OFFLOAD_MODE);
        assert_eq!(
            health.0.structured_output_fallbacks,
            Some(vec![
                "choice_set",
                "regex_subset",
                "gbnf_subset",
                "json_schema_subset",
                "json_object",
                "tagged_json_schema",
            ])
        );
        assert_eq!(
            health
                .0
                .structured_output_capabilities
                .as_ref()
                .map(|capabilities| {
                    capabilities
                        .iter()
                        .map(|capability| capability.kind.label())
                        .collect::<Vec<_>>()
                }),
            Some(vec![
                "choice",
                "regex",
                "grammar",
                "json_schema",
                "json_object",
                "tagged_structure",
            ])
        );
        assert_eq!(
            health.0.tool_calling.as_ref().map(|capability| (
                capability.support_level.label(),
                capability.supported_modes.clone(),
                capability.parser,
                capability.argument_validation,
            )),
            Some((
                "fallback",
                vec!["none", "auto", "required", "named"],
                "tagged_json_schema",
                "json_schema_subset",
            ))
        );
        assert_eq!(
            health.0.execution_profile.batch_posture,
            BatchExecutionPosture::ContinuousBatch
        );
        assert_eq!(
            health.0.execution_profile.queue_policy.discipline,
            QueueDiscipline::Fifo
        );
        assert!(
            health
                .0
                .scheduler_policy
                .as_ref()
                .is_some_and(|policy| policy.max_active_requests > 0)
        );
        let models = tokio::runtime::Runtime::new()?.block_on(generic_list_models(State(
            std::sync::Arc::clone(&server.state),
        )));
        assert_eq!(models.0.data.len(), 2);
        assert!(
            models
                .0
                .data
                .iter()
                .all(|model| model.psionic_residency_mode == Some(CPU_SERVER_RESIDENCY_MODE))
        );
        assert!(models.0.data.iter().all(|model| {
            model.psionic_structured_outputs.as_deref()
                == Some(
                    [
                        "choice_set",
                        "regex_subset",
                        "gbnf_subset",
                        "json_schema_subset",
                        "json_object",
                        "tagged_json_schema",
                    ]
                    .as_slice(),
                )
        }));
        assert!(models.0.data.iter().all(|model| {
            model
                .psionic_structured_output_capabilities
                .as_ref()
                .is_some_and(|capabilities| {
                    capabilities
                        .iter()
                        .all(|capability| capability.support_level.label() == "fallback")
                })
        }));
        assert!(models.0.data.iter().all(|model| {
            model
                .psionic_tool_calling
                .as_ref()
                .is_some_and(|capability| {
                    capability.support_level.label() == "fallback"
                        && capability.supported_modes == vec!["none", "auto", "required", "named"]
                        && capability.parser == "tagged_json_schema"
                })
        }));
        assert!(models.0.data.iter().all(|model| {
            model
                .psionic_execution_profile
                .as_ref()
                .map(|profile| profile.batch_posture)
                == Some(BatchExecutionPosture::ContinuousBatch)
        }));
        assert!(
            models
                .0
                .data
                .iter()
                .all(|model| model.psionic_scheduler_policy.is_some())
        );

        let request = ChatCompletionRequest {
            model: Some(String::from("tiny-qwen")),
            messages: vec![ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(1),
            stop: None,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
            response_format: None,
            psionic_grammar: None,
            psionic_structured_output: None,
            psionic_reasoning: None,
            psionic_prefix_cache: None,
        };
        let prompt_messages =
            chat_messages_to_prompt_messages_for_family(&request.messages, qwen_decoder.family)?;
        let rendered = render_prompt_for_model(qwen_model, prompt_messages.as_slice())?;
        let generation_request = GenerationRequest::new_text(
            String::from("generic-server-qwen"),
            qwen_decoder.descriptor.clone(),
            None,
            rendered.text,
            generation_options_from_chat_request_for_family(
                &request,
                qwen_decoder.family,
                rendered.stop_sequences.as_slice(),
            ),
        );
        let response = tokio::runtime::Runtime::new()?.block_on(
            server
                .state
                .workers
                .get(super::OPENAI_COMPAT_WORKER_ID)
                .expect("generic test worker should exist")
                .generate(qwen_model.model_key.clone(), generation_request),
        )?;
        assert_eq!(response.output.text, "world");
        Ok(())
    }

    #[test]
    fn generic_server_boots_and_generates_for_gpt_oss() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-gpt-oss.gguf");
        write_test_gguf(
            &path,
            gpt_oss_metadata().as_slice(),
            gpt_oss_tensors().as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let model = resolve_generic_model(server.state.as_ref(), None)
            .expect("default model should resolve");
        let decoder = model.decoder().expect("gpt-oss decoder model");
        assert_eq!(decoder.family, GgufDecoderFamily::GptOss);

        let request = ChatCompletionRequest {
            model: None,
            messages: vec![ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(1),
            stop: None,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
            response_format: None,
            psionic_grammar: None,
            psionic_structured_output: None,
            psionic_reasoning: None,
            psionic_prefix_cache: None,
        };
        let prompt_messages =
            chat_messages_to_prompt_messages_for_family(&request.messages, decoder.family)?;
        let rendered = render_prompt_for_model(model, prompt_messages.as_slice())?;
        let generation_request = GenerationRequest::new_text(
            String::from("generic-server-gpt-oss"),
            decoder.descriptor.clone(),
            None,
            rendered.text,
            generation_options_from_chat_request_for_family(
                &request,
                decoder.family,
                rendered.stop_sequences.as_slice(),
            ),
        );
        let response = tokio::runtime::Runtime::new()?.block_on(
            server
                .state
                .workers
                .get(super::OPENAI_COMPAT_WORKER_ID)
                .expect("generic test worker should exist")
                .generate(model.model_key.clone(), generation_request),
        )?;
        assert_eq!(response.usage.output_tokens, 1);
        Ok(())
    }

    #[test]
    fn generic_server_refuses_reasoning_request_for_unsupported_family()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny reasoning llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let error = tokio::runtime::Runtime::new()?
            .block_on(handle_generic_chat_completions(
                std::sync::Arc::clone(&server.state),
                ChatCompletionRequest {
                    model: Some(String::from("tiny-llama")),
                    messages: vec![ChatCompletionMessage {
                        role: String::from("user"),
                        content: String::from("hello"),
                        name: None,
                    }],
                    temperature: Some(0.0),
                    max_tokens: Some(1),
                    stop: None,
                    stream: false,
                    tools: Vec::new(),
                    tool_choice: None,
                    response_format: None,
                    psionic_grammar: None,
                    psionic_structured_output: None,
                    psionic_reasoning: Some(PsionicReasoningRequest {
                        parser: None,
                        mode: PsionicReasoningMode::Separate,
                    }),
                    psionic_prefix_cache: None,
                },
            ))
            .expect_err("llama family should refuse the reasoning parser contract");
        let payload =
            tokio::runtime::Runtime::new()?.block_on(response_json(error.into_response()))?;
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("does not expose a Psionic reasoning parser")
        );
        Ok(())
    }

    #[test]
    fn generic_server_surfaces_embeddings_truthfully() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let llama_path = temp.path().join("tiny-llama.gguf");
        let embeddings_path = temp.path().join("tiny-embed.safetensors");
        write_test_gguf(
            &llama_path,
            dense_llama_metadata("tiny server llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;
        ByteProjectionEmbedder::write_default_safetensors_artifact(&embeddings_path)?;

        let mut config = OpenAiCompatConfig::new(&llama_path);
        config.add_model_path(&embeddings_path);
        let server = OpenAiCompatServer::from_config(&config)?;

        let health = tokio::runtime::Runtime::new()?
            .block_on(generic_health(State(std::sync::Arc::clone(&server.state))));
        assert_eq!(
            health.0.supported_endpoints,
            vec!["/v1/chat/completions", "/v1/embeddings", "/v1/responses"]
        );
        assert_eq!(
            health
                .0
                .response_state
                .as_ref()
                .map(|capability| capability.continuation_modes.clone()),
            Some(vec![String::from("append_turn")])
        );

        let models = tokio::runtime::Runtime::new()?.block_on(generic_list_models(State(
            std::sync::Arc::clone(&server.state),
        )));
        let decoder_model = models
            .0
            .data
            .iter()
            .find(|model| {
                model.psionic_supported_endpoints.contains(&"/v1/responses")
                    && model.psionic_response_state.is_some()
            })
            .expect("decoder model should be listed");
        assert_eq!(
            decoder_model
                .psionic_response_state
                .as_ref()
                .map(|capability| capability.cache_behavior.clone()),
            Some(String::from("prompt_replay_only"))
        );
        let embeddings_model = models
            .0
            .data
            .iter()
            .find(|model| model.psionic_supported_endpoints == vec!["/v1/embeddings"])
            .expect("embeddings model should be listed");
        assert_eq!(embeddings_model.psionic_embedding_dimensions, Some(8));
        assert_eq!(embeddings_model.psionic_response_state, None);

        let response = tokio::runtime::Runtime::new()?.block_on(generic_embeddings(
            State(std::sync::Arc::clone(&server.state)),
            Json(EmbeddingsRequest {
                model: Some(String::from("tiny-embed")),
                input: EmbeddingsInput::Many(vec![String::from("hello"), String::from("world")]),
                dimensions: Some(4),
                encoding_format: Some(String::from("float")),
            }),
        ));
        assert_eq!(response.status(), StatusCode::OK);
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(payload["object"], serde_json::json!("list"));
        assert_eq!(payload["data"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            payload["data"][0]["embedding"].as_array().map(Vec::len),
            Some(4)
        );
        Ok(())
    }

    #[test]
    fn generic_responses_surface_runs_real_generation() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny response llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let response = tokio::runtime::Runtime::new()?.block_on(handle_generic_responses(
            std::sync::Arc::clone(&server.state),
            ResponsesRequest {
                model: Some(String::from("tiny-llama")),
                instructions: Some(String::from("Be brief.")),
                conversation: None,
                input: ResponsesInput::Text(String::from("hello")),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: Vec::new(),
                tool_choice: None,
                previous_response_id: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
            },
        ))?;
        assert_eq!(response.status(), StatusCode::OK);
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(payload["object"], serde_json::json!("response"));
        assert_eq!(payload["status"], serde_json::json!("completed"));
        assert_eq!(payload["output_text"], serde_json::json!("world"));
        assert_eq!(payload["previous_response_id"], serde_json::Value::Null);
        assert_eq!(
            payload["conversation"]["id"],
            serde_json::json!("psionic-conv-1")
        );
        assert_eq!(
            payload["psionic_response_state"]["stored"],
            serde_json::json!(true)
        );
        assert_eq!(payload["output"][0]["type"], serde_json::json!("message"));
        Ok(())
    }

    #[test]
    fn generic_responses_conversation_state_replays_and_updates()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-stateful-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny stateful llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let runtime = tokio::runtime::Runtime::new()?;
        let first_response = runtime.block_on(handle_generic_responses(
            std::sync::Arc::clone(&server.state),
            ResponsesRequest {
                model: Some(String::from("tiny-stateful-llama")),
                instructions: Some(String::from("Be brief.")),
                conversation: None,
                input: ResponsesInput::Text(String::from("hello")),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: Vec::new(),
                tool_choice: None,
                previous_response_id: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
            },
        ))?;
        let first_payload = runtime.block_on(response_json(first_response))?;
        assert_eq!(
            first_payload["psionic_response_state"]["replayed_prompt_messages"],
            serde_json::json!(0)
        );
        assert_eq!(
            first_payload["psionic_response_state"]["input_messages_appended"],
            serde_json::json!(2)
        );
        assert_eq!(
            first_payload["psionic_response_state"]["assistant_messages_recorded"],
            serde_json::json!(1)
        );
        assert_eq!(
            first_payload["psionic_response_state"]["conversation_item_count"],
            serde_json::json!(3)
        );
        let first_response_id = first_payload["id"]
            .as_str()
            .expect("response id")
            .to_string();
        let conversation_id = first_payload["conversation"]["id"]
            .as_str()
            .expect("conversation id")
            .to_string();

        let second_response = runtime.block_on(handle_generic_responses(
            std::sync::Arc::clone(&server.state),
            ResponsesRequest {
                model: None,
                instructions: None,
                conversation: Some(conversation_id.clone()),
                input: ResponsesInput::Text(String::from("again")),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: Vec::new(),
                tool_choice: None,
                previous_response_id: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
            },
        ))?;
        let second_payload = runtime.block_on(response_json(second_response))?;
        assert_eq!(
            second_payload["previous_response_id"],
            serde_json::json!(first_response_id)
        );
        assert_eq!(
            second_payload["conversation"]["id"],
            serde_json::json!(conversation_id)
        );
        assert_eq!(
            second_payload["conversation"]["revision"],
            serde_json::json!(2)
        );
        assert_eq!(
            second_payload["psionic_response_state"]["replayed_prompt_messages"],
            serde_json::json!(3)
        );
        assert_eq!(
            second_payload["psionic_response_state"]["input_messages_appended"],
            serde_json::json!(1)
        );
        assert_eq!(
            second_payload["psionic_response_state"]["conversation_item_count"],
            serde_json::json!(5)
        );
        Ok(())
    }

    #[test]
    fn generic_responses_file_backed_state_survives_server_restart()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let model_path = temp.path().join("tiny-durable-stateful-llama.gguf");
        let state_path = temp.path().join("response-state.json");
        write_test_gguf(
            &model_path,
            dense_llama_metadata("tiny durable stateful llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let config = OpenAiCompatConfig::new(&model_path);
        let runtime = tokio::runtime::Runtime::new()?;
        let first_server = OpenAiCompatServer::from_config_with_response_state_store(
            &config,
            ResponseStateStore::file_backed(&state_path, ResponseStateRetentionPolicy::default())?,
        )?;
        let first_response = runtime.block_on(handle_generic_responses(
            std::sync::Arc::clone(&first_server.state),
            ResponsesRequest {
                model: Some(String::from("tiny-durable-stateful-llama")),
                instructions: Some(String::from("Be brief.")),
                conversation: None,
                input: ResponsesInput::Text(String::from("hello")),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: Vec::new(),
                tool_choice: None,
                previous_response_id: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
            },
        ))?;
        let first_payload = runtime.block_on(response_json(first_response))?;
        let conversation_id = first_payload["conversation"]["id"]
            .as_str()
            .expect("conversation id")
            .to_string();
        let first_response_id = first_payload["id"]
            .as_str()
            .expect("response id")
            .to_string();

        let second_server = OpenAiCompatServer::from_config_with_response_state_store(
            &config,
            ResponseStateStore::file_backed(&state_path, ResponseStateRetentionPolicy::default())?,
        )?;
        let second_response = runtime.block_on(handle_generic_responses(
            std::sync::Arc::clone(&second_server.state),
            ResponsesRequest {
                model: None,
                instructions: None,
                conversation: Some(conversation_id.clone()),
                input: ResponsesInput::Text(String::from("again")),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: Vec::new(),
                tool_choice: None,
                previous_response_id: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
            },
        ))?;
        let second_payload = runtime.block_on(response_json(second_response))?;
        assert_eq!(
            second_payload["previous_response_id"],
            serde_json::json!(first_response_id)
        );
        assert_eq!(
            second_payload["conversation"]["id"],
            serde_json::json!(conversation_id)
        );
        assert_eq!(
            second_payload["psionic_response_state"]["storage"],
            serde_json::json!("json_file")
        );
        assert_eq!(
            second_payload["psionic_response_state"]["retention_scope"],
            serde_json::json!("best_effort_local_durable")
        );
        assert_eq!(
            second_payload["psionic_response_state"]["replayed_prompt_messages"],
            serde_json::json!(3)
        );
        Ok(())
    }

    #[test]
    fn generic_responses_refuse_unknown_state_references() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-unknown-state-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny unknown state llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let error = tokio::runtime::Runtime::new()?
            .block_on(handle_generic_responses(
                std::sync::Arc::clone(&server.state),
                ResponsesRequest {
                    model: Some(String::from("tiny-unknown-state-llama")),
                    instructions: None,
                    conversation: None,
                    input: ResponsesInput::Text(String::from("hello")),
                    temperature: Some(0.0),
                    max_output_tokens: Some(1),
                    stop: None,
                    stream: false,
                    tools: Vec::new(),
                    tool_choice: None,
                    previous_response_id: Some(String::from("resp-missing")),
                    psionic_structured_output: None,
                    psionic_reasoning: None,
                    psionic_response_state: None,
                    psionic_prefix_cache: None,
                },
            ))
            .expect_err("unknown response state should be refused");
        let payload =
            tokio::runtime::Runtime::new()?.block_on(response_json(error.into_response()))?;
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("unknown or expired")
        );
        Ok(())
    }

    #[test]
    fn generic_responses_refuse_instruction_changes_on_continuation()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-instruction-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny instruction llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let runtime = tokio::runtime::Runtime::new()?;
        let first_response = runtime.block_on(handle_generic_responses(
            std::sync::Arc::clone(&server.state),
            ResponsesRequest {
                model: Some(String::from("tiny-instruction-llama")),
                instructions: Some(String::from("Be brief.")),
                conversation: None,
                input: ResponsesInput::Text(String::from("hello")),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: Vec::new(),
                tool_choice: None,
                previous_response_id: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
            },
        ))?;
        let first_payload = runtime.block_on(response_json(first_response))?;
        let error = runtime
            .block_on(handle_generic_responses(
                std::sync::Arc::clone(&server.state),
                ResponsesRequest {
                    model: None,
                    instructions: Some(String::from("Be verbose.")),
                    conversation: Some(
                        first_payload["conversation"]["id"]
                            .as_str()
                            .expect("conversation id")
                            .to_string(),
                    ),
                    input: ResponsesInput::Text(String::from("again")),
                    temperature: Some(0.0),
                    max_output_tokens: Some(1),
                    stop: None,
                    stream: false,
                    tools: Vec::new(),
                    tool_choice: None,
                    previous_response_id: None,
                    psionic_structured_output: None,
                    psionic_reasoning: None,
                    psionic_response_state: None,
                    psionic_prefix_cache: None,
                },
            ))
            .expect_err("instruction drift should be refused");
        let payload = runtime.block_on(response_json(error.into_response()))?;
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("cannot change `instructions`")
        );
        Ok(())
    }

    #[test]
    fn generic_responses_refuse_unsupported_continue_last_assistant()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-continue-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny continue llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let error = tokio::runtime::Runtime::new()?
            .block_on(handle_generic_responses(
                std::sync::Arc::clone(&server.state),
                ResponsesRequest {
                    model: Some(String::from("tiny-continue-llama")),
                    instructions: None,
                    conversation: None,
                    input: ResponsesInput::Text(String::from("hello")),
                    temperature: Some(0.0),
                    max_output_tokens: Some(1),
                    stop: None,
                    stream: false,
                    tools: Vec::new(),
                    tool_choice: None,
                    previous_response_id: None,
                    psionic_structured_output: None,
                    psionic_reasoning: None,
                    psionic_response_state: Some(PsionicResponseStateRequest {
                        store: true,
                        continuation: ResponseContinuationMode::ContinueLastAssistant,
                        invalidate_references: false,
                    }),
                    psionic_prefix_cache: None,
                },
            ))
            .expect_err("continue_last_assistant should be refused on the current runtime");
        let payload =
            tokio::runtime::Runtime::new()?.block_on(response_json(error.into_response()))?;
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("continue_last_assistant")
        );
        Ok(())
    }

    #[test]
    fn generic_server_refuses_model_endpoint_mismatches() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let llama_path = temp.path().join("tiny-llama.gguf");
        let embeddings_path = temp.path().join("tiny-embed.safetensors");
        write_test_gguf(
            &llama_path,
            dense_llama_metadata("tiny server llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;
        ByteProjectionEmbedder::write_default_safetensors_artifact(&embeddings_path)?;

        let mut config = OpenAiCompatConfig::new(&llama_path);
        config.add_model_path(&embeddings_path);
        let server = OpenAiCompatServer::from_config(&config)?;

        let embeddings_response = tokio::runtime::Runtime::new()?.block_on(generic_embeddings(
            State(std::sync::Arc::clone(&server.state)),
            Json(EmbeddingsRequest {
                model: Some(String::from("tiny-llama")),
                input: EmbeddingsInput::One(String::from("hello")),
                dimensions: None,
                encoding_format: None,
            }),
        ));
        assert_eq!(embeddings_response.status(), StatusCode::BAD_REQUEST);
        let embeddings_payload =
            tokio::runtime::Runtime::new()?.block_on(response_json(embeddings_response))?;
        assert!(
            embeddings_payload["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("/v1/chat/completions"),
            "unsupported endpoint error should describe supported surfaces"
        );

        let responses_response = tokio::runtime::Runtime::new()?
            .block_on(handle_generic_responses(
                std::sync::Arc::clone(&server.state),
                ResponsesRequest {
                    model: Some(String::from("tiny-embed")),
                    instructions: None,
                    conversation: None,
                    input: ResponsesInput::Text(String::from("hello")),
                    temperature: Some(0.0),
                    max_output_tokens: Some(1),
                    stop: None,
                    stream: false,
                    tools: Vec::new(),
                    tool_choice: None,
                    previous_response_id: None,
                    psionic_structured_output: None,
                    psionic_reasoning: None,
                    psionic_response_state: None,
                    psionic_prefix_cache: None,
                },
            ))
            .expect_err("embeddings-only model should refuse responses");
        let response = responses_response.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("/v1/embeddings"),
            "unsupported endpoint error should describe supported surfaces"
        );
        Ok(())
    }

    #[test]
    fn generic_server_grammar_fallback_is_machine_checkable()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny grammar llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let request = ChatCompletionRequest {
            model: Some(String::from("tiny-llama")),
            messages: vec![ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(1),
            stop: None,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
            response_format: None,
            psionic_grammar: Some(PsionicGrammarRequest {
                grammar: String::from("root ::= \"psionic\"\n"),
                syntax: Some(StructuredGrammarSyntax::Gbnf),
            }),
            psionic_structured_output: None,
            psionic_reasoning: None,
            psionic_prefix_cache: None,
        };

        let response = tokio::runtime::Runtime::new()?.block_on(
            handle_generic_chat_completions(std::sync::Arc::clone(&server.state), request),
        )?;
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-mode"),
            Some(String::from("fallback_grammar"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-batch-posture"),
            Some(String::from("continuous_batch"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-scheduling-class"),
            Some(String::from("mixed_prefill_decode"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-prefill-decode-mode"),
            Some(String::from("disaggregated_colocated"))
        );
        assert!(
            header_value(response.headers(), "x-psionic-ttft-ns")
                .is_some_and(|value| !value.is_empty()),
            "TTFT header should be surfaced when measured"
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-parser"),
            Some(String::from("gbnf_subset"))
        );
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(payload["choices"][0]["message"]["content"], "psionic");
        assert_eq!(
            payload["psionic_structured_output"]["mode"],
            serde_json::json!("fallback_grammar")
        );
        assert_eq!(
            payload["psionic_structured_output"]["kind"],
            serde_json::json!("grammar")
        );
        assert_eq!(
            payload["psionic_structured_output"]["parser"],
            serde_json::json!("gbnf_subset")
        );
        assert_eq!(
            payload["psionic_structured_value"],
            serde_json::json!({
                "kind": "grammar",
                "value": "psionic"
            })
        );
        Ok(())
    }

    #[test]
    fn generic_server_json_schema_fallback_is_machine_checkable()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-json-llama.gguf");
        write_test_gguf(
            &path,
            json_llama_metadata("tiny json llama").as_slice(),
            dense_decoder_tensors_with_vocab(false, 8, 3, 6).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let request = ChatCompletionRequest {
            model: Some(String::from("tiny-json-llama")),
            messages: vec![ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(1),
            stop: None,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
            response_format: Some(ChatCompletionResponseFormatRequest {
                kind: String::from("json_schema"),
                json_schema: Some(ChatCompletionJsonSchemaRequest {
                    name: Some(String::from("ok_object")),
                    schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "ok": { "type": "boolean" }
                        },
                        "required": ["ok"],
                        "additionalProperties": false
                    }),
                    strict: Some(true),
                }),
                schema: None,
            }),
            psionic_grammar: None,
            psionic_structured_output: None,
            psionic_reasoning: None,
            psionic_prefix_cache: None,
        };

        let response = tokio::runtime::Runtime::new()?.block_on(
            handle_generic_chat_completions(std::sync::Arc::clone(&server.state), request),
        )?;
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-mode"),
            Some(String::from("fallback_json_schema"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-batch-posture"),
            Some(String::from("continuous_batch"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-scheduling-class"),
            Some(String::from("mixed_prefill_decode"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-prefill-decode-mode"),
            Some(String::from("disaggregated_colocated"))
        );
        assert!(
            header_value(response.headers(), "x-psionic-ttft-ns")
                .is_some_and(|value| !value.is_empty()),
            "TTFT header should be surfaced when measured"
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-parser"),
            Some(String::from("json_schema_subset"))
        );
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(payload["choices"][0]["message"]["content"], "{\"ok\":true}");
        assert_eq!(
            payload["psionic_structured_output"]["mode"],
            serde_json::json!("fallback_json_schema")
        );
        assert_eq!(
            payload["psionic_structured_output"]["kind"],
            serde_json::json!("json_schema")
        );
        assert_eq!(
            payload["psionic_structured_output"]["schema_name"],
            serde_json::json!("ok_object")
        );
        assert_eq!(
            payload["psionic_structured_value"],
            serde_json::json!({
                "kind": "json",
                "value": { "ok": true }
            })
        );
        Ok(())
    }

    #[test]
    fn generic_server_choice_structured_output_is_machine_checkable()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-choice-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny choice llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let request = ChatCompletionRequest {
            model: Some(String::from("tiny-choice-llama")),
            messages: vec![ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(1),
            stop: None,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
            response_format: None,
            psionic_grammar: None,
            psionic_structured_output: Some(StructuredOutputRequest::Choice {
                values: vec![String::from("world"), String::from("psionic")],
            }),
            psionic_reasoning: None,
            psionic_prefix_cache: None,
        };

        let response = tokio::runtime::Runtime::new()?.block_on(
            handle_generic_chat_completions(std::sync::Arc::clone(&server.state), request),
        )?;
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-mode"),
            Some(String::from("fallback_choice"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-parser"),
            Some(String::from("choice_set"))
        );
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(
            payload["choices"][0]["message"]["content"],
            serde_json::json!("world")
        );
        assert_eq!(
            payload["psionic_structured_output"]["kind"],
            serde_json::json!("choice")
        );
        assert_eq!(
            payload["psionic_structured_value"],
            serde_json::json!({
                "kind": "choice",
                "value": "world"
            })
        );
        Ok(())
    }

    #[test]
    fn generic_responses_regex_structured_output_is_machine_checkable()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-regex-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny regex llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let response = tokio::runtime::Runtime::new()?.block_on(handle_generic_responses(
            std::sync::Arc::clone(&server.state),
            ResponsesRequest {
                model: Some(String::from("tiny-regex-llama")),
                instructions: None,
                conversation: None,
                input: ResponsesInput::Text(String::from("hello")),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: Vec::new(),
                tool_choice: None,
                previous_response_id: None,
                psionic_structured_output: Some(StructuredOutputRequest::Regex {
                    pattern: String::from("w[a-z]{4}"),
                }),
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
            },
        ))?;
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-mode"),
            Some(String::from("fallback_regex"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-parser"),
            Some(String::from("regex_subset"))
        );
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(payload["output_text"], serde_json::json!("world"));
        assert_eq!(
            payload["psionic_structured_output"]["kind"],
            serde_json::json!("regex")
        );
        assert_eq!(
            payload["psionic_structured_value"],
            serde_json::json!({
                "kind": "regex",
                "value": "world"
            })
        );
        Ok(())
    }

    #[test]
    fn generic_server_tagged_structure_survives_as_machine_value()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-tagged-llama.gguf");
        write_test_gguf(
            &path,
            tagged_llama_metadata("tiny tagged llama").as_slice(),
            dense_decoder_tensors_with_vocab(false, 6, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let request = ChatCompletionRequest {
            model: Some(String::from("tiny-tagged-llama")),
            messages: vec![ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(1),
            stop: None,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
            response_format: None,
            psionic_grammar: None,
            psionic_structured_output: Some(StructuredOutputRequest::TaggedStructure {
                name: Some(String::from("decision")),
                discriminator: String::from("kind"),
                variants: vec![StructuredTaggedVariant {
                    tag: String::from("approve"),
                    schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "reason": { "type": "string", "minLength": 1 }
                        },
                        "required": ["reason"],
                        "additionalProperties": false
                    }),
                }],
            }),
            psionic_reasoning: None,
            psionic_prefix_cache: None,
        };

        let response = tokio::runtime::Runtime::new()?.block_on(
            handle_generic_chat_completions(std::sync::Arc::clone(&server.state), request),
        )?;
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-mode"),
            Some(String::from("fallback_tagged_structure"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-structured-output-parser"),
            Some(String::from("tagged_json_schema"))
        );
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(
            payload["choices"][0]["message"]["content"],
            serde_json::json!("{\"kind\":\"approve\",\"reason\":\"ok\"}")
        );
        assert_eq!(
            payload["psionic_structured_output"]["kind"],
            serde_json::json!("tagged_structure")
        );
        assert_eq!(
            payload["psionic_structured_value"],
            serde_json::json!({
                "kind": "tagged_structure",
                "discriminator": "kind",
                "tag": "approve",
                "value": {
                    "kind": "approve",
                    "reason": "ok"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn generic_server_tool_choice_none_preserves_plain_text_generation()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-tool-none-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny tool none llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let response =
            tokio::runtime::Runtime::new()?.block_on(handle_generic_chat_completions(
                std::sync::Arc::clone(&server.state),
                ChatCompletionRequest {
                    model: Some(String::from("tiny-tool-none-llama")),
                    messages: vec![ChatCompletionMessage {
                        role: String::from("user"),
                        content: String::from("hello"),
                        name: None,
                    }],
                    temperature: Some(0.0),
                    max_tokens: Some(1),
                    stop: None,
                    stream: false,
                    tools: vec![weather_tool_definition()],
                    tool_choice: Some(ToolChoiceRequest::Mode(String::from("none"))),
                    response_format: None,
                    psionic_grammar: None,
                    psionic_structured_output: None,
                    psionic_reasoning: None,
                    psionic_prefix_cache: None,
                },
            ))?;
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(
            payload["choices"][0]["message"]["content"],
            serde_json::json!("world")
        );
        assert!(payload["choices"][0]["message"]["tool_calls"].is_null());
        assert!(payload["psionic_tool_calls"].is_null());
        Ok(())
    }

    #[test]
    fn generic_server_tool_choice_auto_can_return_message_envelope()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-tool-auto-llama.gguf");
        write_test_gguf(
            &path,
            auto_tool_message_llama_metadata("tiny tool auto llama").as_slice(),
            dense_decoder_tensors_with_vocab(false, 6, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let response =
            tokio::runtime::Runtime::new()?.block_on(handle_generic_chat_completions(
                std::sync::Arc::clone(&server.state),
                ChatCompletionRequest {
                    model: Some(String::from("tiny-tool-auto-llama")),
                    messages: vec![ChatCompletionMessage {
                        role: String::from("user"),
                        content: String::from("hello"),
                        name: None,
                    }],
                    temperature: Some(0.0),
                    max_tokens: Some(1),
                    stop: None,
                    stream: false,
                    tools: vec![weather_tool_definition()],
                    tool_choice: Some(ToolChoiceRequest::Mode(String::from("auto"))),
                    response_format: None,
                    psionic_grammar: None,
                    psionic_structured_output: None,
                    psionic_reasoning: None,
                    psionic_prefix_cache: None,
                },
            ))?;
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(
            payload["choices"][0]["message"]["content"],
            serde_json::json!("world")
        );
        assert_eq!(
            payload["psionic_structured_value"],
            serde_json::json!({
                "kind": "tagged_structure",
                "discriminator": "kind",
                "tag": "message",
                "value": {
                    "kind": "message",
                    "content": "world"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn generic_server_required_tool_call_is_machine_checkable()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-tool-call-llama.gguf");
        write_test_gguf(
            &path,
            tool_call_llama_metadata("tiny tool call llama").as_slice(),
            dense_decoder_tensors_with_vocab(false, 6, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let response =
            tokio::runtime::Runtime::new()?.block_on(handle_generic_chat_completions(
                std::sync::Arc::clone(&server.state),
                ChatCompletionRequest {
                    model: Some(String::from("tiny-tool-call-llama")),
                    messages: vec![ChatCompletionMessage {
                        role: String::from("user"),
                        content: String::from("hello"),
                        name: None,
                    }],
                    temperature: Some(0.0),
                    max_tokens: Some(1),
                    stop: None,
                    stream: false,
                    tools: vec![weather_tool_definition()],
                    tool_choice: Some(ToolChoiceRequest::Mode(String::from("required"))),
                    response_format: None,
                    psionic_grammar: None,
                    psionic_structured_output: None,
                    psionic_reasoning: None,
                    psionic_prefix_cache: None,
                },
            ))?;
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(
            payload["choices"][0]["finish_reason"],
            serde_json::json!("tool_calls")
        );
        assert!(payload["choices"][0]["message"]["content"].is_null());
        assert_eq!(
            payload["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
            serde_json::json!("get_weather")
        );
        assert_eq!(
            payload["psionic_tool_calls"][0]["arguments"],
            serde_json::json!({
                "latitude": 48.8566,
                "longitude": 2.3522
            })
        );
        Ok(())
    }

    #[test]
    fn generic_responses_named_tool_choice_surfaces_tool_call()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-tool-response-llama.gguf");
        write_test_gguf(
            &path,
            tool_call_llama_metadata("tiny tool response llama").as_slice(),
            dense_decoder_tensors_with_vocab(false, 6, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let response = tokio::runtime::Runtime::new()?.block_on(handle_generic_responses(
            std::sync::Arc::clone(&server.state),
            ResponsesRequest {
                model: Some(String::from("tiny-tool-response-llama")),
                instructions: None,
                conversation: None,
                input: ResponsesInput::Text(String::from("hello")),
                temperature: Some(0.0),
                max_output_tokens: Some(1),
                stop: None,
                stream: false,
                tools: vec![weather_tool_definition()],
                tool_choice: Some(ToolChoiceRequest::Named(NamedToolChoiceRequest {
                    kind: String::from("function"),
                    function: NamedToolChoiceFunction {
                        name: String::from("get_weather"),
                    },
                })),
                previous_response_id: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_response_state: None,
                psionic_prefix_cache: None,
            },
        ))?;
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert_eq!(payload["output_text"], serde_json::json!(""));
        assert!(
            payload["output"]
                .as_array()
                .is_some_and(|items| items.is_empty())
        );
        assert_eq!(
            payload["psionic_tool_calls"][0]["name"],
            serde_json::json!("get_weather")
        );
        Ok(())
    }

    #[test]
    fn generic_server_tool_call_validation_refuses_invalid_arguments()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-tool-invalid-llama.gguf");
        write_test_gguf(
            &path,
            invalid_tool_call_llama_metadata("tiny tool invalid llama").as_slice(),
            dense_decoder_tensors_with_vocab(false, 6, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let response = tokio::runtime::Runtime::new()?.block_on(super::generic_chat_completions(
            State(std::sync::Arc::clone(&server.state)),
            Json(ChatCompletionRequest {
                model: Some(String::from("tiny-tool-invalid-llama")),
                messages: vec![ChatCompletionMessage {
                    role: String::from("user"),
                    content: String::from("hello"),
                    name: None,
                }],
                temperature: Some(0.0),
                max_tokens: Some(1),
                stop: None,
                stream: false,
                tools: vec![weather_tool_definition()],
                tool_choice: Some(ToolChoiceRequest::Mode(String::from("required"))),
                response_format: None,
                psionic_grammar: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_prefix_cache: None,
            }),
        ));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .len()
                > 8,
            "validation failures should surface through parser-backed refusal"
        );
        Ok(())
    }

    #[test]
    fn generic_server_streaming_tool_calls_preserve_machine_envelope()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-tool-stream-llama.gguf");
        write_test_gguf(
            &path,
            tool_call_llama_metadata("tiny tool stream llama").as_slice(),
            dense_decoder_tensors_with_vocab(false, 6, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let response =
            tokio::runtime::Runtime::new()?.block_on(handle_generic_chat_completions(
                std::sync::Arc::clone(&server.state),
                ChatCompletionRequest {
                    model: Some(String::from("tiny-tool-stream-llama")),
                    messages: vec![ChatCompletionMessage {
                        role: String::from("user"),
                        content: String::from("hello"),
                        name: None,
                    }],
                    temperature: Some(0.0),
                    max_tokens: Some(1),
                    stop: None,
                    stream: true,
                    tools: vec![weather_tool_definition()],
                    tool_choice: Some(ToolChoiceRequest::Mode(String::from("required"))),
                    response_format: None,
                    psionic_grammar: None,
                    psionic_structured_output: None,
                    psionic_reasoning: None,
                    psionic_prefix_cache: None,
                },
            ))?;
        let body = tokio::runtime::Runtime::new()?.block_on(response_text(response))?;
        assert!(body.contains("\"tool_calls\""));
        assert!(body.contains("\"finish_reason\":\"tool_calls\""));
        assert!(body.contains("[DONE]"));
        Ok(())
    }

    #[test]
    fn generic_server_prefix_cache_headers_are_machine_checkable()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-prefix-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny prefix llama").as_slice(),
            dense_decoder_tensors(false, 3, 5).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let tenant = String::from("tenant-a");
        let build_request =
            |prompt: &str, prefix_cache: PrefixCacheControl| ChatCompletionRequest {
                model: Some(String::from("tiny-prefix-llama")),
                messages: vec![ChatCompletionMessage {
                    role: String::from("user"),
                    content: String::from(prompt),
                    name: None,
                }],
                temperature: Some(0.0),
                max_tokens: Some(1),
                stop: None,
                stream: false,
                tools: Vec::new(),
                tool_choice: None,
                response_format: None,
                psionic_grammar: None,
                psionic_structured_output: None,
                psionic_reasoning: None,
                psionic_prefix_cache: Some(prefix_cache),
            };

        let seeded = tokio::runtime::Runtime::new()?.block_on(handle_generic_chat_completions(
            std::sync::Arc::clone(&server.state),
            build_request(
                "hello world",
                PrefixCacheControl {
                    mode: PrefixCacheMode::Auto,
                    tenant_id: Some(tenant.clone()),
                },
            ),
        ))?;
        assert_eq!(
            header_value(seeded.headers(), "x-psionic-prefix-cache-state"),
            Some(String::from("none"))
        );
        assert_eq!(
            header_value(seeded.headers(), "x-psionic-prefix-cache-reused-tokens"),
            Some(String::from("0"))
        );

        let hit = tokio::runtime::Runtime::new()?.block_on(handle_generic_chat_completions(
            std::sync::Arc::clone(&server.state),
            build_request(
                "hello",
                PrefixCacheControl {
                    mode: PrefixCacheMode::Auto,
                    tenant_id: Some(tenant.clone()),
                },
            ),
        ))?;
        assert_eq!(
            header_value(hit.headers(), "x-psionic-prefix-cache-state"),
            Some(String::from("hit"))
        );
        assert_eq!(
            header_value(hit.headers(), "x-psionic-prefix-cache-reused-tokens"),
            Some(String::from("1"))
        );
        assert_eq!(
            header_value(hit.headers(), "x-psionic-prefix-cache-refusal"),
            None
        );

        let bypassed =
            tokio::runtime::Runtime::new()?.block_on(handle_generic_chat_completions(
                std::sync::Arc::clone(&server.state),
                build_request(
                    "hello",
                    PrefixCacheControl {
                        mode: PrefixCacheMode::Bypass,
                        tenant_id: Some(tenant),
                    },
                ),
            ))?;
        assert_eq!(
            header_value(bypassed.headers(), "x-psionic-prefix-cache-state"),
            Some(String::from("bypassed"))
        );
        assert_eq!(
            header_value(bypassed.headers(), "x-psionic-prefix-cache-refusal"),
            Some(String::from("request_opt_out"))
        );
        assert_eq!(
            header_value(bypassed.headers(), "x-psionic-prefix-cache-reused-tokens"),
            Some(String::from("0"))
        );
        Ok(())
    }

    #[test]
    fn generic_server_route_headers_are_machine_checkable() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-route-llama.gguf");
        write_test_gguf(
            &path,
            dense_llama_metadata("tiny route llama").as_slice(),
            dense_decoder_tensors(false, 3, 4).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let response =
            tokio::runtime::Runtime::new()?.block_on(handle_generic_chat_completions(
                std::sync::Arc::clone(&server.state),
                ChatCompletionRequest {
                    model: Some(String::from("tiny-route-llama")),
                    messages: vec![ChatCompletionMessage {
                        role: String::from("user"),
                        content: String::from("hello"),
                        name: None,
                    }],
                    temperature: Some(0.0),
                    max_tokens: Some(1),
                    stop: None,
                    stream: false,
                    tools: Vec::new(),
                    tool_choice: None,
                    response_format: None,
                    psionic_grammar: None,
                    psionic_structured_output: None,
                    psionic_reasoning: None,
                    psionic_prefix_cache: None,
                },
            ))?;
        assert_eq!(
            header_value(response.headers(), "x-psionic-route-worker"),
            Some(String::from(super::OPENAI_COMPAT_WORKER_ID))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-route-strategy"),
            Some(String::from("warm_aware"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-route-eligible-workers"),
            Some(String::from("1"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-route-warm-workers"),
            Some(String::from("1"))
        );
        assert_eq!(
            header_value(response.headers(), "x-psionic-route-cache-matches"),
            Some(String::from("0"))
        );
        Ok(())
    }

    #[test]
    fn generic_server_refuses_unsupported_json_schema_features()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("tiny-json-llama.gguf");
        write_test_gguf(
            &path,
            json_llama_metadata("tiny json llama").as_slice(),
            dense_decoder_tensors_with_vocab(false, 8, 3, 6).as_slice(),
        )?;

        let server = OpenAiCompatServer::from_config(&OpenAiCompatConfig::new(&path))?;
        let request = ChatCompletionRequest {
            model: Some(String::from("tiny-json-llama")),
            messages: vec![ChatCompletionMessage {
                role: String::from("user"),
                content: String::from("hello"),
                name: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(1),
            stop: None,
            stream: false,
            tools: Vec::new(),
            tool_choice: None,
            response_format: Some(ChatCompletionResponseFormatRequest {
                kind: String::from("json_schema"),
                json_schema: Some(ChatCompletionJsonSchemaRequest {
                    name: None,
                    schema: serde_json::json!({
                        "type": "string",
                        "format": "uuid"
                    }),
                    strict: Some(true),
                }),
                schema: None,
            }),
            psionic_grammar: None,
            psionic_structured_output: None,
            psionic_reasoning: None,
            psionic_prefix_cache: None,
        };

        let response = tokio::runtime::Runtime::new()?.block_on(super::generic_chat_completions(
            State(std::sync::Arc::clone(&server.state)),
            Json(request),
        ));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = tokio::runtime::Runtime::new()?.block_on(response_json(response))?;
        assert!(
            payload["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("format"),
            "unsupported schema feature should be reported explicitly"
        );
        Ok(())
    }

    async fn response_json(
        response: Response,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        Ok(serde_json::from_slice(body.as_ref())?)
    }

    async fn response_text(response: Response) -> Result<String, Box<dyn std::error::Error>> {
        let body = to_bytes(response.into_body(), usize::MAX).await?;
        Ok(String::from_utf8(body.to_vec())?)
    }

    fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(String::from)
    }

    fn test_generation_response(text: &str) -> GenerationResponse {
        GenerationResponse {
            request_id: String::from("req-test"),
            product_id: String::from("psionic.text_generation"),
            model_id: String::from("tiny-gpt-oss"),
            session_id: None,
            output: GenerationOutput {
                tokens: TokenSequence::new(Vec::new()),
                text: String::from(text),
                structured: None,
                harmony: None,
            },
            usage: GenerationUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_tokens: 0,
            },
            metrics: GenerationMetrics::default(),
            provenance: None,
            termination: TerminationReason::EndOfSequence,
        }
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

    fn dense_llama_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("llama", name);
        metadata.extend(sentencepiece_tokenizer_metadata_entries());
        metadata
    }

    fn json_llama_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("llama", name);
        metadata.extend(sentencepiece_tokenizer_metadata_entries_with_tokens(vec![
            "<unk>",
            "<s>",
            "</s>",
            "hello",
            "world",
            "psionic",
            "{\"ok\":true}",
            "{\"ok\":false}",
        ]));
        metadata
    }

    fn tagged_llama_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("llama", name);
        metadata.extend(sentencepiece_tokenizer_metadata_entries_with_tokens(vec![
            "<unk>",
            "<s>",
            "</s>",
            "hello",
            "{\"kind\":\"approve\",\"reason\":\"ok\"}",
            "{\"kind\":\"reject\",\"code\":7}",
        ]));
        metadata
    }

    fn auto_tool_message_llama_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("llama", name);
        set_context_length(&mut metadata, "llama", 256);
        metadata.extend(sentencepiece_tokenizer_metadata_entries_with_tokens(vec![
            "<unk>",
            "<s>",
            "</s>",
            "hello",
            "{\"kind\":\"message\",\"content\":\"world\"}",
            "world",
        ]));
        metadata
    }

    fn tool_call_llama_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("llama", name);
        set_context_length(&mut metadata, "llama", 256);
        metadata.extend(sentencepiece_tokenizer_metadata_entries_with_tokens(vec![
            "<unk>",
            "<s>",
            "</s>",
            "hello",
            "{\"kind\":\"tool:get_weather\",\"latitude\":48.8566,\"longitude\":2.3522}",
            "world",
        ]));
        metadata
    }

    fn invalid_tool_call_llama_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("llama", name);
        set_context_length(&mut metadata, "llama", 256);
        metadata.extend(sentencepiece_tokenizer_metadata_entries_with_tokens(vec![
            "<unk>",
            "<s>",
            "</s>",
            "hello",
            "{\"kind\":\"tool:get_weather\",\"latitude\":\"oops\",\"longitude\":2.3522}",
            "world",
        ]));
        metadata
    }

    fn weather_tool_definition() -> ToolDefinitionEnvelope {
        ToolDefinitionEnvelope {
            kind: String::from("function"),
            function: ToolDefinitionRequest {
                name: String::from("get_weather"),
                description: Some(String::from("Get the weather for one coordinate pair.")),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "latitude": { "type": "number" },
                        "longitude": { "type": "number" }
                    },
                    "required": ["latitude", "longitude"],
                    "additionalProperties": false
                })),
            },
        }
    }

    fn dense_qwen_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("qwen2", name);
        metadata.extend(qwen_tokenizer_metadata_entries());
        metadata
    }

    fn set_context_length(
        metadata: &mut [(String, GgufMetadataValue)],
        architecture: &str,
        context_length: u32,
    ) {
        let key = format!("{architecture}.context_length");
        if let Some((_, value)) = metadata.iter_mut().find(|(candidate, _)| candidate == &key) {
            *value = GgufMetadataValue::U32(context_length);
        }
    }

    fn dense_family_header(architecture: &str, name: &str) -> Vec<(String, GgufMetadataValue)> {
        vec![
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
                GgufMetadataValue::U32(32),
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
                format!("{architecture}.attention.head_count_kv"),
                GgufMetadataValue::U32(1),
            ),
            (
                format!("{architecture}.attention.layer_norm_rms_epsilon"),
                GgufMetadataValue::F32(1e-5),
            ),
            (
                format!("{architecture}.rope.freq_base"),
                GgufMetadataValue::F32(10_000.0),
            ),
        ]
    }

    fn sentencepiece_tokenizer_metadata_entries() -> Vec<(String, GgufMetadataValue)> {
        sentencepiece_tokenizer_metadata_entries_with_tokens(vec![
            "<unk>", "<s>", "</s>", "hello", "world", "psionic",
        ])
    }

    fn sentencepiece_tokenizer_metadata_entries_with_tokens(
        tokens: Vec<&str>,
    ) -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("tokenizer.ggml.model"),
                GgufMetadataValue::String(String::from("llama")),
            ),
            (
                String::from("tokenizer.ggml.tokens"),
                GgufMetadataValue::Array(
                    tokens
                        .into_iter()
                        .map(|token| GgufMetadataValue::String(String::from(token)))
                        .collect(),
                ),
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
                GgufMetadataValue::Bool(false),
            ),
            (
                String::from("tokenizer.ggml.add_eos_token"),
                GgufMetadataValue::Bool(false),
            ),
        ]
    }

    fn qwen_tokenizer_metadata_entries() -> Vec<(String, GgufMetadataValue)> {
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
                    GgufMetadataValue::String(String::from("hello")),
                    GgufMetadataValue::String(String::from("world")),
                    GgufMetadataValue::String(String::from("psionic")),
                    GgufMetadataValue::String(String::from("agent")),
                ]),
            ),
            (
                String::from("tokenizer.ggml.merges"),
                GgufMetadataValue::Array(vec![GgufMetadataValue::String(String::from(
                    "hello world",
                ))]),
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
                String::from("tokenizer.ggml.add_bos_token"),
                GgufMetadataValue::Bool(false),
            ),
            (
                String::from("tokenizer.ggml.add_eos_token"),
                GgufMetadataValue::Bool(false),
            ),
        ]
    }

    fn dense_decoder_tensors(
        include_qkv_bias: bool,
        hello_token_index: usize,
        world_token_index: usize,
    ) -> Vec<TestGgufTensor> {
        dense_decoder_tensors_with_vocab(include_qkv_bias, 6, hello_token_index, world_token_index)
    }

    fn dense_decoder_tensors_with_vocab(
        include_qkv_bias: bool,
        vocab_size: usize,
        hello_token_index: usize,
        output_token_index: usize,
    ) -> Vec<TestGgufTensor> {
        let mut tensors = vec![
            dense_tensor(
                "token_embd.weight",
                vec![vocab_size, 4],
                token_embedding_values(vocab_size, hello_token_index),
            ),
            dense_tensor("output_norm.weight", vec![4], vec![1.0, 1.0, 1.0, 1.0]),
            dense_tensor(
                "output.weight",
                vec![vocab_size, 4],
                output_values(vocab_size, output_token_index),
            ),
            dense_tensor("blk.0.attn_norm.weight", vec![4], vec![1.0, 1.0, 1.0, 1.0]),
            dense_tensor("blk.0.attn_q.weight", vec![4, 4], vec![0.0; 16]),
            dense_tensor("blk.0.attn_k.weight", vec![2, 4], vec![0.0; 8]),
            dense_tensor("blk.0.attn_v.weight", vec![2, 4], vec![0.0; 8]),
            dense_tensor("blk.0.attn_output.weight", vec![4, 4], vec![0.0; 16]),
            dense_tensor("blk.0.ffn_gate.weight", vec![8, 4], vec![0.0; 32]),
            dense_tensor("blk.0.ffn_down.weight", vec![4, 8], vec![0.0; 32]),
            dense_tensor("blk.0.ffn_up.weight", vec![8, 4], vec![0.0; 32]),
            dense_tensor("blk.0.ffn_norm.weight", vec![4], vec![1.0, 1.0, 1.0, 1.0]),
        ];
        if include_qkv_bias {
            tensors.push(dense_tensor("blk.0.attn_q.bias", vec![4], vec![0.0; 4]));
            tensors.push(dense_tensor("blk.0.attn_k.bias", vec![2], vec![0.0; 2]));
            tensors.push(dense_tensor("blk.0.attn_v.bias", vec![2], vec![0.0; 2]));
        }
        tensors
    }

    fn token_embedding_values(vocab_size: usize, hello_token_index: usize) -> Vec<f32> {
        let mut values = vec![0.0; vocab_size * 4];
        values[hello_token_index.saturating_mul(4)] = 2.0;
        values
    }

    fn output_values(vocab_size: usize, output_token_index: usize) -> Vec<f32> {
        let mut values = vec![0.0; vocab_size * 4];
        values[output_token_index.saturating_mul(4)] = 1.0;
        values
    }

    fn gpt_oss_metadata() -> Vec<(String, GgufMetadataValue)> {
        vec![
            (
                String::from("general.architecture"),
                GgufMetadataValue::String(String::from("gpt-oss")),
            ),
            (
                String::from("general.name"),
                GgufMetadataValue::String(String::from("tiny psionic gpt-oss")),
            ),
            (
                String::from("general.alignment"),
                GgufMetadataValue::U32(32),
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
                GgufMetadataValue::U32(32),
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

    fn gpt_oss_tensors() -> Vec<TestGgufTensor> {
        let expert_blocks = 3 * 32;
        vec![
            quantized_q8_0_tensor("token_embd.weight", vec![6, 32]),
            dense_f32_tensor("output_norm.weight", vec![32]),
            quantized_q8_0_tensor("output.weight", vec![6, 32]),
            dense_f32_tensor("blk.0.attn_norm.weight", vec![32]),
            quantized_q8_0_tensor("blk.0.attn_q.weight", vec![64, 32]),
            dense_f32_tensor("blk.0.attn_q.bias", vec![64]),
            quantized_q8_0_tensor("blk.0.attn_k.weight", vec![16, 32]),
            dense_f32_tensor("blk.0.attn_k.bias", vec![16]),
            quantized_q8_0_tensor("blk.0.attn_v.weight", vec![16, 32]),
            dense_f32_tensor("blk.0.attn_v.bias", vec![16]),
            quantized_q8_0_tensor("blk.0.attn_output.weight", vec![32, 64]),
            dense_f32_tensor("blk.0.attn_output.bias", vec![32]),
            dense_f32_tensor("blk.0.post_attention_norm.weight", vec![32]),
            dense_f32_tensor("blk.0.attn_sinks.weight", vec![16]),
            dense_f32_tensor("blk.0.ffn_gate_inp.weight", vec![3, 32]),
            dense_f32_tensor("blk.0.ffn_gate_inp.bias", vec![3]),
            quantized_mxfp4_tensor(
                "blk.0.ffn_gate_exps.weight",
                vec![3, 32, 32],
                repeated_mxfp4_bytes(expert_blocks),
            ),
            dense_f32_tensor("blk.0.ffn_gate_exps.bias", vec![3, 32]),
            quantized_mxfp4_tensor(
                "blk.0.ffn_up_exps.weight",
                vec![3, 32, 32],
                repeated_mxfp4_bytes(expert_blocks),
            ),
            dense_f32_tensor("blk.0.ffn_up_exps.bias", vec![3, 32]),
            quantized_mxfp4_tensor(
                "blk.0.ffn_down_exps.weight",
                vec![3, 32, 32],
                repeated_mxfp4_bytes(expert_blocks),
            ),
            dense_f32_tensor("blk.0.ffn_down_exps.bias", vec![3, 32]),
        ]
    }

    fn dense_tensor(name: &str, shape: Vec<usize>, values: Vec<f32>) -> TestGgufTensor {
        TestGgufTensor::new(
            name,
            shape,
            GgufTensorType::F32,
            encode_f32_bytes(values.as_slice()),
        )
    }

    fn dense_f32_tensor(name: &str, shape: Vec<usize>) -> TestGgufTensor {
        let elements = shape.iter().product::<usize>();
        TestGgufTensor::new(
            name,
            shape,
            GgufTensorType::F32,
            encode_f32_bytes(&vec![0.0; elements]),
        )
    }

    fn quantized_q8_0_tensor(name: &str, shape: Vec<usize>) -> TestGgufTensor {
        let rows = shape
            .iter()
            .take(shape.len().saturating_sub(1))
            .product::<usize>();
        TestGgufTensor::new(name, shape, GgufTensorType::Q8_0, repeated_q8_0_bytes(rows))
    }

    fn quantized_mxfp4_tensor(name: &str, shape: Vec<usize>, bytes: Vec<u8>) -> TestGgufTensor {
        TestGgufTensor::new(name, shape, GgufTensorType::MXFP4, bytes)
    }

    fn repeated_q8_0_bytes(row_count: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(row_count * 34);
        for _ in 0..row_count {
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

    fn write_test_gguf(
        path: &std::path::Path,
        metadata: &[(String, GgufMetadataValue)],
        tensors: &[TestGgufTensor],
    ) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(path, build_test_gguf(metadata, tensors)?)?;
        Ok(())
    }

    fn build_test_gguf(
        metadata: &[(String, GgufMetadataValue)],
        tensors: &[TestGgufTensor],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let alignment = metadata
            .iter()
            .find(|(key, _)| key == "general.alignment")
            .and_then(|(_, value)| match value {
                GgufMetadataValue::U64(value) => Some(*value as usize),
                GgufMetadataValue::U32(value) => Some(*value as usize),
                _ => None,
            })
            .unwrap_or(32)
            .max(1);

        let mut bytes = Vec::new();
        bytes.extend(b"GGUF");
        push_u32(&mut bytes, 3);
        push_u64(&mut bytes, u64::try_from(tensors.len())?);
        push_u64(&mut bytes, u64::try_from(metadata.len())?);

        for (key, value) in metadata {
            push_gguf_string(&mut bytes, key)?;
            push_u32(&mut bytes, gguf_metadata_value_type(value));
            push_gguf_value(&mut bytes, value)?;
        }

        let mut next_offset = 0usize;
        let mut tensor_offsets = Vec::with_capacity(tensors.len());
        for tensor in tensors {
            tensor_offsets.push(next_offset);
            next_offset = align_usize(next_offset + tensor.bytes.len(), alignment);
        }

        for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
            push_gguf_string(&mut bytes, tensor.name.as_str())?;
            push_u32(&mut bytes, u32::try_from(tensor.shape.len())?);
            for dimension in tensor.shape.iter().rev() {
                push_u64(&mut bytes, u64::try_from(*dimension)?);
            }
            push_u32(&mut bytes, gguf_tensor_type_code(tensor.tensor_type));
            push_u64(&mut bytes, u64::try_from(*offset)?);
        }

        let tensor_data_offset = align_usize(bytes.len(), alignment);
        bytes.resize(tensor_data_offset, 0);

        for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
            let start = tensor_data_offset + offset;
            if bytes.len() < start {
                bytes.resize(start, 0);
            }
            bytes.extend_from_slice(tensor.bytes.as_slice());
            bytes.resize(align_usize(bytes.len(), alignment), 0);
        }

        Ok(bytes)
    }

    fn align_usize(value: usize, alignment: usize) -> usize {
        let remainder = value % alignment;
        if remainder == 0 {
            value
        } else {
            value + alignment - remainder
        }
    }

    fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>()
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
            GgufTensorType::Q8_0 => 8,
            GgufTensorType::MXFP4 => 39,
            other => panic!("unsupported synthetic gguf tensor type: {other:?}"),
        }
    }

    fn push_gguf_string(
        bytes: &mut Vec<u8>,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        push_u64(bytes, u64::try_from(value.len())?);
        bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn push_gguf_value(
        bytes: &mut Vec<u8>,
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
            GgufMetadataValue::String(value) => push_gguf_string(bytes, value)?,
            GgufMetadataValue::Array(values) => {
                let value_type = values.first().map_or(4, gguf_metadata_value_type);
                push_u32(bytes, value_type);
                push_u64(bytes, u64::try_from(values.len())?);
                for value in values {
                    push_gguf_value(bytes, value)?;
                }
            }
        }
        Ok(())
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend(value.to_le_bytes());
    }

    fn push_u64(bytes: &mut Vec<u8>, value: u64) {
        bytes.extend(value.to_le_bytes());
    }
}
