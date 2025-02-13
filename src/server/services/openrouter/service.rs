use crate::server::services::gateway::types::GatewayMetadata;
use crate::server::services::gateway::Gateway;
use crate::server::services::openrouter::types::{GitHubIssueFiles, OpenRouterConfig};
use anyhow::{anyhow, Result};
use futures::StreamExt;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{pin::Pin, time::Duration};
use tokio_stream::Stream;
use tracing::{debug, error, info, warn};

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
        let client = ClientBuilder::new()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            api_key,
            config: OpenRouterConfig::default(),
        }
    }

    pub fn with_config(api_key: String, config: OpenRouterConfig) -> Self {
        Self {
            client: Client::new(),
            api_key,
            config,
        }
    }

    pub fn is_test_mode(&self) -> bool {
        self.config.test_mode
    }

    fn get_model(&self) -> String {
        // Use Gemini for structured output
        "google/gemini-2.0-flash-lite-preview-02-05:free".to_string()
    }

    fn prepare_messages(&self, prompt: &str) -> Vec<Value> {
        vec![serde_json::json!({
            "role": "user",
            "content": prompt
        })]
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

    async fn make_structured_request<T>(&self, prompt: &str) -> Result<reqwest::Response>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
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

    fn process_stream_chunk(chunk: &[u8]) -> Result<Option<String>> {
        if chunk.is_empty() {
            return Ok(None);
        }

        let chunk_str = String::from_utf8_lossy(chunk);
        if chunk_str == "[DONE]" {
            return Ok(None);
        }

        let value: Value = serde_json::from_str(&chunk_str)?;
        let content = value["choices"][0]["delta"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        if content.is_empty() {
            Ok(None)
        } else {
            Ok(Some(content))
        }
    }

    async fn make_request_with_retry(
        &self,
        prompt: &str,
        stream: bool,
    ) -> Result<reqwest::Response> {
        let mut last_error = None;

        for retry in 0..=MAX_RETRIES {
            if retry > 0 {
                debug!("Retrying OpenRouter request (attempt {})", retry);
                tokio::time::sleep(RETRY_DELAY).await;
            }

            match self.make_request(prompt, stream).await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    }
                    let error = response.text().await?;
                    last_error = Some(anyhow!("OpenRouter API error: {}", error));
                }
                Err(e) => {
                    last_error = Some(anyhow!("Request failed: {}", e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Request failed after retries")))
    }

    async fn make_structured_request_with_retry<T>(&self, prompt: &str) -> Result<reqwest::Response>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let mut last_error = None;

        for retry in 0..=MAX_RETRIES {
            if retry > 0 {
                info!("Retrying OpenRouter request (attempt {})", retry + 1);
                tokio::time::sleep(RETRY_DELAY).await;
            }

            match self.make_structured_request::<T>(prompt).await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    }
                    let error = response.text().await?;
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

    pub async fn analyze_issue(&self, content: &str) -> Result<GitHubIssueFiles> {
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
            .make_structured_request_with_retry::<GitHubIssueFiles>(&request_body.to_string())
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
}

#[async_trait::async_trait]
impl Gateway for OpenRouterService {
    fn metadata(&self) -> GatewayMetadata {
        GatewayMetadata {
            name: "OpenRouter".to_string(),
            openai_compatible: true,
            supported_features: vec!["chat".to_string(), "streaming".to_string()],
            default_model: "anthropic/claude-3.5-sonnet".to_string(),
            available_models: vec![
                "anthropic/claude-3.5-sonnet".to_string(),
                "deepseek/deepseek-chat".to_string(),
            ],
        }
    }

    async fn chat(&self, prompt: String, _use_reasoner: bool) -> Result<(String, Option<String>)> {
        // Return test response if in test mode
        if self.is_test_mode() {
            return Ok(("Test response".to_string(), None));
        }

        let response = self.make_request_with_retry(&prompt, false).await?;

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

        let response = self.make_request_with_retry(&prompt, true).await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            warn!("OpenRouter API error: {}", error);
            return Err(anyhow!("OpenRouter API error: {}", error));
        }

        let mut stream = response.bytes_stream();
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            let mut buffer = Vec::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        buffer.extend_from_slice(&chunk);

                        // Process complete messages
                        while let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
                            let message = buffer[..pos].to_vec();
                            buffer = buffer[pos + 2..].to_vec();

                            if let Ok(Some(content)) = Self::process_stream_chunk(&message) {
                                tx.send(Ok(content)).await.ok();
                            }
                        }
                    }
                    Err(e) => {
                        tx.send(Err(anyhow!("Stream error: {}", e))).await.ok();
                        break;
                    }
                }
            }
        });

        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
}
