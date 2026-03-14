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
    GptOssHarmonyParsedOutput, GptOssHarmonyRenderContext, GptOssTokenizer, PromptChannelConfig,
    PromptMessage, PromptMessageRole, PromptReasoningEffort, PromptRenderOptions,
    parse_gpt_oss_harmony_text, render_gpt_oss_harmony_prompt,
};
use psionic_runtime::{
    ExecutionCapabilityProfile, GenerationSchedulerPolicy, GenerationSchedulerRequestReceipt,
    StructuredGrammarSyntax, StructuredOutputExecutionReport, StructuredOutputParser,
    StructuredOutputRequest, local_structured_output_parsers,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
};
use tokio_stream::iter;

use crate::{
    CpuGgufTextGenerationService, CudaGgufGptOssTextGenerationService,
    CudaGptOssTextGenerationError, DecodeStrategy, DecoderModelDescriptor, GenerationMetrics,
    GenerationOptions, GenerationRequest, GgufDecoderAdapterLoader, GptOssPerformanceMetrics,
    MetalGgufGptOssTextGenerationService, MetalGptOssTextGenerationError, PromptRenderError,
    ReferenceTextGenerationError, TerminationReason, TextGenerationExecutor, TokenSequence,
    continuous_batch_text_generation_execution_profile, default_generation_scheduler_policy,
};

const DEFAULT_MAX_TOKENS: usize = 256;
const HARMONY_RETURN_STOP: &str = "<|return|>";
const HARMONY_CALL_STOP: &str = "<|call|>";
const HARMONY_START_TOKEN: &str = "<|start|>";
const HARMONY_END_TOKEN: &str = "<|end|>";
const HARMONY_MESSAGE_TOKEN: &str = "<|message|>";
const HARMONY_CHANNEL_TOKEN: &str = "<|channel|>";
const CPU_SERVER_RESIDENCY_MODE: &str = "cpu_only";
const CPU_SERVER_HYBRID_OFFLOAD_MODE: &str = "unsupported";
const CPU_SERVER_FALLBACK_POLICY: &str = "refuse";
const CPU_SERVER_PERFORMANCE_CLASS: &str = "portable_cpu_degraded";
const CPU_SERVER_LOAD_STATUS: &str = "loaded";
const CPU_SERVER_WARM_CONTROL: &str = "not_implemented";
const CPU_SERVER_UNLOAD_CONTROL: &str = "not_implemented";
const CPU_SERVER_MEMORY_PRESSURE_REPORTING: &str = "not_implemented";

fn structured_output_parser_labels() -> Vec<&'static str> {
    local_structured_output_parsers()
        .into_iter()
        .map(StructuredOutputParser::label)
        .collect()
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
        let default_model_name = default_model_name(&config.model_path, &descriptor);
        let accepted_model_names = accepted_model_names(&config.model_path, &descriptor);
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
    worker: OpenAiCompatWorker,
    backend_label: &'static str,
    execution_mode_label: &'static str,
    execution_engine_label: &'static str,
    default_model_key: String,
    default_model_name: String,
    models_by_key: BTreeMap<String, OpenAiCompatLoadedModel>,
    model_aliases: BTreeMap<String, String>,
    include_psionic_fields: bool,
    request_counter: AtomicU64,
}

#[derive(Clone)]
struct OpenAiCompatLoadedModel {
    model_key: String,
    canonical_name: String,
    descriptor: DecoderModelDescriptor,
    family: GgufDecoderFamily,
    prompt_renderer: Option<GgufPromptTemplateRenderer>,
    prompt_options: PromptRenderOptions,
    execution_profile: ExecutionCapabilityProfile,
    scheduler_policy: GenerationSchedulerPolicy,
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
}

impl OpenAiCompatServer {
    pub fn from_config(config: &OpenAiCompatConfig) -> Result<Self, OpenAiCompatServerError> {
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
        let mut model_aliases = BTreeMap::new();
        let mut default_model_key = None;
        let mut default_canonical_model_name = None;

        for model_path in &config.model_paths {
            let artifact =
                GgufBlobArtifact::open_path(model_path, gpt_oss_local_blob_open_options())
                    .map_err(|error| OpenAiCompatServerError::Config(error.to_string()))?;
            let adapter = GgufDecoderAdapterLoader
                .load_blob_artifact(&artifact)
                .map_err(|error| OpenAiCompatServerError::Config(error.to_string()))?;
            let descriptor = adapter.descriptor().clone();
            let family = adapter.family_metadata().family;
            let canonical_name = default_model_name(model_path, &descriptor);
            let accepted_names = accepted_model_names(model_path, &descriptor);
            let prompt_options = prompt_options_for_family(family, config.reasoning_budget);
            let loaded_model = OpenAiCompatLoadedModel {
                model_key: descriptor.model.model_id.clone(),
                canonical_name: canonical_name.clone(),
                descriptor: descriptor.clone(),
                family,
                prompt_renderer: (!matches!(family, GgufDecoderFamily::GptOss))
                    .then(|| adapter.prompt_renderer()),
                prompt_options,
                execution_profile: continuous_batch_text_generation_execution_profile(),
                scheduler_policy: default_generation_scheduler_policy(),
            };
            if models_by_key
                .insert(loaded_model.model_key.clone(), loaded_model.clone())
                .is_some()
            {
                return Err(OpenAiCompatServerError::Config(format!(
                    "duplicate loaded model id `{}`",
                    descriptor.model.model_id
                )));
            }
            for accepted_name in accepted_names {
                if let Some(existing) =
                    model_aliases.insert(accepted_name.clone(), descriptor.model.model_id.clone())
                {
                    return Err(OpenAiCompatServerError::Config(format!(
                        "model alias `{accepted_name}` collides between `{existing}` and `{}`",
                        descriptor.model.model_id
                    )));
                }
            }
            if default_model_key.is_none() {
                default_model_key = Some(descriptor.model.model_id.clone());
                default_canonical_model_name = Some(canonical_name);
            }
        }

        let worker = OpenAiCompatWorker::spawn(config.model_paths.clone())?;
        Ok(Self {
            state: Arc::new(OpenAiCompatState {
                worker,
                backend_label: config.backend.label(),
                execution_mode_label: "native",
                execution_engine_label: "psionic",
                default_model_key: default_model_key.expect("validated non-empty model list"),
                default_model_name: default_canonical_model_name
                    .expect("validated non-empty model list"),
                models_by_key,
                model_aliases,
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
            .route("/health", get(generic_health))
            .route("/v1/models", get(generic_list_models))
            .route("/v1/chat/completions", post(generic_chat_completions))
            .route("/v1/embeddings", post(generic_embeddings_unsupported))
            .with_state(Arc::clone(&self.state))
    }

    pub async fn serve(&self, listener: TcpListener) -> Result<(), OpenAiCompatServerError> {
        axum::serve(listener, self.router())
            .await
            .map_err(OpenAiCompatServerError::Io)
    }
}

impl OpenAiCompatWorker {
    fn spawn(model_paths: Vec<PathBuf>) -> Result<Self, OpenAiCompatServerError> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name(String::from("psionic-openai-cpu-worker"))
            .spawn(move || {
                let mut services = BTreeMap::new();
                for model_path in &model_paths {
                    match CpuGgufTextGenerationService::from_gguf_path(model_path) {
                        Ok(service) => {
                            let model_key = service.model_descriptor().model.model_id.clone();
                            services.insert(model_key, service);
                        }
                        Err(error) => {
                            let _ = ready_tx.send(Err::<(), String>(error.to_string()));
                            return;
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
                    }) else {
                        continue;
                    };
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
                            other => remaining.push_back(other),
                        }
                    }
                    pending_commands = remaining;

                    let Some(service) = services.get_mut(model_key.as_str()) else {
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

impl IntoResponse for OpenAiCompatHttpError {
    fn into_response(self) -> Response {
        let (status, kind) = match &self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
            Self::PromptRender(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
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
    psionic_execution_profile: Option<ExecutionCapabilityProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_scheduler_policy: Option<GenerationSchedulerPolicy>,
}

async fn list_models(State(state): State<Arc<GptOssOpenAiCompatState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        data: vec![ModelCard {
            id: state.default_model_name.clone(),
            object: "model",
            owned_by: "psionic",
            psionic_served_backend: None,
            psionic_residency_mode: None,
            psionic_hybrid_offload: None,
            psionic_fallback_policy: None,
            psionic_performance_class: None,
            psionic_structured_outputs: None,
            psionic_execution_profile: None,
            psionic_scheduler_policy: None,
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
    structured_output_fallbacks: Vec<&'static str>,
    execution_profile: ExecutionCapabilityProfile,
    scheduler_policy: GenerationSchedulerPolicy,
}

async fn generic_health(
    State(state): State<Arc<OpenAiCompatState>>,
) -> Json<GenericHealthResponse> {
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
        structured_output_fallbacks: structured_output_parser_labels(),
        execution_profile: continuous_batch_text_generation_execution_profile(),
        scheduler_policy: default_generation_scheduler_policy(),
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
                psionic_served_backend: Some("cpu"),
                psionic_residency_mode: Some(CPU_SERVER_RESIDENCY_MODE),
                psionic_hybrid_offload: Some(CPU_SERVER_HYBRID_OFFLOAD_MODE),
                psionic_fallback_policy: Some(CPU_SERVER_FALLBACK_POLICY),
                psionic_performance_class: Some(CPU_SERVER_PERFORMANCE_CLASS),
                psionic_structured_outputs: Some(structured_output_parser_labels()),
                psionic_execution_profile: Some(model.execution_profile.clone()),
                psionic_scheduler_policy: Some(model.scheduler_policy.clone()),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response_format: Option<ChatCompletionResponseFormatRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_grammar: Option<PsionicGrammarRequest>,
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
    if request.response_format.is_some() && request.psionic_grammar.is_some() {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "structured output fallback accepts either `response_format` or `psionic_grammar`, not both",
        )));
    }

    if let Some(grammar) = &request.psionic_grammar {
        if grammar.grammar.trim().is_empty() {
            return Err(OpenAiCompatHttpError::BadRequest(String::from(
                "`psionic_grammar.grammar` must not be empty",
            )));
        }
        return Ok(Some(StructuredOutputRequest::Grammar {
            syntax: grammar.syntax.unwrap_or(StructuredGrammarSyntax::Gbnf),
            grammar: grammar.grammar.clone(),
        }));
    }

    let Some(response_format) = &request.response_format else {
        return Ok(None);
    };
    match response_format.kind.as_str() {
        "json_object" => {
            if let Some(schema) = response_format.schema.as_ref() {
                Ok(Some(StructuredOutputRequest::JsonSchema {
                    name: None,
                    schema: schema.clone(),
                }))
            } else {
                Ok(Some(StructuredOutputRequest::JsonObject))
            }
        }
        "json_schema" => {
            let Some(schema) = response_format.json_schema.as_ref() else {
                return Err(OpenAiCompatHttpError::BadRequest(String::from(
                    "`response_format.type = json_schema` requires a `json_schema` object",
                )));
            };
            Ok(Some(StructuredOutputRequest::JsonSchema {
                name: schema.name.clone(),
                schema: schema.schema.clone(),
            }))
        }
        other => Err(OpenAiCompatHttpError::BadRequest(format!(
            "unsupported `response_format.type` `{other}` for local structured output fallback"
        ))),
    }
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
    if structured_output_from_chat_request(&request)?.is_some() {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "structured output fallback is only available on `psionic-openai-server` today",
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
    );

    let worker = state.worker.as_ref().ok_or_else(|| {
        OpenAiCompatHttpError::from(GptOssOpenAiCompatGenerationError::BackendUnavailable {
            backend: state.backend_label,
            status: psionic_runtime::HealthStatus::Offline,
            message: String::from("gpt-oss native worker is not available"),
        })
    })?;
    let response = worker.generate(generation_request).await?;
    let parsed = if state.include_psionic_fields {
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
    let choice = completion_choice(&response, parsed.clone());
    if request.stream {
        let terminal_chunk = completion_terminal_chunk(
            request_id.as_str(),
            &response_model_name,
            response.termination,
            unix_timestamp_secs(),
        );
        let delta_chunk = serialize_event_data(&completion_delta_chunk(
            request_id.as_str(),
            response_model_name.as_str(),
            choice.content.as_str(),
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
    let model = resolve_generic_model(state.as_ref(), request.model.as_deref())?;
    let structured_output = structured_output_from_chat_request(&request)?;
    let prompt_messages =
        chat_messages_to_prompt_messages_for_family(&request.messages, model.family)?;
    let rendered = render_prompt_for_model(model, prompt_messages.as_slice())?;
    let request_id = next_generic_request_id(&state);
    let response_model_name = request
        .model
        .clone()
        .unwrap_or_else(|| model.canonical_name.clone());
    let options = generation_options_from_chat_request_for_family(
        &request,
        model.family,
        rendered.stop_sequences.as_slice(),
    );
    let mut options = options;
    options.structured_output = structured_output;
    let generation_request = GenerationRequest::new_text(
        request_id.clone(),
        model.descriptor.clone(),
        None,
        rendered.text,
        options,
    );

    let response = state
        .worker
        .generate(model.model_key.clone(), generation_request)
        .await
        .map_err(|error| {
            OpenAiCompatHttpError::from(GptOssOpenAiCompatGenerationError::Generation(error))
        })?;
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
    let choice = completion_choice_for_family(model.family, &response, parsed.clone());
    let structured_output_report = response
        .provenance
        .as_ref()
        .and_then(|value| value.structured_output.clone());
    let scheduler_receipt = response
        .provenance
        .as_ref()
        .and_then(|value| value.scheduler.clone());
    if request.stream {
        let terminal_chunk = completion_terminal_chunk(
            request_id.as_str(),
            &response_model_name,
            response.termination,
            unix_timestamp_secs(),
        );
        let delta_chunk = serialize_event_data(&completion_delta_chunk(
            request_id.as_str(),
            response_model_name.as_str(),
            choice.content.as_str(),
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
            structured_output_report.as_ref(),
            scheduler_receipt.as_ref(),
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
        psionic_scheduler: state
            .include_psionic_fields
            .then(|| scheduler_receipt.clone())
            .flatten(),
    };
    let mut response = Json(body).into_response();
    insert_generic_execution_headers(
        response.headers_mut(),
        state.as_ref(),
        structured_output_report.as_ref(),
        scheduler_receipt.as_ref(),
    );
    Ok(response)
}

async fn generic_embeddings_unsupported() -> Response {
    OpenAiCompatHttpError::BadRequest(String::from(
        "/v1/embeddings is not implemented on the generic Psionic server yet",
    ))
    .into_response()
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
    psionic_perf: Option<GptOssPerformanceMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_output_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_output_tokens: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_structured_output: Option<StructuredOutputExecutionReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    psionic_scheduler: Option<GenerationSchedulerRequestReceipt>,
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
    content: String,
}

#[derive(Clone, Debug, Serialize)]
struct ChatCompletionUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
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
}

#[derive(Clone, Debug)]
struct ParsedCompletionChoice {
    content: String,
    finish_reason: &'static str,
}

impl ParsedCompletionChoice {
    fn into_full_choice(self) -> ChatCompletionChoice {
        ChatCompletionChoice {
            index: 0,
            message: ChatCompletionResponseMessage {
                role: "assistant",
                content: self.content,
            },
            finish_reason: self.finish_reason,
        }
    }
}

fn completion_choice(
    response: &crate::GenerationResponse,
    parsed: Option<GptOssHarmonyParsedOutput>,
) -> ParsedCompletionChoice {
    let content = fast_final_assistant_content(response.output.text.as_str())
        .or_else(|| parsed.as_ref().and_then(final_assistant_content))
        .unwrap_or_else(|| response.output.text.clone());
    ParsedCompletionChoice {
        content,
        finish_reason: finish_reason(response.termination),
    }
}

fn completion_terminal_chunk(
    request_id: &str,
    model: &str,
    termination: TerminationReason,
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
            finish_reason: Some(finish_reason(termination)),
        }],
    }
}

fn completion_delta_chunk(
    request_id: &str,
    model: &str,
    content: &str,
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
                content: Some(content.to_string()),
            },
            finish_reason: None,
        }],
    }
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

fn next_generic_request_id(state: &OpenAiCompatState) -> String {
    let next = state.request_counter.fetch_add(1, Ordering::Relaxed);
    format!("psionic-chatcmpl-{next}")
}

fn insert_generic_execution_headers(
    headers: &mut HeaderMap,
    state: &OpenAiCompatState,
    structured_output: Option<&StructuredOutputExecutionReport>,
    scheduler: Option<&GenerationSchedulerRequestReceipt>,
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
    insert_structured_output_headers(headers, structured_output);
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

fn resolve_generic_model<'a>(
    state: &'a OpenAiCompatState,
    requested: Option<&str>,
) -> Result<&'a OpenAiCompatLoadedModel, OpenAiCompatHttpError> {
    let model_key = match requested {
        Some(requested) => state.model_aliases.get(requested).ok_or_else(|| {
            OpenAiCompatHttpError::BadRequest(format!(
                "requested model `{requested}` is not loaded"
            ))
        })?,
        None => &state.default_model_key,
    };
    state.models_by_key.get(model_key).ok_or_else(|| {
        OpenAiCompatHttpError::Internal(format!("loaded model `{model_key}` is missing"))
    })
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

fn render_prompt_for_model(
    model: &OpenAiCompatLoadedModel,
    messages: &[PromptMessage],
) -> Result<GenericRenderedPrompt, OpenAiCompatHttpError> {
    if matches!(model.family, GgufDecoderFamily::GptOss) {
        let text = render_gpt_oss_harmony_prompt(messages, true, Some(&model.prompt_options))
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
    let renderer = model.prompt_renderer.as_ref().ok_or_else(|| {
        OpenAiCompatHttpError::Internal(format!(
            "model `{}` is missing a generic prompt renderer",
            model.model_key
        ))
    })?;
    let rendered = match renderer.render_with_options(None, messages, true, &model.prompt_options) {
        Ok(rendered) => rendered,
        Err(PromptRenderError::MissingDefaultTemplate)
            if messages.len() == 1 && messages[0].role == PromptMessageRole::User =>
        {
            return Ok(GenericRenderedPrompt {
                text: messages[0].content.clone(),
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

fn completion_choice_for_family(
    family: GgufDecoderFamily,
    response: &crate::GenerationResponse,
    parsed: Option<GptOssHarmonyParsedOutput>,
) -> ParsedCompletionChoice {
    if matches!(family, GgufDecoderFamily::GptOss) {
        return completion_choice(response, parsed);
    }
    ParsedCompletionChoice {
        content: response.output.text.clone(),
        finish_reason: finish_reason(response.termination),
    }
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

fn final_assistant_content(parsed: &GptOssHarmonyParsedOutput) -> Option<String> {
    parsed
        .messages
        .iter()
        .rev()
        .find(|message| {
            message.role == PromptMessageRole::Assistant
                && message.channel.as_deref().unwrap_or("final") == "final"
        })
        .map(|message| message.content.clone())
}

fn fast_final_assistant_content(text: &str) -> Option<String> {
    let assistant_start = text.rfind(HARMONY_START_TOKEN)?;
    let assistant_header = &text[assistant_start..];
    if !assistant_header.starts_with("<|start|>assistant") {
        return None;
    }
    let message_offset = assistant_header.find(HARMONY_MESSAGE_TOKEN)?;
    let header = &assistant_header[..message_offset];
    if let Some(channel_offset) = header.find(HARMONY_CHANNEL_TOKEN) {
        let channel = &header[channel_offset + HARMONY_CHANNEL_TOKEN.len()..];
        if channel.trim() != "final" {
            return None;
        }
    }
    let content_start = assistant_start + message_offset + HARMONY_MESSAGE_TOKEN.len();
    let content_tail = &text[content_start..];
    let content_end = [HARMONY_END_TOKEN, HARMONY_RETURN_STOP, HARMONY_CALL_STOP]
        .iter()
        .filter_map(|marker| content_tail.find(marker))
        .min()
        .unwrap_or(content_tail.len());
    Some(content_tail[..content_end].to_string())
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

fn default_model_name(path: &Path, descriptor: &DecoderModelDescriptor) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(String::from)
        .unwrap_or_else(|| descriptor.model.model_id.clone())
}

fn accepted_model_names(path: &Path, descriptor: &DecoderModelDescriptor) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    names.insert(descriptor.model.model_id.clone());
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
        ChatCompletionResponseFormatRequest, GptOssMetalExecutionMode, GptOssOpenAiCompatBackend,
        HARMONY_CALL_STOP, HARMONY_RETURN_STOP, OpenAiCompatConfig, OpenAiCompatServer,
        PromptTokenCache, PsionicGrammarRequest, chat_messages_to_prompt_messages,
        chat_messages_to_prompt_messages_for_family, ensure_harmony_stop_sequences,
        fast_final_assistant_content, final_assistant_content,
        generation_options_from_chat_request, generation_options_from_chat_request_for_family,
        generic_health, generic_list_models, handle_generic_chat_completions,
        prompt_request_cache_key, render_prompt_for_model, resolve_execution_summary,
        resolve_generic_model,
    };
    use crate::GenerationRequest;
    use axum::{
        Json,
        body::to_bytes,
        extract::State,
        http::{HeaderMap, StatusCode},
        response::Response,
    };
    use psionic_models::{
        GgufDecoderFamily, GgufMetadataValue, GgufTensorType, GptOssHarmonyParseSource,
        GptOssHarmonyParsedOutput, GptOssHarmonyRenderContext, PromptChannelConfig, PromptMessage,
        PromptMessageRole, PromptReasoningEffort, PromptRenderOptions, TokenId, TokenSequence,
        render_gpt_oss_harmony_prompt,
    };
    use psionic_runtime::{BatchExecutionPosture, QueueDiscipline, StructuredGrammarSyntax};

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
            response_format: None,
            psionic_grammar: None,
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
    fn final_assistant_content_prefers_final_channel() {
        let parsed = GptOssHarmonyParsedOutput {
            source: GptOssHarmonyParseSource::Text,
            messages: vec![
                PromptMessage::new(PromptMessageRole::Assistant, "working")
                    .with_channel("analysis"),
                PromptMessage::new(PromptMessageRole::Assistant, "323").with_channel("final"),
            ],
        };

        assert_eq!(final_assistant_content(&parsed).as_deref(), Some("323"));
    }

    #[test]
    fn fast_final_assistant_content_extracts_final_channel() {
        let text = "<|start|>assistant<|channel|>analysis<|message|>thinking<|end|><|start|>assistant<|channel|>final<|message|>323<|end|>";
        assert_eq!(fast_final_assistant_content(text).as_deref(), Some("323"));
    }

    #[test]
    fn fast_final_assistant_content_ignores_non_final_channel() {
        let text = "<|start|>assistant<|channel|>analysis<|message|>thinking<|end|>";
        assert_eq!(fast_final_assistant_content(text), None);
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

        assert_eq!(llama_model.family, GgufDecoderFamily::Llama);
        assert_eq!(qwen_model.family, GgufDecoderFamily::Qwen);
        assert_eq!(server.state.models_by_key.len(), 2);
        let health = tokio::runtime::Runtime::new()?
            .block_on(generic_health(State(std::sync::Arc::clone(&server.state))));
        assert_eq!(health.0.residency_mode, CPU_SERVER_RESIDENCY_MODE);
        assert_eq!(health.0.fallback_policy, CPU_SERVER_FALLBACK_POLICY);
        assert_eq!(health.0.hybrid_offload, CPU_SERVER_HYBRID_OFFLOAD_MODE);
        assert_eq!(
            health.0.structured_output_fallbacks,
            vec!["gbnf_subset", "json_schema_subset", "json_object"]
        );
        assert_eq!(
            health.0.execution_profile.batch_posture,
            BatchExecutionPosture::ContinuousBatch
        );
        assert_eq!(
            health.0.execution_profile.queue_policy.discipline,
            QueueDiscipline::Fifo
        );
        assert!(health.0.scheduler_policy.max_active_requests > 0);
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
        assert!(
            models
                .0
                .data
                .iter()
                .all(|model| model.psionic_structured_outputs.as_deref()
                    == Some(["gbnf_subset", "json_schema_subset", "json_object"].as_slice()))
        );
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
            response_format: None,
            psionic_grammar: None,
        };
        let prompt_messages =
            chat_messages_to_prompt_messages_for_family(&request.messages, qwen_model.family)?;
        let rendered = render_prompt_for_model(qwen_model, prompt_messages.as_slice())?;
        let generation_request = GenerationRequest::new_text(
            String::from("generic-server-qwen"),
            qwen_model.descriptor.clone(),
            None,
            rendered.text,
            generation_options_from_chat_request_for_family(
                &request,
                qwen_model.family,
                rendered.stop_sequences.as_slice(),
            ),
        );
        let response = tokio::runtime::Runtime::new()?.block_on(
            server
                .state
                .worker
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
        assert_eq!(model.family, GgufDecoderFamily::GptOss);

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
            response_format: None,
            psionic_grammar: None,
        };
        let prompt_messages =
            chat_messages_to_prompt_messages_for_family(&request.messages, model.family)?;
        let rendered = render_prompt_for_model(model, prompt_messages.as_slice())?;
        let generation_request = GenerationRequest::new_text(
            String::from("generic-server-gpt-oss"),
            model.descriptor.clone(),
            None,
            rendered.text,
            generation_options_from_chat_request_for_family(
                &request,
                model.family,
                rendered.stop_sequences.as_slice(),
            ),
        );
        let response = tokio::runtime::Runtime::new()?.block_on(
            server
                .state
                .worker
                .generate(model.model_key.clone(), generation_request),
        )?;
        assert_eq!(response.usage.output_tokens, 1);
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
            response_format: None,
            psionic_grammar: Some(PsionicGrammarRequest {
                grammar: String::from("root ::= \"psionic\"\n"),
                syntax: Some(StructuredGrammarSyntax::Gbnf),
            }),
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
            payload["psionic_structured_output"]["parser"],
            serde_json::json!("gbnf_subset")
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
            payload["psionic_structured_output"]["schema_name"],
            serde_json::json!("ok_object")
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
            response_format: Some(ChatCompletionResponseFormatRequest {
                kind: String::from("json_schema"),
                json_schema: Some(ChatCompletionJsonSchemaRequest {
                    name: None,
                    schema: serde_json::json!({
                        "type": "string",
                        "pattern": "^ok$"
                    }),
                    strict: Some(true),
                }),
                schema: None,
            }),
            psionic_grammar: None,
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
                .contains("pattern"),
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

    fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(String::from)
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

    fn dense_qwen_metadata(name: &str) -> Vec<(String, GgufMetadataValue)> {
        let mut metadata = dense_family_header("qwen2", name);
        metadata.extend(qwen_tokenizer_metadata_entries());
        metadata
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
