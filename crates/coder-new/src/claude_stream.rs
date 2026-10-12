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
pub const THINKING: &str = "claude:thinking";

/// The entry that holds one short notice in the conversation: its words are
/// in `output.text`.
pub const NOTICE: &str = "claude:notice";

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
    ) -> Option<RuntimeEvent> {
        let spoke = self.agents.get(id)?.spoke;
        let output = if failed {
            json!({"error": reply})
        } else if spoke || reply.trim().is_empty() {
            json!({})
        } else {
            json!({"reply": reply})
        };
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

/// The notices for what failed to load when `session` started: an MCP
/// server that did not connect or a plugin that did not load. Each shows
/// once a session, though Claude Code reports them again each turn.
#[must_use]
pub fn load_problems(init: &claude_agent_sdk::protocol::SystemInit) -> Vec<RuntimeEvent> {
    let mut lines = Vec::new();
    for server in &init.mcp_servers {
        match server.status.as_str() {
            "failed" => lines.push(format!("The MCP server {} did not connect.", server.name)),
            "needs-auth" => lines.push(format!(
                "The MCP server {} needs you to sign in.",
                server.name
            )),
            _ => {}
        }
    }
    for error in init.plugin_errors.iter().flatten() {
        let name = ["plugin", "name", "source"]
            .iter()
            .find_map(|field| error[*field].as_str());
        let why = ["error", "message", "reason"]
            .iter()
            .find_map(|field| error[*field].as_str());
        lines.push(match (name, why, error.as_str()) {
            (Some(name), Some(why), _) => format!("The plugin {name} did not load: {why}"),
            (Some(name), None, _) => format!("The plugin {name} did not load."),
            (None, Some(why), _) | (None, None, Some(why)) => {
                format!("A plugin did not load: {why}")
            }
            (None, None, None) => "A plugin did not load.".to_owned(),
        });
    }
    let mut shown = lock(&SHOWN);
    lines
        .into_iter()
        .filter(|line| {
            let seen = (init.session_id.clone(), line.clone());
            let new = !shown.contains(&seen);
            if new {
                shown.push(seen);
            }
            new
        })
        .map(|line| notice_entry(NOTICE, &line))
        .collect()
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

/// The newest session's context: its id, the tokens it holds, its window.
static CONTEXT: Mutex<Option<(String, u64, u64)>> = Mutex::new(None);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// "Context 42% full", for the chat holding `session`, once a turn of it
/// reported its size.
#[must_use]
pub fn context_line(session: &str) -> Option<String> {
    let guard = lock(&CONTEXT);
    let (id, used, window) = guard.as_ref()?;
    (id == session).then(|| {
        let percent = (*used as f64 / *window as f64 * 100.0).clamp(0.0, 100.0);
        format!("Context {percent:.0}% full")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let end = stream.end_agent("t1", "found it", false).unwrap();
        assert!(matches!(&end, RuntimeEvent::Delegation { event, .. }
            if matches!(event.as_ref(), RuntimeEvent::Tool { running: false, output, .. }
                if output.get("reply").is_none())));
        assert!(stream.end_agent("other", "", false).is_none());
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
    fn load_problems_show_once_a_session() {
        let init: claude_agent_sdk::protocol::SystemInit = serde_json::from_value(json!({
            "apiKeySource": "none", "claude_code_version": "2", "cwd": "/", "tools": [],
            "mcp_servers": [{"name": "github", "status": "failed"}, {"name": "ok", "status": "connected"}],
            "model": "m", "permissionMode": "default", "slash_commands": [], "output_style": "default",
            "skills": [], "plugins": [], "plugin_errors": [{"plugin": "lint", "error": "bad manifest"}],
            "uuid": "u", "session_id": "load-problems-test",
        }))
        .unwrap();
        let texts: Vec<String> = load_problems(&init)
            .into_iter()
            .map(|event| match event {
                RuntimeEvent::Tool { output, .. } => entry_text(&output).to_owned(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(
            texts,
            [
                "The MCP server github did not connect.",
                "The plugin lint did not load: bad manifest"
            ]
        );
        assert!(load_problems(&init).is_empty());
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
        assert_eq!(
            context_line("context-test").as_deref(),
            Some("Context 42% full")
        );
        assert_eq!(context_line("someone-else"), None);
    }

    #[test]
    fn empty_thinking_shows_nothing() {
        assert!(thinking(&json!({"type": "thinking", "thinking": ""})).is_none());
        assert!(thinking(&json!({"type": "thinking", "thinking": "Look first."})).is_some());
    }
}
