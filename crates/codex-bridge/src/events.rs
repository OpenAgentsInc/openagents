use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Canonical event envelope the bridge emits to the app, independent of provider.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type")]
#[ts(export, export_to = "../../expo/lib/generated/")]
pub enum ThreadEvent {
    #[serde(rename = "thread.started")]
    ThreadStarted { thread_id: String },
    #[serde(rename = "turn.started")]
    TurnStarted {},
    #[serde(rename = "turn.completed")]
    TurnCompleted { usage: Usage },
    #[serde(rename = "turn.failed")]
    TurnFailed { error: ThreadError },
    #[serde(rename = "item.started")]
    ItemStarted { item: ThreadItem },
    #[serde(rename = "item.updated")]
    ItemUpdated { item: ThreadItem },
    #[serde(rename = "item.completed")]
    ItemCompleted { item: ThreadItem },
    #[serde(rename = "error")]
    Error { message: String },
}

/// Canonical thread item variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThreadItem {
    AgentMessage { id: String, text: String },
    Reasoning { id: String, text: String },
    CommandExecution {
        id: String,
        command: String,
        aggregated_output: String,
        exit_code: Option<i32>,
        status: CommandStatus,
    },
    FileChange {
        id: String,
        changes: Vec<FileUpdateChange>,
        status: PatchApplyStatus,
    },
    McpToolCall { id: String, server: String, tool: String, status: ToolCallStatus },
    WebSearch { id: String, query: String },
    TodoList { id: String, items: Vec<TodoItem> },
}

/// Command execution lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    InProgress,
    Completed,
    Failed,
}

/// Patch apply status for file changes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplyStatus {
    Completed,
    Failed,
}

/// Tool call lifecycle status (for MCP and similar calls).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

/// A single file change entry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct FileUpdateChange {
    pub path: String,
    pub kind: PatchChangeKind,
}

/// Kind of change applied to a file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchChangeKind {
    Add,
    Delete,
    Update,
}

/// Todo list item.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct TodoItem {
    pub text: String,
    pub completed: bool,
}

/// Token usage accounting.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct Usage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
}

/// Error payload for failed turns or errors.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct ThreadError {
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_thread_started_round_trip() {
        let ev = ThreadEvent::ThreadStarted { thread_id: "abc".into() };
        let json = serde_json::to_value(&ev).expect("serialize");
        assert_eq!(json.get("type").and_then(|x| x.as_str()), Some("thread.started"));
        assert_eq!(json.get("thread_id").and_then(|x| x.as_str()), Some("abc"));
        let back: ThreadEvent = serde_json::from_value(json).expect("deserialize");
        match back {
            ThreadEvent::ThreadStarted { thread_id } => assert_eq!(thread_id, "abc"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn item_command_execution_serialization() {
        let it = ThreadItem::CommandExecution {
            id: "i1".into(),
            command: "echo hi".into(),
            aggregated_output: "hi".into(),
            exit_code: Some(0),
            status: CommandStatus::InProgress,
        };
        let json = serde_json::to_value(&it).expect("serialize");
        assert_eq!(json.get("type").and_then(|x| x.as_str()), Some("command_execution"));
        assert_eq!(json.get("status").and_then(|x| x.as_str()), Some("in_progress"));
        assert_eq!(json.get("exit_code").and_then(|x| x.as_i64()), Some(0));
        let back: ThreadItem = serde_json::from_value(json).expect("deserialize");
        match back {
            ThreadItem::CommandExecution { id, command, aggregated_output, exit_code, status } => {
                assert_eq!(id, "i1");
                assert_eq!(command, "echo hi");
                assert_eq!(aggregated_output, "hi");
                assert_eq!(exit_code, Some(0));
                matches!(status, CommandStatus::InProgress);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn item_file_change_serialization() {
        let it = ThreadItem::FileChange {
            id: "f1".into(),
            changes: vec![
                FileUpdateChange { path: "a.txt".into(), kind: PatchChangeKind::Add },
                FileUpdateChange { path: "b.txt".into(), kind: PatchChangeKind::Update },
            ],
            status: PatchApplyStatus::Completed,
        };
        let json = serde_json::to_value(&it).expect("serialize");
        assert_eq!(json.get("type").and_then(|x| x.as_str()), Some("file_change"));
        assert_eq!(json.get("status").and_then(|x| x.as_str()), Some("completed"));
        let changes = json.get("changes").and_then(|x| x.as_array()).expect("changes arr");
        assert_eq!(changes[0].get("kind").and_then(|x| x.as_str()), Some("add"));
        assert_eq!(changes[1].get("kind").and_then(|x| x.as_str()), Some("update"));
        let back: ThreadItem = serde_json::from_value(json).expect("deserialize");
        match back { ThreadItem::FileChange { id, changes, status } => {
            assert_eq!(id, "f1");
            assert_eq!(changes.len(), 2);
            matches!(status, PatchApplyStatus::Completed);
        }, _ => panic!("wrong variant") }
    }

    #[test]
    fn event_turn_completed_with_usage() {
        let ev = ThreadEvent::TurnCompleted { usage: Usage { input_tokens: 10, cached_input_tokens: 2, output_tokens: 7 } };
        let json = serde_json::to_value(&ev).expect("serialize");
        assert_eq!(json.get("type").and_then(|x| x.as_str()), Some("turn.completed"));
        let usage = json.get("usage").expect("usage");
        assert_eq!(usage.get("input_tokens").and_then(|x| x.as_i64()), Some(10));
        let back: ThreadEvent = serde_json::from_value(json).expect("deserialize");
        match back { ThreadEvent::TurnCompleted { usage } => {
            assert_eq!(usage.input_tokens, 10);
            assert_eq!(usage.cached_input_tokens, 2);
            assert_eq!(usage.output_tokens, 7);
        }, _ => panic!("wrong variant") }
    }

    #[test]
    fn event_error_round_trip() {
        let ev = ThreadEvent::Error { message: "boom".into() };
        let json = serde_json::to_value(&ev).expect("serialize");
        assert_eq!(json.get("type").and_then(|x| x.as_str()), Some("error"));
        assert_eq!(json.get("message").and_then(|x| x.as_str()), Some("boom"));
        let back: ThreadEvent = serde_json::from_value(json).expect("deserialize");
        match back { ThreadEvent::Error { message } => assert_eq!(message, "boom"), _ => panic!("wrong variant") }
    }
}
