//! Ollama backend (localhost:11434)

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

/// Ollama inference backend
pub struct OllamaBackend {
    base_url: String,
    client: Client,
}

impl OllamaBackend {
    /// Create a new Ollama backend
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
            std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self::new(base_url)
    }
}

#[async_trait]
impl InferenceBackend for OllamaBackend {
    fn id(&self) -> &str {
        "ollama"
    }

    async fn is_ready(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        self.client.get(&url).send().await.is_ok()
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(BackendError::Unavailable(
                "Ollama not responding".to_string(),
            ));
        }

        let tags: OllamaTagsResponse = response.json().await?;

        Ok(tags
            .models
            .into_iter()
            .map(|m| {
                ModelInfo::new(&m.name, &m.name, 4096) // Ollama doesn't expose context length
                    .with_description(m.details.family.unwrap_or_default())
            })
            .collect())
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/api/generate", self.base_url);

        let ollama_request = OllamaGenerateRequest {
            model: request.model.clone(),
            prompt: request.prompt.clone(),
            stream: false,
            options: Some(OllamaOptions {
                temperature: request.temperature,
                top_p: request.top_p,
                num_predict: request.max_tokens.map(|n| n as i32),
                stop: request.stop,
            }),
        };

        let response = self.client.post(&url).json(&ollama_request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Ollama error {}: {}",
                status, text
            )));
        }

        let ollama_response: OllamaGenerateResponse = response.json().await?;

        Ok(CompletionResponse {
            id: format!("ollama-{}", uuid::Uuid::new_v4()),
            model: ollama_response.model,
            text: ollama_response.response,
            finish_reason: if ollama_response.done {
                Some("stop".to_string())
            } else {
                None
            },
            usage: Some(UsageInfo {
                prompt_tokens: ollama_response.prompt_eval_count.unwrap_or(0) as usize,
                completion_tokens: ollama_response.eval_count.unwrap_or(0) as usize,
                total_tokens: (ollama_response.prompt_eval_count.unwrap_or(0)
                    + ollama_response.eval_count.unwrap_or(0))
                    as usize,
            }),
            extra: HashMap::new(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let url = format!("{}/api/generate", self.base_url);

        let ollama_request = OllamaGenerateRequest {
            model: request.model.clone(),
            prompt: request.prompt.clone(),
            stream: true,
            options: Some(OllamaOptions {
                temperature: request.temperature,
                top_p: request.top_p,
                num_predict: request.max_tokens.map(|n| n as i32),
                stop: request.stop,
            }),
        };

        let response = self.client.post(&url).json(&ollama_request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Ollama error {}: {}",
                status, text
            )));
        }

        let (tx, rx) = mpsc::channel(100);
        let model = request.model.clone();

        tokio::spawn(async move {
            use tokio_stream::StreamExt;

            let mut stream = response.bytes_stream();
            let mut buffer = Vec::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        buffer.extend_from_slice(&bytes);

                        // Try to parse complete JSON objects from buffer
                        while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                            let line: Vec<u8> = buffer.drain(..=pos).collect();
                            if let Ok(text) = std::str::from_utf8(&line) {
                                let text = text.trim();
                                if !text.is_empty() {
                                    if let Ok(ollama_chunk) =
                                        serde_json::from_str::<OllamaGenerateResponse>(text)
                                    {
                                        let chunk = StreamChunk {
                                            id: String::new(),
                                            model: model.clone(),
                                            delta: ollama_chunk.response,
                                            finish_reason: if ollama_chunk.done {
                                                Some("stop".to_string())
                                            } else {
                                                None
                                            },
                                            extra: HashMap::new(),
                                        };
                                        if tx.send(Ok(chunk)).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(BackendError::StreamError(e.to_string()))).await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }
}

// ============================================================================
// Ollama API Types
// ============================================================================

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    model: String,
    response: String,
    done: bool,
    #[serde(default)]
    prompt_eval_count: Option<i32>,
    #[serde(default)]
    eval_count: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
    #[serde(default)]
    details: OllamaModelDetails,
}

#[derive(Debug, Default, Deserialize)]
struct OllamaModelDetails {
    family: Option<String>,
}
