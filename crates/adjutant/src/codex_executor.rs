//! Codex executor using codex-agent-sdk.
//!
//! Uses the Codex CLI for agentic execution with tool support.

use crate::app_server_executor::AppServerExecutor;
use crate::autopilot_loop::AcpEventSender;
use crate::{AdjutantError, Task, TaskResult};
use acp_adapter::converters::codex::thread_event_to_notifications;
use agent_client_protocol_schema as acp;
use codex_agent_sdk::{
    ApprovalMode, Codex, CommandExecutionStatus, McpToolCallStatus, PatchApplyStatus, SandboxMode,
    ThreadEvent, ThreadItemDetails, ThreadOptions, TurnOptions,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Executor that uses Codex via codex-agent-sdk.
pub struct CodexExecutor {
    workspace_root: PathBuf,
}

impl CodexExecutor {
    /// Create a new Codex executor.
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    fn thread_options(&self) -> ThreadOptions {
        ThreadOptions::new()
            .working_directory(&self.workspace_root)
            .skip_git_repo_check(true)
            .sandbox_mode(SandboxMode::WorkspaceWrite)
            .approval_policy(ApprovalMode::OnFailure)
    }

    /// Execute a task using Codex (non-streaming).
    pub async fn execute(&self, task: &Task) -> Result<TaskResult, AdjutantError> {
        if use_app_server_transport() {
            tracing::info!("CodexExecutor: using app-server transport");
            let executor = AppServerExecutor::new(&self.workspace_root);
            return executor.execute(task).await;
        }

        let codex = Codex::new();
        let mut thread = codex.start_thread(self.thread_options());

        let prompt = task.to_prompt();
        let turn = thread
            .run(prompt, TurnOptions::default())
            .await
            .map_err(|e| AdjutantError::ExecutionFailed(format!("Codex run failed: {}", e)))?;

        let modified_files = extract_modified_files(&turn.items);
        let had_failure = detect_failures(&turn.items);

        Ok(TaskResult {
            success: !had_failure,
            summary: turn.final_response,
            modified_files,
            commit_hash: None,
            error: had_failure.then_some("Codex reported failed tool execution".to_string()),
            session_id: thread.id().map(|id| id.to_string()),
        })
    }

    /// Execute a task using Codex with streaming output.
    pub async fn execute_streaming(
        &self,
        task: &Task,
        token_tx: mpsc::UnboundedSender<String>,
        acp_sender: Option<AcpEventSender>,
    ) -> Result<TaskResult, AdjutantError> {
        if use_app_server_transport() {
            tracing::info!("CodexExecutor: streaming via app-server transport");
            let executor = AppServerExecutor::new(&self.workspace_root);
            return executor
                .execute_streaming(task, token_tx, acp_sender)
                .await;
        }

        let codex = Codex::new();
        let mut thread = codex.start_thread(self.thread_options());

        let prompt = task.to_prompt();
        let mut streamed = thread
            .run_streamed(prompt, TurnOptions::default())
            .await
            .map_err(|e| AdjutantError::ExecutionFailed(format!("Codex stream failed: {}", e)))?;

        let mut current_message = String::new();
        let mut final_response = String::new();
        let mut modified_files: HashSet<String> = HashSet::new();
        let mut saw_completion = false;
        let mut had_failure = false;
        let mut error: Option<String> = None;
        let mut session_id: Option<String> = None;

        while let Some(event) = streamed.next().await {
            let event = event
                .map_err(|e| AdjutantError::ExecutionFailed(format!("Codex stream error: {}", e)))?;

            if let Some(sender) = &acp_sender {
                let notifications = thread_event_to_notifications(&sender.session_id, &event);
                for notification in notifications {
                    match notification.update {
                        acp::SessionUpdate::ToolCall(_)
                        | acp::SessionUpdate::ToolCallUpdate(_)
                        | acp::SessionUpdate::Plan(_)
                        | acp::SessionUpdate::AgentThoughtChunk(_) => {
                            sender.send_update(notification.update);
                        }
                        _ => {}
                    }
                }
            }

            match event {
                ThreadEvent::ThreadStarted(started) => {
                    session_id = Some(started.thread_id);
                }
                ThreadEvent::TurnCompleted(_) => {
                    saw_completion = true;
                }
                ThreadEvent::TurnStarted(_) => {}
                ThreadEvent::TurnFailed(failed) => {
                    had_failure = true;
                    error = Some(failed.error.message);
                }
                ThreadEvent::Error(err) => {
                    had_failure = true;
                    error = Some(err.message);
                }
                ThreadEvent::ItemStarted(item) => {
                    if matches!(item.item.details, ThreadItemDetails::AgentMessage(_)) {
                        current_message.clear();
                    }
                }
                ThreadEvent::ItemUpdated(item) => {
                    if let ThreadItemDetails::AgentMessage(msg) = &item.item.details {
                        if msg.text.len() > current_message.len() {
                            let delta = &msg.text[current_message.len()..];
                            let _ = token_tx.send(delta.to_string());
                            current_message = msg.text.clone();
                        }
                    }
                }
                ThreadEvent::ItemCompleted(item) => match &item.item.details {
                    ThreadItemDetails::AgentMessage(msg) => {
                        if msg.text.len() > current_message.len() {
                            let delta = &msg.text[current_message.len()..];
                            let _ = token_tx.send(delta.to_string());
                        }
                        current_message.clear();
                        final_response = msg.text.clone();
                    }
                    ThreadItemDetails::FileChange(fc) => {
                        if matches!(fc.status, PatchApplyStatus::Completed) {
                            for change in &fc.changes {
                                modified_files.insert(change.path.clone());
                            }
                        }
                        if matches!(fc.status, PatchApplyStatus::Failed) {
                            had_failure = true;
                        }
                    }
                    ThreadItemDetails::CommandExecution(cmd) => {
                        if matches!(cmd.status, CommandExecutionStatus::Failed) {
                            had_failure = true;
                        }
                    }
                    _ => {}
                },
            }
        }

        let success = saw_completion && !had_failure;
        let summary = if final_response.is_empty() {
            current_message
        } else {
            final_response
        };

        Ok(TaskResult {
            success,
            summary,
            modified_files: modified_files.into_iter().collect(),
            commit_hash: None,
            error,
            session_id: session_id.or_else(|| thread.id().map(|id| id.to_string())),
        })
    }
}

fn extract_modified_files(items: &[codex_agent_sdk::ThreadItem]) -> Vec<String> {
    let mut files = HashSet::new();
    for item in items {
        if let ThreadItemDetails::FileChange(fc) = &item.details {
            if matches!(fc.status, PatchApplyStatus::Completed) {
                for change in &fc.changes {
                    files.insert(change.path.clone());
                }
            }
        }
    }
    files.into_iter().collect()
}

fn detect_failures(items: &[codex_agent_sdk::ThreadItem]) -> bool {
    items.iter().any(|item| match &item.details {
        ThreadItemDetails::FileChange(fc) => matches!(fc.status, PatchApplyStatus::Failed),
        ThreadItemDetails::CommandExecution(cmd) => matches!(cmd.status, CommandExecutionStatus::Failed),
        ThreadItemDetails::McpToolCall(tool) => matches!(tool.status, McpToolCallStatus::Failed),
        ThreadItemDetails::Error(_) => true,
        _ => false,
    })
}

fn use_app_server_transport() -> bool {
    match std::env::var("AUTOPILOT_CODEX_TRANSPORT") {
        Ok(value) => matches!(
            value.to_ascii_lowercase().as_str(),
            "app-server" | "appserver" | "app_server"
        ),
        Err(_) => false,
    }
}
