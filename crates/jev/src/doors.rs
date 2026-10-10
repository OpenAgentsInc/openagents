//! Jev's doors other than TypeSafe's own, and the failover across them.
//!
//! Jev answers at three HTTP doors with the same request body and the same
//! answer shape (NIP-DEC, "Doors and the backup door"). A server that
//! holds every door's key asks them in this order:
//!
//! | Order | Door | Route | Model | Key |
//! | --- | --- | --- | --- | --- |
//! | 1 | Vercel AI Gateway | `POST https://ai-gateway.vercel.sh/typesafe/v1/systemone` | `typesafe-ai/jev` | `AI_GATEWAY_API_KEY` |
//! | 2 | OpenRouter | `POST https://openrouter.ai/api/alpha/decisions` | `typesafe/jev-1.13` | `OPENROUTER_API_KEY` |
//! | 3 | TypeSafe | `POST https://api.typesafe.ai/v1/systemone` | `jev-1.13.0` | `TYPESAFE_API_KEY` |
//!
//! The gateway is the primary: it routes Jev to TypeSafe itself, with the
//! owner's key as its own fallback, so the gateway handles provider
//! routing. TypeSafe direct is the final backup.
//!
//! The gateway's route is its TypeSafe-compatible API, which "implements
//! the TypeSafe request and response shapes" (Vercel, "TypeSafe API with
//! AI Gateway"): `noul`/`choice`/`score` questions in, the same typed
//! answers and `usage.input_tokens` out, plus `provider_metadata.gateway`
//! with the routing and the cost as a decimal string. Its own errors are
//! `{"message", "error_type"}`; a provider's errors pass through unchanged.
//!
//! [`Failover`] is an [`Exchange`] whose base door is TypeSafe's: every
//! route goes there except a decision, which asks the doors in order. By
//! default TypeSafe is asked first; [`Failover::primary_last`] asks every
//! fallback door first and TypeSafe last, the order above. A door is left
//! for the next only when it could not answer for a reason of its own
//! ([`fails_over_reply`]: 402, 408, 413, 429, any 5xx, the door's own key,
//! account, quota, or admission refusals, a timeout, or no connection).
//! A request too large or unsupported for one door (a 413, a
//! `not_admitted` code, or a 400 that names that door's own limit, such
//! as Ollama's 26-option cap) is that door's limit, not the question's:
//! a later door with other limits may take it. A refusal of the question
//! itself (a malformed request, `invalid_request` at 400) never fails
//! over: the next door would refuse the same question. An answer from any door but TypeSafe names it
//! in `service.door`, so a decision record says which door answered; when
//! no door answers, the first door's own refusal stands. Each door's key
//! is held here, sent only to its own door, and never logged.
//!
//! A door that refuses for its key or its account (401 or 402) will refuse
//! the next call the same way, so [`Failover`] remembers it: for
//! [`BENCH`] (a circuit breaker) that door is skipped and the call goes
//! straight to the next one, then the door is asked again. The bench is
//! logged once, when it starts. When a benched first door is skipped and
//! no other door answers, its remembered refusal stands.
//!
//! A door may also be one another service carries ([`Door::carried`]): the
//! decision is handed to that service's [`Exchange`] as it came, in
//! TypeSafe's shape. A computer whose own TypeSafe key cannot pay uses this
//! for its last door, the hosted OpenAgents decision service
//! (`crates/jev-hosted`). Its answer keeps the `service` object the service
//! added (the door it used, its build) and gains `service.exchange`, the
//! service that carried it, so a decision record says the hosted service
//! answered.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::config::ApiKey;
use crate::exchange::{Call, Exchange, Failure, Pending, Reply};
use crate::nip_dec::{canonical_model, code_for_http_status, openrouter_model};

/// TypeSafe's door: the base door every route but a decision goes to,
/// and the final backup for a decision when the fallbacks lead.
pub const TYPESAFE_DOOR: &str = "https://api.typesafe.ai";

/// The Vercel AI Gateway's door, as an answer's `service.door` names it.
pub const GATEWAY_DOOR: &str = "https://ai-gateway.vercel.sh";
/// The gateway's TypeSafe-compatible System One route.
pub const GATEWAY_URL: &str = "https://ai-gateway.vercel.sh/typesafe/v1/systemone";
/// The variable holding the gateway key.
pub const GATEWAY_KEY_VAR: &str = "AI_GATEWAY_API_KEY";
/// The gateway's name for Jev. The gateway serves one Jev, its current
/// one, and names no version.
pub const GATEWAY_MODEL: &str = "typesafe-ai/jev";

/// OpenRouter's door, as an answer's `service.door` names it.
pub const OPENROUTER_DOOR: &str = "https://openrouter.ai";
/// OpenRouter's Decisions API.
pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/alpha/decisions";
/// The variable holding the OpenRouter key.
pub const OPENROUTER_KEY_VAR: &str = "OPENROUTER_API_KEY";

/// The route a decision takes at TypeSafe's door, relative to its base URL.
pub const SYSTEM_ONE_PATH: &str = "/v1/systemone";

/// How a door names Jev.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Naming {
    /// The canonical name (`jev-1.13.0`): TypeSafe and OpenAgents gateways.
    Canonical,
    /// OpenRouter's name (`typesafe/jev-1.13`).
    OpenRouter,
    /// The Vercel AI Gateway's name (`typesafe-ai/jev`).
    Gateway,
}

impl Naming {
    /// Read a configuration word: `canonical`, `openrouter`, or `gateway`.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "canonical" => Some(Self::Canonical),
            "openrouter" => Some(Self::OpenRouter),
            "gateway" => Some(Self::Gateway),
            _ => None,
        }
    }

    /// The name this door knows `model` by. An alias resolves to its
    /// canonical name first, so `typesafe/jev-1.13` asks the gateway for
    /// `typesafe-ai/jev` and TypeSafe for `jev-1.13.0`.
    #[must_use]
    pub fn model(self, model: &str) -> String {
        let canonical = canonical_model(model);
        match self {
            Self::Canonical => canonical.to_string(),
            Self::OpenRouter => openrouter_model(canonical).into_owned(),
            Self::Gateway if canonical.contains('/') => canonical.to_string(),
            Self::Gateway => GATEWAY_MODEL.to_string(),
        }
    }
}

/// One fallback door as configuration names it: where it answers, the
/// variable its key is in, and how it names Jev.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fallback {
    /// The door named for a person, such as "the Vercel AI Gateway".
    pub name: &'static str,
    /// What an answer from this door names in `service.door`.
    pub door: &'static str,
    /// The full URL a decision posts to.
    pub url: &'static str,
    /// The environment variable holding the door's key.
    pub key_var: &'static str,
    /// How the door names Jev.
    pub naming: Naming,
}

/// The fallback doors, in the order they are asked: the Vercel AI Gateway,
/// then OpenRouter. With [`Failover::primary_last`] they come before
/// TypeSafe; otherwise after it.
pub const FALLBACKS: [Fallback; 2] = [
    Fallback {
        name: "the Vercel AI Gateway",
        door: GATEWAY_DOOR,
        url: GATEWAY_URL,
        key_var: GATEWAY_KEY_VAR,
        naming: Naming::Gateway,
    },
    Fallback {
        name: "OpenRouter",
        door: OPENROUTER_DOOR,
        url: OPENROUTER_URL,
        key_var: OPENROUTER_KEY_VAR,
        naming: Naming::OpenRouter,
    },
];

/// Refusal codes that are a door's own reason, not the question's: its
/// key, its account, its model list, its quota, or its capacity.
const DOOR_OWN_CODES: &[&str] = &[
    "unauthenticated",
    "payment_required",
    "not_admitted",
    "rate_limited",
    "quota_exhausted",
    "internal",
    "door_unavailable",
    "identity_mismatch",
    "busy",
    "unavailable",
    "timeout",
    "overloaded",
];

/// Refusal codes a 400 carries when the request is fine but too large or
/// unsupported for that door: its size, option or context limits, or a
/// feature it lacks.
const DOOR_LIMIT_CODES: &[&str] = &[
    "request_too_large",
    "payload_too_large",
    "context_length_exceeded",
    "unsupported",
    "unsupported_input",
];

/// Error messages of doors that refuse their own limits with a bare 400
/// and no code: Ollama 0.40's Clef path (2–26 options per question, a
/// 64 KiB text and schema cap) and llama.cpp's decision server (the prompt
/// is larger than its physical batch or its context). A fixed table of
/// known protocol errors, matched case-insensitively.
const DOOR_LIMIT_MESSAGES: &[&str] = &[
    "criteria must contain 2–26 candidates",
    "criteria must contain 2-26 candidates",
    "must not exceed 64 kib",
    "input is too large to process",
    "exceeds the available context size",
    "the request exceeds the available context",
];

/// Whether a door's refusal may be asked again at the next door: the door
/// could not pay (402), has no such model or route (404), timed out (408),
/// would not take a request this large (413), was over a rate or quota
/// (429), failed (any 5xx), or refused for a reason of its own (its key,
/// its account, its model list, its admission: a [`DOOR_OWN_CODES`] code
/// such as `not_admitted`, or a [`DOOR_LIMIT_CODES`] code). A refusal of
/// the question itself, such as `invalid_request` at 400 or 422, never
/// fails over. [`fails_over_reply`] also reads the message of a bare 400.
#[must_use]
pub fn fails_over(status: u16, code: Option<&str>) -> bool {
    if matches!(status, 402 | 404 | 408 | 413 | 429 | 500..=599) {
        return true;
    }
    let code = code.unwrap_or_else(|| code_for_http_status(status));
    DOOR_OWN_CODES.contains(&code) || (status == 400 && DOOR_LIMIT_CODES.contains(&code))
}

/// [`fails_over`] for a whole refusal body: a 400 whose message is one of
/// [`DOOR_LIMIT_MESSAGES`] (a door that names its own limit without a
/// code, such as Ollama's 26-option cap) is that door's refusal too.
#[must_use]
pub fn fails_over_reply(status: u16, body: &Value) -> bool {
    if fails_over(status, error_code(body)) {
        return true;
    }
    if status != 400 {
        return false;
    }
    let message = body
        .get("error")
        .and_then(|error| error.get("message").or(Some(error)))
        .and_then(Value::as_str)
        .or_else(|| body.get("message").and_then(Value::as_str))
        .unwrap_or_default()
        .to_lowercase();
    DOOR_LIMIT_MESSAGES
        .iter()
        .any(|limit| message.contains(limit))
}

/// The refusal code an error body carries: `error.code` when it is a
/// string (TypeSafe's and OpenAgents' shape), the gateway's top-level
/// `error_type`, or `None` (a numeric `error.code` is OpenRouter's HTTP
/// status; read the status instead).
#[must_use]
pub fn error_code(body: &Value) -> Option<&str> {
    body.get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
        .or_else(|| body.get("error_type").and_then(Value::as_str))
        .filter(|code| !code.is_empty())
}

/// An error body in the one shape every reader takes,
/// `{"error": {"code", "message"}}`: the gateway's `{"message",
/// "error_type"}` and OpenRouter's numeric `error.code` are rewritten, and
/// a body already in that shape is kept.
#[must_use]
pub fn normalize_error(status: u16, body: &[u8]) -> Value {
    let parsed: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let code = error_code(&parsed)
        .map(str::to_string)
        .unwrap_or_else(|| code_for_http_status(status).to_string());
    let message = parsed
        .get("error")
        .and_then(|error| error.get("message"))
        .or_else(|| parsed.get("message"))
        .and_then(Value::as_str)
        .map_or_else(
            || format!("The door returned HTTP {status} without an error message."),
            str::to_string,
        );
    json!({"error": {"code": code, "message": message}})
}

/// Bring a fallback door's answer to TypeSafe's shape where it differs:
/// the gateway prices a call in `provider_metadata.gateway.cost` (a
/// decimal string), which becomes `usage.cost` when the answer has none.
/// Returns whether anything changed.
pub fn normalize_answer(body: &mut Value) -> bool {
    let cost = body
        .pointer("/provider_metadata/gateway/cost")
        .and_then(|cost| match cost {
            Value::String(text) => text.parse::<f64>().ok(),
            Value::Number(number) => number.as_f64(),
            _ => None,
        })
        .filter(|cost| cost.is_finite() && *cost >= 0.0);
    let Some(cost) = cost else {
        return false;
    };
    let Some(map) = body.as_object_mut() else {
        return false;
    };
    let usage = map.entry("usage").or_insert_with(|| json!({}));
    match usage.as_object_mut() {
        Some(usage) if !usage.contains_key("cost") => {
            usage.insert("cost".to_string(), json!(cost));
            true
        }
        _ => false,
    }
}

/// How long a door that refused for its key or account is skipped before
/// it is asked again.
pub const BENCH: Duration = Duration::from_secs(300);

/// Whether a door's refusal is about its key or its account (401, 402):
/// the next call would be refused the same way, so the door is benched.
#[must_use]
pub fn benches(status: u16) -> bool {
    matches!(status, 401 | 402)
}

/// A door skipped until `until`, and the refusal that benched it.
#[derive(Debug, Clone)]
struct Benched {
    until: Instant,
    refusal: Reply,
}

/// How [`Failover`] reaches a door.
#[derive(Clone)]
enum Carrier {
    /// HTTP, under the door's own key.
    Http(ApiKey),
    /// Another service, which carries the call itself.
    Exchange(Arc<dyn Exchange>),
}

/// A door [`Failover`] asks, with its key or its carrier.
#[derive(Clone)]
pub struct Door {
    /// What an answer from this door names in `service.door`.
    pub door: String,
    /// For the primary, its base URL (a call's path is appended); for a
    /// fallback, the full decision URL; for a carried door, what carries
    /// it.
    pub url: String,
    /// How the door names Jev.
    pub naming: Naming,
    carrier: Carrier,
}

impl Door {
    /// A door at `url` under `key`.
    #[must_use]
    pub fn new(
        door: impl Into<String>,
        url: impl Into<String>,
        naming: Naming,
        key: ApiKey,
    ) -> Self {
        Self {
            door: door.into(),
            url: url.into(),
            naming,
            carrier: Carrier::Http(key),
        }
    }

    /// A door another service carries: each decision attempt goes to
    /// `exchange` as it came (TypeSafe's shape, the canonical model), and
    /// this process holds no key for it. `door` names it in logs and in
    /// [`Failover`]'s description.
    #[must_use]
    pub fn carried(door: impl Into<String>, exchange: Arc<dyn Exchange>) -> Self {
        Self {
            door: door.into(),
            url: exchange.service(),
            naming: Naming::Canonical,
            carrier: Carrier::Exchange(exchange),
        }
    }

    /// A fallback door from its configuration and its key.
    #[must_use]
    pub fn fallback(fallback: &Fallback, key: ApiKey) -> Self {
        Self::new(fallback.door, fallback.url, fallback.naming, key)
    }

    /// The door's key, for the one caller that must scrub it from what it
    /// writes; `None` for a carried door. Never log it.
    #[must_use]
    pub fn key(&self) -> Option<&ApiKey> {
        match &self.carrier {
            Carrier::Http(key) => Some(key),
            Carrier::Exchange(_) => None,
        }
    }

    /// Whether another service carries this door ([`Door::carried`]).
    #[must_use]
    pub fn is_carried(&self) -> bool {
        matches!(self.carrier, Carrier::Exchange(_))
    }
}

impl fmt::Debug for Door {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Door")
            .field("door", &self.door)
            .field("url", &self.url)
            .field("naming", &self.naming)
            .field(
                "key",
                &match self.carrier {
                    Carrier::Http(_) => "***",
                    Carrier::Exchange(_) => "carried",
                },
            )
            .finish()
    }
}

/// TypeSafe and the fallback doors, asked in order (module docs).
#[derive(Debug, Clone)]
pub struct Failover {
    http: reqwest::Client,
    primary: Door,
    fallbacks: Vec<Door>,
    primary_timeout: Option<Duration>,
    primary_last: bool,
    bench: Duration,
    benched: Arc<Mutex<HashMap<String, Benched>>>,
}

impl Failover {
    /// Ask `primary` first and each of `fallbacks` after it. `primary` is a
    /// base URL; every route goes there, and only a decision
    /// (`POST /v1/systemone`) fails over.
    #[must_use]
    pub fn new(primary: Door, fallbacks: Vec<Door>) -> Self {
        Self {
            http: reqwest::Client::new(),
            primary,
            fallbacks,
            primary_timeout: None,
            primary_last: false,
            bench: BENCH,
            benched: Arc::default(),
        }
    }

    /// Ask a decision at every fallback door first, in order, and at the
    /// primary last: the primary becomes the final backup. Every route
    /// other than a decision still goes to the primary alone.
    #[must_use]
    pub fn primary_last(mut self) -> Self {
        self.primary_last = true;
        self
    }

    /// How long a door that refused for its key or account is skipped
    /// ([`BENCH`] unless set).
    #[must_use]
    pub fn bench(mut self, bench: Duration) -> Self {
        self.bench = bench;
        self
    }

    /// The refusal that benched `door`, while its bench lasts.
    fn benched_refusal(&self, door: &str) -> Option<Reply> {
        let mut benched = self
            .benched
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match benched.get(door) {
            Some(bench) if Instant::now() < bench.until => Some(bench.refusal.clone()),
            Some(_) => {
                benched.remove(door);
                None
            }
            None => None,
        }
    }

    /// Bench `door` when `reply` refused for its key or account; logged once
    /// per bench.
    fn note(&self, door: &str, reply: &Reply) {
        if !benches(reply.status) {
            return;
        }
        let mut benched = self
            .benched
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        benched.insert(
            door.to_string(),
            Benched {
                until: Instant::now() + self.bench,
                refusal: reply.clone(),
            },
        );
        tracing::warn!(
            target: "jev",
            door = %door,
            status = reply.status,
            bench_s = self.bench.as_secs(),
            "a door refused for its key or account; skipping it until the bench ends"
        );
    }

    /// Cap the share of an attempt the first door asked may take, so a
    /// door that hangs leaves the doors after it the rest of the attempt's
    /// time. Unset, the first door may take the whole attempt.
    #[must_use]
    pub fn primary_timeout(mut self, timeout: Duration) -> Self {
        self.primary_timeout = Some(timeout);
        self
    }

    /// The fallback doors, in order.
    #[must_use]
    pub fn fallbacks(&self) -> &[Door] {
        &self.fallbacks
    }

    /// Ask `door` once: over HTTP at `url` under its key, or through its
    /// carrier with `body` in place of the call's.
    async fn ask(
        &self,
        door: &Door,
        url: &str,
        call: &Call,
        body: Option<Vec<u8>>,
        timeout: Duration,
    ) -> Result<Reply, Failure> {
        match &door.carrier {
            Carrier::Http(key) => self.send(url, key, call, body, timeout).await,
            Carrier::Exchange(exchange) => {
                let carried = Call {
                    body,
                    timeout,
                    ..call.clone()
                };
                tokio::time::timeout(timeout, exchange.exchange(carried))
                    .await
                    .unwrap_or(Err(Failure::Timeout))
            }
        }
    }

    async fn send(
        &self,
        url: &str,
        key: &ApiKey,
        call: &Call,
        body: Option<Vec<u8>>,
        timeout: Duration,
    ) -> Result<Reply, Failure> {
        let method = reqwest::Method::from_bytes(call.method.as_bytes())
            .map_err(|_| Failure::Unreachable(format!("{} is not an HTTP method", call.method)))?;
        let mut request = self
            .http
            .request(method, url)
            .bearer_auth(key.expose())
            .header("accept", "application/json")
            .header("x-attempt", call.attempt.to_string())
            .timeout(timeout);
        if let Some(idempotency) = &call.idempotency_key {
            request = request.header("idempotency-key", idempotency);
        }
        if let Some(body) = body {
            request = request
                .header("content-type", "application/json")
                .body(body);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) if error.is_timeout() => return Err(Failure::Timeout),
            Err(error) => {
                return Err(Failure::Unreachable(format!(
                    "the call to {url} failed: {}",
                    without_url(&error)
                )));
            }
        };
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_string(), value.to_str().ok()?.to_string()))
            })
            .collect();
        let body = match response.bytes().await {
            Ok(bytes) => bytes.to_vec(),
            Err(error) if error.is_timeout() => return Err(Failure::Timeout),
            Err(error) => {
                return Err(Failure::Unreachable(format!(
                    "the answer from {url} did not arrive: {}",
                    without_url(&error)
                )));
            }
        };
        Ok(Reply {
            status,
            headers,
            body,
        })
    }

    /// The doors a decision asks, in order: the primary first, or, with
    /// [`Failover::primary_last`], every fallback first and the primary
    /// last.
    fn order(&self) -> Vec<&Door> {
        let mut order = Vec::with_capacity(self.fallbacks.len() + 1);
        if !self.primary_last {
            order.push(&self.primary);
        }
        order.extend(self.fallbacks.iter());
        if self.primary_last {
            order.push(&self.primary);
        }
        order
    }

    async fn carry(&self, call: Call) -> Result<Reply, Failure> {
        let began = Instant::now();
        let decision = call.method.eq_ignore_ascii_case("POST") && call.path == SYSTEM_ONE_PATH;
        let base = format!("{}{}", self.primary.url.trim_end_matches('/'), call.path);
        // A primary that names Jev its own way (the person's gateway or
        // OpenRouter key leading, `model-access`) takes a decision at its
        // full URL with its own model name, as a fallback door does.
        let renamed = self.primary.naming != Naming::Canonical && !self.primary.is_carried();
        if !decision || (self.fallbacks.is_empty() && !renamed) {
            return self
                .ask(&self.primary, &base, &call, call.body.clone(), call.timeout)
                .await;
        }
        let request: Option<Value> = call
            .body
            .as_deref()
            .and_then(|body| serde_json::from_slice(body).ok());
        let order = self.order();
        // The first door's outcome, which stands when no door answers, and
        // why it could not answer.
        let mut first: Option<(Result<Reply, Failure>, String)> = None;
        let mut asked = 0usize;
        for (at, door) in order.iter().enumerate() {
            let is_primary = std::ptr::eq(*door, &self.primary);
            let left = call.timeout.saturating_sub(began.elapsed());
            if left.is_zero() {
                break;
            }
            let (answered, benched) = if let Some(refusal) = self.benched_refusal(&door.door) {
                (Ok(refusal), true)
            } else {
                // The first door asked keeps only its capped share, so a
                // door that hangs leaves the others the rest.
                let timeout = match self.primary_timeout {
                    Some(cap) if asked == 0 && at + 1 < order.len() => cap.min(left),
                    _ => left,
                };
                asked += 1;
                let (url, body) = if (is_primary && !renamed) || door.is_carried() {
                    (base.clone(), call.body.clone())
                } else {
                    let body = request.clone().map(|mut body| {
                        if let Some(model) = body.get("model").and_then(Value::as_str) {
                            body["model"] = Value::String(door.naming.model(model));
                        }
                        serde_json::to_vec(&body).unwrap_or_default()
                    });
                    (door.url.clone(), body)
                };
                (self.ask(door, &url, &call, body, timeout).await, false)
            };
            match answered {
                Ok(mut reply) if (200..300).contains(&reply.status) => {
                    if (!is_primary || renamed)
                        && let Ok(mut value) = serde_json::from_slice::<Value>(&reply.body)
                    {
                        normalize_answer(&mut value);
                        if let Some(map) = value.as_object_mut() {
                            let service = match &door.carrier {
                                // The carrier's own `service` (the door it
                                // used, its build) stands, and names it.
                                Carrier::Exchange(exchange) => {
                                    let mut service = map
                                        .get("service")
                                        .filter(|service| service.is_object())
                                        .cloned()
                                        .unwrap_or_else(|| json!({}));
                                    service["exchange"] = json!(exchange.service());
                                    service
                                }
                                Carrier::Http(_) => json!({"door": door.door}),
                            };
                            map.insert("service".to_string(), service);
                        }
                        if let Ok(bytes) = serde_json::to_vec(&value) {
                            reply.body = bytes;
                        }
                    }
                    if let Some((_, why)) = &first {
                        tracing::info!(
                            target: "jev",
                            first = %order[0].door,
                            why = %why,
                            door = %door.door,
                            elapsed_ms = began.elapsed().as_millis(),
                            "a fallback door answered"
                        );
                    }
                    return Ok(reply);
                }
                Ok(reply) => {
                    let body: Value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
                    let code = error_code(&body).map(str::to_string);
                    let over = fails_over_reply(reply.status, &body);
                    if !benched {
                        tracing::info!(
                            target: "jev",
                            door = %door.door,
                            status = reply.status,
                            code = code.as_deref().unwrap_or("-"),
                            "a door refused"
                        );
                        if over {
                            self.note(&door.door, &reply);
                        }
                    }
                    if !over {
                        // A refusal of the question itself: the next door
                        // would refuse it too.
                        if first.is_none() {
                            return Ok(reply);
                        }
                        break;
                    }
                    let why = if benched {
                        format!("HTTP {} (benched)", reply.status)
                    } else {
                        format!("HTTP {}", reply.status)
                    };
                    first.get_or_insert((Ok(reply), why));
                }
                Err(failure) => {
                    let why = match &failure {
                        Failure::Timeout => "timeout",
                        Failure::Unreachable(_) => "unreachable",
                    };
                    tracing::info!(target: "jev", door = %door.door, why, "a door did not answer");
                    first.get_or_insert((Err(failure), why.to_string()));
                }
            }
        }
        first.map_or(Err(Failure::Timeout), |(outcome, _)| outcome)
    }
}

/// A reqwest error's text without the URL it carries (a URL never holds a
/// key here, but the door is already named beside it).
fn without_url(error: &reqwest::Error) -> String {
    let mut text = error.to_string();
    if let Some(url) = error.url() {
        text = text.replace(url.as_str(), "the door");
    }
    text
}

impl Exchange for Failover {
    fn exchange(&self, call: Call) -> Pending<'_> {
        Box::pin(self.carry(call))
    }

    fn service(&self) -> String {
        let doors: Vec<&str> = self.order().iter().map(|door| door.door.as_str()).collect();
        format!("doors {}", doors.join(" → "))
    }

    fn relays(&self) -> bool {
        false
    }
}

/// [`Failover`] as the shared exchange a client is built with.
#[must_use]
pub fn exchange(failover: Failover) -> Arc<dyn Exchange> {
    Arc::new(failover)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_door_names_jev_its_own_way() {
        assert_eq!(Naming::Canonical.model("typesafe/jev-1.13"), "jev-1.13.0");
        assert_eq!(Naming::OpenRouter.model("jev-1.13.0"), "typesafe/jev-1.13");
        assert_eq!(Naming::Gateway.model("jev-1.13.0"), "typesafe-ai/jev");
        assert_eq!(
            Naming::Gateway.model("typesafe/jev-1.13"),
            "typesafe-ai/jev"
        );
        assert_eq!(Naming::Gateway.model("jev-latest"), "typesafe-ai/jev");
        assert_eq!(Naming::parse("gateway"), Some(Naming::Gateway));
        assert_eq!(Naming::parse("nope"), None);
    }

    #[test]
    fn only_a_doors_own_refusals_fail_over() {
        for status in [402, 408, 429, 500, 502, 503, 524, 529] {
            assert!(fails_over(status, None), "{status}");
        }
        // A typed 402 fails over whatever its code says.
        assert!(fails_over(402, Some("insufficient_credits")));
        assert!(fails_over(401, Some("unauthenticated")));
        assert!(!fails_over(400, Some("invalid_request")));
        assert!(!fails_over(400, None));
        assert!(!fails_over(422, Some("invalid_request")));
        // A door's own size or admission limit is not the question's.
        assert!(fails_over(413, Some("limit_exceeded")));
        assert!(fails_over(413, None));
        assert!(fails_over(413, Some("not_admitted")));
        assert!(fails_over(400, Some("not_admitted")));
        assert!(fails_over(400, Some("request_too_large")));
        assert!(fails_over(400, Some("unsupported")));
        assert!(!fails_over(422, Some("unsupported")));
    }

    #[test]
    fn a_bare_400_naming_a_doors_limit_fails_over() {
        // Ollama 0.40: `{"error": "<message>"}`, no code.
        let ollama_options =
            json!({"error": "question \"answer\": criteria must contain 2–26 candidates"});
        assert!(fails_over_reply(400, &ollama_options));
        let ollama_size = json!({"error": "text and schema must not exceed 64 KiB"});
        assert!(fails_over_reply(400, &ollama_size));
        let llama = json!({"error": {"code": 400, "message": "input is too large to process. increase the physical batch size", "type": "invalid_request_error"}});
        assert!(fails_over_reply(400, &llama));
        // Psionic's refusal of a prompt over its token budget.
        let psionic = json!({"error": {"code": "not_admitted", "message": "the prompt is 20000 Clef tokens; this server admits 16384"}});
        assert!(fails_over_reply(413, &psionic));
        assert!(fails_over_reply(400, &psionic));
        // A malformed question fails everywhere: it stays.
        let malformed = json!({"error": {"code": "invalid_request", "message": "questions.x.type is not a question type"}});
        assert!(!fails_over_reply(400, &malformed));
        let ollama_malformed = json!({"error": "question \"x\": unknown type \"maybe\""});
        assert!(!fails_over_reply(400, &ollama_malformed));
    }

    #[test]
    fn error_bodies_read_in_one_shape() {
        let gateway = br#"{"message": "questions.refund.type: expected one of 'noul'", "error_type": "invalid_request"}"#;
        assert_eq!(
            normalize_error(400, gateway),
            json!({"error": {"code": "invalid_request", "message": "questions.refund.type: expected one of 'noul'"}})
        );
        let openrouter = br#"{"error": {"code": 402, "message": "Insufficient credits"}}"#;
        assert_eq!(
            normalize_error(402, openrouter)["error"]["code"],
            "payment_required"
        );
        assert_eq!(normalize_error(503, b"")["error"]["code"], "unavailable");
    }

    #[test]
    fn the_gateways_cost_becomes_usage_cost() {
        let mut body = json!({
            "model": "typesafe-ai/jev",
            "answers": {"refund": {"type": "noul", "noul": 0.98}},
            "usage": {"input_tokens": 275, "output_tokens": 20},
            "provider_metadata": {"gateway": {"cost": "0.00001155"}}
        });
        assert!(normalize_answer(&mut body));
        assert_eq!(body["usage"]["cost"], json!(0.000_011_55));
        assert!(
            !normalize_answer(&mut body),
            "a priced answer keeps its price"
        );
        let mut typesafe = json!({"model": "jev-1.13.0", "answers": {}});
        assert!(!normalize_answer(&mut typesafe));
    }

    #[test]
    fn a_door_never_shows_its_key() {
        let door = Door::fallback(&FALLBACKS[0], ApiKey::new("vck_secret_value"));
        assert!(!format!("{door:?}").contains("secret"));
        let failover = Failover::new(
            Door::new(
                TYPESAFE_DOOR,
                TYPESAFE_DOOR,
                Naming::Canonical,
                ApiKey::new("ts_secret"),
            ),
            vec![door],
        );
        assert!(!format!("{failover:?}").contains("secret"));
        assert_eq!(
            failover.service(),
            "doors https://api.typesafe.ai → https://ai-gateway.vercel.sh"
        );
    }
}
