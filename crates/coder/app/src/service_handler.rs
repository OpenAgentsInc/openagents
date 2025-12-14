//! Background chat handler using ChatService.
//!
//! Runs in a separate thread with a tokio runtime to handle async
//! operations while the main thread runs the winit event loop.

use coder_service::{ChatService, ChatUpdate, ServiceConfig};
use futures::StreamExt;
use mechacoder::ServerMessage;
use std::path::PathBuf;
use std::thread::JoinHandle;
use tokio::sync::mpsc;

/// Messages from the UI to the service handler.
#[derive(Debug, Clone)]
pub enum ServiceRequest {
    /// Send a chat message.
    SendMessage { content: String, cwd: String },
    /// Cancel the current operation.
    Cancel,
}

/// Spawns a background thread with a tokio runtime to handle chat via ChatService.
///
/// Returns a JoinHandle for the spawned thread.
pub fn spawn_service_handler(
    request_rx: mpsc::UnboundedReceiver<ServiceRequest>,
    response_tx: mpsc::UnboundedSender<ServerMessage>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        runtime.block_on(async move {
            let mut handler = ServiceHandler::new(request_rx, response_tx).await;
            handler.run().await;
        });
    })
}

/// Background handler that uses ChatService for AI operations.
struct ServiceHandler {
    request_rx: mpsc::UnboundedReceiver<ServiceRequest>,
    response_tx: mpsc::UnboundedSender<ServerMessage>,
    service: ChatService,
}

impl ServiceHandler {
    async fn new(
        request_rx: mpsc::UnboundedReceiver<ServiceRequest>,
        response_tx: mpsc::UnboundedSender<ServerMessage>,
    ) -> Self {
        // Create service config
        let config = ServiceConfig::from_env();

        // Initialize ChatService
        let service = match ChatService::new(config).await {
            Ok(s) => {
                log::info!("[ServiceHandler] ChatService initialized");
                s
            }
            Err(e) => {
                log::error!("[ServiceHandler] Failed to initialize ChatService: {}", e);
                panic!("Failed to initialize ChatService: {}", e);
            }
        };

        Self {
            request_rx,
            response_tx,
            service,
        }
    }

    async fn run(&mut self) {
        log::info!("[ServiceHandler] Starting message loop");

        while let Some(request) = self.request_rx.recv().await {
            self.handle_request(request).await;
        }

        log::info!("[ServiceHandler] Message loop ended");
    }

    async fn handle_request(&mut self, request: ServiceRequest) {
        match request {
            ServiceRequest::SendMessage { content, cwd } => {
                log::info!("[ServiceHandler] Received message: {}", content);

                // Create or get session with the working directory
                let session = match self.service.create_session(Some(PathBuf::from(&cwd))).await {
                    Ok(s) => {
                        log::info!("[ServiceHandler] Created session: {}", s.id);
                        // Send session init to UI
                        let _ = self.response_tx.send(ServerMessage::SessionInit {
                            session_id: s.id.to_string(),
                        });
                        s
                    }
                    Err(e) => {
                        log::error!("[ServiceHandler] Failed to create session: {}", e);
                        let _ = self.response_tx.send(ServerMessage::Done {
                            error: Some(format!("Failed to create session: {}", e)),
                        });
                        return;
                    }
                };

                // Send message and process stream
                let stream = self.service.send_message(session.id, content);
                futures::pin_mut!(stream);

                while let Some(update) = stream.next().await {
                    if let Some(msg) = self.map_update_to_message(update) {
                        if self.response_tx.send(msg.clone()).is_err() {
                            log::warn!("[ServiceHandler] UI channel closed");
                            break;
                        }

                        // Check if done
                        if matches!(msg, ServerMessage::Done { .. }) {
                            break;
                        }
                    }
                }
            }
            ServiceRequest::Cancel => {
                log::info!("[ServiceHandler] Cancel requested");
                // TODO: implement cancellation using service.cancel()
            }
        }
    }

    /// Map ChatUpdate to ServerMessage for UI consumption.
    fn map_update_to_message(&self, update: ChatUpdate) -> Option<ServerMessage> {
        match update {
            ChatUpdate::TextDelta { delta, .. } => Some(ServerMessage::TextDelta { text: delta }),

            ChatUpdate::ToolStarted {
                tool_call_id,
                tool_name,
                ..
            } => Some(ServerMessage::ToolStart {
                tool_use_id: tool_call_id,
                tool_name,
            }),

            ChatUpdate::ToolInputDelta {
                tool_call_id,
                delta,
                ..
            } => Some(ServerMessage::ToolInput {
                tool_use_id: tool_call_id,
                partial_json: delta,
            }),

            ChatUpdate::ToolProgress {
                tool_call_id,
                message,
                ..
            } => {
                log::debug!("[ServiceHandler] Tool progress: {} - {}", tool_call_id, message);
                // ServerMessage::ToolProgress uses elapsed_seconds, not message
                // For now, skip this as we can't easily convert a message to seconds
                None
            }

            ChatUpdate::ToolCompleted {
                tool_call_id,
                output,
                is_error,
                ..
            } => Some(ServerMessage::ToolResult {
                tool_use_id: tool_call_id,
                output,
                is_error,
            }),

            ChatUpdate::SessionEnded { error, .. } => Some(ServerMessage::Done { error }),

            ChatUpdate::Error { message, .. } => Some(ServerMessage::Done {
                error: Some(message),
            }),

            // Events we log but don't send to UI yet
            ChatUpdate::SessionStarted { session_id, .. } => {
                log::info!("[ServiceHandler] Session started: {}", session_id);
                None
            }

            ChatUpdate::MessageStarted { message_id, .. } => {
                log::debug!("[ServiceHandler] Message started: {}", message_id);
                None
            }

            ChatUpdate::MessageCompleted {
                finish_reason,
                message_id,
                ..
            } => {
                log::debug!(
                    "[ServiceHandler] Message completed: {} ({})",
                    message_id,
                    finish_reason
                );
                None
            }

            ChatUpdate::ReasoningDelta { delta, .. } => {
                log::debug!("[ServiceHandler] Reasoning: {}", delta);
                // Could map to a new ServerMessage variant for extended thinking
                None
            }

            ChatUpdate::PermissionRequired { request, .. } => {
                log::info!(
                    "[ServiceHandler] Permission required: {} - {}",
                    request.title,
                    request.description
                );
                // TODO: Send permission request to UI
                None
            }

            ChatUpdate::PermissionResolved { granted, .. } => {
                log::info!("[ServiceHandler] Permission resolved: granted={}", granted);
                None
            }

            ChatUpdate::UsageUpdate {
                total_tokens,
                cost_usd,
                ..
            } => {
                log::info!(
                    "[ServiceHandler] Usage: {} tokens, ${:.4}",
                    total_tokens,
                    cost_usd
                );
                None
            }

            ChatUpdate::AgentInfo {
                agent_id,
                model_id,
                provider_id,
                ..
            } => {
                log::info!(
                    "[ServiceHandler] Agent: {} (model={}, provider={})",
                    agent_id,
                    model_id,
                    provider_id
                );
                None
            }

            _ => None,
        }
    }
}
