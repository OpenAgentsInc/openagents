/// HTTP client for Foundation Model API
use crate::error::{FMError, Result};
use crate::types::*;
use reqwest::Client;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio_stream::Stream;

const DEFAULT_BASE_URL: &str = "http://localhost:3030";
const DEFAULT_MODEL: &str = "gpt-4o-mini-2024-07-18";
const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Foundation Model API client
#[derive(Clone)]
pub struct FMClient {
    base_url: String,
    http_client: Client,
    default_model: String,
    ready: Arc<AtomicBool>,
}

impl FMClient {
    /// Create a new client with default settings
    ///
    /// The base URL can be configured via FM_BRIDGE_URL environment variable.
    /// Defaults to http://localhost:3030 if not set.
    pub fn new() -> Result<Self> {
        let base_url =
            std::env::var("FM_BRIDGE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        Self::with_base_url(base_url)
    }

    /// Create a new client with custom base URL
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()?;

        Ok(Self {
            base_url: base_url.into(),
            http_client,
            default_model: DEFAULT_MODEL.to_string(),
            ready: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Create a builder for more configuration options
    pub fn builder() -> FMClientBuilder {
        FMClientBuilder::new()
    }

    pub(crate) fn mark_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::SeqCst);
    }

    pub(crate) fn is_ready_flag(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// Complete a prompt (non-streaming)
    pub async fn complete(
        &self,
        prompt: impl Into<String>,
        options: Option<CompletionOptions>,
    ) -> Result<CompletionResponse> {
        let options = options.unwrap_or_default();
        let request = CompletionRequest {
            model: options.model.unwrap_or_else(|| self.default_model.clone()),
            messages: vec![crate::types::ChatMessage {
                role: "user".to_string(),
                content: prompt.into(),
            }],
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            top_p: options.top_p,
            stop: options.stop,
            stream: false,
        };

        let url = format!("{}/v1/chat/completions", self.base_url);

        let response = self.http_client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(FMError::ApiError {
                status,
                message: text,
            });
        }

        let completion = response.json::<CompletionResponse>().await?;
        Ok(completion)
    }

    /// Stream a completion
    pub async fn stream(
        &self,
        prompt: impl Into<String>,
        options: Option<CompletionOptions>,
    ) -> Result<impl Stream<Item = Result<StreamChunk>>> {
        let options = options.unwrap_or_default();
        let request = CompletionRequest {
            model: options.model.unwrap_or_else(|| self.default_model.clone()),
            messages: vec![crate::types::ChatMessage {
                role: "user".to_string(),
                content: prompt.into(),
            }],
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            top_p: options.top_p,
            stop: options.stop,
            stream: true,
        };

        let url = format!("{}/v1/chat/completions", self.base_url);

        let response = self.http_client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(FMError::ApiError {
                status,
                message: text,
            });
        }

        // Parse SSE stream
        let stream = eventsource_stream::EventStream::new(response.bytes_stream());

        use tokio_stream::StreamExt;
        let chunk_stream = stream.map(|result| match result {
            Ok(event) => {
                // Check for [DONE] sentinel
                if event.data == "[DONE]" {
                    return Ok(StreamChunk {
                        text: String::new(),
                        finish_reason: Some(FinishReason::Stop),
                    });
                }

                // Parse JSON
                match serde_json::from_str::<StreamResponse>(&event.data) {
                    Ok(response) => response
                        .into_chunk()
                        .ok_or_else(|| FMError::StreamError("Empty choices".to_string())),
                    Err(e) => Err(FMError::JsonError(e)),
                }
            }
            Err(e) => Err(FMError::StreamError(e.to_string())),
        });

        Ok(chunk_stream)
    }

    /// List available models
    pub async fn models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/v1/models", self.base_url);

        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(FMError::ApiError {
                status,
                message: text,
            });
        }

        let models_response = response.json::<ModelsResponse>().await?;
        Ok(models_response.data)
    }

    /// Health check
    pub async fn health(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);

        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            return Ok(false);
        }

        let health = response.json::<HealthResponse>().await?;
        Ok(health.status == "ok" || health.status == "healthy")
    }
}

impl Default for FMClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default FMClient")
    }
}

/// Builder for FMClient
pub struct FMClientBuilder {
    base_url: String,
    default_model: String,
    timeout: Duration,
}

impl FMClientBuilder {
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            default_model: DEFAULT_MODEL.to_string(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> Result<FMClient> {
        let http_client = Client::builder().timeout(self.timeout).build()?;

        Ok(FMClient {
            base_url: self.base_url,
            http_client,
            default_model: self.default_model,
            ready: Arc::new(AtomicBool::new(false)),
        })
    }
}

impl Default for FMClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
