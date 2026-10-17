//! The one slice of an OpenAgents waiting tool's input kept in the doc
//! (#11969): which chat it waits on, so the chat view can say "Waiting for
//! 'Fix composer'" while the call runs. Prompts and message text never leave
//! the host's run journal.

use openagents_proto::ToolCall;

/// The app's own MCP server, as each harness names it.
const SERVERS: [&str; 2] = ["openagents", "zeron"];
/// Tools that can block on another chat.
const TOOLS: [&str; 5] = [
    "wait_for_turn",
    "send_message",
    "send_messages",
    "create_chat",
    "create_chats",
];
/// Short identifiers only: a chat id, prefix or title, and the wait flag.
const KEEP: [&str; 3] = ["chat", "title", "wait"];
const MAX_CHARS: usize = 120;

/// `None` for any other tool, or when the input names none of [`KEEP`].
/// Idempotent: the kept keys are themselves kept.
pub fn wait_target(call: &ToolCall) -> Option<serde_json::Value> {
    let ToolCall::Mcp {
        server,
        tool,
        input: Some(input),
    } = call
    else {
        return None;
    };
    if !SERVERS.contains(&server.as_str()) || !TOOLS.contains(&tool.as_str()) {
        return None;
    }
    let kept: serde_json::Map<String, serde_json::Value> = KEEP
        .iter()
        .filter_map(|key| {
            let value = match input.get(key)? {
                serde_json::Value::Bool(flag) => serde_json::Value::Bool(*flag),
                serde_json::Value::String(text) => {
                    let text = text.trim();
                    if text.is_empty() {
                        return None;
                    }
                    serde_json::Value::from(text.chars().take(MAX_CHARS).collect::<String>())
                }
                _ => return None,
            };
            Some(((*key).to_owned(), value))
        })
        .collect();
    (!kept.is_empty()).then_some(serde_json::Value::Object(kept))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mcp(server: &str, tool: &str, input: serde_json::Value) -> ToolCall {
        ToolCall::Mcp {
            server: server.into(),
            tool: tool.into(),
            input: Some(input),
        }
    }

    #[test]
    fn keeps_only_the_chat_a_wait_names() {
        let call = mcp(
            "openagents",
            "send_message",
            json!({ "chat": "Fix composer", "text": "secret", "wait": true }),
        );
        let kept = wait_target(&call).unwrap();
        assert_eq!(kept, json!({ "chat": "Fix composer", "wait": true }));
        // A fixpoint, as the sanitize policy requires.
        assert_eq!(
            wait_target(&mcp("openagents", "send_message", kept.clone())),
            Some(kept)
        );
        assert_eq!(
            wait_target(&mcp("other", "send_message", json!({ "chat": "x" }))),
            None
        );
        assert_eq!(
            wait_target(&mcp("openagents", "read_chat", json!({ "chat": "x" }))),
            None
        );
        assert_eq!(
            wait_target(&mcp(
                "openagents",
                "create_chats",
                json!({ "requests": [] })
            )),
            None
        );
    }

    #[test]
    fn sanitize_keeps_the_wait_target() {
        let call = mcp(
            "openagents",
            "wait_for_turn",
            json!({ "chat": "9fa93200", "timeout_secs": 120 }),
        );
        assert_eq!(
            crate::sanitize_tool_call(&call),
            mcp("openagents", "wait_for_turn", json!({ "chat": "9fa93200" }))
        );
    }
}
