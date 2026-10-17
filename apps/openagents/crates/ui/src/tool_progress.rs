//! What a chat is doing while it runs a tool (#11969): the working row says
//! "Waiting for “Fix composer”… 1m 20s" or "Running cargo test… 12s" instead
//! of a bare "Working…". Plain words only; a tool we can't describe in plain
//! words keeps "Working".

use std::cell::RefCell;
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use openagents_doc::{MessagePart, MessageRole, MessageStatus, SessionMessageEntry};
use openagents_proto::{Chat, ToolCall};

/// The running tool's label and how long it has run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProgress {
    pub label: String,
    pub elapsed_secs: i64,
}

thread_local! {
    /// When this app first saw each running tool: the doc records no start
    /// time for a tool call.
    static STARTED: RefCell<HashMap<String, DateTime<Utc>>> = RefCell::default();
}

/// The tool the newest assistant message is running, when that is the
/// last thing it did.
pub fn running(
    transcript: &[SessionMessageEntry],
    chats: &[Chat],
    now: DateTime<Utc>,
) -> Option<ToolProgress> {
    let entry = transcript.last().filter(|e| {
        e.role == MessageRole::Assistant && e.status == Some(MessageStatus::Streaming)
    })?;
    let (id, call) = match entry
        .parts
        .iter()
        .rev()
        .find(|p| !matches!(p, MessagePart::Reasoning { .. }))?
    {
        MessagePart::Tool {
            id,
            call,
            resolved: false,
            ..
        } => (id, call),
        _ => return None,
    };
    let label = label(call, chats)?;
    let since = STARTED.with(|started| {
        let mut started = started.borrow_mut();
        if started.len() > 64 && !started.contains_key(id) {
            started.clear();
        }
        *started.entry(id.clone()).or_insert(now)
    });
    Some(ToolProgress {
        label,
        elapsed_secs: (now - since).num_seconds().max(0),
    })
}

fn label(call: &ToolCall, chats: &[Chat]) -> Option<String> {
    Some(match call {
        ToolCall::Exec { command } => format!("Running {}", clip(first_line(command), 48)),
        ToolCall::ReadFile { path } => format!("Reading {}", file_name(path)),
        ToolCall::WriteFile { path, .. } | ToolCall::EditFile { path, .. } => {
            format!("Editing {}", file_name(path))
        }
        ToolCall::ApplyPatch { path } => match path {
            Some(path) => format!("Editing {}", file_name(path)),
            None => "Editing files".into(),
        },
        ToolCall::Search { pattern, .. } => format!("Searching for {}", quoted(pattern)),
        ToolCall::Glob { .. } => "Finding files".into(),
        ToolCall::WebFetch { url, .. } => format!("Reading {}", host(url)),
        ToolCall::WebSearch { query } => format!("Searching the web for {}", quoted(query)),
        ToolCall::Mcp {
            server,
            tool,
            input,
        } if matches!(server.as_str(), "openagents" | "zeron") => {
            let arg = |key: &str| input.as_ref()?.get(key)?.as_str();
            match tool.as_str() {
                "wait_for_turn" => format!("Waiting for {}", chat_name(arg("chat"), chats)),
                "send_message" if input.as_ref().is_some_and(|i| i["wait"] == true) => {
                    format!("Waiting for {}", chat_name(arg("chat"), chats))
                }
                "send_message" => format!("Messaging {}", chat_name(arg("chat"), chats)),
                "send_messages" | "create_chats" => "Waiting for chats".into(),
                "create_chat" => match arg("title") {
                    Some(title) => format!("Starting {}", quoted(title)),
                    None => "Starting a chat".into(),
                },
                _ => return None,
            }
        }
        _ => return None,
    })
}

/// A chat by id, unique id prefix, or exact title, named by its title.
fn chat_name(key: Option<&str>, chats: &[Chat]) -> String {
    let title = key.map(str::trim).and_then(|key| {
        let chat = chats.iter().find(|c| c.id == key).or_else(|| {
            let mut prefixed = chats.iter().filter(|c| c.id.starts_with(key));
            match (prefixed.next(), prefixed.next()) {
                (Some(one), None) => Some(one),
                _ => None,
            }
        });
        match chat {
            Some(chat) => chat.title.clone(),
            None => chats
                .iter()
                .filter_map(|c| c.title.as_deref())
                .find(|t| t.trim().eq_ignore_ascii_case(key))
                .map(str::to_owned),
        }
    });
    match title.filter(|t| !t.trim().is_empty()) {
        Some(title) => quoted(&title),
        None => "a chat".into(),
    }
}

fn quoted(text: &str) -> String {
    let text = first_line(text);
    let clipped = clip(text, 32);
    if clipped.len() < text.len() {
        format!("“{clipped}…”")
    } else {
        format!("“{clipped}”")
    }
}

fn first_line(text: &str) -> &str {
    text.trim().lines().next().unwrap_or("").trim()
}

/// At most `max` characters; the working row's own "…" marks the cut.
fn clip(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((cut, _)) => text[..cut].trim_end(),
        None => text,
    }
}

fn file_name(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
}

fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chats() -> Vec<Chat> {
        serde_json::from_value(json!([
            {"id": "9fa93200-c1e1", "deviceId": "d", "title": "Fix composer focus after send",
             "archived": false, "createdAt": "2026-10-17T00:00:00Z"},
            {"id": "77aa0000-0000", "deviceId": "d", "title": "Docs",
             "archived": false, "createdAt": "2026-10-17T00:00:00Z"}
        ]))
        .unwrap()
    }

    fn streaming(parts: serde_json::Value) -> Vec<SessionMessageEntry> {
        serde_json::from_value(json!([{
            "id": "a1", "role": "assistant", "createdAt": 1, "deviceId": "d",
            "status": "streaming", "parts": parts
        }]))
        .unwrap()
    }

    fn tool(id: &str, call: serde_json::Value, resolved: bool) -> serde_json::Value {
        json!({"kind": "tool", "id": id, "call": call, "resolved": resolved})
    }

    #[test]
    fn names_the_running_tool_and_times_it() {
        let start = Utc::now();
        let wait = streaming(json!([
            {"kind": "text", "id": "t", "text": "Waiting on the composer fix."},
            tool("p-wait", json!({"kind": "mcp", "server": "openagents", "tool": "wait_for_turn",
                "input": {"chat": "9fa93200"}}), false),
        ]));
        let first = running(&wait, &chats(), start).unwrap();
        assert_eq!(first.label, "Waiting for “Fix composer focus after send”");
        assert_eq!(first.elapsed_secs, 0);
        let later = running(&wait, &chats(), start + chrono::Duration::seconds(80)).unwrap();
        assert_eq!(later.elapsed_secs, 80);

        let cases = [
            (
                json!({"kind": "exec", "command": "cargo test -p openagents-mcp\necho done"}),
                "Running cargo test -p openagents-mcp",
            ),
            (
                json!({"kind": "readFile", "path": "/repo/src/tools.rs"}),
                "Reading tools.rs",
            ),
            (
                json!({"kind": "webFetch", "url": "https://docs.rs/tokio/latest"}),
                "Reading docs.rs",
            ),
            (
                json!({"kind": "mcp", "server": "openagents", "tool": "send_message",
                "input": {"chat": "Docs", "wait": true}}),
                "Waiting for “Docs”",
            ),
            (
                json!({"kind": "mcp", "server": "openagents", "tool": "send_message",
                "input": {"chat": "missing"}}),
                "Messaging a chat",
            ),
            (
                json!({"kind": "mcp", "server": "openagents", "tool": "send_messages"}),
                "Waiting for chats",
            ),
        ];
        for (call, want) in cases {
            let parts = streaming(json!([tool("p", call, false)]));
            let label = running(&parts, &chats(), start).unwrap().label;
            assert_eq!(label, want);
            assert!(oa_copy::violations(&label, &[]).is_empty(), "{label}");
        }
    }

    #[test]
    fn keeps_working_when_no_tool_is_running() {
        let now = Utc::now();
        let done = streaming(json!([tool(
            "p",
            json!({"kind": "glob", "pattern": "*"}),
            true
        )]));
        assert_eq!(running(&done, &chats(), now), None);
        let after_text = streaming(json!([
            tool("p", json!({"kind": "glob", "pattern": "*"}), false),
            {"kind": "text", "id": "t", "text": "Found them."},
        ]));
        assert_eq!(running(&after_text, &chats(), now), None);
        let other = streaming(json!([tool(
            "p",
            json!({"kind": "mcp", "server": "figma",
            "tool": "get_design_context"}),
            false
        )]));
        assert_eq!(running(&other, &chats(), now), None);
    }
}
