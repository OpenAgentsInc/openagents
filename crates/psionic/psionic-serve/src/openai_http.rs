use std::{
    collections::BTreeSet,
    convert::Infallible,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use psionic_catalog::LocalBlobOpenOptions;
use psionic_models::{
    GgufBlobArtifact, GptOssHarmonyParseOptions, GptOssHarmonyParsedOutput,
    GptOssHarmonyRenderContext, PromptChannelConfig, PromptMessage, PromptMessageRole,
    PromptReasoningEffort, PromptRenderOptions, parse_gpt_oss_harmony_tokens,
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
    DecoderModelDescriptor, GenerationOptions, GenerationRequest, GgufDecoderAdapterLoader,
    GptOssPerformanceMetrics, PromptRenderError, TerminationReason, TextGenerationExecutor,
};

const DEFAULT_MAX_TOKENS: usize = 256;
const HARMONY_RETURN_STOP: &str = "<|return|>";
const HARMONY_CALL_STOP: &str = "<|call|>";

#[derive(Clone, Debug)]
pub struct GptOssOpenAiCompatConfig {
    pub model_path: PathBuf,
    pub host: String,
    pub port: u16,
    pub context_length: Option<usize>,
    pub gpu_layers: Option<i32>,
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
            context_length: None,
            gpu_layers: None,
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
pub struct GptOssCudaOpenAiCompatServer {
    state: Arc<GptOssCudaOpenAiCompatState>,
}

struct GptOssCudaOpenAiCompatState {
    worker: GptOssCudaWorker,
    descriptor: DecoderModelDescriptor,
    prompt_options: PromptRenderOptions,
    default_model_name: String,
    accepted_model_names: BTreeSet<String>,
    request_counter: AtomicU64,
}

impl GptOssCudaOpenAiCompatServer {
    pub fn from_config(config: &GptOssOpenAiCompatConfig) -> Result<Self, OpenAiCompatServerError> {
        let artifact =
            GgufBlobArtifact::open_path(&config.model_path, LocalBlobOpenOptions::default())
                .map_err(|error| OpenAiCompatServerError::Config(error.to_string()))?;
        let descriptor = GgufDecoderAdapterLoader
            .load_blob_artifact(&artifact)
            .map_err(|error| OpenAiCompatServerError::Config(error.to_string()))?
            .descriptor()
            .clone();
        let default_model_name = default_model_name(&config.model_path, &descriptor);
        let accepted_model_names = accepted_model_names(&config.model_path, &descriptor);
        let prompt_options = PromptRenderOptions {
            gpt_oss_harmony: Some(GptOssHarmonyRenderContext {
                reasoning_effort: Some(reasoning_effort(config.reasoning_budget)),
                channel_config: Some(PromptChannelConfig::default()),
                ..Default::default()
            }),
        };
        Ok(Self {
            state: Arc::new(GptOssCudaOpenAiCompatState {
                worker: GptOssCudaWorker::spawn(config.model_path.clone())?,
                descriptor,
                prompt_options,
                default_model_name,
                accepted_model_names,
                request_counter: AtomicU64::new(1),
            }),
        })
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

#[derive(Debug, thiserror::Error)]
pub enum OpenAiCompatServerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Config(String),
}

#[derive(Clone)]
struct GptOssCudaWorker {
    sender: mpsc::UnboundedSender<GptOssCudaWorkerCommand>,
}

enum GptOssCudaWorkerCommand {
    Generate {
        request: GenerationRequest,
        reply: oneshot::Sender<Result<crate::GenerationResponse, CudaGptOssTextGenerationError>>,
    },
}

impl GptOssCudaWorker {
    fn spawn(model_path: PathBuf) -> Result<Self, OpenAiCompatServerError> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name(String::from("psionic-gpt-oss-cuda-worker"))
            .spawn(move || {
                match CudaGgufGptOssTextGenerationService::from_gguf_path(&model_path) {
                    Ok(mut service) => {
                        let _ = ready_tx.send(Ok::<(), String>(()));
                        while let Some(command) = receiver.blocking_recv() {
                            match command {
                                GptOssCudaWorkerCommand::Generate { request, reply } => {
                                    let _ = reply.send(service.generate(&request));
                                }
                            }
                        }
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                    }
                }
            })?;
        match ready_rx.recv().map_err(|error| {
            OpenAiCompatServerError::Config(format!(
                "failed to receive GPT-OSS CUDA worker readiness: {error}"
            ))
        })? {
            Ok(()) => Ok(Self { sender }),
            Err(message) => Err(OpenAiCompatServerError::Config(message)),
        }
    }

    async fn generate(
        &self,
        request: GenerationRequest,
    ) -> Result<crate::GenerationResponse, CudaGptOssTextGenerationError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.sender
            .send(GptOssCudaWorkerCommand::Generate {
                request,
                reply: reply_tx,
            })
            .map_err(|_| CudaGptOssTextGenerationError::BackendUnavailable {
                status: psionic_runtime::HealthStatus::Offline,
                message: String::from("gpt-oss cuda worker is no longer available"),
            })?;
        reply_rx
            .await
            .map_err(|_| CudaGptOssTextGenerationError::BackendUnavailable {
                status: psionic_runtime::HealthStatus::Offline,
                message: String::from("gpt-oss cuda worker dropped the response channel"),
            })?
    }
}

#[derive(Debug, thiserror::Error)]
enum OpenAiCompatHttpError {
    #[error("{0}")]
    BadRequest(String),
    #[error(transparent)]
    PromptRender(#[from] PromptRenderError),
    #[error(transparent)]
    Generation(#[from] CudaGptOssTextGenerationError),
}

impl IntoResponse for OpenAiCompatHttpError {
    fn into_response(self) -> Response {
        let (status, kind) = match &self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
            Self::PromptRender(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
            Self::Generation(CudaGptOssTextGenerationError::BackendUnavailable { .. }) => {
                (StatusCode::SERVICE_UNAVAILABLE, "backend_unavailable")
            }
            Self::Generation(CudaGptOssTextGenerationError::Generation(error)) => (
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
    model: String,
}

async fn health(State(state): State<Arc<GptOssCudaOpenAiCompatState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        backend: "cuda",
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

async fn list_models(
    State(state): State<Arc<GptOssCudaOpenAiCompatState>>,
) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        data: vec![ModelCard {
            id: state.default_model_name.clone(),
            object: "model",
            owned_by: "psionic",
        }],
    })
}

#[derive(Clone, Debug, Deserialize)]
struct ChatCompletionRequest {
    #[serde(default)]
    model: Option<String>,
    messages: Vec<ChatCompletionMessage>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    max_tokens: Option<usize>,
    #[serde(default)]
    stop: Option<StopSequences>,
    #[serde(default)]
    stream: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct ChatCompletionMessage {
    role: String,
    content: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
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
    State(state): State<Arc<GptOssCudaOpenAiCompatState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    match handle_chat_completions(state, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn handle_chat_completions(
    state: Arc<GptOssCudaOpenAiCompatState>,
    request: ChatCompletionRequest,
) -> Result<Response, OpenAiCompatHttpError> {
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
    let generation_request = GenerationRequest::new_text(
        request_id.clone(),
        state.descriptor.clone(),
        None,
        rendered,
        generation_options_from_chat_request(&request),
    );

    let response = state.worker.generate(generation_request).await?;
    let parsed = parse_gpt_oss_harmony_tokens(
        response.output.tokens.as_slice(),
        GptOssHarmonyParseOptions {
            role_hint: Some(PromptMessageRole::Assistant),
            strict: false,
        },
    )
    .ok();
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
        return Ok(Sse::new(iter(events)).into_response());
    }

    let psionic_harmony = parsed;
    let full_choice = choice.into_full_choice();
    Ok(Json(ChatCompletionResponse {
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
        psionic_harmony,
        psionic_perf: response.metrics.gpt_oss_perf.clone(),
    })
    .into_response())
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
    let content = parsed
        .as_ref()
        .and_then(final_assistant_content)
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

fn next_request_id(state: &GptOssCudaOpenAiCompatState) -> String {
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
        ChatCompletionMessage, ChatCompletionRequest, HARMONY_CALL_STOP, HARMONY_RETURN_STOP,
        chat_messages_to_prompt_messages, ensure_harmony_stop_sequences, final_assistant_content,
        generation_options_from_chat_request,
    };
    use psionic_models::{
        GptOssHarmonyParseSource, GptOssHarmonyParsedOutput, PromptMessage, PromptMessageRole,
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
}
