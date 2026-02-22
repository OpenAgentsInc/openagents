use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexMessageRole {
    System,
    User,
    Assistant,
    Reasoning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexThreadMessage {
    pub id: String,
    pub role: CodexMessageRole,
    pub text: String,
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexThreadState {
    pub thread_id: Option<String>,
    pub messages: Vec<CodexThreadMessage>,
    #[serde(skip)]
    next_local_id: u64,
    #[serde(skip)]
    assistant_message_index_by_item: BTreeMap<String, usize>,
    #[serde(skip)]
    reasoning_message_index_by_item: BTreeMap<String, usize>,
    #[serde(skip)]
    pending_local_user_texts: Vec<String>,
}

impl Default for CodexThreadState {
    fn default() -> Self {
        Self {
            thread_id: None,
            messages: Vec::new(),
            next_local_id: 1,
            assistant_message_index_by_item: BTreeMap::new(),
            reasoning_message_index_by_item: BTreeMap::new(),
            pending_local_user_texts: Vec::new(),
        }
    }
}

#[cfg_attr(test, allow(dead_code))]
impl CodexThreadState {
    pub fn set_thread_id(&mut self, thread_id: Option<String>) {
        if self.thread_id == thread_id {
            return;
        }

        self.thread_id = thread_id;
        self.messages.clear();
        self.next_local_id = 1;
        self.assistant_message_index_by_item.clear();
        self.reasoning_message_index_by_item.clear();
        self.pending_local_user_texts.clear();
    }

    pub fn append_local_user_message(&mut self, text: &str) -> bool {
        let normalized = normalize_text(text);
        if normalized.is_empty() {
            return false;
        }

        self.pending_local_user_texts.push(normalized.clone());
        self.push_message(
            CodexMessageRole::User,
            normalized,
            false,
            Some(format!("local:user:{}", self.next_local_id)),
        );
        true
    }

    pub fn append_local_system_message(&mut self, text: &str) -> bool {
        let normalized = normalize_text(text);
        if normalized.is_empty() {
            return false;
        }

        self.push_message(
            CodexMessageRole::System,
            normalized,
            false,
            Some(format!("local:system:{}", self.next_local_id)),
        );
        true
    }

    pub fn ingest_khala_payload(&mut self, payload: &Value) -> bool {
        let Some(worker_payload) = decode_worker_payload(payload) else {
            return false;
        };
        let method = normalized_string(worker_payload.get("method")).unwrap_or_default();
        if method.is_empty() {
            return false;
        }

        let params = worker_payload
            .get("params")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let thread_id = extract_thread_id(worker_payload, &params);
        if !self.accept_thread(thread_id.as_deref()) {
            return false;
        }

        match method.as_str() {
            "thread/started" => false,
            "turn/started" => self.append_system_once("Turn started."),
            "turn/completed" => self.append_system_once("Turn completed."),
            "turn/error" | "turn/errored" => {
                let message =
                    extract_error_message(&params).unwrap_or_else(|| "Turn errored.".to_string());
                self.append_system_once(&message)
            }
            "codex/event/user_message" => {
                let Some(text) = extract_user_message_text(&params) else {
                    return false;
                };
                if self.consume_pending_user_text(&text) {
                    return false;
                }
                self.push_message(CodexMessageRole::User, text, false, None);
                true
            }
            "codex/event/agent_message" => {
                let Some(text) = extract_agent_message_text(&params) else {
                    return false;
                };
                self.push_message(CodexMessageRole::Assistant, text, false, None);
                true
            }
            "codex/event/agent_message_content_delta" | "codex/event/agent_message_delta" => {
                let Some(item_id) = extract_item_id(&params) else {
                    return false;
                };
                let Some(delta) = extract_agent_delta(&params) else {
                    return false;
                };
                self.append_stream_delta(CodexMessageRole::Assistant, &item_id, &delta)
            }
            "item/reasoning/summaryTextDelta"
            | "item/reasoning/textDelta"
            | "item/reasoning/contentDelta" => {
                let Some(item_id) = extract_item_id(&params) else {
                    return false;
                };
                let Some(delta) = normalized_string(params.get("delta")) else {
                    return false;
                };
                self.append_stream_delta(CodexMessageRole::Reasoning, &item_id, &delta)
            }
            "item/completed" => {
                if let Some(item_id) = extract_item_id(&params) {
                    self.mark_completed(&item_id)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn ingest_vercel_sse_wire(&mut self, wire: &str) -> usize {
        let mut changed = 0usize;
        for line in wire.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some(raw_event) = trimmed.strip_prefix("data:") else {
                continue;
            };
            let raw_event = raw_event.trim();
            if raw_event == "[DONE]" {
                if self.mark_all_streaming_complete() {
                    changed = changed.saturating_add(1);
                }
                continue;
            }
            let Ok(event) = serde_json::from_str::<Value>(raw_event) else {
                continue;
            };
            if self.ingest_vercel_sse_event(&event) {
                changed = changed.saturating_add(1);
            }
        }
        changed
    }

    pub fn ingest_vercel_sse_event(&mut self, event: &Value) -> bool {
        let Some(event_type) = normalized_string(event.get("type")) else {
            return false;
        };
        match event_type.as_str() {
            "start" => {
                let incoming_thread = normalized_string(event.get("threadId"))
                    .or_else(|| normalized_string(event.get("thread_id")));
                if let Some(incoming_thread) = incoming_thread {
                    let previous = self.thread_id.clone();
                    let _ = self.accept_thread(Some(incoming_thread.as_str()));
                    self.thread_id != previous
                } else {
                    false
                }
            }
            "start-step" => self.append_system_once("Turn started."),
            "text-start" => false,
            "text-delta" => {
                let Some(item_id) = normalized_string(event.get("id")) else {
                    return false;
                };
                let Some(delta) = normalized_string(event.get("delta")) else {
                    return false;
                };
                let role = match normalized_string(event.get("channel"))
                    .unwrap_or_else(|| "assistant".to_string())
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "reasoning" => CodexMessageRole::Reasoning,
                    _ => CodexMessageRole::Assistant,
                };
                self.append_stream_delta(role, &item_id, &delta)
            }
            "tool-input" => {
                let tool_name =
                    normalized_string(event.get("toolName")).unwrap_or_else(|| "tool".to_string());
                self.append_system_once(&format!("Tool started: {tool_name}"))
            }
            "tool-output" => {
                if normalized_string(event.get("status"))
                    .map(|status| status.eq_ignore_ascii_case("completed"))
                    .unwrap_or(false)
                {
                    self.append_system_once("Tool completed.")
                } else {
                    false
                }
            }
            "finish-step" => {
                let mut changed = self.mark_all_streaming_complete();
                if self.append_system_once("Turn completed.") {
                    changed = true;
                }
                changed
            }
            "error" => {
                let mut changed = self.mark_all_streaming_complete();
                let message = normalized_string(event.get("message"))
                    .unwrap_or_else(|| "Stream error.".to_string());
                if self.append_system_once(&message) {
                    changed = true;
                }
                changed
            }
            "finish" => {
                let mut changed = self.mark_all_streaming_complete();
                let status = normalized_string(event.get("status")).unwrap_or_default();
                if status.eq_ignore_ascii_case("error") && self.append_system_once("Turn failed.") {
                    changed = true;
                }
                changed
            }
            _ => false,
        }
    }

    pub fn hydrate_history_if_empty(
        &mut self,
        thread_id: Option<String>,
        messages: Vec<CodexThreadMessage>,
    ) -> bool {
        let mut changed = false;
        if self.thread_id != thread_id {
            self.set_thread_id(thread_id);
            changed = true;
        }

        if !self.messages.is_empty() {
            return changed;
        }

        if messages.is_empty() {
            return changed;
        }

        self.messages = messages;
        self.next_local_id = u64::try_from(self.messages.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        self.assistant_message_index_by_item.clear();
        self.reasoning_message_index_by_item.clear();
        self.pending_local_user_texts.clear();
        true
    }

    fn accept_thread(&mut self, incoming_thread_id: Option<&str>) -> bool {
        if let Some(incoming_thread_id) = incoming_thread_id {
            let incoming = incoming_thread_id.to_string();
            if self.thread_id.is_none() {
                self.thread_id = Some(incoming);
                return true;
            }
            return self.thread_id.as_deref() == Some(incoming_thread_id);
        }
        true
    }

    fn append_system_once(&mut self, message: &str) -> bool {
        let normalized = normalize_text(message);
        if normalized.is_empty() {
            return false;
        }
        if self
            .messages
            .last()
            .is_some_and(|entry| entry.role == CodexMessageRole::System && entry.text == normalized)
        {
            return false;
        }
        self.push_message(CodexMessageRole::System, normalized, false, None);
        true
    }

    fn consume_pending_user_text(&mut self, text: &str) -> bool {
        let normalized = normalize_text(text);
        if normalized.is_empty() {
            return false;
        }

        if let Some(index) = self
            .pending_local_user_texts
            .iter()
            .position(|entry| entry == &normalized)
        {
            self.pending_local_user_texts.remove(index);
            true
        } else {
            false
        }
    }

    fn append_stream_delta(&mut self, role: CodexMessageRole, item_id: &str, delta: &str) -> bool {
        let normalized_delta = normalize_text(delta);
        if normalized_delta.is_empty() {
            return false;
        }

        let maybe_index = match role {
            CodexMessageRole::Assistant => {
                self.assistant_message_index_by_item.get(item_id).copied()
            }
            CodexMessageRole::Reasoning => {
                self.reasoning_message_index_by_item.get(item_id).copied()
            }
            CodexMessageRole::System | CodexMessageRole::User => None,
        };

        if let Some(index) = maybe_index {
            if let Some(entry) = self.messages.get_mut(index) {
                let merged = merge_streaming_text(&entry.text, &normalized_delta);
                if merged == entry.text {
                    return false;
                }
                entry.text = merged;
                entry.streaming = true;
                return true;
            }
        }

        let id = format!(
            "stream:{}:{}",
            match role {
                CodexMessageRole::Assistant => "assistant",
                CodexMessageRole::Reasoning => "reasoning",
                CodexMessageRole::System => "system",
                CodexMessageRole::User => "user",
            },
            item_id
        );
        self.push_message(role.clone(), normalized_delta, true, Some(id));
        let index = self.messages.len().saturating_sub(1);
        match role {
            CodexMessageRole::Assistant => {
                self.assistant_message_index_by_item
                    .insert(item_id.to_string(), index);
            }
            CodexMessageRole::Reasoning => {
                self.reasoning_message_index_by_item
                    .insert(item_id.to_string(), index);
            }
            CodexMessageRole::System | CodexMessageRole::User => {}
        }
        true
    }

    fn mark_all_streaming_complete(&mut self) -> bool {
        let mut changed = false;
        for message in &mut self.messages {
            if message.streaming {
                message.streaming = false;
                changed = true;
            }
        }
        if changed {
            self.assistant_message_index_by_item.clear();
            self.reasoning_message_index_by_item.clear();
        }
        changed
    }

    fn mark_completed(&mut self, item_id: &str) -> bool {
        let assistant_index = self.assistant_message_index_by_item.get(item_id).copied();
        let reasoning_index = self.reasoning_message_index_by_item.get(item_id).copied();

        let mut changed = false;
        if let Some(index) = assistant_index {
            if let Some(entry) = self.messages.get_mut(index) {
                if entry.streaming {
                    entry.streaming = false;
                    changed = true;
                }
            }
        }

        if let Some(index) = reasoning_index {
            if let Some(entry) = self.messages.get_mut(index) {
                if entry.streaming {
                    entry.streaming = false;
                    changed = true;
                }
            }
        }
        changed
    }

    fn push_message(
        &mut self,
        role: CodexMessageRole,
        text: String,
        streaming: bool,
        explicit_id: Option<String>,
    ) {
        if text.trim().is_empty() {
            return;
        }
        let id = explicit_id.unwrap_or_else(|| {
            let current = self.next_local_id;
            self.next_local_id = self.next_local_id.saturating_add(1);
            format!("msg:{current}")
        });
        self.messages.push(CodexThreadMessage {
            id,
            role,
            text,
            streaming,
        });
    }
}

fn decode_worker_payload(payload: &Value) -> Option<&Map<String, Value>> {
    let object = payload.as_object()?;
    if let Some(inner) = object.get("payload").and_then(Value::as_object) {
        return Some(inner);
    }
    Some(object)
}

fn extract_thread_id(
    worker_payload: &Map<String, Value>,
    params: &Map<String, Value>,
) -> Option<String> {
    normalized_string(worker_payload.get("threadId"))
        .or_else(|| normalized_string(worker_payload.get("thread_id")))
        .or_else(|| normalized_string(params.get("threadId")))
        .or_else(|| normalized_string(params.get("thread_id")))
}

fn extract_item_id(params: &Map<String, Value>) -> Option<String> {
    normalized_string(params.get("itemId"))
        .or_else(|| normalized_string(params.get("item_id")))
        .or_else(|| {
            params
                .get("msg")
                .and_then(Value::as_object)
                .and_then(|msg| {
                    normalized_string(msg.get("itemId"))
                        .or_else(|| normalized_string(msg.get("item_id")))
                })
        })
}

fn extract_user_message_text(params: &Map<String, Value>) -> Option<String> {
    params
        .get("msg")
        .and_then(Value::as_object)
        .and_then(|msg| normalized_string(msg.get("text")))
        .or_else(|| normalized_string(params.get("text")))
        .or_else(|| normalized_string(params.get("message")))
}

fn extract_agent_message_text(params: &Map<String, Value>) -> Option<String> {
    params
        .get("msg")
        .and_then(Value::as_object)
        .and_then(|msg| normalized_string(msg.get("text")))
        .or_else(|| {
            params
                .get("message")
                .and_then(Value::as_object)
                .and_then(|message| normalized_string(message.get("text")))
        })
        .or_else(|| normalized_string(params.get("text")))
}

fn extract_agent_delta(params: &Map<String, Value>) -> Option<String> {
    params
        .get("msg")
        .and_then(Value::as_object)
        .and_then(|msg| normalized_string(msg.get("delta")))
        .or_else(|| normalized_string(params.get("delta")))
}

fn extract_error_message(params: &Map<String, Value>) -> Option<String> {
    normalized_string(params.get("message"))
        .or_else(|| normalized_string(params.get("error")))
        .or_else(|| {
            params
                .get("error")
                .and_then(Value::as_object)
                .and_then(|error| normalized_string(error.get("message")))
        })
}

fn normalized_string(value: Option<&Value>) -> Option<String> {
    let value = value?;
    let raw = value.as_str()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_text(value: &str) -> String {
    value.trim().to_string()
}

fn merge_streaming_text(current: &str, delta: &str) -> String {
    if current.is_empty() {
        return delta.to_string();
    }
    if delta.is_empty() {
        return current.to_string();
    }
    if current.ends_with(delta) {
        return current.to_string();
    }

    let max_overlap = current.len().min(delta.len());
    for overlap in (1..=max_overlap).rev() {
        let current_start = current.len().saturating_sub(overlap);
        if !current.is_char_boundary(current_start) || !delta.is_char_boundary(overlap) {
            continue;
        }
        if current[current_start..] == delta[..overlap] {
            return format!("{current}{}", &delta[overlap..]);
        }
    }

    format!("{current}{delta}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn suppresses_noisy_thread_started_event() {
        let mut state = CodexThreadState::default();
        let changed = state.ingest_khala_payload(&json!({
            "eventType": "worker.event",
            "payload": {
                "method": "thread/started",
                "threadId": "thread-1",
                "params": {}
            }
        }));

        assert!(!changed);
        assert!(state.messages.is_empty());
    }

    #[test]
    fn appends_turn_system_events() {
        let mut state = CodexThreadState::default();
        assert!(state.ingest_khala_payload(&json!({
            "eventType": "worker.event",
            "payload": {
                "method": "turn/started",
                "threadId": "thread-1",
                "params": {}
            }
        })));
        assert!(state.ingest_khala_payload(&json!({
            "eventType": "worker.event",
            "payload": {
                "method": "turn/completed",
                "threadId": "thread-1",
                "params": {}
            }
        })));
        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[0].text, "Turn started.");
        assert_eq!(state.messages[1].text, "Turn completed.");
    }

    #[test]
    fn dedupes_local_and_stream_user_messages() {
        let mut state = CodexThreadState::default();
        assert!(state.append_local_user_message("who are you?"));
        assert!(state.messages.len() == 1);
        let changed = state.ingest_khala_payload(&json!({
            "eventType": "worker.event",
            "payload": {
                "method": "codex/event/user_message",
                "threadId": "thread-1",
                "params": {
                    "msg": {
                        "text": "who are you?"
                    }
                }
            }
        }));
        assert!(!changed);
        assert_eq!(state.messages.len(), 1);
    }

    #[test]
    fn merges_streaming_deltas_without_overlap_duplication() {
        let mut state = CodexThreadState::default();
        assert!(state.ingest_khala_payload(&json!({
            "eventType": "worker.event",
            "payload": {
                "method": "codex/event/agent_message_content_delta",
                "threadId": "thread-1",
                "params": {
                    "msg": { "item_id": "item-1", "delta": "Doing well." }
                }
            }
        })));
        assert!(state.ingest_khala_payload(&json!({
            "eventType": "worker.event",
            "payload": {
                "method": "codex/event/agent_message_content_delta",
                "threadId": "thread-1",
                "params": {
                    "msg": { "item_id": "item-1", "delta": "well. How can I help today?" }
                }
            }
        })));
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, CodexMessageRole::Assistant);
        assert_eq!(state.messages[0].text, "Doing well. How can I help today?");
        assert!(state.messages[0].streaming);
    }

    #[test]
    fn reasoning_delta_tracks_reasoning_lane() {
        let mut state = CodexThreadState::default();
        assert!(state.ingest_khala_payload(&json!({
            "eventType": "worker.event",
            "payload": {
                "method": "item/reasoning/textDelta",
                "threadId": "thread-1",
                "params": {
                    "itemId": "reason-1",
                    "delta": "Requesting brief identity..."
                }
            }
        })));

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].role, CodexMessageRole::Reasoning);
        assert_eq!(state.messages[0].text, "Requesting brief identity...");
    }

    #[test]
    fn hydrate_history_seeds_empty_thread() {
        let mut state = CodexThreadState::default();
        let changed = state.hydrate_history_if_empty(
            Some("thread-1".to_string()),
            vec![
                CodexThreadMessage {
                    id: "msg-1".to_string(),
                    role: CodexMessageRole::User,
                    text: "hello".to_string(),
                    streaming: false,
                },
                CodexThreadMessage {
                    id: "msg-2".to_string(),
                    role: CodexMessageRole::Assistant,
                    text: "hi there".to_string(),
                    streaming: false,
                },
            ],
        );

        assert!(changed);
        assert_eq!(state.thread_id.as_deref(), Some("thread-1"));
        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[0].text, "hello");
        assert_eq!(state.messages[1].text, "hi there");
    }

    #[test]
    fn hydrate_history_does_not_overwrite_existing_messages() {
        let mut state = CodexThreadState::default();
        state.set_thread_id(Some("thread-1".to_string()));
        assert!(state.append_local_user_message("pending local message"));

        let changed = state.hydrate_history_if_empty(
            Some("thread-1".to_string()),
            vec![CodexThreadMessage {
                id: "msg-1".to_string(),
                role: CodexMessageRole::Assistant,
                text: "history response".to_string(),
                streaming: false,
            }],
        );

        assert!(!changed);
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].text, "pending local message");
    }

    #[test]
    fn vercel_sse_wire_lifecycle_start_delta_finish() {
        let mut state = CodexThreadState::default();
        let changed = state.ingest_vercel_sse_wire(
            "data: {\"type\":\"start\",\"threadId\":\"thread-1\"}\n\n\
             data: {\"type\":\"start-step\"}\n\n\
             data: {\"type\":\"text-delta\",\"id\":\"item-1\",\"channel\":\"assistant\",\"delta\":\"Hello\"}\n\n\
             data: {\"type\":\"finish-step\"}\n\n\
             data: {\"type\":\"finish\",\"status\":\"completed\"}\n\n\
             data: [DONE]\n\n",
        );

        assert!(changed > 0);
        assert_eq!(state.thread_id.as_deref(), Some("thread-1"));
        assert_eq!(state.messages[0].text, "Turn started.");
        assert_eq!(state.messages[1].role, CodexMessageRole::Assistant);
        assert_eq!(state.messages[1].text, "Hello");
        assert!(!state.messages[1].streaming);
        assert_eq!(state.messages[2].text, "Turn completed.");
    }

    #[test]
    fn vercel_sse_wire_lifecycle_tool_events() {
        let mut state = CodexThreadState::default();
        let changed = state.ingest_vercel_sse_wire(
            "data: {\"type\":\"start\",\"threadId\":\"thread-2\"}\n\n\
             data: {\"type\":\"start-step\"}\n\n\
             data: {\"type\":\"tool-input\",\"toolName\":\"web.search\"}\n\n\
             data: {\"type\":\"tool-output\",\"status\":\"completed\"}\n\n\
             data: {\"type\":\"finish-step\"}\n\n\
             data: {\"type\":\"finish\",\"status\":\"completed\"}\n\n\
             data: [DONE]\n\n",
        );

        assert!(changed > 0);
        let tool_started = state
            .messages
            .iter()
            .any(|message| message.text == "Tool started: web.search");
        let tool_completed = state
            .messages
            .iter()
            .any(|message| message.text == "Tool completed.");
        assert!(tool_started);
        assert!(tool_completed);
    }

    #[test]
    fn vercel_sse_wire_lifecycle_error() {
        let mut state = CodexThreadState::default();
        let changed = state.ingest_vercel_sse_wire(
            "data: {\"type\":\"start\",\"threadId\":\"thread-3\"}\n\n\
             data: {\"type\":\"start-step\"}\n\n\
             data: {\"type\":\"text-delta\",\"id\":\"item-err\",\"channel\":\"assistant\",\"delta\":\"Partial\"}\n\n\
             data: {\"type\":\"error\",\"message\":\"runtime unavailable\"}\n\n\
             data: {\"type\":\"finish\",\"status\":\"error\"}\n\n\
             data: [DONE]\n\n",
        );

        assert!(changed > 0);
        assert!(
            state
                .messages
                .iter()
                .any(|message| message.text == "runtime unavailable")
        );
        assert!(
            state
                .messages
                .iter()
                .any(|message| message.text == "Turn failed.")
        );
        let assistant = state
            .messages
            .iter()
            .find(|message| message.role == CodexMessageRole::Assistant)
            .expect("assistant stream message");
        assert!(!assistant.streaming);
    }
}
