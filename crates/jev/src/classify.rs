//! `POST /v1/classify` — an ordered batch under one selection policy.
//!
//! The envelope is the caller's `openagents.classify.v1` document —
//! mode, labels, levels, dimensions, inputs, and the policy whose
//! `select`, `review`, and `fallback` sub-documents decide what each
//! input's answers become. The SDK carries the envelope as the caller
//! wrote it; the door owns validation. What comes back is the report,
//! decoded: every input's outcome and units in input order, refused and
//! unattempted work named rather than dropped.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Result;
use crate::client::Client;
use crate::error::Error;
use crate::options::CallOptions;
use crate::transport::parse_body;
use reqwest::Method;

/// The route this module calls.
const PATH: &str = "/v1/classify";

/// The schema an envelope declares.
pub const SCHEMA: &str = "openagents.classify.v1";

/// One `POST /v1/classify`, scoped to its client.
#[derive(Debug)]
pub struct Classify<'a> {
    client: &'a Client,
}

impl<'a> Classify<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Run one envelope and read the report.
    ///
    /// A `mixed` report is a success — partial outcomes are the
    /// contract's point. A `503` carrying a completed report is an
    /// [`Error::Api`]; callers that want the report bytes regardless
    /// call [`Classify::run_raw`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when the envelope is not an
    /// `openagents.classify.v1` object, [`Error::Api`],
    /// [`Error::Connection`], or [`Error::Timeout`] when the call
    /// fails, and [`Error::ResponseValidation`] when a successful
    /// body does not decode as the report.
    pub async fn run(&self, request: ClassifyRequest) -> Result<ClassifyReport> {
        let raw = self.dispatch(&request).await?;
        decode(&raw.bytes, raw.status, &raw.headers)
    }

    /// Run one envelope and hand back the response unread — the path a
    /// caller takes when it wants a `503` report's own accounting
    /// rather than the refusal.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Classify::run`] other than
    /// [`Error::ResponseValidation`].
    pub async fn run_raw(&self, request: ClassifyRequest) -> Result<crate::RawResponse> {
        self.dispatch(&request).await
    }

    /// The call both paths share.
    async fn dispatch(&self, request: &ClassifyRequest) -> Result<crate::RawResponse> {
        let body = serde_json::to_vec(&request.envelope).map_err(|error| {
            Error::Config(format!(
                "the classification request can't be encoded as JSON: {error}"
            ))
        })?;
        self.client
            .request_read(
                Method::POST,
                PATH,
                Some(body),
                &request.options.headers,
                request.options.timeout,
                request.options.retry.clone(),
            )
            .await
    }
}

/// The envelope and the call's overrides.
#[derive(Debug, Clone)]
pub struct ClassifyRequest {
    envelope: Value,
    options: CallOptions,
}

impl ClassifyRequest {
    /// A request over one `openagents.classify.v1` envelope.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when the value is not an object
    /// declaring `v: "openagents.classify.v1"` — the door's own
    /// validation still owns everything deeper.
    pub fn new(envelope: Value) -> Result<Self> {
        let valid = envelope
            .get("v")
            .and_then(Value::as_str)
            .is_some_and(|v| v == SCHEMA);
        if !envelope.is_object() || !valid {
            return Err(Error::Config(format!(
                "the classification request must be a JSON object with `\"v\": \"{SCHEMA}\"`"
            )));
        }
        Ok(Self {
            envelope,
            options: CallOptions::new(),
        })
    }

    /// Options for this call — retry, timeout, headers, an idempotency
    /// key.
    #[must_use]
    pub fn options(mut self, options: CallOptions) -> Self {
        self.options = options;
        self
    }
}

/// The report `POST /v1/classify` answers.
///
/// Fields the schema leaves open — `policy`, `served`, `selections`,
/// `aggregates`, `review` — stay as sent; the typed surface is what a
/// caller reconciles against: the outcome, its counts, and the items.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ClassifyReport {
    /// The schema the report declares.
    pub v: String,
    /// The model the report was served under.
    pub model: String,
    /// The capacity lane it ran on.
    pub capacity: String,
    /// `answered` or `mixed` — whether every input came back or some
    /// part of it did.
    pub outcome: String,
    /// The tally by outcome.
    pub outcomes: ClassifyOutcomes,
    /// One item per input, in input order.
    #[serde(default)]
    pub results: Vec<ClassifyItem>,
    /// The policy's selections, as the report carries them.
    #[serde(default)]
    pub selections: Vec<Value>,
    /// The policy's aggregates, as the report carries them.
    #[serde(default)]
    pub aggregates: Vec<Value>,
    /// What the run spent, when the doors metered it.
    #[serde(default)]
    pub usage: Option<ClassifyUsage>,
    /// The wall clock the report measured.
    #[serde(default)]
    pub timing: Option<ClassifyTiming>,
    /// The policy the report ran, echoed.
    #[serde(default)]
    pub policy: Option<Value>,
    /// The served identity the report echoes.
    #[serde(default)]
    pub served: Option<Value>,
    /// The review summary, when the policy asked for one.
    #[serde(default)]
    pub review: Option<Value>,
}

/// The report's outcome tally — partial work counts under its own name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClassifyOutcomes {
    /// Inputs that came back answered.
    #[serde(default)]
    pub answered: u64,
    /// Inputs the door refused.
    #[serde(default)]
    pub refused: u64,
    /// Inputs the door could not reach.
    #[serde(default)]
    pub unavailable: u64,
    /// Inputs never dispatched.
    #[serde(default)]
    pub unattempted: u64,
}

/// One input's row in the report.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ClassifyItem {
    /// The input's id, as the envelope named it.
    pub input: String,
    /// `answered`, `mixed`, `refused`, `unavailable`, or `unattempted`.
    pub outcome: String,
    /// The judgments the item's mode asked, each its own typed unit.
    #[serde(default)]
    pub units: Vec<ClassifyUnit>,
    /// Why the item did not answer, when it did not.
    #[serde(default)]
    pub cause: Option<String>,
    /// The item's own clock.
    #[serde(default)]
    pub latency_ms: Option<u64>,
    /// The model the item's attempts ran under.
    #[serde(default)]
    pub model: Option<String>,
    /// The item's metered spend, when the door reported one.
    #[serde(default)]
    pub usage: Option<Value>,
    /// `not-reviewed`, `reviewed`, or `review-incomplete`.
    #[serde(default)]
    pub review_status: Option<String>,
    /// What review replaced, when it changed the answer.
    #[serde(default)]
    pub original: Option<Value>,
    /// Every attempt the item took, primary and secondary.
    #[serde(default)]
    pub attempts: Option<Value>,
    /// The fallback record, when the policy sent one.
    #[serde(default)]
    pub fallback: Option<Value>,
}

/// One judgment inside an item — the mode's own answer shape stays as
/// sent in `selected` and `raw`, while outcome and provenance type.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ClassifyUnit {
    /// `single-label`, `multi-label`, `binary`, or `score`.
    pub mode: String,
    /// The dimension this unit judged, when the mode carries several.
    #[serde(default)]
    pub dimension: Option<String>,
    /// `answered`, `refused`, `unavailable`, or `unattempted`.
    pub outcome: String,
    /// Why the unit did not answer, when it did not.
    #[serde(default)]
    pub cause: Option<String>,
    /// What the policy selected — the mode's own shape: a label, a
    /// list, a position, or null.
    #[serde(default)]
    pub selected: Value,
    /// The model's own answer document, when the unit answered.
    #[serde(default)]
    pub raw: Option<Value>,
    /// The selection fell under the uncertainty floor.
    #[serde(default)]
    pub uncertain: Option<bool>,
    /// No label matched the input.
    #[serde(default)]
    pub no_match: Option<bool>,
    /// What review replaced, when it changed this unit.
    #[serde(default)]
    pub original: Option<Value>,
    /// The review record — reason, outcome, latency, and the
    /// reviewer's own raw answer.
    #[serde(default)]
    pub review: Option<Value>,
    /// `primary` or `reviewer` — which attempt the answer stands on.
    #[serde(default)]
    pub final_source: Option<String>,
}

/// What a classify run metered, including the secondary passes.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct ClassifyUsage {
    /// Native requests the run forwarded.
    pub forwards: u64,
    /// Every forward's input usage was reported.
    #[serde(default)]
    pub input_tokens_complete: bool,
    /// Every forward's output usage was reported.
    #[serde(default)]
    pub output_tokens_complete: bool,
    /// Summed input tokens, present only while `input_tokens_complete`.
    #[serde(default)]
    pub input_tokens: Option<u64>,
    /// Summed output tokens, present only while `output_tokens_complete`.
    #[serde(default)]
    pub output_tokens: Option<u64>,
    /// What the review pass spent, when the policy asked for one.
    #[serde(default)]
    pub review: Option<Value>,
    /// What fallback dispatches spent, when any ran.
    #[serde(default)]
    pub fallback: Option<Value>,
}

/// The report's own clock.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClassifyTiming {
    /// Milliseconds the run took end to end.
    pub latency_ms: u64,
}

/// Decode one successful body into the report.
fn decode(
    bytes: &[u8],
    status: u16,
    headers: &reqwest::header::HeaderMap,
) -> Result<ClassifyReport> {
    serde_json::from_slice::<ClassifyReport>(bytes).map_err(|error| Error::ResponseValidation {
        status,
        field_path: format!("classify report: {error}"),
        body: parse_body(bytes).map(Box::new),
        request_id: headers
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
    })
}
