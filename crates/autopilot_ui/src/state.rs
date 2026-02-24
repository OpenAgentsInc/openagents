use super::*;

pub(super) fn build_model_options() -> Vec<DropdownOption> {
    MODEL_OPTIONS
        .iter()
        .map(|(id, _)| DropdownOption::new(*id, *id))
        .collect()
}

pub(super) fn reasoning_options_for_model(model: &str) -> &'static [&'static str] {
    match model {
        "gpt-5.1-codex-mini" => &REASONING_OPTIONS_MINI,
        _ => &REASONING_OPTIONS_FULL,
    }
}

pub(super) fn build_reasoning_options(model: &str) -> Vec<DropdownOption> {
    reasoning_options_for_model(model)
        .iter()
        .map(|value| DropdownOption::new(*value, *value))
        .collect()
}

pub(super) fn reasoning_index(model: &str, effort: &str) -> Option<usize> {
    reasoning_options_for_model(model)
        .iter()
        .position(|value| value.eq_ignore_ascii_case(effort))
}

pub(super) fn default_reasoning_for_model(model: &str) -> &'static str {
    if model.contains("mini") {
        "high"
    } else {
        DEFAULT_REASONING_EFFORT
    }
}

pub(super) fn model_index(model: &str) -> Option<usize> {
    MODEL_OPTIONS.iter().position(|(id, _)| *id == model)
}

pub(super) fn extract_message_text(item: &Value) -> Option<String> {
    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }

    if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
        for entry in content {
            if let Some(text) = entry.get("text").and_then(|t| t.as_str()) {
                return Some(text.to_string());
            }
        }
    }

    None
}

pub(super) fn item_id(item: &Value) -> Option<String> {
    item.get("id")
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
}

pub(super) fn item_string(item: &Value, key: &str) -> Option<String> {
    item.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub(super) fn non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

pub(super) fn extract_session_hint(params: Option<&Value>) -> Option<String> {
    let params = params?;
    non_empty_string(params.get("sessionId").or_else(|| params.get("session_id")))
}

pub(super) fn extract_thread_hint(params: Option<&Value>) -> Option<String> {
    let params = params?;

    non_empty_string(params.get("threadId").or_else(|| params.get("thread_id")))
        .or_else(|| {
            non_empty_string(
                params
                    .get("conversationId")
                    .or_else(|| params.get("conversation_id")),
            )
        })
        .or_else(|| {
            params
                .get("thread")
                .and_then(|thread| non_empty_string(thread.get("id")))
        })
        .or_else(|| {
            let msg = params.get("msg")?;
            non_empty_string(msg.get("thread_id").or_else(|| msg.get("threadId")))
                .or_else(|| {
                    non_empty_string(
                        msg.get("conversationId")
                            .or_else(|| msg.get("conversation_id")),
                    )
                })
                .or_else(|| {
                    msg.get("thread")
                        .and_then(|thread| non_empty_string(thread.get("id")))
                })
        })
        .or_else(|| {
            let item = params.get("item")?;
            non_empty_string(item.get("thread_id").or_else(|| item.get("threadId"))).or_else(|| {
                non_empty_string(
                    item.get("conversationId")
                        .or_else(|| item.get("conversation_id")),
                )
            })
        })
}

pub(super) fn value_to_command_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            if text.trim().is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        Value::Array(parts) => {
            let items: Vec<&str> = parts.iter().filter_map(|val| val.as_str()).collect();
            if items.is_empty() {
                None
            } else {
                Some(items.join(" "))
            }
        }
        _ => None,
    }
}

pub(super) fn command_string_from_item(item: &Value) -> Option<String> {
    item.get("command").and_then(value_to_command_string)
}

pub(super) fn extract_file_changes(item: &Value) -> (Vec<String>, Option<String>, Option<String>) {
    let mut paths = Vec::new();
    let mut first_path = None;
    let mut first_diff = None;
    if let Some(changes) = item.get("changes").and_then(Value::as_array) {
        for change in changes {
            if let Some(path) = change.get("path").and_then(Value::as_str) {
                if first_path.is_none() {
                    first_path = Some(path.to_string());
                }
                paths.push(path.to_string());
            }
            if first_diff.is_none() {
                if let Some(diff) = change.get("diff").and_then(Value::as_str) {
                    first_diff = Some(diff.to_string());
                }
            }
        }
    }
    (paths, first_path, first_diff)
}
