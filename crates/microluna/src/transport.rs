//! The seam between a session and the model.
//!
//! A [`Transport`] takes one [`Request`] — instructions, input items, tool
//! declarations, and a cache key — and returns one [`Reply`]: the output
//! items and the usage the provider reported. A session never learns how
//! the reply was reached, so a test swaps the network for a script.

use std::fmt;
use std::future::Future;

use serde_json::Value;

/// One model request, as a session builds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// The model slug, such as `gpt-6-luna`.
    pub model: String,
    /// The system instructions: the most stable part of the prefix.
    pub instructions: String,
    /// Responses API input items, in the order the session chose.
    pub input: Vec<Value>,
    /// Native function tool declarations.
    pub tools: Vec<Value>,
    /// The reasoning effort, when the session sets one.
    pub effort: Option<String>,
    /// The provider's prompt-cache key. Sessions of one task share it, so
    /// their common prefix is cached across sessions.
    pub cache_key: String,
    /// Whether the model may call several tools in one turn.
    pub parallel_tools: bool,
}

/// Tokens one reply consumed and produced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenUsage {
    /// Input tokens, cached ones included.
    pub input: u64,
    /// The input tokens the provider served from its cache.
    pub cached: u64,
    /// Output tokens, reasoning included.
    pub output: u64,
    /// The output tokens spent on reasoning.
    pub reasoning: u64,
}

impl TokenUsage {
    /// Input tokens the cache didn't serve.
    #[must_use]
    pub fn uncached(&self) -> u64 {
        self.input.saturating_sub(self.cached)
    }

    /// Adds another reply's usage to this one.
    pub fn add(&mut self, other: TokenUsage) {
        self.input += other.input;
        self.cached += other.cached;
        self.output += other.output;
        self.reasoning += other.reasoning;
    }
}

/// One function call the model made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionCall {
    /// The call's identifier, which its output names back.
    pub call_id: String,
    /// The tool's name.
    pub name: String,
    /// The arguments, as the JSON text the model wrote.
    pub arguments: String,
}

/// What the model answered to one request.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reply {
    /// The provider's response identifier, when it sent one.
    pub id: Option<String>,
    /// The model that answered, as the provider named it.
    pub model: String,
    /// The output items, in order: messages, reasoning, and function calls.
    pub items: Vec<Value>,
    /// What the reply cost in tokens.
    pub usage: TokenUsage,
}

impl Reply {
    /// The function calls among the output items, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<FunctionCall> {
        self.items
            .iter()
            .filter(|item| item["type"] == "function_call")
            .map(|item| FunctionCall {
                call_id: text(&item["call_id"]),
                name: text(&item["name"]),
                arguments: text(&item["arguments"]),
            })
            .collect()
    }

    /// The assistant's message text, joined.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = Vec::new();
        for item in self.items.iter().filter(|item| item["type"] == "message") {
            for part in item["content"].as_array().into_iter().flatten() {
                if let Some(piece) = part["text"].as_str() {
                    out.push(piece);
                }
            }
        }
        out.join("\n")
    }

    /// The reasoning summary text, joined, when the provider sent any.
    #[must_use]
    pub fn reasoning(&self) -> String {
        let mut out = Vec::new();
        for item in self.items.iter().filter(|item| item["type"] == "reasoning") {
            for part in item["summary"].as_array().into_iter().flatten() {
                if let Some(piece) = part["text"].as_str() {
                    out.push(piece);
                }
            }
        }
        out.join("\n")
    }
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

/// Why a request got no reply.
#[derive(Debug)]
pub enum TransportError {
    /// The Codex login couldn't be used.
    Login(crate::codex::LoginError),
    /// The provider answered with an error status.
    Http {
        /// The HTTP status code.
        status: u16,
        /// The start of the response body.
        body: String,
    },
    /// The connection or the event stream broke.
    Stream(String),
    /// The provider reported that the response failed.
    Failed(String),
    /// The provider stopped the response before it completed.
    Incomplete(String),
    /// The script a fake transport follows ran out.
    Exhausted,
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Login(error) => write!(f, "the Codex login can't be used: {error}"),
            TransportError::Http { status, body } => {
                write!(f, "the provider returned HTTP {status}: {body}")
            }
            TransportError::Stream(why) => write!(f, "the response stream stopped early: {why}"),
            TransportError::Failed(why) => write!(f, "the response failed: {why}"),
            TransportError::Incomplete(why) => write!(f, "the response is incomplete: {why}"),
            TransportError::Exhausted => {
                write!(f, "the test transport has no scripted replies left")
            }
        }
    }
}

impl std::error::Error for TransportError {}

impl TransportError {
    /// Whether sending the request again may succeed: a broken stream, a
    /// rate limit, or a server error. A refused login or a failed or
    /// incomplete response is not.
    #[must_use]
    pub fn transient(&self) -> bool {
        match self {
            TransportError::Stream(_) => true,
            TransportError::Http { status, .. } => *status == 429 || *status >= 500,
            _ => false,
        }
    }
}

/// Something that answers a [`Request`].
pub trait Transport {
    /// Sends one request and waits for the whole reply.
    fn respond(&self, request: &Request) -> impl Future<Output = Result<Reply, TransportError>>;
}
