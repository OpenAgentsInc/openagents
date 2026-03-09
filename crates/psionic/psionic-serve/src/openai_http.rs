use std::{
    collections::{BTreeSet, VecDeque},
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
    GgufBlobArtifact, GptOssHarmonyParseOptions, GptOssHarmonyParsedOutput,
    GptOssHarmonyRenderContext, GptOssTokenizer, PromptChannelConfig, PromptMessage,
    PromptMessageRole, PromptReasoningEffort, PromptRenderOptions, parse_gpt_oss_harmony_tokens,
    render_gpt_oss_harmony_prompt,
};
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
};
use tokio_stream::iter;

use crate::{
    CudaGgufGptOssTextGenerationService, CudaGptOssTextGenerationError, DecodeStrategy,
    DecoderModelDescriptor, GenerationMetrics, GenerationOptions, GenerationRequest,
    GgufDecoderAdapterLoader, GptOssPerformanceMetrics, MetalGgufGptOssTextGenerationService,
    MetalGptOssTextGenerationError, PromptRenderError, ReferenceTextGenerationError,
    TerminationReason, TextGenerationExecutor, TokenSequence,
};

const DEFAULT_MAX_TOKENS: usize = 256;
const HARMONY_RETURN_STOP: &str = "<|return|>";
const HARMONY_CALL_STOP: &str = "<|call|>";
const HARMONY_START_TOKEN: &str = "<|start|>";
const HARMONY_END_TOKEN: &str = "<|end|>";
const HARMONY_MESSAGE_TOKEN: &str = "<|message|>";
const HARMONY_CHANNEL_TOKEN: &str = "<|channel|>";

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
        if let Ok(mut child) = self.child.lock() {
            if let Some(mut child) = child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[derive(Clone, Debug)]
struct PromptTokenCacheEntry {
    rendered_prompt: String,
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

    fn lookup(&mut self, rendered_prompt: &str) -> Option<TokenSequence> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.rendered_prompt == rendered_prompt)?;
        let entry = self.entries.remove(index)?;
        let tokens = entry.tokens.clone();
        self.entries.push_front(entry);
        Some(tokens)
    }

    fn record(&mut self, rendered_prompt: String, tokens: TokenSequence) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.rendered_prompt == rendered_prompt)
        {
            self.entries.remove(index);
        }
        self.entries.push_front(PromptTokenCacheEntry {
            rendered_prompt,
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

    #[must_use]
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

    #[must_use]
    pub fn router(&self) -> Router {
        self.inner.router()
    }

    pub async fn serve(&self, listener: TcpListener) -> Result<(), OpenAiCompatServerError> {
        self.inner.serve(listener).await
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
        if let Ok(response) = health_client.get(health_url.as_str()).send() {
            if response.status().is_success() {
                if let Ok(response) = chat_client.post(chat_url.as_str()).json(&probe).send() {
                    if response.status().is_success() {
                        return Ok(());
                    }
                    if response.status() != reqwest::StatusCode::SERVICE_UNAVAILABLE {
                        return Err(OpenAiCompatServerError::Config(format!(
                            "llama.cpp proxy readiness probe failed with status {}",
                            response.status()
                        )));
                    }
                }
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
    #[error(transparent)]
    PromptRender(#[from] PromptRenderError),
    #[error(transparent)]
    Generation(#[from] GptOssOpenAiCompatGenerationError),
}

impl IntoResponse for OpenAiCompatHttpError {
    fn into_response(self) -> Response {
        let (status, kind) = match &self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
            Self::PromptRender(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
            Self::Generation(GptOssOpenAiCompatGenerationError::BackendUnavailable { .. }) => {
                (StatusCode::SERVICE_UNAVAILABLE, "backend_unavailable")
            }
            Self::Generation(GptOssOpenAiCompatGenerationError::Generation(error)) => (
                StatusCode::from_u16(error.diagnostic().status)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                "generation_error",
            ),
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
}

async fn list_models(State(state): State<Arc<GptOssOpenAiCompatState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        data: vec![ModelCard {
            id: state.default_model_name.clone(),
            object: "model",
            owned_by: "psionic",
        }],
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

impl StopSequences {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
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
    if let Some(proxy) = state.proxy.as_ref() {
        return proxy_chat_completions(state.as_ref(), proxy, &request).await;
    }
    validate_requested_model(request.model.as_deref(), &state.accepted_model_names)?;
    let prompt_messages = chat_messages_to_prompt_messages(&request.messages)?;
    let rendered = render_gpt_oss_harmony_prompt(
        prompt_messages.as_slice(),
        true,
        Some(&state.prompt_options),
    )
    .map_err(|error| {
        OpenAiCompatHttpError::PromptRender(PromptRenderError::HarmonyRendering {
            message: error.to_string(),
        })
    })?;

    let request_id = next_request_id(&state);
    let response_model_name = request
        .model
        .clone()
        .unwrap_or_else(|| state.default_model_name.clone());
    let options = generation_options_from_chat_request(&request);
    let prompt_tokens = {
        let mut cache = state.prompt_token_cache.lock().map_err(|_| {
            OpenAiCompatHttpError::Generation(GptOssOpenAiCompatGenerationError::Generation(
                ReferenceTextGenerationError::Runtime(psionic_runtime::RuntimeError::Backend(
                    String::from("openai prompt token cache is poisoned"),
                )),
            ))
        })?;
        if let Some(tokens) = cache.lookup(rendered.as_str()) {
            tokens
        } else {
            let tokens = state.tokenizer.encode_with_defaults(rendered.as_str());
            cache.record(rendered, tokens.clone());
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

    let response = state
        .worker
        .as_ref()
        .expect("native worker is present when proxy mode is disabled")
        .generate(generation_request)
        .await?;
    let parsed = if state.include_psionic_fields {
        parse_gpt_oss_harmony_tokens(
            response.output.tokens.as_slice(),
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
        let events = vec![
            Ok::<_, Infallible>(
                Event::default().data(
                    serde_json::to_string(&completion_delta_chunk(
                        request_id.as_str(),
                        response_model_name.as_str(),
                        choice.content.as_str(),
                        unix_timestamp_secs(),
                    ))
                    .unwrap(),
                ),
            ),
            Ok::<_, Infallible>(
                Event::default().data(serde_json::to_string(&terminal_chunk).unwrap()),
            ),
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
    })
    .into_response();
    insert_execution_headers(response.headers_mut(), state.as_ref());
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
            OpenAiCompatHttpError::Generation(
                GptOssOpenAiCompatGenerationError::BackendUnavailable {
                    backend: "metal-proxy",
                    status: psionic_runtime::HealthStatus::Offline,
                    message: format!("llama.cpp proxy request failed: {error}"),
                },
            )
        })?;
    let status = upstream.status();
    let content_type = upstream
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .cloned();
    let body = upstream.bytes().await.map_err(|error| {
        OpenAiCompatHttpError::Generation(GptOssOpenAiCompatGenerationError::BackendUnavailable {
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

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn generation_options_from_chat_request(request: &ChatCompletionRequest) -> GenerationOptions {
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
    ensure_harmony_stop_sequences(&mut options.stop_sequences);
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
    if messages.is_empty() {
        return Err(OpenAiCompatHttpError::BadRequest(String::from(
            "chat completions require at least one message",
        )));
    }
    messages
        .iter()
        .map(|message| {
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
            Ok(prompt)
        })
        .collect()
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
        ChatCompletionMessage, ChatCompletionRequest, GptOssMetalExecutionMode,
        GptOssOpenAiCompatBackend, HARMONY_CALL_STOP, HARMONY_RETURN_STOP, PromptTokenCache,
        chat_messages_to_prompt_messages, ensure_harmony_stop_sequences,
        fast_final_assistant_content, final_assistant_content,
        generation_options_from_chat_request, resolve_execution_summary,
    };
    use psionic_models::{
        GptOssHarmonyParseSource, GptOssHarmonyParsedOutput, PromptMessage, PromptMessageRole,
        TokenId, TokenSequence,
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

        assert_eq!(prompt[0].role, PromptMessageRole::System);
        assert_eq!(prompt[1].role, PromptMessageRole::Tool);
        assert_eq!(
            prompt[1].author_name.as_deref(),
            Some("functions.lookup_weather")
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
            String::from("one"),
            TokenSequence::new(vec![TokenId(1), TokenId(2)]),
        );
        cache.record(String::from("two"), TokenSequence::new(vec![TokenId(3)]));

        assert_eq!(
            cache.lookup("one").expect("cached prompt").as_slice(),
            &[TokenId(1), TokenId(2)]
        );

        cache.record(String::from("three"), TokenSequence::new(vec![TokenId(4)]));

        assert!(cache.lookup("two").is_none());
        assert_eq!(
            cache.lookup("three").expect("cached prompt").as_slice(),
            &[TokenId(4)]
        );
    }
}
