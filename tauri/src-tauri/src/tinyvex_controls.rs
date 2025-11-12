//! Control message schema and parser for WebSocket payloads.
//!
//! Simplified version for Tauri - supports core tinyvex operations
//! without the full oa-bridge control surface.

#[derive(Debug)]
pub enum ControlCommand {
    /// Echo/ping for connection testing
    Echo { payload: Option<String>, tag: Option<String> },

    /// Subscribe to a tinyvex stream (threads, messages, etc.)
    TvxSubscribe { stream: String, thread_id: Option<String> },

    /// Query tinyvex data (threads.list, messages.list, etc.)
    TvxQuery { name: String, args: serde_json::Value },

    /// Submit a new user message to start/continue a conversation
    RunSubmit {
        thread_id: String,
        text: String,
    },
}

/// Parse a control command from a raw JSON string. Returns None on errors.
pub fn parse_control_command(payload: &str) -> Option<ControlCommand> {
    let v: serde_json::Value = serde_json::from_str(payload).ok()?;
    let ty = v.get("control").and_then(|x| x.as_str())?;

    match ty {
        "echo" | "debug.echo" | "debug.ping" => {
            let payload = v.get("payload").and_then(|x| x.as_str()).map(|s| s.to_string());
            let tag = v.get("tag").and_then(|x| x.as_str()).map(|s| s.to_string());
            Some(ControlCommand::Echo { payload, tag })
        }

        "tvx.subscribe" => {
            let stream = v.get("stream").and_then(|x| x.as_str())?.to_string();
            let thread_id = v
                .get("thread_id")
                .or_else(|| v.get("threadId"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            Some(ControlCommand::TvxSubscribe { stream, thread_id })
        }

        "tvx.query" => {
            let name = v.get("name").and_then(|x| x.as_str())?.to_string();
            let args = v.get("args").cloned().unwrap_or(serde_json::json!({}));
            Some(ControlCommand::TvxQuery { name, args })
        }

        "run.submit" => {
            let thread_id = v
                .get("threadId")
                .or_else(|| v.get("thread_id"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())?;
            let text = v
                .get("text")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();
            Some(ControlCommand::RunSubmit { thread_id, text })
        }

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_echo() {
        let cmd = parse_control_command("{\"control\":\"echo\",\"payload\":\"test\"}");
        match cmd {
            Some(ControlCommand::Echo { payload, .. }) => {
                assert_eq!(payload.as_deref(), Some("test"));
            }
            _ => panic!("failed to parse echo"),
        }
    }

    #[test]
    fn parses_tvx_subscribe() {
        let cmd = parse_control_command(
            "{\"control\":\"tvx.subscribe\",\"stream\":\"messages\",\"threadId\":\"t1\"}"
        );
        match cmd {
            Some(ControlCommand::TvxSubscribe { stream, thread_id }) => {
                assert_eq!(stream, "messages");
                assert_eq!(thread_id.as_deref(), Some("t1"));
            }
            _ => panic!("failed to parse tvx.subscribe"),
        }
    }

    #[test]
    fn parses_tvx_query() {
        let cmd = parse_control_command(
            "{\"control\":\"tvx.query\",\"name\":\"threads.list\",\"args\":{\"limit\":10}}"
        );
        match cmd {
            Some(ControlCommand::TvxQuery { name, args }) => {
                assert_eq!(name, "threads.list");
                assert_eq!(args["limit"], 10);
            }
            _ => panic!("failed to parse tvx.query"),
        }
    }

    #[test]
    fn parses_run_submit() {
        let cmd = parse_control_command(
            "{\"control\":\"run.submit\",\"threadId\":\"t1\",\"text\":\"hello\"}"
        );
        match cmd {
            Some(ControlCommand::RunSubmit { thread_id, text }) => {
                assert_eq!(thread_id, "t1");
                assert_eq!(text, "hello");
            }
            _ => panic!("failed to parse run.submit"),
        }
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_control_command("not json").is_none());
        assert!(parse_control_command("{\"foo\":1}").is_none());
    }
}
