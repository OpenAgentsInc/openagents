//! Thread item types for the Codex Agent SDK.
//!
//! These types represent the various items that can appear in a thread,
//! such as agent messages, command executions, file changes, and more.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// A single item in the thread.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadItem {
    /// Unique identifier for this item.
    pub id: String,

    /// The typed payload for this item.
    #[serde(flatten)]
    pub details: ThreadItemDetails,
}

/// Typed payloads for each supported thread item type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThreadItemDetails {
    /// Response from the agent.
    AgentMessage(AgentMessageItem),

    /// Agent's reasoning summary.
    Reasoning(ReasoningItem),

    /// A command executed by the agent.
    CommandExecution(CommandExecutionItem),

    /// A set of file changes by the agent.
    FileChange(FileChangeItem),

    /// A call to an MCP tool.
    McpToolCall(McpToolCallItem),

    /// A web search request.
    WebSearch(WebSearchItem),

    /// Agent's running to-do list.
    TodoList(TodoListItem),

    /// A non-fatal error surfaced as an item.
    Error(ErrorItem),
}

/// Response from the agent.
///
/// Either a natural-language response or a JSON string when structured output is requested.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentMessageItem {
    /// The text content of the agent's message.
    pub text: String,
}

/// Agent's reasoning summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReasoningItem {
    /// The reasoning text.
    pub text: String,
}

/// Status of a command execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutionStatus {
    /// The command is currently running.
    #[default]
    InProgress,
    /// The command completed successfully.
    Completed,
    /// The command failed.
    Failed,
    /// The command was declined by the user.
    Declined,
}

/// A command executed by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandExecutionItem {
    /// The command that was executed.
    pub command: String,

    /// Aggregated stdout/stderr output from the command.
    pub aggregated_output: String,

    /// Exit code of the command (when completed).
    pub exit_code: Option<i32>,

    /// Current status of the command execution.
    pub status: CommandExecutionStatus,
}

/// A single file change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileUpdateChange {
    /// Path to the file that was changed.
    pub path: String,

    /// Type of change (add, delete, update).
    pub kind: PatchChangeKind,
}

/// Type of file change.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatchChangeKind {
    /// A new file was added.
    Add,
    /// A file was deleted.
    Delete,
    /// A file was updated.
    Update,
}

/// Status of a file change operation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplyStatus {
    /// The patch is being applied.
    #[default]
    InProgress,
    /// The patch was applied successfully.
    Completed,
    /// The patch failed to apply.
    Failed,
}

/// A set of file changes by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileChangeItem {
    /// List of file changes.
    pub changes: Vec<FileUpdateChange>,

    /// Current status of the patch application.
    pub status: PatchApplyStatus,
}

/// Status of an MCP tool call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallStatus {
    /// The tool call is in progress.
    #[default]
    InProgress,
    /// The tool call completed successfully.
    Completed,
    /// The tool call failed.
    Failed,
}

/// Result payload from an MCP tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolCallItemResult {
    /// Content blocks from the tool response.
    pub content: Vec<JsonValue>,

    /// Structured content if available.
    pub structured_content: Option<JsonValue>,
}

/// Error details from a failed MCP tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolCallItemError {
    /// Error message.
    pub message: String,
}

/// A call to an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolCallItem {
    /// Name of the MCP server.
    pub server: String,

    /// Name of the tool being called.
    pub tool: String,

    /// Arguments passed to the tool.
    #[serde(default)]
    pub arguments: JsonValue,

    /// Result of the tool call (when completed).
    pub result: Option<McpToolCallItemResult>,

    /// Error from the tool call (when failed).
    pub error: Option<McpToolCallItemError>,

    /// Current status of the tool call.
    pub status: McpToolCallStatus,
}

/// A web search request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebSearchItem {
    /// The search query.
    pub query: String,
}

/// A single to-do item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoItem {
    /// Text description of the to-do item.
    pub text: String,

    /// Whether the item has been completed.
    pub completed: bool,
}

/// Agent's running to-do list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoListItem {
    /// List of to-do items.
    pub items: Vec<TodoItem>,
}

/// A non-fatal error notification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorItem {
    /// Error message.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_agent_message() {
        let json = r#"{"id":"item_0","type":"agent_message","text":"Hello, world!"}"#;
        let item: ThreadItem = serde_json::from_str(json).unwrap();

        assert_eq!(item.id, "item_0");
        match item.details {
            ThreadItemDetails::AgentMessage(msg) => {
                assert_eq!(msg.text, "Hello, world!");
            }
            _ => panic!("Expected AgentMessage"),
        }
    }

    #[test]
    fn test_deserialize_command_execution() {
        let json = r#"{
            "id": "item_1",
            "type": "command_execution",
            "command": "ls -la",
            "aggregated_output": "total 0\n",
            "exit_code": 0,
            "status": "completed"
        }"#;
        let item: ThreadItem = serde_json::from_str(json).unwrap();

        match item.details {
            ThreadItemDetails::CommandExecution(cmd) => {
                assert_eq!(cmd.command, "ls -la");
                assert_eq!(cmd.exit_code, Some(0));
                assert_eq!(cmd.status, CommandExecutionStatus::Completed);
            }
            _ => panic!("Expected CommandExecution"),
        }
    }

    #[test]
    fn test_deserialize_file_change() {
        let json = r#"{
            "id": "item_2",
            "type": "file_change",
            "changes": [{"path": "foo.txt", "kind": "add"}],
            "status": "completed"
        }"#;
        let item: ThreadItem = serde_json::from_str(json).unwrap();

        match item.details {
            ThreadItemDetails::FileChange(fc) => {
                assert_eq!(fc.changes.len(), 1);
                assert_eq!(fc.changes[0].path, "foo.txt");
                assert_eq!(fc.changes[0].kind, PatchChangeKind::Add);
            }
            _ => panic!("Expected FileChange"),
        }
    }
}
