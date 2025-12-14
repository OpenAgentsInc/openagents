//! Session processor - the main conversation loop.

use crate::{Session, SessionError, SessionEvent, SessionStatus};
use coder_domain::MessageId;
use coder_permission::{PermissionError, PermissionManager, PermissionRequestBuilder};
use futures::StreamExt;
use llm::stream::StreamEvent;
use llm::CompletionStream;
use std::sync::Arc;
use tokio::sync::mpsc;
use tool_registry::{ToolContext, ToolRegistry};
use tracing::{debug, error, info, warn};

/// Configuration for the processor.
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    /// Maximum consecutive tool calls before requiring confirmation.
    pub doom_loop_threshold: u32,
    /// Maximum retry attempts for transient errors.
    pub max_retries: u32,
    /// Base delay for exponential backoff (ms).
    pub retry_base_delay_ms: u64,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            doom_loop_threshold: 3,
            max_retries: 3,
            retry_base_delay_ms: 1000,
        }
    }
}

/// Result of processing a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessResult {
    /// Continue processing (tool use requires another turn).
    Continue,
    /// Stop processing (assistant finished or error).
    Stop,
}

/// The session processor handles the main conversation loop.
pub struct Processor {
    #[allow(dead_code)]
    config: ProcessorConfig,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
    event_tx: mpsc::UnboundedSender<SessionEvent>,
}

impl Processor {
    /// Create a new processor.
    pub fn new(
        config: ProcessorConfig,
        tool_registry: Arc<ToolRegistry>,
        permission_manager: Arc<PermissionManager>,
        event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Self {
        Self {
            config,
            tool_registry,
            permission_manager,
            event_tx,
        }
    }

    /// Process a streaming response from the LLM.
    pub async fn process_stream(
        &self,
        session: &mut Session,
        mut stream: CompletionStream,
    ) -> Result<ProcessResult, SessionError> {
        let message_id = MessageId::new();
        session.current_message_id = Some(message_id);
        session.set_status(SessionStatus::Busy);

        self.emit(SessionEvent::MessageStarted {
            session_id: session.id,
            message_id,
        });

        let mut has_tool_use = false;
        let mut tool_results: Vec<ToolCallResult> = Vec::new();
        let mut current_tool: Option<PendingToolCall> = None;
        let mut finish_reason = "unknown".to_string();

        // Process the stream
        while let Some(result) = stream.next().await {
            let event = match result {
                Ok(e) => e,
                Err(e) => {
                    let error_str = e.to_string();
                    error!(error = %error_str, "Stream error");
                    self.emit(SessionEvent::Error {
                        session_id: session.id,
                        message_id: Some(message_id),
                        error: error_str.clone(),
                    });
                    session.set_status(SessionStatus::Error);
                    return Err(SessionError::Llm(error_str));
                }
            };

            match event {
                StreamEvent::Start { .. } => {
                    debug!("Stream started");
                }

                StreamEvent::TextDelta { delta, .. } => {
                    self.emit(SessionEvent::TextDelta {
                        session_id: session.id,
                        message_id,
                        delta,
                    });
                }

                StreamEvent::ToolCall {
                    tool_call_id,
                    tool_name,
                    input,
                    ..
                } => {
                    has_tool_use = true;

                    self.emit(SessionEvent::ToolStarted {
                        session_id: session.id,
                        message_id,
                        tool_name: tool_name.clone(),
                        tool_call_id: tool_call_id.clone(),
                    });

                    current_tool = Some(PendingToolCall {
                        tool_call_id,
                        tool_name,
                        input,
                    });
                }

                StreamEvent::Finish { finish_reason: reason, usage, .. } => {
                    finish_reason = format!("{:?}", reason);

                    // Update session usage
                    let cost = 0.0; // TODO: Calculate from model pricing
                    session.add_usage(cost, usage.total_tokens());

                    debug!(finish_reason = ?reason, "Stream finished");
                }

                StreamEvent::Error { error } => {
                    let error_str = error.to_string();
                    error!(error = %error_str, "Stream error");
                    self.emit(SessionEvent::Error {
                        session_id: session.id,
                        message_id: Some(message_id),
                        error: error_str.clone(),
                    });
                    session.set_status(SessionStatus::Error);
                    return Err(SessionError::Llm(error_str));
                }

                _ => {
                    // Handle other events as needed
                }
            }

            // Execute tool if we have a pending one
            if let Some(tool) = current_tool.take() {
                let result = self
                    .execute_tool(session, message_id, &tool)
                    .await;

                self.emit(SessionEvent::ToolCompleted {
                    session_id: session.id,
                    message_id,
                    tool_call_id: tool.tool_call_id.clone(),
                    success: result.success,
                });

                tool_results.push(result);
            }
        }

        self.emit(SessionEvent::MessageCompleted {
            session_id: session.id,
            message_id,
            finish_reason: finish_reason.clone(),
        });

        session.current_message_id = None;
        session.set_status(SessionStatus::Idle);

        // Determine if we should continue (tool use) or stop
        if has_tool_use && tool_results.iter().all(|r| r.success) && finish_reason == "tool_use" {
            Ok(ProcessResult::Continue)
        } else {
            Ok(ProcessResult::Stop)
        }
    }

    /// Execute a tool call with permission checking.
    async fn execute_tool(
        &self,
        session: &Session,
        _message_id: MessageId,
        tool: &PendingToolCall,
    ) -> ToolCallResult {
        let ctx = ToolContext::new(&session.working_directory)
            .with_session(session.id.to_string());

        // Check permission
        if let Some(permission_request) = self.tool_registry.check_permission(
            &tool.tool_name,
            &tool.input,
            &ctx,
        ) {
            // Build and submit permission request
            let request = PermissionRequestBuilder::new(session.id, &permission_request.permission_type)
                .title(&permission_request.title)
                .description(&permission_request.description)
                .patterns(permission_request.patterns.clone())
                .metadata(permission_request.metadata.clone())
                .build();

            // Note: status will be set properly by the UI/controller

            match self.permission_manager.ask(request).await {
                Ok(()) => {
                    debug!(tool = %tool.tool_name, "Permission granted");
                }
                Err(PermissionError::Rejected { reason, .. }) => {
                    warn!(tool = %tool.tool_name, reason = %reason, "Permission rejected");
                    return ToolCallResult {
                        tool_call_id: tool.tool_call_id.clone(),
                        success: false,
                        output: format!("Permission denied: {}", reason),
                    };
                }
                Err(e) => {
                    error!(tool = %tool.tool_name, error = %e, "Permission error");
                    return ToolCallResult {
                        tool_call_id: tool.tool_call_id.clone(),
                        success: false,
                        output: format!("Permission error: {}", e),
                    };
                }
            }
        }

        // Execute the tool
        match self.tool_registry.execute(&tool.tool_name, tool.input.clone(), &ctx).await {
            Ok(output) => {
                info!(
                    tool = %tool.tool_name,
                    success = output.success,
                    "Tool executed"
                );
                ToolCallResult {
                    tool_call_id: tool.tool_call_id.clone(),
                    success: output.success,
                    output: output.content,
                }
            }
            Err(e) => {
                error!(
                    tool = %tool.tool_name,
                    error = %e,
                    "Tool execution failed"
                );
                ToolCallResult {
                    tool_call_id: tool.tool_call_id.clone(),
                    success: false,
                    output: format!("Tool error: {}", e),
                }
            }
        }
    }

    /// Emit a session event.
    fn emit(&self, event: SessionEvent) {
        if let Err(e) = self.event_tx.send(event) {
            warn!(error = %e, "Failed to send session event");
        }
    }
}

/// A pending tool call waiting to be executed.
struct PendingToolCall {
    tool_call_id: String,
    tool_name: String,
    input: serde_json::Value,
}

/// Result of a tool call execution.
#[derive(Debug)]
#[allow(dead_code)]
struct ToolCallResult {
    tool_call_id: String,
    success: bool,
    output: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_config_default() {
        let config = ProcessorConfig::default();
        assert_eq!(config.doom_loop_threshold, 3);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_base_delay_ms, 1000);
    }
}
