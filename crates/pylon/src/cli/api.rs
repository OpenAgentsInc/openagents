//! pylon api - Local HTTP API for completions

use axum::extract::State;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use clap::Args;
use compute::backends::{BackendRegistry, CompletionRequest, StreamChunk};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
#[cfg(feature = "gpt-oss-gguf")]
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tower_http::cors::{Any, CorsLayer};

#[cfg(feature = "gpt-oss-gguf")]
use ml::GptOssGgufBackend;

#[derive(Args)]
pub struct ApiArgs {
    /// Bind address
    #[arg(long, default_value = "127.0.0.1:9899")]
    pub bind: String,
}

pub async fn run(args: ApiArgs) -> anyhow::Result<()> {
    #[allow(unused_mut)]
    let mut registry = BackendRegistry::detect().await;

    #[cfg(feature = "gpt-oss-gguf")]
    if let Ok(backend) = GptOssGgufBackend::from_env() {
        registry.register_with_id(
            "gpt-oss-gguf",
            Arc::new(RwLock::new(backend)),
        );
    }

    let state = ApiState {
        registry: Arc::new(registry),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = axum::Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .route("/v1/completions", post(completions))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
struct ApiState {
    registry: Arc<BackendRegistry>,
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

#[derive(Serialize)]
struct ModelList {
    data: Vec<ModelEntry>,
}

#[derive(Serialize)]
struct ModelEntry {
    id: String,
    object: &'static str,
    owned_by: &'static str,
    context_length: usize,
}

async fn list_models(State(state): State<ApiState>) -> impl IntoResponse {
    let models = state.registry.list_all_models().await;
    let data = models
        .into_iter()
        .map(|(_, info)| ModelEntry {
            id: info.id,
            object: "model",
            owned_by: "local",
            context_length: info.context_length,
        })
        .collect::<Vec<_>>();
    Json(ModelList { data })
}

#[derive(Deserialize)]
struct CompletionRequestIn {
    model: String,
    prompt: String,
    max_tokens: Option<usize>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    stop: Option<Vec<String>>,
    stream: Option<bool>,
    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[derive(Serialize)]
struct CompletionResponseOut {
    id: String,
    object: &'static str,
    model: String,
    choices: Vec<CompletionChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<CompletionUsage>,
}

#[derive(Serialize)]
struct CompletionChoice {
    text: String,
    index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Serialize)]
struct CompletionUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

async fn completions(
    State(state): State<ApiState>,
    Json(payload): Json<CompletionRequestIn>,
) -> Result<Response, ApiError> {
    let (backend_id, model_id) = find_backend(&state.registry, &payload.model).await?;
    let backend = state
        .registry
        .get(&backend_id)
        .ok_or_else(|| ApiError::not_found("backend not available"))?;

    let mut request = CompletionRequest::new(model_id, payload.prompt);
    request.max_tokens = payload.max_tokens;
    request.temperature = payload.temperature;
    request.top_p = payload.top_p;
    request.stop = payload.stop;
    request.stream = payload.stream.unwrap_or(false);
    request.extra = payload.extra;

    if request.stream {
        let rx = backend
            .read()
            .await
            .complete_stream(request)
            .await
            .map_err(ApiError::from_backend)?;
        let stream = ReceiverStream::new(rx).map(|chunk| {
            let event = match chunk {
                Ok(chunk) => render_stream_event(chunk),
                Err(err) => {
                    let body = serde_json::json!({
                        "error": err.to_string(),
                    });
                    Event::default().data(body.to_string())
                }
            };
            Ok::<Event, Infallible>(event)
        });
        let sse = Sse::new(stream)
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)));
        Ok(sse.into_response())
    } else {
        let response = backend
            .read()
            .await
            .complete(request)
            .await
            .map_err(ApiError::from_backend)?;
        let body = CompletionResponseOut {
            id: response.id,
            object: "text_completion",
            model: response.model,
            choices: vec![CompletionChoice {
                text: response.text,
                index: 0,
                finish_reason: response.finish_reason,
            }],
            usage: response.usage.map(|usage| CompletionUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            }),
        };
        Ok(Json(body).into_response())
    }
}

fn render_stream_event(chunk: StreamChunk) -> Event {
    if let Some(reason) = &chunk.finish_reason {
        if !reason.is_empty() {
            return Event::default().data("[DONE]");
        }
    }

    let mut body = serde_json::json!({
        "id": chunk.id,
        "object": "text_completion",
        "model": chunk.model,
        "choices": [{
            "text": chunk.delta,
            "index": 0,
            "finish_reason": chunk.finish_reason,
        }]
    });
    if !chunk.extra.is_empty() {
        let extra = serde_json::Value::Object(chunk.extra.into_iter().collect());
        body["extra"] = extra;
    }
    Event::default().data(serde_json::to_string(&body).unwrap_or_default())
}

async fn find_backend(
    registry: &BackendRegistry,
    model: &str,
) -> Result<(String, String), ApiError> {
    let models = registry.list_all_models().await;
    if let Some((backend_id, info)) = models.into_iter().find(|(_, info)| info.id == model) {
        return Ok((backend_id, info.id));
    }
    Err(ApiError::not_found("model not found"))
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn not_found(message: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.to_string(),
        }
    }

    fn from_backend(err: compute::backends::BackendError) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: err.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = Json(serde_json::json!({
            "error": self.message,
        }))
        .into_response();
        *response.status_mut() = self.status;
        response.headers_mut().insert(
            axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
        response
    }
}
