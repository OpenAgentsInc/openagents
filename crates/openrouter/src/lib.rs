//! A small client for OpenRouter's chat completions API, built for one job:
//! send a prompt and get back a JSON object that matches a schema.
//!
//! It's a Rust reimplementation of the parts of OpenRouter's TypeScript SDK
//! (<https://github.com/OpenRouterTeam/typescript-sdk>, Apache-2.0) that
//! this needs: the chat request, the `json_schema` response format
//! (`ChatFormatJsonSchemaConfig` and `ChatJsonSchemaConfig` there), the
//! result's usage and cost, provider routing's `require_parameters`, the
//! `response-healing` plugin, the SDK's error classes, the embeddings
//! call, a streamed chat reply ([`Client::stream`]), and streamed function
//! calls ([`Client::stream_tools`]). Other endpoints are left out.
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

/// One OpenRouter plugin (the spec's `ResponseHealingPlugin` and kin).
#[derive(Clone, Debug, Serialize, PartialEq)]
struct Plugin {
    id: String,
}

/// OpenRouter's provider routing preferences.
#[derive(Clone, Debug, Serialize, PartialEq)]
struct ProviderPreferences {
    /// Route only to providers that support every parameter sent, such as
    /// `response_format`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    require_parameters: bool,
    /// `deny` routes only to providers that do not collect (keep or train
    /// on) the request; set by [`ChatRequest::no_retention`].
    #[serde(skip_serializing_if = "Option::is_none")]
    data_collection: Option<&'static str>,
    /// Routes only to zero-data-retention endpoints; set by
    /// [`ChatRequest::no_retention`].
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    zdr: bool,
}

/// One chat completions request, not streamed.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ChatRequest {
    #[serde(skip_serializing_if = "String::is_empty")]
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
    /// Provider routing: set by [`Client::structured`] so the request goes
    /// only to providers that honor every parameter it sends.
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<ProviderPreferences>,
    /// Plugins: set by [`Client::structured`] to OpenRouter's
    /// `response-healing`, which repairs a reply that misses the schema.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    plugins: Vec<Plugin>,
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
            provider: None,
            plugins: Vec::new(),
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

    /// The same request with a sampling temperature.
    #[must_use]
    pub fn temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// The same request with a token limit on the reply.
    #[must_use]
    pub fn max_tokens(mut self, max: u32) -> Self {
        self.max_tokens = Some(max);
        self
    }

    /// The same request asking OpenRouter to route only to providers that
    /// neither collect nor keep it: `data_collection: "deny"` and, when
    /// `zero_retention`, `zdr: true` (#11040). A model with no such
    /// endpoint is refused rather than sent elsewhere.
    #[must_use]
    pub fn no_retention(mut self, zero_retention: bool) -> Self {
        let mut provider = self.provider.take().unwrap_or(ProviderPreferences {
            require_parameters: false,
            data_collection: None,
            zdr: false,
        });
        provider.data_collection = Some("deny");
        provider.zdr = zero_retention;
        self.provider = Some(provider);
        self
    }

    /// The request [`Client::structured`] sends: a strict `json_schema`
    /// response format named `name`, routing only to providers that
    /// support every parameter sent, and the response-healing plugin for a
    /// reply that still misses the schema.
    #[must_use]
    pub fn structured(mut self, name: &str, schema: Value) -> Self {
        let mut provider = self.provider.take().unwrap_or(ProviderPreferences {
            require_parameters: false,
            data_collection: None,
            zdr: false,
        });
        provider.require_parameters = true;
        self.provider = Some(provider);
        self.plugins = vec![Plugin {
            id: "response-healing".to_string(),
        }];
        self.response_format = Some(ResponseFormat {
            kind: "json_schema".to_string(),
            json_schema: JsonSchema {
                name: name.to_string(),
                schema,
                strict: true,
            },
        });
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

/// The embedding model used when none is named.
pub const EMBEDDING_MODEL: &str = "openai/text-embedding-3-small";

/// One embeddings request: every input is embedded with `model`.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: Vec<String>,
}

impl EmbeddingRequest {
    /// A request to embed `input` with `model`.
    #[must_use]
    pub fn new(model: &str, input: Vec<String>) -> Self {
        EmbeddingRequest {
            model: model.to_string(),
            input,
        }
    }
}

/// One vector in an embeddings response.
#[derive(Clone, Debug, Deserialize)]
struct EmbeddingItem {
    #[serde(default)]
    index: usize,
    embedding: Vec<f32>,
}

/// An embeddings response.
#[derive(Clone, Debug, Deserialize)]
struct EmbeddingResponse {
    #[serde(default)]
    model: String,
    #[serde(default)]
    data: Vec<EmbeddingItem>,
    #[serde(default)]
    usage: Usage,
}

/// The vectors for an embeddings request, one per input in input order,
/// and what the call cost.
#[derive(Clone, Debug)]
pub struct Embeddings {
    pub vectors: Vec<Vec<f32>>,
    /// The model OpenRouter used.
    pub model: String,
    pub usage: Usage,
    /// Milliseconds the call took, retries included.
    pub milliseconds: u64,
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
        /// The response's `Retry-After`, in seconds, when it held a number.
        retry_after: Option<u64>,
        /// The response body, at most [`ERROR_BODY_LIMIT`] bytes, for a
        /// caller that reads a provider's typed error detail.
        body: String,
    },
    /// The request didn't reach OpenRouter, or the connection broke.
    Connection(String),
    /// An attempt ran past its time limit.
    Timeout,
    /// The response wasn't a chat completion or an embeddings result, or had
    /// no reply text.
    Decode { detail: String, excerpt: String },
    /// The reply text doesn't match the requested shape. The call still
    /// cost what `usage` says.
    Schema {
        detail: String,
        excerpt: String,
        usage: Usage,
    },
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
                "no OpenRouter key: set {KEY_VAR} or run openagents settings provider-key set openrouter"
            ),
            Error::Client(why) => write!(f, "the HTTP client couldn't start: {why}"),
            Error::Api {
                kind,
                status,
                message,
                ..
            } => write!(f, "OpenRouter returned HTTP {status} ({kind:?}): {message}"),
            Error::Connection(why) => write!(f, "couldn't reach OpenRouter: {why}"),
            Error::Timeout => f.write_str("the request to OpenRouter timed out"),
            Error::Decode { detail, excerpt } => {
                write!(
                    f,
                    "OpenRouter's response couldn't be read: {detail}: {excerpt}"
                )
            }
            Error::Schema {
                detail, excerpt, ..
            } => {
                write!(
                    f,
                    "the reply doesn't match the requested shape: {detail}: {excerpt}"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

/// The most bytes of an error response's body [`Error::Api`] keeps.
pub const ERROR_BODY_LIMIT: usize = 8 * 1024;

/// `text`, cut to [`ERROR_BODY_LIMIT`] bytes at a character boundary.
fn bounded_body(text: &str) -> String {
    let mut end = text.len().min(ERROR_BODY_LIMIT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

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
        self.post("chat/completions", request).await
    }

    /// Embeds each of `request`'s inputs, with the same retries as
    /// [`Client::chat`]. The vectors come back in input order.
    ///
    /// # Errors
    ///
    /// [`Client::chat`]'s errors, and [`Error::Decode`] when the response
    /// doesn't hold one vector per input.
    pub async fn embeddings(&self, request: &EmbeddingRequest) -> Result<Embeddings, Error> {
        let started = std::time::Instant::now();
        let mut response: EmbeddingResponse = self.post("embeddings", request).await?;
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        response.data.sort_by_key(|item| item.index);
        if response.data.len() != request.input.len() {
            return Err(Error::Decode {
                detail: format!(
                    "{} vectors for {} inputs",
                    response.data.len(),
                    request.input.len()
                ),
                excerpt: String::new(),
            });
        }
        Ok(Embeddings {
            vectors: response
                .data
                .into_iter()
                .map(|item| item.embedding)
                .collect(),
            model: response.model,
            usage: response.usage,
            milliseconds,
        })
    }

    /// Posts `body` as JSON to `path` under the base URL and reads the
    /// response as a `T`, with the same bearer, retries, and error mapping
    /// as [`Client::chat`]. For JSON APIs on other hosts that take a
    /// bearer token, such as Vertex AI's `predict`.
    ///
    /// # Errors
    ///
    /// [`Client::chat`]'s errors, and [`Error::Decode`] when the response
    /// isn't a `T`.
    pub async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        self.post(path, body).await
    }

    /// Posts `body` to `path` under the base URL, with retries, and reads
    /// the response as a `T`.
    async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, Error> {
        let mut attempt = 0;
        loop {
            match self.attempt(path, body).await {
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

    async fn attempt<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, (Error, Option<Duration>)> {
        let mut builder = self
            .http
            .post(format!("{}/{path}", self.config.base_url))
            .bearer_auth(self.config.api_key.expose())
            .json(body);
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
                    retry_after: wait.map(|wait| wait.as_secs()),
                    body: bounded_body(&text),
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
                    retry_after: None,
                    body: bounded_body(&text),
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
        request: ChatRequest,
        name: &str,
        schema: Value,
    ) -> Result<Structured<T>, Error> {
        // Three layers, from OpenRouter's API: see ChatRequest::structured.
        let request = request.structured(name, schema);
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
        // The first complete JSON value is the reply; a model that adds
        // stray characters after it still answered.
        let first = serde_json::Deserializer::from_str(strip_fence(&raw))
            .into_iter::<T>()
            .next();
        let value = match first {
            Some(Ok(value)) => value,
            Some(Err(error)) => {
                return Err(Error::Schema {
                    detail: error.to_string(),
                    excerpt: excerpt(&raw, 400),
                    usage: response.usage,
                });
            }
            None => {
                return Err(Error::Schema {
                    detail: "the reply holds no JSON value".to_string(),
                    excerpt: excerpt(&raw, 400),
                    usage: response.usage,
                });
            }
        };
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

/// A streamed reply: its text and what it cost.
#[derive(Clone, Debug, Default)]
pub struct Streamed {
    /// The reply's text, every delta joined.
    pub text: String,
    /// The model OpenRouter used.
    pub model: String,
    pub finish_reason: Option<String>,
    /// The usage the last chunk carried, or zeros when none did.
    pub usage: Usage,
    /// Milliseconds from sending the request to the first text delta.
    pub first_text_ms: Option<u64>,
    /// Milliseconds the call took.
    pub milliseconds: u64,
}

/// One complete function call requested by a streamed reply.
///
/// The caller decides whether to execute it and validates the arguments against
/// the tool's schema. [`Client::stream_tools`] requires an argument object;
/// [`Client::stream_tools_for_repair`] can return invalid arguments for correction.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// A streamed reply and its complete, validated function calls.
#[derive(Clone, Debug, Default)]
pub struct ToolStreamed {
    pub reply: Streamed,
    pub calls: Vec<FunctionCall>,
}

const TOOL_CALL_LIMIT: usize = 16;
const TOOL_ARGUMENT_LIMIT: usize = 64 * 1024;
const TOOL_AGGREGATE_LIMIT: usize = 256 * 1024;
const TOOL_ID_LIMIT: usize = 256;
const TOOL_NAME_LIMIT: usize = 128;
const STREAM_EVENT_LIMIT: usize = ERROR_BODY_LIMIT * 64;

#[derive(Clone, Copy)]
enum ToolMode {
    None,
    Strict,
    Repair,
}

fn tool_decode(detail: &str) -> Error {
    Error::Decode {
        detail: detail.to_string(),
        excerpt: String::new(),
    }
}

/// Arguments, names, and IDs arrive as fragments keyed by a bounded index.
#[derive(Default)]
struct ToolCalls {
    calls: std::collections::BTreeMap<usize, FunctionCall>,
    bytes: usize,
}

impl ToolCalls {
    fn push(&mut self, value: &Value) -> Result<(), Error> {
        if value.is_null() {
            return Ok(());
        }
        let fragments = value
            .as_array()
            .ok_or_else(|| tool_decode("streamed tool calls were not an array"))?;
        for fragment in fragments {
            if !fragment.is_object() {
                return Err(tool_decode("a streamed tool call was not an object"));
            }
            let index = fragment["index"]
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index < TOOL_CALL_LIMIT)
                .ok_or_else(|| tool_decode("a streamed tool call had an invalid index"))?;
            if let Some(kind) = fragment.get("type")
                && !kind.is_null()
                && kind.as_str() != Some("function")
            {
                return Err(tool_decode("a streamed tool call was not a function"));
            }
            let call = self.calls.entry(index).or_insert_with(|| FunctionCall {
                id: String::new(),
                name: String::new(),
                arguments: String::new(),
            });
            Self::append(
                &mut call.id,
                fragment.get("id"),
                TOOL_ID_LIMIT,
                &mut self.bytes,
            )?;
            if let Some(function) = fragment.get("function")
                && !function.is_null()
            {
                if !function.is_object() {
                    return Err(tool_decode("a streamed function was not an object"));
                }
                Self::append(
                    &mut call.name,
                    function.get("name"),
                    TOOL_NAME_LIMIT,
                    &mut self.bytes,
                )?;
                Self::append(
                    &mut call.arguments,
                    function.get("arguments"),
                    TOOL_ARGUMENT_LIMIT,
                    &mut self.bytes,
                )?;
            }
        }
        Ok(())
    }

    fn append(
        target: &mut String,
        value: Option<&Value>,
        limit: usize,
        bytes: &mut usize,
    ) -> Result<(), Error> {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            return Ok(());
        };
        let text = value
            .as_str()
            .ok_or_else(|| tool_decode("a streamed function field was not text"))?;
        if text.len() > limit.saturating_sub(target.len())
            || text.len() > TOOL_AGGREGATE_LIMIT.saturating_sub(*bytes)
        {
            return Err(tool_decode(
                "streamed function fields exceeded their size limit",
            ));
        }
        target.push_str(text);
        *bytes += text.len();
        Ok(())
    }

    fn finish(self, finish_reason: Option<&str>) -> Result<Vec<FunctionCall>, Error> {
        self.finish_with_arguments(finish_reason, true)
    }

    fn finish_for_repair(self, finish_reason: Option<&str>) -> Result<Vec<FunctionCall>, Error> {
        self.finish_with_arguments(finish_reason, false)
    }

    fn finish_with_arguments(
        self,
        finish_reason: Option<&str>,
        validate_arguments: bool,
    ) -> Result<Vec<FunctionCall>, Error> {
        if self.calls.is_empty() {
            if finish_reason == Some("tool_calls") {
                return Err(tool_decode(
                    "the stream reported tool calls but supplied none",
                ));
            }
            return Ok(Vec::new());
        }
        if finish_reason != Some("tool_calls") {
            return Err(tool_decode(
                "the stream ended without completing its tool calls",
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for call in self.calls.values() {
            if call.id.is_empty()
                || !call.id.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')
                })
                || !ids.insert(&call.id)
            {
                return Err(tool_decode(
                    "a streamed tool call had an invalid or duplicate ID",
                ));
            }
            if call.name.is_empty()
                || !call
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
            {
                return Err(tool_decode(
                    "a streamed tool call had an invalid or missing name",
                ));
            }
            if validate_arguments
                && !serde_json::from_str::<Value>(&call.arguments)
                    .is_ok_and(|arguments| arguments.is_object())
            {
                return Err(tool_decode(
                    "streamed tool arguments were not a JSON object",
                ));
            }
        }
        Ok(self.calls.into_values().collect())
    }
}

/// The Server-Sent Events reader for a streamed chat completion: bytes in,
/// text deltas and the final chunk's fields out. Lines are split at LF (a
/// CR before it is dropped) and decoded whole, so a character split across
/// two network chunks arrives intact. Data lines within an event are joined
/// with newlines and decoded at the blank line. Comments (`: OPENROUTER
/// PROCESSING`) carry nothing, and `data: [DONE]` ends the stream.
#[derive(Default)]
struct StreamReader {
    buffer: Vec<u8>,
    data: String,
    reply: Streamed,
    done: bool,
    tools: Option<ToolCalls>,
}

impl StreamReader {
    /// Reads one network chunk, handing each text delta to `sink`.
    fn push(
        &mut self,
        chunk: &[u8],
        sink: &mut (dyn FnMut(&str) + Send),
        model_sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        if self.done {
            return Ok(());
        }
        self.buffer.extend_from_slice(chunk);
        while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
            if end > STREAM_EVENT_LIMIT {
                return Err(tool_decode("a stream line exceeded its size limit"));
            }
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            let line = std::str::from_utf8(&line[..end]).map_err(|_| Error::Decode {
                detail: "a stream line was not UTF-8".to_string(),
                excerpt: String::new(),
            })?;
            self.line(line.trim_end_matches('\r'), sink, model_sink)?;
            if self.done {
                self.buffer.clear();
                break;
            }
        }
        if self.buffer.len() > STREAM_EVENT_LIMIT {
            return Err(Error::Decode {
                detail: "a stream line ran on without ending".to_string(),
                excerpt: String::new(),
            });
        }
        Ok(())
    }

    fn line(
        &mut self,
        line: &str,
        sink: &mut (dyn FnMut(&str) + Send),
        model_sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        if line.is_empty() {
            let data = std::mem::take(&mut self.data);
            return self.event(data.trim(), sink, model_sink);
        }
        let Some(data) = line
            .strip_prefix("data:")
            .or_else(|| (line == "data").then_some(""))
        else {
            return Ok(());
        };
        let data = data.strip_prefix(' ').unwrap_or(data);
        if data.len().saturating_add(1) > STREAM_EVENT_LIMIT.saturating_sub(self.data.len()) {
            return Err(tool_decode("a stream event exceeded its size limit"));
        }
        self.data.push_str(data);
        self.data.push('\n');
        Ok(())
    }

    fn event(
        &mut self,
        data: &str,
        sink: &mut (dyn FnMut(&str) + Send),
        model_sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(), Error> {
        if data == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        if data.is_empty() {
            return Ok(());
        }
        let chunk: Value = serde_json::from_str(data).map_err(|error| Error::Decode {
            detail: error.to_string(),
            excerpt: excerpt(data, 400),
        })?;
        // A provider's failure mid-stream arrives as a chunk with `error`.
        if let Some(message) = chunk["error"]["message"].as_str() {
            let status = chunk["error"]["code"]
                .as_u64()
                .and_then(|code| u16::try_from(code).ok())
                .unwrap_or(502);
            return Err(Error::Api {
                kind: ApiErrorKind::of(status),
                status,
                message: message.to_string(),
                retry_after: None,
                body: bounded_body(data),
            });
        }
        if let Some(model) = chunk["model"].as_str()
            && model != self.reply.model
        {
            self.reply.model = model.to_string();
            model_sink(model);
        }
        let choice = &chunk["choices"][0];
        if let Some(tools) = &mut self.tools
            && let Some(calls) = choice["delta"].get("tool_calls")
        {
            tools.push(calls)?;
        }
        if let Some(delta) = choice["delta"]["content"].as_str()
            && !delta.is_empty()
        {
            self.reply.text.push_str(delta);
            sink(delta);
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.reply.finish_reason = Some(reason.to_string());
        }
        if chunk["usage"].is_object()
            && let Ok(usage) = serde_json::from_value::<Usage>(chunk["usage"].clone())
        {
            self.reply.usage = usage;
        }
        Ok(())
    }
}

impl Client {
    /// Sends `request` as a streamed chat completion and hands each text
    /// delta to `sink` as it arrives.
    ///
    /// One attempt only: a stream that has shown text cannot be taken back,
    /// so a caller that wants another try decides for itself. The client's
    /// timeout bounds the whole call.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] for an error status or an error chunk,
    /// [`Error::Connection`] and [`Error::Timeout`] as for
    /// [`Client::chat`], and [`Error::Decode`] when a chunk is not JSON or
    /// the stream ends before `[DONE]` or a finish reason.
    pub async fn stream(
        &self,
        request: &ChatRequest,
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Streamed, Error> {
        self.stream_with_model(request, sink, &mut |_| {}).await
    }

    /// Streams a reply and reports the model named by response chunks.
    ///
    /// `model_sink` receives each changed model before that chunk's text.
    /// The requested model is never used as a substitute. An error leaves
    /// previously delivered text and model notifications with the caller.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Client::stream`].
    pub async fn stream_with_model(
        &self,
        request: &ChatRequest,
        sink: &mut (dyn FnMut(&str) + Send),
        model_sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Streamed, Error> {
        let mut request = request.clone();
        request.stream = true;
        self.stream_body(&request, ToolMode::None, sink, model_sink)
            .await
            .map(|stream| stream.reply)
    }

    /// Streams text and function calls with the supplied wire messages and tools.
    ///
    /// `messages` replaces the request's text-only messages and can include
    /// assistant function calls and tool results. The tool definitions accompany
    /// every request. Provider routing requires support for every supplied
    /// parameter. This makes one attempt, even when
    /// an HTTP status or a late error would normally qualify for a retry.
    ///
    /// Calls are returned only after their IDs, names, argument objects, size
    /// limits, and completion reason pass validation. This method executes none.
    ///
    /// # Errors
    ///
    /// Returns [`Client::stream`]'s errors, and [`Error::Decode`] for malformed,
    /// duplicate, incomplete, or oversized function calls.
    pub async fn stream_tools(
        &self,
        request: &ChatRequest,
        messages: &[Value],
        tools: &[Value],
        sink: &mut (dyn FnMut(&str) + Send),
        model_sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<ToolStreamed, Error> {
        self.stream_tools_body(request, messages, tools, ToolMode::Strict, sink, model_sink)
            .await
    }

    /// Streams function calls whose arguments the caller can reject and repair.
    ///
    /// This has the same wire format and single-attempt behavior as
    /// [`Client::stream_tools`]. IDs, names, size limits, and the completion reason
    /// still pass validation. Argument strings are returned unchanged, including
    /// malformed JSON and non-object JSON, so the caller can send a tool error back
    /// to the model. The caller must parse and validate them before executing a
    /// tool. This method executes none.
    ///
    /// # Errors
    ///
    /// Returns [`Client::stream`]'s errors, and [`Error::Decode`] for malformed,
    /// duplicate, incomplete, or oversized call metadata or argument fragments.
    pub async fn stream_tools_for_repair(
        &self,
        request: &ChatRequest,
        messages: &[Value],
        tools: &[Value],
        sink: &mut (dyn FnMut(&str) + Send),
        model_sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<ToolStreamed, Error> {
        self.stream_tools_body(request, messages, tools, ToolMode::Repair, sink, model_sink)
            .await
    }

    async fn stream_tools_body(
        &self,
        request: &ChatRequest,
        messages: &[Value],
        tools: &[Value],
        mode: ToolMode,
        sink: &mut (dyn FnMut(&str) + Send),
        model_sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<ToolStreamed, Error> {
        let mut body = serde_json::to_value(request)
            .map_err(|_| tool_decode("the tool request could not be encoded"))?;
        body["stream"] = Value::Bool(true);
        body["messages"] = Value::Array(messages.to_vec());
        body["tools"] = Value::Array(tools.to_vec());
        if body["provider"].is_object() {
            body["provider"]["require_parameters"] = Value::Bool(true);
        } else {
            body["provider"] = serde_json::json!({ "require_parameters": true });
        }
        self.stream_body(&body, mode, sink, model_sink).await
    }

    async fn stream_body<B: Serialize>(
        &self,
        body: &B,
        tool_mode: ToolMode,
        sink: &mut (dyn FnMut(&str) + Send),
        model_sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<ToolStreamed, Error> {
        let started = std::time::Instant::now();
        let mut builder = self
            .http
            .post(format!("{}/chat/completions", self.config.base_url))
            .bearer_auth(self.config.api_key.expose())
            .json(body);
        if let Some(referer) = &self.config.referer {
            builder = builder.header("HTTP-Referer", referer);
        }
        if let Some(title) = &self.config.title {
            builder = builder.header("X-Title", title);
        }
        let broken = |error: reqwest::Error| {
            if error.is_timeout() {
                Error::Timeout
            } else {
                Error::Connection(error.without_url().to_string())
            }
        };
        let mut response = builder.send().await.map_err(broken)?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<u64>().ok());
            let text = response.text().await.map_err(broken)?;
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|value| value["error"]["message"].as_str().map(str::to_string))
                .unwrap_or_else(|| excerpt(&text, 400));
            return Err(Error::Api {
                kind: ApiErrorKind::of(status),
                status,
                message,
                retry_after,
                body: bounded_body(&text),
            });
        }
        let mut reader = StreamReader {
            tools: (!matches!(tool_mode, ToolMode::None)).then(ToolCalls::default),
            ..StreamReader::default()
        };
        let mut first: Option<u64> = None;
        while let Some(chunk) = response.chunk().await.map_err(broken)? {
            let mut timed = |delta: &str| {
                if first.is_none() {
                    first = Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
                }
                sink(delta);
            };
            reader.push(&chunk, &mut timed, model_sink)?;
            if reader.done {
                break;
            }
        }
        if !reader.done && reader.reply.finish_reason.is_none() {
            return Err(Error::Decode {
                detail: "the stream ended before [DONE]".to_string(),
                excerpt: excerpt(&reader.reply.text, 400),
            });
        }
        let calls = reader
            .tools
            .map(|tools| match tool_mode {
                ToolMode::Repair => tools.finish_for_repair(reader.reply.finish_reason.as_deref()),
                ToolMode::None | ToolMode::Strict => {
                    tools.finish(reader.reply.finish_reason.as_deref())
                }
            })
            .transpose()?
            .unwrap_or_default();
        let mut reply = reader.reply;
        reply.first_text_ms = first;
        reply.milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        Ok(ToolStreamed { reply, calls })
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

    /// #11040: a no-retention request denies data collection and asks for
    /// zero retention, alongside a structured request's own preference.
    #[test]
    fn a_no_retention_request_asks_providers_not_to_keep_it() {
        let plain = serde_json::to_value(ChatRequest::new("m", vec![])).unwrap();
        assert_eq!(plain.get("provider"), None);

        let kept = serde_json::to_value(ChatRequest::new("m", vec![]).no_retention(true)).unwrap();
        assert_eq!(
            kept["provider"],
            serde_json::json!({ "data_collection": "deny", "zdr": true })
        );
        let no_training =
            serde_json::to_value(ChatRequest::new("m", vec![]).no_retention(false)).unwrap();
        assert_eq!(
            no_training["provider"],
            serde_json::json!({ "data_collection": "deny" })
        );

        let both = ChatRequest::new("m", vec![])
            .no_retention(true)
            .structured("x", serde_json::json!({ "type": "object" }));
        let both = serde_json::to_value(both).unwrap();
        assert_eq!(both["provider"]["require_parameters"], true);
        assert_eq!(both["provider"]["data_collection"], "deny");
        assert_eq!(both["provider"]["zdr"], true);
    }

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
    fn a_blank_model_is_omitted_for_the_account_default() {
        let body = serde_json::to_value(ChatRequest::new("", vec![Message::user("hi")])).unwrap();
        assert!(body.get("model").is_none());
        assert_eq!(body["messages"][0]["content"], "hi");
    }

    #[test]
    fn a_fenced_reply_is_unwrapped() {
        assert_eq!(strip_fence("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(strip_fence(" {\"a\":1} "), "{\"a\":1}");
    }

    #[test]
    fn a_stream_is_read_across_split_chunks() {
        let body = concat!(
            ": OPENROUTER PROCESSING\n\n",
            "data: {\"model\":\"m/x\",\"choices\":[{\"delta\":{\"content\":\"café \"}}]}\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5,\"cost\":0.0001}}\n\n",
            "data: [DONE]\n\n",
        )
        .as_bytes();
        let mut reader = StreamReader::default();
        let mut seen = Vec::new();
        let mut models = Vec::new();
        // One byte at a time, so a line and the two-byte "é" both split.
        for byte in body {
            reader
                .push(
                    std::slice::from_ref(byte),
                    &mut |delta: &str| seen.push(delta.to_string()),
                    &mut |model: &str| models.push(model.to_string()),
                )
                .unwrap();
        }
        assert!(reader.done);
        assert_eq!(seen, vec!["café ", "ok"]);
        assert_eq!(reader.reply.text, "café ok");
        assert_eq!(reader.reply.model, "m/x");
        assert_eq!(models, ["m/x"]);
        assert_eq!(reader.reply.finish_reason.as_deref(), Some("stop"));
        assert_eq!(reader.reply.usage.total_tokens, 5);
    }

    #[test]
    fn an_error_chunk_ends_the_stream_with_its_class() {
        let mut reader = StreamReader::default();
        let error = reader
            .push(
                b"data: {\"error\":{\"code\":429,\"message\":\"slow down\"}}\n\n",
                &mut |_: &str| {},
                &mut |_: &str| {},
            )
            .unwrap_err();
        assert!(
            matches!(
                error,
                Error::Api {
                    kind: ApiErrorKind::TooManyRequests,
                    ..
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn model_notifications_precede_text_and_survive_a_later_error() {
        use std::sync::{Arc, Mutex};

        let mut reader = StreamReader::default();
        let events = Arc::new(Mutex::new(Vec::new()));
        let text_events = Arc::clone(&events);
        let model_events = Arc::clone(&events);
        let mut text_sink = move |text: &str| {
            text_events.lock().unwrap().push(format!("text:{text}"));
        };
        let mut model_sink = move |model: &str| {
            model_events.lock().unwrap().push(format!("model:{model}"));
        };
        let body = concat!(
            "data: {\"model\":\"provider/first\",\"choices\":[{\"delta\":{\"content\":\"One\"}}]}\n\n",
            "data: {\"model\":\"provider/first\",\"choices\":[{\"delta\":{\"content\":\" two\"}}]}\n\n",
            "data: {\"model\":\"provider/second\",\"choices\":[]}\n\n",
            "data: {\"error\":{\"code\":502,\"message\":\"fixture error\"}}\n\n"
        );
        assert!(
            reader
                .push(body.as_bytes(), &mut text_sink, &mut model_sink)
                .is_err()
        );
        assert_eq!(
            *events.lock().unwrap(),
            [
                "model:provider/first",
                "text:One",
                "text: two",
                "model:provider/second",
            ]
        );
        assert_eq!(reader.reply.model, "provider/second");
        assert_eq!(reader.reply.text, "One two");
    }

    #[test]
    fn fragmented_tool_calls_preserve_order_text_model_and_usage() {
        let chunks = [
            serde_json::json!({
                "model": "provider/model",
                "choices": [{"delta": {
                    "content": "Checking café. ",
                    "tool_calls": [
                        {"index": 1, "id": "call_", "type": "function", "function": {"name": "openagents_", "arguments": "{\"args\":"}},
                        {"index": 0, "id": "judge_", "type": "function", "function": {"name": "j", "arguments": "{\"state\":\""}}
                    ]
                }}]
            }),
            serde_json::json!({"choices": [{"delta": {"tool_calls": [
                {"index": 0, "id": "1", "function": {"name": "ev", "arguments": "café\"}"}},
                {"index": 1, "id": "2", "function": {"name": "cli", "arguments": "[\"--help\"]}"}}
            ]}}]}),
            serde_json::json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}),
            serde_json::json!({"choices": [], "usage": {"total_tokens": 23, "cost": 0.001}}),
        ];
        let mut body = chunks
            .iter()
            .map(|chunk| format!("data: {chunk}\r\n\r\n"))
            .collect::<String>();
        body.push_str("data: [DONE]\n\n");
        let mut reader = StreamReader {
            tools: Some(ToolCalls::default()),
            ..StreamReader::default()
        };
        let mut text = String::new();
        let mut models = Vec::new();
        for byte in body.as_bytes() {
            reader
                .push(
                    std::slice::from_ref(byte),
                    &mut |delta| text.push_str(delta),
                    &mut |model| models.push(model.to_string()),
                )
                .unwrap();
        }
        assert_eq!(text, "Checking café. ");
        assert_eq!(models, ["provider/model"]);
        assert_eq!(reader.reply.usage.total_tokens, 23);
        assert_eq!(reader.reply.usage.cost, Some(0.001));
        assert!(reader.done);
        let calls = reader
            .tools
            .unwrap()
            .finish(reader.reply.finish_reason.as_deref())
            .unwrap();
        assert_eq!(
            calls,
            [
                FunctionCall {
                    id: "judge_1".into(),
                    name: "jev".into(),
                    arguments: "{\"state\":\"café\"}".into()
                },
                FunctionCall {
                    id: "call_2".into(),
                    name: "openagents_cli".into(),
                    arguments: "{\"args\":[\"--help\"]}".into()
                },
            ]
        );
    }

    #[test]
    fn multiline_events_preserve_tool_calls_text_and_accounting() {
        let body = concat!(
            "event: message\r\n",
            "data: {\"model\":\"provider/model\",\r\n",
            ": OPENROUTER PROCESSING\r\n",
            "data: \"choices\":[{\"delta\":{\"content\":\"café\",\r\n",
            "data: \"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"jev\",\"arguments\":\"{}\"}}]}}]}\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}],\n",
            "data: \"usage\":{\"total_tokens\":11}}\n\n",
            "data: [DONE]\n\n",
            "data: ignored after the stream ends\n\n",
        );
        let mut reader = StreamReader {
            tools: Some(ToolCalls::default()),
            ..StreamReader::default()
        };
        let mut text = String::new();
        let mut models = Vec::new();
        for byte in body.as_bytes() {
            reader
                .push(
                    std::slice::from_ref(byte),
                    &mut |delta| text.push_str(delta),
                    &mut |model| models.push(model.to_string()),
                )
                .unwrap();
        }
        assert!(reader.done);
        assert_eq!(text, "café");
        assert_eq!(models, ["provider/model"]);
        assert_eq!(reader.reply.usage.total_tokens, 11);
        let calls = reader
            .tools
            .unwrap()
            .finish(reader.reply.finish_reason.as_deref())
            .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "jev");
        assert_eq!(calls[0].arguments, "{}");
    }

    #[test]
    fn unfinished_multiline_events_remain_bounded() {
        let mut reader = StreamReader::default();
        let line = format!("data: {}\n", " ".repeat(STREAM_EVENT_LIMIT / 2));
        reader
            .push(line.as_bytes(), &mut |_| {}, &mut |_| {})
            .unwrap();
        assert!(matches!(
            reader.push(line.as_bytes(), &mut |_| {}, &mut |_| {}),
            Err(Error::Decode { .. })
        ));
    }

    #[test]
    fn tool_calls_reject_malformed_fields_and_out_of_bounds_indices() {
        for fragment in [
            serde_json::json!({}),
            serde_json::json!({"index": -1}),
            serde_json::json!({"index": TOOL_CALL_LIMIT}),
            serde_json::json!({"index": 0, "type": "other"}),
            serde_json::json!({"index": 0, "id": 7}),
            serde_json::json!({"index": 0, "function": []}),
            serde_json::json!({"index": 0, "function": {"name": 7}}),
            serde_json::json!({"index": 0, "function": {"arguments": {}}}),
            serde_json::json!(null),
        ] {
            assert!(matches!(
                ToolCalls::default().push(&serde_json::json!([fragment])),
                Err(Error::Decode { .. })
            ));
        }
        assert!(ToolCalls::default().push(&serde_json::json!({})).is_err());
    }

    #[test]
    fn tool_calls_validate_ids_names_arguments_and_completion_before_returning() {
        assert!(ToolCalls::default().finish(Some("tool_calls")).is_err());
        let valid = serde_json::json!({"index": 0, "id": "call_1", "function": {"name": "jev", "arguments": "{}"}});
        for (path, value) in [
            ("/id", serde_json::json!("")),
            ("/id", serde_json::json!("bad id")),
            ("/id", serde_json::json!("bad\u{1b}id")),
            ("/function/name", serde_json::json!("")),
            ("/function/name", serde_json::json!("bad name")),
            ("/function/arguments", serde_json::json!("{\"partial\":")),
            ("/function/arguments", serde_json::json!("[]")),
        ] {
            let mut fragment = valid.clone();
            *fragment.pointer_mut(path).unwrap() = value;
            let mut tools = ToolCalls::default();
            tools.push(&serde_json::json!([fragment])).unwrap();
            assert!(matches!(
                tools.finish(Some("tool_calls")),
                Err(Error::Decode { .. })
            ));
        }
        let mut duplicate = valid.clone();
        duplicate["index"] = 1.into();
        let mut tools = ToolCalls::default();
        tools
            .push(&serde_json::json!([valid.clone(), duplicate]))
            .unwrap();
        assert!(tools.finish(Some("tool_calls")).is_err());
        let mut tools = ToolCalls::default();
        tools.push(&serde_json::json!([valid])).unwrap();
        assert!(tools.finish(Some("length")).is_err());
    }

    #[test]
    fn repair_calls_still_require_complete_unique_metadata() {
        let valid = serde_json::json!({"index": 0, "id": "call_1", "function": {"name": "jev", "arguments": "not JSON"}});
        for (path, value) in [
            ("/id", serde_json::json!("")),
            ("/id", serde_json::json!("bad id")),
            ("/function/name", serde_json::json!("")),
            ("/function/name", serde_json::json!("bad name")),
        ] {
            let mut fragment = valid.clone();
            *fragment.pointer_mut(path).unwrap() = value;
            let mut tools = ToolCalls::default();
            tools.push(&serde_json::json!([fragment])).unwrap();
            assert!(tools.finish_for_repair(Some("tool_calls")).is_err());
        }
        let mut duplicate = valid.clone();
        duplicate["index"] = 1.into();
        let mut tools = ToolCalls::default();
        tools
            .push(&serde_json::json!([valid.clone(), duplicate]))
            .unwrap();
        assert!(tools.finish_for_repair(Some("tool_calls")).is_err());
        for reason in [None, Some("stop"), Some("length")] {
            let mut tools = ToolCalls::default();
            tools.push(&serde_json::json!([valid.clone()])).unwrap();
            assert!(tools.finish_for_repair(reason).is_err());
        }
        assert!(
            ToolCalls::default()
                .finish_for_repair(Some("tool_calls"))
                .is_err()
        );
    }

    #[test]
    fn tool_call_limits_bound_each_argument_and_the_aggregate() {
        for (field, text) in [
            ("id", "a".repeat(TOOL_ID_LIMIT + 1)),
            ("name", "a".repeat(TOOL_NAME_LIMIT + 1)),
            ("arguments", "a".repeat(TOOL_ARGUMENT_LIMIT + 1)),
        ] {
            let mut fragment = serde_json::json!({"index": 0, "function": {}});
            if field == "id" {
                fragment["id"] = text.into();
            } else {
                fragment["function"][field] = text.into();
            }
            assert!(
                ToolCalls::default()
                    .push(&serde_json::json!([fragment]))
                    .is_err()
            );
        }
        let mut tools = ToolCalls::default();
        for index in 0..4 {
            tools
                .push(&serde_json::json!([{
                    "index": index, "function": {"arguments": "a".repeat(TOOL_ARGUMENT_LIMIT)}
                }]))
                .unwrap();
        }
        assert!(
            tools
                .push(&serde_json::json!([{
                    "index": 4, "function": {"arguments": "a"}
                }]))
                .is_err()
        );
    }

    async fn serve_tool_stream(
        body: &str,
        status: u16,
    ) -> (Client, tokio::task::JoinHandle<Value>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body = body.to_string();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            let request_body = loop {
                let read = socket.read(&mut buffer).await.unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&buffer[..read]);
                let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
                    continue;
                };
                let headers = std::str::from_utf8(&request[..end]).unwrap();
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break serde_json::from_slice::<Value>(&request[end + 4..end + 4 + length])
                        .unwrap();
                }
            };
            let response = format!(
                "HTTP/1.1 {status} Test\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            request_body
        });
        let mut config = Config::new(ApiKey::new("fixture"));
        config.base_url = format!("http://{address}");
        config.timeout = Duration::from_secs(2);
        config.retries = 3;
        (Client::new(config).unwrap(), server)
    }

    #[tokio::test]
    async fn a_tool_request_overrides_wire_messages_and_requires_provider_support() {
        let body = concat!(
            "data: {\"model\":\"provider/model\",\"choices\":[{\"delta\":{\"content\":\"Checking.\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"jev\",\"arguments\":\"{}\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"total_tokens\":11}}\n\n",
            "data: [DONE]\n\n"
        );
        let (client, server) = serve_tool_stream(body, 200).await;
        let messages = [
            serde_json::json!({"role": "assistant", "content": null, "tool_calls": [{"id": "previous", "type": "function", "function": {"name": "jev", "arguments": "{}"}}]}),
            serde_json::json!({"role": "tool", "tool_call_id": "previous", "content": "{}"}),
        ];
        let tools = [
            serde_json::json!({"type": "function", "function": {"name": "jev", "parameters": {"type": "object"}}}),
        ];
        let request = ChatRequest::new("openrouter/free", vec![Message::user("replaced")])
            .effort("low")
            .max_tokens(32);
        let mut text = String::new();
        let mut models = Vec::new();
        let result = client
            .stream_tools(
                &request,
                &messages,
                &tools,
                &mut |delta| text.push_str(delta),
                &mut |model| models.push(model.to_string()),
            )
            .await
            .unwrap();
        assert_eq!(text, "Checking.");
        assert_eq!(models, ["provider/model"]);
        assert_eq!(result.reply.model, "provider/model");
        assert_eq!(result.reply.usage.total_tokens, 11);
        assert!(result.reply.first_text_ms.is_some());
        assert_eq!(result.calls[0].name, "jev");
        let sent = server.await.unwrap();
        assert_eq!(sent["messages"], serde_json::json!(messages));
        assert_eq!(sent["tools"], serde_json::json!(tools));
        assert_eq!(sent["provider"]["require_parameters"], true);
        assert_eq!(sent["stream"], true);
        assert_eq!(sent["model"], "openrouter/free");
        assert_eq!(sent["max_tokens"], 32);
        assert_eq!(sent["reasoning"]["effort"], "low");
    }

    #[tokio::test]
    async fn only_the_repair_method_returns_invalid_argument_strings() {
        for arguments in ["{\"state\":\"café\"", "[]", "null", ""] {
            let first = serde_json::json!({
                "model": "provider/model",
                "choices": [{"delta": {
                    "content": "Correcting the call.",
                    "tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "jev", "arguments": arguments}}]
                }}]
            });
            let body = format!(
                "data: {first}\n\ndata: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"tool_calls\"}}]}}\n\ndata: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"tool_calls\"}}],\"usage\":{{\"total_tokens\":11}}}}\n\ndata: [DONE]\n\n"
            );
            let request = ChatRequest::new("openrouter/free", Vec::new());
            let (client, server) = serve_tool_stream(&body, 200).await;
            let error = client
                .stream_tools(&request, &[], &[], &mut |_| {}, &mut |_| {})
                .await
                .unwrap_err();
            assert!(matches!(error, Error::Decode { .. }));
            let strict_request = server.await.unwrap();

            let (client, server) = serve_tool_stream(&body, 200).await;
            let mut text = String::new();
            let mut models = Vec::new();
            let result = client
                .stream_tools_for_repair(
                    &request,
                    &[],
                    &[],
                    &mut |delta| text.push_str(delta),
                    &mut |model| models.push(model.to_string()),
                )
                .await
                .unwrap();
            assert_eq!(result.calls[0].id, "call_1");
            assert_eq!(result.calls[0].name, "jev");
            assert_eq!(result.calls[0].arguments, arguments);
            assert_eq!(text, "Correcting the call.");
            assert_eq!(models, ["provider/model"]);
            assert_eq!(result.reply.usage.total_tokens, 11);
            assert_eq!(strict_request, server.await.unwrap());
        }
    }

    #[tokio::test]
    async fn repair_streams_reject_oversized_and_unfinished_calls() {
        for (arguments, finish_reason) in [
            ("x".repeat(TOOL_ARGUMENT_LIMIT + 1), "tool_calls"),
            ("not JSON".into(), "length"),
        ] {
            let chunk = serde_json::json!({"choices": [{
                "delta": {"tool_calls": [{"index": 0, "id": "call_1", "function": {"name": "jev", "arguments": arguments}}]},
                "finish_reason": finish_reason
            }]});
            let body = format!("data: {chunk}\n\ndata: [DONE]\n\n");
            let (client, server) = serve_tool_stream(&body, 200).await;
            let request = ChatRequest::new("openrouter/free", Vec::new());
            assert!(matches!(
                client
                    .stream_tools_for_repair(&request, &[], &[], &mut |_| {}, &mut |_| {})
                    .await,
                Err(Error::Decode { .. })
            ));
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn tool_stream_errors_never_retry_or_return_partial_calls() {
        for (body, status) in [
            (
                concat!(
                    "data: {\"model\":\"provider/model\",\"choices\":[{\"delta\":{\"content\":\"Checking.\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"jev\",\"arguments\":\"{}\"}}]}}]}\n\n",
                    "data: {\"error\":{\"code\":503,\"message\":\"fixture late error\"}}\n\n"
                ),
                200,
            ),
            ("{\"error\":{\"message\":\"fixture refusal\"}}", 503),
        ] {
            let (client, server) = serve_tool_stream(body, status).await;
            let request = ChatRequest::new("openrouter/free", Vec::new());
            let mut text = String::new();
            let error = client
                .stream_tools(
                    &request,
                    &[],
                    &[],
                    &mut |delta| text.push_str(delta),
                    &mut |_| {},
                )
                .await
                .unwrap_err();
            assert!(matches!(error, Error::Api { status: 503, .. }));
            if status == 200 {
                assert_eq!(text, "Checking.");
            } else {
                assert!(text.is_empty());
            }
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn a_streamed_request_asks_for_a_stream_and_returns_its_text() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            // Read until the JSON body closes.
            loop {
                let read = socket.read(&mut buffer).await.unwrap();
                request.extend_from_slice(&buffer[..read]);
                if read == 0 || request.ends_with(b"}") {
                    break;
                }
            }
            let body = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&request).to_string()
        });
        let client =
            Client::new(Config::new(ApiKey::new("k")).base_url(&format!("http://{address}")))
                .unwrap();
        let request = ChatRequest::new("m/x", vec![Message::user("hi")]).max_tokens(8);
        let mut seen = String::new();
        let reply = client
            .stream(&request, &mut |delta: &str| seen.push_str(delta))
            .await
            .unwrap();
        assert_eq!(reply.text, "hello");
        assert_eq!(seen, "hello");
        assert!(reply.first_text_ms.is_some());
        let sent = server.await.unwrap();
        assert!(sent.contains("\"stream\":true"), "{sent}");
        assert!(sent.contains("\"max_tokens\":8"), "{sent}");
    }
}
