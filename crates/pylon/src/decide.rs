//! Decisions a pylon answers (#11225): NIP-DEC jobs (`25910` in, `27010`
//! status and `26910` result out, NIP-44 encrypted) answered by a local
//! System One server, in production Psionic's Clef lane
//! (`psionic-openai-server -m Clef-Flash-Q4_K_M.gguf`, `POST /v1/systemone`).
//!
//! The beacon advertises the service as `<pylon key>:pylon/decision` on the
//! `cj-decision` lane, its model as the served identity
//! (`clef-flash@sha256:<artifact digest>`), and the pylon's free slots. The
//! gateway's `POST /v1/systemone` reads those beacons and sends each
//! decision to a pylon with a free slot. The work is free (`free-v1`).

use std::sync::Mutex;
use std::time::Duration;

use nostr::decision::{Refusal, code_for_http_status};
use serde_json::{Map, Value, json};

use crate::engine::Pending;

/// The capability ID a decision service carries in the beacon:
/// `<pylon key>:pylon/decision`.
#[must_use]
pub fn capability(pubkey: &str) -> String {
    format!("{pubkey}:pylon/decision")
}

/// What answered a decision, as the pylon names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The served model's name, such as `clef-flash`.
    pub model: String,
    /// The loaded artifact's digest (`sha256:…`), when the server names it.
    pub artifact_digest: Option<String>,
}

impl Identity {
    /// The beacon's `model` for the service: `clef-flash@sha256:…`, or the
    /// bare name when the server names no digest. At most 128 bytes.
    #[must_use]
    pub fn advertised(&self) -> String {
        let text = match &self.artifact_digest {
            Some(digest) => format!("{}@{digest}", self.model),
            None => self.model.clone(),
        };
        text.chars().take(128).collect()
    }
}

/// A System One server a pylon answers decisions with.
pub trait Decider: Send + Sync {
    /// What the server serves, as of the last check.
    fn identity(&self) -> Identity;
    /// Whether the server answers right now (also refreshes the identity).
    fn healthy(&self) -> Pending<'_, bool>;
    /// Answer `state` and `questions`: the `POST /v1/systemone` response
    /// body, or the server's typed refusal.
    fn decide<'a>(
        &'a self,
        state: &'a Value,
        questions: &'a Map<String, Value>,
    ) -> Pending<'a, Result<Value, Refusal>>;
}

/// A loopback System One server (Psionic's Clef lane).
pub struct Clef {
    base: String,
    /// The model to ask for; `None` takes the first decision model the
    /// server lists.
    wanted: Option<String>,
    identity: Mutex<Identity>,
    client: reqwest::Client,
}

impl Clef {
    /// The server at `base` (such as `http://127.0.0.1:18096`). `model`
    /// names the decision model when the server serves several.
    ///
    /// # Errors
    ///
    /// When the HTTP client cannot be built.
    pub fn new(base: &str, model: Option<&str>) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            wanted: model.map(str::to_string),
            identity: Mutex::new(Identity {
                model: model.unwrap_or("clef-flash").to_string(),
                artifact_digest: None,
            }),
            client,
        })
    }

    /// Read `GET /v1/models` and keep the decision model's identity.
    ///
    /// # Errors
    ///
    /// When the server does not answer or lists no decision model.
    pub async fn refresh(&self) -> Result<Identity, String> {
        let body: Value = self
            .client
            .get(format!("{}/v1/models", self.base))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("the decision server at {} did not answer: {e}", self.base))?
            .json()
            .await
            .map_err(|e| format!("the decision server's model list is not JSON: {e}"))?;
        let entry = body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| {
                let decides = entry["capabilities"]
                    .as_array()
                    .is_some_and(|caps| caps.iter().any(|c| c == "decision"));
                let named = self
                    .wanted
                    .as_deref()
                    .is_none_or(|wanted| entry["id"] == wanted);
                decides && named
            })
            .ok_or_else(|| format!("the server at {} lists no decision model", self.base))?;
        let identity = Identity {
            model: entry["id"].as_str().unwrap_or("clef-flash").to_string(),
            artifact_digest: entry["psionic"]["artifact_digest"]
                .as_str()
                .map(str::to_string),
        };
        if let Ok(mut held) = self.identity.lock() {
            *held = identity.clone();
        }
        Ok(identity)
    }
}

impl Decider for Clef {
    fn identity(&self) -> Identity {
        self.identity
            .lock()
            .map(|held| held.clone())
            .unwrap_or(Identity {
                model: "clef-flash".into(),
                artifact_digest: None,
            })
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async move { self.refresh().await.is_ok() })
    }

    fn decide<'a>(
        &'a self,
        state: &'a Value,
        questions: &'a Map<String, Value>,
    ) -> Pending<'a, Result<Value, Refusal>> {
        Box::pin(async move {
            let model = self.identity().model;
            let response = self
                .client
                .post(format!("{}/v1/systemone", self.base))
                .json(&json!({"model": model, "state": state, "questions": questions}))
                .send()
                .await
                .map_err(|e| {
                    Refusal::new("unavailable").message(format!("the model server is down: {e}"))
                })?;
            let status = response.status().as_u16();
            let body: Value = response.json().await.unwrap_or(Value::Null);
            if status == 200 && body["answers"].is_object() {
                return Ok(body);
            }
            let code = body["error"]["code"]
                .as_str()
                .map_or_else(|| code_for_http_status(status).to_string(), str::to_string);
            let message = body["error"]["message"]
                .as_str()
                .unwrap_or("the model server refused")
                .chars()
                .take(512)
                .collect::<String>();
            Err(Refusal::new(code).message(message))
        })
    }
}

/// A fake decider for tests: every `noul` is 0.75, every `choice` picks
/// its first option at 0.9, every `score` its last level.
pub struct Fixed {
    pub identity: Identity,
}

impl Decider for Fixed {
    fn identity(&self) -> Identity {
        self.identity.clone()
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn decide<'a>(
        &'a self,
        _state: &'a Value,
        questions: &'a Map<String, Value>,
    ) -> Pending<'a, Result<Value, Refusal>> {
        Box::pin(async move {
            let mut answers = Map::new();
            for (id, question) in questions {
                let answer = match question["type"].as_str() {
                    Some("choice") => {
                        let options: Vec<&String> = question["criteria"]
                            .as_object()
                            .map(|o| o.keys().collect())
                            .unwrap_or_default();
                        let n = options.len().max(1) as f64;
                        let rest = if options.len() > 1 {
                            0.1 / (n - 1.0)
                        } else {
                            0.0
                        };
                        let mut probabilities = Map::new();
                        for (i, option) in options.iter().enumerate() {
                            let p = if i == 0 {
                                if options.len() > 1 { 0.9 } else { 1.0 }
                            } else {
                                rest
                            };
                            probabilities.insert((*option).clone(), json!(p));
                        }
                        let first = options.first().map(|s| s.as_str()).unwrap_or_default();
                        json!({"type": "choice", "choice": first,
                               "confidence": if options.len() > 1 { 0.9 } else { 1.0 },
                               "probabilities": probabilities})
                    }
                    Some("score") => {
                        let levels = question["criteria"].as_array().cloned().unwrap_or_default();
                        let last = levels.len().saturating_sub(1);
                        let mut probabilities = Map::new();
                        let mut legend = Map::new();
                        for (i, level) in levels.iter().enumerate() {
                            probabilities
                                .insert(i.to_string(), json!(if i == last { 1.0 } else { 0.0 }));
                            legend.insert(i.to_string(), level.clone());
                        }
                        json!({"type": "score", "score": last as f64, "confidence": 1.0,
                               "legend": legend, "probabilities": probabilities})
                    }
                    _ => json!({"type": "noul", "noul": 0.75}),
                };
                answers.insert(id.clone(), answer);
            }
            Ok(json!({"model": self.identity.model, "answers": answers,
                      "usage": {"input_tokens": 1, "output_tokens": 0}}))
        })
    }
}
