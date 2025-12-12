//! Pi Thread - wraps Pi agent for GPUI integration.
//!
//! Provides a similar interface to SdkThread but uses the Pi coding agent
//! instead of Claude Agent SDK.

use futures::StreamExt;
use gpui::{Context, EventEmitter, Task};
use gpui_tokio::Tokio;
use pi::{AgentEvent, PiAgent, PiConfig, PiError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::pin;
use tokio::sync::mpsc;
use tracing::debug;

use crate::panels::CostTracker;
use crate::sdk_thread::{
    AssistantMessage, ThreadEntry, ThreadStatus, TodoItem, TodoState, ToolStatus, ToolUse,
    UserMessage,
};

/// Events emitted by PiThread.
#[derive(Clone, Debug)]
pub enum PiThreadEvent {
    /// A new entry was added.
    EntryAdded(usize),
    /// An entry was updated.
    EntryUpdated(usize),
    /// Thread status changed.
    StatusChanged(ThreadStatus),
    /// Todo list was updated.
    TodosUpdated,
    /// Cost tracking was updated.
    CostUpdated,
    /// Session ID was updated.
    SessionUpdated,
    /// Error occurred.
    Error(String),
}

/// Internal message from Tokio task to GPUI.
#[derive(Clone, Debug)]
enum PiUpdate {
    /// Session started
    SessionStarted { session_id: String, model: String },
    /// Streaming text content
    TextDelta(String),
    /// Tool use started
    ToolUseStart { id: String, name: String },
    /// Tool input delta
    ToolInputDelta { id: String, json: String },
    /// Tool is executing
    ToolExecuting { id: String, _name: String },
    /// Tool completed
    ToolResult {
        id: String,
        name: String,
        output: String,
        is_error: bool,
    },
    /// Turn completed
    TurnComplete {
        _turn: u32,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: f64,
    },
    /// Message complete
    MessageComplete { text: String },
    /// Agent completed
    Completed {
        _total_turns: u32,
        total_cost_usd: f64,
    },
    /// Agent cancelled
    Cancelled,
    /// Error occurred
    Error { message: String, _retryable: bool },
}

/// A conversation thread using the Pi agent.
pub struct PiThread {
    /// Project root directory.
    project_root: PathBuf,
    /// Thread entries.
    entries: Vec<ThreadEntry>,
    /// Current status.
    status: ThreadStatus,
    /// Current streaming content.
    streaming_content: Option<String>,
    /// Session ID.
    session_id: Option<String>,
    /// Model being used.
    model: Option<String>,
    /// Map from tool_use_id to entry index.
    tool_use_index: HashMap<String, usize>,
    /// Current todo list state.
    todo_state: TodoState,
    /// Cost tracking state.
    cost_tracker: CostTracker,
    /// Accumulated text for current response.
    accumulated_text: String,
    /// Accumulated tool input JSON.
    tool_input_buffers: HashMap<String, String>,
}

impl PiThread {
    /// Create a new Pi thread.
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
            cost_tracker: CostTracker::default(),
            accumulated_text: String::new(),
            tool_input_buffers: HashMap::new(),
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

    /// Send a user message.
    pub fn send_message(&mut self, content: String, cx: &mut Context<Self>) -> Task<()> {
        // Add user message to entries
        self.entries.push(ThreadEntry::UserMessage(UserMessage {
            content: content.clone(),
        }));
        let entry_idx = self.entries.len() - 1;
        cx.emit(PiThreadEvent::EntryAdded(entry_idx));

        // Update status
        self.status = ThreadStatus::Streaming;
        self.streaming_content = Some(String::new());
        self.accumulated_text.clear();
        self.tool_input_buffers.clear();
        cx.emit(PiThreadEvent::StatusChanged(self.status.clone()));

        let project_root = self.project_root.clone();

        // Create channel for updates
        let (tx, mut rx) = mpsc::unbounded_channel::<PiUpdate>();

        // Spawn Pi agent work on Tokio runtime
        debug!("Spawning Pi agent for: {}", content);
        Tokio::spawn(cx, async move {
            debug!("Pi agent task started");

            // Create Pi agent configuration
            let config = PiConfig::new("claude-sonnet-4-20250514")
                .working_directory(&project_root)
                .max_turns(50);

            // Create agent
            let mut agent = match PiAgent::anthropic(config) {
                Ok(a) => a,
                Err(e) => {
                    let _ = tx.send(PiUpdate::Error {
                        message: e.to_string(),
                        _retryable: false,
                    });
                    return;
                }
            };

            // Send session started
            let _ = tx.send(PiUpdate::SessionStarted {
                session_id: agent.session_id().to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
            });

            // Run agent and process events
            let mut stream = pin!(agent.run(&content));

            while let Some(event_result) = stream.next().await {
                let event = match event_result {
                    Ok(e) => e,
                    Err(e) => {
                        let _ = tx.send(PiUpdate::Error {
                            message: e.to_string(),
                            _retryable: is_retryable(&e),
                        });
                        continue;
                    }
                };

                debug!("Pi event: {:?}", event);

                match event {
                    AgentEvent::Started { session_id, model } => {
                        let _ = tx.send(PiUpdate::SessionStarted { session_id, model });
                    }
                    AgentEvent::TextDelta { text } => {
                        let _ = tx.send(PiUpdate::TextDelta(text));
                    }
                    AgentEvent::ToolUseStart { id, name } => {
                        let _ = tx.send(PiUpdate::ToolUseStart { id, name });
                    }
                    AgentEvent::ToolInputDelta { id, json } => {
                        let _ = tx.send(PiUpdate::ToolInputDelta { id, json });
                    }
                    AgentEvent::ToolExecuting { id, name, input: _ } => {
                        let _ = tx.send(PiUpdate::ToolExecuting { id, _name: name });
                    }
                    AgentEvent::ToolResult {
                        id,
                        name,
                        output,
                        is_error,
                    } => {
                        let _ = tx.send(PiUpdate::ToolResult {
                            id,
                            name,
                            output,
                            is_error,
                        });
                    }
                    AgentEvent::MessageComplete { text } => {
                        let _ = tx.send(PiUpdate::MessageComplete { text });
                    }
                    AgentEvent::TurnComplete {
                        turn,
                        usage,
                        cost_usd,
                        stop_reason: _,
                    } => {
                        let _ = tx.send(PiUpdate::TurnComplete {
                            _turn: turn,
                            input_tokens: usage.input_tokens,
                            output_tokens: usage.output_tokens,
                            cost_usd,
                        });
                    }
                    AgentEvent::Completed {
                        total_turns,
                        total_cost_usd,
                        outcome: _,
                    } => {
                        let _ = tx.send(PiUpdate::Completed {
                            _total_turns: total_turns,
                            total_cost_usd,
                        });
                    }
                    AgentEvent::Cancelled => {
                        let _ = tx.send(PiUpdate::Cancelled);
                    }
                    AgentEvent::Error { message, retryable } => {
                        let _ = tx.send(PiUpdate::Error { message, _retryable: retryable });
                    }
                    _ => {}
                }
            }
        })
        .detach();

        // Spawn GPUI task to receive updates
        cx.spawn(async move |this, cx| {
            while let Some(update) = rx.recv().await {
                let should_break = matches!(
                    update,
                    PiUpdate::Completed { .. } | PiUpdate::Cancelled | PiUpdate::Error { .. }
                );

                let _ = this.update(cx, |this, cx| {
                    match update {
                        PiUpdate::SessionStarted { session_id, model } => {
                            this.session_id = Some(session_id);
                            this.model = Some(model);
                            cx.emit(PiThreadEvent::SessionUpdated);
                        }
                        PiUpdate::TextDelta(text) => {
                            this.accumulated_text.push_str(&text);
                            this.streaming_content = Some(this.accumulated_text.clone());
                            cx.notify();
                        }
                        PiUpdate::ToolUseStart { id, name } => {
                            let tool_entry = ThreadEntry::ToolUse(ToolUse {
                                tool_use_id: id.clone(),
                                tool_name: name,
                                input: String::new(),
                                output: None,
                                status: ToolStatus::Pending,
                            });
                            this.entries.push(tool_entry);
                            let idx = this.entries.len() - 1;
                            this.tool_use_index.insert(id.clone(), idx);
                            this.tool_input_buffers.insert(id, String::new());
                            cx.emit(PiThreadEvent::EntryAdded(idx));
                        }
                        PiUpdate::ToolInputDelta { id, json } => {
                            if let Some(buffer) = this.tool_input_buffers.get_mut(&id) {
                                buffer.push_str(&json);
                            }
                            if let Some(&idx) = this.tool_use_index.get(&id) {
                                if let ThreadEntry::ToolUse(tool_use) = &mut this.entries[idx] {
                                    if let Some(buffer) = this.tool_input_buffers.get(&id) {
                                        tool_use.input = buffer.clone();
                                    }
                                    cx.emit(PiThreadEvent::EntryUpdated(idx));
                                }
                            }
                        }
                        PiUpdate::ToolExecuting { id, _name: _ } => {
                            if let Some(&idx) = this.tool_use_index.get(&id) {
                                if let ThreadEntry::ToolUse(tool_use) = &mut this.entries[idx] {
                                    tool_use.status = ToolStatus::Running;
                                    cx.emit(PiThreadEvent::EntryUpdated(idx));
                                }
                            }
                        }
                        PiUpdate::ToolResult {
                            id,
                            name,
                            output,
                            is_error,
                        } => {
                            if let Some(&idx) = this.tool_use_index.get(&id) {
                                if let ThreadEntry::ToolUse(tool_use) = &mut this.entries[idx] {
                                    // Check if this is a TodoWrite tool
                                    if name == "TodoWrite" {
                                        if let Some(items) =
                                            TodoItem::parse_from_json(&tool_use.input)
                                        {
                                            debug!("Parsed {} todos from TodoWrite", items.len());
                                            this.todo_state = TodoState { items };
                                            cx.emit(PiThreadEvent::TodosUpdated);
                                        }
                                    }

                                    tool_use.output = Some(output);
                                    tool_use.status = if is_error {
                                        ToolStatus::Failed("Tool execution failed".to_string())
                                    } else {
                                        ToolStatus::Completed
                                    };
                                    cx.emit(PiThreadEvent::EntryUpdated(idx));
                                }
                            }
                        }
                        PiUpdate::MessageComplete { text } => {
                            this.entries
                                .push(ThreadEntry::AssistantMessage(AssistantMessage {
                                    content: text,
                                }));
                            let idx = this.entries.len() - 1;
                            cx.emit(PiThreadEvent::EntryAdded(idx));
                        }
                        PiUpdate::TurnComplete {
                            _turn: _,
                            input_tokens,
                            output_tokens,
                            cost_usd,
                        } => {
                            this.cost_tracker.total_input_tokens += input_tokens as u64;
                            this.cost_tracker.total_output_tokens += output_tokens as u64;
                            this.cost_tracker.total_cost_usd += cost_usd;
                            cx.emit(PiThreadEvent::CostUpdated);
                        }
                        PiUpdate::Completed {
                            _total_turns: _,
                            total_cost_usd,
                        } => {
                            this.cost_tracker.total_cost_usd = total_cost_usd;
                            this.streaming_content = None;
                            this.accumulated_text.clear();
                            this.status = ThreadStatus::Completed;
                            cx.emit(PiThreadEvent::CostUpdated);
                            cx.emit(PiThreadEvent::StatusChanged(this.status.clone()));
                        }
                        PiUpdate::Cancelled => {
                            this.streaming_content = None;
                            this.accumulated_text.clear();
                            this.status = ThreadStatus::Idle;
                            cx.emit(PiThreadEvent::StatusChanged(this.status.clone()));
                        }
                        PiUpdate::Error { message, _retryable: _ } => {
                            this.streaming_content = None;
                            this.accumulated_text.clear();
                            this.status = ThreadStatus::Error(message.clone());
                            cx.emit(PiThreadEvent::Error(message));
                            cx.emit(PiThreadEvent::StatusChanged(this.status.clone()));
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
        // TODO: Implement cancellation via Pi agent's cancel token
        self.status = ThreadStatus::Idle;
        self.streaming_content = None;
        self.accumulated_text.clear();
        cx.emit(PiThreadEvent::StatusChanged(self.status.clone()));
    }
}

impl EventEmitter<PiThreadEvent> for PiThread {}

/// Check if a Pi error is retryable
fn is_retryable(error: &PiError) -> bool {
    error.is_retryable()
}
