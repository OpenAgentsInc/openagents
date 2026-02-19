//! Apple Foundation Models backend (localhost:11435)
//!
//! Enhanced with session management, tool registration, and FRLM integration via FM Bridge.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};

use super::{
    BackendError, CompletionRequest, CompletionResponse, InferenceBackend, ModelInfo, Result,
    StreamChunk, UsageInfo,
};
use crate::frlm_tool_handler::FrlmToolHandler;

const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Apple Foundation Models inference backend
///
/// Supports:
/// - Standard completions via `/v1/chat/completions`
/// - Session-based multi-turn conversations via `/v1/sessions`
/// - Tool registration via `/v1/sessions/{id}/tools`
/// - FRLM tool execution for recursive LLM calls
/// - SSE streaming for real-time token delivery
pub struct AppleFmBackend {
    base_url: String,
    client: Client,
    /// FRLM tool handler for executing recursive LLM tools.
    frlm_handler: Arc<Mutex<FrlmToolHandler>>,
}

/// Session information returned from FM Bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmSession {
    pub id: String,
    pub created: String,
}

/// Transcript message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmTranscriptMessage {
    pub role: String,
    pub content: String,
}

/// Tool definition for registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

impl AppleFmBackend {
    /// Create a new Apple FM backend
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| BackendError::InitializationError(e.to_string()))?;

        Ok(Self {
            base_url: base_url.into(),
            client,
            frlm_handler: Arc::new(Mutex::new(FrlmToolHandler::new())),
        })
    }

    /// Create from environment variable or default
    pub fn from_env() -> Result<Self> {
        let base_url =
            std::env::var("FM_BRIDGE_URL").unwrap_or_else(|_| "http://localhost:11435".to_string());
        Self::new(base_url)
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get FRLM tool definitions for registration with FM Bridge.
    pub fn get_frlm_tools(&self) -> Vec<FmToolDefinition> {
        crate::frlm_tools::create_frlm_tools()
    }

    /// Execute an FRLM tool by name.
    ///
    /// This is called by the Swift FM Bridge to execute tools locally via FRLM.
    pub async fn execute_frlm_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> std::result::Result<Value, BackendError> {
        let handler = self.frlm_handler.lock().await;
        handler
            .execute(name, arguments)
            .await
            .map_err(|e| BackendError::InferenceError(e.to_string()))
    }

    // ========================================================================
    // Session Management
    // ========================================================================

    /// Create a new session for multi-turn conversations
    pub async fn create_session(&self) -> Result<FmSession> {
        let url = format!("{}/v1/sessions", self.base_url);

        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Failed to create session: {} {}",
                status, text
            )));
        }

        let session: FmSession = response.json().await?;
        Ok(session)
    }

    /// Get session information
    pub async fn get_session(&self, session_id: &str) -> Result<FmSession> {
        let url = format!("{}/v1/sessions/{}", self.base_url, session_id);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Failed to get session: {} {}",
                status, text
            )));
        }

        let session: FmSession = response.json().await?;
        Ok(session)
    }

    /// Get session transcript (conversation history)
    pub async fn get_transcript(&self, session_id: &str) -> Result<Vec<FmTranscriptMessage>> {
        let url = format!("{}/v1/sessions/{}/transcript", self.base_url, session_id);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Failed to get transcript: {} {}",
                status, text
            )));
        }

        #[derive(Deserialize)]
        struct TranscriptResponse {
            messages: Vec<FmTranscriptMessage>,
        }

        let transcript: TranscriptResponse = response.json().await?;
        Ok(transcript.messages)
    }

    /// Complete with session context (multi-turn)
    pub async fn complete_with_session(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Result<CompletionResponse> {
        let url = format!("{}/v1/sessions/{}/complete", self.base_url, session_id);

        let request = FmCompletionRequest {
            model: "apple-foundation-model".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
        };

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Session completion failed: {} {}",
                status, text
            )));
        }

        let fm_response: FmCompletionResponse = response.json().await?;

        let text = fm_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(CompletionResponse {
            id: fm_response.id,
            model: fm_response.model,
            text,
            finish_reason: fm_response
                .choices
                .first()
                .and_then(|c| c.finish_reason.as_ref().map(|f| format!("{:?}", f))),
            usage: fm_response.usage.map(|u| UsageInfo {
                prompt_tokens: u.prompt_tokens.unwrap_or(0) as usize,
                completion_tokens: u.completion_tokens.unwrap_or(0) as usize,
                total_tokens: u.total_tokens.unwrap_or(0) as usize,
            }),
            extra: HashMap::new(),
        })
    }

    /// Stream completion with session context
    pub async fn stream_with_session(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let url = format!("{}/v1/sessions/{}/complete", self.base_url, session_id);

        let request = FmCompletionRequest {
            model: "apple-foundation-model".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: true,
        };

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Session stream failed: {} {}",
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

                        if let Ok(stream_response) =
                            serde_json::from_str::<FmStreamResponse>(&event.data)
                        {
                            if let Some(choice) = stream_response.choices.first() {
                                let delta_text = choice
                                    .delta
                                    .as_ref()
                                    .and_then(|d| d.content.clone())
                                    .unwrap_or_default();

                                let chunk = StreamChunk {
                                    id: String::new(),
                                    model: String::new(),
                                    delta: delta_text,
                                    finish_reason: choice
                                        .finish_reason
                                        .as_ref()
                                        .map(|f| format!("{:?}", f)),
                                    extra: HashMap::new(),
                                };

                                if tx.send(Ok(chunk)).await.is_err() {
                                    break;
                                }
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

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let url = format!("{}/v1/sessions/{}", self.base_url, session_id);

        let response = self.client.delete(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Failed to delete session: {} {}",
                status, text
            )));
        }

        Ok(())
    }

    // ========================================================================
    // Tool Management
    // ========================================================================

    /// Register tools for a session
    pub async fn register_tools(
        &self,
        session_id: &str,
        tools: Vec<FmToolDefinition>,
    ) -> Result<usize> {
        let url = format!("{}/v1/sessions/{}/tools", self.base_url, session_id);

        #[derive(Serialize)]
        struct RegisterRequest {
            tools: Vec<FmToolDefinition>,
        }

        let request = RegisterRequest { tools };
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Failed to register tools: {} {}",
                status, text
            )));
        }

        #[derive(Deserialize)]
        struct RegisterResponse {
            count: usize,
        }

        let result: RegisterResponse = response.json().await?;
        Ok(result.count)
    }

    /// List tools registered for a session
    pub async fn list_tools(&self, session_id: &str) -> Result<Vec<FmToolDefinition>> {
        let url = format!("{}/v1/sessions/{}/tools", self.base_url, session_id);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Failed to list tools: {} {}",
                status, text
            )));
        }

        #[derive(Deserialize)]
        struct ListResponse {
            tools: Vec<FmToolDefinition>,
        }

        let result: ListResponse = response.json().await?;
        Ok(result.tools)
    }

    /// Remove all tools from a session
    pub async fn remove_tools(&self, session_id: &str) -> Result<()> {
        let url = format!("{}/v1/sessions/{}/tools", self.base_url, session_id);

        let response = self.client.delete(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Failed to remove tools: {} {}",
                status, text
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl InferenceBackend for AppleFmBackend {
    fn id(&self) -> &str {
        "apple_fm"
    }

    async fn is_ready(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(response) => {
                if let Ok(health) = response.json::<HealthResponse>().await {
                    health.status == "ok" || health.status == "healthy"
                } else {
                    false
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
                "Apple FM bridge not responding".to_string(),
            ));
        }

        let models_response: ModelsResponse = response.json().await?;

        Ok(models_response
            .data
            .into_iter()
            .map(|m| ModelInfo::new(&m.id, &m.id, 8192).with_description(m.owned_by))
            .collect())
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let fm_request = FmCompletionRequest {
            model: request.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: request.prompt.clone(),
            }],
            temperature: request.temperature,
            max_tokens: request.max_tokens.map(|n| n as u32),
            top_p: request.top_p,
            stop: request.stop,
            stream: false,
        };

        let response = self.client.post(&url).json(&fm_request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Apple FM error {}: {}",
                status, text
            )));
        }

        let fm_response: FmCompletionResponse = response.json().await?;

        let text = fm_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(CompletionResponse {
            id: fm_response.id,
            model: fm_response.model,
            text,
            finish_reason: fm_response
                .choices
                .first()
                .and_then(|c| c.finish_reason.as_ref().map(|f| format!("{:?}", f))),
            usage: fm_response.usage.map(|u| UsageInfo {
                prompt_tokens: u.prompt_tokens.unwrap_or(0) as usize,
                completion_tokens: u.completion_tokens.unwrap_or(0) as usize,
                total_tokens: u.total_tokens.unwrap_or(0) as usize,
            }),
            extra: HashMap::new(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let fm_request = FmCompletionRequest {
            model: request.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: request.prompt.clone(),
            }],
            temperature: request.temperature,
            max_tokens: request.max_tokens.map(|n| n as u32),
            top_p: request.top_p,
            stop: request.stop,
            stream: true,
        };

        let response = self.client.post(&url).json(&fm_request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(BackendError::InferenceError(format!(
                "Apple FM error {}: {}",
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

                        if let Ok(stream_response) =
                            serde_json::from_str::<FmStreamResponse>(&event.data)
                        {
                            if let Some(choice) = stream_response.choices.first() {
                                let delta_text = choice
                                    .delta
                                    .as_ref()
                                    .and_then(|d| d.content.clone())
                                    .unwrap_or_default();

                                let chunk = StreamChunk {
                                    id: String::new(),
                                    model: String::new(),
                                    delta: delta_text,
                                    finish_reason: choice
                                        .finish_reason
                                        .as_ref()
                                        .map(|f| format!("{:?}", f)),
                                    extra: HashMap::new(),
                                };

                                if tx.send(Ok(chunk)).await.is_err() {
                                    break;
                                }
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

// ============================================================================
// Apple FM API Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct FmCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct FmCompletionResponse {
    id: String,
    model: String,
    choices: Vec<FmChoice>,
    usage: Option<FmUsage>,
}

#[derive(Debug, Deserialize)]
struct FmChoice {
    message: ChatMessage,
    finish_reason: Option<FinishReason>,
}

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FinishReason {
    Stop,
    Length,
    ToolCalls,
}

#[derive(Debug, Deserialize)]
struct FmUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct FmStreamResponse {
    choices: Vec<FmStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct FmStreamChoice {
    delta: Option<FmStreamDelta>,
    finish_reason: Option<FinishReason>,
}

#[derive(Debug, Deserialize)]
struct FmStreamDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<FmModelInfo>,
}

#[derive(Debug, Deserialize)]
struct FmModelInfo {
    id: String,
    owned_by: String,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
}
