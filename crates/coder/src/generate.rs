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
//!
//! Every wait a door may keep is bounded, and the bounds are three rather
//! than one: [`CONNECT_TIMEOUT`] for the connection, [`Patience::first_word`]
//! for the response headers, and [`Patience::quiet`] for the silence
//! between two events of a stream. Read [`Patience`] for why they are
//! separate.
//!
//! This module is the only place in the crate a model name belongs. The
//! names are on [`Lane`], and a caller that needs one names a lane or
//! reads a variable rather than writing an identifier of its own.

use std::env;
use std::fmt;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde_json::{Value, json};

/// The default door: the public Vercel AI Gateway. `CODER_DOOR_URL`
/// overrides it for a deployment's own endpoint.
pub const DEFAULT_DOOR_URL: &str = "https://ai-gateway.vercel.sh";

/// A lane: a model the gateway serves, under a short name.
///
/// The gateway answers one Open Responses shape for every model in its
/// catalog, so a second model is a configuration change rather than a
/// second client. A lane is that configuration, and this enum is where
/// every model name in the crate lives — the worker, the runtime, and a
/// bench name a lane and ask for the model, so a name never spreads to a
/// fourth place.
///
/// A model name is also door identity. `docs/gym/regression.md` refuses a
/// comparison when door identity moves, so a run has to be able to say
/// which lane answered it; [`Door::model`] is what a trace records, and it
/// reports the model rather than the lane for exactly that reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Google's Gemini Flash, the lane a door runs when nothing names one.
    Gemini,
    /// Z.ai's GLM Flash.
    Glm,
}

impl Lane {
    /// Every lane, in the order the crate documents them.
    pub const ALL: [Lane; 2] = [Lane::Gemini, Lane::Glm];

    /// The lane's short name, which configuration may use in place of the
    /// model id.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Lane::Gemini => "gemini",
            Lane::Glm => "glm",
        }
    }

    /// The gateway model the lane runs.
    #[must_use]
    pub const fn model(self) -> &'static str {
        match self {
            Lane::Gemini => "google/gemini-3.8-flash",
            Lane::Glm => "zai/glm-5.3-flash",
        }
    }

    /// The lane `asked` names, by short name or by model id. `None` when
    /// it names neither.
    #[must_use]
    pub fn read(asked: &str) -> Option<Lane> {
        let asked = asked.trim();
        Lane::ALL
            .into_iter()
            .find(|lane| lane.name() == asked || lane.model() == asked)
    }
}

/// The lane a door runs when nothing names one.
pub const DEFAULT_LANE: Lane = Lane::Gemini;

/// The model the default lane runs. `CODER_MODEL` overrides it, by lane
/// name or by gateway model id.
pub const DEFAULT_MODEL: &str = DEFAULT_LANE.model();

/// The variable that names the model or lane the agent's door runs.
pub const MODEL_VAR: &str = "CODER_MODEL";

/// The variable that names the model or lane `coder-worker` answers
/// through.
///
/// The worker reads this rather than [`MODEL_VAR`] because the model a
/// service pays for is not automatically the model someone would pick at
/// their own terminal, and one constant cannot be both.
pub const WORKER_MODEL_VAR: &str = "CODER_WORKER_MODEL";

/// The model `asked` names: the lane's model when it names a lane, and
/// `asked` itself otherwise.
///
/// A value that names no lane is taken as a gateway model id, so the
/// gateway's whole catalog stays reachable without a lane of its own.
#[must_use]
pub fn model_named(asked: &str) -> &str {
    match Lane::read(asked) {
        Some(lane) => lane.model(),
        None => asked.trim(),
    }
}

/// The model `variable` asks for, with a lane name resolved to the model
/// it runs. `None` when the variable is unset or holds only blanks.
#[must_use]
pub fn model_from_env(variable: &str) -> Option<String> {
    let asked = env::var(variable).ok()?;
    let model = model_named(&asked);
    (!model.is_empty()).then(|| model.to_string())
}

/// How long a door has to accept a connection.
///
/// Separate from [`Patience::first_word`] because a refused or
/// unroutable endpoint is a different failure from a door that took the
/// request and thought about it, and the second should not have to wait
/// out the first.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long each wait on a streaming door lasts, and how the waits
/// between attempts grow.
///
/// The bounds exist because a door that accepts a request and then sends
/// nothing held the turn open with no limit at all: nothing bounded the
/// wait for the first token, nothing bounded the silence between tokens,
/// and nothing asked again. An unattended fan-out over such a door has no
/// upper bound on anything.
///
/// Reimplemented from the `~/work/coder` service's `Patience`, whose
/// values are these and whose record of the failure is an operator
/// waiting three and five minutes between a tool result and the next
/// token with nothing written down about where the time went.
///
/// The three are separate because they fail differently:
///
/// - [`Patience::first_word`] ends in a request that is sent again.
/// - [`Patience::quiet`] ends the turn, because a caller has already seen
///   part of the answer and a second attempt would repeat it.
/// - [`Patience::retry_wait`] is neither; it is the pause between two
///   attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patience {
    /// The wait for the door's response headers. A door that has not
    /// answered by then is treated like one that dropped the connection,
    /// and the request is sent again.
    pub first_word: Duration,
    /// The longest silence between two events of a stream.
    ///
    /// Measured between events rather than between bytes, because a
    /// stream's silence is not a byte-level property: a door that keeps a
    /// connection warm with blank lines is quiet, and a chunk that carries
    /// half an event is not an event. A door that goes quiet for this long
    /// has lost the turn, and the turn fails with a line saying how long
    /// it waited and how much had arrived.
    pub quiet: Duration,
    /// The wait before the second attempt; the third waits twice as long.
    pub retry_wait: Duration,
}

impl Default for Patience {
    fn default() -> Self {
        Patience {
            first_word: Duration::from_secs(30),
            quiet: Duration::from_secs(120),
            retry_wait: Duration::from_secs(1),
        }
    }
}

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
    /// The door took the request and went quiet.
    ///
    /// `heard` draws the same line the relay door draws with
    /// [`GenerateError::Silent`], because the two doors should fail in one
    /// vocabulary rather than two: `false` means the door never sent
    /// response headers, and `true` means it sent them and then stopped,
    /// whether or not any of the answer had arrived. The first is asked
    /// again and the second is not.
    Quiet {
        /// Whether anything came back before the wait ran out.
        heard: bool,
        /// What the wait was, in a sentence: how long it was, and how much
        /// had arrived. "It hung" and "it sent 400 characters and stopped"
        /// are different problems, and a turn that dies here should say
        /// which one it met.
        reason: String,
    },
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
    ///
    /// `door_absent` and `door_stalled` are the streaming door's two words
    /// for the same pair of problems, and they are named for the door
    /// rather than for a worker because a direct door has no worker behind
    /// it. A harness reads either pair as a field.
    #[must_use]
    pub fn cause(&self) -> &'static str {
        match self {
            GenerateError::Config(_) => "config",
            GenerateError::Transport(_) | GenerateError::Status(..) => "door",
            GenerateError::Stream(_) => "stream",
            GenerateError::Quiet { heard: false, .. } => "door_absent",
            GenerateError::Quiet { heard: true, .. } => "door_stalled",
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
            GenerateError::Quiet {
                heard: false,
                reason,
            } => {
                write!(f, "the door did not answer: {reason}")
            }
            GenerateError::Quiet {
                heard: true,
                reason,
            } => {
                write!(f, "the door went quiet mid-answer: {reason}")
            }
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

/// How many times a request goes to a door that never sends response
/// headers.
///
/// A door that has not answered within [`Patience::first_word`] is
/// treated like one that dropped the connection, so the request is sent
/// again. Three attempts bounds the whole wait at roughly three times
/// `first_word` plus the two pauses between them, which is a number an
/// unattended run can be reasoned about with.
const HEADER_ATTEMPTS: u32 = 3;

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

/// An HTTP client with the connect bound every door here shares.
///
/// The read bounds are not set on the client, because a client-wide read
/// timeout bounds the whole request and a streaming turn is meant to be
/// long. What has to be bounded is the silence inside it, and that is
/// measured per stream against [`Patience::quiet`].
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// A `Generate` backed by an Open Responses endpoint.
pub struct ResponsesDoor {
    http: reqwest::Client,
    /// The door's base URL; the route is `/v1/responses` under it.
    pub url: String,
    /// The model the door runs.
    pub model: String,
    key: String,
    patience: Patience,
}

impl ResponsesDoor {
    /// A door for `url` serving `model` behind `key`.
    pub fn new(url: impl Into<String>, model: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            http: client(),
            url: url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            key: key.into(),
            patience: Patience::default(),
        }
    }

    /// A door from the environment: `CODER_DOOR_URL` or the public gateway,
    /// `CODER_MODEL` or the default lane's model, `CODER_DOOR_KEY` or
    /// `CODER_AI_GATEWAY_KEY` for the bearer. `None` when no key is set.
    ///
    /// `CODER_MODEL` takes a lane's short name as readily as a model id,
    /// so `glm` and `zai/glm-5.3-flash` ask for the same door.
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
        let model = model_from_env(MODEL_VAR).unwrap_or_else(|| DEFAULT_MODEL.to_string());
        Some(Self::new(url, model, key))
    }

    /// The same door running `model`, which may be named as a lane.
    #[must_use]
    pub fn serving(mut self, model: &str) -> Self {
        self.model = model_named(model).to_string();
        self
    }

    /// The same door with different waits, so a test can exercise a bound
    /// without spending the real one.
    #[must_use]
    pub fn waiting(mut self, patience: Patience) -> Self {
        self.patience = patience;
        self
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

/// The Server-Sent Events reader: bytes in, answer text and usage out.
///
/// One reader serves every lane, because the gateway sends one event shape
/// for every model in its catalog. That is what makes the recorded streams
/// under `crates/coder/fixtures/gateway/` worth pinning: a change in the
/// gateway's event shape reaches a test here rather than a broken turn.
#[derive(Default)]
struct Reader {
    /// Bytes that have not yet formed a whole line. Held as bytes rather
    /// than as text, because a chunk boundary can fall inside a character
    /// and decoding each chunk on its own puts replacement characters into
    /// the answer.
    buffer: Vec<u8>,
    /// The answer as it has arrived.
    text: String,
    /// The token counts the completed event carried.
    usage: Option<Usage>,
    /// How many stream events have been read, which a timeout reports.
    events: u32,
}

impl Reader {
    /// Reads one chunk, handing each text delta to `sink`.
    ///
    /// Answers with how many events the chunk completed, so a caller can
    /// measure a stream's silence between events rather than between
    /// bytes.
    ///
    /// # Errors
    ///
    /// Returns [`GenerateError::Stream`] when the stream carries a failure
    /// event.
    fn push(
        &mut self,
        chunk: &[u8],
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<u32, GenerateError> {
        self.buffer.extend_from_slice(chunk);
        let mut read = 0;
        // `data: {json}` lines, blank-line separated. An SSE line never
        // spans a newline, so decoding one whole line at a time is safe
        // where decoding a chunk is not.
        while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let Some(data) = line.trim_end_matches(['\n', '\r']).strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(event) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            read += 1;
            self.events += 1;
            match event["type"].as_str().unwrap_or_default() {
                "response.output_text.delta" => {
                    if let Some(delta) = event["delta"].as_str() {
                        self.text.push_str(delta);
                        sink(delta);
                    }
                }
                "response.completed" => {
                    self.usage = event["response"]["usage"].as_object().map(|u| Usage {
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
                // Everything else is the shape around the answer:
                // lifecycle events, content-part frames, and the reasoning
                // deltas a thinking model streams. None of them is the
                // answer, and a lane that sends them must not have them
                // spliced into one.
                _ => {}
            }
        }
        Ok(read)
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
        let sent = self
            .http
            .post(format!("{}/v1/responses", self.url))
            .bearer_auth(&self.key)
            .json(&self.body(instructions, input))
            .send();
        let response = match tokio::time::timeout(self.patience.first_word, sent).await {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => return Err((String::new(), GenerateError::Transport(error))),
            // No headers, so nothing was heard and the request may go
            // again. The attempt count is added where the attempts are
            // counted.
            Err(_) => {
                return Err((
                    String::new(),
                    GenerateError::Quiet {
                        heard: false,
                        reason: format!(
                            "no response headers in {} seconds",
                            self.patience.first_word.as_secs()
                        ),
                    },
                ));
            }
        };
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err((
                String::new(),
                GenerateError::Status(status.as_u16(), clip(&body, 400)),
            ));
        }

        let mut reader = Reader::default();
        let mut stream = response.bytes_stream();
        // The quiet clock runs from the last event rather than from the
        // last byte, so a door that dribbles bytes without completing an
        // event is still quiet.
        let mut spoke = Instant::now();
        loop {
            let left = self.patience.quiet.saturating_sub(spoke.elapsed());
            let chunk = match tokio::time::timeout(left, stream.next()).await {
                Ok(Some(Ok(chunk))) => chunk,
                Ok(Some(Err(error))) => {
                    return Err((reader.text, GenerateError::Transport(error)));
                }
                Ok(None) => break,
                Err(_) => return Err((reader.text.clone(), self.went_quiet(&reader))),
            };
            match reader.push(&chunk, sink) {
                Ok(0) => {}
                Ok(_) => spoke = Instant::now(),
                Err(error) => return Err((reader.text, error)),
            }
        }
        if reader.text.is_empty() {
            return Err((
                reader.text,
                GenerateError::Stream("the stream carried no text".to_string()),
            ));
        }
        Ok((reader.text, reader.usage))
    }

    /// The failure a stream that stopped sending becomes.
    ///
    /// It names the wait and what had arrived, because a door that hung
    /// before saying anything and a door that answered part way and
    /// stopped are different problems, and a turn that dies here is often
    /// the only record of which one happened.
    fn went_quiet(&self, reader: &Reader) -> GenerateError {
        GenerateError::Quiet {
            heard: true,
            reason: format!(
                "{} seconds of silence after {} events and {} characters",
                self.patience.quiet.as_secs(),
                reader.events,
                reader.text.chars().count()
            ),
        }
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
        // Two kinds of attempt are counted, because two kinds of failure
        // earn another one.
        //
        // A stream that fails before showing anything — empty, or holding
        // only a hidden plan — is safe to redo: the user saw nothing and
        // the request is idempotent. Upstream flakes like a model-side
        // malformed function call are transient, and the retry is
        // invisible.
        //
        // A door that never sent response headers is the other kind. It
        // is indistinguishable from one that dropped the connection, so
        // the request goes again after `Patience::retry_wait`, which
        // doubles on the third attempt.
        //
        // Nothing else is redone. A door that sent headers and then went
        // quiet has already shown the caller where the turn got to, and a
        // second attempt would repeat it. A door that keeps failing after
        // the last attempt surfaces its error — hidden retries, honest
        // failures.
        let mut empty: usize = 0;
        let mut unanswered: u32 = 0;
        loop {
            let (partial, error) = match self.once(instructions, input, sink).await {
                Ok(done) => return Ok(done),
                Err(failed) => failed,
            };
            match &error {
                GenerateError::Quiet { heard: false, .. } => {
                    unanswered += 1;
                    if unanswered >= HEADER_ATTEMPTS {
                        return Err(GenerateError::Quiet {
                            heard: false,
                            reason: format!(
                                "no response headers in {} seconds, over {unanswered} attempts",
                                self.patience.first_word.as_secs()
                            ),
                        });
                    }
                    tokio::time::sleep(self.patience.retry_wait * (1 << (unanswered - 1))).await;
                }
                GenerateError::Stream(_) | GenerateError::Transport(_)
                    if partial.is_empty() || planish(&partial) =>
                {
                    empty += 1;
                    if empty >= EMPTY_STREAM_ATTEMPTS {
                        return Err(error);
                    }
                    tokio::time::sleep(Duration::from_millis(300 * empty as u64)).await;
                }
                _ => return Err(error),
            }
        }
    }
}

/// A door that is not there: it answers with a fixed line. The shell and
/// the tests use it so neither needs credentials.
///
/// A test that needs a conversation rather than one line gives the stub a
/// script: [`StubGenerate::scripted`] plays each line once, in order, and
/// then says `line` for every generation after that.
pub struct StubGenerate {
    /// What the stub says once the script, if any, is spent.
    pub line: String,
    /// Lines still to play before `line`, front first.
    script: std::sync::Mutex<std::collections::VecDeque<String>>,
}

impl StubGenerate {
    /// A stub that says `line`, every time.
    #[must_use]
    pub fn saying(line: impl Into<String>) -> Self {
        Self {
            line: line.into(),
            script: std::sync::Mutex::default(),
        }
    }

    /// A stub that plays `script` once and then says `line`.
    #[must_use]
    pub fn scripted(script: Vec<String>, line: impl Into<String>) -> Self {
        Self {
            line: line.into(),
            script: std::sync::Mutex::new(script.into()),
        }
    }
}

impl Default for StubGenerate {
    fn default() -> Self {
        Self::saying("(stub door: set CODER_DOOR_KEY or CODER_AI_GATEWAY_KEY for a real answer)")
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
        let line = self
            .script
            .lock()
            .ok()
            .and_then(|mut script| script.pop_front())
            .unwrap_or_else(|| self.line.clone());
        sink(&line);
        Ok((line, None))
    }
}

/// Whatever the environment gives: an own-key door when a key is set,
/// the relay when a worker is configured, the stub otherwise.
pub enum Door {
    /// A live Open Responses endpoint.
    Live(ResponsesDoor),
    /// A Nostr relay running the NIP-CJ job protocol.
    Relay(Box<crate::relay::RelayDoor>),
    /// An approved local executor, such as the Devin CLI, run through
    /// `delegate` under its boundary.
    Executor(Box<crate::executor_door::ExecutorDoor>),
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
        let executor =
            env::var(crate::executor_door::EXECUTOR_VAR).is_ok_and(|value| !value.is_empty());
        match asked_for(key, worker, executor)? {
            Asked::Own => ResponsesDoor::from_env()
                .map(Door::Live)
                .ok_or_else(|| "the door key is set and empty".to_string()),
            Asked::Relay => {
                crate::relay::RelayDoor::from_env().map(|door| Door::Relay(Box::new(door)))
            }
            Asked::Executor => crate::executor_door::ExecutorDoor::from_env()?
                .map(|door| Door::Executor(Box::new(door)))
                .ok_or_else(|| "the executor slug is set and empty".to_string()),
            Asked::Stub => Ok(Door::Stub(StubGenerate::default())),
        }
    }

    /// The same door running `model`, which may be named as a lane.
    ///
    /// This is how a caller whose lane is configured separately from the
    /// agent's takes it: `coder-worker` reads [`WORKER_MODEL_VAR`] and
    /// passes what it finds here.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the door does not pick its own model. A
    /// relay door's worker picks, and the stub answers a fixed line, so a
    /// lane named for either is refused rather than ignored — the same
    /// reason [`Door::from_env`] refuses an environment that names two
    /// doors.
    pub fn serving(self, model: &str) -> Result<Self, String> {
        // Resolved first, so a refusal names the model rather than the
        // lane: what a person needs to read is what would have run.
        let model = model_named(model);
        match self {
            Door::Live(door) => Ok(Door::Live(door.serving(model))),
            Door::Relay(_) => Err(format!(
                "{model} is named for a door that does not pick its model: \
                 the relay carries the turn to a worker and the worker picks."
            )),
            Door::Executor(door) => Err(format!(
                "{model} is named for a door that does not pick its model: \
                 {} runs the turn and picks its own.",
                door.slug()
            )),
            Door::Stub(_) => Err(format!(
                "{model} is named and no door key is set, so the stub door \
                 would answer instead. Set {} or {}.",
                KEY_VARS[0], KEY_VARS[1]
            )),
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
            Door::Executor(door) => door.slug(),
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
            Door::Executor(_) => "executor",
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
            Door::Executor(door) => door.generate(instructions, input, sink, meta).await,
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
    /// An approved local executor.
    Executor,
    /// None of them, so the canned answer.
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
fn asked_for(key: Option<&str>, worker: bool, executor: bool) -> Result<Asked, String> {
    let mut named = Vec::new();
    if let Some(key) = key {
        named.push(format!("{key} asks for an own-key door"));
    }
    if worker {
        named.push(format!("{WORKER_VAR} asks for the relay"));
    }
    if executor {
        named.push(format!(
            "{} asks for a local executor",
            crate::executor_door::EXECUTOR_VAR
        ));
    }
    if named.len() > 1 {
        return Err(format!(
            "the environment names {} doors: {}. Unset all but one.",
            named.len(),
            named.join(", ")
        ));
    }
    Ok(match (key, worker, executor) {
        (Some(_), _, _) => Asked::Own,
        (None, true, _) => Asked::Relay,
        (None, false, true) => Asked::Executor,
        (None, false, false) => Asked::Stub,
    })
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
        let both = asked_for(Some("CODER_DOOR_KEY"), true, false).expect_err("two doors");
        assert!(both.contains("CODER_DOOR_KEY"), "{both}");
        assert!(both.contains("CODER_WORKER"), "{both}");
        let with_executor = asked_for(None, true, true).expect_err("two doors");
        assert!(with_executor.contains("CODER_EXECUTOR"), "{with_executor}");

        assert_eq!(
            asked_for(Some("CODER_DOOR_KEY"), false, false),
            Ok(Asked::Own)
        );
        assert_eq!(
            asked_for(Some("CODER_AI_GATEWAY_KEY"), false, false),
            Ok(Asked::Own)
        );
        assert_eq!(asked_for(None, true, false), Ok(Asked::Relay));
        assert_eq!(asked_for(None, false, true), Ok(Asked::Executor));
        assert_eq!(asked_for(None, false, false), Ok(Asked::Stub));
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

    /// Two lanes, two models, one client. A lane is readable by short
    /// name and by model id, so a shell that says `glm` and one that says
    /// `zai/glm-5.3-flash` ask for the same door.
    #[test]
    fn a_lane_is_a_model_under_a_short_name() {
        assert_eq!(Lane::Gemini.model(), "google/gemini-3.8-flash");
        assert_eq!(Lane::Glm.model(), "zai/glm-5.3-flash");
        assert_eq!(DEFAULT_MODEL, Lane::Gemini.model());

        assert_eq!(Lane::read("glm"), Some(Lane::Glm));
        assert_eq!(Lane::read(" zai/glm-5.3-flash "), Some(Lane::Glm));
        assert_eq!(Lane::read("gemini"), Some(Lane::Gemini));
        assert_eq!(Lane::read("moonshot/kimi"), None);

        // A name that is no lane is a model id, so the gateway's whole
        // catalog stays reachable without a lane of its own.
        assert_eq!(model_named("glm"), "zai/glm-5.3-flash");
        assert_eq!(model_named(" moonshot/kimi "), "moonshot/kimi");

        // Every lane's name and model are distinct, which is what lets
        // one field carry either.
        let mut seen: Vec<&str> = Lane::ALL
            .iter()
            .flat_map(|lane| [lane.name(), lane.model()])
            .collect();
        seen.sort_unstable();
        let count = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), count);
    }

    /// A door that picks its own model takes a lane; one that does not
    /// refuses rather than dropping the request.
    ///
    /// A worker told to run `glm` while its door is the relay would answer
    /// on whatever model the far end picked, and the trace would name that
    /// model with nothing recording that the lane had been asked for and
    /// ignored.
    #[test]
    fn only_a_door_that_picks_its_model_takes_a_lane() {
        let live = Door::Live(ResponsesDoor::new(DEFAULT_DOOR_URL, DEFAULT_MODEL, "k"))
            .serving("glm")
            .expect("a live door takes a lane");
        assert_eq!(live.model(), Lane::Glm.model());
        assert_eq!(live.label(), Lane::Glm.model());

        let Err(stub) = Door::Stub(StubGenerate::default()).serving("glm") else {
            panic!("the stub door does not run a model");
        };
        assert!(stub.contains(Lane::Glm.model()), "{stub}");
        assert!(stub.contains("CODER_DOOR_KEY"), "{stub}");
    }

    /// Four door failures, four causes, and the streaming door's two words
    /// for silence sit beside the relay's rather than replacing them.
    #[test]
    fn a_quiet_door_files_under_two_causes() {
        let absent = GenerateError::Quiet {
            heard: false,
            reason: "no response headers in 30 seconds, over 3 attempts".to_string(),
        };
        let stalled = GenerateError::Quiet {
            heard: true,
            reason: "120 seconds of silence after 12 events and 400 characters".to_string(),
        };

        assert_eq!(absent.cause(), "door_absent");
        assert_eq!(stalled.cause(), "door_stalled");
        assert_eq!(absent.refusal(), None);
        assert_eq!(stalled.refusal(), None);
        assert_eq!(
            absent.to_string(),
            "the door did not answer: no response headers in 30 seconds, over 3 attempts"
        );
        // How long it waited and how much had arrived: "it hung" and "it
        // sent 400 characters and stopped" are different problems.
        assert_eq!(
            stalled.to_string(),
            "the door went quiet mid-answer: 120 seconds of silence after 12 events \
             and 400 characters"
        );
    }

    #[test]
    fn no_key_means_no_door() {
        // The lookup reads the real environment; with neither variable set a
        // door cannot be built. When a key IS set the test still passes: the
        // door exists. What matters is it never panics.
        let _ = ResponsesDoor::from_env();
    }
}
