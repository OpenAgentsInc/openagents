use reqwest::Url;
use reqwest::blocking::Client;
use thiserror::Error;

use crate::contract::{
    APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH, APPLE_FM_BRIDGE_HEALTH_PATH,
    APPLE_FM_BRIDGE_MODELS_PATH, APPLE_FM_BRIDGE_SESSIONS_PATH, AppleFmChatCompletionRequest,
    AppleFmChatCompletionResponse, AppleFmCompletionResult, AppleFmHealthResponse,
    AppleFmModelsResponse, AppleFmSession, AppleFmSessionCreateRequest,
    AppleFmSessionCreateResponse, AppleFmSessionRespondRequest, AppleFmSessionRespondResponse,
    AppleFmSystemLanguageModelAvailability,
};

/// Reusable blocking client for the current Apple FM bridge contract.
#[derive(Clone, Debug)]
pub struct AppleFmBridgeClient {
    base_url: String,
    client: Client,
}

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

    /// Fetches bridge health.
    pub fn health(&self) -> Result<AppleFmHealthResponse, AppleFmBridgeClientError> {
        self.client
            .get(self.endpoint(APPLE_FM_BRIDGE_HEALTH_PATH)?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "health",
                error: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| AppleFmBridgeClientError::Status {
                operation: "health",
                error: error.to_string(),
            })?
            .json::<AppleFmHealthResponse>()
            .map_err(|error| AppleFmBridgeClientError::Decode {
                operation: "health",
                error: error.to_string(),
            })
    }

    /// Fetches the full model-list response.
    pub fn list_models(&self) -> Result<AppleFmModelsResponse, AppleFmBridgeClientError> {
        self.client
            .get(self.endpoint(APPLE_FM_BRIDGE_MODELS_PATH)?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "models",
                error: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| AppleFmBridgeClientError::Status {
                operation: "models",
                error: error.to_string(),
            })?
            .json::<AppleFmModelsResponse>()
            .map_err(|error| AppleFmBridgeClientError::Decode {
                operation: "models",
                error: error.to_string(),
            })
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
        self.client
            .post(self.endpoint(APPLE_FM_BRIDGE_SESSIONS_PATH)?)
            .json(request)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "create_session",
                error: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| AppleFmBridgeClientError::Status {
                operation: "create_session",
                error: error.to_string(),
            })?
            .json::<AppleFmSessionCreateResponse>()
            .map(|response| response.session)
            .map_err(|error| AppleFmBridgeClientError::Decode {
                operation: "create_session",
                error: error.to_string(),
            })
    }

    /// Fetches current session state.
    pub fn session(&self, session_id: &str) -> Result<AppleFmSession, AppleFmBridgeClientError> {
        self.client
            .get(self.endpoint(&session_path(session_id))?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "session",
                error: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| AppleFmBridgeClientError::Status {
                operation: "session",
                error: error.to_string(),
            })?
            .json::<AppleFmSession>()
            .map_err(|error| AppleFmBridgeClientError::Decode {
                operation: "session",
                error: error.to_string(),
            })
    }

    /// Deletes a session handle.
    pub fn delete_session(&self, session_id: &str) -> Result<(), AppleFmBridgeClientError> {
        self.client
            .delete(self.endpoint(&session_path(session_id))?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "delete_session",
                error: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| AppleFmBridgeClientError::Status {
                operation: "delete_session",
                error: error.to_string(),
            })?;
        Ok(())
    }

    /// Resets a session after failure/cancellation semantics.
    pub fn reset_session(
        &self,
        session_id: &str,
    ) -> Result<AppleFmSession, AppleFmBridgeClientError> {
        self.client
            .post(self.endpoint(&session_reset_path(session_id))?)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "reset_session",
                error: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| AppleFmBridgeClientError::Status {
                operation: "reset_session",
                error: error.to_string(),
            })?
            .json::<AppleFmSession>()
            .map_err(|error| AppleFmBridgeClientError::Decode {
                operation: "reset_session",
                error: error.to_string(),
            })
    }

    /// Executes a prompt inside a persistent Apple FM session.
    pub fn respond_in_session(
        &self,
        session_id: &str,
        request: &AppleFmSessionRespondRequest,
    ) -> Result<AppleFmSessionRespondResponse, AppleFmBridgeClientError> {
        self.client
            .post(self.endpoint(&session_responses_path(session_id))?)
            .json(request)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "respond_in_session",
                error: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| AppleFmBridgeClientError::Status {
                operation: "respond_in_session",
                error: error.to_string(),
            })?
            .json::<AppleFmSessionRespondResponse>()
            .map_err(|error| AppleFmBridgeClientError::Decode {
                operation: "respond_in_session",
                error: error.to_string(),
            })
    }

    /// Executes a raw chat-completion request against the bridge.
    pub fn chat_completion(
        &self,
        request: &AppleFmChatCompletionRequest,
    ) -> Result<AppleFmChatCompletionResponse, AppleFmBridgeClientError> {
        self.client
            .post(self.endpoint(APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH)?)
            .json(request)
            .send()
            .map_err(|error| AppleFmBridgeClientError::Transport {
                operation: "chat_completion",
                error: error.to_string(),
            })?
            .error_for_status()
            .map_err(|error| AppleFmBridgeClientError::Status {
                operation: "chat_completion",
                error: error.to_string(),
            })?
            .json::<AppleFmChatCompletionResponse>()
            .map_err(|error| AppleFmBridgeClientError::Decode {
                operation: "chat_completion",
                error: error.to_string(),
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
        let request =
            AppleFmChatCompletionRequest::from_user_prompt(prompt, model, max_tokens, temperature);
        Ok(self.chat_completion(&request)?.completion_result())
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
    /// Response decode failed.
    #[error("Apple FM {operation} decode failed: {error}")]
    Decode {
        /// Bridge operation label.
        operation: &'static str,
        /// Error detail.
        error: String,
    },
}

#[cfg(test)]
mod tests {
    use std::io::{ErrorKind, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use super::{AppleFmBridgeClient, AppleFmBridgeClientError};
    use crate::contract::{
        AppleFmSessionCreateRequest, AppleFmSessionRespondRequest, AppleFmSessionToolMetadata,
        AppleFmSystemLanguageModel, AppleFmSystemLanguageModelGuardrails,
        AppleFmSystemLanguageModelUseCase,
    };

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
                let (method, path) = read_request_line(&mut stream);
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
                    let next_id = session_counter_handle.fetch_add(1, Ordering::SeqCst) + 1;
                    (200, serde_json::json!({
                        "session": {
                            "id": format!("sess-{next_id}"),
                            "instructions": "You are a helper",
                            "model": {
                                "id": "apple-foundation-model",
                                "use_case": "general",
                                "guardrails": "default"
                            },
                            "tools": [{ "name": "search", "description": "Search docs" }],
                            "is_responding": false,
                            "transcript_json": "{\"type\":\"FoundationModels.Transcript\",\"entries\":[]}"
                        }
                    })
                    .to_string())
                } else if method == "GET" && path.starts_with("/v1/sessions/sess-") {
                    let session_id = path.trim_start_matches("/v1/sessions/");
                    (200, serde_json::json!({
                        "id": session_id,
                        "instructions": "You are a helper",
                        "model": {
                            "id": "apple-foundation-model",
                            "use_case": "general",
                            "guardrails": "default"
                        },
                        "tools": [{ "name": "search", "description": "Search docs" }],
                        "is_responding": false,
                        "transcript_json": "{\"type\":\"FoundationModels.Transcript\",\"entries\":[]}"
                    })
                    .to_string())
                } else if method == "POST"
                    && path.starts_with("/v1/sessions/sess-")
                    && path.ends_with("/reset")
                {
                    let session_id = path
                        .trim_start_matches("/v1/sessions/")
                        .trim_end_matches("/reset")
                        .trim_end_matches('/');
                    (200, serde_json::json!({
                        "id": session_id,
                        "instructions": "You are a helper",
                        "model": {
                            "id": "apple-foundation-model",
                            "use_case": "general",
                            "guardrails": "default"
                        },
                        "tools": [{ "name": "search", "description": "Search docs" }],
                        "is_responding": false,
                        "transcript_json": "{\"type\":\"FoundationModels.Transcript\",\"entries\":[]}"
                    })
                    .to_string())
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
                    let session_id = path
                        .trim_start_matches("/v1/sessions/")
                        .trim_end_matches("/responses")
                        .trim_end_matches('/');
                    (200, serde_json::json!({
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
                            "transcript_json": "{\"type\":\"FoundationModels.Transcript\",\"entries\":[{\"role\":\"user\",\"content\":\"hello\"}]}"
                        },
                        "model": "apple-foundation-model",
                        "output": "session hello from apple fm",
                        "usage": {
                            "prompt_tokens": 6,
                            "completion_tokens": 5,
                            "total_tokens": 11
                        }
                    })
                    .to_string())
                } else if method == "DELETE" && path.starts_with("/v1/sessions/sess-") {
                    (200, String::new())
                } else if method == "POST" && path == "/v1/chat/completions" {
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
                                "prompt_tokens": 11,
                                "completion_tokens": 4,
                                "total_tokens": 15
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

    fn read_request_line(stream: &mut TcpStream) -> (String, String) {
        let mut buffer = [0_u8; 4096];
        let read = stream.read(&mut buffer).expect("read request");
        let request = String::from_utf8_lossy(&buffer[..read]);
        let line = request.lines().next().expect("request line");
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let path = parts.next().unwrap_or_default().to_string();
        (method, path)
    }

    fn write_response(stream: &mut TcpStream, status_code: u16, body: &str) {
        let response = format!(
            "HTTP/1.1 {status_code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
            .completion_from_prompt("hello", Some("apple-foundation-model"), Some(128), None)
            .expect("completion");
        assert_eq!(completion.model, "apple-foundation-model");
        assert_eq!(completion.output, "hello from apple fm");
        assert_eq!(completion.prompt_tokens, Some(11));
        assert_eq!(completion.completion_tokens, Some(4));

        let session = client
            .create_session(&AppleFmSessionCreateRequest {
                instructions: Some("You are a helper".to_string()),
                model: Some(AppleFmSystemLanguageModel::default()),
                tools: vec![AppleFmSessionToolMetadata {
                    name: "search".to_string(),
                    description: Some("Search docs".to_string()),
                }],
                transcript_json: None,
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

        let response = client
            .respond_in_session(
                "sess-1",
                &AppleFmSessionRespondRequest {
                    prompt: "hello".to_string(),
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
                "{\"type\":\"FoundationModels.Transcript\",\"entries\":[]}",
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
                },
            )
            .expect_err("busy session should fail");
        assert!(matches!(
            busy_error,
            AppleFmBridgeClientError::Status {
                operation: "respond_in_session",
                ..
            }
        ));

        handle.join().expect("mock bridge thread");
    }
}
