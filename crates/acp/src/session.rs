//! Session and thread state management for ACP.
//!
//! This module provides the AcpThread struct which manages the state of a
//! conversation with Claude Code, including messages, tool calls, and terminals.

use agent_client_protocol as acp;
use anyhow::Result;
use futures::channel::oneshot;
use gpui::{Context, EventEmitter, SharedString, Task};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crate::types::{
    AgentConnection, AssistantMessage, AssistantMessageChunk, ContentBlock, Project, ThreadEntry,
    ThreadStatus, ToolCall, ToolCallContent, ToolCallStatus, UserMessage, UserMessageId,
};
use crate::AcpConnection;

/// Events emitted by AcpThread.
#[derive(Clone, Debug)]
pub enum AcpThreadEvent {
    /// A new entry was added.
    EntryAdded(usize),
    /// An entry was updated.
    EntryUpdated(usize),
    /// Thread status changed.
    StatusChanged(ThreadStatus),
    /// Permission request received.
    PermissionRequested {
        tool_call: acp::ToolCallUpdate,
        options: Vec<acp::PermissionOption>,
    },
    /// Error occurred.
    Error(String),
}

/// Terminal state.
#[derive(Clone, Debug)]
pub struct TerminalState {
    pub id: acp::TerminalId,
    pub label: String,
    pub cwd: Option<PathBuf>,
    pub output: String,
    pub exit_status: Option<acp::TerminalExitStatus>,
}

/// Pending permission request.
struct PendingPermission {
    tool_call: acp::ToolCallUpdate,
    options: Vec<acp::PermissionOption>,
    response_tx: oneshot::Sender<acp::RequestPermissionOutcome>,
}

/// A conversation thread with Claude Code.
pub struct AcpThread {
    /// Server name.
    server_name: SharedString,
    /// Connection to the agent.
    connection: Rc<AcpConnection>,
    /// Project context.
    project: Project,
    /// Session ID.
    session_id: acp::SessionId,
    /// Prompt capabilities.
    prompt_capabilities: acp::PromptCapabilities,
    /// Thread entries (messages, tool calls).
    entries: Vec<ThreadEntry>,
    /// Current status.
    status: ThreadStatus,
    /// Registered terminals.
    terminals: HashMap<acp::TerminalId, TerminalState>,
    /// Pending permission request.
    pending_permission: Option<PendingPermission>,
    /// Current streaming assistant message chunks.
    streaming_chunks: Vec<AssistantMessageChunk>,
    /// Current streaming tool call.
    streaming_tool_call: Option<ToolCall>,
}

impl AcpThread {
    /// Create a new ACP thread.
    pub fn new(
        server_name: SharedString,
        connection: Rc<AcpConnection>,
        project: Project,
        session_id: acp::SessionId,
        prompt_capabilities: acp::PromptCapabilities,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            server_name,
            connection,
            project,
            session_id,
            prompt_capabilities,
            entries: Vec::new(),
            status: ThreadStatus::Idle,
            terminals: HashMap::new(),
            pending_permission: None,
            streaming_chunks: Vec::new(),
            streaming_tool_call: None,
        }
    }

    /// Get the server name.
    pub fn server_name(&self) -> &SharedString {
        &self.server_name
    }

    /// Get the connection.
    pub fn connection(&self) -> &Rc<AcpConnection> {
        &self.connection
    }

    /// Get the project.
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &acp::SessionId {
        &self.session_id
    }

    /// Get the prompt capabilities.
    pub fn prompt_capabilities(&self) -> &acp::PromptCapabilities {
        &self.prompt_capabilities
    }

    /// Get all entries.
    pub fn entries(&self) -> &[ThreadEntry] {
        &self.entries
    }

    /// Get the current status.
    pub fn status(&self) -> &ThreadStatus {
        &self.status
    }

    /// Check if there's a pending permission request.
    pub fn has_pending_permission(&self) -> bool {
        self.pending_permission.is_some()
    }

    /// Get pending permission info.
    pub fn pending_permission_info(&self) -> Option<(&acp::ToolCallUpdate, &[acp::PermissionOption])> {
        self.pending_permission
            .as_ref()
            .map(|p| (&p.tool_call, p.options.as_slice()))
    }

    /// Send a user message.
    pub fn send_message(
        &mut self,
        content: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let content = content.into();
        let message_id = UserMessageId::new();

        // Add user message to entries
        let user_message = UserMessage {
            id: Some(message_id.clone()),
            content: ContentBlock::text(&content),
            chunks: vec![acp::ContentBlock::Text(acp::TextContent::new(content.clone()))],
        };
        self.entries.push(ThreadEntry::UserMessage(user_message));
        let entry_idx = self.entries.len() - 1;
        cx.emit(AcpThreadEvent::EntryAdded(entry_idx));

        // Update status
        self.status = ThreadStatus::Streaming;
        cx.emit(AcpThreadEvent::StatusChanged(self.status.clone()));

        // Build prompt request
        let params = acp::PromptRequest::new(
            self.session_id.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(content))],
        );

        // Send prompt - call before spawning to get the Task
        let prompt_task = self.connection.prompt(Some(message_id), params, cx);

        // Process result in background
        cx.spawn::<_, anyhow::Result<()>>(async move |this, cx| {
            let result = prompt_task.await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(_response) => {
                        // Finalize any streaming content
                        this.finalize_streaming(cx);
                        this.status = ThreadStatus::Idle;
                        cx.emit(AcpThreadEvent::StatusChanged(this.status.clone()));
                    }
                    Err(e) => {
                        this.status = ThreadStatus::Error(e.to_string());
                        cx.emit(AcpThreadEvent::Error(e.to_string()));
                    }
                }
            })?;

            Ok(())
        })
    }

    /// Cancel the current generation.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.connection.cancel(&self.session_id, cx);
        self.status = ThreadStatus::Idle;
        cx.emit(AcpThreadEvent::StatusChanged(self.status.clone()));
    }

    /// Handle a session update from the agent.
    pub fn handle_session_update(
        &mut self,
        update: acp::SessionUpdate,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        match update {
            acp::SessionUpdate::UserMessageChunk(_chunk) => {
                // User message chunk echoed back - usually we already have it
                log::trace!("Received user message chunk");
            }

            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                // Agent message chunk - append to streaming message
                self.status = ThreadStatus::Streaming;
                cx.emit(AcpThreadEvent::StatusChanged(self.status.clone()));

                match chunk.content {
                    acp::ContentBlock::Text(text) => {
                        self.streaming_chunks.push(AssistantMessageChunk::Message {
                            content: text.text,
                        });
                    }
                    _ => {
                        log::warn!("Unhandled content block type in agent message chunk");
                    }
                }
                cx.notify();
            }

            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                // Agent thought chunk - append to streaming thoughts
                match chunk.content {
                    acp::ContentBlock::Text(text) => {
                        self.streaming_chunks.push(AssistantMessageChunk::Thought {
                            content: text.text,
                        });
                    }
                    _ => {
                        log::warn!("Unhandled content block type in agent thought chunk");
                    }
                }
                cx.notify();
            }

            acp::SessionUpdate::ToolCall(tool_call) => {
                // New tool call - finalize any streaming content first
                self.finalize_streaming(cx);

                let tc = ToolCall {
                    id: tool_call.tool_call_id,
                    title: tool_call.title,
                    kind: tool_call.kind,
                    content: Vec::new(),
                    status: ToolCallStatus::Pending,
                    raw_input: tool_call.raw_input,
                    raw_output: tool_call.raw_output,
                };
                self.entries.push(ThreadEntry::ToolCall(tc));
                let entry_idx = self.entries.len() - 1;
                cx.emit(AcpThreadEvent::EntryAdded(entry_idx));
            }

            acp::SessionUpdate::ToolCallUpdate(update) => {
                // Find and update the tool call
                if let Some(entry) = self.entries.iter_mut().rev().find_map(|e| {
                    if let ThreadEntry::ToolCall(tc) = e {
                        if tc.id == update.tool_call_id {
                            return Some(tc);
                        }
                    }
                    None
                }) {
                    // Update status
                    if let Some(status) = update.fields.status {
                        entry.status = status.into();
                    }

                    // Update title
                    if let Some(title) = update.fields.title {
                        entry.title = title;
                    }

                    // Update content
                    if let Some(content) = update.fields.content {
                        entry.content = content
                            .into_iter()
                            .filter_map(|c| match c {
                                acp::ToolCallContent::Content(content_item) => {
                                    match content_item.content {
                                        acp::ContentBlock::Text(t) => {
                                            Some(ToolCallContent::Text(t.text))
                                        }
                                        _ => None,
                                    }
                                }
                                acp::ToolCallContent::Diff(d) => Some(ToolCallContent::Diff {
                                    path: d.path,
                                    old_content: d.old_text.unwrap_or_default(),
                                    new_content: d.new_text,
                                }),
                                acp::ToolCallContent::Terminal(t) => {
                                    Some(ToolCallContent::Terminal {
                                        terminal_id: t.terminal_id,
                                        output: String::new(),
                                    })
                                }
                                _ => None,
                            })
                            .collect();
                    }

                    // Update raw fields
                    if let Some(raw_input) = update.fields.raw_input {
                        entry.raw_input = Some(raw_input);
                    }
                    if let Some(raw_output) = update.fields.raw_output {
                        entry.raw_output = Some(raw_output);
                    }

                    // Find entry index and emit update
                    if let Some(idx) = self.entries.iter().position(|e| {
                        if let ThreadEntry::ToolCall(tc) = e {
                            tc.id == update.tool_call_id
                        } else {
                            false
                        }
                    }) {
                        cx.emit(AcpThreadEvent::EntryUpdated(idx));
                    }
                }
            }

            acp::SessionUpdate::Plan(_plan) => {
                // Agent execution plan - log for now
                log::trace!("Received agent plan");
            }

            acp::SessionUpdate::AvailableCommandsUpdate(_commands) => {
                // Available commands changed - log for now
                log::trace!("Available commands updated");
            }

            acp::SessionUpdate::CurrentModeUpdate(_) => {
                // Mode changed - handled in connection layer
            }

            _ => {
                log::trace!("Unhandled session update: {:?}", update);
            }
        }

        cx.notify();
        Ok(())
    }

    /// Request permission for a tool call.
    pub fn request_permission(
        &mut self,
        tool_call: acp::ToolCallUpdate,
        options: Vec<acp::PermissionOption>,
        cx: &mut Context<Self>,
    ) -> Result<Task<acp::RequestPermissionOutcome>> {
        let (tx, rx) = oneshot::channel();

        self.pending_permission = Some(PendingPermission {
            tool_call: tool_call.clone(),
            options: options.clone(),
            response_tx: tx,
        });

        self.status = ThreadStatus::WaitingForConfirmation;
        cx.emit(AcpThreadEvent::StatusChanged(self.status.clone()));
        cx.emit(AcpThreadEvent::PermissionRequested { tool_call, options });
        cx.notify();

        Ok(cx.foreground_executor().spawn(async move {
            rx.await.unwrap_or_else(|_| {
                // Default to cancelled if channel closed
                acp::RequestPermissionOutcome::Cancelled
            })
        }))
    }

    /// Respond to a pending permission request.
    pub fn respond_permission(&mut self, option_id: acp::PermissionOptionId, cx: &mut Context<Self>) {
        if let Some(pending) = self.pending_permission.take() {
            let outcome = acp::RequestPermissionOutcome::Selected(
                acp::SelectedPermissionOutcome::new(option_id)
            );
            let _ = pending.response_tx.send(outcome);
            self.status = ThreadStatus::Streaming;
            cx.emit(AcpThreadEvent::StatusChanged(self.status.clone()));
            cx.notify();
        }
    }

    /// Register a terminal.
    pub fn register_terminal(
        &mut self,
        terminal_id: acp::TerminalId,
        label: String,
        cwd: Option<PathBuf>,
    ) {
        self.terminals.insert(
            terminal_id.clone(),
            TerminalState {
                id: terminal_id,
                label,
                cwd,
                output: String::new(),
                exit_status: None,
            },
        );
    }

    /// Append terminal output.
    pub fn append_terminal_output(&mut self, terminal_id: &acp::TerminalId, data: &[u8]) {
        if let Some(terminal) = self.terminals.get_mut(terminal_id) {
            if let Ok(text) = std::str::from_utf8(data) {
                terminal.output.push_str(text);
            }
        }
    }

    /// Set terminal exit status.
    pub fn set_terminal_exit(&mut self, terminal_id: &acp::TerminalId, status: acp::TerminalExitStatus) {
        if let Some(terminal) = self.terminals.get_mut(terminal_id) {
            terminal.exit_status = Some(status);
        }
    }

    /// Get terminal output.
    pub fn get_terminal_output(&self, terminal_id: &acp::TerminalId) -> Option<String> {
        self.terminals.get(terminal_id).map(|t| t.output.clone())
    }

    /// Get all terminals.
    pub fn terminals(&self) -> &HashMap<acp::TerminalId, TerminalState> {
        &self.terminals
    }

    /// Finalize any streaming content.
    fn finalize_streaming(&mut self, cx: &mut Context<Self>) {
        // Finalize streaming assistant message if any
        if !self.streaming_chunks.is_empty() {
            let message = AssistantMessage {
                chunks: std::mem::take(&mut self.streaming_chunks),
            };
            self.entries.push(ThreadEntry::AssistantMessage(message));
            let entry_idx = self.entries.len() - 1;
            cx.emit(AcpThreadEvent::EntryAdded(entry_idx));
        }
    }
}

impl EventEmitter<AcpThreadEvent> for AcpThread {}
