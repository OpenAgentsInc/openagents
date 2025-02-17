use crate::server::services::gateway::types::GatewayMetadata;
use crate::server::services::gateway::Gateway;
use crate::server::services::openrouter::types::{
    CodeChanges, GitHubIssueFiles, OpenRouterConfig, FREE_MODELS,
};
use anyhow::{anyhow, Result};
use futures::future;
use futures::StreamExt;
use reqwest::{Client, ClientBuilder, Response};
use serde_json::{json, Value};
use std::{pin::Pin, time::Duration};
use tokio_stream::Stream;
use tracing::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use bytes::Bytes;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_RETRIES: u32 = 2;
const RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
pub struct OpenRouterService {
    client: Client,
    api_key: String,
    config: OpenRouterConfig,
}

impl OpenRouterService {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            config: OpenRouterConfig::default(),
            client: ClientBuilder::new()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub fn with_config(api_key: String, config: OpenRouterConfig) -> Self {
        Self {
            api_key,
            config,
            client: ClientBuilder::new()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub fn is_test_mode(&self) -> bool {
        self.config.test_mode
    }

    fn get_model(&self) -> String {
        self.config.model.clone()
    }

    fn get_next_available_model(&self) -> String {
        for model in FREE_MODELS.iter() {
            if !self.config.rate_limited_models.contains(*model) {
                return model.to_string();
            }
        }
        // If all models are rate limited, use the first one
        FREE_MODELS[0].to_string()
    }

    fn mark_model_rate_limited(&mut self, model: &str) {
        info!("Marking model {} as rate limited", model);
        self.config.rate_limited_models.insert(model.to_string());

        // Schedule removal of rate limit after 1 hour
        let model = model.to_string();
        let mut rate_limited_models = self.config.rate_limited_models.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            rate_limited_models.remove(&model);
            info!("Removed rate limit for model {}", model);
        });
    }

    fn prepare_messages(&self, prompt: &str) -> Value {
        json!([{
            "role": "user",
            "content": prompt
        }])
    }

    async fn make_request(&self, prompt: &str, stream: bool) -> Result<reqwest::Response> {
        let model = self.get_model();
        debug!("Using model: {}", model);

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header(
                "HTTP-Referer",
                "https://github.com/OpenAgentsInc/openagents",
            )
            .json(&serde_json::json!({
                "model": model,
                "messages": self.prepare_messages(prompt),
                "stream": stream
            }))
            .send()
            .await?;

        Ok(response)
    }

    async fn make_structured_request(&self, prompt: &str) -> Result<reqwest::Response> {
        let model = self.get_model();
        info!("Making OpenRouter request to model: {}", model);

        let request_body = serde_json::json!({
            "model": model,
            "messages": self.prepare_messages(prompt),
            "stream": false,
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "github_issue_files",
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": {
                            "files": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "filepath": {
                                            "type": "string",
                                            "description": "Path to the relevant file"
                                        },
                                        "comment": {
                                            "type": "string",
                                            "description": "Why this file is relevant to the issue"
                                        },
                                        "priority": {
                                            "type": "integer",
                                            "minimum": 1,
                                            "maximum": 10,
                                            "description": "Priority of this file from 1 (low) to 10 (high)"
                                        }
                                    },
                                    "required": ["filepath", "comment", "priority"],
                                    "additionalProperties": false
                                }
                            }
                        },
                        "required": ["files"],
                        "additionalProperties": false
                    }
                }
            }
        });

        info!("Sending request to OpenRouter API...");
        debug!(
            "Request body: {}",
            serde_json::to_string_pretty(&request_body)?
        );

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header(
                "HTTP-Referer",
                "https://github.com/OpenAgentsInc/openagents",
            )
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        info!("OpenRouter API response status: {}", status);

        Ok(response)
    }

    async fn process_stream_chunk(chunk: &[u8]) -> Result<Option<String>> {
        if chunk.is_empty() {
            return Ok(None);
        }

        let chunk_str = String::from_utf8_lossy(chunk);
        debug!("Processing chunk: {}", chunk_str);

        if chunk_str == "[DONE]" {
            debug!("Received [DONE] message");
            return Ok(None);
        }

        // Check if it's a keep-alive message
        if chunk_str.starts_with(": OPENROUTER PROCESSING") {
            return Ok(None);
        }

        // Extract just the content from the JSON
        if let Some(data) = chunk_str.strip_prefix("data: ") {
            if let Ok(value) = serde_json::from_str::<Value>(data) {
                if let Some(content) = value["choices"][0]["delta"]["content"].as_str() {
                    if !content.is_empty() {
                        debug!("Extracted content token: {}", content);
                        return Ok(Some(content.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn make_structured_request_with_retry(
        &mut self,
        prompt: &str,
    ) -> Result<reqwest::Response> {
        let mut last_error = None;

        for retry in 0..=MAX_RETRIES {
            if retry > 0 {
                info!("Retrying OpenRouter request (attempt {})", retry + 1);
                tokio::time::sleep(RETRY_DELAY).await;
            }

            let current_model = self.get_model();
            match self.make_structured_request(prompt).await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    }
                    let error = response.text().await?;

                    // Check for rate limit error
                    if error.contains("Rate limit exceeded") {
                        self.mark_model_rate_limited(&current_model);
                        let next_model = self.get_next_available_model();
                        info!("Switching to model: {}", next_model);
                        self.config.model = next_model;
                        continue;
                    }

                    error!("OpenRouter API error response: {}", error);
                    last_error = Some(anyhow!("OpenRouter API error: {}", error));
                }
                Err(e) => {
                    error!("OpenRouter request failed: {}", e);
                    last_error = Some(anyhow!("Request failed: {}", e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Request failed after retries")))
    }

    pub async fn analyze_issue(&mut self, content: &str) -> Result<GitHubIssueFiles> {
        let json_schema = json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "filepath": {
                                "type": "string",
                                "description": "Path to the relevant file"
                            },
                            "comment": {
                                "type": "string",
                                "description": "Why this file is relevant to the issue"
                            },
                            "priority": {
                                "type": "integer",
                                "minimum": 1,
                                "maximum": 10,
                                "description": "Priority of this file from 1 (low) to 10 (high)"
                            }
                        },
                        "required": ["filepath", "comment", "priority"]
                    }
                }
            },
            "required": ["files"]
        });

        let request_body = json!({
            "model": self.get_model(),
            "messages": [{
                "role": "user",
                "content": format!(
                    "Analyze this GitHub issue and list the most relevant files that would need to be understood or modified to solve it. For each file, explain why it's relevant and rate its priority from 1-10. Return the response in the exact JSON schema format specified, with no markdown formatting or additional text.\n\nIssue content:\n{}",
                    content
                )
            }],
            "response_format": {
                "type": "json_schema",
                "schema": json_schema
            }
        });

        let response = self
            .make_structured_request_with_retry(&request_body.to_string())
            .await?;
        info!("Raw OpenRouter response: {:?}", response);

        let response_text = response.text().await?;
        info!("Response text: {}", response_text);

        // Try to parse the raw response first
        if let Ok(files) = serde_json::from_str::<GitHubIssueFiles>(&response_text) {
            info!("Successfully parsed raw response");
            return Ok(files);
        }

        // If raw parsing fails, try to extract from OpenRouter response structure
        if let Ok(router_response) = serde_json::from_str::<Value>(&response_text) {
            if let Some(content) = router_response["choices"][0]["message"]["content"].as_str() {
                if let Ok(files) = serde_json::from_str::<GitHubIssueFiles>(content) {
                    info!("Successfully parsed nested response");
                    return Ok(files);
                }
            }
        }

        error!("Failed to parse response in any format");
        error!("Response content: {}", response_text);
        Err(anyhow!(
            "Failed to parse OpenRouter response into GitHubIssueFiles"
        ))
    }

    pub async fn analyze_issue_with_schema(
        &mut self,
        _prompt: &str,
        request_body: serde_json::Value,
    ) -> Result<CodeChanges> {
        info!("Making OpenRouter request with schema");
        debug!(
            "Request body: {}",
            serde_json::to_string_pretty(&request_body)?
        );

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header(
                "HTTP-Referer",
                "https://github.com/OpenAgentsInc/openagents",
            )
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        info!("OpenRouter API response status: {}", status);

        if !status.is_success() {
            let error = response.text().await?;
            error!("OpenRouter API error response: {}", error);
            return Err(anyhow!("OpenRouter API error: {}", error));
        }

        let response_text = response.text().await?;
        info!("Response text: {}", response_text);

        // Try to parse the response content from the OpenRouter response structure
        if let Ok(router_response) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(content) = router_response["choices"][0]["message"]["content"].as_str() {
                if let Ok(changes) = serde_json::from_str::<CodeChanges>(content) {
                    info!("Successfully parsed response");
                    return Ok(changes);
                }
            }
        }

        error!("Failed to parse response");
        error!("Response content: {}", response_text);
        Err(anyhow!("Failed to parse OpenRouter response"))
    }
}

#[async_trait::async_trait]
impl Gateway for OpenRouterService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "OpenRouter".to_string(),
            openai_compatible: true,
            supported_features: vec!["chat".to_string(), "streaming".to_string()],
            default_model: "google/gemini-2.0-flash-001:free".to_string(),
            available_models: vec![
                "google/gemini-2.0-flash-001:free".to_string(),
                "deepseek/deepseek-coder-33b-instruct:free".to_string(),
            ],
        }
    }

    async fn chat(&self, prompt: String, _use_reasoner: bool) -> Result<(String, Option<String>)> {
        // Return test response if in test mode
        if self.is_test_mode() {
            return Ok(("Test response".to_string(), None));
        }

        let response = self.make_request(&prompt, false).await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            warn!("OpenRouter API error: {}", error);
            return Err(anyhow!("OpenRouter API error: {}", error));
        }

        let json: Value = response.json().await?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid response format"))?
            .to_string();

        Ok((content, None))
    }

    async fn chat_stream(
        &self,
        prompt: String,
        _use_reasoner: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        // Return test stream if in test mode
        if self.is_test_mode() {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                tx.send(Ok("Test response".to_string())).await.ok();
            });
            return Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)));
        }

        let response = self.make_request(&prompt, true).await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            warn!("OpenRouter API error: {}", error);
            return Err(anyhow!("OpenRouter API error: {}", error));
        }

        let stream = response
            .bytes_stream()
            .then(move |result| async move {
                match result {
                    Ok(bytes) => {
                        match Self::process_stream_chunk(&bytes).await {
                            Ok(Some(content)) => Ok(content),
                            Ok(None) => Ok(String::new()), // Return empty string for keep-alive messages
                            Err(e) => Err(anyhow!("Error processing chunk: {}", e)),
                        }
                    }
                    Err(e) => Err(anyhow!("Error reading stream: {}", e)),
                }
            })
            .filter(|result| {
                future::ready(match result {
                    Ok(content) => !content.is_empty(), // Filter out empty strings (keep-alive messages)
                    Err(_) => true,                     // Keep errors in the stream
                })
            });

        Ok(Box::pin(stream))
    }
}
