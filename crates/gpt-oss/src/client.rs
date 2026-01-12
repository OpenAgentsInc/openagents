/// HTTP client for GPT-OSS Responses API
use crate::error::{GptOssError, Result};
use crate::types::*;
use reqwest::Client;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio_stream::Stream;

const DEFAULT_BASE_URL: &str = "http://localhost:8000";
const DEFAULT_MODEL: &str = "gpt-oss-20b";
const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// GPT-OSS Responses API client
#[derive(Clone)]
pub struct GptOssClient {
    base_url: String,
    http_client: Client,
    default_model: String,
    initialized: Arc<AtomicBool>,
}

impl GptOssClient {
    /// Create a new client with default settings
    ///
    /// The base URL can be configured via GPT_OSS_URL environment variable.
    /// Defaults to http://localhost:8000 if not set.
    pub fn new() -> Result<Self> {
        let base_url =
            std::env::var("GPT_OSS_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
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
            initialized: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Create a builder for more configuration options
    pub fn builder() -> GptOssClientBuilder {
        GptOssClientBuilder::new()
    }

    /// Complete a prompt (non-streaming)
    pub async fn complete(&self, request: GptOssRequest) -> Result<GptOssResponse> {
        let url = format!("{}/v1/completions", self.base_url);

        let response = self.http_client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(GptOssError::ApiError {
                status,
                message: text,
            });
        }

        let value = response.json::<serde_json::Value>().await?;
        let completion = parse_completion_response(value)?;
        Ok(completion)
    }

    /// Complete with simple prompt using default model
    pub async fn complete_simple(&self, model: &str, prompt: &str) -> Result<String> {
        let request = GptOssRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
        };

        let response = self.complete(request).await?;
        Ok(response.text)
    }

    /// Call the Responses API (tool use, reasoning effort, rich outputs)
    pub async fn responses(
        &self,
        request: GptOssResponsesRequest,
    ) -> Result<GptOssResponsesResponse> {
        let url = format!("{}/v1/responses", self.base_url);

        let response = self.http_client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(GptOssError::ApiError {
                status,
                message: text,
            });
        }

        let output = response.json::<GptOssResponsesResponse>().await?;
        Ok(output)
    }

    /// Convenience wrapper around `responses` with string input
    pub async fn responses_simple(
        &self,
        model: &str,
        input: &str,
    ) -> Result<GptOssResponsesResponse> {
        let request = GptOssResponsesRequest::new(model, input);
        self.responses(request).await
    }

    /// Stream a completion
    pub async fn stream(
        &self,
        request: GptOssRequest,
    ) -> Result<impl Stream<Item = Result<GptOssStreamChunk>>> {
        let url = format!("{}/v1/completions", self.base_url);

        let mut stream_request = request;
        stream_request.stream = true;

        let response = self
            .http_client
            .post(&url)
            .json(&stream_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(GptOssError::ApiError {
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
                    return Ok(GptOssStreamChunk {
                        id: String::new(),
                        model: String::new(),
                        choices: vec![crate::types::CompletionChoice {
                            index: 0,
                            text: String::new(),
                            finish_reason: Some("stop".to_string()),
                        }],
                    });
                }

                // Parse JSON
                match serde_json::from_str::<GptOssStreamChunk>(&event.data) {
                    Ok(chunk) => Ok(chunk),
                    Err(e) => Err(GptOssError::JsonError(e)),
                }
            }
            Err(e) => Err(GptOssError::StreamError(e.to_string())),
        });

        Ok(chunk_stream)
    }

    /// List available models
    pub async fn models(&self) -> Result<Vec<GptOssModelInfo>> {
        let url = format!("{}/v1/models", self.base_url);

        let response = self.http_client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(GptOssError::ApiError {
                status,
                message: text,
            });
        }

        let value = response.json::<serde_json::Value>().await?;
        let models = parse_models_response(value)?;
        Ok(models)
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

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get the default model
    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    pub(crate) fn set_initialized(&self, ready: bool) {
        self.initialized.store(ready, Ordering::SeqCst);
    }

    pub(crate) fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }
}

fn parse_models_response(value: serde_json::Value) -> Result<Vec<GptOssModelInfo>> {
    if value.is_array() {
        return serde_json::from_value(value).map_err(GptOssError::JsonError);
    }

    let data = value.get("data").cloned().ok_or_else(|| {
        GptOssError::InvalidRequest("Models response missing data array".to_string())
    })?;

    serde_json::from_value(data).map_err(GptOssError::JsonError)
}

fn parse_completion_response(value: serde_json::Value) -> Result<GptOssResponse> {
    if let Ok(response) = serde_json::from_value::<GptOssResponse>(value.clone()) {
        return Ok(response);
    }

    let choices = value
        .get("choices")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            GptOssError::InvalidRequest("Completion response missing choices".to_string())
        })?;
    let first_choice = choices.first().ok_or_else(|| {
        GptOssError::InvalidRequest("Completion response missing first choice".to_string())
    })?;
    let text = first_choice
        .get("text")
        .and_then(|v| v.as_str())
        .or_else(|| {
            first_choice
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("")
        .to_string();

    let finish_reason = first_choice
        .get("finish_reason")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());

    let usage = value.get("usage").and_then(parse_usage_stats).or_else(|| {
        value
            .get("usage")
            .and_then(|usage| serde_json::from_value::<UsageStats>(usage.clone()).ok())
    });

    let response = GptOssResponse {
        id: value
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        model: value
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        text,
        finish_reason,
        usage,
    };

    Ok(response)
}

fn parse_usage_stats(value: &serde_json::Value) -> Option<UsageStats> {
    let prompt_tokens = value.get("prompt_tokens")?.as_u64()? as usize;
    let completion_tokens = value.get("completion_tokens")?.as_u64()? as usize;
    let total_tokens = value.get("total_tokens")?.as_u64()? as usize;

    Some(UsageStats {
        prompt_tokens,
        completion_tokens,
        total_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_models_response_array() {
        let value = serde_json::json!([
            {"id": "gpt-oss-20b", "name": "GPT-OSS 20B", "context_length": 8192},
            {"id": "gpt-oss-120b", "name": "GPT-OSS 120B", "context_length": 8192}
        ]);

        let models = parse_models_response(value).expect("Should parse array response");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-oss-20b");
    }

    #[test]
    fn test_parse_models_response_data_wrapper() {
        let value = serde_json::json!({
            "data": [
                {"id": "gpt-oss-20b", "name": "GPT-OSS 20B", "context_length": 8192}
            ]
        });

        let models = parse_models_response(value).expect("Should parse data wrapper response");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gpt-oss-20b");
    }
}

impl Default for GptOssClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default GptOssClient")
    }
}

/// Builder for GptOssClient
pub struct GptOssClientBuilder {
    base_url: String,
    default_model: String,
    timeout: Duration,
}

impl GptOssClientBuilder {
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

    pub fn build(self) -> Result<GptOssClient> {
        let http_client = Client::builder().timeout(self.timeout).build()?;

        Ok(GptOssClient {
            base_url: self.base_url,
            http_client,
            default_model: self.default_model,
            initialized: Arc::new(AtomicBool::new(false)),
        })
    }
}

impl Default for GptOssClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
