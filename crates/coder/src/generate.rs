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
    /// The model that produced the answer, as the far end names it (the
    /// NIP-CJ result payload's `model` field).
    ///
    /// A door that forwards a turn somewhere else does not know which
    /// model will take it, so the session header cannot say. The answer
    /// can, and this is how it says so: the door emits the name as the
    /// result lands and the trace puts it on that step.
    Model(String),
}

/// What generation can fail with.
///
/// The relay door splits its failures into three, because they are three
/// different states and a harness should not have to read prose to tell
/// them apart: the relay would not take the job, the relay took it and no
/// worker answered, or a worker answered by declining. `gym::eval::classify`
/// draws the same line — a typed refusal is an answer, a failure with no
/// code is the harness — and [`GenerateError::cause`] is where it is drawn
/// here.
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
    /// The relay would not take the job: the socket never opened, the
    /// NIP-42 challenge went unanswered, or the relay rejected the
    /// request event.
    Relay(String),
    /// The relay took the job and no answer came back.
    ///
    /// `heard` is what separates a worker that is not there from one that
    /// is slow, as far as an ephemeral protocol lets a client separate
    /// them: `false` means nothing at all came back from the worker's key,
    /// and `true` means something did and then the answer never finished.
    Silent {
        /// Whether anything came back from the worker before the wait ran
        /// out.
        heard: bool,
        /// What the wait was, in a sentence.
        reason: String,
    },
    /// A worker answered with a typed refusal: NIP-CJ `status: error`
    /// feedback carrying a machine-readable code. The job reached a
    /// worker, and the worker said no.
    Refused {
        /// The NIP-CJ refusal code, such as `quota_exhausted`.
        code: String,
        /// The worker's display text for it.
        message: String,
    },
}

impl GenerateError {
    /// The word a harness files this failure under.
    ///
    /// `worker_declined` is an answer in `gym::eval::classify`'s sense: the
    /// job reached a worker and the worker refused it with a code. Every
    /// other word is the harness.
    ///
    /// `worker_absent` and `worker_stalled` are both silence, and they are
    /// two words because they are two problems: nothing was listening, or
    /// something was listening and did not finish. The first is a
    /// judgment call — an ephemeral protocol gives a client no way to
    /// prove a worker is missing — and it is the judgment the short wait
    /// in [`crate::relay`] makes.
    #[must_use]
    pub fn cause(&self) -> &'static str {
        match self {
            GenerateError::Config(_) => "config",
            GenerateError::Transport(_) | GenerateError::Status(..) => "door",
            GenerateError::Stream(_) => "stream",
            GenerateError::Relay(_) => "relay_unreachable",
            GenerateError::Silent { heard: false, .. } => "worker_absent",
            GenerateError::Silent { heard: true, .. } => "worker_stalled",
            GenerateError::Refused { .. } => "worker_declined",
        }
    }

    /// The typed refusal code, when the failure carries one.
    ///
    /// Read as a field rather than searched for in the message, so a door
    /// whose prose mentions a code is not recorded as having refused with
    /// it.
    #[must_use]
    pub fn refusal(&self) -> Option<&str> {
        match self {
            GenerateError::Refused { code, .. } => Some(code),
            _ => None,
        }
    }
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerateError::Config(why) => write!(f, "config: {why}"),
            GenerateError::Transport(error) => write!(f, "transport: {error}"),
            GenerateError::Status(status, body) => write!(f, "door answered {status}: {body}"),
            GenerateError::Stream(why) => write!(f, "stream: {why}"),
            GenerateError::Relay(why) => write!(f, "relay: {why}"),
            GenerateError::Silent {
                heard: false,
                reason,
            } => {
                write!(f, "no worker answered: {reason}")
            }
            GenerateError::Silent {
                heard: true,
                reason,
            } => {
                write!(f, "the worker stopped mid-answer: {reason}")
            }
            GenerateError::Refused { code, message } => {
                write!(f, "the worker declined ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for GenerateError {}

impl From<reqwest::Error> for GenerateError {
    fn from(error: reqwest::Error) -> Self {
        GenerateError::Transport(error)
    }
}

/// How many times an empty stream is attempted before its error surfaces:
/// transient upstream failures — a malformed function call, a dropped
/// connection — retry invisibly; a dead door still reports dead.
const EMPTY_STREAM_ATTEMPTS: usize = 6;

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
            // No tools exist on this door; Gemini will still try to emit a
            // function call when the instructions carry a JSON example, and
            // the call dies as MALFORMED_FUNCTION_CALL. Forbid it outright —
            // an empty tools array plus tool_choice none, since gateways
            // differ on which one they translate.
            "tools": [],
            "tool_choice": "none",
        })
    }
}

impl ResponsesDoor {
    /// One streaming attempt: the request plus the SSE read. On failure the
    /// error carries whatever text streamed before it died, so the caller
    /// can tell whether anything user-visible arrived.
    async fn once(
        &self,
        instructions: &str,
        input: &[Message],
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(String, Option<Usage>), (String, GenerateError)> {
        let response = self
            .http
            .post(format!("{}/v1/responses", self.url))
            .bearer_auth(&self.key)
            .json(&self.body(instructions, input))
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => return Err((String::new(), GenerateError::Transport(error))),
        };
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err((
                String::new(),
                GenerateError::Status(status.as_u16(), clip(&body, 400)),
            ));
        }

        // The SSE stream: `data: {json}` lines, blank-line separated. Delta
        // events feed the sink; the completed event carries usage.
        let mut text = String::new();
        let mut usage = None;
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => return Err((text, GenerateError::Transport(error))),
            };
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
                        return Err((text, GenerateError::Stream(message.to_string())));
                    }
                    _ => {}
                }
            }
        }
        if text.is_empty() {
            return Err((
                text,
                GenerateError::Stream("the stream carried no text".to_string()),
            ));
        }
        Ok((text, usage))
    }
}

/// Whether streamed text is user-visible: a reply that opens as a JSON
/// object or a code fence is a shell plan's wire format, which the
/// terminal hides while it streams — so a stream that died mid-plan
/// showed nothing and earns another attempt like an empty one did.
fn planish(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with('{') || text.starts_with("```")
}

impl Generate for ResponsesDoor {
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        _meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        // A stream that fails before showing anything — empty, or holding
        // only a hidden plan — is safe to redo: the user saw nothing and
        // the request is idempotent. Keep retrying with a growing wait;
        // upstream flakes like a model-side malformed function call are
        // transient, and the retry is invisible. A door that keeps failing
        // after the last attempt still surfaces its error — hidden
        // retries, honest failures.
        for attempt in 0..EMPTY_STREAM_ATTEMPTS {
            match self.once(instructions, input, sink).await {
                Err((partial, error)) => {
                    let visible = !partial.is_empty() && !planish(&partial);
                    if visible
                        || !matches!(
                            error,
                            GenerateError::Stream(_) | GenerateError::Transport(_)
                        )
                        || attempt + 1 == EMPTY_STREAM_ATTEMPTS
                    {
                        return Err(error);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(
                        300 * (attempt as u64 + 1),
                    ))
                    .await;
                    continue;
                }
                Ok(done) => return Ok(done),
            }
        }
        unreachable!()
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

/// What a session header says for a door that cannot name its model until
/// something answers. The answer steps carry the model that did.
pub const UNKNOWN_MODEL: &str = "unknown";

/// The two variables that ask for an own-key door, in the order
/// [`ResponsesDoor::from_env`] reads them.
const KEY_VARS: [&str; 2] = ["CODER_DOOR_KEY", "CODER_AI_GATEWAY_KEY"];

/// The variable that asks for the relay door.
const WORKER_VAR: &str = "CODER_WORKER";

impl Door {
    /// The configured door: own-key when a key is present, the relay when
    /// `CODER_WORKER` names a worker, stub when neither is set.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the environment asks for two doors at once,
    /// and when the door it asks for cannot be built. Both used to be
    /// silent: a key and a worker together took the key, and an unparseable
    /// worker fell through to the stub. Either precedence is defensible and
    /// the silence is not — someone measuring the relay with a key still
    /// set measures the other transport and gets a plausible number.
    pub fn from_env() -> Result<Self, String> {
        let key = KEY_VARS
            .into_iter()
            .find(|name| env::var(name).is_ok_and(|value| !value.is_empty()));
        let worker = env::var(WORKER_VAR).is_ok_and(|value| !value.is_empty());
        match asked_for(key, worker)? {
            Asked::Own => ResponsesDoor::from_env()
                .map(Door::Live)
                .ok_or_else(|| "the door key is set and empty".to_string()),
            Asked::Relay => {
                crate::relay::RelayDoor::from_env().map(|door| Door::Relay(Box::new(door)))
            }
            Asked::Stub => Ok(Door::Stub(StubGenerate::default())),
        }
    }

    /// The model name the door serves, which a trace's session header
    /// records.
    ///
    /// A relay door answers [`UNKNOWN_MODEL`]: the worker picks the model
    /// and names it in the NIP-CJ result, so the session header cannot
    /// know it and each answer step carries what answered. Recording the
    /// word `relay` there instead claimed a model by that name.
    pub fn model(&self) -> &str {
        match self {
            Door::Live(door) => &door.model,
            Door::Relay(_) => UNKNOWN_MODEL,
            Door::Stub(_) => "stub",
        }
    }

    /// What the composer's location rail shows: the model when the door
    /// knows one before answering, the door's own name when it does not.
    pub fn label(&self) -> &str {
        match self {
            Door::Relay(_) => self.name(),
            _ => self.model(),
        }
    }

    /// Which kind of door this is, which a trace records beside the model:
    /// the same model answered through a relay and through an own-key
    /// endpoint is two different paths.
    pub fn name(&self) -> &'static str {
        match self {
            Door::Live(_) => "live",
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

/// Which door the environment asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    /// An own-key Open Responses endpoint.
    Own,
    /// The relay, and whichever worker answers on it.
    Relay,
    /// Neither, so the canned answer.
    Stub,
}

/// The door a key variable and a worker variable ask for between them,
/// split out from [`Door::from_env`] so it is decided without reading the
/// process environment. `key` names the key variable that is set, when one
/// is.
///
/// # Errors
///
/// Returns a sentence when both are set. Refusing is better than choosing,
/// because the thing the person needs to see is that they asked for two
/// things, and a door that quietly picks one answers the question they did
/// not ask.
fn asked_for(key: Option<&str>, worker: bool) -> Result<Asked, String> {
    match (key, worker) {
        (Some(key), true) => Err(format!(
            "the environment names two doors: {key} asks for an own-key door \
             and {WORKER_VAR} asks for the relay. Unset one of them."
        )),
        (Some(_), false) => Ok(Asked::Own),
        (None, true) => Ok(Asked::Relay),
        (None, false) => Ok(Asked::Stub),
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
        assert_eq!(body["tool_choice"], "none");
        assert_eq!(body["tools"], json!([]));
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

    /// Four relay failures, four causes. A typed refusal carries its code
    /// as a field; the three that never got an answer carry none.
    #[test]
    fn the_relay_failures_file_under_four_causes() {
        let unreachable = GenerateError::Relay("connect: refused".to_string());
        let absent = GenerateError::Silent {
            heard: false,
            reason: "nothing came back in 30 seconds".to_string(),
        };
        let stalled = GenerateError::Silent {
            heard: true,
            reason: "no result in 180 seconds".to_string(),
        };
        let declined = GenerateError::Refused {
            code: "quota_exhausted".to_string(),
            message: "free allowance used".to_string(),
        };

        assert_eq!(unreachable.cause(), "relay_unreachable");
        assert_eq!(declined.cause(), "worker_declined");

        // Silence is two states, and a harness reads which on the field
        // rather than on the sentence.
        assert_eq!(absent.cause(), "worker_absent");
        assert_eq!(stalled.cause(), "worker_stalled");
        assert_eq!(
            absent.to_string(),
            "no worker answered: nothing came back in 30 seconds"
        );
        assert_eq!(
            stalled.to_string(),
            "the worker stopped mid-answer: no result in 180 seconds"
        );

        assert_eq!(unreachable.refusal(), None);
        assert_eq!(absent.refusal(), None);
        assert_eq!(stalled.refusal(), None);
        assert_eq!(declined.refusal(), Some("quota_exhausted"));

        assert_eq!(
            declined.to_string(),
            "the worker declined (quota_exhausted): free allowance used"
        );
        // A door failure is not a relay failure, whatever the prose says.
        assert_eq!(
            GenerateError::Stream("the relay went away".to_string()).cause(),
            "stream"
        );
    }

    /// An environment that asks for two doors is refused, and the refusal
    /// names both variables. Silently preferring one is how a relay
    /// measurement comes to be a measurement of the other transport.
    #[test]
    fn two_configured_doors_are_a_refusal_rather_than_a_choice() {
        let both = asked_for(Some("CODER_DOOR_KEY"), true).expect_err("two doors");
        assert!(both.contains("CODER_DOOR_KEY"), "{both}");
        assert!(both.contains("CODER_WORKER"), "{both}");

        assert_eq!(asked_for(Some("CODER_DOOR_KEY"), false), Ok(Asked::Own));
        assert_eq!(
            asked_for(Some("CODER_AI_GATEWAY_KEY"), false),
            Ok(Asked::Own)
        );
        assert_eq!(asked_for(None, true), Ok(Asked::Relay));
        assert_eq!(asked_for(None, false), Ok(Asked::Stub));
    }

    /// A relay door cannot name its model before a worker answers, and
    /// says so rather than naming the transport as if it were a model.
    #[test]
    fn a_door_that_cannot_name_its_model_says_unknown() {
        let live = Door::Live(ResponsesDoor::new("https://door.example", "a/model", "k"));
        assert_eq!(live.model(), "a/model");
        assert_eq!(live.label(), "a/model");
        assert_eq!(Door::Stub(StubGenerate::default()).model(), "stub");
        assert_eq!(UNKNOWN_MODEL, "unknown");
    }

    #[test]
    fn no_key_means_no_door() {
        // The lookup reads the real environment; with neither variable set a
        // door cannot be built. When a key IS set the test still passes: the
        // door exists. What matters is it never panics.
        let _ = ResponsesDoor::from_env();
    }
}
