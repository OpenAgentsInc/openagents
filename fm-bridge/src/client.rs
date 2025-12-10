use crate::error::*;
use crate::types::*;
use reqwest::Client;
use std::time::Duration;

pub struct FMClient {
    base_url: String,
    http_client: Client,
    default_model: String,
}

impl FMClient {
    /// Create a new client with default settings
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a client builder
    pub fn builder() -> FMClientBuilder {
        FMClientBuilder::new()
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get the HTTP client
    pub fn http_client(&self) -> &Client {
        &self.http_client
    }

    /// Create a session client
    pub fn sessions(&self) -> crate::sessions::SessionClient {
        crate::sessions::SessionClient::new(&self.base_url, self.http_client.clone())
    }

    /// Create a tools client
    pub fn tools(&self) -> crate::tools::ToolClient {
        crate::tools::ToolClient::new(&self.base_url, self.http_client.clone())
    }

    /// Check health status
    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.base_url);
        let response = self
            .http_client
            .get(&url)
            .send()
            .await?
            .json::<HealthResponse>()
            .await?;

        Ok(response)
    }

    /// List available models
    pub async fn models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/v1/models", self.base_url);
        let response = self
            .http_client
            .get(&url)
            .send()
            .await?
            .json::<ModelsResponse>()
            .await?;

        Ok(response.data)
    }

    /// Complete a prompt (non-streaming)
    pub async fn complete(
        &self,
        prompt: impl Into<String>,
        options: Option<CompletionOptions>,
    ) -> Result<CompletionResponse> {
        let options = options.unwrap_or_default();

        let request = CompletionRequest {
            model: options.model.or(Some(self.default_model.clone())),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.into(),
            }],
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            stream: Some(false),
            response_format: None,
        };

        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let completion_response = response.json::<CompletionResponse>().await?;
        Ok(completion_response)
    }

    /// Complete with full chat messages
    pub async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        options: Option<CompletionOptions>,
    ) -> Result<CompletionResponse> {
        let options = options.unwrap_or_default();

        let request = CompletionRequest {
            model: options.model.or(Some(self.default_model.clone())),
            messages,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            stream: Some(false),
            response_format: None,
        };

        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let completion_response = response.json::<CompletionResponse>().await?;
        Ok(completion_response)
    }
}

impl Default for FMClient {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FMClientBuilder {
    base_url: Option<String>,
    timeout: Option<Duration>,
    default_model: Option<String>,
}

impl FMClientBuilder {
    pub fn new() -> Self {
        Self {
            base_url: None,
            timeout: None,
            default_model: None,
        }
    }

    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    pub fn build(self) -> FMClient {
        let base_url = self
            .base_url
            .unwrap_or_else(|| "http://localhost:11435".to_string());
        let timeout = self.timeout.unwrap_or(Duration::from_secs(300));
        let default_model = self
            .default_model
            .unwrap_or_else(|| "apple-foundation-model".to_string());

        let http_client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        FMClient {
            base_url,
            http_client,
            default_model,
        }
    }
}

impl Default for FMClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
