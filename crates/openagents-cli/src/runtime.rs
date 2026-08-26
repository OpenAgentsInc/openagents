//! The coder execution layer: tiers, threads, grants, and the streaming turn.
//!
//! Two lanes reach a model, and they share nothing but this file's message
//! list:
//!
//! - The **thread lane** opens `POST /api/v1/threads`, takes the grant that
//!   comes back, and streams `POST /api/inference/proxy` with the grant's
//!   bearer token. The thread is revoked with `DELETE /api/v1/threads/{id}`.
//! - The **local lane** talks to an Ollama server on this machine and never
//!   touches openagents.com at all, so it answers with the proxy unreachable.
//!
//! ## A turn that cannot reach a model fails
//!
//! Every path out of [`CoderRuntimeSession::execute_turn`] is either the
//! model's own words or an `Err`. There is no fallback model, no synthesized
//! reply, and no invented grant. Two of those existed here and both reached
//! readers: `create_thread` answered a refusal by returning a grant it made up
//! with the caller's own PAT inside it, and the proxy arm answered a rejected
//! request with the sentence `Completed autonomous reasoning turn (offline
//! fallback).` and exit 0. Neither comes back.
//!
//! ## Model ids are the server's, not this file's
//!
//! The deployment publishes its catalog at `GET /api/v1/models` and refuses
//! anything outside it. A previous version of this file carried five model ids
//! it had invented — `gemini-3.7-pro`, `claude-3-7-sonnet`, `codex-preview`
//! among them — and sent them as the thread's `lane`, which the server also
//! refuses. The tier table below maps the names a reader types onto ids the
//! catalog actually served when it was written, and a name outside that table
//! is checked against the live catalog before a thread is opened rather than
//! guessed at.

use crate::tools::{HarnessToolRegistry, ToolCall, ToolDefinition};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::time::Duration;

type Failure = Box<dyn std::error::Error + Send + Sync>;

pub const THREAD_LANE_NOTICE: &str =
    "You answer through the OpenAgents inference proxy, on a thread opened for this session. \
    Every round of tool calls re-sends the whole conversation to a metered model, so batch \
    independent commands into one call and keep large dumps out of the transcript.";

pub const LOCAL_LANE_NOTICE: &str =
    "You answer from a model running on this machine through Ollama. Nothing in this \
    conversation leaves the machine and nothing is metered, but the context window is a \
    fraction of a hosted model's, so keep large dumps out of the transcript.";

/// Where an Ollama server listens unless `OPENAGENTS_OLLAMA_HOST` says otherwise.
pub const OLLAMA_HOST: &str = "http://127.0.0.1:11434";

/// How many rounds of tool calls one turn may take before it has to answer.
///
/// A backstop against a model that loops, not a budget.
const MAX_TOOL_STEPS: usize = 30;

/// The tier names a reader may type, and the catalog id each one opens on.
///
/// A tier is the unit `--lane` deals in: `flash` is the fast lane whatever
/// model is behind it this month, so renaming a vendor model is a one-line
/// change here and nothing else in the crate holds a vendor string. The ids on
/// the right were served by `GET /api/v1/models` when this was written; the
/// server is the authority and refuses any that stop being true.
pub const TIERS: &[(&str, &str)] = &[
    ("flash", "gemini-3.7-flash"),
    ("pro", "gpt-5.6-luna"),
    ("ox-alpha", "ox-alpha"),
];

/// What `--lane` takes, for a refusal that leaves the reader somewhere to go.
pub fn admitted_lanes() -> String {
    let tiers = TIERS
        .iter()
        .map(|(name, id)| format!("{name} ({id})"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "auto (the deployment's own default), {tiers}, \
         local or ollama:<model> for a model on this machine"
    )
}

/// Which lane a turn runs on.
///
/// [`Lane::from_str`] is total on purpose: an unrecognised name becomes
/// [`Lane::Named`] and is checked against the live catalog at the top of the
/// turn, so `--lane bogus` is refused by name with the list of what this
/// deployment serves. It used to fall through to `_ => Lane::OxAlpha`, which
/// ran the default lane while the reader believed they had chosen another.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Lane {
    /// No model named at thread open. The deployment's own default answers.
    Auto,
    /// The `ox-alpha` tier.
    #[default]
    OxAlpha,
    /// The fast tier.
    Flash,
    /// The strong tier.
    Pro,
    /// A model id named directly, checked against `GET /api/v1/models`.
    Named(String),
    /// Ollama on this machine. An empty string means "whatever is installed".
    Local(String),
}

impl Lane {
    /// Named `from_str` rather than `FromStr::from_str` because it cannot
    /// fail: a name nothing admits is carried to the turn, where the catalog
    /// settles it and the refusal can name what this deployment serves.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "" | "auto" => Lane::Auto,
            "ox-alpha" | "ox" | "openagents" => Lane::OxAlpha,
            "flash" | "coder-flash" | "gemini" | "gemini-flash" | "gemini-3.7-flash" => Lane::Flash,
            "pro" | "coder-pro" | "gpt-5.6-luna" | "luna" => Lane::Pro,
            "local" | "ollama" => Lane::Local(String::new()),
            other if other.starts_with("ollama:") => {
                Lane::Local(other.trim_start_matches("ollama:").trim().to_string())
            }
            other => Lane::Named(other.to_string()),
        }
    }

    /// The catalog id to send at thread open, or `None` to let the server pick.
    pub fn model_id(&self) -> Option<&str> {
        match self {
            Lane::Auto => None,
            Lane::OxAlpha => Some("ox-alpha"),
            Lane::Flash => Some(TIERS[0].1),
            Lane::Pro => Some(TIERS[1].1),
            Lane::Named(id) => Some(id.as_str()),
            // The local lane names its model to Ollama, never to the server.
            Lane::Local(_) => None,
        }
    }

    /// Whether this lane answers from this machine.
    pub fn is_local(&self) -> bool {
        matches!(self, Lane::Local(_))
    }

    /// The tier this lane belongs to: `auto`, `flash`, `pro`, or `local`.
    ///
    /// A model id no tier pins has no tier, which is a different answer from
    /// "auto" and is worth keeping separate — a reader who named a model
    /// directly did not ask for a tier.
    pub fn tier(&self) -> Option<&'static str> {
        match self {
            Lane::Auto => Some("auto"),
            Lane::Flash => Some("flash"),
            Lane::Pro => Some("pro"),
            Lane::Local(_) => Some("local"),
            Lane::OxAlpha | Lane::Named(_) => None,
        }
    }

    /// The name for this lane on a status line.
    pub fn label(&self) -> String {
        match self {
            Lane::Auto => "Coder Auto".to_string(),
            Lane::Flash => "Coder Flash".to_string(),
            Lane::Pro => "Coder Pro".to_string(),
            Lane::Local(model) if model.is_empty() => "Coder Local".to_string(),
            Lane::Local(model) => format!("Coder Local ({model})"),
            Lane::OxAlpha => "Coder (ox-alpha)".to_string(),
            Lane::Named(id) => format!("Coder ({id})"),
        }
    }
}

/// As much of an error body as belongs in a one-line message.
fn snippet(body: &str) -> String {
    let body = body.trim();
    if body.chars().count() <= 200 {
        return body.to_string();
    }
    let head: String = body.chars().take(200).collect();
    format!("{head}…")
}

/// One model as `GET /api/v1/models` publishes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedModel {
    pub id: String,
    /// Served here *and* its provider credential configured.
    pub available: bool,
    pub default: bool,
}

/// What one turn spent, as the server reported it.
///
/// Reported rather than estimated. The proxy sends a final chunk carrying
/// `usage` and Ollama sends its counts on the `done` line; both land here, and
/// a lane that reports nothing leaves this zero rather than guessing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

impl TurnUsage {
    fn add(&mut self, other: TurnUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
    }

    pub fn reported(&self) -> bool {
        self.total_tokens > 0 || self.prompt_tokens > 0 || self.completion_tokens > 0
    }

    /// One line for a transcript or a status bar.
    pub fn line(&self) -> String {
        format!(
            "{} prompt + {} completion = {} tokens",
            self.prompt_tokens, self.completion_tokens, self.total_tokens
        )
    }
}

/// A grant the server issued. Never constructed from anything else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceGrant {
    pub thread_id: String,
    pub token: String,
    pub proxy_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

pub struct CoderRuntimeSession {
    pub lane: Lane,
    /// The grant this session opened, reused across its turns.
    ///
    /// `None` on the local lane, always: there is no grant to hold, and
    /// putting a made-up one here is the bug this file exists to keep dead.
    pub last_grant: Option<InferenceGrant>,
    /// The model that actually answered the last turn.
    ///
    /// From the grant on the thread lane, and from the resolved Ollama name on
    /// the local lane. A caller reporting what answered should read this
    /// rather than the lane, which is only what was asked for.
    pub last_model: Option<String>,
    /// What the last turn spent, summed over its steps.
    pub last_usage: TurnUsage,
    /// The reasoning the last turn emitted, if the model emits any.
    ///
    /// Kept off the content callback deliberately: `delta.reasoning` and
    /// `delta.content` interleave on the wire, and appending both to the
    /// transcript would put the model's scratch work in the middle of its
    /// answer. It is parsed, summed and kept here so a caller that wants to
    /// show it can, and the transcript stays the answer.
    pub last_reasoning: String,
    pub api_base: String,
    pub user_token: Option<String>,
    pub ollama_host: String,
    pub http: reqwest::Client,
    pub tools: HarnessToolRegistry,
    pub messages: Vec<ChatMessage>,
    /// The thread to revoke when the session closes.
    thread_id: Option<String>,
}

impl CoderRuntimeSession {
    pub fn new(
        lane: Lane,
        api_base: Option<String>,
        user_token: Option<String>,
        tools: HarnessToolRegistry,
    ) -> Self {
        Self {
            lane,
            last_grant: None,
            last_model: None,
            last_usage: TurnUsage::default(),
            last_reasoning: String::new(),
            // `OPENAGENTS_API_BASE` points the session at another host. A test
            // that has to prove the streaming path end to end needs somewhere
            // to point it that is not production, and an operator on staging
            // needs the same switch.
            api_base: api_base
                .or_else(|| {
                    std::env::var("OPENAGENTS_API_BASE")
                        .ok()
                        .filter(|v| !v.trim().is_empty())
                })
                .unwrap_or_else(|| "https://openagents.com/api/v1".to_string()),
            user_token,
            ollama_host: std::env::var("OPENAGENTS_OLLAMA_HOST")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| OLLAMA_HOST.to_string()),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
            tools,
            messages: Vec::new(),
            thread_id: None,
        }
    }

    pub fn build_system_prompt(&self, tool_defs: &[ToolDefinition]) -> String {
        let notice = if self.lane.is_local() {
            LOCAL_LANE_NOTICE
        } else {
            THREAD_LANE_NOTICE
        };
        let mut lines = vec![
            format!("You are `openagents coder`, a coding assistant in a terminal. {notice}"),
            "".to_string(),
            "Answer very concisely unless the reader asks for a longer response.".to_string(),
            "".to_string(),
        ];

        if tool_defs.is_empty() {
            lines.push(
                "You have no tools in this session: you cannot read or write files, run commands, or \
                reach anything outside this conversation. Answer from what the reader tells you, and \
                say plainly when something would need a tool you do not have.".to_string()
            );
        } else {
            lines.push(format!(
                "You have {} tools, and no others:",
                tool_defs.len()
            ));
            for t in tool_defs {
                lines.push(format!("- `{}`", t.name));
            }
            lines.push("".to_string());
            lines.push(
                "That list is complete: a capability not on it is one you do not have, whatever a model \
                like you usually has. Read a tool's description before assuming what it covers. Where \
                a description says what a child agent can do, that is the child's capability and not \
                yours. Never say you ran something you did not run.".to_string()
            );
        }

        // Skill injection. The `skill` tool's catalog is names and
        // descriptions only, so a body costs nothing until it is asked for.
        // A skill marked `auto: true` in its front matter is the exception:
        // it says how to approach the work, and a session needs the method
        // before its first decision rather than after thinking to ask.
        if let Some(context) = self.tools.standing_context() {
            lines.push("".to_string());
            lines.push(context);
        }

        lines.join("\n")
    }

    // ───────────────────────────────────────────────────────── the catalog

    /// What this deployment serves, read from the server rather than assumed.
    pub async fn served_models(&self) -> Result<Vec<ServedModel>, Failure> {
        let url = format!("{}/models", self.api_base);
        let mut request = self.http.get(&url).timeout(Duration::from_secs(15));
        if let Some(token) = &self.user_token {
            request = request.bearer_auth(token);
        }
        let resp = request.send().await.map_err(|error| -> Failure {
            format!("{url} could not be reached: {error}").into()
        })?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "{url} refused the catalog request: {status} {}",
                snippet(&body)
            )
            .into());
        }
        let body: serde_json::Value = resp.json().await?;
        let models = body
            .get("models")
            .and_then(|v| v.as_array())
            .ok_or_else(|| -> Failure { format!("{url} published no model list").into() })?;
        Ok(models
            .iter()
            .filter_map(|model| {
                let id = model.get("id").and_then(|v| v.as_str())?;
                Some(ServedModel {
                    id: id.to_string(),
                    // Any word other than `available` is read as unavailable: a
                    // vocabulary this client has not seen is a reason to pick
                    // another model, not to assume the new word is benign.
                    available: model.get("availability").and_then(|v| v.as_str())
                        == Some("available"),
                    default: model.get("default").and_then(|v| v.as_bool()) == Some(true),
                })
            })
            .collect())
    }

    /// Refuse a directly-named model the catalog cannot answer with.
    ///
    /// The tiers skip this: they are checked at open, and the server's own 422
    /// carries the news if one is retired. Only a name this crate does not
    /// recognise pays for the round trip, and it pays it so that `--lane
    /// bogus` earns a sentence naming what it could have said instead.
    async fn check_named(&self, id: &str) -> Result<(), Failure> {
        let served = match self.served_models().await {
            Ok(served) => served,
            Err(error) => {
                return Err(format!(
                    "'{id}' is not a lane this CLI knows, and the catalog that would settle it \
                     could not be read: {error}. Lanes: {}.",
                    admitted_lanes()
                )
                .into())
            }
        };
        let usable: Vec<&str> = served
            .iter()
            .filter(|m| m.available)
            .map(|m| m.id.as_str())
            .collect();
        let alternatives = if usable.is_empty() {
            "This deployment has no model with a configured provider credential.".to_string()
        } else {
            format!("This deployment serves {}.", usable.join(", "))
        };

        match served.iter().find(|m| m.id == id) {
            None => Err(format!(
                "'{id}' is not a lane this CLI knows and no model of that name is served here. \
                 {alternatives} Lanes: {}.",
                admitted_lanes()
            )
            .into()),
            Some(model) if !model.available => Err(format!(
                "'{id}' is in the catalog but its provider is not configured on this deployment. \
                 {alternatives}"
            )
            .into()),
            Some(_) => Ok(()),
        }
    }

    // ────────────────────────────────────────────────── threads and grants

    pub async fn create_thread(&self) -> Result<InferenceGrant, Failure> {
        let url = format!("{}/threads", self.api_base);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(tok) = &self.user_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {tok}"))?,
            );
        }

        // `lane` is the thread's execution shape and the server admits only
        // `thread` and `local`; this path is the proxy, which is `thread`. It
        // used to send a model name here and every request was refused with
        // `"ox-alpha" is not an admitted lane` — invisibly, because the caller
        // answered the refusal with a fabricated grant.
        //
        // `model` is separate and optional. Omitting it opens on the
        // deployment's own default, which is what `Lane::Auto` wants; naming
        // one pins the lane. Either way the grant that comes back is the
        // authority on which model answers, and that is what gets reported.
        let mut body = serde_json::json!({
            "objective": "Coding assistant session",
            "lane": "thread",
        });
        if let Some(model) = self.lane.model_id() {
            body["model"] = serde_json::json!(model);
        }

        let resp = self
            .http
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            // This used to invent a grant with a placeholder token and carry
            // on, so a refused request reached the reader as a completed turn.
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!(
                "{url} refused the thread request: {status} {}",
                snippet(&text)
            )
            .into());
        }

        let body: serde_json::Value = resp.json().await?;
        let thread = body.get("thread").cloned().unwrap_or(serde_json::json!({}));
        let grant = body.get("grant").cloned().unwrap_or(serde_json::json!({}));

        let thread_id = thread
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| -> Failure {
                format!("{url} accepted the thread but published no id").into()
            })?
            .to_string();
        let token = grant
            .get("token")
            .and_then(|v| v.as_str())
            .filter(|t| !t.is_empty())
            .ok_or_else(|| -> Failure {
                format!(
                    "{url} opened thread {thread_id} but minted no inference grant, \
                     so there is no token to call the proxy with"
                )
                .into()
            })?
            .to_string();
        let proxy_url = grant
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| -> Failure {
                format!("the grant on thread {thread_id} names no proxy url").into()
            })?
            .to_string();
        let model = grant
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| -> Failure {
                format!("the grant on thread {thread_id} names no model").into()
            })?
            .to_string();

        Ok(InferenceGrant {
            thread_id,
            token,
            proxy_url,
            model,
        })
    }

    /// Revoke this session's thread.
    ///
    /// A thread left open holds its grant's remaining budget. `DELETE
    /// /api/v1/threads/{id}` closes both and returns the grant's spend, which
    /// is why the reply is worth reading rather than discarding.
    pub async fn close(&mut self) -> Result<Option<TurnUsage>, Failure> {
        let Some(thread_id) = self.thread_id.take() else {
            return Ok(None);
        };
        self.last_grant = None;
        let url = format!("{}/threads/{thread_id}", self.api_base);
        let mut request = self.http.delete(&url).timeout(Duration::from_secs(30));
        if let Some(token) = &self.user_token {
            request = request.bearer_auth(token);
        }
        let resp = request.send().await.map_err(|error| -> Failure {
            format!("{url} could not be reached: {error}").into()
        })?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(
                format!("{url} refused the revocation: {status} {}", snippet(&body)).into(),
            );
        }
        let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
        let spent = body.get("grant").and_then(|g| g.get("spent"));
        Ok(spent.map(|spent| TurnUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: spent
                .get("total_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        }))
    }

    // ─────────────────────────────────────────────────────────── the turn

    pub async fn execute_turn<F>(
        &mut self,
        prompt: &str,
        chunk_callback: F,
    ) -> Result<String, Failure>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let tool_defs = self.tools.list_tools();
        if self.messages.is_empty() {
            let sys = self.build_system_prompt(&tool_defs);
            self.messages.push(ChatMessage {
                role: "system".to_string(),
                content: Some(sys),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: Some(prompt.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });

        self.last_usage = TurnUsage::default();
        self.last_reasoning.clear();

        if self.lane.is_local() {
            self.run_local_turn(&tool_defs, chunk_callback).await
        } else {
            self.run_thread_turn(&tool_defs, chunk_callback).await
        }
    }

    async fn run_thread_turn<F>(
        &mut self,
        tool_defs: &[ToolDefinition],
        mut chunk_callback: F,
    ) -> Result<String, Failure>
    where
        F: FnMut(&str) + Send + 'static,
    {
        if let Lane::Named(id) = self.lane.clone() {
            self.check_named(&id).await?;
        }

        // One thread per session, reused across its turns. Opening a fresh one
        // per turn threw away the conversation's own budget and left a trail of
        // open threads nothing ever revoked.
        let grant = match &self.last_grant {
            Some(grant) => grant.clone(),
            None => {
                let grant = self.create_thread().await?;
                self.thread_id = Some(grant.thread_id.clone());
                self.last_grant = Some(grant.clone());
                grant
            }
        };
        self.last_model = Some(grant.model.clone());

        let mut final_answer = String::new();

        for _ in 0..MAX_TOOL_STEPS {
            let req_body = serde_json::json!({
                "model": grant.model,
                "messages": self.messages,
                "tools": tool_defs.iter().map(|t| serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })).collect::<Vec<_>>(),
                "stream": true
            });

            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", grant.token))?,
            );

            let resp = self
                .http
                .post(&grant.proxy_url)
                .headers(headers)
                .json(&req_body)
                .send()
                .await;

            // A refused or unreachable proxy is a failed turn. The version
            // this replaces streamed the words `Completed autonomous reasoning
            // turn (offline fallback).` and returned success, so a rejected
            // request and a finished one looked the same on screen.
            let resp = match resp {
                Ok(r) if r.status().is_success() => r,
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    return Err(format!(
                        "{} refused the turn: {status} {}",
                        grant.proxy_url,
                        snippet(&body)
                    )
                    .into());
                }
                Err(error) => {
                    return Err(format!("{} could not be reached: {error}", grant.proxy_url).into())
                }
            };

            let mut stream = resp.bytes_stream().eventsource();
            let mut step = StepAccumulator::default();

            while let Some(event) = stream.next().await {
                let event = match event {
                    Ok(event) => event,
                    Err(error) => {
                        return Err(format!(
                            "the reply from {} stopped mid-stream: {error}",
                            grant.proxy_url
                        )
                        .into())
                    }
                };
                if event.data == "[DONE]" {
                    break;
                }
                let Ok(json) = serde_json::from_str::<serde_json::Value>(&event.data) else {
                    continue;
                };
                step.absorb_openai(&json, &mut chunk_callback);
            }

            self.last_usage.add(step.usage);
            self.last_reasoning.push_str(&step.reasoning);

            if step.tool_calls.is_empty() {
                final_answer = step.content;
                break;
            }
            self.run_tools(step).await;
        }

        Ok(final_answer)
    }

    /// Record the assistant's tool calls, run them, and put the results back.
    async fn run_tools(&mut self, step: StepAccumulator) {
        let recorded: Vec<serde_json::Value> = step
            .tool_calls
            .values()
            .map(|(id, name, args)| {
                serde_json::json!({
                    "id": id,
                    "type": "function",
                    "function": { "name": name, "arguments": args }
                })
            })
            .collect();

        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: if step.content.is_empty() {
                None
            } else {
                Some(step.content)
            },
            tool_calls: Some(recorded),
            tool_call_id: None,
        });

        for (id, name, args_str) in step.tool_calls.into_values() {
            let arguments: serde_json::Value =
                serde_json::from_str(&args_str).unwrap_or(serde_json::json!({}));
            let call = ToolCall {
                id: id.clone(),
                name: name.clone(),
                arguments,
            };
            let result = self.tools.execute_tool(&call).await;
            self.messages.push(ChatMessage {
                role: "tool".to_string(),
                content: Some(result.output),
                tool_calls: None,
                tool_call_id: Some(id),
            });
        }
    }

    // ────────────────────────────────────────────────────── the local lane

    /// The Ollama models installed here, most recently modified first.
    pub async fn installed_local_models(&self) -> Result<Vec<String>, Failure> {
        let url = format!("{}/api/tags", self.ollama_host.trim_end_matches('/'));
        let resp = self
            .http
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|error| -> Failure {
                format!(
                    "no Ollama server answered at {}: {error}. \
                     Start one, or choose a hosted lane.",
                    self.ollama_host
                )
                .into()
            })?;
        if !resp.status().is_success() {
            return Err(format!("{url} refused the model list: {}", resp.status()).into());
        }
        let body: serde_json::Value = resp.json().await?;
        let mut models: Vec<(String, String)> = body
            .get("models")
            .and_then(|v| v.as_array())
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| {
                        let name = m.get("name").and_then(|v| v.as_str())?;
                        let modified = m
                            .get("modified_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        Some((name.to_string(), modified.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        models.sort_by(|left, right| right.1.cmp(&left.1));
        Ok(models.into_iter().map(|(name, _)| name).collect())
    }

    /// The installed model a name means.
    ///
    /// Exact first, so a full name is never reinterpreted; then the family
    /// prefix, because a reader who pulled `qwen3.8:27b-mtp-q8_0` says
    /// `qwen3.8` and sending that unresolved earns `model not found` from a
    /// server that has the model. An empty name takes the most recently
    /// modified, which is the one they were last working with.
    async fn resolve_local_model(&self, wanted: &str) -> Result<String, Failure> {
        let installed = self.installed_local_models().await?;
        if installed.is_empty() {
            return Err(format!(
                "the Ollama server at {} has no models installed. \
                 Pull one with `ollama pull <model>`.",
                self.ollama_host
            )
            .into());
        }
        if wanted.is_empty() {
            return Ok(installed[0].clone());
        }
        if installed.iter().any(|m| m == wanted) {
            return Ok(wanted.to_string());
        }
        if let Some(family) = installed
            .iter()
            .find(|m| m.starts_with(&format!("{wanted}:")))
        {
            return Ok(family.clone());
        }
        Err(format!(
            "the Ollama server at {} does not have '{wanted}'. It has: {}.",
            self.ollama_host,
            installed.join(", ")
        )
        .into())
    }

    async fn run_local_turn<F>(
        &mut self,
        tool_defs: &[ToolDefinition],
        mut chunk_callback: F,
    ) -> Result<String, Failure>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let Lane::Local(wanted) = self.lane.clone() else {
            return Err("run_local_turn was called off the local lane".into());
        };
        let model = self.resolve_local_model(&wanted).await?;
        self.last_model = Some(format!("ollama:{model}"));
        // No grant is minted here and none is invented: the model is on this
        // machine, so there is nothing for the server to authorise.
        self.last_grant = None;

        let url = format!("{}/api/chat", self.ollama_host.trim_end_matches('/'));
        let mut final_answer = String::new();

        for _ in 0..MAX_TOOL_STEPS {
            let req_body = serde_json::json!({
                "model": model,
                "messages": self.messages.iter().map(ollama_message).collect::<Vec<_>>(),
                "tools": tool_defs.iter().map(|t| serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })).collect::<Vec<_>>(),
                "stream": true
            });

            let resp = self.http.post(&url).json(&req_body).send().await;
            let resp = match resp {
                Ok(r) if r.status().is_success() => r,
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    return Err(
                        format!("{url} refused the turn: {status} {}", snippet(&body)).into(),
                    );
                }
                Err(error) => return Err(format!("{url} could not be reached: {error}").into()),
            };

            // Ollama streams newline-delimited JSON rather than server-sent
            // events, so the frames are split here rather than by `Eventsource`.
            let mut bytes = resp.bytes_stream();
            let mut pending = String::new();
            let mut step = StepAccumulator::default();

            while let Some(chunk) = bytes.next().await {
                let chunk = chunk.map_err(|error| -> Failure {
                    format!("the reply from {url} stopped mid-stream: {error}").into()
                })?;
                pending.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(newline) = pending.find('\n') {
                    let line: String = pending.drain(..=newline).collect();
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                        step.absorb_ollama(&json, &mut chunk_callback);
                    }
                }
            }
            let tail = pending.trim();
            if !tail.is_empty() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(tail) {
                    step.absorb_ollama(&json, &mut chunk_callback);
                }
            }

            self.last_usage.add(step.usage);
            self.last_reasoning.push_str(&step.reasoning);

            if step.tool_calls.is_empty() {
                final_answer = step.content;
                break;
            }
            self.run_tools(step).await;
        }

        Ok(final_answer)
    }
}

/// A thread left open holds its grant's remaining budget, and the interactive
/// session has no place to await a revocation on its way out. This is the
/// backstop: best effort, on whatever runtime is still up. `close` is the path
/// that can be awaited and proven, and it clears the id so this does not fire
/// twice.
impl Drop for CoderRuntimeSession {
    fn drop(&mut self) {
        let Some(thread_id) = self.thread_id.take() else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let url = format!("{}/threads/{thread_id}", self.api_base);
        let token = self.user_token.clone();
        let http = self.http.clone();
        handle.spawn(async move {
            let mut request = http.delete(&url).timeout(Duration::from_secs(10));
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            let _ = request.send().await;
        });
    }
}

/// What one call to a model produced, before the caller decides what it means.
#[derive(Default)]
struct StepAccumulator {
    content: String,
    reasoning: String,
    usage: TurnUsage,
    /// Keyed by the wire's `index` so fragments land in call order.
    tool_calls: std::collections::BTreeMap<usize, (String, String, String)>,
}

impl StepAccumulator {
    /// One OpenAI-shaped streaming chunk.
    ///
    /// `content`, `reasoning` and `tool_calls` interleave inside one `delta`,
    /// and `usage` arrives on a chunk of its own with an empty `choices` array
    /// — which is why usage is read before the choices are.
    fn absorb_openai<F: FnMut(&str)>(&mut self, json: &serde_json::Value, on_chunk: &mut F) {
        if let Some(usage) = json.get("usage") {
            self.usage.add(TurnUsage {
                prompt_tokens: field(usage, "prompt_tokens"),
                completion_tokens: field(usage, "completion_tokens"),
                total_tokens: field(usage, "total_tokens"),
            });
        }

        let Some(delta) = json
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))
        else {
            return;
        };

        if let Some(reasoning) = delta.get("reasoning").and_then(|v| v.as_str()) {
            self.reasoning.push_str(reasoning);
        }
        if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
            if !content.is_empty() {
                on_chunk(content);
                self.content.push_str(content);
            }
        }
        if let Some(calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
            for call in calls {
                let index = call.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let entry = self.tool_calls.entry(index).or_default();
                if let Some(id) = call.get("id").and_then(|v| v.as_str()) {
                    entry.0.push_str(id);
                }
                if let Some(function) = call.get("function") {
                    if let Some(name) = function.get("name").and_then(|v| v.as_str()) {
                        entry.1.push_str(name);
                    }
                    if let Some(args) = function.get("arguments").and_then(|v| v.as_str()) {
                        entry.2.push_str(args);
                    }
                }
            }
        }
    }

    /// One Ollama-shaped streaming line.
    ///
    /// The counts arrive on the `done` line, the model's scratch work under
    /// `thinking`, and a tool call whole rather than in fragments — with its
    /// arguments as an object, where the proxy sends a string.
    fn absorb_ollama<F: FnMut(&str)>(&mut self, json: &serde_json::Value, on_chunk: &mut F) {
        if json.get("done").and_then(|v| v.as_bool()) == Some(true) {
            self.usage.add(TurnUsage {
                prompt_tokens: field(json, "prompt_eval_count"),
                completion_tokens: field(json, "eval_count"),
                total_tokens: field(json, "prompt_eval_count") + field(json, "eval_count"),
            });
        }

        let Some(message) = json.get("message") else {
            return;
        };
        if let Some(thinking) = message.get("thinking").and_then(|v| v.as_str()) {
            self.reasoning.push_str(thinking);
        }
        if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
            if !content.is_empty() {
                on_chunk(content);
                self.content.push_str(content);
            }
        }
        if let Some(calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
            for call in calls {
                let Some(function) = call.get("function") else {
                    continue;
                };
                let index = self.tool_calls.len();
                let name = function
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let arguments = match function.get("arguments") {
                    Some(serde_json::Value::String(raw)) => raw.clone(),
                    Some(value) => value.to_string(),
                    None => "{}".to_string(),
                };
                // Ollama mints no call id and the tool result has to name the
                // call it answers, so one is made from the position. This is a
                // local correlation key, not a server-issued identifier.
                self.tool_calls
                    .insert(index, (format!("local_{index}_{name}"), name, arguments));
            }
        }
    }
}

fn field(value: &serde_json::Value, key: &str) -> u64 {
    value.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

/// One message in the shape Ollama's chat API takes back.
///
/// The differences from the proxy's shape are small and all load-bearing:
/// `arguments` is an object rather than a string, and a tool result is named by
/// `tool_name` rather than by a call id Ollama never issued.
fn ollama_message(message: &ChatMessage) -> serde_json::Value {
    let mut out = serde_json::json!({
        "role": message.role,
        "content": message.content.clone().unwrap_or_default(),
    });
    if let Some(calls) = &message.tool_calls {
        out["tool_calls"] = serde_json::Value::Array(
            calls
                .iter()
                .map(|call| {
                    let function = call
                        .get("function")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));
                    let name = function
                        .get("name")
                        .cloned()
                        .unwrap_or(serde_json::json!(""));
                    let arguments = match function.get("arguments") {
                        Some(serde_json::Value::String(raw)) => {
                            serde_json::from_str(raw).unwrap_or(serde_json::json!({}))
                        }
                        Some(value) => value.clone(),
                        None => serde_json::json!({}),
                    };
                    serde_json::json!({ "function": { "name": name, "arguments": arguments } })
                })
                .collect(),
        );
    }
    if message.role == "tool" {
        if let Some(id) = &message.tool_call_id {
            // `local_<index>_<name>` — the name is what Ollama matches on.
            let name = id.splitn(3, '_').nth(2).unwrap_or(id);
            out["tool_name"] = serde_json::json!(name);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_names_map_onto_catalog_ids() {
        assert_eq!(Lane::from_str("flash").model_id(), Some("gemini-3.7-flash"));
        assert_eq!(Lane::from_str("pro").model_id(), Some("gpt-5.6-luna"));
        assert_eq!(Lane::from_str("ox-alpha").model_id(), Some("ox-alpha"));
        assert_eq!(Lane::from_str("auto").model_id(), None);
    }

    /// The invented ids are gone. Every id this file can send is one the
    /// deployment's catalog listed: `gemini-3.7-flash`, `ox-alpha`,
    /// `gpt-5.6-luna`. `gemini-3.7-pro`, `claude-3-7-sonnet` and
    /// `codex-preview` were never served by anything.
    #[test]
    fn no_lane_sends_a_model_id_that_was_made_up() {
        const SERVED: [&str; 3] = ["gemini-3.7-flash", "ox-alpha", "gpt-5.6-luna"];
        for (name, _) in TIERS {
            let lane = Lane::from_str(name);
            let id = lane.model_id().expect("a tier pins a model");
            assert!(
                SERVED.contains(&id),
                "tier '{name}' opens on '{id}', which the catalog did not list"
            );
        }
    }

    #[test]
    fn an_unknown_name_is_carried_as_named_rather_than_becoming_the_default() {
        assert_eq!(Lane::from_str("bogus"), Lane::Named("bogus".to_string()));
        assert_eq!(Lane::from_str("claude"), Lane::Named("claude".to_string()));
        assert_ne!(Lane::from_str("bogus"), Lane::OxAlpha);
    }

    #[test]
    fn the_local_lane_is_parsed_from_both_spellings() {
        assert_eq!(Lane::from_str("local"), Lane::Local(String::new()));
        assert_eq!(
            Lane::from_str("ollama:qwen3:0.6b"),
            Lane::Local("qwen3:0.6b".to_string())
        );
        assert!(Lane::from_str("ollama:qwen3:0.6b").is_local());
        assert!(!Lane::from_str("flash").is_local());
        // The local lane names no catalog model, because no grant carries it.
        assert_eq!(Lane::from_str("ollama:qwen3").model_id(), None);
    }

    #[test]
    fn every_tier_has_a_name_and_a_label() {
        assert_eq!(Lane::from_str("auto").tier(), Some("auto"));
        assert_eq!(Lane::from_str("flash").tier(), Some("flash"));
        assert_eq!(Lane::from_str("pro").tier(), Some("pro"));
        assert_eq!(Lane::from_str("local").tier(), Some("local"));
        assert_eq!(Lane::from_str("bogus").tier(), None);
        assert_eq!(Lane::from_str("flash").label(), "Coder Flash");
        assert_eq!(
            Lane::from_str("ollama:qwen3:0.6b").label(),
            "Coder Local (qwen3:0.6b)"
        );
    }

    #[test]
    fn the_admitted_lane_sentence_names_every_way_in() {
        let sentence = admitted_lanes();
        for fragment in ["auto", "flash", "pro", "ox-alpha", "ollama:<model>"] {
            assert!(
                sentence.contains(fragment),
                "'{fragment}' missing from: {sentence}"
            );
        }
    }

    #[test]
    fn usage_sums_across_the_steps_of_a_turn() {
        let mut usage = TurnUsage::default();
        assert!(!usage.reported());
        usage.add(TurnUsage {
            prompt_tokens: 99,
            completion_tokens: 17,
            total_tokens: 116,
        });
        usage.add(TurnUsage {
            prompt_tokens: 140,
            completion_tokens: 8,
            total_tokens: 148,
        });
        assert!(usage.reported());
        assert_eq!(usage.line(), "239 prompt + 25 completion = 264 tokens");
    }

    /// The exact frames a live `ox-alpha` turn sent, in order: reasoning
    /// first, then the answer, then usage on a chunk with no choices at all.
    #[test]
    fn a_real_proxy_stream_yields_the_answer_the_reasoning_and_the_counts() {
        let frames = [
            r#"{"choices":[{"delta":{"reasoning":"The user wants "},"index":0}],"model":"ox-alpha"}"#,
            r#"{"choices":[{"delta":{"reasoning":"PONG."},"index":0}],"model":"ox-alpha"}"#,
            r#"{"choices":[{"delta":{"content":"PONG"},"index":0}],"model":"ox-alpha"}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop","index":0}],"model":"ox-alpha"}"#,
            r#"{"choices":[],"model":"ox-alpha","usage":{"completion_tokens":17,"prompt_tokens":99,"total_tokens":116}}"#,
        ];
        let mut step = StepAccumulator::default();
        let mut streamed = String::new();
        {
            let mut sink = |chunk: &str| streamed.push_str(chunk);
            for frame in frames {
                step.absorb_openai(&serde_json::from_str(frame).unwrap(), &mut sink);
            }
        }

        assert_eq!(step.content, "PONG");
        // Reasoning is parsed and kept, and stays off the transcript.
        assert_eq!(step.reasoning, "The user wants PONG.");
        assert_eq!(streamed, "PONG", "reasoning leaked into the answer");
        assert_eq!(step.usage.total_tokens, 116);
        assert_eq!(step.usage.prompt_tokens, 99);
        assert_eq!(step.usage.completion_tokens, 17);
    }

    #[test]
    fn tool_call_fragments_are_joined_by_their_index() {
        let frames = [
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"read_","arguments":"{\"path\":"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"file","arguments":"\"a.txt\"}"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_b","function":{"name":"ls","arguments":"{}"}}]}}]}"#,
        ];
        let mut step = StepAccumulator::default();
        let mut sink = |_: &str| {};
        for frame in frames {
            step.absorb_openai(&serde_json::from_str(frame).unwrap(), &mut sink);
        }
        let calls: Vec<_> = step.tool_calls.into_values().collect();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "call_a");
        assert_eq!(calls[0].1, "read_file");
        assert_eq!(calls[0].2, r#"{"path":"a.txt"}"#);
        assert_eq!(calls[1].1, "ls");
    }

    /// The exact frames a live `qwen3:0.6b` turn sent.
    #[test]
    fn a_real_ollama_stream_yields_the_answer_and_the_counts() {
        let frames = [
            r#"{"model":"qwen3:0.6b","message":{"role":"assistant","thinking":"short"},"done":false}"#,
            r#"{"model":"qwen3:0.6b","message":{"role":"assistant","content":"PO"},"done":false}"#,
            r#"{"model":"qwen3:0.6b","message":{"role":"assistant","content":"NG"},"done":false}"#,
            r#"{"model":"qwen3:0.6b","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop","prompt_eval_count":19,"eval_count":28}"#,
        ];
        let mut step = StepAccumulator::default();
        let mut streamed = String::new();
        {
            let mut sink = |chunk: &str| streamed.push_str(chunk);
            for frame in frames {
                step.absorb_ollama(&serde_json::from_str(frame).unwrap(), &mut sink);
            }
        }

        assert_eq!(streamed, "PONG");
        assert_eq!(step.content, "PONG");
        assert_eq!(step.reasoning, "short");
        assert_eq!(step.usage.prompt_tokens, 19);
        assert_eq!(step.usage.completion_tokens, 28);
        assert_eq!(step.usage.total_tokens, 47);
    }

    #[test]
    fn an_ollama_tool_call_arrives_whole_with_object_arguments() {
        let frame = r#"{"message":{"role":"assistant","tool_calls":[{"function":{"name":"read_file","arguments":{"path":"a.txt"}}}]},"done":false}"#;
        let mut step = StepAccumulator::default();
        let mut sink = |_: &str| {};
        step.absorb_ollama(&serde_json::from_str(frame).unwrap(), &mut sink);
        let calls: Vec<_> = step.tool_calls.into_values().collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "read_file");
        let parsed: serde_json::Value = serde_json::from_str(&calls[0].2).unwrap();
        assert_eq!(parsed["path"], "a.txt");
    }

    #[test]
    fn a_tool_result_goes_back_to_ollama_named_by_its_tool() {
        let message = ChatMessage {
            role: "tool".to_string(),
            content: Some("hello".to_string()),
            tool_calls: None,
            tool_call_id: Some("local_0_read_file".to_string()),
        };
        let wire = ollama_message(&message);
        assert_eq!(wire["role"], "tool");
        assert_eq!(wire["content"], "hello");
        assert_eq!(wire["tool_name"], "read_file");
    }

    #[test]
    fn an_assistant_tool_call_reaches_ollama_with_its_arguments_parsed() {
        let message = ChatMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![serde_json::json!({
                "id": "call_a",
                "type": "function",
                "function": { "name": "read_file", "arguments": "{\"path\":\"a.txt\"}" }
            })]),
            tool_call_id: None,
        };
        let wire = ollama_message(&message);
        assert_eq!(wire["tool_calls"][0]["function"]["name"], "read_file");
        assert_eq!(
            wire["tool_calls"][0]["function"]["arguments"]["path"], "a.txt",
            "Ollama takes arguments as an object, not as the proxy's string"
        );
    }

    /// The local lane's system prompt must not promise a metered proxy, and the
    /// thread lane's must not promise that nothing leaves the machine.
    #[test]
    fn each_lane_tells_the_model_where_it_is_running() {
        let local = CoderRuntimeSession::new(
            Lane::Local("qwen3".to_string()),
            Some("http://127.0.0.1:1/api/v1".to_string()),
            None,
            HarnessToolRegistry::new(Some(std::env::temp_dir())),
        );
        assert!(local.build_system_prompt(&[]).contains("on this machine"));

        let hosted = CoderRuntimeSession::new(
            Lane::OxAlpha,
            Some("http://127.0.0.1:1/api/v1".to_string()),
            None,
            HarnessToolRegistry::new(Some(std::env::temp_dir())),
        );
        assert!(hosted
            .build_system_prompt(&[])
            .contains("OpenAgents inference proxy"));
    }
}
