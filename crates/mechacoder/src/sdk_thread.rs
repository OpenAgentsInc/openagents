//! SDK Thread - wraps Claude Agent SDK for GPUI integration.

use claude_agent_sdk::{
    query, QueryOptions, SdkMessage, SdkResultMessage, SdkSystemMessage,
};
use futures::StreamExt;
use gpui::{Context, EventEmitter, Task};
use gpui_tokio::Tokio;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Events emitted by SdkThread.
#[derive(Clone, Debug)]
pub enum SdkThreadEvent {
    /// A new entry was added.
    EntryAdded(usize),
    /// An entry was updated.
    EntryUpdated(usize),
    /// Thread status changed.
    StatusChanged(ThreadStatus),
    /// Todo list was updated (plan mode).
    TodosUpdated,
    /// Error occurred.
    Error(String),
}

/// Status of the thread.
#[derive(Clone, Debug, Default)]
pub enum ThreadStatus {
    #[default]
    Idle,
    Streaming,
    Completed,
    Error(String),
}

/// Entry in the conversation thread.
#[derive(Clone, Debug)]
pub enum ThreadEntry {
    UserMessage(UserMessage),
    AssistantMessage(AssistantMessage),
    ToolUse(ToolUse),
}

/// User message.
#[derive(Clone, Debug)]
pub struct UserMessage {
    pub content: String,
}

/// Assistant message.
#[derive(Clone, Debug)]
pub struct AssistantMessage {
    pub content: String,
}

/// Tool use entry.
#[derive(Clone, Debug)]
pub struct ToolUse {
    /// Unique tool use ID from Claude.
    pub tool_use_id: String,
    /// Name of the tool being used.
    pub tool_name: String,
    /// JSON input for the tool.
    pub input: String,
    /// Output from tool execution.
    pub output: Option<String>,
    /// Current status of the tool.
    pub status: ToolStatus,
}

/// Tool status.
#[derive(Clone, Debug, Default)]
pub enum ToolStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Failed(String),
}

/// Todo item status for plan mode.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum TodoStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
}

/// Single todo item from TodoWrite tool.
#[derive(Clone, Debug)]
pub struct TodoItem {
    /// The task description (imperative form).
    pub content: String,
    /// The active form shown during execution.
    pub active_form: String,
    /// Current status.
    pub status: TodoStatus,
}

impl TodoItem {
    /// Parse todo items from TodoWrite tool input JSON.
    pub fn parse_from_json(input: &str) -> Option<Vec<Self>> {
        let parsed: serde_json::Value = serde_json::from_str(input).ok()?;
        let todos = parsed.get("todos")?.as_array()?;
        Some(
            todos
                .iter()
                .filter_map(|item| {
                    Some(TodoItem {
                        content: item.get("content")?.as_str()?.to_string(),
                        active_form: item.get("activeForm")?.as_str()?.to_string(),
                        status: match item.get("status")?.as_str()? {
                            "in_progress" => TodoStatus::InProgress,
                            "completed" => TodoStatus::Completed,
                            _ => TodoStatus::Pending,
                        },
                    })
                })
                .collect(),
        )
    }
}

/// Current todo list state.
#[derive(Clone, Debug, Default)]
pub struct TodoState {
    /// All todo items (complete replacement on each update).
    pub items: Vec<TodoItem>,
}

/// Internal message from Tokio task to GPUI.
#[derive(Clone, Debug)]
enum SdkUpdate {
    Init { session_id: String, model: String },
    StreamingContent(String),
    /// Tool use started from stream event content_block_start
    ToolUseStarted {
        tool_use_id: String,
        tool_name: String,
    },
    /// Tool input streaming from stream event input_json_delta
    ToolInputDelta {
        tool_use_id: String,
        partial_json: String,
    },
    /// Tool progress from tool_progress message
    ToolProgress {
        tool_use_id: String,
        tool_name: String,
        elapsed_seconds: f64,
    },
    /// Tool result from user message (tool_result)
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
    },
    FinalContent(String),
    Completed,
    Error(String),
}

/// A conversation thread using the Claude Agent SDK.
pub struct SdkThread {
    /// Project root directory.
    project_root: PathBuf,
    /// Thread entries.
    entries: Vec<ThreadEntry>,
    /// Current status.
    status: ThreadStatus,
    /// Current streaming content.
    streaming_content: Option<String>,
    /// Session ID (set after first message).
    session_id: Option<String>,
    /// Model being used.
    model: Option<String>,
    /// Map from tool_use_id to entry index for quick lookups.
    tool_use_index: HashMap<String, usize>,
    /// Current todo list state (plan mode).
    todo_state: TodoState,
}

impl SdkThread {
    /// Create a new SDK thread.
    pub fn new(project_root: PathBuf, _cx: &mut Context<Self>) -> Self {
        Self {
            project_root,
            entries: Vec::new(),
            status: ThreadStatus::Idle,
            streaming_content: None,
            session_id: None,
            model: None,
            tool_use_index: HashMap::new(),
            todo_state: TodoState::default(),
        }
    }

    /// Get the project root.
    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }

    /// Get all entries.
    pub fn entries(&self) -> &[ThreadEntry] {
        &self.entries
    }

    /// Get the current status.
    pub fn status(&self) -> &ThreadStatus {
        &self.status
    }

    /// Get the current streaming content.
    pub fn streaming_content(&self) -> Option<&str> {
        self.streaming_content.as_deref()
    }

    /// Get the session ID.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Get the current todo state.
    pub fn todo_state(&self) -> &TodoState {
        &self.todo_state
    }

    /// Check if there are active todos.
    pub fn has_todos(&self) -> bool {
        !self.todo_state.items.is_empty()
    }

    /// Send a user message.
    pub fn send_message(&mut self, content: String, cx: &mut Context<Self>) -> Task<()> {
        // Add user message to entries
        self.entries.push(ThreadEntry::UserMessage(UserMessage {
            content: content.clone(),
        }));
        let entry_idx = self.entries.len() - 1;
        cx.emit(SdkThreadEvent::EntryAdded(entry_idx));

        // Update status
        self.status = ThreadStatus::Streaming;
        self.streaming_content = Some(String::new());
        cx.emit(SdkThreadEvent::StatusChanged(self.status.clone()));

        let project_root = self.project_root.clone();

        // Create channel for SDK updates
        let (tx, mut rx) = mpsc::unbounded_channel::<SdkUpdate>();

        // Spawn SDK work on Tokio runtime - must detach to prevent abort on drop
        info!("Spawning SDK query for: {}", content);
        Tokio::spawn(cx, async move {
            info!("Tokio task started");
            // Build options - only use dangerously_skip_permissions (don't also set permission_mode)
            let options = QueryOptions::new()
                .cwd(&project_root)
                .dangerously_skip_permissions(true)
                .include_partial_messages(true);

            // Create query
            info!("Creating query...");
            let query_result = query(&content, options).await;
            info!("Query created: {:?}", query_result.is_ok());

            let mut stream = match query_result {
                Ok(q) => q,
                Err(e) => {
                    let _ = tx.send(SdkUpdate::Error(e.to_string()));
                    return;
                }
            };

            let mut assistant_content = String::new();

            // Process stream
            info!("Starting to process stream...");
            while let Some(msg_result) = stream.next().await {
                info!("Got message from stream");
                let msg = match msg_result {
                    Ok(m) => m,
                    Err(e) => {
                        info!("SDK error: {}", e);
                        let _ = tx.send(SdkUpdate::Error(e.to_string()));
                        return;
                    }
                };

                debug!("SDK message: {:?}", msg);

                match msg {
                    SdkMessage::System(sys) => {
                        if let SdkSystemMessage::Init(init) = sys {
                            let _ = tx.send(SdkUpdate::Init {
                                session_id: init.session_id,
                                model: init.model,
                            });
                        }
                    }
                    SdkMessage::Assistant(assistant) => {
                        // Extract text and tool_use inputs from message
                        info!("Assistant message: {}", assistant.message);
                        if let Some(content) = assistant.message.get("content") {
                            if let Some(blocks) = content.as_array() {
                                for block in blocks {
                                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                    match block_type {
                                        "text" => {
                                            // Only extract text if we haven't accumulated from streaming
                                            if assistant_content.is_empty() {
                                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                                    assistant_content.push_str(text);
                                                }
                                            }
                                        }
                                        "tool_use" => {
                                            // Extract tool input for later use (e.g., TodoWrite parsing)
                                            let tool_use_id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                                            if let Some(input) = block.get("input") {
                                                let input_str = serde_json::to_string(input).unwrap_or_default();
                                                info!("Tool input for {}: {}", tool_use_id, input_str);
                                                let _ = tx.send(SdkUpdate::ToolInputDelta {
                                                    tool_use_id: tool_use_id.to_string(),
                                                    partial_json: input_str,
                                                });
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        if !assistant_content.is_empty() {
                            info!("Extracted content from assistant: {}", assistant_content);
                            let _ = tx.send(SdkUpdate::StreamingContent(assistant_content.clone()));
                        }
                    }
                    SdkMessage::StreamEvent(event) => {
                        // Handle streaming events
                        let event_type = event.event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        debug!("Stream event type: {}", event_type);

                        match event_type {
                            "content_block_start" => {
                                // Check if this is a tool_use block starting
                                if let Some(content_block) = event.event.get("content_block") {
                                    let block_type = content_block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                    if block_type == "tool_use" {
                                        let tool_use_id = content_block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                                        let tool_name = content_block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                        info!("Tool use started: {} ({})", tool_name, tool_use_id);
                                        let _ = tx.send(SdkUpdate::ToolUseStarted {
                                            tool_use_id: tool_use_id.to_string(),
                                            tool_name: tool_name.to_string(),
                                        });
                                    }
                                }
                            }
                            "content_block_delta" => {
                                if let Some(delta) = event.event.get("delta") {
                                    let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                    match delta_type {
                                        "text_delta" => {
                                            // Regular text streaming
                                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                                assistant_content.push_str(text);
                                                debug!("Streaming content now: {}", assistant_content);
                                                let _ = tx.send(SdkUpdate::StreamingContent(assistant_content.clone()));
                                            }
                                        }
                                        "input_json_delta" => {
                                            // Tool input streaming - could track partial JSON here
                                            if let Some(partial_json) = delta.get("partial_json").and_then(|p| p.as_str()) {
                                                debug!("Tool input delta: {}", partial_json);
                                                // We don't track partial tool input for now
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            _ => {
                                // Log other stream event types
                                debug!("Other stream event: {:?}", event.event);
                            }
                        }
                    }
                    SdkMessage::User(user_msg) => {
                        // Handle user message echos (including tool results)
                        if let Some(content) = user_msg.message.get("content") {
                            if let Some(blocks) = content.as_array() {
                                for block in blocks {
                                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                    if block_type == "tool_result" {
                                        let tool_use_id = block.get("tool_use_id").and_then(|i| i.as_str()).unwrap_or("");
                                        let is_error = block.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false);
                                        let output = block.get("content").and_then(|c| c.as_str()).unwrap_or("");
                                        info!("Tool result for {}: {} (error: {})", tool_use_id, output.len(), is_error);
                                        let _ = tx.send(SdkUpdate::ToolResult {
                                            tool_use_id: tool_use_id.to_string(),
                                            output: output.to_string(),
                                            is_error,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    SdkMessage::ToolProgress(progress) => {
                        info!("Tool progress: {} ({})", progress.tool_name, progress.tool_use_id);
                        let _ = tx.send(SdkUpdate::ToolProgress {
                            tool_use_id: progress.tool_use_id,
                            tool_name: progress.tool_name,
                            elapsed_seconds: progress.elapsed_time_seconds,
                        });
                    }
                    SdkMessage::Result(result) => {
                        if !assistant_content.is_empty() {
                            let _ = tx.send(SdkUpdate::FinalContent(assistant_content.clone()));
                        }
                        match result {
                            SdkResultMessage::Success(_) => {
                                let _ = tx.send(SdkUpdate::Completed);
                            }
                            _ => {
                                let _ = tx.send(SdkUpdate::Error("Query failed".to_string()));
                            }
                        }
                    }
                    _ => {}
                }
            }

            // If stream ends without explicit result, send what we have
            if !assistant_content.is_empty() {
                let _ = tx.send(SdkUpdate::FinalContent(assistant_content));
            }
            let _ = tx.send(SdkUpdate::Completed);
        })
        .detach();

        // Spawn GPUI task to receive updates
        cx.spawn(async move |this, cx| {
            while let Some(update) = rx.recv().await {
                let should_break = matches!(update, SdkUpdate::Completed | SdkUpdate::Error(_));

                let _ = this.update(cx, |this, cx| {
                    match update {
                        SdkUpdate::Init { session_id, model } => {
                            this.session_id = Some(session_id);
                            this.model = Some(model);
                        }
                        SdkUpdate::StreamingContent(content) => {
                            this.streaming_content = Some(content);
                            cx.notify();
                        }
                        SdkUpdate::ToolUseStarted { tool_use_id, tool_name } => {
                            // Create a new tool use entry
                            let tool_entry = ThreadEntry::ToolUse(ToolUse {
                                tool_use_id: tool_use_id.clone(),
                                tool_name,
                                input: String::new(),
                                output: None,
                                status: ToolStatus::Running,
                            });
                            this.entries.push(tool_entry);
                            let idx = this.entries.len() - 1;
                            this.tool_use_index.insert(tool_use_id, idx);
                            cx.emit(SdkThreadEvent::EntryAdded(idx));
                        }
                        SdkUpdate::ToolInputDelta { tool_use_id, partial_json } => {
                            // Update tool input (if we want to show partial input)
                            if let Some(&idx) = this.tool_use_index.get(&tool_use_id) {
                                if let ThreadEntry::ToolUse(tool_use) = &mut this.entries[idx] {
                                    tool_use.input.push_str(&partial_json);
                                    cx.emit(SdkThreadEvent::EntryUpdated(idx));
                                }
                            }
                        }
                        SdkUpdate::ToolProgress { tool_use_id, tool_name, elapsed_seconds: _ } => {
                            // Update or create tool entry based on tool_progress
                            if let Some(&idx) = this.tool_use_index.get(&tool_use_id) {
                                // Already have this tool use, just update status
                                if let ThreadEntry::ToolUse(tool_use) = &mut this.entries[idx] {
                                    tool_use.status = ToolStatus::Running;
                                    cx.emit(SdkThreadEvent::EntryUpdated(idx));
                                }
                            } else {
                                // Create new tool entry from progress
                                let tool_entry = ThreadEntry::ToolUse(ToolUse {
                                    tool_use_id: tool_use_id.clone(),
                                    tool_name,
                                    input: String::new(),
                                    output: None,
                                    status: ToolStatus::Running,
                                });
                                this.entries.push(tool_entry);
                                let idx = this.entries.len() - 1;
                                this.tool_use_index.insert(tool_use_id, idx);
                                cx.emit(SdkThreadEvent::EntryAdded(idx));
                            }
                        }
                        SdkUpdate::ToolResult { tool_use_id, output, is_error } => {
                            // Update tool with result
                            if let Some(&idx) = this.tool_use_index.get(&tool_use_id) {
                                if let ThreadEntry::ToolUse(tool_use) = &mut this.entries[idx] {
                                    // Check if this is a TodoWrite tool - parse and update todo state
                                    if tool_use.tool_name == "TodoWrite" {
                                        if let Some(items) = TodoItem::parse_from_json(&tool_use.input) {
                                            info!("Parsed {} todos from TodoWrite", items.len());
                                            this.todo_state = TodoState { items };
                                            cx.emit(SdkThreadEvent::TodosUpdated);
                                        }
                                    }

                                    tool_use.output = Some(output);
                                    tool_use.status = if is_error {
                                        ToolStatus::Failed("Tool execution failed".to_string())
                                    } else {
                                        ToolStatus::Completed
                                    };
                                    cx.emit(SdkThreadEvent::EntryUpdated(idx));
                                }
                            }
                        }
                        SdkUpdate::FinalContent(content) => {
                            this.entries.push(ThreadEntry::AssistantMessage(AssistantMessage {
                                content,
                            }));
                            let idx = this.entries.len() - 1;
                            cx.emit(SdkThreadEvent::EntryAdded(idx));
                        }
                        SdkUpdate::Completed => {
                            this.streaming_content = None;
                            this.status = ThreadStatus::Completed;
                            cx.emit(SdkThreadEvent::StatusChanged(this.status.clone()));
                        }
                        SdkUpdate::Error(e) => {
                            this.streaming_content = None;
                            this.status = ThreadStatus::Error(e.clone());
                            cx.emit(SdkThreadEvent::Error(e));
                            cx.emit(SdkThreadEvent::StatusChanged(this.status.clone()));
                        }
                    }
                });

                if should_break {
                    break;
                }
            }
        })
    }

    /// Cancel the current operation.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        // SDK doesn't have direct cancel yet, just update status
        self.status = ThreadStatus::Idle;
        self.streaming_content = None;
        cx.emit(SdkThreadEvent::StatusChanged(self.status.clone()));
    }
}

impl EventEmitter<SdkThreadEvent> for SdkThread {}
