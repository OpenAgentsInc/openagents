//! SDK Thread - wraps Claude Agent SDK for GPUI integration.

use claude_agent_sdk::{
    query, QueryOptions, SdkMessage, SdkResultMessage, SdkSystemMessage, ModelInfo, AccountInfo,
};
use futures::StreamExt;
use gpui::{Context, EventEmitter, Task};
use gpui_tokio::Tokio;
use harbor::StreamEvent;
use std::collections::HashMap;
use std::path::PathBuf;
use terminalbench::TBRunStatus;
use tokio::sync::mpsc;
use tracing::debug;
use crate::panels::CostTracker;

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
    /// Cost tracking was updated.
    CostUpdated,
    /// Available models were updated.
    ModelsUpdated,
    /// Session ID was updated.
    SessionUpdated,
    /// Account info was updated.
    AccountInfoUpdated,
    /// Tools and MCP servers were updated.
    ToolsUpdated,
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
    /// TB2 run header entry
    TBenchRun(TBenchRunEntry),
    /// TB2 stream event entry
    TBenchEvent(TBenchStreamEntry),
    /// TestGen message entry
    TestGenMessage(TestGenMessageEntry),
}

/// TB2 run header entry - shown when a TB2 run starts
#[derive(Clone, Debug)]
pub struct TBenchRunEntry {
    /// Unique run ID
    pub run_id: String,
    /// Task ID
    pub task_id: String,
    /// Human-readable task name
    pub task_name: String,
    /// Current run status
    pub status: TBRunStatus,
    /// Number of turns completed
    pub turns: u32,
    /// Maximum turns allowed
    pub max_turns: u32,
    /// Cost in USD (if known)
    pub cost: Option<f64>,
    /// Error message (if any)
    pub error: Option<String>,
    /// Docker container ID (if running in container)
    pub container_id: Option<String>,
    /// Docker image name
    pub image_name: Option<String>,
}

/// TB2 stream event entry - individual events from the run
#[derive(Clone, Debug)]
pub struct TBenchStreamEntry {
    /// Run ID this event belongs to
    pub run_id: String,
    /// The stream event from harbor
    pub event: StreamEvent,
}

/// TestGen message entry - messages from TestGen runs
#[derive(Clone, Debug)]
pub struct TestGenMessageEntry {
    /// Run ID this message belongs to
    pub run_id: String,
    /// Message content
    pub message: String,
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
#[allow(dead_code)] // Some fields are captured for future use
enum SdkUpdate {
    Init {
        session_id: String,
        model: String,
        tools: Vec<String>,
        mcp_servers: Vec<(String, String)>,
    },
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
    /// Cost tracking update from result
    CostUpdate {
        total_cost: f64,
        model_usage: HashMap<String, f64>,
        input_tokens: u64,
        output_tokens: u64,
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
    /// Available models from supported_models() call.
    available_models: Vec<ModelInfo>,
    /// Account information from account_info() call.
    account_info: Option<AccountInfo>,
    /// Available tools.
    tools: Vec<String>,
    /// MCP servers (name, status).
    mcp_servers: Vec<(String, String)>,
    /// Map from tool_use_id to entry index for quick lookups.
    tool_use_index: HashMap<String, usize>,
    /// Current todo list state (plan mode).
    todo_state: TodoState,
    /// Cost tracking state.
    cost_tracker: CostTracker,
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
            available_models: Vec::new(),
            account_info: None,
            tools: Vec::new(),
            mcp_servers: Vec::new(),
            tool_use_index: HashMap::new(),
            todo_state: TodoState::default(),
            cost_tracker: CostTracker::default(),
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

    /// Get the cost tracker.
    pub fn cost_tracker(&self) -> &CostTracker {
        &self.cost_tracker
    }

    /// Update cost tracking from SDK result data.
    pub fn update_cost(
        &mut self,
        total: f64,
        model_usage: HashMap<String, f64>,
        input_tokens: u64,
        output_tokens: u64,
        cx: &mut Context<Self>,
    ) {
        self.cost_tracker.total_cost_usd = total;
        self.cost_tracker.model_usage = model_usage;
        self.cost_tracker.total_input_tokens = input_tokens;
        self.cost_tracker.total_output_tokens = output_tokens;
        cx.emit(SdkThreadEvent::CostUpdated);
    }

    /// Get available models.
    pub fn available_models(&self) -> &[ModelInfo] {
        &self.available_models
    }

    /// Update available models list.
    pub fn set_available_models(&mut self, models: Vec<ModelInfo>, cx: &mut Context<Self>) {
        self.available_models = models;
        cx.emit(SdkThreadEvent::ModelsUpdated);
    }

    /// Set the session ID.
    pub fn set_session_id(&mut self, session_id: Option<String>, cx: &mut Context<Self>) {
        self.session_id = session_id;
        cx.emit(SdkThreadEvent::SessionUpdated);
    }

    /// Get the account information.
    pub fn account_info(&self) -> Option<&AccountInfo> {
        self.account_info.as_ref()
    }

    /// Set the account information.
    pub fn set_account_info(&mut self, account_info: Option<AccountInfo>, cx: &mut Context<Self>) {
        self.account_info = account_info;
        cx.emit(SdkThreadEvent::AccountInfoUpdated);
    }

    /// Get the available tools.
    pub fn tools(&self) -> &[String] {
        &self.tools
    }

    /// Get the MCP servers.
    pub fn mcp_servers(&self) -> &[(String, String)] {
        &self.mcp_servers
    }

    /// Set the tools and MCP servers.
    pub fn set_tools_and_mcp(&mut self, tools: Vec<String>, mcp_servers: Vec<(String, String)>, cx: &mut Context<Self>) {
        self.tools = tools;
        self.mcp_servers = mcp_servers;
        cx.emit(SdkThreadEvent::ToolsUpdated);
    }

    /// Add a TB2 run entry to the thread
    pub fn add_tbench_run_entry(&mut self, entry: TBenchRunEntry, cx: &mut Context<Self>) {
        self.entries.push(ThreadEntry::TBenchRun(entry));
        let entry_idx = self.entries.len() - 1;
        cx.emit(SdkThreadEvent::EntryAdded(entry_idx));
    }

    /// Add a TB2 stream event entry to the thread
    pub fn add_tbench_stream_entry(&mut self, entry: TBenchStreamEntry, cx: &mut Context<Self>) {
        self.entries.push(ThreadEntry::TBenchEvent(entry));
        let entry_idx = self.entries.len() - 1;
        cx.emit(SdkThreadEvent::EntryAdded(entry_idx));
    }

    /// Update container info for a TB2 run
    pub fn update_tb2_container_info(
        &mut self,
        run_id: &str,
        container_id: String,
        cx: &mut Context<Self>,
    ) {
        for entry in &mut self.entries {
            if let ThreadEntry::TBenchRun(run_entry) = entry {
                if run_entry.run_id == run_id {
                    run_entry.container_id = Some(container_id);
                    cx.notify();
                    return;
                }
            }
        }
    }

    /// Add a TestGen message to the thread
    pub fn add_testgen_message(&mut self, run_id: &str, message: &str, cx: &mut Context<Self>) {
        self.entries.push(ThreadEntry::TestGenMessage(TestGenMessageEntry {
            run_id: run_id.to_string(),
            message: message.to_string(),
        }));
        let idx = self.entries.len() - 1;
        cx.emit(SdkThreadEvent::EntryAdded(idx));
        cx.notify();
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
        debug!("Spawning SDK query for: {}", content);
        Tokio::spawn(cx, async move {
            debug!("Tokio task started");
            // Build options - only use dangerously_skip_permissions (don't also set permission_mode)
            let options = QueryOptions::new()
                .cwd(&project_root)
                .dangerously_skip_permissions(true)
                .include_partial_messages(true);

            // Create query
            debug!("Creating query...");
            let query_result = query(&content, options).await;
            debug!("Query created: {:?}", query_result.is_ok());

            let mut stream = match query_result {
                Ok(q) => q,
                Err(e) => {
                    let _ = tx.send(SdkUpdate::Error(e.to_string()));
                    return;
                }
            };

            let mut assistant_content = String::new();

            // Process stream
            debug!("Starting to process stream...");
            while let Some(msg_result) = stream.next().await {
                debug!("Got message from stream");
                let msg = match msg_result {
                    Ok(m) => m,
                    Err(e) => {
                        debug!("SDK error: {}", e);
                        let _ = tx.send(SdkUpdate::Error(e.to_string()));
                        return;
                    }
                };

                debug!("SDK message: {:?}", msg);

                match msg {
                    SdkMessage::System(sys) => {
                        if let SdkSystemMessage::Init(init) = sys {
                            // Extract MCP server info (name, status)
                            let mcp_servers = init.mcp_servers.iter()
                                .map(|server| (server.name.clone(), server.status.clone()))
                                .collect();
                            let _ = tx.send(SdkUpdate::Init {
                                session_id: init.session_id,
                                model: init.model,
                                tools: init.tools,
                                mcp_servers,
                            });
                        }
                    }
                    SdkMessage::Assistant(assistant) => {
                        // Extract text and tool_use inputs from message
                        debug!("Assistant message: {}", assistant.message);
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
                                                debug!("Tool input for {}: {}", tool_use_id, input_str);
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
                            debug!("Extracted content from assistant: {}", assistant_content);
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
                                        debug!("Tool use started: {} ({})", tool_name, tool_use_id);
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
                                        debug!("Tool result for {}: {} (error: {})", tool_use_id, output.len(), is_error);
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
                        debug!("Tool progress: {} ({})", progress.tool_name, progress.tool_use_id);
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
                            SdkResultMessage::Success(success) => {
                                // Extract cost information
                                let total_cost = success.total_cost_usd;
                                let mut model_usage = HashMap::new();
                                for (model, usage) in &success.model_usage {
                                    // Estimate cost based on token usage
                                    // This is a placeholder - actual pricing varies by model
                                    let cost = usage.input_tokens as f64 * 0.00001 + usage.output_tokens as f64 * 0.00003;
                                    model_usage.insert(model.clone(), cost);
                                }
                                let input_tokens = success.usage.input_tokens as u64;
                                let output_tokens = success.usage.output_tokens as u64;

                                let _ = tx.send(SdkUpdate::CostUpdate {
                                    total_cost,
                                    model_usage,
                                    input_tokens,
                                    output_tokens,
                                });
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
                        SdkUpdate::Init { session_id, model, tools, mcp_servers } => {
                            this.session_id = Some(session_id.clone());
                            this.model = Some(model);
                            this.tools = tools;
                            this.mcp_servers = mcp_servers;
                            cx.emit(SdkThreadEvent::SessionUpdated);
                            cx.emit(SdkThreadEvent::ToolsUpdated);
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
                                            debug!("Parsed {} todos from TodoWrite", items.len());
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
                        SdkUpdate::CostUpdate {
                            total_cost,
                            model_usage,
                            input_tokens,
                            output_tokens,
                        } => {
                            // Update cost tracking
                            this.update_cost(total_cost, model_usage, input_tokens, output_tokens, cx);
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
