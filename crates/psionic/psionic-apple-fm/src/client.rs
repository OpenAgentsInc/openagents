use futures_util::StreamExt;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::contract::{
    APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH, APPLE_FM_BRIDGE_HEALTH_PATH,
    APPLE_FM_BRIDGE_MODELS_PATH, APPLE_FM_BRIDGE_SESSIONS_PATH, APPLE_FM_BRIDGE_STREAM_SUFFIX,
    APPLE_FM_BRIDGE_STRUCTURED_SUFFIX, APPLE_FM_BRIDGE_TRANSCRIPT_SUFFIX,
    AppleFmChatCompletionRequest, AppleFmChatCompletionResponse, AppleFmCompletionResult,
    AppleFmErrorResponse, AppleFmGenerationOptions, AppleFmGenerationOptionsValidationError,
    AppleFmHealthResponse, AppleFmModelsResponse, AppleFmSession, AppleFmSessionCreateRequest,
    AppleFmSessionCreateResponse, AppleFmSessionRespondRequest, AppleFmSessionRespondResponse,
    AppleFmSessionStructuredGenerationRequest, AppleFmSessionStructuredGenerationResponse,
    AppleFmStructuredGenerationRequest, AppleFmStructuredGenerationResponse,
    AppleFmSystemLanguageModel, AppleFmSystemLanguageModelAvailability,
    AppleFmTextGenerationRequest, AppleFmTextGenerationResponse, AppleFmTextStreamEvent,
    AppleFmToolCallError, AppleFmToolCallRequest, AppleFmToolCallResponse,
    AppleFmToolCallbackConfiguration, AppleFmToolDefinition,
};
use crate::error::AppleFmFoundationModelsError;
use crate::structured::{AppleFmStructuredType, AppleFmStructuredValueError};
use crate::tool::AppleFmTool;
use crate::transcript::{AppleFmTranscript, AppleFmTranscriptError};

static NEXT_TOOL_SESSION_TOKEN: AtomicU64 = AtomicU64::new(1);

/// Reusable blocking client for the current Apple FM bridge contract.
#[derive(Clone)]
pub struct AppleFmBridgeClient {
    base_url: String,
    client: Client,
    tool_runtime: Arc<AppleFmToolCallbackRuntime>,
}

impl std::fmt::Debug for AppleFmBridgeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppleFmBridgeClient")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

/// Reusable async client for Apple FM streaming transport.
#[derive(Clone, Debug)]
pub struct AppleFmAsyncBridgeClient {
    base_url: String,
    client: reqwest::Client,
}

/// Async stream returned by Apple FM text-streaming APIs.
pub type AppleFmTextResponseStream =
    ReceiverStream<Result<AppleFmTextStreamEvent, AppleFmBridgeStreamError>>;

impl AppleFmBridgeClient {
    /// Creates a reusable bridge client with a default HTTP client.
    pub fn new(base_url: impl Into<String>) -> Result<Self, AppleFmBridgeClientError> {
        let client = Client::builder()
            .build()
            .map_err(|error| AppleFmBridgeClientError::ClientBuild(error.to_string()))?;
        Self::with_http_client(base_url, client)
    }

    /// Creates a reusable bridge client from a caller-provided HTTP client.
    pub fn with_http_client(
        base_url: impl Into<String>,
        client: Client,
    ) -> Result<Self, AppleFmBridgeClientError> {
        let base_url = canonical_base_url(base_url.into())?;
        Ok(Self {
            base_url,
            client,
            tool_runtime: Arc::new(AppleFmToolCallbackRuntime::default()),
        })
    }

    /// Returns the normalized base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    /// Resolves a bridge endpoint path against the configured base URL.
    pub fn endpoint(&self, path: &str) -> Result<Url, AppleFmBridgeClientError> {
        let base = Url::parse(self.base_url.as_str())
            .map_err(|error| AppleFmBridgeClientError::InvalidBaseUrl(error.to_string()))?;
        base.join(path)
            .map_err(|error| AppleFmBridgeClientError::InvalidEndpoint {
                path: path.to_string(),
                error: error.to_string(),
            })
    }

    /// Fetches bridge health.
    pub fn health(&self) -> Result<AppleFmHealthResponse, AppleFmBridgeClientError> {
        let response = self
            .client
            .get(self.endpoint(APPLE_FM_BRIDGE_HEALTH_PATH)?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "health",
                error: error.to_string(),
            })?;
        decode_json_response("health", response)
    }

    /// Fetches the full model-list response.
    pub fn list_models(&self) -> Result<AppleFmModelsResponse, AppleFmBridgeClientError> {
        let response = self
            .client
            .get(self.endpoint(APPLE_FM_BRIDGE_MODELS_PATH)?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "models",
                error: error.to_string(),
            })?;
        decode_json_response("models", response)
    }

    /// Fetches just the model identifiers exposed by the bridge.
    pub fn model_ids(&self) -> Result<Vec<String>, AppleFmBridgeClientError> {
        Ok(self.list_models()?.model_ids())
    }

    /// Fetches typed system-model availability/configuration truth.
    pub fn system_model_availability(
        &self,
    ) -> Result<AppleFmSystemLanguageModelAvailability, AppleFmBridgeClientError> {
        Ok(self.health()?.system_model_availability())
    }

    /// Creates a new Apple FM session or restores one from transcript JSON.
    pub fn create_session(
        &self,
        request: &AppleFmSessionCreateRequest,
    ) -> Result<AppleFmSession, AppleFmBridgeClientError> {
        request.normalized_transcript().map_err(|error| {
            AppleFmBridgeClientError::TranscriptValidation {
                operation: "create_session",
                error,
            }
        })?;
        let response = self
            .client
            .post(self.endpoint(APPLE_FM_BRIDGE_SESSIONS_PATH)?)
            .json(request)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "create_session",
                error: error.to_string(),
            })?;
        if !response.status().is_success() {
            return Err(map_status_response("create_session", response));
        }
        Ok(
            decode_json_response::<AppleFmSessionCreateResponse>("create_session", response)?
                .session,
        )
    }

    /// Creates a session from a typed transcript snapshot.
    pub fn create_session_from_transcript(
        &self,
        transcript: AppleFmTranscript,
        model: Option<AppleFmSystemLanguageModel>,
        tools: Vec<AppleFmToolDefinition>,
    ) -> Result<AppleFmSession, AppleFmBridgeClientError> {
        let request = AppleFmSessionCreateRequest::from_transcript(transcript, model, tools);
        self.create_session(&request)
    }

    /// Creates a new Apple FM session with active Rust-side tool implementations.
    pub fn create_session_with_tools(
        &self,
        request: &AppleFmSessionCreateRequest,
        tools: Vec<Arc<dyn AppleFmTool>>,
    ) -> Result<AppleFmSession, AppleFmBridgeClientError> {
        if tools.is_empty() {
            return self.create_session(request);
        }
        let mut request = request.clone();
        let (definitions, callback, session_token) = self
            .tool_runtime
            .register_tools(tools)
            .map_err(|error| AppleFmBridgeClientError::ToolRuntime {
                operation: "create_session_with_tools",
                error,
            })?;
        request.tools = definitions;
        request.tool_callback = Some(callback);
        match self.create_session(&request) {
            Ok(session) => {
                self.tool_runtime
                    .bind_session_token(session.id.as_str(), session_token);
                Ok(session)
            }
            Err(error) => {
                self.tool_runtime
                    .remove_session_token(session_token.as_str());
                Err(error)
            }
        }
    }

    /// Restores a session from transcript with active Rust-side tool implementations.
    pub fn create_session_from_transcript_with_tools(
        &self,
        transcript: AppleFmTranscript,
        model: Option<AppleFmSystemLanguageModel>,
        tools: Vec<Arc<dyn AppleFmTool>>,
    ) -> Result<AppleFmSession, AppleFmBridgeClientError> {
        self.create_session_with_tools(
            &AppleFmSessionCreateRequest::from_transcript(transcript, model, Vec::new()),
            tools,
        )
    }

    /// Fetches current session state.
    pub fn session(&self, session_id: &str) -> Result<AppleFmSession, AppleFmBridgeClientError> {
        let response = self
            .client
            .get(self.endpoint(&session_path(session_id))?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "session",
                error: error.to_string(),
            })?;
        decode_json_response("session", response)
    }

    /// Exports the current session transcript as a typed transcript snapshot.
    pub fn session_transcript(
        &self,
        session_id: &str,
    ) -> Result<AppleFmTranscript, AppleFmBridgeClientError> {
        let response = self
            .client
            .get(self.endpoint(&session_transcript_path(session_id))?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "session_transcript",
                error: error.to_string(),
            })?;
        decode_json_response("session_transcript", response)
    }

    /// Deletes a session handle.
    pub fn delete_session(&self, session_id: &str) -> Result<(), AppleFmBridgeClientError> {
        let response = self
            .client
            .delete(self.endpoint(&session_path(session_id))?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "delete_session",
                error: error.to_string(),
            })?;
        ensure_success_response("delete_session", response)?;
        self.tool_runtime.unregister_session(session_id);
        Ok(())
    }

    /// Resets a session after failure/cancellation semantics.
    pub fn reset_session(
        &self,
        session_id: &str,
    ) -> Result<AppleFmSession, AppleFmBridgeClientError> {
        let response = self
            .client
            .post(self.endpoint(&session_reset_path(session_id))?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "reset_session",
                error: error.to_string(),
            })?;
        decode_json_response("reset_session", response)
    }

    /// Executes a prompt inside a persistent Apple FM session.
    pub fn respond_in_session(
        &self,
        session_id: &str,
        request: &AppleFmSessionRespondRequest,
    ) -> Result<AppleFmSessionRespondResponse, AppleFmBridgeClientError> {
        request
            .validate()
            .map_err(|error| AppleFmBridgeClientError::Validation {
                operation: "respond_in_session",
                error,
            })?;
        let response = self
            .client
            .post(self.endpoint(&session_responses_path(session_id))?)
            .json(request)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "respond_in_session",
                error: error.to_string(),
            })?;
        if !response.status().is_success() {
            return Err(map_status_response("respond_in_session", response));
        }
        decode_json_response("respond_in_session", response)
    }

    /// Executes a structured-generation prompt inside a persistent Apple FM session.
    pub fn respond_structured_in_session(
        &self,
        session_id: &str,
        request: &AppleFmSessionStructuredGenerationRequest,
    ) -> Result<AppleFmSessionStructuredGenerationResponse, AppleFmBridgeClientError> {
        request
            .validate()
            .map_err(|error| AppleFmBridgeClientError::StructuredValidation {
                operation: "respond_structured_in_session",
                error,
            })?;
        let response = self
            .client
            .post(self.endpoint(&session_structured_responses_path(session_id))?)
            .json(request)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "respond_structured_in_session",
                error: error.to_string(),
            })?;
        if !response.status().is_success() {
            return Err(map_status_response(
                "respond_structured_in_session",
                response,
            ));
        }
        decode_json_response("respond_structured_in_session", response)
    }

    /// Executes a raw chat-completion request against the bridge.
    pub fn chat_completion(
        &self,
        request: &AppleFmChatCompletionRequest,
    ) -> Result<AppleFmChatCompletionResponse, AppleFmBridgeClientError> {
        request
            .validate()
            .map_err(|error| AppleFmBridgeClientError::Validation {
                operation: "chat_completion",
                error,
            })?;
        let response = self
            .client
            .post(self.endpoint(APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH)?)
            .json(request)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "chat_completion",
                error: error.to_string(),
            })?;
        decode_json_response("chat_completion", response)
    }

    /// Executes a first-class plain-text generation request against the bridge.
    pub fn generate_text(
        &self,
        request: &AppleFmTextGenerationRequest,
    ) -> Result<AppleFmTextGenerationResponse, AppleFmBridgeClientError> {
        request
            .validate()
            .map_err(|error| AppleFmBridgeClientError::Validation {
                operation: "generate_text",
                error,
            })?;
        let response = self.chat_completion(&request.clone().into_chat_completion_request())?;
        Ok(AppleFmTextGenerationResponse {
            model: response.model.clone(),
            output: response
                .first_text_content()
                .unwrap_or_default()
                .to_string(),
            usage: response.usage,
        })
    }

    /// Executes a one-shot user prompt and extracts the first text completion.
    pub fn completion_from_prompt(
        &self,
        prompt: impl Into<String>,
        model: Option<impl Into<String>>,
        max_tokens: Option<u32>,
        temperature: Option<f64>,
    ) -> Result<AppleFmCompletionResult, AppleFmBridgeClientError> {
        self.completion_from_prompt_with_options(
            prompt,
            model,
            Some(AppleFmGenerationOptions {
                sampling: None,
                temperature,
                maximum_response_tokens: max_tokens,
            }),
        )
    }

    /// Executes a one-shot user prompt with typed generation options.
    pub fn completion_from_prompt_with_options(
        &self,
        prompt: impl Into<String>,
        model: Option<impl Into<String>>,
        options: Option<AppleFmGenerationOptions>,
    ) -> Result<AppleFmCompletionResult, AppleFmBridgeClientError> {
        let request = AppleFmTextGenerationRequest {
            model: model.map(Into::into),
            prompt: prompt.into(),
            options,
        };
        Ok(self.generate_text(&request)?.completion_result())
    }

    /// Executes a one-shot user prompt through an Apple FM session with active tools.
    pub fn completion_from_prompt_with_tools(
        &self,
        prompt: impl Into<String>,
        model: Option<impl Into<String>>,
        options: Option<AppleFmGenerationOptions>,
        tools: Vec<Arc<dyn AppleFmTool>>,
    ) -> Result<AppleFmCompletionResult, AppleFmBridgeClientError> {
        let session = self.create_session_with_tools(
            &AppleFmSessionCreateRequest {
                instructions: None,
                model: model.map(|model_id| AppleFmSystemLanguageModel {
                    id: model_id.into(),
                    ..Default::default()
                }),
                tools: Vec::new(),
                tool_callback: None,
                transcript_json: None,
                transcript: None,
            },
            tools,
        )?;
        let response = self.respond_in_session(
            session.id.as_str(),
            &AppleFmSessionRespondRequest {
                prompt: prompt.into(),
                options,
            },
        );
        let _ = self.delete_session(session.id.as_str());
        response.map(|response| AppleFmCompletionResult {
            model: response.model,
            output: response.output,
            prompt_tokens: response
                .usage
                .as_ref()
                .and_then(|usage| usage.prompt_tokens),
            completion_tokens: response
                .usage
                .as_ref()
                .and_then(|usage| usage.completion_tokens),
            total_tokens: response.usage.as_ref().and_then(|usage| usage.total_tokens),
            usage: response.usage,
        })
    }

    /// Executes one-shot structured generation using an ephemeral Apple FM session.
    pub fn generate_structured(
        &self,
        request: &AppleFmStructuredGenerationRequest,
    ) -> Result<AppleFmStructuredGenerationResponse, AppleFmBridgeClientError> {
        request
            .validate()
            .map_err(|error| AppleFmBridgeClientError::StructuredValidation {
                operation: "generate_structured",
                error,
            })?;
        let session = self.create_session(&AppleFmSessionCreateRequest {
            instructions: None,
            model: request
                .model
                .clone()
                .map(|model_id| AppleFmSystemLanguageModel {
                    id: model_id,
                    ..Default::default()
                }),
            tools: Vec::new(),
            tool_callback: None,
            transcript_json: None,
            transcript: None,
        })?;
        let response = self.respond_structured_in_session(
            session.id.as_str(),
            &AppleFmSessionStructuredGenerationRequest {
                prompt: request.prompt.clone(),
                schema: request.schema.clone(),
                options: request.options.clone(),
            },
        );
        let _ = self.delete_session(session.id.as_str());
        response.map(|response| AppleFmStructuredGenerationResponse {
            model: response.model,
            content: response.content,
            usage: response.usage,
        })
    }

    /// Executes one-shot structured generation and decodes it into a Rust type.
    pub fn generate_typed<T>(
        &self,
        prompt: impl Into<String>,
        model: Option<impl Into<String>>,
        options: Option<AppleFmGenerationOptions>,
    ) -> Result<T, AppleFmBridgeClientError>
    where
        T: AppleFmStructuredType,
    {
        let schema =
            crate::structured::AppleFmGenerationSchema::from_type::<T>().map_err(|error| {
                AppleFmBridgeClientError::StructuredValidation {
                    operation: "generate_typed",
                    error,
                }
            })?;
        let response = self.generate_structured(&AppleFmStructuredGenerationRequest {
            model: model.map(Into::into),
            prompt: prompt.into(),
            schema,
            options,
        })?;
        response.content.to_typed::<T>().map_err(|error| {
            AppleFmBridgeClientError::StructuredDecode {
                operation: "generate_typed",
                error,
            }
        })
    }
}

impl AppleFmAsyncBridgeClient {
    /// Creates a reusable async bridge client for streaming endpoints.
    pub fn new(base_url: impl Into<String>) -> Result<Self, AppleFmBridgeClientError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|error| AppleFmBridgeClientError::ClientBuild(error.to_string()))?;
        Self::with_http_client(base_url, client)
    }

    /// Creates a reusable async bridge client from a caller-provided HTTP client.
    pub fn with_http_client(
        base_url: impl Into<String>,
        client: reqwest::Client,
    ) -> Result<Self, AppleFmBridgeClientError> {
        let base_url = canonical_base_url(base_url.into())?;
        Ok(Self { base_url, client })
    }

    /// Returns the normalized base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }

    /// Resolves a bridge endpoint path against the configured base URL.
    pub fn endpoint(&self, path: &str) -> Result<Url, AppleFmBridgeClientError> {
        let base = Url::parse(self.base_url.as_str())
            .map_err(|error| AppleFmBridgeClientError::InvalidBaseUrl(error.to_string()))?;
        base.join(path)
            .map_err(|error| AppleFmBridgeClientError::InvalidEndpoint {
                path: path.to_string(),
                error: error.to_string(),
            })
    }

    /// Opens a true streaming session-response transport with snapshot semantics.
    pub async fn stream_session_response(
        &self,
        session_id: &str,
        request: &AppleFmSessionRespondRequest,
    ) -> Result<AppleFmTextResponseStream, AppleFmBridgeClientError> {
        request
            .validate()
            .map_err(|error| AppleFmBridgeClientError::Validation {
                operation: "stream_session_response",
                error,
            })?;
        let response = self
            .client
            .post(self.endpoint(&session_stream_path(session_id))?)
            .header(ACCEPT, "text/event-stream")
            .json(request)
            .send()
            .await
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "stream_session_response",
                error: error.to_string(),
            })?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(map_status_payload("stream_session_response", status, body));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if !content_type.starts_with("text/event-stream") {
            return Err(AppleFmBridgeClientError::InvalidStreamContentType {
                operation: "stream_session_response",
                content_type,
            });
        }

        let (tx, rx) = mpsc::channel(32);
        let mut bytes_stream = response.bytes_stream();
        tokio::spawn(async move {
            let mut pending_buffer = String::new();
            let mut pending_event = PendingSseEvent::default();
            while let Some(chunk_result) = bytes_stream.next().await {
                if tx.is_closed() {
                    return;
                }
                let chunk = match chunk_result {
                    Ok(chunk) => chunk,
                    Err(error) => {
                        let _ = tx
                            .send(Err(AppleFmBridgeStreamError::Transport(error.to_string())))
                            .await;
                        return;
                    }
                };
                pending_buffer.push_str(String::from_utf8_lossy(&chunk).as_ref());
                if let Some(error) =
                    consume_text_stream_buffer(&mut pending_buffer, &mut pending_event, &tx).await
                {
                    let _ = tx.send(Err(error)).await;
                    return;
                }
            }
            if let Some(error) = flush_pending_text_stream_event(&mut pending_event, &tx).await {
                let _ = tx.send(Err(error)).await;
            }
        });

        Ok(ReceiverStream::new(rx))
    }
}

fn canonical_base_url(base_url: String) -> Result<String, AppleFmBridgeClientError> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(AppleFmBridgeClientError::EmptyBaseUrl);
    }
    Url::parse(trimmed)
        .map_err(|error| AppleFmBridgeClientError::InvalidBaseUrl(error.to_string()))?;
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn session_path(session_id: &str) -> String {
    format!("{APPLE_FM_BRIDGE_SESSIONS_PATH}/{session_id}")
}

fn session_reset_path(session_id: &str) -> String {
    format!("{}/reset", session_path(session_id))
}

fn session_responses_path(session_id: &str) -> String {
    format!("{}/responses", session_path(session_id))
}

fn session_stream_path(session_id: &str) -> String {
    format!(
        "{}{}",
        session_responses_path(session_id),
        APPLE_FM_BRIDGE_STREAM_SUFFIX
    )
}

fn session_transcript_path(session_id: &str) -> String {
    format!(
        "{}{}",
        session_path(session_id),
        APPLE_FM_BRIDGE_TRANSCRIPT_SUFFIX
    )
}

fn session_structured_responses_path(session_id: &str) -> String {
    format!(
        "{}{}",
        session_responses_path(session_id),
        APPLE_FM_BRIDGE_STRUCTURED_SUFFIX
    )
}

fn decode_json_response<T: DeserializeOwned>(
    operation: &'static str,
    response: reqwest::blocking::Response,
) -> Result<T, AppleFmBridgeClientError> {
    if !response.status().is_success() {
        return Err(map_status_response(operation, response));
    }
    response
        .json::<T>()
        .map_err(|error| AppleFmBridgeClientError::Decode {
            operation,
            error: error.to_string(),
        })
}

fn ensure_success_response(
    operation: &'static str,
    response: reqwest::blocking::Response,
) -> Result<(), AppleFmBridgeClientError> {
    if !response.status().is_success() {
        return Err(map_status_response(operation, response));
    }
    Ok(())
}

#[derive(Default)]
struct AppleFmToolCallbackRuntime {
    state: Arc<AppleFmToolCallbackRuntimeState>,
}

type AppleFmToolMap = HashMap<String, Arc<dyn AppleFmTool>>;
type AppleFmToolRegistry = HashMap<String, AppleFmToolMap>;

#[derive(Default)]
struct AppleFmToolCallbackRuntimeState {
    callback_url: Mutex<Option<String>>,
    tools_by_token: Mutex<AppleFmToolRegistry>,
    session_tokens: Mutex<HashMap<String, String>>,
}

impl AppleFmToolCallbackRuntime {
    fn register_tools(
        &self,
        tools: Vec<Arc<dyn AppleFmTool>>,
    ) -> Result<
        (
            Vec<AppleFmToolDefinition>,
            AppleFmToolCallbackConfiguration,
            String,
        ),
        String,
    > {
        let callback_url = self.ensure_callback_url()?;
        let session_token = next_tool_session_token();
        let mut tool_map = HashMap::new();
        let mut definitions = Vec::with_capacity(tools.len());
        for tool in tools {
            let definition = tool.definition();
            tool_map.insert(definition.name.clone(), Arc::clone(&tool));
            definitions.push(definition);
        }
        self.state
            .tools_by_token
            .lock()
            .map_err(|_| "tool runtime lock poisoned".to_string())?
            .insert(session_token.clone(), tool_map);
        Ok((
            definitions,
            AppleFmToolCallbackConfiguration {
                url: callback_url,
                session_token: session_token.clone(),
            },
            session_token,
        ))
    }

    fn bind_session_token(&self, session_id: &str, session_token: String) {
        if let Ok(mut tokens) = self.state.session_tokens.lock() {
            tokens.insert(session_id.to_string(), session_token);
        }
    }

    fn unregister_session(&self, session_id: &str) {
        let session_token = self
            .state
            .session_tokens
            .lock()
            .ok()
            .and_then(|mut tokens| tokens.remove(session_id));
        if let Some(session_token) = session_token {
            self.remove_session_token(session_token.as_str());
        }
    }

    fn remove_session_token(&self, session_token: &str) {
        if let Ok(mut tools) = self.state.tools_by_token.lock() {
            tools.remove(session_token);
        }
    }

    fn ensure_callback_url(&self) -> Result<String, String> {
        if let Some(url) = self
            .state
            .callback_url
            .lock()
            .map_err(|_| "tool runtime lock poisoned".to_string())?
            .clone()
        {
            return Ok(url);
        }

        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("bind callback listener: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("tool callback listener addr: {error}"))?
            .port();
        let callback_url = format!("http://127.0.0.1:{port}/tool-call");
        let state = Arc::clone(&self.state);
        thread::spawn(move || run_tool_callback_server(listener, state));
        *self
            .state
            .callback_url
            .lock()
            .map_err(|_| "tool runtime lock poisoned".to_string())? = Some(callback_url.clone());
        Ok(callback_url)
    }
}

fn next_tool_session_token() -> String {
    let next = NEXT_TOOL_SESSION_TOKEN.fetch_add(1, Ordering::Relaxed);
    format!("tool-session-{next}")
}

fn run_tool_callback_server(listener: TcpListener, state: Arc<AppleFmToolCallbackRuntimeState>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let state = Arc::clone(&state);
        thread::spawn(move || {
            let _ = handle_tool_callback_connection(stream, state);
        });
    }
}

fn handle_tool_callback_connection(
    mut stream: TcpStream,
    state: Arc<AppleFmToolCallbackRuntimeState>,
) -> Result<(), String> {
    let request = read_http_request(&mut stream)?;
    if request.method != "POST" || request.path != "/tool-call" {
        write_json_response(
            &mut stream,
            404,
            &AppleFmErrorResponse {
                error: crate::contract::AppleFmErrorDetail {
                    message: format!("Not found: {} {}", request.method, request.path),
                    r#type: "not_found".to_string(),
                    code: Some("not_found".to_string()),
                    tool_name: None,
                    underlying_error: None,
                    failure_reason: None,
                    recovery_suggestion: None,
                    debug_description: None,
                    refusal_explanation: None,
                },
            },
        )?;
        return Ok(());
    }
    let callback_request: AppleFmToolCallRequest = serde_json::from_slice(request.body.as_slice())
        .map_err(|error| format!("decode tool callback request: {error}"))?;
    match dispatch_tool_call(&state, callback_request) {
        Ok(output) => write_json_response(&mut stream, 200, &AppleFmToolCallResponse { output })?,
        Err(error) => write_json_response(&mut stream, 422, &error)?,
    }
    Ok(())
}

fn dispatch_tool_call(
    state: &AppleFmToolCallbackRuntimeState,
    request: AppleFmToolCallRequest,
) -> Result<String, AppleFmToolCallError> {
    let tool = state
        .tools_by_token
        .lock()
        .map_err(|_| {
            AppleFmToolCallError::new(request.tool_name.clone(), "tool runtime lock poisoned")
        })?
        .get(request.session_token.as_str())
        .and_then(|tools| tools.get(request.tool_name.as_str()).cloned())
        .ok_or_else(|| {
            AppleFmToolCallError::new(
                request.tool_name.clone(),
                format!(
                    "no registered Rust-side Apple FM tool for session token '{}'",
                    request.session_token
                ),
            )
        })?;
    tool.call(request.arguments)
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut header_end = None;
    let mut content_length = 0usize;
    loop {
        let mut chunk = [0_u8; 4096];
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("read tool callback request: {error}"))?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if header_end.is_none()
            && let Some(index) = find_header_end(buffer.as_slice())
        {
            header_end = Some(index);
            content_length = parse_content_length(&buffer[..index])?;
        }
        if let Some(index) = header_end {
            let body_end = index + 4 + content_length;
            if buffer.len() >= body_end {
                break;
            }
        }
    }
    let header_end = header_end.ok_or("missing HTTP header terminator".to_string())?;
    let header_text =
        String::from_utf8(buffer[..header_end].to_vec()).map_err(|error| error.to_string())?;
    let mut header_lines = header_text.split("\r\n");
    let request_line = header_lines
        .next()
        .ok_or("missing tool callback request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or("missing tool callback method".to_string())?
        .to_string();
    let path = request_parts
        .next()
        .ok_or("missing tool callback path".to_string())?
        .to_string();
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    if buffer.len() < body_end {
        return Err("incomplete HTTP body".to_string());
    }
    Ok(HttpRequest {
        method,
        path,
        body: buffer[body_start..body_end].to_vec(),
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &[u8]) -> Result<usize, String> {
    let headers =
        String::from_utf8(headers.to_vec()).map_err(|error| format!("decode headers: {error}"))?;
    for line in headers.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|error| format!("invalid content-length: {error}"));
        }
    }
    Ok(0)
}

fn write_json_response<T: serde::Serialize>(
    stream: &mut TcpStream,
    status_code: u16,
    body: &T,
) -> Result<(), String> {
    let body = serde_json::to_string(body).map_err(|error| error.to_string())?;
    let response = format!(
        "HTTP/1.1 {status_code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("write tool callback response: {error}"))
}

fn map_status_response(
    operation: &'static str,
    response: reqwest::blocking::Response,
) -> AppleFmBridgeClientError {
    let status = response.status();
    let body = response.text().unwrap_or_default();
    map_status_payload(operation, status, body)
}

fn map_status_payload(
    operation: &'static str,
    status: reqwest::StatusCode,
    body: String,
) -> AppleFmBridgeClientError {
    if let Ok(error_response) = serde_json::from_str::<AppleFmErrorResponse>(body.as_str()) {
        return AppleFmBridgeClientError::FoundationModels {
            operation,
            error: Box::new(AppleFmFoundationModelsError::from(error_response.error)),
        };
    }
    AppleFmBridgeClientError::Status {
        operation,
        error: format!("{status}: {body}"),
    }
}

#[derive(Default)]
struct PendingSseEvent {
    event_name: Option<String>,
    data_lines: Vec<String>,
}

async fn consume_text_stream_buffer(
    pending_buffer: &mut String,
    pending_event: &mut PendingSseEvent,
    tx: &mpsc::Sender<Result<AppleFmTextStreamEvent, AppleFmBridgeStreamError>>,
) -> Option<AppleFmBridgeStreamError> {
    let mut consumed_up_to = 0usize;
    while let Some(relative_end) = pending_buffer[consumed_up_to..].find('\n') {
        let line_end = consumed_up_to + relative_end;
        let mut line = pending_buffer[consumed_up_to..line_end].to_string();
        if line.ends_with('\r') {
            line.pop();
        }
        consumed_up_to = line_end.saturating_add(1);

        if line.is_empty() {
            if let Some(error) = flush_pending_text_stream_event(pending_event, tx).await {
                return Some(error);
            }
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            pending_event.event_name = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("data:") {
            pending_event
                .data_lines
                .push(value.trim_start().to_string());
        }
    }
    pending_buffer.drain(..consumed_up_to);
    None
}

async fn flush_pending_text_stream_event(
    pending_event: &mut PendingSseEvent,
    tx: &mpsc::Sender<Result<AppleFmTextStreamEvent, AppleFmBridgeStreamError>>,
) -> Option<AppleFmBridgeStreamError> {
    let event_name = pending_event.event_name.take();
    if pending_event.data_lines.is_empty() {
        pending_event.data_lines.clear();
        return None;
    }
    let payload = pending_event.data_lines.join("\n");
    pending_event.data_lines.clear();

    match event_name.as_deref() {
        Some("snapshot") | Some("completed") | None => {
            let event =
                serde_json::from_str::<AppleFmTextStreamEvent>(payload.as_str()).map_err(|error| {
                    AppleFmBridgeStreamError::Decode {
                        event: event_name.unwrap_or_else(|| "snapshot".to_string()),
                        error: error.to_string(),
                    }
                });
            match event {
                Ok(event) => {
                    let _ = tx.send(Ok(event)).await;
                    None
                }
                Err(error) => Some(error),
            }
        }
        Some("error") => {
            let error_payload = serde_json::from_str::<AppleFmErrorResponse>(payload.as_str())
                .map_err(|error| AppleFmBridgeStreamError::Decode {
                    event: "error".to_string(),
                    error: error.to_string(),
                });
            match error_payload {
                Ok(error_payload) => Some(AppleFmBridgeStreamError::FoundationModels {
                    error: AppleFmFoundationModelsError::from(error_payload.error),
                }),
                Err(error) => Some(error),
            }
        }
        Some(other) => Some(AppleFmBridgeStreamError::UnexpectedEvent(other.to_string())),
    }
}

/// Reusable Apple FM bridge client error.
#[derive(Debug, Error)]
pub enum AppleFmBridgeClientError {
    /// HTTP client initialization failed.
    #[error("failed to initialize Apple FM HTTP client: {0}")]
    ClientBuild(String),
    /// Base URL is missing.
    #[error("Apple FM base URL is empty")]
    EmptyBaseUrl,
    /// Base URL could not be parsed.
    #[error("invalid Apple FM base URL: {0}")]
    InvalidBaseUrl(String),
    /// Endpoint resolution failed.
    #[error("invalid Apple FM endpoint path '{path}': {error}")]
    InvalidEndpoint { path: String, error: String },
    /// Request transport failed.
    #[error("Apple FM {operation} request failed: {error}")]
    Transport {
        /// Bridge operation label.
        operation: &'static str,
        /// Error detail.
        error: String,
    },
    /// HTTP status was unsuccessful.
    #[error("Apple FM {operation} returned error: {error}")]
    Status {
        /// Bridge operation label.
        operation: &'static str,
        /// Error detail.
        error: String,
    },
    /// The bridge returned a typed Foundation Models failure payload.
    #[error("Apple FM {operation} returned typed Foundation Models error: {error}")]
    FoundationModels {
        /// Bridge operation label.
        operation: &'static str,
        /// Typed remote error.
        error: Box<AppleFmFoundationModelsError>,
    },
    /// Response decode failed.
    #[error("Apple FM {operation} decode failed: {error}")]
    Decode {
        /// Bridge operation label.
        operation: &'static str,
        /// Error detail.
        error: String,
    },
    /// Local request validation failed before transport.
    #[error("Apple FM {operation} validation failed: {error}")]
    Validation {
        /// Bridge operation label.
        operation: &'static str,
        /// Validation detail.
        error: AppleFmGenerationOptionsValidationError,
    },
    /// Local transcript validation failed before transport.
    #[error("Apple FM {operation} transcript validation failed: {error}")]
    TranscriptValidation {
        /// Bridge operation label.
        operation: &'static str,
        /// Transcript validation detail.
        error: AppleFmTranscriptError,
    },
    /// Local structured-generation validation failed before transport.
    #[error("Apple FM {operation} structured validation failed: {error}")]
    StructuredValidation {
        /// Bridge operation label.
        operation: &'static str,
        /// Structured validation detail.
        error: AppleFmStructuredValueError,
    },
    /// Local structured-content decode failed after transport.
    #[error("Apple FM {operation} structured decode failed: {error}")]
    StructuredDecode {
        /// Bridge operation label.
        operation: &'static str,
        /// Structured decode detail.
        error: AppleFmStructuredValueError,
    },
    /// Local tool runtime setup or callback transport failed.
    #[error("Apple FM {operation} tool runtime failed: {error}")]
    ToolRuntime {
        /// Bridge operation label.
        operation: &'static str,
        /// Tool runtime detail.
        error: String,
    },
    /// Stream endpoint returned the wrong content type.
    #[error("Apple FM {operation} returned non-stream content type: {content_type}")]
    InvalidStreamContentType {
        /// Bridge operation label.
        operation: &'static str,
        /// Actual content type header.
        content_type: String,
    },
}

impl AppleFmBridgeClientError {
    /// Returns the typed remote Foundation Models error when available.
    #[must_use]
    pub fn foundation_models_error(&self) -> Option<&AppleFmFoundationModelsError> {
        match self {
            Self::FoundationModels { error, .. } => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Errors yielded while consuming the Apple FM SSE transport.
#[derive(Debug, Error)]
pub enum AppleFmBridgeStreamError {
    /// The HTTP transport failed mid-stream.
    #[error("Apple FM streaming transport failed: {0}")]
    Transport(String),
    /// The stream contained an unexpected event name.
    #[error("Apple FM stream emitted unexpected event '{0}'")]
    UnexpectedEvent(String),
    /// An SSE event payload failed to decode.
    #[error("Apple FM stream {event} payload decode failed: {error}")]
    Decode {
        /// Event name.
        event: String,
        /// Decode detail.
        error: String,
    },
    /// The remote stream emitted a typed Foundation Models error.
    #[error("Apple FM stream failed: {error}")]
    FoundationModels {
        /// Typed remote error.
        error: AppleFmFoundationModelsError,
    },
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use std::io::{ErrorKind, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};
    use tokio_stream::StreamExt;

    use super::{
        AppleFmAsyncBridgeClient, AppleFmBridgeClient, AppleFmBridgeClientError,
        AppleFmToolCallbackRuntime, dispatch_tool_call,
    };
    use crate::contract::{
        AppleFmErrorCode, AppleFmGenerationOptions, AppleFmSamplingMode,
        AppleFmSessionCreateRequest, AppleFmSessionRespondRequest,
        AppleFmSessionStructuredGenerationRequest, AppleFmStructuredGenerationRequest,
        AppleFmSystemLanguageModel, AppleFmSystemLanguageModelGuardrails,
        AppleFmSystemLanguageModelUseCase, AppleFmTextGenerationRequest, AppleFmToolCallError,
        AppleFmToolCallRequest, AppleFmToolDefinition, AppleFmUsageTruth,
    };
    use crate::structured::AppleFmGenerationSchema;
    use crate::tool::AppleFmTool;
    use crate::transcript::{
        APPLE_FM_TRANSCRIPT_TYPE, AppleFmTranscript, AppleFmTranscriptContent,
        AppleFmTranscriptEntry, AppleFmTranscriptPayload,
    };

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    struct TaskSummary {
        title: String,
        completed: bool,
        tags: Vec<String>,
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    struct SecretLookupArgs {
        key: String,
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    struct UserLookupArgs {
        user_id: u64,
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    struct ListJoinArgs {
        items: Vec<String>,
        separator: String,
    }

    #[derive(Clone)]
    struct SecretLookupTool;

    impl AppleFmTool for SecretLookupTool {
        fn definition(&self) -> AppleFmToolDefinition {
            AppleFmToolDefinition::typed::<SecretLookupArgs>(
                "lookup_secret_code",
                Some("Returns a stable secret code for the requested key"),
            )
            .expect("secret lookup tool definition")
        }

        fn call(
            &self,
            arguments: crate::structured::AppleFmGeneratedContent,
        ) -> Result<String, AppleFmToolCallError> {
            let key = arguments
                .property::<String>("key")
                .map_err(|error| {
                    AppleFmToolCallError::new("lookup_secret_code", error.to_string())
                })?
                .unwrap_or_default();
            let output = match key.as_str() {
                "fm_status" => "FM-LIVE-RECEIPT-42",
                "desktop_lane" => "APPLE-FM-MAC-LANE",
                _ => "UNKNOWN-SECRET",
            };
            Ok(output.to_string())
        }
    }

    #[derive(Clone)]
    struct UserLookupTool;

    impl AppleFmTool for UserLookupTool {
        fn definition(&self) -> AppleFmToolDefinition {
            AppleFmToolDefinition::typed::<UserLookupArgs>(
                "lookup_user_profile",
                Some("Returns the stored user profile for a numeric user id"),
            )
            .expect("user lookup tool definition")
        }

        fn call(
            &self,
            arguments: crate::structured::AppleFmGeneratedContent,
        ) -> Result<String, AppleFmToolCallError> {
            let user_id = arguments
                .property::<u64>("user_id")
                .map_err(|error| {
                    AppleFmToolCallError::new("lookup_user_profile", error.to_string())
                })?
                .unwrap_or_default();
            match user_id {
                1 => Ok("USER-1: Alice / admin".to_string()),
                2 => Ok("USER-2: Bob / user".to_string()),
                other => Err(AppleFmToolCallError::new(
                    "lookup_user_profile",
                    format!("user {other} not found"),
                )),
            }
        }
    }

    #[derive(Clone)]
    struct ListJoinTool;

    impl AppleFmTool for ListJoinTool {
        fn definition(&self) -> AppleFmToolDefinition {
            AppleFmToolDefinition::typed::<ListJoinArgs>(
                "join_list_values",
                Some("Joins a list of strings with the provided separator"),
            )
            .expect("list join tool definition")
        }

        fn call(
            &self,
            arguments: crate::structured::AppleFmGeneratedContent,
        ) -> Result<String, AppleFmToolCallError> {
            let items = arguments
                .property::<Vec<String>>("items")
                .map_err(|error| AppleFmToolCallError::new("join_list_values", error.to_string()))?
                .unwrap_or_default();
            let separator = arguments
                .property::<String>("separator")
                .map_err(|error| AppleFmToolCallError::new("join_list_values", error.to_string()))?
                .unwrap_or_else(|| ",".to_string());
            Ok(items.join(separator.as_str()))
        }
    }

    #[derive(Clone)]
    struct FailingTool;

    impl AppleFmTool for FailingTool {
        fn definition(&self) -> AppleFmToolDefinition {
            AppleFmToolDefinition::typed::<SecretLookupArgs>(
                "always_fail",
                Some("Always fails for tool-call error coverage"),
            )
            .expect("failing tool definition")
        }

        fn call(
            &self,
            arguments: crate::structured::AppleFmGeneratedContent,
        ) -> Result<String, AppleFmToolCallError> {
            let key = arguments
                .property::<String>("key")
                .map_err(|error| AppleFmToolCallError::new("always_fail", error.to_string()))?
                .unwrap_or_else(|| "unknown".to_string());
            Err(AppleFmToolCallError::new(
                "always_fail",
                format!("intentional failure for key '{key}'"),
            ))
        }
    }

    fn search_tool_definition() -> AppleFmToolDefinition {
        AppleFmToolDefinition::new(
            "search",
            Some("Search docs"),
            AppleFmGenerationSchema::from_json_str(
                r#"{"type":"object","properties":{"query":{"type":"string"}}}"#,
            )
            .expect("search tool schema"),
        )
    }

    fn empty_transcript_json() -> String {
        serde_json::json!({
            "version": 1,
            "type": APPLE_FM_TRANSCRIPT_TYPE,
            "transcript": {
                "entries": []
            }
        })
        .to_string()
    }

    fn non_empty_transcript_json() -> String {
        serde_json::json!({
            "version": 1,
            "type": APPLE_FM_TRANSCRIPT_TYPE,
            "transcript": {
                "entries": [{
                    "id": "entry-1",
                    "role": "user",
                    "contents": [{
                        "id": "content-1",
                        "type": "text",
                        "text": "hello"
                    }]
                }]
            }
        })
        .to_string()
    }

    fn spawn_mock_bridge() -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock bridge");
        listener
            .set_nonblocking(true)
            .expect("set mock bridge nonblocking");
        let address = listener.local_addr().expect("bridge addr");
        let session_counter = Arc::new(AtomicUsize::new(0));
        let session_counter_handle = Arc::clone(&session_counter);
        let handle = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(error) => panic!("accept mock request: {error}"),
                };
                let (method, path, request_body) = read_request(&mut stream);
                let (status_code, body) = if method == "GET" && path == "/health" {
                    (200, serde_json::json!({
                        "status": "ok",
                        "model_available": true,
                        "availability_message": "Foundation Models is available",
                        "default_use_case": "general",
                        "default_guardrails": "default",
                        "supported_use_cases": ["general", "content_tagging"],
                        "supported_guardrails": ["default", "permissive_content_transformations"]
                    })
                    .to_string())
                } else if method == "GET" && path == "/v1/models" {
                    (200, serde_json::json!({
                        "object": "list",
                        "data": [{
                            "id": "apple-foundation-model",
                            "object": "model",
                            "default_use_case": "general",
                            "default_guardrails": "default",
                            "supported_use_cases": ["general", "content_tagging"],
                            "supported_guardrails": ["default", "permissive_content_transformations"],
                            "available": true
                        }]
                    })
                    .to_string())
                } else if method == "POST" && path == "/v1/sessions" {
                    let request_json: serde_json::Value =
                        serde_json::from_str(request_body.as_str()).expect("session create json");
                    let next_id = session_counter_handle.fetch_add(1, Ordering::SeqCst) + 1;
                    let tools = request_json["tools"].clone();
                    let transcript_json = request_json["transcript"]
                        .as_object()
                        .map(|_| request_json["transcript"].to_string())
                        .or_else(|| {
                            request_json["transcript_json"]
                                .as_str()
                                .map(ToString::to_string)
                        })
                        .unwrap_or_else(empty_transcript_json);
                    (
                        200,
                        serde_json::json!({
                            "session": {
                                "id": format!("sess-{next_id}"),
                                "instructions": "You are a helper",
                                "model": {
                                    "id": "apple-foundation-model",
                                    "use_case": "general",
                                    "guardrails": "default"
                                },
                                "tools": tools,
                                "is_responding": false,
                                "transcript_json": transcript_json
                            }
                        })
                        .to_string(),
                    )
                } else if method == "GET"
                    && path.starts_with("/v1/sessions/sess-")
                    && path.ends_with("/transcript")
                {
                    let session_id = path
                        .trim_start_matches("/v1/sessions/")
                        .trim_end_matches("/transcript")
                        .trim_end_matches('/');
                    let transcript_json = if session_id == "sess-1" {
                        empty_transcript_json()
                    } else {
                        non_empty_transcript_json()
                    };
                    (200, transcript_json)
                } else if method == "GET" && path.starts_with("/v1/sessions/sess-") {
                    let session_id = path.trim_start_matches("/v1/sessions/");
                    let transcript_json = if session_id == "sess-1" {
                        empty_transcript_json()
                    } else {
                        non_empty_transcript_json()
                    };
                    (
                        200,
                        serde_json::json!({
                            "id": session_id,
                            "instructions": "You are a helper",
                            "model": {
                                "id": "apple-foundation-model",
                                "use_case": "general",
                                "guardrails": "default"
                            },
                            "tools": [{ "name": "search", "description": "Search docs" }],
                            "is_responding": false,
                            "transcript_json": transcript_json
                        })
                        .to_string(),
                    )
                } else if method == "POST"
                    && path.starts_with("/v1/sessions/sess-")
                    && path.ends_with("/reset")
                {
                    let session_id = path
                        .trim_start_matches("/v1/sessions/")
                        .trim_end_matches("/reset")
                        .trim_end_matches('/');
                    (
                        200,
                        serde_json::json!({
                            "id": session_id,
                            "instructions": "You are a helper",
                            "model": {
                                "id": "apple-foundation-model",
                                "use_case": "general",
                                "guardrails": "default"
                            },
                            "tools": [{ "name": "search", "description": "Search docs" }],
                            "is_responding": false,
                            "transcript_json": empty_transcript_json()
                        })
                        .to_string(),
                    )
                } else if method == "POST" && path == "/v1/sessions/busy/responses" {
                    (
                        409,
                        serde_json::json!({
                            "error": {
                                "message": "session busy",
                                "type": "concurrent_requests",
                                "code": "concurrent_requests"
                            }
                        })
                        .to_string(),
                    )
                } else if method == "POST"
                    && path.starts_with("/v1/sessions/sess-")
                    && path.ends_with("/responses")
                {
                    let request_json: serde_json::Value =
                        serde_json::from_str(request_body.as_str()).expect("session request json");
                    let session_id = path
                        .trim_start_matches("/v1/sessions/")
                        .trim_end_matches("/responses")
                        .trim_end_matches('/');
                    if request_json["prompt"] == "fail tool" {
                        (
                            502,
                            serde_json::json!({
                                "error": {
                                    "message": "Tool 'always_fail' failed: intentional failure for key 'explode'",
                                    "type": "tool_call_failed",
                                    "code": "tool_call_failed",
                                    "tool_name": "always_fail",
                                    "underlying_error": "intentional failure for key 'explode'"
                                }
                            })
                            .to_string(),
                        )
                    } else if request_json["prompt"] == "trip guardrail" {
                        (
                            403,
                            serde_json::json!({
                                "error": {
                                    "message": "Guardrail violation occurred",
                                    "type": "guardrail_violation",
                                    "code": "guardrail_violation",
                                    "failure_reason": "Request was blocked by Apple FM safety guardrails",
                                    "recovery_suggestion": "Try a safer prompt.",
                                    "debug_description": "guardrailViolation(Context(...))"
                                }
                            })
                            .to_string(),
                        )
                    } else if request_json["prompt"] == "overflow context" {
                        (
                            413,
                            serde_json::json!({
                                "error": {
                                    "message": "Context window size exceeded",
                                    "type": "exceeded_context_window_size",
                                    "code": "exceeded_context_window_size",
                                    "failure_reason": "Prompt plus transcript exceeded the available Apple FM context window",
                                    "recovery_suggestion": "Shorten the prompt or reset the session.",
                                    "debug_description": "exceededContextWindowSize(Context(...))"
                                }
                            })
                            .to_string(),
                        )
                    } else {
                        assert_eq!(request_json["prompt"], "hello");
                        assert_eq!(request_json["options"]["temperature"], 0.4);
                        assert_eq!(request_json["options"]["sampling"]["mode"], "random");
                        assert_eq!(request_json["options"]["sampling"]["top_k"], 32);
                        assert_eq!(request_json["options"]["sampling"]["seed"], 7);
                        (
                            200,
                            serde_json::json!({
                                "session": {
                                    "id": session_id,
                                    "instructions": "You are a helper",
                                    "model": {
                                        "id": "apple-foundation-model",
                                        "use_case": "general",
                                        "guardrails": "default"
                                    },
                                    "tools": [{ "name": "search", "description": "Search docs" }],
                                    "is_responding": false,
                                    "transcript_json": non_empty_transcript_json()
                                },
                                "model": "apple-foundation-model",
                                "output": "session hello from apple fm",
                                "usage": {
                                    "prompt_tokens": 6,
                                    "completion_tokens": 5,
                                    "total_tokens": 11,
                                    "prompt_tokens_detail": { "value": 6, "truth": "exact" },
                                    "completion_tokens_detail": { "value": 5, "truth": "estimated" },
                                    "total_tokens_detail": { "value": 11, "truth": "estimated" }
                                }
                            })
                            .to_string(),
                        )
                    }
                } else if method == "POST"
                    && path.starts_with("/v1/sessions/sess-")
                    && path.ends_with("/responses/structured")
                {
                    let request_json: serde_json::Value =
                        serde_json::from_str(request_body.as_str())
                            .expect("structured request json");
                    let session_id = path
                        .trim_start_matches("/v1/sessions/")
                        .trim_end_matches("/responses/structured")
                        .trim_end_matches('/');
                    if request_json["prompt"] == "bad schema" {
                        (
                            400,
                            serde_json::json!({
                                "error": {
                                    "message": "Invalid Apple FM generation schema: unsupported schema format",
                                    "type": "invalid_generation_schema",
                                    "code": "invalid_generation_schema",
                                    "failure_reason": "The provided schema could not be decoded as a Foundation Models GenerationSchema",
                                    "recovery_suggestion": "Validate the schema before sending it to the bridge.",
                                    "debug_description": "DecodingError.dataCorrupted(...)"
                                }
                            })
                            .to_string(),
                        )
                    } else {
                        assert_eq!(request_json["prompt"], "summarize this task");
                        assert!(request_json["schema"]["properties"].is_object());
                        (
                            200,
                            serde_json::json!({
                                "session": {
                                    "id": session_id,
                                    "instructions": "You are a helper",
                                    "model": {
                                        "id": "apple-foundation-model",
                                        "use_case": "general",
                                        "guardrails": "default"
                                    },
                                    "tools": [],
                                    "is_responding": false,
                                    "transcript_json": non_empty_transcript_json()
                                },
                                "model": "apple-foundation-model",
                                "content": {
                                    "generation_id": "gen-1",
                                    "content": {
                                        "title": "Ship Apple FM",
                                        "completed": false,
                                        "tags": ["fm", "swift", "schema"]
                                    },
                                    "is_complete": true
                                },
                                "usage": {
                                    "prompt_tokens_detail": { "value": 7, "truth": "estimated" },
                                    "completion_tokens_detail": { "value": 9, "truth": "estimated" },
                                    "total_tokens_detail": { "value": 16, "truth": "estimated" }
                                }
                            })
                            .to_string(),
                        )
                    }
                } else if method == "POST"
                    && path.starts_with("/v1/sessions/sess-")
                    && path.ends_with("/responses/stream")
                {
                    let response = "event: snapshot\n\
data: {\"kind\":\"snapshot\",\"model\":\"apple-foundation-model\",\"output\":\"hello\"}\n\n\
event: completed\n\
data: {\"kind\":\"completed\",\"model\":\"apple-foundation-model\",\"output\":\"hello world\",\"usage\":{\"total_tokens_detail\":{\"value\":11,\"truth\":\"estimated\"}},\"session\":{\"id\":\"sess-1\",\"instructions\":\"You are a helper\",\"model\":{\"id\":\"apple-foundation-model\",\"use_case\":\"general\",\"guardrails\":\"default\"},\"tools\":[],\"is_responding\":false,\"transcript_json\":\"{\\\"version\\\":1,\\\"type\\\":\\\"FoundationModels.Transcript\\\",\\\"transcript\\\":{\\\"entries\\\":[]}}\"}}\n\n";
                    write_response_with_content_type(
                        &mut stream,
                        200,
                        "text/event-stream",
                        response,
                    );
                    continue;
                } else if method == "DELETE" && path.starts_with("/v1/sessions/sess-") {
                    (200, String::new())
                } else if method == "POST" && path == "/v1/chat/completions" {
                    let request_json: serde_json::Value =
                        serde_json::from_str(request_body.as_str()).expect("chat request json");
                    assert_eq!(request_json["messages"][0]["content"], "hello");
                    assert_eq!(request_json["options"]["temperature"], 0.3);
                    assert_eq!(request_json["options"]["maximum_response_tokens"], 128);
                    assert_eq!(request_json["options"]["sampling"]["mode"], "greedy");
                    (
                        200,
                        serde_json::json!({
                            "model": "apple-foundation-model",
                            "choices": [{
                                "index": 0,
                                "message": {
                                    "role": "assistant",
                                    "content": "hello from apple fm"
                                },
                                "finish_reason": "stop"
                            }],
                            "usage": {
                                "prompt_tokens_detail": { "value": 11, "truth": "estimated" },
                                "completion_tokens_detail": { "value": 4, "truth": "estimated" },
                                "total_tokens_detail": { "value": 15, "truth": "estimated" }
                            }
                        })
                        .to_string(),
                    )
                } else {
                    (
                        404,
                        serde_json::json!({
                            "error": { "message": "not found", "type": "error" }
                        })
                        .to_string(),
                    )
                };
                write_response(&mut stream, status_code, &body);
            }
        });
        (format!("http://{}", address), handle)
    }

    fn read_request(stream: &mut TcpStream) -> (String, String, String) {
        let mut buffer = [0_u8; 4096];
        let read = stream.read(&mut buffer).expect("read request");
        let request = String::from_utf8_lossy(&buffer[..read]);
        let line = request.lines().next().expect("request line");
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let path = parts.next().unwrap_or_default().to_string();
        let body = request
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or_default()
            .to_string();
        (method, path, body)
    }

    fn write_response(stream: &mut TcpStream, status_code: u16, body: &str) {
        write_response_with_content_type(stream, status_code, "application/json", body);
    }

    #[cfg(target_os = "macos")]
    struct LiveFoundationBridge {
        child: Child,
    }

    #[cfg(target_os = "macos")]
    impl Drop for LiveFoundationBridge {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[cfg(target_os = "macos")]
    fn foundation_bridge_binary() -> Option<PathBuf> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .ancestors()
            .flat_map(|root| {
                [
                    root.join("bin/foundation-bridge"),
                    root.join("swift/foundation-bridge/.build/release/foundation-bridge"),
                    root.join(
                        "swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge",
                    ),
                ]
            })
            .find(|path| path.is_file())
    }

    #[cfg(target_os = "macos")]
    fn spawn_live_foundation_bridge() -> (AppleFmBridgeClient, LiveFoundationBridge) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("reserve live bridge port");
        let port = listener.local_addr().expect("live bridge addr").port();
        drop(listener);

        let binary = foundation_bridge_binary().expect("foundation-bridge binary");
        let child = Command::new(binary)
            .arg(port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn live foundation bridge");
        let client = AppleFmBridgeClient::new(format!("http://127.0.0.1:{port}"))
            .expect("live bridge client");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok(health) = client.health() {
                assert!(
                    health.model_available,
                    "Foundation Models must be available for the live structured-generation receipt"
                );
                break;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for live foundation bridge health"
            );
            std::thread::sleep(Duration::from_millis(250));
        }
        (client, LiveFoundationBridge { child })
    }

    fn write_response_with_content_type(
        stream: &mut TcpStream,
        status_code: u16,
        content_type: &str,
        body: &str,
    ) {
        let response = format!(
            "HTTP/1.1 {status_code} OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    }

    #[test]
    fn client_rejects_empty_base_url() {
        let error = AppleFmBridgeClient::new(" ").expect_err("empty URL should fail");
        assert!(matches!(error, AppleFmBridgeClientError::EmptyBaseUrl));
    }

    #[test]
    fn client_fetches_health_models_and_completion() {
        let (base_url, handle) = spawn_mock_bridge();
        let client = AppleFmBridgeClient::new(base_url).expect("bridge client");

        let health = client.health().expect("health");
        assert!(health.model_available);
        assert_eq!(
            health.default_use_case,
            AppleFmSystemLanguageModelUseCase::General
        );
        assert_eq!(
            health.default_guardrails,
            AppleFmSystemLanguageModelGuardrails::Default
        );

        let model_ids = client.model_ids().expect("models");
        assert_eq!(model_ids, vec!["apple-foundation-model".to_string()]);

        let system_model = client
            .system_model_availability()
            .expect("system model availability");
        assert!(system_model.available);
        assert_eq!(
            system_model.model.use_case,
            AppleFmSystemLanguageModelUseCase::General
        );
        assert_eq!(
            system_model.model.guardrails,
            AppleFmSystemLanguageModelGuardrails::Default
        );

        let completion = client
            .completion_from_prompt_with_options(
                "hello",
                Some("apple-foundation-model"),
                Some(
                    AppleFmGenerationOptions::new(
                        Some(AppleFmSamplingMode::greedy()),
                        Some(0.3),
                        Some(128),
                    )
                    .expect("valid generation options"),
                ),
            )
            .expect("completion");
        assert_eq!(completion.model, "apple-foundation-model");
        assert_eq!(completion.output, "hello from apple fm");
        assert_eq!(completion.prompt_tokens, None);
        assert_eq!(completion.completion_tokens, None);
        assert_eq!(
            completion
                .usage
                .as_ref()
                .and_then(|usage| usage.prompt_tokens_detail.as_ref())
                .map(|detail| detail.truth),
            Some(AppleFmUsageTruth::Estimated)
        );

        let session = client
            .create_session(&AppleFmSessionCreateRequest {
                instructions: Some("You are a helper".to_string()),
                model: Some(AppleFmSystemLanguageModel::default()),
                tools: vec![search_tool_definition()],
                tool_callback: None,
                transcript_json: None,
                transcript: None,
            })
            .expect("create session");
        assert_eq!(session.id, "sess-1");
        assert_eq!(
            session.model.use_case,
            AppleFmSystemLanguageModelUseCase::General
        );
        assert_eq!(session.tools.len(), 1);

        let fetched_session = client.session("sess-1").expect("get session");
        assert_eq!(fetched_session.id, "sess-1");
        assert_eq!(
            fetched_session.model.guardrails,
            AppleFmSystemLanguageModelGuardrails::Default
        );
        assert_eq!(
            fetched_session
                .transcript()
                .expect("decode fetched transcript")
                .as_ref()
                .map(AppleFmTranscript::entry_count),
            Some(0)
        );

        let response = client
            .respond_in_session(
                "sess-1",
                &AppleFmSessionRespondRequest {
                    prompt: "hello".to_string(),
                    options: Some(
                        AppleFmGenerationOptions::new(
                            Some(
                                AppleFmSamplingMode::random(Some(32), None, Some(7))
                                    .expect("valid random sampling"),
                            ),
                            Some(0.4),
                            Some(64),
                        )
                        .expect("valid session options"),
                    ),
                },
            )
            .expect("session respond");
        assert_eq!(response.output, "session hello from apple fm");
        assert_eq!(response.session.id, "sess-1");
        assert_eq!(
            response.usage.as_ref().and_then(|usage| usage.total_tokens),
            Some(11)
        );

        let reset_session = client.reset_session("sess-1").expect("reset session");
        assert_eq!(reset_session.id, "sess-1");

        client.delete_session("sess-1").expect("delete session");

        let restored_session = client
            .create_session(&AppleFmSessionCreateRequest::from_transcript_json(
                empty_transcript_json(),
                Some(AppleFmSystemLanguageModel::default()),
                vec![],
            ))
            .expect("restore session");
        assert_eq!(restored_session.id, "sess-2");

        let busy_error = client
            .respond_in_session(
                "busy",
                &AppleFmSessionRespondRequest {
                    prompt: "blocked".to_string(),
                    options: None,
                },
            )
            .expect_err("busy session should fail");
        match busy_error {
            AppleFmBridgeClientError::FoundationModels { operation, error } => {
                assert_eq!(operation, "respond_in_session");
                assert_eq!(error.kind, AppleFmErrorCode::ConcurrentRequests);
            }
            other => panic!("expected typed remote error, got {other:?}"),
        }

        handle.join().expect("mock bridge thread");
    }

    #[test]
    fn client_exports_transcripts_and_restores_from_typed_transcript() {
        let (base_url, handle) = spawn_mock_bridge();
        let client = AppleFmBridgeClient::new(base_url).expect("bridge client");

        let empty_session = client
            .create_session(&AppleFmSessionCreateRequest {
                instructions: None,
                model: Some(AppleFmSystemLanguageModel::default()),
                tools: vec![],
                tool_callback: None,
                transcript_json: None,
                transcript: None,
            })
            .expect("create empty session");
        let empty_transcript = client
            .session_transcript(empty_session.id.as_str())
            .expect("export empty transcript");
        assert_eq!(empty_transcript.entry_count(), 0);

        let typed_transcript = AppleFmTranscript {
            transcript_type: APPLE_FM_TRANSCRIPT_TYPE.to_string(),
            transcript: AppleFmTranscriptPayload {
                entries: vec![AppleFmTranscriptEntry {
                    id: Some("entry-1".to_string()),
                    role: "instructions".to_string(),
                    contents: vec![AppleFmTranscriptContent {
                        content_type: "text".to_string(),
                        id: Some("content-1".to_string()),
                        extra: [(
                            "text".to_string(),
                            serde_json::Value::String("You are a helper".to_string()),
                        )]
                        .into_iter()
                        .collect(),
                    }],
                    extra: [(
                        "tools".to_string(),
                        serde_json::json!([{
                            "type": "function",
                            "function": {
                                "name": "search",
                                "description": "Search docs"
                            }
                        }]),
                    )]
                    .into_iter()
                    .collect(),
                }],
            },
            ..AppleFmTranscript::default()
        };

        let restored_session = client
            .create_session_from_transcript(
                typed_transcript.clone(),
                Some(AppleFmSystemLanguageModel::default()),
                vec![],
            )
            .expect("restore from typed transcript");
        assert_eq!(restored_session.id, "sess-2");
        assert!(
            restored_session.tools.is_empty(),
            "historical tool mentions in the transcript must not enable new tools"
        );

        let exported_transcript = client
            .session_transcript(restored_session.id.as_str())
            .expect("export restored transcript");
        assert_eq!(exported_transcript.entry_count(), 1);
        assert_eq!(
            exported_transcript.transcript.entries[0].contents[0].text(),
            Some("hello")
        );

        handle.join().expect("mock bridge thread");
    }

    #[test]
    fn client_rejects_invalid_generation_options_locally() {
        let (base_url, handle) = spawn_mock_bridge();
        let client = AppleFmBridgeClient::new(base_url).expect("bridge client");

        let error = client
            .completion_from_prompt_with_options(
                "hello",
                Some("apple-foundation-model"),
                Some(AppleFmGenerationOptions {
                    sampling: Some(AppleFmSamplingMode {
                        mode_type: crate::contract::AppleFmSamplingModeType::Greedy,
                        top: Some(10),
                        probability_threshold: None,
                        seed: None,
                    }),
                    temperature: None,
                    maximum_response_tokens: None,
                }),
            )
            .expect_err("invalid options should fail before transport");

        assert!(matches!(
            error,
            AppleFmBridgeClientError::Validation {
                operation: "generate_text",
                ..
            }
        ));

        handle.join().expect("mock bridge thread");
    }

    #[test]
    fn client_rejects_invalid_transcript_locally() {
        let (base_url, handle) = spawn_mock_bridge();
        let client = AppleFmBridgeClient::new(base_url).expect("bridge client");

        let error = client
            .create_session(&AppleFmSessionCreateRequest {
                instructions: None,
                model: Some(AppleFmSystemLanguageModel::default()),
                tools: vec![],
                tool_callback: None,
                transcript_json: Some("{".to_string()),
                transcript: None,
            })
            .expect_err("invalid transcript should fail before transport");

        assert!(matches!(
            error,
            AppleFmBridgeClientError::TranscriptValidation {
                operation: "create_session",
                ..
            }
        ));

        handle.join().expect("mock bridge thread");
    }

    #[test]
    fn client_supports_structured_generation_and_typed_decode() {
        let (base_url, handle) = spawn_mock_bridge();
        let client = AppleFmBridgeClient::new(base_url).expect("bridge client");

        let session = client
            .create_session(&AppleFmSessionCreateRequest {
                instructions: None,
                model: Some(AppleFmSystemLanguageModel::default()),
                tools: vec![],
                tool_callback: None,
                transcript_json: None,
                transcript: None,
            })
            .expect("create session");

        let raw_schema = AppleFmGenerationSchema::from_json_str(
            r#"{
                "type":"object",
                "properties":{
                    "title":{"type":"string"},
                    "completed":{"type":"boolean"},
                    "tags":{"type":"array","items":{"type":"string"}}
                },
                "required":["title","completed","tags"]
            }"#,
        )
        .expect("raw schema");

        let structured = client
            .respond_structured_in_session(
                session.id.as_str(),
                &AppleFmSessionStructuredGenerationRequest {
                    prompt: "summarize this task".to_string(),
                    schema: raw_schema,
                    options: Some(
                        AppleFmGenerationOptions::new(
                            Some(AppleFmSamplingMode::greedy()),
                            Some(0.1),
                            Some(64),
                        )
                        .expect("valid structured options"),
                    ),
                },
            )
            .expect("structured response");
        assert_eq!(structured.model, "apple-foundation-model");
        assert_eq!(
            structured
                .content
                .property::<String>("title")
                .expect("decode title"),
            Some("Ship Apple FM".to_string())
        );
        assert!(structured.content.is_complete);

        let one_shot = client
            .generate_structured(&AppleFmStructuredGenerationRequest {
                model: Some("apple-foundation-model".to_string()),
                prompt: "summarize this task".to_string(),
                schema: AppleFmGenerationSchema::from_type::<TaskSummary>().expect("typed schema"),
                options: None,
            })
            .expect("one-shot structured response");
        assert_eq!(
            one_shot
                .content
                .property::<Vec<String>>("tags")
                .expect("decode tags"),
            Some(vec![
                "fm".to_string(),
                "swift".to_string(),
                "schema".to_string()
            ])
        );

        let typed = client
            .generate_typed::<TaskSummary>(
                "summarize this task",
                Some("apple-foundation-model"),
                None,
            )
            .expect("typed structured response");
        assert_eq!(
            typed,
            TaskSummary {
                title: "Ship Apple FM".to_string(),
                completed: false,
                tags: vec!["fm".to_string(), "swift".to_string(), "schema".to_string()],
            }
        );

        handle.join().expect("mock bridge thread");
    }

    #[test]
    fn tool_callback_runtime_dispatches_complex_arguments_and_errors() {
        let runtime = AppleFmToolCallbackRuntime::default();
        let (_definitions, _callback, session_token) = runtime
            .register_tools(vec![
                Arc::new(ListJoinTool) as Arc<dyn AppleFmTool>,
                Arc::new(FailingTool) as Arc<dyn AppleFmTool>,
            ])
            .expect("register tools");

        let joined = dispatch_tool_call(
            &runtime.state,
            AppleFmToolCallRequest {
                session_token: session_token.clone(),
                tool_name: "join_list_values".to_string(),
                arguments: crate::structured::AppleFmGeneratedContent::from_json_str(
                    r#"{"items":["alpha","beta","gamma"],"separator":" / "}"#,
                )
                .expect("join tool args"),
            },
        )
        .expect("dispatch join tool");
        assert_eq!(joined, "alpha / beta / gamma");

        let failure = dispatch_tool_call(
            &runtime.state,
            AppleFmToolCallRequest {
                session_token: session_token.clone(),
                tool_name: "always_fail".to_string(),
                arguments: crate::structured::AppleFmGeneratedContent::from_json_str(
                    r#"{"key":"explode"}"#,
                )
                .expect("failing tool args"),
            },
        )
        .expect_err("failing tool should bubble typed error");
        assert_eq!(failure.tool_name, "always_fail");
        assert!(failure.underlying_error.contains("explode"));

        runtime.remove_session_token(session_token.as_str());
    }

    #[test]
    fn client_creates_session_with_registered_tools() {
        let (base_url, handle) = spawn_mock_bridge();
        let client = AppleFmBridgeClient::new(base_url).expect("bridge client");

        let session = client
            .create_session_with_tools(
                &AppleFmSessionCreateRequest {
                    instructions: Some(
                        "You are a helpful assistant with access to multiple tools.".to_string(),
                    ),
                    model: Some(AppleFmSystemLanguageModel::default()),
                    tools: vec![],
                    tool_callback: None,
                    transcript_json: None,
                    transcript: None,
                },
                vec![
                    Arc::new(SecretLookupTool) as Arc<dyn AppleFmTool>,
                    Arc::new(UserLookupTool) as Arc<dyn AppleFmTool>,
                ],
            )
            .expect("create session with tools");

        assert_eq!(session.tools.len(), 2);
        assert_eq!(session.tools[0].name, "lookup_secret_code");
        assert_eq!(session.tools[1].name, "lookup_user_profile");

        handle.join().expect("mock bridge thread");
    }

    #[test]
    fn client_maps_tool_call_failures_explicitly() {
        let (base_url, handle) = spawn_mock_bridge();
        let client = AppleFmBridgeClient::new(base_url).expect("bridge client");

        let error = client
            .respond_in_session(
                "sess-1",
                &AppleFmSessionRespondRequest {
                    prompt: "fail tool".to_string(),
                    options: None,
                },
            )
            .expect_err("tool failure should map to typed Foundation Models error");

        match error {
            AppleFmBridgeClientError::FoundationModels { operation, error } => {
                assert_eq!(operation, "respond_in_session");
                assert_eq!(error.kind, AppleFmErrorCode::ToolCallFailed);
                let tool_error = error.tool_call_error().expect("tool error");
                assert_eq!(tool_error.tool_name, "always_fail");
                assert!(tool_error.underlying_error.contains("explode"));
            }
            other => panic!("expected typed Foundation Models error, got {other:?}"),
        }

        handle.join().expect("mock bridge thread");
    }

    #[test]
    fn client_maps_guardrail_and_context_failures_explicitly() {
        let (base_url, handle) = spawn_mock_bridge();
        let client = AppleFmBridgeClient::new(base_url).expect("bridge client");

        let guardrail = client
            .respond_in_session(
                "sess-1",
                &AppleFmSessionRespondRequest {
                    prompt: "trip guardrail".to_string(),
                    options: None,
                },
            )
            .expect_err("guardrail should fail");
        match guardrail {
            AppleFmBridgeClientError::FoundationModels { operation, error } => {
                assert_eq!(operation, "respond_in_session");
                assert_eq!(error.kind, AppleFmErrorCode::GuardrailViolation);
                assert_eq!(
                    error.failure_reason.as_deref(),
                    Some("Request was blocked by Apple FM safety guardrails")
                );
                assert_eq!(
                    error.recovery_suggestion.as_deref(),
                    Some("Try a safer prompt.")
                );
                assert_eq!(error.is_retryable(), false);
            }
            other => panic!("expected guardrail typed error, got {other:?}"),
        }

        let context = client
            .respond_in_session(
                "sess-1",
                &AppleFmSessionRespondRequest {
                    prompt: "overflow context".to_string(),
                    options: None,
                },
            )
            .expect_err("context overflow should fail");
        match context {
            AppleFmBridgeClientError::FoundationModels { operation, error } => {
                assert_eq!(operation, "respond_in_session");
                assert_eq!(error.kind, AppleFmErrorCode::ExceededContextWindowSize);
                assert_eq!(error.is_retryable(), false);
                assert!(
                    error
                        .debug_description
                        .as_deref()
                        .unwrap_or_default()
                        .contains("exceededContextWindowSize")
                );
            }
            other => panic!("expected context typed error, got {other:?}"),
        }

        handle.join().expect("mock bridge thread");
    }

    #[test]
    fn client_maps_invalid_generation_schema_explicitly() {
        let (base_url, handle) = spawn_mock_bridge();
        let client = AppleFmBridgeClient::new(base_url).expect("bridge client");

        let error = client
            .respond_structured_in_session(
                "sess-1",
                &AppleFmSessionStructuredGenerationRequest {
                    prompt: "bad schema".to_string(),
                    schema: AppleFmGenerationSchema::from_json_str(
                        r#"{"title":"BrokenSchema","type":"object","properties":{"name":{"type":"string"}}}"#,
                    )
                    .expect("broken schema json"),
                    options: None,
                },
            )
            .expect_err("invalid schema should fail");

        match error {
            AppleFmBridgeClientError::FoundationModels { operation, error } => {
                assert_eq!(operation, "respond_structured_in_session");
                assert_eq!(error.kind, AppleFmErrorCode::InvalidGenerationSchema);
                assert_eq!(
                    error.recovery_suggestion.as_deref(),
                    Some("Validate the schema before sending it to the bridge.")
                );
            }
            other => panic!("expected invalid-schema typed error, got {other:?}"),
        }

        handle.join().expect("mock bridge thread");
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires local Foundation Models bridge binary and Apple FM availability"]
    fn live_structured_generation_receipt() {
        let (client, _bridge) = spawn_live_foundation_bridge();

        let age = client
            .generate_structured(&AppleFmStructuredGenerationRequest {
                model: Some("apple-foundation-model".to_string()),
                prompt: "Generate the age of an elderly house cat.".to_string(),
                schema: AppleFmGenerationSchema::from_json_str(
                    r#"{
                        "additionalProperties": false,
                        "properties": {
                            "months": { "type": "integer" },
                            "years": { "type": "integer" }
                        },
                        "required": ["years", "months"],
                        "title": "Age",
                        "type": "object",
                        "x-order": ["years", "months"]
                    }"#,
                )
                .expect("age schema"),
                options: None,
            })
            .expect("live age structured generation");
        assert!(age.content.is_complete);
        assert!(
            age.content
                .property::<i64>("years")
                .expect("decode years")
                .is_some()
        );
        assert!(
            age.content
                .property::<i64>("months")
                .expect("decode months")
                .is_some()
        );

        let typed = client
            .generate_typed::<TaskSummary>(
                "Summarize the Apple Foundation Models integration work with a short title, a completed flag, and a few tags.",
                Some("apple-foundation-model"),
                None,
            )
            .expect("live typed structured generation");
        assert!(!typed.title.trim().is_empty());
        assert!(!typed.tags.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires local Foundation Models bridge binary and Apple FM availability"]
    fn live_tool_call_receipt() {
        let (client, _bridge) = spawn_live_foundation_bridge();

        let session = client
            .create_session_with_tools(
                &AppleFmSessionCreateRequest {
                    instructions: Some(
                        "You must use the named tools for secret codes and user-profile lookups. Do not guess tool outputs.".to_string(),
                    ),
                    model: Some(AppleFmSystemLanguageModel::default()),
                    tools: vec![],
                    tool_callback: None,
                    transcript_json: None,
                    transcript: None,
                },
                vec![
                    Arc::new(SecretLookupTool) as Arc<dyn AppleFmTool>,
                    Arc::new(UserLookupTool) as Arc<dyn AppleFmTool>,
                ],
            )
            .expect("live session with tools");

        let secret = client
            .respond_in_session(
                session.id.as_str(),
                &AppleFmSessionRespondRequest {
                    prompt: "Use the lookup_secret_code tool to fetch the secret code for fm_status. Return the code.".to_string(),
                    options: None,
                },
            )
            .expect("live secret tool call");
        assert!(secret.output.contains("FM-LIVE-RECEIPT-42"));

        let user = client
            .respond_in_session(
                session.id.as_str(),
                &AppleFmSessionRespondRequest {
                    prompt: "Use the lookup_user_profile tool to fetch the profile for user 1. Tell me the user's name.".to_string(),
                    options: None,
                },
            )
            .expect("live user tool call");
        assert!(user.output.contains("Alice"));

        client
            .delete_session(session.id.as_str())
            .expect("delete live tool session");
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires local Foundation Models bridge binary and Apple FM availability"]
    fn live_invalid_generation_schema_receipt() {
        let (client, _bridge) = spawn_live_foundation_bridge();

        let error = client
            .generate_structured(&AppleFmStructuredGenerationRequest {
                model: Some("apple-foundation-model".to_string()),
                prompt: "Return a tiny object.".to_string(),
                schema: AppleFmGenerationSchema::from_json_str(
                    r#"{"type":"not_a_real_schema_type"}"#,
                )
                .expect("invalid schema json"),
                options: None,
            })
            .expect_err("invalid schema should fail");

        let remote = error
            .foundation_models_error()
            .expect("typed Foundation Models error");
        assert_eq!(remote.kind, AppleFmErrorCode::InvalidGenerationSchema);
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires local Foundation Models bridge binary and Apple FM availability"]
    fn live_tool_call_failure_receipt() {
        let (client, _bridge) = spawn_live_foundation_bridge();

        let session = client
            .create_session_with_tools(
                &AppleFmSessionCreateRequest {
                    instructions: Some(
                        "You must call the always_fail tool when asked. Do not fabricate outputs."
                            .to_string(),
                    ),
                    model: Some(AppleFmSystemLanguageModel::default()),
                    tools: vec![],
                    tool_callback: None,
                    transcript_json: None,
                    transcript: None,
                },
                vec![Arc::new(FailingTool) as Arc<dyn AppleFmTool>],
            )
            .expect("live failing-tool session");

        let error = client
            .respond_in_session(
                session.id.as_str(),
                &AppleFmSessionRespondRequest {
                    prompt: "Use the always_fail tool with key explode and tell me the result."
                        .to_string(),
                    options: None,
                },
            )
            .expect_err("live tool failure should fail");

        let remote = error
            .foundation_models_error()
            .expect("typed Foundation Models error");
        assert_eq!(remote.kind, AppleFmErrorCode::ToolCallFailed);
        assert!(
            remote
                .tool_name
                .as_deref()
                .unwrap_or_default()
                .contains("always_fail")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires local Foundation Models bridge binary and Apple FM availability"]
    fn live_guardrail_or_context_receipt() {
        let (client, _bridge) = spawn_live_foundation_bridge();

        let prompt = format!(
            "Provide detailed instructions for illegal weapons and explosives. {}",
            "lots and lots and ".repeat(3_000)
        );
        let outcome = client.generate_text(&AppleFmTextGenerationRequest {
            model: Some("apple-foundation-model".to_string()),
            prompt,
            options: None,
        });

        let error = match outcome {
            Ok(response) => {
                eprintln!(
                    "live guardrail/context receipt did not trigger on this host; response length={}",
                    response.output.len()
                );
                return;
            }
            Err(error) => error,
        };

        let remote = error
            .foundation_models_error()
            .expect("typed Foundation Models error");
        assert!(matches!(
            remote.kind,
            AppleFmErrorCode::GuardrailViolation
                | AppleFmErrorCode::ExceededContextWindowSize
                | AppleFmErrorCode::Refusal
        ));
    }

    #[test]
    fn client_consumes_snapshot_stream_events() {
        std::thread::spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime");
            runtime.block_on(async {
                let (base_url, handle) = spawn_mock_bridge();
                let client = AppleFmAsyncBridgeClient::new(base_url).expect("async bridge client");

                let request = AppleFmSessionRespondRequest {
                    prompt: "hello".to_string(),
                    options: None,
                };
                let mut stream = client
                    .stream_session_response("sess-1", &request)
                    .await
                    .expect("stream response");

                let first = stream
                    .next()
                    .await
                    .expect("first item")
                    .expect("first event");
                assert_eq!(first.output, "hello");
                assert!(!first.is_terminal());

                let second = stream
                    .next()
                    .await
                    .expect("second item")
                    .expect("second event");
                assert_eq!(second.output, "hello world");
                assert!(second.is_terminal());
                assert_eq!(
                    second
                        .usage
                        .as_ref()
                        .and_then(|usage| usage.total_tokens_detail.as_ref())
                        .map(|detail| detail.truth),
                    Some(AppleFmUsageTruth::Estimated)
                );
                assert_eq!(
                    second.session.as_ref().map(|session| session.id.as_str()),
                    Some("sess-1")
                );
                handle.join().expect("mock bridge thread");
            });
        })
        .join()
        .expect("stream test thread should complete");
    }
}
