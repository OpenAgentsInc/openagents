//! A small client for OpenRouter's chat completions API, built for one job:
//! send a prompt and get back a JSON object that matches a schema.
//!
//! It's a Rust reimplementation of the parts of OpenRouter's TypeScript SDK
//! (<https://github.com/OpenRouterTeam/typescript-sdk>, Apache-2.0) that
//! this needs: the chat request, the `json_schema` response format
//! (`ChatFormatJsonSchemaConfig` and `ChatJsonSchemaConfig` there), the
//! result's usage and cost, and the SDK's error classes. Streaming, tools,
//! and every other endpoint are left out.
//!
//! ```no_run
//! # async fn run() -> Result<(), openrouter::Error> {
//! use openrouter::{ChatRequest, Client, Config, Message};
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Answer { text: String }
//!
//! let client = Client::new(Config::from_env()?)?;
//! let schema = serde_json::json!({
//!     "type": "object",
//!     "properties": { "text": { "type": "string" } },
//!     "required": ["text"],
//!     "additionalProperties": false,
//! });
//! let request = ChatRequest::new("openai/gpt-6-luna", vec![Message::user("Say hi.")]);
//! let answer = client.structured::<Answer>(request, "answer", schema).await?;
//! println!("{} (${:.5})", answer.value.text, answer.usage.cost.unwrap_or(0.0));
//! # Ok(())
//! # }
//! ```
//!
//! The key comes from `OPENROUTER_API_KEY`, or else from `api_key` in
//! `~/.openagents/openrouter.json`. It never appears in a `Debug` string, an
//! error, or a log line.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// OpenRouter's API base URL.
pub const BASE_URL: &str = "https://openrouter.ai/api/v1";

/// The variable that holds the key.
pub const KEY_VAR: &str = "OPENROUTER_API_KEY";

/// Seconds one attempt may take, by default.
pub const TIMEOUT: Duration = Duration::from_secs(300);

/// Retries after the first attempt, by default.
pub const RETRIES: u32 = 2;

/// An API key that never prints.
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    /// A key. Surrounding whitespace is dropped.
    #[must_use]
    pub fn new(key: &str) -> Self {
        ApiKey(key.trim().to_string())
    }

    /// The key's text, for the request header only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

/// The client's settings.
#[derive(Clone, Debug)]
pub struct Config {
    pub api_key: ApiKey,
    pub base_url: String,
    /// The `HTTP-Referer` attribution header, when set.
    pub referer: Option<String>,
    /// The `X-Title` attribution header, when set.
    pub title: Option<String>,
    pub timeout: Duration,
    pub retries: u32,
}

impl Config {
    /// Settings with `key` and the defaults.
    #[must_use]
    pub fn new(key: ApiKey) -> Self {
        Config {
            api_key: key,
            base_url: BASE_URL.to_string(),
            referer: None,
            title: Some("OpenAgents".to_string()),
            timeout: TIMEOUT,
            retries: RETRIES,
        }
    }

    /// Settings with the key from `OPENROUTER_API_KEY`, or else from
    /// `api_key` in `~/.openagents/openrouter.json`.
    ///
    /// # Errors
    ///
    /// [`Error::NoKey`] when neither holds a key.
    pub fn from_env() -> Result<Self, Error> {
        if let Ok(key) = std::env::var(KEY_VAR)
            && !key.trim().is_empty()
        {
            return Ok(Config::new(ApiKey::new(&key)));
        }
        let file = key_file();
        let key = file
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| value["api_key"].as_str().map(str::to_string))
            .filter(|key| !key.trim().is_empty());
        match key {
            Some(key) => Ok(Config::new(ApiKey::new(&key))),
            None => Err(Error::NoKey),
        }
    }

    /// The same settings with another base URL.
    #[must_use]
    pub fn base_url(mut self, url: &str) -> Self {
        self.base_url = url.trim_end_matches('/').to_string();
        self
    }
}

/// `~/.openagents/openrouter.json`.
#[must_use]
pub fn key_file() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/openrouter.json"))
}

/// One message.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Message {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Message {
            role: "user".to_string(),
            content: content.into(),
        }
    }
}

/// `response_format` with `type: "json_schema"`.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub kind: String,
    pub json_schema: JsonSchema,
}

/// The schema a structured reply must match.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct JsonSchema {
    pub name: String,
    pub schema: Value,
    pub strict: bool,
}

/// Reasoning settings, for a model that reasons.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Reasoning {
    /// `low`, `medium`, or `high`.
    pub effort: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct UsageRequest {
    include: bool,
}

/// One chat completions request, not streamed.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Reasoning>,
    stream: bool,
    usage: UsageRequest,
}

impl ChatRequest {
    /// A request to `model` with `messages`, asking OpenRouter to report the
    /// call's cost.
    #[must_use]
    pub fn new(model: &str, messages: Vec<Message>) -> Self {
        ChatRequest {
            model: model.to_string(),
            messages,
            response_format: None,
            temperature: None,
            max_tokens: None,
            reasoning: None,
            stream: false,
            usage: UsageRequest { include: true },
        }
    }

    /// The same request with a reasoning effort.
    #[must_use]
    pub fn effort(mut self, effort: &str) -> Self {
        self.reasoning = Some(Reasoning {
            effort: effort.to_string(),
        });
        self
    }

    /// The same request with a token limit on the reply.
    #[must_use]
    pub fn max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = Some(max);
        self
    }
}

/// Token counts and cost, as OpenRouter reports them.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    /// The call's cost in dollars, when OpenRouter reports it.
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub completion_tokens_details: Option<CompletionDetails>,
}

/// The completion's breakdown.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct CompletionDetails {
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
}

/// One choice.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Choice {
    pub message: ReplyMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// The reply's message.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ReplyMessage {
    #[serde(default)]
    pub content: Option<String>,
}

/// A chat completions response.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ChatResponse {
    #[serde(default)]
    pub id: String,
    /// The model OpenRouter used.
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Usage,
}

/// A structured reply: the parsed value and what the call cost.
#[derive(Clone, Debug)]
pub struct Structured<T> {
    pub value: T,
    /// The reply's text, as the model wrote it.
    pub raw: String,
    pub model: String,
    pub finish_reason: Option<String>,
    pub usage: Usage,
    /// Milliseconds the call took, retries included.
    pub milliseconds: u64,
}

/// What went wrong.
#[derive(Debug)]
pub enum Error {
    /// No key in `OPENROUTER_API_KEY` or `~/.openagents/openrouter.json`.
    NoKey,
    /// The HTTP client couldn't be built.
    Client(String),
    /// OpenRouter answered with an error status.
    Api {
        kind: ApiErrorKind,
        status: u16,
        message: String,
    },
    /// The request didn't reach OpenRouter, or the connection broke.
    Connection(String),
    /// An attempt ran past its time limit.
    Timeout,
    /// The response wasn't a chat completion, or had no reply text.
    Decode { detail: String, excerpt: String },
    /// The reply text doesn't match the requested shape.
    Schema { detail: String, excerpt: String },
}

/// The error classes of OpenRouter's SDK, by status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiErrorKind {
    BadRequest,
    Unauthorized,
    PaymentRequired,
    Forbidden,
    NotFound,
    RequestTimeout,
    PayloadTooLarge,
    UnprocessableEntity,
    TooManyRequests,
    InternalServer,
    BadGateway,
    ServiceUnavailable,
    GatewayTimeout,
    Other,
}

impl ApiErrorKind {
    /// The class of `status`.
    #[must_use]
    pub fn of(status: u16) -> Self {
        match status {
            400 => ApiErrorKind::BadRequest,
            401 => ApiErrorKind::Unauthorized,
            402 => ApiErrorKind::PaymentRequired,
            403 => ApiErrorKind::Forbidden,
            404 => ApiErrorKind::NotFound,
            408 => ApiErrorKind::RequestTimeout,
            413 => ApiErrorKind::PayloadTooLarge,
            422 => ApiErrorKind::UnprocessableEntity,
            429 => ApiErrorKind::TooManyRequests,
            500 => ApiErrorKind::InternalServer,
            502 => ApiErrorKind::BadGateway,
            503 => ApiErrorKind::ServiceUnavailable,
            504 => ApiErrorKind::GatewayTimeout,
            _ => ApiErrorKind::Other,
        }
    }

    /// Whether a request with this status is worth trying again.
    #[must_use]
    pub fn retryable(self) -> bool {
        matches!(
            self,
            ApiErrorKind::TooManyRequests
                | ApiErrorKind::InternalServer
                | ApiErrorKind::BadGateway
                | ApiErrorKind::ServiceUnavailable
                | ApiErrorKind::GatewayTimeout
                | ApiErrorKind::RequestTimeout
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoKey => write!(
                f,
                "no OpenRouter key: set {KEY_VAR} or put api_key in ~/.openagents/openrouter.json"
            ),
            Error::Client(why) => write!(f, "the HTTP client couldn't start: {why}"),
            Error::Api {
                kind,
                status,
                message,
            } => write!(f, "OpenRouter returned HTTP {status} ({kind:?}): {message}"),
            Error::Connection(why) => write!(f, "couldn't reach OpenRouter: {why}"),
            Error::Timeout => f.write_str("the request to OpenRouter timed out"),
            Error::Decode { detail, excerpt } => {
                write!(
                    f,
                    "OpenRouter's response couldn't be read: {detail}: {excerpt}"
                )
            }
            Error::Schema { detail, excerpt } => {
                write!(
                    f,
                    "the reply doesn't match the requested shape: {detail}: {excerpt}"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

/// The first `max` characters of `text`.
fn excerpt(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push('…');
    }
    out
}

/// The OpenRouter client.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    config: Config,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.config.base_url)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A client with `config`.
    ///
    /// # Errors
    ///
    /// [`Error::Client`] when the HTTP client can't be built.
    pub fn new(config: Config) -> Result<Self, Error> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|error| Error::Client(error.to_string()))?;
        Ok(Client { http, config })
    }

    /// Sends `request` once per attempt, retrying a retryable status or a
    /// broken connection up to the configured count, and honoring
    /// `Retry-After`.
    ///
    /// # Errors
    ///
    /// The last attempt's [`Error`].
    pub async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, Error> {
        let mut attempt = 0;
        loop {
            match self.attempt(request).await {
                Ok(response) => return Ok(response),
                Err((error, wait)) => {
                    let retryable = match &error {
                        Error::Api { kind, .. } => kind.retryable(),
                        Error::Connection(_) | Error::Timeout => true,
                        _ => false,
                    };
                    if !retryable || attempt >= self.config.retries {
                        return Err(error);
                    }
                    attempt += 1;
                    let backoff = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                    tokio::time::sleep(wait.unwrap_or(backoff).min(Duration::from_secs(60))).await;
                }
            }
        }
    }

    async fn attempt(
        &self,
        request: &ChatRequest,
    ) -> Result<ChatResponse, (Error, Option<Duration>)> {
        let mut builder = self
            .http
            .post(format!("{}/chat/completions", self.config.base_url))
            .bearer_auth(self.config.api_key.expose())
            .json(request);
        if let Some(referer) = &self.config.referer {
            builder = builder.header("HTTP-Referer", referer);
        }
        if let Some(title) = &self.config.title {
            builder = builder.header("X-Title", title);
        }
        let response = builder.send().await.map_err(|error| {
            if error.is_timeout() {
                (Error::Timeout, None)
            } else {
                (Error::Connection(error.without_url().to_string()), None)
            }
        })?;
        let status = response.status().as_u16();
        let wait = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        let text = response.text().await.map_err(|error| {
            if error.is_timeout() {
                (Error::Timeout, None)
            } else {
                (Error::Connection(error.without_url().to_string()), None)
            }
        })?;
        if !(200..300).contains(&status) {
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|value| value["error"]["message"].as_str().map(str::to_string))
                .unwrap_or_else(|| excerpt(&text, 400));
            return Err((
                Error::Api {
                    kind: ApiErrorKind::of(status),
                    status,
                    message,
                },
                wait,
            ));
        }
        let value: Value = serde_json::from_str(&text).map_err(|error| {
            (
                Error::Decode {
                    detail: error.to_string(),
                    excerpt: excerpt(&text, 400),
                },
                None,
            )
        })?;
        // OpenRouter can report a provider's failure inside a 200.
        if let Some(message) = value["error"]["message"].as_str() {
            let status = value["error"]["code"]
                .as_u64()
                .and_then(|code| u16::try_from(code).ok())
                .unwrap_or(502);
            return Err((
                Error::Api {
                    kind: ApiErrorKind::of(status),
                    status,
                    message: message.to_string(),
                },
                None,
            ));
        }
        serde_json::from_value(value).map_err(|error| {
            (
                Error::Decode {
                    detail: error.to_string(),
                    excerpt: excerpt(&text, 400),
                },
                None,
            )
        })
    }

    /// Sends `request` with a strict `json_schema` response format named
    /// `name`, and parses the first choice's text into `T`.
    ///
    /// # Errors
    ///
    /// [`Client::chat`]'s errors, [`Error::Decode`] when there's no reply
    /// text, and [`Error::Schema`] when the text isn't a `T`.
    pub async fn structured<T: DeserializeOwned>(
        &self,
        mut request: ChatRequest,
        name: &str,
        schema: Value,
    ) -> Result<Structured<T>, Error> {
        request.response_format = Some(ResponseFormat {
            kind: "json_schema".to_string(),
            json_schema: JsonSchema {
                name: name.to_string(),
                schema,
                strict: true,
            },
        });
        let started = std::time::Instant::now();
        let response = self.chat(&request).await?;
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let choice = response.choices.first().ok_or_else(|| Error::Decode {
            detail: "the response has no choices".to_string(),
            excerpt: String::new(),
        })?;
        let raw = choice.message.content.clone().unwrap_or_default();
        if raw.trim().is_empty() {
            return Err(Error::Decode {
                detail: format!(
                    "the reply has no text (finish reason {})",
                    choice.finish_reason.as_deref().unwrap_or("unknown")
                ),
                excerpt: String::new(),
            });
        }
        let value =
            serde_json::from_str::<T>(strip_fence(&raw)).map_err(|error| Error::Schema {
                detail: error.to_string(),
                excerpt: excerpt(&raw, 400),
            })?;
        Ok(Structured {
            value,
            raw,
            model: response.model,
            finish_reason: choice.finish_reason.clone(),
            usage: response.usage,
            milliseconds,
        })
    }
}

/// The JSON inside a Markdown code fence, when a model wraps its reply in
/// one; otherwise the text itself.
fn strip_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    rest.strip_suffix("```").unwrap_or(rest).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_never_prints() {
        let config = Config::new(ApiKey::new("sk-or-secret-value"));
        let shown = format!("{config:?} {:?}", Client::new(config.clone()).unwrap());
        assert!(!shown.contains("secret"), "{shown}");
        assert_eq!(config.api_key.expose(), "sk-or-secret-value");
    }

    #[test]
    fn statuses_map_to_the_sdk_error_classes() {
        assert_eq!(ApiErrorKind::of(401), ApiErrorKind::Unauthorized);
        assert_eq!(ApiErrorKind::of(429), ApiErrorKind::TooManyRequests);
        assert!(ApiErrorKind::of(429).retryable());
        assert!(ApiErrorKind::of(503).retryable());
        assert!(!ApiErrorKind::of(400).retryable());
        assert!(!ApiErrorKind::of(402).retryable());
    }

    #[test]
    fn a_request_serializes_as_openrouter_expects() {
        let mut request =
            ChatRequest::new("openai/gpt-6-luna", vec![Message::user("hi")]).effort("low");
        request.response_format = Some(ResponseFormat {
            kind: "json_schema".to_string(),
            json_schema: JsonSchema {
                name: "next".to_string(),
                schema: serde_json::json!({"type": "object"}),
                strict: true,
            },
        });
        let body = serde_json::to_value(&request).unwrap();
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["response_format"]["json_schema"]["name"], "next");
        assert_eq!(body["response_format"]["json_schema"]["strict"], true);
        assert_eq!(body["usage"]["include"], true);
        assert_eq!(body["stream"], false);
        assert_eq!(body["reasoning"]["effort"], "low");
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn a_fenced_reply_is_unwrapped() {
        assert_eq!(strip_fence("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(strip_fence(" {\"a\":1} "), "{\"a\":1}");
    }
}
