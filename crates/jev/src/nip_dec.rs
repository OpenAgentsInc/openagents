//! One decision, exactly as NIP-DEC defines it (`nips/openagents/NIP-DEC.md`):
//! the request body every caller builds, the model names a host admits, the
//! bodies the two HTTP doors take, and the refusal an HTTP error stands for.
//!
//! [`DecisionRequest`] is `{model, state, questions}`. The same value is the
//! JSON body of TypeSafe's `POST /v1/systemone`, of OpenRouter's
//! `POST /api/alpha/decisions` (with the model under OpenRouter's name), of
//! an OpenAgents gateway's `POST /v1/systemone`, and — with a request id and
//! attempt — the `state`, `questions`, and `model` of a NIP-DEC decision job
//! (`jev_hosted::wire_body` builds that one). [`crate::SystemOneRequest`]
//! wraps one with the per-call settings (retry, timeout, headers).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::error::{ApiError, Error};
use crate::questions::{Entry, Questions};

/// What every question reads: text or a JSON object (NIP-DEC, "Request").
///
/// A JSON array or scalar handed to [`State::from`] becomes its compact JSON
/// text, the one lossless reading a NIP-DEC host admits.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// Text.
    Text(String),
    /// A JSON object whose fields the questions can name, as
    /// `` `ticket.messages[0].text` ``.
    Object(Map<String, Value>),
}

impl Default for State {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl State {
    /// The state as a JSON value.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Text(text) => Value::String(text.clone()),
            Self::Object(map) => Value::Object(map.clone()),
        }
    }

    /// Build a state from anything that serializes to a JSON object or
    /// string.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when the value does not serialize.
    pub fn json<T: Serialize>(value: &T) -> crate::Result<Self> {
        serde_json::to_value(value)
            .map(Self::from)
            .map_err(|error| Error::Config(format!("the state can't be encoded as JSON: {error}")))
    }
}

impl From<&str> for State {
    fn from(text: &str) -> Self {
        Self::Text(text.to_string())
    }
}

impl From<String> for State {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&String> for State {
    fn from(text: &String) -> Self {
        Self::Text(text.clone())
    }
}

impl From<Map<String, Value>> for State {
    fn from(map: Map<String, Value>) -> Self {
        Self::Object(map)
    }
}

impl From<Value> for State {
    fn from(value: Value) -> Self {
        match value {
            Value::String(text) => Self::Text(text),
            Value::Object(map) => Self::Object(map),
            Value::Null => Self::Text(String::new()),
            other => Self::Text(other.to_string()),
        }
    }
}

impl From<Entry> for State {
    fn from(entry: Entry) -> Self {
        Self::from(entry.to_value())
    }
}

impl Serialize for State {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Text(text) => text.serialize(serializer),
            Self::Object(map) => map.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for State {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::String(text) => Ok(Self::Text(text)),
            Value::Object(map) => Ok(Self::Object(map)),
            _ => Err(serde::de::Error::custom(
                "state is a string or a JSON object",
            )),
        }
    }
}

/// One decision request: a model, one state, and the typed questions about
/// it. Serializes to exactly `{"model", "state", "questions"}`.
///
/// ```
/// use jev::{Choice, DecisionRequest, Noul, NoulCriteria, Questions, State};
/// use serde_json::json;
///
/// let request = DecisionRequest::new(
///     "jev-1.13.0",
///     State::from(json!({"ticket": "I was charged twice."})),
///     Questions::new()
///         .with(
///             "refund",
///             Noul::with_criteria(
///                 "Does the customer ask for money back?",
///                 NoulCriteria::new().when_true(json!({"what": "A refund or credit"})),
///             ),
///         )
///         .with(
///             "queue",
///             Choice::default()
///                 .option("billing", json!({"what": "Charges", "not_for": "Bugs"}))
///                 .option("technical", "Something is broken"),
///         ),
/// );
/// let body = request.to_value();
/// assert_eq!(DecisionRequest::from_value(body.clone()).unwrap().to_value(), body);
/// assert_eq!(request.openrouter_body()["model"], "typesafe/jev-1.13");
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DecisionRequest {
    /// The model to ask, such as `jev-1.13.0`, `jev-latest`, or an alias.
    pub model: String,
    /// What every question reads.
    pub state: State,
    /// The questions, in the order they were added.
    pub questions: Questions,
}

impl DecisionRequest {
    /// A request for `model`.
    #[must_use]
    pub fn new<M: Into<String>, S: Into<State>>(model: M, state: S, questions: Questions) -> Self {
        Self {
            model: model.into(),
            state: state.into(),
            questions,
        }
    }

    /// The body TypeSafe's, an OpenAgents gateway's, and a decision job's
    /// `POST /v1/systemone` take: `{model, state, questions}`, the model as
    /// the caller named it.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut body = Map::new();
        body.insert("model".to_string(), Value::String(self.model.clone()));
        body.insert("state".to_string(), self.state.to_value());
        body.insert("questions".to_string(), self.questions.to_value());
        Value::Object(body)
    }

    /// The body OpenRouter's Decisions API (`POST /api/alpha/decisions`)
    /// takes: the same fields with the model under OpenRouter's name
    /// ([`openrouter_model`]).
    #[must_use]
    pub fn openrouter_body(&self) -> Value {
        let mut body = self.to_value();
        body["model"] = Value::String(openrouter_model(&self.model).to_string());
        body
    }

    /// Read a body in the NIP-DEC shape.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] naming the field that is missing or of the
    /// wrong shape: `model` (a nonempty string), `state` (a string or an
    /// object), or `questions` (an object).
    pub fn from_value(value: Value) -> crate::Result<Self> {
        let Value::Object(mut map) = value else {
            return Err(Error::Config("a decision request is a JSON object".into()));
        };
        let model = match map.remove("model") {
            Some(Value::String(model)) if !model.is_empty() => model,
            _ => return Err(Error::Config("a decision request names a `model`".into())),
        };
        let state = match map.remove("state") {
            Some(Value::String(text)) => State::Text(text),
            Some(Value::Object(object)) => State::Object(object),
            _ => {
                return Err(Error::Config(
                    "a decision request's `state` is a string or a JSON object".into(),
                ));
            }
        };
        let questions = match map.remove("questions") {
            Some(Value::Object(questions)) => Questions::from_map(questions),
            _ => {
                return Err(Error::Config(
                    "a decision request's `questions` is an object".into(),
                ));
            }
        };
        Ok(Self {
            model,
            state,
            questions,
        })
    }

    /// The same request asking the canonical model its name stands for.
    #[must_use]
    pub fn canonical(mut self) -> Self {
        self.model = canonical_model(&self.model).to_string();
        self
    }
}

impl Serialize for DecisionRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DecisionRequest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::from_value(Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Jev's published list price, US dollars per million input tokens,
/// retrieved 2026-09-22 (TypeSafe bills input tokens). Used to price an
/// answer whose door reported tokens and no cost.
pub const USD_PER_MILLION_INPUT: f64 = 0.042;

/// Model aliases a host admits beside the canonical name, alias first
/// (NIP-DEC, "Models"). The same table as `nostr::decision::MODEL_ALIASES`;
/// `jev-hosted` tests that they agree.
pub const MODEL_ALIASES: &[(&str, &str)] = &[("typesafe/jev-1.13", "jev-1.13.0")];

/// The canonical name a model alias stands for, or the name itself.
#[must_use]
pub fn canonical_model(model: &str) -> &str {
    MODEL_ALIASES
        .iter()
        .find(|(alias, _)| *alias == model)
        .map_or(model, |(_, canonical)| canonical)
}

/// OpenRouter's name for a model: the alias of a canonical name, or
/// `typesafe/<name>` for another bare Jev name (OpenRouter maps bare
/// System One names onto the `typesafe/` namespace), or the name itself
/// when it already names a namespace. `jev-latest`, the default, asks for
/// the newest Jev OpenRouter carries, the first alias: OpenRouter refuses
/// `typesafe/jev-latest` ("does not exist", a 400 that never fails over
/// to the next door; #11106).
#[must_use]
pub fn openrouter_model(model: &str) -> std::borrow::Cow<'_, str> {
    if model.contains('/') {
        return model.into();
    }
    if model == "jev-latest"
        && let Some((alias, _)) = MODEL_ALIASES.first()
    {
        return (*alias).into();
    }
    if let Some((alias, _)) = MODEL_ALIASES
        .iter()
        .find(|(_, canonical)| *canonical == model)
    {
        return (*alias).into();
    }
    format!("typesafe/{model}").into()
}

/// A typed refusal read off an HTTP error: the NIP-DEC code, its message,
/// and the status it came with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The HTTP status.
    pub status: u16,
    /// The NIP-DEC refusal code, such as `quota_exhausted`.
    pub code: String,
    /// What the service said.
    pub message: String,
}

impl Refusal {
    /// The refusal an API error stands for: its body's `error.code` when it
    /// is a string (the OpenAgents and TypeSafe shape), else the code its
    /// status stands for ([`code_for_http_status`]; OpenRouter puts the
    /// status number in `error.code`).
    #[must_use]
    pub fn of(error: &ApiError) -> Self {
        let body = error.body.as_ref().and_then(crate::ResponseBody::as_json);
        let code = body
            .and_then(|body| body["error"]["code"].as_str())
            .filter(|code| !code.is_empty())
            .map_or_else(
                || code_for_http_status(error.status).to_string(),
                str::to_string,
            );
        Self {
            status: error.status,
            code,
            message: error.message(),
        }
    }
}

impl Error {
    /// The typed refusal, when the service answered with an HTTP error.
    #[must_use]
    pub fn refusal(&self) -> Option<Refusal> {
        match self {
            Self::Api(api) => Some(Refusal::of(api)),
            _ => None,
        }
    }
}

/// The HTTP status a gateway answers a refusal code with (NIP-DEC,
/// "Refusals and HTTP status"). The same table as
/// `nostr::decision::http_status`; `jev-hosted` tests that they agree.
#[must_use]
pub fn http_status(code: &str) -> u16 {
    match code {
        "malformed"
        | "invalid_request"
        | "too_many_questions"
        | "too_many_options"
        | "unsupported_version"
        | "stale"
        | "idempotency_conflict"
        | "uncalibrated" => 400,
        "unauthenticated" => 401,
        "payment_required" => 402,
        "not_admitted" => 403,
        "door_not_bound" | "not_found" => 404,
        "limit_exceeded" => 413,
        "rate_limited" | "quota_exhausted" => 429,
        "internal" => 500,
        "busy"
        | "unavailable"
        | "registry_unavailable"
        | "membership_unavailable"
        | "ledger_unavailable" => 503,
        "timeout" => 524,
        "overloaded" => 529,
        _ => 502,
    }
}

/// The refusal code an HTTP status with no typed error stands for, the
/// inverse of [`http_status`] on the statuses NIP-DEC names.
#[must_use]
pub fn code_for_http_status(status: u16) -> &'static str {
    match status {
        400 => "invalid_request",
        401 => "unauthenticated",
        402 => "payment_required",
        403 => "not_admitted",
        404 => "door_not_bound",
        413 => "limit_exceeded",
        429 => "rate_limited",
        500 => "internal",
        502 => "door_unavailable",
        524 => "timeout",
        529 => "overloaded",
        _ => "unavailable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Choice, Noul, NoulCriteria, Question, Score};
    use serde_json::json;

    fn docs_examples() -> Vec<Value> {
        vec![
            // TypeSafe's API reference, noul with criteria.
            json!({"state": "Help! My payouts have been failing for 3 days.", "model": "jev-latest",
                "questions": {"is_urgent": {"type": "noul", "instructions": "Does this convey urgency?",
                    "criteria": {"true": "Explicitly time-sensitive", "false": "No urgency expressed"}}}}),
            // Structured instructions with a `field`.
            json!({"state": {"invoice": "Total due: EUR 1,204.00"}, "model": "jev-1.13.0",
                "questions": {"currency": {"type": "choice",
                    "instructions": {"field": {"name": "total_due", "meaning": "The amount owed."}, "question": "Which currency is the field in?"},
                    "criteria": {"EUR": "Euro", "USD": "US dollar", "other": null}}}}),
            // Rubric options, object score levels, a structured noul.
            json!({"state": "I was billed twice", "model": "typesafe/jev-1.13",
                "questions": {
                    "queue": {"type": "choice", "instructions": "Which queue?", "criteria": {
                        "billing": {"what": "Charges", "not_for": "Plan features", "examples": ["I was billed twice"]},
                        "account": {"what": "Login and profile"}}},
                    "severity": {"type": "score", "instructions": "How severe?", "criteria": [
                        {"summary": "Cosmetic", "signals": ["typo"]}, {"summary": "Critical", "signals": ["revenue path down"]}]},
                    "refund": {"type": "noul", "instructions": "Money back?", "criteria": {
                        "true": {"what": "A refund", "examples": ["Refund me"]}, "false": {"what": "Anything else"}}}}}),
        ]
    }

    #[test]
    fn every_docs_example_round_trips_exactly_and_reads_typed() {
        for body in docs_examples() {
            let request = DecisionRequest::from_value(body.clone()).unwrap();
            assert_eq!(request.to_value(), body);
            for (id, question) in request.questions.iter() {
                assert!(
                    !matches!(question, Question::Raw(_)),
                    "{id} should read as a typed question"
                );
            }
            assert!(request.questions.validate().is_ok());
        }
    }

    #[test]
    fn what_the_model_does_not_cover_stays_raw_and_keeps_its_bytes() {
        for question in [
            json!({"type": "noul", "instructions": "Legacy?", "criteria": "yes or no"}),
            json!({"type": "noul", "instructions": "Null criteria?", "criteria": null}),
            json!({"type": "choice", "instructions": "x", "criteria": {"a": 1}}),
            json!({"type": "score", "instructions": "x", "criteria": ["a", "b"], "extra": true}),
            json!({"type": "rank", "instructions": "x"}),
        ] {
            let read = Question::from_value(question.clone());
            assert!(matches!(read, Question::Raw(_)), "{question}");
            assert_eq!(serde_json::to_value(&read).unwrap(), question);
        }
    }

    #[test]
    fn builders_write_the_nip_dec_shapes() {
        let questions = Questions::new()
            .with("n", Noul::new("Is it?"))
            .with(
                "c",
                Noul::with_criteria(
                    json!({"question": "Is it?", "focus": ["a"]})
                        .as_object()
                        .cloned()
                        .unwrap(),
                    NoulCriteria::new().when_false("No"),
                ),
            )
            .with("o", Choice::default().bare_option("a").option("b", "B"))
            .with("s", Score::new("How?", vec![None, Some(Entry::from("hi"))]));
        assert_eq!(
            questions.to_value(),
            json!({
                "n": {"type": "noul", "instructions": "Is it?"},
                "c": {"type": "noul", "instructions": {"question": "Is it?", "focus": ["a"]}, "criteria": {"false": "No"}},
                "o": {"type": "choice", "criteria": {"a": null, "b": "B"}},
                "s": {"type": "score", "instructions": "How?", "criteria": [null, "hi"]},
            })
        );
    }

    #[test]
    fn state_is_text_or_an_object() {
        assert_eq!(State::from(json!(["a"])), State::Text("[\"a\"]".into()));
        assert!(serde_json::from_value::<State>(json!([1])).is_err());
        assert!(
            DecisionRequest::from_value(json!({"model": "m", "state": [1], "questions": {}}))
                .is_err()
        );
    }

    #[test]
    fn model_names_resolve_both_ways() {
        assert_eq!(canonical_model("typesafe/jev-1.13"), "jev-1.13.0");
        assert_eq!(canonical_model("jev-latest"), "jev-latest");
        assert_eq!(openrouter_model("jev-1.13.0"), "typesafe/jev-1.13");
        assert_eq!(openrouter_model("jev-latest"), "typesafe/jev-1.13");
        assert_eq!(openrouter_model("typesafe/jev-1.13"), "typesafe/jev-1.13");
    }

    #[test]
    fn statuses_and_codes_invert_on_the_named_rows() {
        for status in [400, 401, 402, 403, 404, 413, 429, 500, 502, 524, 529] {
            assert_eq!(http_status(code_for_http_status(status)), status);
        }
        assert_eq!(code_for_http_status(503), "unavailable");
        assert_eq!(http_status("something_new"), 502);
    }
}
