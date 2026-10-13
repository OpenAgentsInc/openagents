//! The rest of a claude turn's stream, as Coder shows it (#11438): the
//! reply's words as they arrive, the model's thinking, each subagent as its
//! own delegation, a retry while the API is busy, what failed to load when
//! Claude Code started, and how full the session's context is.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use claude_agent_sdk::{SdkApiRetryMessage, SdkStreamEvent};
use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;

/// The entry that holds a thinking block: its words are in `output.text`.
pub const THINKING: &str = atif::row::THINKING;

/// The entry that holds one short notice in the conversation: its words are
/// in `output.text`.
pub const NOTICE: &str = atif::row::NOTICE;

/// What the person reads in a [`THINKING`] or [`NOTICE`] entry.
#[must_use]
pub fn entry_text(output: &Value) -> &str {
    output["text"].as_str().unwrap_or_default()
}

/// Whether `name` is the tool Claude Code starts a subagent with.
#[must_use]
pub fn is_agent_tool(name: &str) -> bool {
    matches!(name, "Agent" | "Task")
}

/// The delegation tool name Coder's agent list reads as a run's start and
/// end.
const AGENT_RUN: &str = "acp_subagent";

/// One subagent on screen.
struct Agent {
    name: String,
    task: String,
    /// Whether any of its words reached the screen, so its end does not
    /// repeat the reply it already showed.
    spoke: bool,
}

/// What one turn's stream remembers between messages.
#[derive(Default)]
pub(crate) struct Stream {
    /// The main reply's message whose words are arriving now.
    current: Option<String>,
    /// The main reply's messages whose words already arrived one by one.
    streamed: HashSet<String>,
    /// Subagents, by the id of the call that started each.
    agents: HashMap<String, Agent>,
    /// What the newest main call read and wrote, in tokens.
    context: u64,
    /// Whether the main reply's thinking is streaming in now.
    thinking: bool,
    /// The main reply's thinking words, as they stream (#11552).
    pub(crate) thoughts: ClaudeThoughts,
}

impl Stream {
    /// The main reply's words in one partial `event`, when it carries some.
    /// A subagent's partial words are left to its whole messages.
    pub(crate) fn partial(&mut self, event: &SdkStreamEvent) -> Option<String> {
        if event.parent_tool_use_id.is_some() {
            return None;
        }
        let event = &event.event;
        match event["type"].as_str() {
            Some("message_start") => {
                self.current = event["message"]["id"].as_str().map(str::to_owned);
                None
            }
            Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
                let text = event["delta"]["text"].as_str().filter(|t| !t.is_empty())?;
                if let Some(id) = &self.current {
                    self.streamed.insert(id.clone());
                }
                Some(text.to_owned())
            }
            _ => None,
        }
    }

    /// The main reply's thinking words in one partial `event`, as they
    /// arrive. A subagent's thinking is left to its whole messages.
    pub(crate) fn thought(&mut self, event: &SdkStreamEvent) -> Vec<RuntimeEvent> {
        if event.parent_tool_use_id.is_some() {
            return Vec::new();
        }
        self.thoughts.event(&event.event)
    }

    /// Whether the main reply's thinking started or stopped streaming with
    /// `event`, when that changed: the input's rail says `Thinking` while
    /// it streams.
    pub(crate) fn thinking(&mut self, event: &SdkStreamEvent) -> Option<bool> {
        if event.parent_tool_use_id.is_some() {
            return None;
        }
        let event = &event.event;
        let now = match event["type"].as_str() {
            Some("content_block_start") => event["content_block"]["type"] == "thinking",
            Some("content_block_delta") => event["delta"]["type"] == "thinking_delta",
            Some("content_block_stop" | "message_stop" | "message_delta") => false,
            _ => return None,
        };
        (now != self.thinking).then(|| {
            self.thinking = now;
            now
        })
    }

    /// Whether the main reply's `message` already showed its words one by
    /// one, so its whole text must not show again.
    pub(crate) fn already_streamed(&self, message: &Value) -> bool {
        message["id"]
            .as_str()
            .is_some_and(|id| self.streamed.contains(id))
    }

    /// Remember the context the main reply's `message` reports.
    pub(crate) fn note_usage(&mut self, message: &Value) {
        let usage = &message["usage"];
        let read = [
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
        ]
        .iter()
        .filter_map(|field| usage[*field].as_u64())
        .sum::<u64>();
        if read > 0 {
            self.context = read.saturating_add(usage["output_tokens"].as_u64().unwrap_or(0));
        }
    }

    /// The delegation a subagent call `id` with `input` opens.
    pub(crate) fn start_agent(&mut self, id: &str, input: &Value) -> RuntimeEvent {
        let name = input["subagent_type"]
            .as_str()
            .filter(|name| !name.is_empty())
            .unwrap_or("Claude")
            .to_owned();
        let task = input["description"]
            .as_str()
            .or_else(|| input["prompt"].as_str())
            .unwrap_or_default()
            .to_owned();
        self.agents.insert(
            id.to_owned(),
            Agent {
                name,
                task,
                spoke: false,
            },
        );
        self.under(
            Some(id),
            RuntimeEvent::Tool {
                name: AGENT_RUN.into(),
                input: Value::Null,
                output: Value::Null,
                running: true,
            },
        )
    }

    /// The delegation's end, when `id` is a subagent call: its reply shows
    /// only when none of its words did.
    pub(crate) fn end_agent(
        &mut self,
        id: &str,
        reply: &str,
        failed: bool,
        result: Option<&Value>,
    ) -> Option<RuntimeEvent> {
        let spoke = self.agents.get(id)?.spoke;
        // A background subagent's call returns as soon as it starts; it
        // keeps running until Claude Code reports it settled (#11482).
        if !failed && result.is_some_and(|result| result["status"] == "async_launched") {
            return Some(self.under(
                Some(id),
                RuntimeEvent::Tool {
                    name: AGENT_RUN.into(),
                    input: Value::Null,
                    output: Value::Null,
                    running: true,
                },
            ));
        }
        let mut output = if failed {
            json!({"error": reply})
        } else if spoke || reply.trim().is_empty() {
            json!({})
        } else {
            json!({"reply": reply})
        };
        // What the subagent used, as Claude Code reports it in the call's
        // result, for the agent list's token count.
        if let Some(tokens) = result.and_then(subagent_tokens) {
            output["tokens"] = json!(tokens);
        }
        Some(self.under(
            Some(id),
            RuntimeEvent::Tool {
                name: AGENT_RUN.into(),
                input: Value::Null,
                output,
                running: false,
            },
        ))
    }

    /// A subagent's tokens so far, from Claude Code's progress report on
    /// the call `id` that started it (#11482).
    pub(crate) fn agent_progress(&mut self, id: &str, tokens: u64) -> RuntimeEvent {
        self.under(Some(id), RuntimeEvent::Tokens(tokens))
    }

    /// A subagent settled, as Claude Code reports it for the call `id` that
    /// started it: done, or stopped or failed with its `summary` (#11482).
    pub(crate) fn agent_settled(
        &mut self,
        id: &str,
        status: &str,
        summary: &str,
        tokens: u64,
    ) -> RuntimeEvent {
        let mut output = if status == "completed" {
            json!({})
        } else if summary.trim().is_empty() {
            json!({"error": "The agent stopped before it finished."})
        } else {
            json!({"error": summary})
        };
        if tokens > 0 {
            output["tokens"] = json!(tokens);
        }
        self.under(
            Some(id),
            RuntimeEvent::Tool {
                name: AGENT_RUN.into(),
                input: Value::Null,
                output,
                running: false,
            },
        )
    }

    /// `event` as it reaches the screen: in the subagent `parent`'s own
    /// conversation when it came from one, in the main one otherwise.
    pub(crate) fn under(&mut self, parent: Option<&str>, event: RuntimeEvent) -> RuntimeEvent {
        let Some(parent) = parent else {
            return event;
        };
        let agent = self
            .agents
            .entry(parent.to_owned())
            .or_insert_with(|| Agent {
                name: "Claude".into(),
                task: String::new(),
                spoke: false,
            });
        agent.spoke |= matches!(event, RuntimeEvent::Text(_));
        RuntimeEvent::Delegation {
            id: parent.to_owned(),
            name: agent.name.clone(),
            task: agent.task.clone(),
            event: Box::new(event),
        }
    }

    /// Record how full `session`'s context is, from the newest main call
    /// and the window the turn's `result` reports for `model`.
    pub(crate) fn record_context(
        &self,
        session: Option<&str>,
        model: &str,
        usage: &HashMap<String, claude_agent_sdk::ModelUsage>,
    ) {
        let window = usage
            .get(model)
            .map(|usage| usage.context_window)
            .filter(|window| *window > 0)
            .or_else(|| usage.values().map(|usage| usage.context_window).max());
        if let (Some(session), Some(window)) = (session, window)
            && self.context > 0
            && window > 0
        {
            *lock(&CONTEXT) = Some((session.to_owned(), self.context, window));
        }
    }
}

/// What a turn says once when the model thought but its provider showed
/// none of the words.
pub const HIDDEN_THINKING: &str = "Thinking (hidden by the model)";

/// The model's thinking as it streams in pieces, on any harness (Codex's
/// reasoning summary, an ACP agent's thought chunks, Claude's thinking
/// deltas): each piece shows at once in a running thinking entry, the
/// rail says `Thinking` meanwhile, and the end closes the entry with the
/// whole text, which syncs and exports as the model's reasoning.
#[derive(Default)]
pub(crate) struct Thought {
    text: String,
    open: bool,
    /// The turn already said its thinking was hidden.
    hidden_said: bool,
}

impl Thought {
    /// What one streamed `piece` shows.
    pub(crate) fn piece(&mut self, piece: &str) -> Vec<RuntimeEvent> {
        if piece.is_empty() {
            return Vec::new();
        }
        let mut shown = Vec::new();
        if !self.open {
            self.open = true;
            shown.push(RuntimeEvent::Thinking(true));
        }
        self.text.push_str(piece);
        shown.push(RuntimeEvent::ThinkingText(piece.to_owned()));
        shown
    }

    /// Whether a thinking entry is streaming now.
    pub(crate) fn open(&self) -> bool {
        self.open
    }

    /// The streaming thinking ended: its entry closes with the whole text
    /// (`whole`, when the harness sent it, else what streamed).
    pub(crate) fn end(&mut self, whole: Option<&str>) -> Vec<RuntimeEvent> {
        if !self.open {
            return Vec::new();
        }
        self.open = false;
        let streamed = std::mem::take(&mut self.text);
        let text = whole.unwrap_or(&streamed).trim();
        let mut shown = Vec::new();
        if !text.is_empty() {
            shown.push(notice_entry(THINKING, text));
        }
        shown.push(RuntimeEvent::Thinking(false));
        shown
    }

    /// The model thought but showed no words: a quiet line, once a turn.
    pub(crate) fn hidden(&mut self) -> Option<RuntimeEvent> {
        (!std::mem::replace(&mut self.hidden_said, true))
            .then(|| notice_entry(NOTICE, HIDDEN_THINKING))
    }
}

/// Claude's thinking from stream-json with partial messages (#11552): each
/// `thinking_delta` streams into the running thinking entry; the finished
/// block (the `assistant` message, which Claude Code sends before the
/// block's `content_block_stop`) closes it, and a block that already
/// showed is not shown again. The rail's `Thinking` is left to its own
/// reader.
#[derive(Default)]
pub(crate) struct ClaudeThoughts {
    thought: Thought,
    /// Blocks that closed before their whole message arrived.
    ended: Vec<String>,
}

impl ClaudeThoughts {
    /// What one stream `event` (the API's, inside `stream_event`) shows.
    pub(crate) fn event(&mut self, event: &Value) -> Vec<RuntimeEvent> {
        let shown = match event["type"].as_str() {
            Some("content_block_delta") if event["delta"]["type"] == "thinking_delta" => self
                .thought
                .piece(event["delta"]["thinking"].as_str().unwrap_or_default()),
            Some("content_block_stop" | "message_stop") if self.thought.open() => {
                let shown = self.thought.end(None);
                if let Some(RuntimeEvent::Tool { output, .. }) = shown.first() {
                    self.ended.push(entry_text(output).to_owned());
                }
                shown
            }
            _ => Vec::new(),
        };
        shown
            .into_iter()
            .filter(|event| !matches!(event, RuntimeEvent::Thinking(_)))
            .collect()
    }

    /// A finished thinking `block`: the entry that closes the streamed one,
    /// or the block's own entry when none streamed, or nothing when it
    /// already showed.
    pub(crate) fn block(&mut self, block: &Value) -> Option<RuntimeEvent> {
        let text = block["thinking"].as_str().unwrap_or_default().trim();
        if self.thought.open() {
            return self
                .thought
                .end(Some(text))
                .into_iter()
                .find(|event| matches!(event, RuntimeEvent::Tool { .. }));
        }
        if let Some(index) = self.ended.iter().position(|ended| ended == text) {
            self.ended.remove(index);
            return None;
        }
        thinking(block)
    }

    /// The call ended: thinking still streaming closes with what it said.
    pub(crate) fn end(&mut self) -> Vec<RuntimeEvent> {
        self.ended.clear();
        self.thought
            .end(None)
            .into_iter()
            .filter(|event| !matches!(event, RuntimeEvent::Thinking(_)))
            .collect()
    }
}

/// A thinking `block` as its entry, when it carries words.
#[must_use]
pub fn thinking(block: &Value) -> Option<RuntimeEvent> {
    let text = block["thinking"].as_str()?.trim();
    (!text.is_empty()).then(|| notice_entry(THINKING, text))
}

/// The notice an API retry shows.
#[must_use]
pub fn retry(retry: &SdkApiRetryMessage) -> RuntimeEvent {
    let seconds = retry.retry_delay_ms.div_ceil(1000);
    let reason = match retry.error_status {
        Some(429) => "the API is limiting requests".to_owned(),
        Some(529) => "the API is overloaded".to_owned(),
        Some(status) => format!("the API answered {status}"),
        None => "the API did not answer".to_owned(),
    };
    notice_entry(
        NOTICE,
        &format!(
            "Retrying in {seconds}s because {reason} (try {} of {}).",
            retry.attempt, retry.max_retries
        ),
    )
}

/// How long the input's rail shows what failed to load.
pub const LOAD_WARNING_FOR: std::time::Duration = std::time::Duration::from_secs(5);

/// One thing that failed to load when a session started.
#[derive(Clone, Debug, PartialEq, Eq)]
enum LoadProblem {
    SignIn(String),
    Failed(String),
    Plugin(Option<String>),
}

/// Notes what failed to load when a session started, so the input's rail
/// flashes it for a few seconds beside `claude` rather than the transcript
/// keeping a row for it.
pub fn note_load_problems(init: &claude_agent_sdk::protocol::SystemInit) {
    if let Some(summary) = load_problems(init) {
        *lock(&WARNING) = Some((summary, std::time::Instant::now()));
    }
}

/// What the rail flashes at `now`: the newest load problems, for
/// [`LOAD_WARNING_FOR`] after they arrived.
#[must_use]
pub fn load_warning(now: std::time::Instant) -> Option<String> {
    lock(&WARNING)
        .as_ref()
        .filter(|(_, at)| now.saturating_duration_since(*at) < LOAD_WARNING_FOR)
        .map(|(summary, _)| summary.clone())
}

/// A short summary of what failed to load when `session` started: an MCP
/// server that needs sign-in or did not connect, or a plugin that did not
/// load. Each counts once a session, though Claude Code reports them again
/// each turn, so a turn with nothing new has no summary.
#[must_use]
pub fn load_problems(init: &claude_agent_sdk::protocol::SystemInit) -> Option<String> {
    let mut problems = Vec::new();
    for server in &init.mcp_servers {
        match server.status.as_str() {
            "failed" => problems.push(LoadProblem::Failed(server.name.clone())),
            "needs-auth" => problems.push(LoadProblem::SignIn(server.name.clone())),
            _ => {}
        }
    }
    for error in init.plugin_errors.iter().flatten() {
        let name = ["plugin", "name", "source"]
            .iter()
            .find_map(|field| error[*field].as_str());
        problems.push(LoadProblem::Plugin(name.map(str::to_owned)));
    }
    let mut shown = lock(&SHOWN);
    let fresh: Vec<LoadProblem> = problems
        .into_iter()
        .filter(|problem| {
            let seen = (init.session_id.clone(), format!("{problem:?}"));
            let new = !shown.contains(&seen);
            if new {
                shown.push(seen);
            }
            new
        })
        .collect();
    summarize(&fresh)
}

/// A few words for `problems`: the one problem by name, or how many of
/// each kind when there are several.
fn summarize(problems: &[LoadProblem]) -> Option<String> {
    match problems {
        [] => None,
        [LoadProblem::SignIn(name)] => Some(format!("{name} needs sign-in")),
        [LoadProblem::Failed(name)] => Some(format!("{name} didn't connect")),
        [LoadProblem::Plugin(Some(name))] => Some(format!("plugin {name} didn't load")),
        [LoadProblem::Plugin(None)] => Some("a plugin didn't load".to_owned()),
        _ => {
            let count =
                |kind: fn(&LoadProblem) -> bool| problems.iter().filter(|p| kind(p)).count();
            let sign_in = count(|p| matches!(p, LoadProblem::SignIn(_)));
            let failed = count(|p| matches!(p, LoadProblem::Failed(_)));
            let plugins = count(|p| matches!(p, LoadProblem::Plugin(_)));
            let mut parts = Vec::new();
            match sign_in {
                0 => {}
                1 => parts.push("1 MCP server needs sign-in".to_owned()),
                n => parts.push(format!("{n} MCP servers need sign-in")),
            }
            match failed {
                0 => {}
                1 => parts.push("1 MCP server didn't connect".to_owned()),
                n => parts.push(format!("{n} MCP servers didn't connect")),
            }
            match plugins {
                0 => {}
                1 => parts.push("1 plugin didn't load".to_owned()),
                n => parts.push(format!("{n} plugins didn't load")),
            }
            Some(parts.join(", "))
        }
    }
}

/// A Claude Code subagent's tokens from its call's result: `totalTokens`,
/// or the sum of its `usage`.
fn subagent_tokens(result: &Value) -> Option<u64> {
    if let Some(total) = result["totalTokens"].as_u64() {
        return Some(total);
    }
    let usage = &result["usage"];
    let sum: u64 = [
        "input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
        "output_tokens",
    ]
    .iter()
    .filter_map(|field| usage[*field].as_u64())
    .sum();
    (sum > 0).then_some(sum)
}

fn notice_entry(name: &str, text: &str) -> RuntimeEvent {
    RuntimeEvent::Tool {
        name: name.into(),
        input: Value::Null,
        output: json!({"text": text}),
        running: false,
    }
}

/// Load problems already shown, by session.
static SHOWN: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// The newest load problems' summary and when they arrived.
static WARNING: Mutex<Option<(String, std::time::Instant)>> = Mutex::new(None);

/// The newest session's context: its id, the tokens it holds, its window.
static CONTEXT: Mutex<Option<(String, u64, u64)>> = Mutex::new(None);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// "Context 42%", for the chat holding `session`, once a turn of it
/// reported its size.
#[must_use]
pub fn context_line(session: &str) -> Option<String> {
    let guard = lock(&CONTEXT);
    let (id, used, window) = guard.as_ref()?;
    (id == session).then(|| {
        let percent = (*used as f64 / *window as f64 * 100.0).clamp(0.0, 100.0);
        format!("Context {percent:.0}%")
    })
}

/// Claude Code's model calls in one turn, for the export's `model_calls`
/// (owner, 2026-10-13), as Codex and OpenCode report theirs: each API
/// message of the main reply is one call, named by its model, with its
/// usage and how long it took since the turn started or the tools before
/// it returned.
#[derive(Default)]
pub(crate) struct ModelCalls {
    id: Option<String>,
    model: String,
    usage: Value,
    started: Option<std::time::Instant>,
    first_token_ms: Option<u64>,
}

impl ModelCalls {
    /// The turn started at `now`, unless a call already set its start.
    pub(crate) fn mark(&mut self, now: std::time::Instant) {
        if self.started.is_none() {
            self.started = Some(now);
        }
    }

    /// An assistant `message` of the main reply, at `now`: the call it
    /// belongs to, and the call before it when that one ended.
    pub(crate) fn see(
        &mut self,
        message: &Value,
        now: std::time::Instant,
    ) -> Option<crate::bundled_runtime::RoundUsage> {
        let id = message["id"].as_str().filter(|id| !id.is_empty())?;
        let ended = (self.id.as_deref() != Some(id))
            .then(|| self.finish(now))
            .flatten();
        if self.id.is_none() {
            self.id = Some(id.to_owned());
            let started = *self.started.get_or_insert(now);
            self.first_token_ms = Some(now.duration_since(started).as_millis() as u64);
        }
        if let Some(model) = message["model"].as_str().filter(|model| !model.is_empty()) {
            model.clone_into(&mut self.model);
        }
        if message["usage"].is_object() {
            self.usage = message["usage"].clone();
        }
        ended
    }

    /// The call running now, ended at `now`.
    pub(crate) fn finish(
        &mut self,
        now: std::time::Instant,
    ) -> Option<crate::bundled_runtime::RoundUsage> {
        self.id.take()?;
        let started = self.started.take().unwrap_or(now);
        let usage = std::mem::take(&mut self.usage);
        let field = |name: &str| usage[name].as_u64();
        let cached = field("cache_read_input_tokens");
        let written = field("cache_creation_input_tokens");
        self.started = Some(now);
        Some(crate::bundled_runtime::RoundUsage {
            model: self.model.clone(),
            prompt_tokens: field("input_tokens").unwrap_or(0)
                + cached.unwrap_or(0)
                + written.unwrap_or(0),
            completion_tokens: field("output_tokens").unwrap_or(0),
            reasoning_tokens: None,
            cost: None,
            milliseconds: (now.duration_since(started).as_millis() as u64).max(1),
            first_token_ms: self.first_token_ms.take(),
            cached_tokens: cached,
            cache_write_tokens: written,
        })
    }
}

/// What a recorded stream's events leave in a transcript, as the terminal
/// applies them, and the thinking pieces that arrived on the way.
#[cfg(test)]
pub(crate) fn shown_in_chat(events: &[RuntimeEvent]) -> (crate::live::Chat, Vec<String>) {
    let mut chat = crate::live::Chat::default();
    let mut pieces = Vec::new();
    for event in events.iter().cloned() {
        match event {
            RuntimeEvent::Text(text) => chat.partial.push_str(&text),
            RuntimeEvent::Thinking(on) => chat.thinking = on,
            RuntimeEvent::ThinkingText(text) => {
                chat.think(&text);
                pieces.push(text);
            }
            RuntimeEvent::Tool {
                name,
                input,
                output,
                running,
            } => chat.tool(name, input, output, running),
            _ => {}
        }
    }
    chat.finish_partial();
    (chat, pieces)
}

/// The thinking entries a transcript holds: their words, and whether each
/// still streams.
#[cfg(test)]
pub(crate) fn thinking_entries(chat: &crate::live::Chat) -> Vec<(String, bool)> {
    chat.entries
        .iter()
        .filter_map(|entry| match entry {
            crate::live::Entry::Tool {
                name,
                output,
                running,
                ..
            } if name == THINKING => Some((entry_text(output).to_owned(), *running)),
            _ => None,
        })
        .collect()
}

/// A recorded stream under `fixtures/thinking/`, one JSON value a line.
#[cfg(test)]
pub(crate) fn recorded(name: &str) -> Vec<Value> {
    let path = format!("{}/fixtures/thinking/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    /// Each API message of Claude Code's reply is one model call with its
    /// model, usage and a non-zero duration (owner's gate, 2026-10-13).
    #[test]
    fn each_api_message_is_one_model_call() {
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let mut calls = super::ModelCalls::default();
        calls.mark(start);
        let first = serde_json::json!({"id": "msg_1", "model": "claude-haiku-5-5",
            "usage": {"input_tokens": 10, "cache_read_input_tokens": 100, "output_tokens": 5}});
        assert!(
            calls
                .see(&first, start + Duration::from_millis(300))
                .is_none()
        );
        // The same message again (another content block) is the same call.
        assert!(
            calls
                .see(&first, start + Duration::from_millis(400))
                .is_none()
        );
        let second = serde_json::json!({"id": "msg_2", "model": "claude-haiku-5-5",
            "usage": {"input_tokens": 3, "output_tokens": 7}});
        let ended = calls
            .see(&second, start + Duration::from_millis(900))
            .expect("the first call ended");
        assert_eq!(ended.model, "claude-haiku-5-5");
        assert_eq!(ended.prompt_tokens, 110);
        assert_eq!(ended.completion_tokens, 5);
        assert_eq!(ended.cached_tokens, Some(100));
        assert_eq!(ended.milliseconds, 900);
        assert_eq!(ended.first_token_ms, Some(300));
        let last = calls
            .finish(start + Duration::from_millis(1500))
            .expect("the second call");
        assert_eq!(last.completion_tokens, 7);
        assert!(last.milliseconds > 0);
        assert!(calls.finish(start + Duration::from_secs(2)).is_none());
    }

    use super::*;

    /// #11552: Claude Code's thinking (stream-json, partial messages, as
    /// `claude` 2.1.296 printed it on 2026-10-13 for "Is 9991 prime?")
    /// streams into one thinking entry piece by piece; the finished block
    /// closes it with the whole text, and nothing shows twice.
    #[test]
    fn claude_codes_thinking_streams_into_one_entry() {
        let mut stream = Stream::default();
        let mut events = Vec::new();
        let mut open_while_streaming = false;
        for line in recorded("claude-code-stream.jsonl") {
            match line["type"].as_str() {
                Some("stream_event") => {
                    let event = event(line["event"].clone());
                    events.extend(stream.thinking(&event).map(RuntimeEvent::Thinking));
                    events.extend(stream.thought(&event));
                    if let Some(text) = stream.partial(&event) {
                        events.push(RuntimeEvent::Text(text));
                    }
                }
                Some("assistant") => {
                    for block in line["message"]["content"].as_array().into_iter().flatten() {
                        if block["type"] == "thinking" {
                            events.extend(stream.thoughts.block(block));
                        }
                    }
                }
                _ => {}
            }
            let (chat, _) = shown_in_chat(&events);
            open_while_streaming |= thinking_entries(&chat).iter().any(|(_, running)| *running);
        }
        events.extend(stream.thoughts.end());
        let (chat, pieces) = shown_in_chat(&events);
        assert!(open_while_streaming, "the entry shows while it streams");
        assert!(pieces.len() > 5, "{pieces:?}");
        let entries = thinking_entries(&chat);
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert!(entries[0].0.starts_with("Checking 9991"), "{entries:?}");
        assert!(!entries[0].1);
        // The thinking sits above the answer.
        assert!(
            matches!(chat.entries.last(), Some(crate::live::Entry::Assistant { text, .. }) if text.starts_with("No, 9991"))
        );
        assert!(!chat.thinking);
    }

    /// A thinking block that closed before its whole message arrived is
    /// not shown again by that message.
    #[test]
    fn a_block_closed_first_is_not_repeated() {
        let mut thoughts = ClaudeThoughts::default();
        let mut events = thoughts.event(&json!({"type": "content_block_delta",
            "delta": {"type": "thinking_delta", "thinking": "Look first."}}));
        events.extend(thoughts.event(&json!({"type": "content_block_stop", "index": 0})));
        assert!(
            thoughts
                .block(&json!({"type": "thinking", "thinking": "Look first."}))
                .is_none()
        );
        let (chat, _) = shown_in_chat(&events);
        assert_eq!(thinking_entries(&chat), [("Look first.".to_owned(), false)]);
    }

    fn event(value: Value) -> SdkStreamEvent {
        SdkStreamEvent {
            event: value,
            parent_tool_use_id: None,
            uuid: String::new(),
            session_id: String::new(),
        }
    }

    #[test]
    fn streamed_words_arrive_once_and_the_whole_message_is_not_repeated() {
        let mut stream = Stream::default();
        assert_eq!(
            stream.partial(&event(
                json!({"type": "message_start", "message": {"id": "m1"}})
            )),
            None
        );
        let words = stream.partial(&event(json!({
            "type": "content_block_delta",
            "delta": {"type": "text_delta", "text": "Hello"},
        })));
        assert_eq!(words.as_deref(), Some("Hello"));
        assert!(stream.already_streamed(&json!({"id": "m1"})));
        assert!(!stream.already_streamed(&json!({"id": "m2"})));
        let mut sub = event(json!({
            "type": "content_block_delta",
            "delta": {"type": "text_delta", "text": "inside"},
        }));
        sub.parent_tool_use_id = Some("t1".into());
        assert_eq!(stream.partial(&sub), None);
    }

    #[test]
    fn thinking_starts_with_its_block_and_stops_when_it_ends() {
        let mut stream = Stream::default();
        let start = json!({"type": "content_block_start", "content_block": {"type": "thinking"}});
        assert_eq!(stream.thinking(&event(start)), Some(true));
        let delta = json!({"type": "content_block_delta", "delta": {"type": "thinking_delta"}});
        assert_eq!(stream.thinking(&event(delta)), None);
        let stop = json!({"type": "content_block_stop", "index": 0});
        assert_eq!(stream.thinking(&event(stop)), Some(false));
        let text = json!({"type": "content_block_start", "content_block": {"type": "text"}});
        assert_eq!(stream.thinking(&event(text)), None);
        let mut sub =
            event(json!({"type": "content_block_start", "content_block": {"type": "thinking"}}));
        sub.parent_tool_use_id = Some("t1".into());
        assert_eq!(stream.thinking(&sub), None);
    }

    #[test]
    fn a_background_subagent_runs_until_claude_code_says_it_settled() {
        let mut stream = Stream::default();
        stream.start_agent(
            "t1",
            &json!({"subagent_type": "Plan", "description": "sketch"}),
        );
        let launched = stream
            .end_agent(
                "t1",
                "Async agent launched successfully.",
                false,
                Some(&json!({"isAsync": true, "status": "async_launched"})),
            )
            .unwrap();
        assert!(matches!(&launched, RuntimeEvent::Delegation { event, .. }
            if matches!(event.as_ref(), RuntimeEvent::Tool { running: true, .. })));
        // In a later turn the stream is new; the id carries the agent.
        let mut later = Stream::default();
        assert!(
            matches!(later.agent_progress("t1", 14_180), RuntimeEvent::Delegation { id, event, .. }
            if id == "t1" && matches!(event.as_ref(), RuntimeEvent::Tokens(14_180)))
        );
        let done = later.agent_settled("t1", "completed", "3 files", 15_000);
        assert!(matches!(&done, RuntimeEvent::Delegation { event, .. }
            if matches!(event.as_ref(), RuntimeEvent::Tool { running: false, output, .. }
                if output["tokens"] == 15_000 && output.get("error").is_none())));
        let failed = later.agent_settled("t2", "failed", "", 0);
        assert!(matches!(&failed, RuntimeEvent::Delegation { event, .. }
            if matches!(event.as_ref(), RuntimeEvent::Tool { output, .. }
                if output["error"].is_string() && output.get("tokens").is_none())));
    }

    #[test]
    fn a_subagent_opens_speaks_and_ends_as_one_delegation() {
        let mut stream = Stream::default();
        let start = stream.start_agent(
            "t1",
            &json!({"subagent_type": "Explore", "description": "find the parser"}),
        );
        assert!(
            matches!(&start, RuntimeEvent::Delegation { id, name, task, event }
            if id == "t1" && name == "Explore" && task == "find the parser"
                && matches!(event.as_ref(), RuntimeEvent::Tool { running: true, .. }))
        );
        let said = stream.under(Some("t1"), RuntimeEvent::Text("found it".into()));
        assert!(matches!(&said, RuntimeEvent::Delegation { event, .. }
            if matches!(event.as_ref(), RuntimeEvent::Text(text) if text == "found it")));
        let end = stream
            .end_agent("t1", "found it", false, Some(&json!({"totalTokens": 740})))
            .unwrap();
        assert!(matches!(&end, RuntimeEvent::Delegation { event, .. }
                if matches!(event.as_ref(), RuntimeEvent::Tool { output, .. } if output["tokens"] == 740)));
        assert!(matches!(&end, RuntimeEvent::Delegation { event, .. }
            if matches!(event.as_ref(), RuntimeEvent::Tool { running: false, output, .. }
                if output.get("reply").is_none())));
        assert!(stream.end_agent("other", "", false, None).is_none());
        assert!(matches!(
            stream.under(None, RuntimeEvent::Text("x".into())),
            RuntimeEvent::Text(_)
        ));
    }

    #[test]
    fn a_retry_says_when_and_why() {
        let retry: SdkApiRetryMessage = serde_json::from_value(json!({
            "attempt": 2, "max_retries": 10, "retry_delay_ms": 1500,
            "error_status": 529, "error": "overloaded", "uuid": "u", "session_id": "s",
        }))
        .unwrap();
        let RuntimeEvent::Tool { name, output, .. } = super::retry(&retry) else {
            panic!("a notice");
        };
        assert_eq!(name, NOTICE);
        assert_eq!(
            entry_text(&output),
            "Retrying in 2s because the API is overloaded (try 2 of 10)."
        );
    }

    #[test]
    fn load_problems_summarize_once_a_session() {
        let init: claude_agent_sdk::protocol::SystemInit = serde_json::from_value(json!({
            "apiKeySource": "none", "claude_code_version": "2", "cwd": "/", "tools": [],
            "mcp_servers": [
                {"name": "github", "status": "failed"},
                {"name": "stripe", "status": "needs-auth"},
                {"name": "figma", "status": "needs-auth"},
                {"name": "ok", "status": "connected"},
            ],
            "model": "m", "permissionMode": "default", "slash_commands": [], "output_style": "default",
            "skills": [], "plugins": [], "plugin_errors": [{"plugin": "lint", "error": "bad manifest"}],
            "uuid": "u", "session_id": "load-problems-test",
        }))
        .unwrap();
        assert_eq!(
            load_problems(&init).as_deref(),
            Some("2 MCP servers need sign-in, 1 MCP server didn't connect, 1 plugin didn't load")
        );
        assert_eq!(load_problems(&init), None);
    }

    #[test]
    fn one_load_problem_is_named() {
        assert_eq!(
            summarize(&[LoadProblem::SignIn("stripe".into())]).as_deref(),
            Some("stripe needs sign-in")
        );
        assert_eq!(
            summarize(&[LoadProblem::Failed("openagents".into())]).as_deref(),
            Some("openagents didn't connect")
        );
        assert_eq!(
            summarize(&[
                LoadProblem::SignIn("a".into()),
                LoadProblem::SignIn("b".into()),
                LoadProblem::SignIn("c".into()),
            ])
            .as_deref(),
            Some("3 MCP servers need sign-in")
        );
        assert_eq!(summarize(&[]), None);
    }

    #[test]
    fn the_load_warning_flashes_for_a_few_seconds() {
        let init: claude_agent_sdk::protocol::SystemInit = serde_json::from_value(json!({
            "apiKeySource": "none", "claude_code_version": "2", "cwd": "/", "tools": [],
            "mcp_servers": [{"name": "stripe", "status": "needs-auth"}],
            "model": "m", "permissionMode": "default", "slash_commands": [], "output_style": "default",
            "skills": [], "plugins": [], "plugin_errors": [], "uuid": "u", "session_id": "load-warning-test",
        }))
        .unwrap();
        note_load_problems(&init);
        let now = std::time::Instant::now();
        assert_eq!(load_warning(now).as_deref(), Some("stripe needs sign-in"));
        assert_eq!(load_warning(now + LOAD_WARNING_FOR), None);
    }

    #[test]
    fn the_context_line_is_for_its_own_session() {
        let mut stream = Stream::default();
        stream.note_usage(&json!({"usage": {
            "input_tokens": 10, "cache_read_input_tokens": 80_000, "output_tokens": 4_000,
        }}));
        let usage = serde_json::from_value(json!({"m": {
            "inputTokens": 0, "outputTokens": 0, "cacheReadInputTokens": 0,
            "cacheCreationInputTokens": 0, "webSearchRequests": 0, "costUSD": 0.0,
            "contextWindow": 200_000,
        }}))
        .unwrap();
        stream.record_context(Some("context-test"), "m", &usage);
        assert_eq!(context_line("context-test").as_deref(), Some("Context 42%"));
        assert_eq!(context_line("someone-else"), None);
    }

    #[test]
    fn empty_thinking_shows_nothing() {
        assert!(thinking(&json!({"type": "thinking", "thinking": ""})).is_none());
        assert!(thinking(&json!({"type": "thinking", "thinking": "Look first."})).is_some());
    }
}
