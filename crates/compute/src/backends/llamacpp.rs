//! Llama.cpp / GPT-OSS backend (localhost:8080)

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

use super::{
    BackendError, CompletionRequest, CompletionResponse, InferenceBackend, ModelInfo, Result,
    StreamChunk, UsageInfo,
};

const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Llama.cpp / GPT-OSS inference backend
pub struct LlamaCppBackend {
    base_url: String,
    client: Client,
}

impl LlamaCppBackend {
    /// Create a new Llama.cpp backend
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| BackendError::InitializationError(e.to_string()))?;

        Ok(Self {
            base_url: base_url.into(),
            client,
        })
    }

    /// Create from environment variable or default
    pub fn from_env() -> Result<Self> {
        let base_url =
            std::env::var("LLAMACPP_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
        Self::new(base_url)
    }
}

#[async_trait]
impl InferenceBackend for LlamaCppBackend {
    fn id(&self) -> &str {
        "llamacpp"
    }

    async fn is_ready(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => {
                let status_ok = response.status().is_success();
                if let Ok(health) = response.json::<HealthResponse>().await {
                    health.status == "ok" || health.status == "healthy"
                } else {
                    // Some llama.cpp servers just return 200 OK
                    status_ok
                }
            }
            Err(_) => false,
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/v1/models", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(BackendError::Unavailable(
                "Llama.cpp server not responding".to_string(),
            ));
        }

        let value: serde_json::Value = response.json().await?;
        let models = parse_models_response(value)?;

        Ok(models
            .into_iter()
            .map(|m| {
                ModelInfo::new(&m.id, &m.name, m.context_length)
                    .with_description(m.description.unwrap_or_default())
            })
            .collect())
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/v1/completions", self.base_url);

        let llama_request = LlamaCompletionRequest {
            model: request.model.clone(),
            prompt: request.prompt.clone(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop: request.stop,
            stream: false,
        };

        let response = self.client.post(&url).json(&llama_request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Llama.cpp error {}: {}",
                status, text
            )));
        }

        let llama_response: LlamaCompletionResponse = response.json().await?;

        Ok(CompletionResponse {
            id: llama_response.id,
            model: llama_response.model,
            text: llama_response.text,
            finish_reason: llama_response.finish_reason,
            usage: llama_response.usage.map(|u| UsageInfo {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            extra: HashMap::new(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let url = format!("{}/v1/completions", self.base_url);

        let llama_request = LlamaCompletionRequest {
            model: request.model.clone(),
            prompt: request.prompt.clone(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop: request.stop,
            stream: true,
        };

        let response = self.client.post(&url).json(&llama_request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Llama.cpp error {}: {}",
                status, text
            )));
        }

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            use eventsource_stream::Eventsource;
            use tokio_stream::StreamExt;

            let stream = response.bytes_stream().eventsource();
            let mut stream = Box::pin(stream);

            while let Some(event_result) = stream.next().await {
                match event_result {
                    Ok(event) => {
                        if event.data == "[DONE]" {
                            let chunk = StreamChunk {
                                id: String::new(),
                                model: String::new(),
                                delta: String::new(),
                                finish_reason: Some("stop".to_string()),
                                extra: HashMap::new(),
                            };
                            let _ = tx.send(Ok(chunk)).await;
                            break;
                        }

                        if let Ok(stream_chunk) =
                            serde_json::from_str::<LlamaStreamChunk>(&event.data)
                        {
                            let chunk = StreamChunk {
                                id: stream_chunk.id,
                                model: stream_chunk.model,
                                delta: stream_chunk.delta,
                                finish_reason: stream_chunk.finish_reason,
                                extra: HashMap::new(),
                            };

                            if tx.send(Ok(chunk)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(BackendError::StreamError(e.to_string()))).await;
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }
}

fn parse_models_response(value: serde_json::Value) -> Result<Vec<LlamaModelInfo>> {
    // Handle both array and {data: [...]} formats
    if value.is_array() {
        return serde_json::from_value(value).map_err(BackendError::from);
    }

    let data = value
        .get("data")
        .cloned()
        .ok_or_else(|| BackendError::InvalidRequest("Models response missing data".to_string()))?;

    serde_json::from_value(data).map_err(BackendError::from)
}

// ============================================================================
// Llama.cpp API Types
// ============================================================================

#[derive(Debug, Serialize)]
struct LlamaCompletionRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct LlamaCompletionResponse {
    id: String,
    model: String,
    text: String,
    finish_reason: Option<String>,
    usage: Option<LlamaUsage>,
}

#[derive(Debug, Deserialize)]
struct LlamaUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[derive(Debug, Deserialize)]
struct LlamaStreamChunk {
    #[serde(default)]
    id: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    delta: String,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LlamaModelInfo {
    id: String,
    #[serde(default)]
    name: String,
    description: Option<String>,
    #[serde(default = "default_context_length")]
    context_length: usize,
}

fn default_context_length() -> usize {
    8192
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
}
