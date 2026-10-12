//! The OpenAgents Decisions API, `POST /v1/decisions`, in OpenAI's
//! Decisions shape (docs/api/decisions.md).
//!
//! The rest of this crate speaks the NIP-DEC body (`POST /v1/systemone`).
//! This module speaks the newer shape: an ordered list of `predicate`,
//! `choice`, and `score` questions about one `input`, answered in request
//! order with probabilities. No key is needed on `openagents.com`; an
//! `oak_` key may be sent.
//!
//! ```no_run
//! use jev::decisions::{DecisionsClient, DecisionsRequest};
//!
//! # async fn ask() -> jev::Result<()> {
//! let client = DecisionsClient::new(jev::decisions::OPENAGENTS)?;
//! let request = DecisionsRequest::new("openagents/decide", "I was charged twice.")
//!     .predicate("refund", "Does the customer ask for money back?")
//!     .choice(
//!         "department",
//!         "Which department should handle this?",
//!         [("billing", "Payments and refunds."), ("technical", "Product problems.")],
//!     );
//! let decision = client.create(&request).await?;
//! let refund = decision.probability("refund");
//! let department = decision.choice("department");
//! # let _ = (refund, department);
//! # Ok(())
//! # }
//! ```

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Result;
use crate::error::{ApiError, Error};

/// The OpenAgents API root that answers `/v1/decisions`.
pub const OPENAGENTS: &str = "https://openagents.com/api";
/// The route.
pub const PATH: &str = "/v1/decisions";
/// How long one call may take by default.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// The shared evidence every question reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Input {
    /// Plain text.
    Text(String),
    /// User messages, each `{"role": "user", "content": ...}` with a string
    /// or `input_text` / `input_image` parts.
    Messages(Vec<Value>),
}

impl From<&str> for Input {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<String> for Input {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

/// One option of a choice question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One level of a score question, lowest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DecisionQuestion {
    /// The probability that a condition is true.
    Predicate {
        name: String,
        instructions: String,
        #[serde(rename = "true", default, skip_serializing_if = "Option::is_none")]
        yes: Option<String>,
        #[serde(rename = "false", default, skip_serializing_if = "Option::is_none")]
        no: Option<String>,
    },
    /// One option from a fixed set.
    Choice {
        name: String,
        instructions: String,
        choices: Vec<ChoiceOption>,
    },
    /// A score against ordered levels.
    Score {
        name: String,
        instructions: String,
        levels: Vec<Level>,
    },
}

/// A `POST /v1/decisions` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionsRequest {
    pub model: String,
    pub input: Input,
    pub questions: Vec<DecisionQuestion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
}

impl DecisionsRequest {
    /// A request to `model` (`openagents/decide`, `clef-flash`, `clef`,
    /// `jev-latest`) about `input`, with no questions yet.
    #[must_use]
    pub fn new(model: impl Into<String>, input: impl Into<Input>) -> Self {
        Self {
            model: model.into(),
            input: input.into(),
            questions: Vec::new(),
            metadata: None,
        }
    }

    /// Add a predicate question.
    #[must_use]
    pub fn predicate(mut self, name: impl Into<String>, instructions: impl Into<String>) -> Self {
        self.questions.push(DecisionQuestion::Predicate {
            name: name.into(),
            instructions: instructions.into(),
            yes: None,
            no: None,
        });
        self
    }

    /// Add a choice question over `(value, description)` options.
    #[must_use]
    pub fn choice<V: Into<String>, D: Into<String>>(
        mut self,
        name: impl Into<String>,
        instructions: impl Into<String>,
        choices: impl IntoIterator<Item = (V, D)>,
    ) -> Self {
        self.questions.push(DecisionQuestion::Choice {
            name: name.into(),
            instructions: instructions.into(),
            choices: choices
                .into_iter()
                .map(|(value, description)| ChoiceOption {
                    value: value.into(),
                    description: Some(description.into()),
                })
                .collect(),
        });
        self
    }

    /// Add a score question over `(label, description)` levels, lowest
    /// first.
    #[must_use]
    pub fn score<L: Into<String>, D: Into<String>>(
        mut self,
        name: impl Into<String>,
        instructions: impl Into<String>,
        levels: impl IntoIterator<Item = (L, D)>,
    ) -> Self {
        self.questions.push(DecisionQuestion::Score {
            name: name.into(),
            instructions: instructions.into(),
            levels: levels
                .into_iter()
                .map(|(label, description)| Level {
                    label: label.into(),
                    description: Some(description.into()),
                })
                .collect(),
        });
        self
    }

    /// Echoed back and kept with the call's record.
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .get_or_insert_with(BTreeMap::new)
            .insert(key.into(), value.into());
        self
    }
}

/// One option's probability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceProbability {
    pub value: String,
    pub probability: f64,
}

/// One level's probability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelProbability {
    pub value: u32,
    pub label: String,
    pub probability: f64,
}

/// One answer, in request order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DecisionAnswer {
    Predicate {
        name: String,
        probability: f64,
    },
    Choice {
        name: String,
        choice: String,
        probabilities: Vec<ChoiceProbability>,
        confidence: f64,
    },
    Score {
        name: String,
        score: f64,
        probabilities: Vec<LevelProbability>,
        confidence: f64,
    },
    /// The question could not be judged (an input the model does not read).
    Refusal {
        name: String,
        code: String,
        #[serde(default)]
        message: String,
    },
}

impl DecisionAnswer {
    /// The question's name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Predicate { name, .. }
            | Self::Choice { name, .. }
            | Self::Score { name, .. }
            | Self::Refusal { name, .. } => name,
        }
    }
}

/// Token counts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub cached_input_tokens: u64,
}

/// Ours, beyond OpenAI's shape: which door answered and how fast.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Served {
    /// `clef`, `pylon:<slug>`, `jev`, or `vertex`; none for a request no
    /// door was asked (every question refused).
    #[serde(default)]
    pub door: Option<String>,
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub receipt: Option<String>,
}

/// A `POST /v1/decisions` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionsResponse {
    pub id: String,
    pub object: String,
    /// The model that answered, by its served name.
    pub model: String,
    pub answers: Vec<DecisionAnswer>,
    #[serde(default)]
    pub usage: DecisionUsage,
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub openagents: Served,
}

impl DecisionsResponse {
    /// The answer to question `name`.
    #[must_use]
    pub fn answer(&self, name: &str) -> Option<&DecisionAnswer> {
        self.answers.iter().find(|a| a.name() == name)
    }

    /// A predicate's probability.
    #[must_use]
    pub fn probability(&self, name: &str) -> Option<f64> {
        match self.answer(name)? {
            DecisionAnswer::Predicate { probability, .. } => Some(*probability),
            _ => None,
        }
    }

    /// A choice's picked value.
    #[must_use]
    pub fn choice(&self, name: &str) -> Option<&str> {
        match self.answer(name)? {
            DecisionAnswer::Choice { choice, .. } => Some(choice),
            _ => None,
        }
    }

    /// A score's probability-weighted level index.
    #[must_use]
    pub fn score(&self, name: &str) -> Option<f64> {
        match self.answer(name)? {
            DecisionAnswer::Score { score, .. } => Some(*score),
            _ => None,
        }
    }
}

/// A client for `POST /v1/decisions`.
#[derive(Debug, Clone)]
pub struct DecisionsClient {
    http: reqwest::Client,
    base_url: String,
    key: Option<crate::ApiKey>,
    timeout: Duration,
}

impl DecisionsClient {
    /// A client for the API root `base_url` (such as [`OPENAGENTS`], or a
    /// local gateway), with no key.
    ///
    /// # Errors
    ///
    /// [`Error::Config`] when `base_url` is not an http(s) URL.
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_owned();
        let parsed = url::Url::parse(&base_url)
            .map_err(|error| Error::Config(format!("`base_url` is not a URL: {error}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(Error::Config("`base_url` uses http or https".into()));
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .user_agent(concat!("jev-rust/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(Error::connection)?;
        Ok(Self {
            http,
            base_url,
            key: None,
            timeout: TIMEOUT,
        })
    }

    /// Send this key (an `oak_` key) as a bearer.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<crate::ApiKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// How long one call may take.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The API root.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Ask the questions. One attempt; a caller that retries sends the same
    /// `idempotency_key` with [`DecisionsClient::create_with`].
    ///
    /// # Errors
    ///
    /// [`Error::Api`] for a status outside 2xx (the one error object in its
    /// body), [`Error::Timeout`] or [`Error::Connection`] when no answer
    /// arrived, [`Error::ResponseValidation`] for a body that is not a
    /// decision.
    pub async fn create(&self, request: &DecisionsRequest) -> Result<DecisionsResponse> {
        self.create_with(request, None, &[]).await
    }

    /// [`DecisionsClient::create`] with an `Idempotency-Key` and extra
    /// headers (such as `("x-decision-order", "hosted")`).
    ///
    /// # Errors
    ///
    /// As [`DecisionsClient::create`].
    pub async fn create_with(
        &self,
        request: &DecisionsRequest,
        idempotency_key: Option<&str>,
        headers: &[(&str, &str)],
    ) -> Result<DecisionsResponse> {
        let url = format!("{}{PATH}", self.base_url);
        let mut call = self.http.post(&url).timeout(self.timeout).json(request);
        if let Some(key) = &self.key {
            call = call.bearer_auth(key.expose());
        }
        if let Some(key) = idempotency_key {
            call = call.header("idempotency-key", key);
        }
        for (name, value) in headers {
            call = call.header(*name, *value);
        }
        let response = call.send().await.map_err(|error| {
            if error.is_timeout() {
                Error::Timeout {
                    timeout: self.timeout,
                }
            } else {
                Error::connection(error)
            }
        })?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = response.bytes().await.map_err(Error::connection)?;
        if !(200..300).contains(&status) {
            return Err(Error::Api(Box::new(ApiError::new(
                format!("POST {url}"),
                status,
                headers,
                crate::transport::parse_body(&bytes),
            ))));
        }
        serde_json::from_slice(&bytes).map_err(|error| Error::ResponseValidation {
            status,
            field_path: format!("decision: {error}"),
            body: crate::transport::parse_body(&bytes).map(Box::new),
            request_id: headers
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn the_request_serializes_to_the_documented_shape() {
        let request = DecisionsRequest::new("clef", "Export fails in Safari.")
            .predicate("bug", "Is this a bug report?")
            .choice(
                "area",
                "Which area?",
                [("web", "The website."), ("app", "The app.")],
            )
            .score(
                "severity",
                "How severe?",
                [
                    ("Cosmetic", "Looks only."),
                    ("Blocked", "No way around it."),
                ],
            )
            .metadata("ticket", "T-1");
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({
                "model": "clef",
                "input": "Export fails in Safari.",
                "questions": [
                    {"type": "predicate", "name": "bug", "instructions": "Is this a bug report?"},
                    {"type": "choice", "name": "area", "instructions": "Which area?",
                     "choices": [{"value": "web", "description": "The website."},
                                 {"value": "app", "description": "The app."}]},
                    {"type": "score", "name": "severity", "instructions": "How severe?",
                     "levels": [{"label": "Cosmetic", "description": "Looks only."},
                                {"label": "Blocked", "description": "No way around it."}]}
                ],
                "metadata": {"ticket": "T-1"}
            })
        );
    }

    fn sample() -> Value {
        json!({
            "id": "dec_01", "object": "decision", "model": "clef-flash",
            "answers": [
                {"type": "predicate", "name": "bug", "probability": 0.9},
                {"type": "choice", "name": "area", "choice": "web",
                 "probabilities": [{"value": "web", "probability": 0.8}, {"value": "app", "probability": 0.2}],
                 "confidence": 0.8},
                {"type": "score", "name": "severity", "score": 0.3,
                 "probabilities": [{"value": 0, "label": "Cosmetic", "probability": 0.7},
                                   {"value": 1, "label": "Blocked", "probability": 0.3}],
                 "confidence": 0.7},
                {"type": "refusal", "name": "photo", "code": "unsupported_input", "message": "no images"}
            ],
            "usage": {"input_tokens": 212},
            "openagents": {"door": "clef", "latency_ms": 120, "cost": {"price_sats": 0, "price_usd": "0.000000"}}
        })
    }

    #[test]
    fn the_response_reads_each_answer_type() {
        let response: DecisionsResponse = serde_json::from_value(sample()).unwrap();
        assert_eq!(response.probability("bug"), Some(0.9));
        assert_eq!(response.choice("area"), Some("web"));
        assert_eq!(response.score("severity"), Some(0.3));
        assert!(
            matches!(response.answer("photo"), Some(DecisionAnswer::Refusal { code, .. }) if code == "unsupported_input")
        );
        assert_eq!(response.openagents.door.as_deref(), Some("clef"));
        assert_eq!(response.usage.input_tokens, 212);
    }

    /// One HTTP exchange on a local socket: answers `status` with `body`,
    /// and hands back the request it read.
    async fn serve_once(status: u16, body: Value) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut seen = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let n = socket.read(&mut buffer).await.unwrap();
                seen.extend_from_slice(&buffer[..n]);
                let text = String::from_utf8_lossy(&seen);
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text[..end]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if seen.len() >= end + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let payload = body.to_string();
            let reply = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
                payload.len()
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&seen).into_owned()
        });
        (url, task)
    }

    #[tokio::test]
    async fn create_posts_the_request_and_reads_the_decision() {
        let (url, seen) = serve_once(200, sample()).await;
        let client = DecisionsClient::new(format!("{url}/api")).unwrap();
        let request = DecisionsRequest::new("openagents/decide", "x").predicate("bug", "Bug?");
        let response = client
            .create_with(&request, Some("k-1"), &[("x-decision-order", "hosted")])
            .await
            .unwrap();
        assert_eq!(response.id, "dec_01");
        let sent = seen.await.unwrap();
        assert!(sent.starts_with("POST /api/v1/decisions HTTP/1.1"));
        assert!(sent.to_ascii_lowercase().contains("idempotency-key: k-1"));
        assert!(
            sent.to_ascii_lowercase()
                .contains("x-decision-order: hosted")
        );
        assert!(sent.contains("\"model\":\"openagents/decide\""));
    }

    #[tokio::test]
    async fn an_error_object_becomes_an_api_error() {
        let (url, _seen) = serve_once(
            503,
            json!({"error": {"type": "unavailable", "code": "unavailable", "message": "No decision door could answer right now."}}),
        )
        .await;
        let client = DecisionsClient::new(url).unwrap();
        let error = client
            .create(&DecisionsRequest::new("clef", "x").predicate("a", "A?"))
            .await
            .unwrap_err();
        match error {
            Error::Api(api) => {
                assert_eq!(api.status, 503);
                assert_eq!(api.message(), "No decision door could answer right now.");
            }
            other => panic!("{other:?}"),
        }
    }
}
