use reqwest::Url;
use reqwest::blocking::Client;
use thiserror::Error;

use crate::contract::{
    APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH, APPLE_FM_BRIDGE_HEALTH_PATH,
    APPLE_FM_BRIDGE_MODELS_PATH, AppleFmChatCompletionRequest, AppleFmChatCompletionResponse,
    AppleFmCompletionResult, AppleFmHealthResponse, AppleFmModelsResponse,
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
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use super::{AppleFmBridgeClient, AppleFmBridgeClientError};

    fn spawn_mock_bridge() -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock bridge");
        listener
            .set_nonblocking(true)
            .expect("set mock bridge nonblocking");
        let address = listener.local_addr().expect("bridge addr");
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
                let body = match (method.as_str(), path.as_str()) {
                    ("GET", "/health") => serde_json::json!({
                        "status": "ok",
                        "model_available": true,
                        "availability_message": "Foundation Models is available"
                    })
                    .to_string(),
                    ("GET", "/v1/models") => serde_json::json!({
                        "object": "list",
                        "data": [{ "id": "apple-foundation-model", "object": "model" }]
                    })
                    .to_string(),
                    ("POST", "/v1/chat/completions") => serde_json::json!({
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
                    _ => serde_json::json!({
                        "error": { "message": "not found", "type": "error" }
                    })
                    .to_string(),
                };
                write_response(&mut stream, &body);
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

    fn write_response(stream: &mut TcpStream, body: &str) {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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

        let model_ids = client.model_ids().expect("models");
        assert_eq!(model_ids, vec!["apple-foundation-model".to_string()]);

        let completion = client
            .completion_from_prompt("hello", Some("apple-foundation-model"), Some(128), None)
            .expect("completion");
        assert_eq!(completion.model, "apple-foundation-model");
        assert_eq!(completion.output, "hello from apple fm");
        assert_eq!(completion.prompt_tokens, Some(11));
        assert_eq!(completion.completion_tokens, Some(4));

        handle.join().expect("mock bridge thread");
    }
}
