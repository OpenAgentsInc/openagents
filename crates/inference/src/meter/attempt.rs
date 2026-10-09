//! One record per upstream attempt (`docs/inference/gateway.md`, section 6).
//!
//! An adapter or the router fills an [`Attempt`] when an attempt ends and
//! hands it to a [`Recorder`]. The record holds ids, words from fixed
//! vocabularies, counts, timings, and amounts. It never holds prompt or
//! completion text, and nothing here takes text from a request body.
//!
//! Amounts are integer millionths ("micros") of the attempt's
//! `currency`, the same scale as the gateway's money ledger. An adapter
//! leaves the cost fields empty; the [`super::Meter`] prices the attempt
//! from its rate card when it records it. When the upstream sends its own
//! cost (OpenRouter, Vercel), the adapter puts it in `reported_cost`.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Which API the caller used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Api {
    /// Open Responses (`/v1/responses`).
    #[default]
    Responses,
    /// OpenAI Chat Completions (`/v1/chat/completions`).
    Chat,
}

/// How an attempt ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The attempt answered the caller.
    #[default]
    Ok,
    /// The attempt failed before its first output token and the router
    /// moved to the next candidate; `error` says why.
    Fallback,
    /// The attempt failed and the caller saw the failure.
    Failed,
    /// The caller went away.
    Canceled,
}

/// Why an attempt failed, from a fixed vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    /// 401 or 403: the account's credential was refused.
    Auth,
    /// 402: the account is out of money or credit.
    Payment,
    /// 429: the upstream asked us to slow down.
    RateLimited,
    /// Another 4xx: the upstream refused the request as sent.
    BadRequest,
    /// A 5xx from the upstream.
    Server,
    /// No first token within the class's deadline.
    FirstTokenDeadline,
    /// The whole attempt ran past its timeout.
    Timeout,
    /// The stream ended with no output.
    EmptyStream,
    /// The stream broke or sent a failure event after it started.
    StreamFailed,
    /// The connection could not be made or was reset.
    Network,
    /// Anything else.
    Other,
}

impl ErrorClass {
    /// Whether this failure counts against the upstream's uptime: the
    /// upstream was down or unusable, not refusing the caller's request.
    pub fn is_outage(self) -> bool {
        matches!(
            self,
            ErrorClass::Server
                | ErrorClass::FirstTokenDeadline
                | ErrorClass::Timeout
                | ErrorClass::EmptyStream
                | ErrorClass::StreamFailed
                | ErrorClass::Network
        )
    }
}

/// Token counts for one attempt.
///
/// `input` is every input token, including `cached_input` and
/// `cache_write`; `output` is every output token, including `reasoning`
/// (OpenAI's usage convention). Reasoning tokens are priced as output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub input: u64,
    #[serde(default)]
    pub cached_input: u64,
    #[serde(default)]
    pub cache_write: u64,
    pub output: u64,
    #[serde(default)]
    pub reasoning: u64,
}

impl Tokens {
    /// Input plus output.
    pub fn total(&self) -> u64 {
        self.input.saturating_add(self.output)
    }
}

/// Who used the API. Old records remain unclassified.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    #[default]
    Unknown,
    Internal,
    Outside,
}

/// How the caller pays, independent of our upstream costs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Payment {
    #[default]
    Unknown,
    Free,
    Paid,
    OwnKey,
}

/// Server-assigned traffic labels, never read from the request body.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Traffic {
    pub audience: Audience,
    pub payment: Payment,
    #[serde(default)]
    pub synthetic: bool,
}

/// One upstream attempt.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    /// The gateway's request id.
    pub request_id: String,
    /// 1 for the first attempt; at most 3 per request.
    pub attempt: u8,
    /// When the attempt started, Unix milliseconds.
    pub at_ms: u64,
    /// The caller's tenant, when keyed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    /// The caller's key id (never the key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    pub api: Api,
    /// The task class (`classify`, `fast`, `chat`, `code`, `long`,
    /// `reason`), when the request named or was given one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// The model id the caller asked for (`openagents/auto` included).
    pub requested_model: String,
    /// The model id this attempt used.
    pub model: String,
    /// The upstream this attempt went to (`vertex`, `zai`, `pro`,
    /// `openrouter`, `vercel`, ...).
    pub upstream: String,
    /// The credit account billed, when the upstream bills one we hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    pub outcome: Outcome,
    /// Milliseconds the request waited before this attempt was sent.
    #[serde(default)]
    pub queue_ms: u64,
    /// Milliseconds from send to the first output token, when one came.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_token_ms: Option<u64>,
    /// Milliseconds from send to the attempt's end.
    pub total_ms: u64,
    pub tokens: Tokens,
    /// True when the upstream gave no usage and we counted the tokens.
    #[serde(default)]
    pub tokens_counted: bool,
    /// False for old records and when the upstream sent no usage.
    #[serde(default)]
    pub usage_reported: bool,
    #[serde(default)]
    pub traffic: Traffic,
    /// The currency of every amount below (`USD` unless a rate row says
    /// otherwise). Set by the meter from the rate row.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub currency: String,
    /// Our cost from the rate row, micros. Set by the meter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<u64>,
    /// Our margin, micros. Set by the meter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<u64>,
    /// The caller's price (cost plus margin), micros. Set by the meter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<u64>,
    /// The cost the upstream itself reported, micros, when it sends one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_cost: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorClass>,
    /// The upstream's HTTP status, when it answered with one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_status: Option<u16>,
}

impl Attempt {
    /// A record with the ids filled and everything else empty.
    pub fn new(
        request_id: impl Into<String>,
        attempt: u8,
        upstream: impl Into<String>,
        model: impl Into<String>,
        at_ms: u64,
    ) -> Self {
        let model = model.into();
        Self {
            request_id: request_id.into(),
            attempt,
            at_ms,
            requested_model: model.clone(),
            model,
            upstream: upstream.into(),
            ..Self::default()
        }
    }

    /// When the attempt ended, Unix milliseconds.
    pub fn end_ms(&self) -> u64 {
        self.at_ms.saturating_add(self.total_ms)
    }

    /// Output tokens per second after the first token, when measurable.
    pub fn throughput(&self) -> Option<f64> {
        let first = self.first_token_ms?;
        let streaming = self.total_ms.checked_sub(first)?;
        if streaming == 0 || self.tokens.output == 0 {
            return None;
        }
        Some(self.tokens.output as f64 * 1000.0 / streaming as f64)
    }

    /// Whether the attempt failed (fell back or failed).
    pub fn is_error(&self) -> bool {
        matches!(self.outcome, Outcome::Fallback | Outcome::Failed)
    }
}

/// Where adapters and the router report attempts. Recording never fails
/// the request: an implementation that cannot keep a record drops it.
pub trait Recorder: Send + Sync {
    fn record(&self, attempt: Attempt);
}

/// Keeps nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoRecorder;

impl Recorder for NoRecorder {
    fn record(&self, _attempt: Attempt) {}
}

/// Keeps every attempt in memory, for tests.
#[derive(Debug, Default)]
pub struct Collect(pub Mutex<Vec<Attempt>>);

impl Collect {
    /// The attempts recorded so far.
    pub fn taken(&self) -> Vec<Attempt> {
        self.0.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

impl Recorder for Collect {
    fn record(&self, attempt: Attempt) {
        if let Ok(mut v) = self.0.lock() {
            v.push(attempt);
        }
    }
}

impl<R: Recorder + ?Sized> Recorder for std::sync::Arc<R> {
    fn record(&self, attempt: Attempt) {
        (**self).record(attempt);
    }
}
