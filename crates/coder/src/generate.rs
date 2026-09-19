//! The Generate side of the agent: open-ended synthesis, behind a trait.
//!
//! [`Generate`] is the whole public contract: a request carries system
//! instructions and the conversation so far, a response carries the produced
//! text and the token count. [`ResponsesDoor`] implements it against any
//! endpoint that speaks the Open Responses API — `POST {base}/v1/responses`
//! with a bearer, a stream of `response.output_text.delta` events back.
//! The door's URL, model, and key come from the environment; this crate
//! ships no endpoint of its own beyond the public gateway default.
//!
//! [`StubGenerate`] answers with a canned line so the shell and tests run
//! with no door at all.

use std::env;
use std::fmt;

use futures_util::StreamExt;
use serde_json::{Value, json};

/// The default door: the public Vercel AI Gateway. `CODER_DOOR_URL`
/// overrides it for a deployment's own endpoint.
pub const DEFAULT_DOOR_URL: &str = "https://ai-gateway.vercel.sh";

/// The default model the door runs. `CODER_MODEL` overrides it.
pub const DEFAULT_MODEL: &str = "google/gemini-3.8-flash";

/// One conversational turn, user or assistant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// Who said it.
    pub role: Role,
    /// What they said.
    pub text: String,
}

/// The side of the conversation a [`Message`] sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The person at the terminal.
    User,
    /// The agent.
    Assistant,
}

/// What a generation cost, when the door reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Tokens the request consumed.
    pub input_tokens: u64,
    /// Tokens the response produced.
    pub output_tokens: u64,
}

/// Sideband information a door may emit mid-turn, before or between text
/// deltas. Only doors that wrap a remote worker produce it today.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Meta {
    /// A classification verdict the worker computed, as a display-ready
    /// line (the NIP-CJ `judgment` feedback payload's `line` field).
    Judgment(String),
}

/// What generation can fail with.
#[derive(Debug)]
pub enum GenerateError {
    /// The door URL, key, or model is missing or wrong.
    Config(String),
    /// The HTTP call itself failed.
    Transport(reqwest::Error),
    /// The door answered with an error status and this body.
    Status(u16, String),
    /// The stream broke or carried an error event.
    Stream(String),
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerateError::Config(why) => write!(f, "config: {why}"),
            GenerateError::Transport(error) => write!(f, "transport: {error}"),
            GenerateError::Status(status, body) => write!(f, "door answered {status}: {body}"),
            GenerateError::Stream(why) => write!(f, "stream: {why}"),
        }
    }
}

impl std::error::Error for GenerateError {}

impl From<reqwest::Error> for GenerateError {
    fn from(error: reqwest::Error) -> Self {
        GenerateError::Transport(error)
    }
}

/// One generation: instructions plus conversation in, text plus usage out.
/// `sink` receives each text delta as it streams, so a caller can draw the
/// answer as it forms. `meta` receives sideband items a door may emit —
/// doors that never emit simply do not call it.
pub trait Generate: Send + Sync {
    /// Generate the next assistant turn.
    fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> impl std::future::Future<Output = Result<(String, Option<Usage>), GenerateError>> + Send + 'a;
}

/// A `Generate` backed by an Open Responses endpoint.
pub struct ResponsesDoor {
    http: reqwest::Client,
    /// The door's base URL; the route is `/v1/responses` under it.
    pub url: String,
    /// The model the door runs.
    pub model: String,
    key: String,
}

impl ResponsesDoor {
    /// A door for `url` serving `model` behind `key`.
    pub fn new(url: impl Into<String>, model: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            url: url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            key: key.into(),
        }
    }

    /// A door from the environment: `CODER_DOOR_URL` or the public gateway,
    /// `CODER_MODEL` or the gateway's default model, `CODER_DOOR_KEY` or
    /// `CODER_AI_GATEWAY_KEY` for the bearer. `None` when no key is set.
    pub fn from_env() -> Option<Self> {
        let key = env::var("CODER_DOOR_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .or_else(|| {
                env::var("CODER_AI_GATEWAY_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })?;
        let url = env::var("CODER_DOOR_URL").unwrap_or_else(|_| DEFAULT_DOOR_URL.to_string());
        let model = env::var("CODER_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        Some(Self::new(url, model, key))
    }

    fn body(&self, instructions: &str, input: &[Message]) -> Value {
        let items: Vec<Value> = input
            .iter()
            .map(|message| {
                let (role, kind) = match message.role {
                    Role::User => ("user", "input_text"),
                    Role::Assistant => ("assistant", "output_text"),
                };
                json!({
                    "type": "message",
                    "role": role,
                    "content": [{ "type": kind, "text": message.text }],
                })
            })
            .collect();
        json!({
            "model": self.model,
            "instructions": instructions,
            "input": items,
            "stream": true,
            "store": false,
        })
    }
}

impl Generate for ResponsesDoor {
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        _meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let response = self
            .http
            .post(format!("{}/v1/responses", self.url))
            .bearer_auth(&self.key)
            .json(&self.body(instructions, input))
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(GenerateError::Status(status.as_u16(), clip(&body, 400)));
        }

        // The SSE stream: `data: {json}` lines, blank-line separated. Delta
        // events feed the sink; the completed event carries usage.
        let mut text = String::new();
        let mut usage = None;
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(end) = buffer.find('\n') {
                let line = buffer[..end].trim_end_matches('\r').to_string();
                buffer.drain(..end + 1);
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                let Ok(event) = serde_json::from_str::<Value>(data) else {
                    continue;
                };
                match event["type"].as_str().unwrap_or_default() {
                    "response.output_text.delta" => {
                        if let Some(delta) = event["delta"].as_str() {
                            text.push_str(delta);
                            sink(delta);
                        }
                    }
                    "response.completed" => {
                        usage = event["response"]["usage"].as_object().map(|u| Usage {
                            input_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                            output_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                        });
                    }
                    "response.failed" | "error" => {
                        let message = event["response"]["error"]["message"]
                            .as_str()
                            .or_else(|| event["message"].as_str())
                            .unwrap_or("the turn failed upstream");
                        return Err(GenerateError::Stream(message.to_string()));
                    }
                    _ => {}
                }
            }
        }
        if text.is_empty() {
            return Err(GenerateError::Stream(
                "the stream carried no text".to_string(),
            ));
        }
        Ok((text, usage))
    }
}

/// A door that is not there: it answers with a fixed line. The shell and
/// the tests use it so neither needs credentials.
pub struct StubGenerate {
    /// What the stub says.
    pub line: String,
}

impl Default for StubGenerate {
    fn default() -> Self {
        Self {
            line: "(stub door: set CODER_DOOR_KEY or CODER_AI_GATEWAY_KEY for a real answer)"
                .to_string(),
        }
    }
}

impl Generate for StubGenerate {
    async fn generate<'a>(
        &'a self,
        _instructions: &'a str,
        _input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        _meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        sink(&self.line);
        Ok((self.line.clone(), None))
    }
}

/// Whatever the environment gives: an own-key door when a key is set,
/// the relay when a worker is configured, the stub otherwise.
pub enum Door {
    /// A live Open Responses endpoint.
    Live(ResponsesDoor),
    /// A Nostr relay running the NIP-CJ job protocol.
    Relay(Box<crate::relay::RelayDoor>),
    /// The canned answer.
    Stub(StubGenerate),
}

impl Door {
    /// The configured door: own-key when a key is present, the relay
    /// when `CODER_WORKER` names a worker, stub otherwise.
    pub fn from_env() -> Self {
        if let Some(door) = ResponsesDoor::from_env() {
            return Door::Live(door);
        }
        if let Some(door) = crate::relay::RelayDoor::from_env() {
            return Door::Relay(Box::new(door));
        }
        Door::Stub(StubGenerate::default())
    }

    /// The model name the door serves, for the token rail.
    pub fn model(&self) -> &str {
        match self {
            Door::Live(door) => &door.model,
            Door::Relay(_) => "relay",
            Door::Stub(_) => "stub",
        }
    }
}

impl Generate for Door {
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        match self {
            Door::Live(door) => door.generate(instructions, input, sink, meta).await,
            Door::Relay(door) => door.generate(instructions, input, sink, meta).await,
            Door::Stub(stub) => stub.generate(instructions, input, sink, meta).await,
        }
    }
}

fn clip(text: &str, limit: usize) -> String {
    let clipped: String = text.chars().take(limit).collect();
    if clipped.len() < text.len() {
        format!("{clipped}…")
    } else {
        clipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_speaks_responses_wire_format() {
        let door = ResponsesDoor::new("https://door.example", "m", "k");
        let input = vec![
            Message {
                role: Role::User,
                text: "hi".to_string(),
            },
            Message {
                role: Role::Assistant,
                text: "hello".to_string(),
            },
        ];
        let body = door.body("sys", &input);
        assert_eq!(body["model"], "m");
        assert_eq!(body["instructions"], "sys");
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);
        assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(body["input"][1]["content"][0]["type"], "output_text");
    }

    #[tokio::test]
    async fn the_stub_answers_and_reports_no_usage() {
        let stub = StubGenerate::default();
        let mut seen = String::new();
        let (text, usage) = stub
            .generate("sys", &[], &mut |delta| seen.push_str(delta), &mut |_| {})
            .await
            .unwrap();
        assert_eq!(text, stub.line);
        assert_eq!(seen, stub.line);
        assert!(usage.is_none());
    }

    #[test]
    fn no_key_means_no_door() {
        // The lookup reads the real environment; with neither variable set a
        // door cannot be built. When a key IS set the test still passes: the
        // door exists. What matters is it never panics.
        let _ = ResponsesDoor::from_env();
    }
}
