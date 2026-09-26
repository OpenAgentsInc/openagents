//! Generation through the OpenAgents door: `POST {base}/v1/responses` on
//! an openagents.com bearer, the Open Responses API that Coder One uses.
//!
//! The door serves other vendors' models under their gateway slugs, such as
//! `google/gemini-3.8-flash`, with the same native function tools as the
//! Codex endpoint. So the request is [`microluna::codex::body`] and the
//! stream is read by [`microluna::codex::Events`]; only the endpoint, the
//! bearer, and the cost change. The door reports each call's cost as
//! `usage.cost_microusd`, so a step's cost is that figure
//! ([`Basis::Billed`]), not a list price computed here. A call whose cost
//! the door didn't report, or an attempt that failed after it was sent,
//! leaves the cost unknown.
//!
//! The bearer comes from `OPENAGENTS_API_KEY`, or else from
//! `~/.openagents/bearer`. It never reaches a log line, an error, or a
//! `Debug` string, and commands run in the task's container, which never
//! sees it.

use std::fmt;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use microluna::codex::{Events, REQUEST_TIMEOUT, body};
use microluna::transport::{Reply, Request, Transport, TransportError};
use serde_json::{Value, json};

use crate::models::{Basis, Generate, Generated, NextAction, next_action_tool};

/// The door the loop uses when `OPENAGENTS_DOOR_URL` names none.
pub const BASE_URL: &str = "https://openagents.com";

/// The variable that names another door.
pub const URL_VAR: &str = "OPENAGENTS_DOOR_URL";

/// The variable that holds the bearer.
pub const KEY_VAR: &str = "OPENAGENTS_API_KEY";

/// Attempts after the first, for a refused or broken request.
pub const RETRIES: u32 = 3;

/// The door's transport: one Responses request per call.
pub struct DoorTransport {
    http: reqwest::Client,
    url: String,
    bearer: String,
    /// The cost the door reported for the latest completed reply, in
    /// millionths of a dollar.
    last_cost: Mutex<Option<u64>>,
}

impl fmt::Debug for DoorTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DoorTransport")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl DoorTransport {
    /// A transport to the door at `base` on `bearer`.
    ///
    /// # Errors
    ///
    /// When the bearer is empty or the HTTP client can't be built.
    pub fn new(base: &str, bearer: &str) -> Result<DoorTransport, String> {
        let bearer = bearer.trim();
        if bearer.is_empty() {
            return Err("the OpenAgents bearer is empty".to_string());
        }
        let http = reqwest::Client::builder()
            .user_agent(concat!("microcoder/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(DoorTransport {
            http,
            url: format!("{}/v1/responses", base.trim_end_matches('/')),
            bearer: bearer.to_string(),
            last_cost: Mutex::new(None),
        })
    }

    /// A transport to `OPENAGENTS_DOOR_URL` (default [`BASE_URL`]) on the
    /// bearer in `OPENAGENTS_API_KEY`, or else in `~/.openagents/bearer`.
    ///
    /// # Errors
    ///
    /// When neither holds a bearer.
    pub fn from_env() -> Result<DoorTransport, String> {
        let base = std::env::var(URL_VAR)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| BASE_URL.to_string());
        let bearer = std::env::var(KEY_VAR)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                let home = std::env::var_os("HOME")?;
                std::fs::read_to_string(std::path::PathBuf::from(home).join(".openagents/bearer"))
                    .ok()
            })
            .ok_or(format!(
                "no OpenAgents bearer: set {KEY_VAR} or put it in ~/.openagents/bearer"
            ))?;
        DoorTransport::new(&base, &bearer)
    }

    /// The cost the door reported for the latest completed reply.
    #[must_use]
    pub fn last_cost(&self) -> Option<u64> {
        self.last_cost.lock().ok().and_then(|cost| *cost)
    }
}

/// The request body: the Codex body with the one tool required, and no
/// encrypted reasoning asked for.
#[must_use]
pub fn request_body(request: &Request) -> Value {
    let mut body = body(request);
    body["tool_choice"] = json!("required");
    if let Some(fields) = body.as_object_mut() {
        fields.remove("include");
    }
    body
}

/// The `cost_microusd` a stream's `response.completed` event reports.
#[must_use]
pub fn reported_cost(stream: &[u8]) -> Option<u64> {
    let text = String::from_utf8_lossy(stream);
    text.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|data| serde_json::from_str::<Value>(data.trim_start()).ok())
        .filter(|event| event["type"] == "response.completed")
        .find_map(|event| event["response"]["usage"]["cost_microusd"].as_u64())
}

impl Transport for DoorTransport {
    async fn respond(&self, request: &Request) -> Result<Reply, TransportError> {
        if let Ok(mut cost) = self.last_cost.lock() {
            *cost = None;
        }
        let mut response = self
            .http
            .post(&self.url)
            .timeout(REQUEST_TIMEOUT)
            .bearer_auth(&self.bearer)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&request_body(request))
            .send()
            .await
            .map_err(|error| TransportError::Stream(error.without_url().to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TransportError::Http {
                status: status.as_u16(),
                body: crate::state::cut(&text, 400, 0),
            });
        }
        let mut events = Events::default();
        let mut stream = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| TransportError::Stream(error.without_url().to_string()))?
        {
            stream.extend_from_slice(&chunk);
            events.push(&chunk)?;
        }
        let reply = events.finish()?;
        if let Ok(mut cost) = self.last_cost.lock() {
            *cost = reported_cost(&stream);
        }
        Ok(reply)
    }
}

/// Generation through the door: one request per step, and the action is
/// the arguments of a call to the one declared tool, `next_action`.
pub struct DoorGenerator<T: Transport = DoorTransport> {
    pub transport: T,
    /// The gateway slug, such as `google/gemini-3.8-flash`.
    pub model: String,
    /// `low`, `medium`, or `high`, or `None` for the model's default.
    pub effort: Option<String>,
    /// The prompt-cache key; steps of one run share it.
    pub cache_key: String,
    /// Reads the cost the door reported for the latest completed reply.
    pub cost: fn(&T) -> Option<u64>,
}

impl DoorGenerator<DoorTransport> {
    /// A generator on `transport` running `model`.
    #[must_use]
    pub fn new(
        transport: DoorTransport,
        model: &str,
        effort: Option<String>,
        cache_key: &str,
    ) -> Self {
        DoorGenerator {
            transport,
            model: model.to_string(),
            effort,
            cache_key: cache_key.to_string(),
            cost: DoorTransport::last_cost,
        }
    }
}

/// Whether a failed attempt may have been billed: it failed after the
/// door accepted it.
fn may_have_spent(error: &TransportError) -> bool {
    matches!(
        error,
        TransportError::Stream(_) | TransportError::Failed(_) | TransportError::Incomplete(_)
    )
}

impl<T: Transport> Generate for DoorGenerator<T> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let started = Instant::now();
        let request = Request {
            model: self.model.clone(),
            instructions: format!("{system} Reply by calling next_action exactly once."),
            input: vec![json!({
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": prompt }],
            })],
            tools: vec![next_action_tool()],
            effort: self.effort.clone(),
            cache_key: self.cache_key.clone(),
            parallel_tools: false,
        };
        let mut spent: Vec<String> = Vec::new();
        let mut attempt = 0u32;
        let reply = loop {
            match self.transport.respond(&request).await {
                Ok(reply) => break Ok(reply),
                Err(error) => {
                    if may_have_spent(&error) {
                        spent.push(format!("attempt {}: {error}", attempt + 1));
                    }
                    if error.transient() && attempt < RETRIES {
                        attempt += 1;
                        tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                        continue;
                    }
                    break Err(error.to_string());
                }
            }
        };
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let (action, model, input, output, billed) = match reply {
            Ok(reply) => {
                let action = reply
                    .calls()
                    .into_iter()
                    .find(|c| c.name == "next_action")
                    .ok_or_else(|| {
                        format!(
                            "the reply called no next_action tool; it said: {}",
                            reply.text().chars().take(300).collect::<String>()
                        )
                    })
                    .and_then(|call| {
                        serde_json::from_str::<NextAction>(&call.arguments)
                            .map_err(|e| format!("next_action's arguments didn't parse: {e}"))
                    });
                let model = if reply.model.is_empty() {
                    self.model.clone()
                } else {
                    reply.model.clone()
                };
                let billed = (self.cost)(&self.transport);
                let reported = billed.map(|micro| micro as f64 / 1_000_000.0);
                (
                    action,
                    model,
                    reply.usage.input,
                    reply.usage.output,
                    Some(reported),
                )
            }
            Err(error) => (Err(error), self.model.clone(), 0, 0, None),
        };
        // `billed` is `Some(Some(usd))` for a reported cost, `Some(None)`
        // for a reply without one, and `None` when no reply came back.
        let known_usd = billed.flatten().unwrap_or(0.0);
        let cost_unknown = if !spent.is_empty() {
            Some(format!(
                "{} failed after the request was sent and may have been billed ({})",
                if spent.len() == 1 {
                    "an attempt".to_string()
                } else {
                    format!("{} attempts", spent.len())
                },
                spent.join("; ")
            ))
        } else if billed.is_some_and(|b| b.is_none()) {
            Some("the door reported no cost for the reply".to_string())
        } else {
            None
        };
        // A request refused with an error status cost nothing.
        let usd = match (&cost_unknown, billed) {
            (Some(_), _) => None,
            (None, Some(reported)) => reported,
            (None, None) => Some(0.0),
        };
        Generated {
            action,
            model,
            prompt_tokens: input,
            completion_tokens: output,
            usd,
            known_usd,
            cost_unknown,
            cost_basis: Basis::Billed,
            milliseconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use microluna::fake::FakeTransport;
    use microluna::transport::TokenUsage;

    fn reply(arguments: &str) -> Reply {
        Reply {
            id: None,
            model: "google/gemini-3.8-flash".to_string(),
            items: vec![json!({
                "type": "function_call", "call_id": "c1", "name": "next_action",
                "arguments": arguments,
            })],
            usage: TokenUsage {
                input: 53,
                output: 195,
                ..Default::default()
            },
        }
    }

    fn generator(
        replies: Vec<Reply>,
        cost: fn(&FakeTransport) -> Option<u64>,
    ) -> DoorGenerator<FakeTransport> {
        DoorGenerator {
            transport: FakeTransport::new(replies),
            model: "google/gemini-3.8-flash".to_string(),
            effort: Some("medium".to_string()),
            cache_key: "run".to_string(),
            cost,
        }
    }

    const ACTION: &str = r#"{"rationale":"look","commands":["ls"],"view":[],"expand":[],"freeze_tests":false,"finished":false}"#;

    #[tokio::test]
    async fn a_reported_cost_is_the_steps_billed_cost() {
        let g = generator(vec![reply(ACTION)], |_| Some(771));
        let out = g.generate("system", "prompt").await;
        assert_eq!(out.action.unwrap().commands, ["ls"]);
        assert_eq!((out.prompt_tokens, out.completion_tokens), (53, 195));
        assert_eq!(out.usd, Some(0.000_771));
        assert_eq!(out.cost_basis, Basis::Billed);
        assert!(out.cost_unknown.is_none());
        let sent = g.transport.requests();
        assert_eq!(sent[0].model, "google/gemini-3.8-flash");
        assert_eq!(request_body(&sent[0])["tool_choice"], "required");
        assert!(request_body(&sent[0]).get("include").is_none());
    }

    #[tokio::test]
    async fn a_reply_without_a_reported_cost_leaves_the_cost_unknown() {
        let out = generator(vec![reply(ACTION)], |_| None)
            .generate("s", "p")
            .await;
        assert!(out.action.is_ok());
        assert_eq!(out.usd, None);
        assert!(out.cost_unknown.unwrap().contains("reported no cost"));
    }

    #[test]
    fn the_completed_event_names_the_cost() {
        let stream = b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n\
event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":53,\"output_tokens\":195,\"cost_microusd\":771}}}\n\n";
        assert_eq!(reported_cost(stream), Some(771));
        assert_eq!(
            reported_cost(b"data: {\"type\":\"response.completed\",\"response\":{}}\n\n"),
            None
        );
    }
}
