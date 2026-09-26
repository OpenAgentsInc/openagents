use crate::{MAX_READABLE_RECORD_BYTES, Readable};
use serde_json::Value;

fn field(value: &Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 128)
        .map(str::to_owned)
}

fn trim(value: &str, max: usize) -> (String, bool) {
    let mut end = value.len().min(max);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), end < value.len())
}

/// Project a complete JSON record for display without executing its contents.
/// At most 256 KiB is parsed; larger records return None and remain available
/// through raw chunks. Unknown and malformed records are explicit projections.
/// Text is a 1 KiB preview, never a substitute for the original record bytes.
pub fn readable_record(raw: &[u8]) -> Option<Readable> {
    project(raw, 1024)
}

/// Project all recognized text from a locally assembled record. The input is
/// still capped at 256 KiB, but text is not shortened to the host-page preview.
/// The caller must page the resulting text for display and retain original
/// bytes for unknown fields and records larger than the parser bound.
pub fn readable_record_full(raw: &[u8]) -> Option<Readable> {
    project(raw, usize::MAX)
}

fn project(raw: &[u8], text_limit: usize) -> Option<Readable> {
    if raw.len() > MAX_READABLE_RECORD_BYTES {
        return None;
    }
    let parsed = serde_json::from_slice::<Value>(raw);
    let Ok(value) = parsed else {
        return Some(Readable {
            kind: "invalid_json".into(),
            native_id: None,
            role: None,
            timestamp: None,
            tool_name: None,
            call_id: None,
            text: "Unrecognized JSON record; original bytes retained.".into(),
            text_truncated: false,
            unknown: true,
        });
    };
    let outer = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let payload = value.get("payload").unwrap_or(&value);
    let kind = payload.get("type").and_then(Value::as_str).unwrap_or(outer);
    let message = payload
        .get("message")
        .filter(|v| v.is_object())
        .unwrap_or(payload);
    let tool = message
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("tool_use" | "tool_result")
                )
            })
        });
    let mut pieces = Vec::new();
    if let Some(content) = message.get("content") {
        collect(content, &mut pieces);
    }
    for key in ["text", "message", "arguments", "input", "output"] {
        if let Some(item) = payload
            .get(key)
            .filter(|v| !v.is_object() || key != "message")
        {
            if key == "input" && item.is_object() {
                pieces.push(item.to_string());
            } else {
                collect(item, &mut pieces);
            }
        }
    }
    let text = pieces.join("\n");
    let (text, text_truncated) = trim(&text, text_limit);
    let known = matches!(
        kind,
        "message"
            | "user"
            | "assistant"
            | "user_message"
            | "agent_message"
            | "function_call"
            | "function_call_output"
            | "custom_tool_call"
            | "custom_tool_call_output"
            | "tool_use"
            | "tool_result"
            | "reasoning"
            | "session_meta"
            | "turn_context"
            | "token_count"
            | "task_started"
            | "task_complete"
            | "turn_aborted"
            | "compacted"
            | "summary"
    );
    let role = field(message, "role").or_else(|| match kind {
        "user_message" | "user" => Some("user".into()),
        "agent_message" | "assistant" => Some("assistant".into()),
        _ => None,
    });
    Some(Readable {
        kind: trim(kind, 128).0,
        native_id: field(&value, "uuid").or_else(|| field(payload, "id")),
        role,
        timestamp: field(&value, "timestamp").or_else(|| field(payload, "timestamp")),
        tool_name: field(payload, "name").or_else(|| tool.and_then(|t| field(t, "name"))),
        call_id: field(payload, "call_id")
            .or_else(|| tool.and_then(|t| field(t, "id").or_else(|| field(t, "tool_use_id")))),
        text,
        text_truncated,
        unknown: !known,
    })
}

fn collect(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(values) => {
            for value in values {
                collect(value, out);
            }
        }
        Value::Object(value) => {
            if let Some(text) = value.get("text").and_then(Value::as_str) {
                out.push(text.to_owned());
            }
            if let Some(name) = value.get("name").and_then(Value::as_str) {
                out.push(format!("Tool: {name}"));
            }
            if let Some(input) = value.get("input") {
                out.push(input.to_string());
            }
            if let Some(content) = value.get("content") {
                collect(content, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_full_projection_preserves_unicode_and_tool_text_beyond_preview() {
        let text = "日本語 👩🏽‍💻 ".repeat(300);
        let raw = serde_json::to_vec(&serde_json::json!({
            "type": "assistant",
            "message": {"role": "assistant", "content": [
                {"type": "text", "text": text},
                {"type": "tool_use", "id": "call-one", "name": "Read", "input": {"file": "sample.rs"}}
            ]}
        })).unwrap();
        let preview = readable_record(&raw).unwrap();
        assert!(preview.text_truncated);
        assert!(preview.text.len() <= 1024);
        let full = readable_record_full(&raw).unwrap();
        assert!(!full.text_truncated);
        assert!(full.text.starts_with(&text));
        assert!(full.text.contains("Tool: Read"));
        assert!(full.text.contains("sample.rs"));
        assert_eq!(full.call_id.as_deref(), Some("call-one"));
        assert!(readable_record_full(&vec![b' '; MAX_READABLE_RECORD_BYTES + 1]).is_none());
    }
}
