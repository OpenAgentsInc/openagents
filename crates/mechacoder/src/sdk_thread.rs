//! SDK Thread - wraps Claude Agent SDK for GPUI integration.

use claude_agent_sdk::{
    query, QueryOptions, SdkMessage, SdkResultMessage, SdkSystemMessage,
    PermissionMode,
};
use futures::StreamExt;
use gpui::{Context, EventEmitter, Task};
use std::path::PathBuf;

/// Events emitted by SdkThread.
#[derive(Clone, Debug)]
pub enum SdkThreadEvent {
    /// A new entry was added.
    EntryAdded(usize),
    /// An entry was updated.
    EntryUpdated(usize),
    /// Thread status changed.
    StatusChanged(ThreadStatus),
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
    pub tool_name: String,
    pub input: String,
    pub output: Option<String>,
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

        cx.spawn(async move |this, cx| {
            // Build options
            let options = QueryOptions::new()
                .cwd(&project_root)
                .permission_mode(PermissionMode::BypassPermissions)
                .include_partial_messages(true);

            // Create query
            let query_result = query(&content, options).await;

            let mut stream = match query_result {
                Ok(q) => q,
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.status = ThreadStatus::Error(e.to_string());
                        this.streaming_content = None;
                        cx.emit(SdkThreadEvent::Error(e.to_string()));
                    });
                    return;
                }
            };

            let mut assistant_content = String::new();

            // Process stream
            while let Some(msg_result) = stream.next().await {
                let msg = match msg_result {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = this.update(cx, |this, cx| {
                            this.status = ThreadStatus::Error(e.to_string());
                            this.streaming_content = None;
                            cx.emit(SdkThreadEvent::Error(e.to_string()));
                        });
                        return;
                    }
                };

                match msg {
                    SdkMessage::System(sys) => {
                        if let SdkSystemMessage::Init(init) = sys {
                            let _ = this.update(cx, |this, _cx| {
                                this.session_id = Some(init.session_id);
                                this.model = Some(init.model);
                            });
                        }
                    }
                    SdkMessage::Assistant(assistant) => {
                        // Extract text from message
                        if let Some(content) = assistant.message.get("content") {
                            if let Some(blocks) = content.as_array() {
                                for block in blocks {
                                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                        assistant_content.push_str(text);
                                    }
                                }
                            }
                        }
                        let _ = this.update(cx, |this, cx| {
                            this.streaming_content = Some(assistant_content.clone());
                            cx.notify();
                        });
                    }
                    SdkMessage::StreamEvent(event) => {
                        // Handle streaming delta
                        if let Some(delta) = event.event.get("delta") {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                assistant_content.push_str(text);
                                let _ = this.update(cx, |this, cx| {
                                    this.streaming_content = Some(assistant_content.clone());
                                    cx.notify();
                                });
                            }
                        }
                    }
                    SdkMessage::ToolProgress(progress) => {
                        let _ = this.update(cx, |this, cx| {
                            // Add or update tool entry
                            let tool_entry = ThreadEntry::ToolUse(ToolUse {
                                tool_name: progress.tool_name,
                                input: String::new(),
                                output: None,
                                status: ToolStatus::Running,
                            });
                            this.entries.push(tool_entry);
                            let idx = this.entries.len() - 1;
                            cx.emit(SdkThreadEvent::EntryAdded(idx));
                        });
                    }
                    SdkMessage::Result(result) => {
                        let _ = this.update(cx, |this, cx| {
                            // Finalize assistant message
                            if !assistant_content.is_empty() {
                                this.entries.push(ThreadEntry::AssistantMessage(AssistantMessage {
                                    content: assistant_content.clone(),
                                }));
                                let idx = this.entries.len() - 1;
                                cx.emit(SdkThreadEvent::EntryAdded(idx));
                            }

                            this.streaming_content = None;
                            this.status = match result {
                                SdkResultMessage::Success(_) => ThreadStatus::Completed,
                                _ => ThreadStatus::Error("Query failed".to_string()),
                            };
                            cx.emit(SdkThreadEvent::StatusChanged(this.status.clone()));
                        });
                    }
                    _ => {}
                }
            }

            // Ensure we finalize if stream ends without result
            let _ = this.update(cx, |this, cx| {
                if !matches!(this.status, ThreadStatus::Completed | ThreadStatus::Error(_)) {
                    if !assistant_content.is_empty() && this.streaming_content.is_some() {
                        this.entries.push(ThreadEntry::AssistantMessage(AssistantMessage {
                            content: assistant_content,
                        }));
                        let idx = this.entries.len() - 1;
                        cx.emit(SdkThreadEvent::EntryAdded(idx));
                    }
                    this.streaming_content = None;
                    this.status = ThreadStatus::Idle;
                    cx.emit(SdkThreadEvent::StatusChanged(this.status.clone()));
                }
            });
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
