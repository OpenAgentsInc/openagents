use super::*;

pub(super) fn normalize_notification(
    notification: AppServerNotification,
) -> Option<CodexLaneNotification> {
    let raw_method = notification.method;
    let method = codex_client::canonical_notification_method(raw_method.as_str());
    let params = notification.params;

    match method {
        "thread/started" => {
            let thread_id = thread_id_from_params(params.as_ref())?;
            Some(CodexLaneNotification::ThreadStatusChanged {
                thread_id,
                status: "active".to_string(),
            })
        }
        "thread/status/changed" => {
            let params = params?;
            let thread_id = string_field(&params, "threadId")?;
            let status = params
                .get("status")
                .and_then(thread_status_label)
                .unwrap_or_else(|| "unknown".to_string());
            Some(CodexLaneNotification::ThreadStatusChanged { thread_id, status })
        }
        "thread/archived" => {
            let params = params?;
            Some(CodexLaneNotification::ThreadArchived {
                thread_id: string_field(&params, "threadId")?,
            })
        }
        "thread/unarchived" => {
            let params = params?;
            Some(CodexLaneNotification::ThreadUnarchived {
                thread_id: string_field(&params, "threadId")?,
            })
        }
        "thread/closed" => {
            let params = params?;
            Some(CodexLaneNotification::ThreadClosed {
                thread_id: string_field(&params, "threadId")?,
            })
        }
        "thread/name/updated" => {
            let params = params?;
            Some(CodexLaneNotification::ThreadNameUpdated {
                thread_id: string_field(&params, "threadId")?,
                thread_name: string_field(&params, "threadName"),
            })
        }
        "account/updated" => {
            let params = params?;
            Some(CodexLaneNotification::AccountUpdated {
                auth_mode: string_field(&params, "authMode"),
            })
        }
        "account/login/completed" => {
            let params = params?;
            Some(CodexLaneNotification::AccountLoginCompleted {
                login_id: string_field(&params, "loginId"),
                success: params
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                error: string_field(&params, "error"),
            })
        }
        "account/rateLimits/updated" => {
            let params = params?;
            let rate_limits = params.get("rateLimits")?;
            Some(CodexLaneNotification::AccountRateLimitsLoaded {
                summary: rate_limits_summary(rate_limits),
            })
        }
        "model/rerouted" => {
            let params = params?;
            Some(CodexLaneNotification::ModelRerouted {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                from_model: string_field(&params, "fromModel")?,
                to_model: string_field(&params, "toModel")?,
                reason: string_field(&params, "reason").unwrap_or_else(|| "unknown".to_string()),
            })
        }
        "mcpServer/oauthLogin/completed" => {
            let params = params?;
            Some(CodexLaneNotification::McpServerOauthLoginCompleted {
                server_name: string_field(&params, "name").unwrap_or_else(|| "unknown".to_string()),
                success: params
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                error: string_field(&params, "error"),
            })
        }
        "app/list/updated" => Some(CodexLaneNotification::AppsListUpdated),
        "fuzzyFileSearch/sessionUpdated" => {
            let params = params?;
            let session_id =
                string_field(&params, "sessionId").unwrap_or_else(|| "unknown".to_string());
            let status = string_field(&params, "status").unwrap_or_else(|| {
                serde_json::to_string(&params).unwrap_or_else(|_| "updated".to_string())
            });
            Some(CodexLaneNotification::FuzzySessionUpdated { session_id, status })
        }
        "fuzzyFileSearch/sessionCompleted" => {
            let params = params?;
            let session_id =
                string_field(&params, "sessionId").unwrap_or_else(|| "unknown".to_string());
            Some(CodexLaneNotification::FuzzySessionCompleted { session_id })
        }
        "thread/realtime/started" => {
            let params = params?;
            Some(CodexLaneNotification::RealtimeStarted {
                thread_id: string_field(&params, "threadId")?,
                session_id: string_field(&params, "sessionId"),
            })
        }
        "thread/realtime/closed" => {
            let params = params?;
            Some(CodexLaneNotification::RealtimeStopped {
                thread_id: string_field(&params, "threadId")?,
            })
        }
        "thread/realtime/error" => {
            let params = params?;
            let message = params
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| string_field(&params, "message"))
                .unwrap_or_else(|| "thread realtime error".to_string());
            Some(CodexLaneNotification::RealtimeError {
                thread_id: string_field(&params, "threadId")
                    .unwrap_or_else(|| "unknown-thread".to_string()),
                message,
            })
        }
        "windowsSandbox/setupCompleted" => {
            let params = params?;
            Some(CodexLaneNotification::WindowsSandboxSetupCompleted {
                mode: string_field(&params, "mode"),
                success: params.get("success").and_then(Value::as_bool),
            })
        }
        "turn/started" => {
            let params = params?;
            let thread_id = string_field(&params, "threadId")?;
            let turn_id = turn_id_from_value(&params)?;
            Some(CodexLaneNotification::TurnStarted { thread_id, turn_id })
        }
        "item/started" => {
            let params = params?;
            if let Some((review, completed)) = review_progress_from_item_params(&params) {
                return Some(CodexLaneNotification::ReviewProgressUpdated {
                    thread_id: thread_id_field(&params)?,
                    turn_id: turn_id_field(&params)?,
                    review,
                    completed,
                });
            }
            Some(CodexLaneNotification::ItemStarted {
                thread_id: thread_id_field(&params)?,
                turn_id: turn_id_field(&params)?,
                item_id: item_id_from_params(&params),
                item_type: item_type_from_params(&params),
            })
        }
        "item/completed" => {
            let params = params?;
            if let Some((review, completed)) = review_progress_from_item_params(&params) {
                return Some(CodexLaneNotification::ReviewProgressUpdated {
                    thread_id: thread_id_field(&params)?,
                    turn_id: turn_id_field(&params)?,
                    review,
                    completed,
                });
            }
            Some(CodexLaneNotification::ItemCompleted {
                thread_id: thread_id_field(&params)?,
                turn_id: turn_id_field(&params)?,
                item_id: item_id_from_params(&params),
                item_type: item_type_from_params(&params),
                message: agent_message_from_item_params(&params),
            })
        }
        "item/agentMessage/completed" => {
            let params = params?;
            Some(CodexLaneNotification::ItemCompleted {
                thread_id: thread_id_field(&params)?,
                turn_id: turn_id_field(&params)?,
                item_id: item_id_from_params(&params),
                item_type: item_type_from_params(&params),
                message: agent_message_from_item_params(&params),
            })
        }
        "agent_message_delta" => {
            let params = params?;
            let thread_id = thread_id_field(&params)?;
            let turn_id = turn_id_field(&params)?;
            let (item_id, delta) = if let Some(msg) = params.get("msg") {
                let item_id = msg
                    .get("item_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| string_field(msg, "itemId"))
                    .or_else(|| string_field(&params, "itemId"))
                    .unwrap_or_else(|| "event-agent-message".to_string());
                let delta = msg
                    .get("delta")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| string_field(msg, "text"))
                    .or_else(|| string_field(msg, "message"))
                    .or_else(|| string_field(&params, "delta"))?;
                (item_id, delta)
            } else {
                (
                    string_field(&params, "itemId")
                        .or_else(|| item_id_from_params(&params))
                        .unwrap_or_else(|| "event-agent-message".to_string()),
                    string_field(&params, "delta")?,
                )
            };
            Some(CodexLaneNotification::AgentMessageDelta {
                thread_id,
                turn_id,
                item_id,
                delta,
            })
        }
        "agent_message" => {
            let params = params?;
            let msg = params.get("msg").unwrap_or(&params);
            Some(CodexLaneNotification::AgentMessageCompleted {
                thread_id: thread_id_field(&params).or_else(|| thread_id_field(msg))?,
                turn_id: turn_id_field(&params).or_else(|| turn_id_field(msg))?,
                item_id: msg
                    .get("item_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| string_field(msg, "itemId")),
                message: msg
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| string_field(msg, "text"))?,
            })
        }
        "agent_reasoning_delta" => {
            let params = params?;
            let msg = params.get("msg");
            Some(CodexLaneNotification::ReasoningDelta {
                thread_id: thread_id_field(&params)?,
                turn_id: turn_id_field(&params)?,
                item_id: string_field(&params, "itemId").or_else(|| {
                    msg.and_then(|value| {
                        value
                            .get("item_id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                }),
                delta: string_field(&params, "delta")
                    .or_else(|| msg.and_then(|value| string_field(value, "delta")))?,
            })
        }
        "agent_reasoning" => {
            let params = params?;
            let msg = params.get("msg").unwrap_or(&params);
            Some(CodexLaneNotification::ReasoningDelta {
                thread_id: thread_id_field(&params).or_else(|| thread_id_field(msg))?,
                turn_id: turn_id_field(&params).or_else(|| turn_id_field(msg))?,
                item_id: msg
                    .get("item_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| string_field(msg, "itemId")),
                delta: string_field(msg, "text")
                    .or_else(|| string_field(msg, "message"))
                    .or_else(|| string_field(&params, "text"))
                    .or_else(|| string_field(&params, "message"))?,
            })
        }
        "task_started" => {
            let params = params?;
            Some(CodexLaneNotification::TurnStarted {
                thread_id: thread_id_field(&params)?,
                turn_id: turn_id_field(&params)?,
            })
        }
        "task_complete" => {
            let params = params?;
            let msg = params.get("msg");
            Some(CodexLaneNotification::TurnCompleted {
                thread_id: thread_id_field(&params)?,
                turn_id: turn_id_field(&params)?,
                status: Some("completed".to_string()),
                error_message: None,
                final_message: msg
                    .and_then(|value| string_field(value, "last_agent_message"))
                    .and_then(non_empty_text)
                    .or_else(|| {
                        msg.and_then(|value| string_field(value, "lastAgentMessage"))
                            .and_then(non_empty_text)
                    })
                    .or_else(|| msg.and_then(|value| string_field(value, "message")))
                    .or_else(|| string_field(&params, "message"))
                    .and_then(non_empty_text),
            })
        }
        "task_failed" => {
            let params = params?;
            let msg = params.get("msg");
            let message = msg
                .and_then(|value| string_field(value, "message"))
                .or_else(|| {
                    msg.and_then(|value| value.get("error"))
                        .and_then(|error| string_field(error, "message"))
                })
                .or_else(|| string_field(&params, "message"))
                .unwrap_or_else(|| "task failed".to_string());
            Some(CodexLaneNotification::TurnError {
                thread_id: thread_id_field(&params).unwrap_or_else(|| "unknown-thread".to_string()),
                turn_id: turn_id_field(&params).unwrap_or_else(|| "unknown-turn".to_string()),
                message,
            })
        }
        "turn/completed" => {
            let params = params?;
            let thread_id = string_field(&params, "threadId")?;
            let turn_id =
                turn_id_from_value(&params).or_else(|| string_field(&params, "turnId"))?;
            let status = params
                .get("turn")
                .and_then(|turn| turn.get("status"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| string_field(&params, "status"));
            let error_message = params
                .get("turn")
                .and_then(|turn| turn.get("error"))
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    params
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            let final_message = params
                .get("turn")
                .and_then(|turn| string_field(turn, "lastAgentMessage"))
                .and_then(non_empty_text)
                .or_else(|| string_field(&params, "lastAgentMessage").and_then(non_empty_text))
                .or_else(|| string_field(&params, "last_agent_message").and_then(non_empty_text))
                .or_else(|| string_field(&params, "message").and_then(non_empty_text));
            Some(CodexLaneNotification::TurnCompleted {
                thread_id,
                turn_id,
                status,
                error_message,
                final_message,
            })
        }
        "turn/diff/updated" => {
            let params = params?;
            Some(CodexLaneNotification::TurnDiffUpdated {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                diff: string_field(&params, "diff")?,
            })
        }
        "turn/plan/updated" => {
            let params = params?;
            Some(CodexLaneNotification::TurnPlanUpdated {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                explanation: string_field(&params, "explanation"),
                plan: turn_plan_from_params(&params),
            })
        }
        "thread/compacted" => {
            let params = params?;
            Some(CodexLaneNotification::ThreadCompacted {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
            })
        }
        "thread/tokenUsage/updated" => {
            let params = params?;
            let token_usage = params.get("tokenUsage")?;
            let usage_scope = token_usage
                .get("last")
                .filter(|last| last.is_object())
                .unwrap_or(token_usage);
            Some(CodexLaneNotification::ThreadTokenUsageUpdated {
                thread_id: string_field(&params, "threadId")?,
                turn_id: string_field(&params, "turnId")?,
                input_tokens: i64_field(usage_scope, "inputTokens").unwrap_or_default(),
                cached_input_tokens: i64_field(usage_scope, "cachedInputTokens")
                    .unwrap_or_default(),
                output_tokens: i64_field(usage_scope, "outputTokens").unwrap_or_default(),
            })
        }
        "turn/error" => {
            let params = params?;
            let message = params
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("Unknown turn error")
                .to_string();
            Some(CodexLaneNotification::TurnError {
                thread_id: string_field(&params, "threadId")
                    .unwrap_or_else(|| "unknown-thread".to_string()),
                turn_id: string_field(&params, "turnId")
                    .unwrap_or_else(|| "unknown-turn".to_string()),
                message,
            })
        }
        _ => Some(CodexLaneNotification::Raw { method: raw_method }),
    }
}

pub(super) fn thread_id_from_params(params: Option<&Value>) -> Option<String> {
    let params = params?;
    params
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| string_field(params, "threadId"))
}

pub(super) fn turn_id_from_value(value: &Value) -> Option<String> {
    value
        .get("turn")
        .and_then(|turn| turn.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(super) fn item_id_from_params(value: &Value) -> Option<String> {
    item_value_from_params(value)
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| string_field(value, "itemId"))
        .or_else(|| {
            value
                .get("msg")
                .and_then(|msg| msg.get("item_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

pub(super) fn item_type_from_params(value: &Value) -> Option<String> {
    item_value_from_params(value)
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(super) fn item_value_from_params(value: &Value) -> Option<&Value> {
    value
        .get("item")
        .or_else(|| value.get("msg").and_then(|msg| msg.get("item")))
}

pub(super) fn item_type_is_agent_like(item_type: &str) -> bool {
    let lower = item_type.to_ascii_lowercase();
    lower.contains("agent") || lower.contains("assistant")
}

pub(super) fn agent_message_from_item_params(value: &Value) -> Option<String> {
    let item = item_value_from_params(value)?;
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
    if !item_type_is_agent_like(item_type) {
        return None;
    }
    string_field(item, "text")
        .and_then(non_empty_text)
        .or_else(|| string_field(item, "message").and_then(non_empty_text))
        .or_else(|| item.get("content").and_then(extract_content_text))
}

pub(super) fn turn_plan_from_params(value: &Value) -> Vec<CodexTurnPlanStep> {
    value
        .get("plan")
        .and_then(Value::as_array)
        .map(|steps| {
            steps
                .iter()
                .filter_map(|step| {
                    Some(CodexTurnPlanStep {
                        step: step.get("step")?.as_str()?.to_string(),
                        status: step.get("status")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn extract_thread_transcript_messages(
    thread: &codex_client::ThreadSnapshot,
) -> Vec<CodexThreadTranscriptMessage> {
    let mut messages = Vec::new();
    for turn in &thread.turns {
        for item in &turn.items {
            collect_transcript_messages(item, &mut messages);
        }
    }
    messages
}

pub(super) fn extract_latest_thread_plan_artifact(
    thread: &codex_client::ThreadSnapshot,
) -> Option<CodexThreadPlanArtifact> {
    for turn in thread.turns.iter().rev() {
        for item in turn.items.iter().rev() {
            if let Some(text) = extract_plan_text_from_item(item) {
                return Some(CodexThreadPlanArtifact {
                    turn_id: turn.id.clone(),
                    text,
                });
            }
        }
    }
    None
}

pub(super) fn extract_latest_thread_review_artifact(
    thread: &codex_client::ThreadSnapshot,
) -> Option<CodexThreadReviewArtifact> {
    for turn in thread.turns.iter().rev() {
        for item in turn.items.iter().rev() {
            if let Some((review, completed)) = review_progress_from_item_value(item) {
                return Some(CodexThreadReviewArtifact {
                    turn_id: turn.id.clone(),
                    review,
                    completed,
                });
            }
        }
    }
    None
}

pub(super) fn extract_latest_thread_compaction_artifact(
    thread: &codex_client::ThreadSnapshot,
) -> Option<CodexThreadCompactionArtifact> {
    for turn in thread.turns.iter().rev() {
        for item in turn.items.iter().rev() {
            if item_type_from_item_value(item)
                .is_some_and(|item_type| item_type == "contextCompaction")
            {
                return Some(CodexThreadCompactionArtifact {
                    turn_id: turn.id.clone(),
                });
            }
        }
    }
    None
}

pub(super) fn extract_plan_text_from_item(value: &Value) -> Option<String> {
    let Some(object) = value.as_object() else {
        return None;
    };

    if let Some(payload) = object.get("payload") {
        if let Some(text) = extract_plan_text_from_item(payload) {
            return Some(text);
        }
    }

    match object.get("type").and_then(Value::as_str) {
        Some("plan") => string_field(value, "text").and_then(non_empty_text),
        _ => None,
    }
}

fn review_progress_from_item_params(value: &Value) -> Option<(String, bool)> {
    let item = item_value_from_params(value)?;
    review_progress_from_item_value(item)
}

fn review_progress_from_item_value(value: &Value) -> Option<(String, bool)> {
    let item_type = item_type_from_item_value(value)?;
    let completed = match item_type.as_str() {
        "enteredReviewMode" => false,
        "exitedReviewMode" => true,
        _ => return None,
    };
    let review = string_field(value, "review").and_then(non_empty_text)?;
    Some((review, completed))
}

fn item_type_from_item_value(value: &Value) -> Option<String> {
    let Some(object) = value.as_object() else {
        return None;
    };
    if let Some(payload) = object.get("payload") {
        if let Some(item_type) = item_type_from_item_value(payload) {
            return Some(item_type);
        }
    }
    object
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(super) fn collect_transcript_messages(
    value: &Value,
    messages: &mut Vec<CodexThreadTranscriptMessage>,
) {
    let Some(object) = value.as_object() else {
        return;
    };

    if let Some(payload) = object.get("payload") {
        collect_transcript_messages(payload, messages);
    }

    let kind = object.get("type").and_then(Value::as_str);
    match kind {
        Some("user_message") | Some("userMessage") => {
            let content = value
                .get("content")
                .and_then(extract_content_text)
                .or_else(|| string_field(value, "message").and_then(non_empty_text));
            if let Some(content) = content {
                messages.push(CodexThreadTranscriptMessage {
                    role: CodexThreadTranscriptRole::User,
                    content,
                });
            }
        }
        Some("agent_message") | Some("agentMessage") => {
            let content = string_field(value, "text")
                .and_then(non_empty_text)
                .or_else(|| string_field(value, "message").and_then(non_empty_text))
                .or_else(|| value.get("content").and_then(extract_content_text));
            if let Some(content) = content {
                messages.push(CodexThreadTranscriptMessage {
                    role: CodexThreadTranscriptRole::Codex,
                    content,
                });
            }
        }
        _ => {
            let Some(role) = object.get("role").and_then(Value::as_str) else {
                return;
            };
            let Some(mapped_role) = map_transcript_role(role) else {
                return;
            };

            let content = object
                .get("content")
                .and_then(extract_content_text)
                .or_else(|| string_field(value, "message").and_then(non_empty_text));
            if let Some(content) = content {
                messages.push(CodexThreadTranscriptMessage {
                    role: mapped_role,
                    content,
                });
            }
        }
    }
}

pub(super) fn map_transcript_role(role: &str) -> Option<CodexThreadTranscriptRole> {
    match role {
        "user" => Some(CodexThreadTranscriptRole::User),
        "assistant" | "codex" => Some(CodexThreadTranscriptRole::Codex),
        _ => None,
    }
}

pub(super) fn extract_content_text(content: &Value) -> Option<String> {
    match content {
        Value::String(value) => non_empty_text(value.to_string()),
        Value::Array(entries) => {
            let parts = entries
                .iter()
                .filter_map(|entry| {
                    entry
                        .get("text")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| entry.as_str().map(str::to_string))
                })
                .filter_map(non_empty_text)
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .and_then(non_empty_text),
        _ => None,
    }
}

pub(super) fn non_empty_text(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

pub(super) fn thread_id_field(value: &Value) -> Option<String> {
    non_empty_string_field(value, "threadId")
        .or_else(|| non_empty_string_field(value, "conversationId"))
        .or_else(|| non_empty_string_field(value, "thread_id"))
        .or_else(|| non_empty_string_field(value, "conversation_id"))
        .or_else(|| {
            value.get("msg").and_then(|msg| {
                non_empty_string_field(msg, "threadId")
                    .or_else(|| non_empty_string_field(msg, "conversationId"))
                    .or_else(|| non_empty_string_field(msg, "thread_id"))
                    .or_else(|| non_empty_string_field(msg, "conversation_id"))
            })
        })
}

pub(super) fn turn_id_field(value: &Value) -> Option<String> {
    non_empty_string_field(value, "turnId")
        .or_else(|| non_empty_string_field(value, "id"))
        .or_else(|| non_empty_string_field(value, "turn_id"))
        .or_else(|| {
            value.get("msg").and_then(|msg| {
                non_empty_string_field(msg, "turnId")
                    .or_else(|| non_empty_string_field(msg, "turn_id"))
                    .or_else(|| {
                        msg.get("turn")
                            .and_then(|turn| turn.get("id"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .and_then(non_empty_text)
                    })
                    .or_else(|| non_empty_string_field(msg, "id"))
            })
        })
}

pub(super) fn non_empty_string_field(value: &Value, field: &str) -> Option<String> {
    string_field(value, field).and_then(non_empty_text)
}

pub(super) fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

pub(super) fn i64_field(value: &Value, field: &str) -> Option<i64> {
    value.get(field).and_then(Value::as_i64)
}

pub(super) fn thread_status_label(status: &Value) -> Option<String> {
    if let Some(value) = status.as_str() {
        return Some(value.to_string());
    }

    let status_type = status
        .get("type")
        .and_then(Value::as_str)
        .map(str::to_string)?;
    if status_type != "active" {
        return Some(status_type);
    }

    let flags = status
        .get("activeFlags")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("+")
        })
        .unwrap_or_default();
    if flags.is_empty() {
        Some("active".to_string())
    } else {
        Some(format!("active:{flags}"))
    }
}
