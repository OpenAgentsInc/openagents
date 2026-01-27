//! ACP Agent implementation
//!
//! Wraps ACP connections (codex-acp, claude-code-acp, cursor-acp) into the unified Agent trait.

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};

use crate::acp::{AcpConnection, AcpEvent};
use crate::agent::trait_def::Agent;
use crate::agent::unified::{AgentId, UnifiedConversationItem, UnifiedEvent};
use tauri::AppHandle;

/// ACP-based agent implementation
///
/// This wraps an ACP connection (like codex-acp) and provides the unified Agent interface.
pub struct AcpAgent {
    agent_id: AgentId,
    workspace_id: String,
    connection: Arc<Mutex<Option<AcpConnection>>>,
    session_id: Arc<Mutex<Option<String>>>,
    events_tx: Arc<tokio::sync::broadcast::Sender<UnifiedEvent>>,
    app: AppHandle,
    command: String,
    args: Vec<String>,
    env: std::collections::HashMap<String, String>,
}

impl AcpAgent {
    /// Create a new ACP agent
    pub fn new(
        agent_id: AgentId,
        workspace_id: String,
        app: AppHandle,
        command: String,
        args: Vec<String>,
        env: std::collections::HashMap<String, String>,
    ) -> Self {
        let (events_tx, _) = broadcast::channel(1000);

        Self {
            agent_id,
            workspace_id: workspace_id.clone(),
            connection: Arc::new(Mutex::new(None)),
            session_id: Arc::new(Mutex::new(None)),
            events_tx: Arc::new(events_tx),
            app,
            command,
            args,
            env,
        }
    }

    async fn connect_inner(&self, workspace_path: &Path) -> Result<String, String> {
        // Create ACP connection
        let acp_conn = AcpConnection::new(
            self.workspace_id.clone(),
            workspace_path,
            self.command.clone(),
            self.args.clone(),
            self.env.clone(),
            self.app.clone(),
        )
        .await
        .map_err(|e| format!("Failed to connect ACP agent: {}", e))?;

        // Set up event forwarding from ACP connection to unified stream
        let events_tx = self.events_tx.clone();
        let session_id_clone = self.session_id.clone();
        let agent_id = self.agent_id;

        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AcpEvent>();

        tokio::spawn(async move {
            while let Some(event_clone) = event_rx.recv().await {
                tracing::debug!("AcpAgent callback: Received ACP event");

                // Extract and store session ID from session/new response
                if let Some(message) = event_clone
                    .message
                    .get("message")
                    .and_then(|m| m.as_object())
                {
                    // Check if this is a response to session/new
                    if let Some(result) = message.get("result").and_then(|r| r.as_object())
                        && let Some(session_id) = result.get("sessionId").and_then(|s| s.as_str()) {
                            let mut session_id_guard = session_id_clone.lock().await;
                            if session_id_guard.is_none() {
                                *session_id_guard = Some(session_id.to_string());
                                tracing::debug!(
                                    session_id = %session_id,
                                    "AcpAgent: Stored session ID from session/new"
                                );
                            }
                        }
                }

                // Log the raw event structure for debugging
                if let Some(message_obj) = event_clone.message.as_object()
                    && let Some(inner_msg) = message_obj.get("message").and_then(|m| m.as_object())
                    {
                        tracing::debug!(
                            has_id = inner_msg.contains_key("id"),
                            has_method = inner_msg.contains_key("method"),
                            method = inner_msg.get("method").and_then(|m| m.as_str()),
                            "AcpAgent callback: Event structure"
                        );
                        // Log a snippet of the event for debugging
                        if let Ok(json_str) = serde_json::to_string(&inner_msg) {
                            let snippet = if json_str.len() > 200 {
                                format!("{}...", &json_str[..200])
                            } else {
                                json_str
                            };
                            tracing::debug!(snippet = %snippet, "AcpAgent callback: Event snippet");
                        }
                    }

                // Map ACP event to unified event
                if let Some(unified_event) =
                    AcpAgent::map_acp_event_static(&event_clone, &session_id_clone, agent_id).await
                {
                    tracing::debug!(
                        unified_event = ?unified_event,
                        "AcpAgent: Mapped to unified event"
                    );
                    let _ = events_tx.send(unified_event);
                } else {
                    tracing::debug!(
                        "AcpAgent callback: Event did not map to unified event (non-notification or unmapped type)"
                    );
                }
            }
        });

        // Set callback on AcpConnection to forward events
        let callback: crate::acp::AcpEventCallback = Arc::new(move |event| {
            let _ = event_tx.send(event.clone());
        });

        acp_conn.set_event_callback(callback).await;

        // Store connection
        *self.connection.lock().await = Some(acp_conn);

        // Don't emit SessionStarted here - wait for actual session/new response
        // Return workspace_id as temporary session_id (will be updated when session/new responds)
        Ok(self.workspace_id.clone())
    }

    async fn disconnect_inner(&self, _session_id: &str) -> Result<(), String> {
        let mut conn_guard = self.connection.lock().await;
        if let Some(conn) = conn_guard.take() {
            conn.kill()
                .await
                .map_err(|e| format!("Failed to disconnect: {}", e))?;
        }
        Ok(())
    }

    async fn start_session_inner(&self, _session_id: &str, cwd: &Path) -> Result<(), String> {
        let conn_guard = self.connection.lock().await;
        let conn = conn_guard.as_ref().ok_or("Not connected")?;

        // Send session/new request
        let params = serde_json::json!({
            "cwd": cwd,
            "mcpServers": [],
        });

        conn.send_request("session/new", params)
            .await
            .map_err(|e| format!("Failed to start session: {}", e))?;

        // Wait a bit for the response to come back with the actual session ID
        // The callback will store it in self.session_id
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // Check if we got the session ID from the response
        let session_id_guard = self.session_id.lock().await;
        if let Some(actual_session_id) = session_id_guard.as_ref() {
            // Update the session mapping in AgentManager
            // We need to update active_sessions to map the actual ACP session ID
            // But we don't have direct access to AgentManager here
            // Instead, we'll emit SessionStarted and let the frontend/commands handle it
            // OR we could store both mappings

            // Emit SessionStarted event with the real session ID
            let _ = self.events_tx.send(UnifiedEvent::SessionStarted {
                session_id: actual_session_id.clone(),
                agent_id: self.agent_id,
            });

            tracing::info!(
                session_id = %actual_session_id,
                "AcpAgent: Emitted SessionStarted with actual session ID"
            );
        } else {
            tracing::warn!("session/new response didn't contain sessionId");
        }

        Ok(())
    }

    async fn send_message_inner(&self, session_id: &str, text: String) -> Result<(), String> {
        tracing::debug!(
            session_id = %session_id,
            text_len = text.len(),
            "AcpAgent::send_message called"
        );

        let conn_guard = self.connection.lock().await;
        let conn = conn_guard.as_ref().ok_or("Not connected")?;

        // Get the actual ACP session ID
        let acp_session_id = conn
            .get_session_id()
            .await
            .ok_or("Session ID not available")?;

        tracing::debug!(session_id = %acp_session_id, "AcpAgent: Using ACP session ID");

        // Send session/prompt
        let params = serde_json::json!({
            "sessionId": acp_session_id,
            "prompt": vec![serde_json::json!({
                "type": "text",
                "text": text,
            })],
        });

        tracing::debug!("AcpAgent: Sending session/prompt request");

        conn.send_request("session/prompt", params)
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "AcpAgent: Failed to send request");
                format!("Failed to send message: {}", e)
            })?;

        tracing::debug!("AcpAgent: session/prompt request sent successfully");

        Ok(())
    }

    fn events_receiver_inner(&self) -> mpsc::Receiver<UnifiedEvent> {
        // Subscribe to broadcast channel and convert to mpsc
        // This is a workaround - ideally we'd use broadcast everywhere
        let mut broadcast_rx = self.events_tx.subscribe();
        let (tx, rx) = mpsc::channel(1000);

        // Forward from broadcast to mpsc
        tokio::spawn(async move {
            while let Ok(event) = broadcast_rx.recv().await {
                if tx.send(event).await.is_err() {
                    break;
                }
            }
        });

        rx
    }

    async fn get_conversation_items_inner(
        &self,
        _session_id: &str,
    ) -> Result<Vec<UnifiedConversationItem>, String> {
        // TODO: Implement conversation item retrieval
        // This would need to query the ACP connection or maintain state
        Ok(vec![])
    }
}

#[async_trait]
impl Agent for AcpAgent {
    fn agent_id(&self) -> AgentId {
        self.agent_id
    }

    async fn connect(&self, workspace_path: &Path) -> Result<String, String> {
        self.connect_inner(workspace_path).await
    }

    async fn disconnect(&self, session_id: &str) -> Result<(), String> {
        self.disconnect_inner(session_id).await
    }

    async fn start_session(&self, session_id: &str, cwd: &Path) -> Result<(), String> {
        self.start_session_inner(session_id, cwd).await
    }

    async fn send_message(&self, session_id: &str, text: String) -> Result<(), String> {
        self.send_message_inner(session_id, text).await
    }

    fn events_receiver(&self) -> mpsc::Receiver<UnifiedEvent> {
        self.events_receiver_inner()
    }

    async fn get_conversation_items(
        &self,
        session_id: &str,
    ) -> Result<Vec<UnifiedConversationItem>, String> {
        self.get_conversation_items_inner(session_id).await
    }
}

impl AcpAgent {
    /// Get broadcast receiver for events (used by AgentManager)
    /// This is not part of the Agent trait, but is used internally
    pub(crate) fn events_broadcast_receiver(&self) -> broadcast::Receiver<UnifiedEvent> {
        self.events_tx.subscribe()
    }

    /// Static helper to map ACP events (used in callbacks)
    async fn map_acp_event_static(
        event: &AcpEvent,
        session_id_storage: &Arc<Mutex<Option<String>>>,
        agent_id: AgentId,
    ) -> Option<UnifiedEvent> {
        let message = event.message.as_object()?;
        let inner_msg = message.get("message")?.as_object()?;

        // Extract session ID (try to get from event params first, then fall back to stored session_id)
        let session_id_from_params =
            if let Some(params) = inner_msg.get("params").and_then(|p| p.as_object()) {
                params
                    .get("sessionId")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };

        // Use session_id from params if available, otherwise get from storage
        let session_id: String = if let Some(sid) = session_id_from_params {
            sid
        } else {
            session_id_storage.lock().await.clone()?
        };

        // ACP notifications don't have an "id" field - they're notifications
        // ACP responses have an "id" field but no "method" field
        // ACP requests have both "id" and "method" fields

        // Check for notifications (SessionNotification types)
        if !inner_msg.contains_key("id") && inner_msg.contains_key("method")
            && let Some(method) = inner_msg.get("method").and_then(|m| m.as_str()) {
                // ACP SessionNotification comes as a notification with method "session/notification"
                // The actual notification type is in params.notification
                if method == "session/notification" {
                    if let Some(params) = inner_msg.get("params").and_then(|p| p.as_object())
                        && let Some(notification) =
                            params.get("notification").and_then(|n| n.as_object())
                        {
                            return Self::map_session_notification_static(
                                notification,
                                &session_id,
                                agent_id,
                            )
                            .await;
                        }
                } else if method == "session/update" {
                    // Session started or updated
                    if let Some(params) = inner_msg.get("params").and_then(|p| p.as_object())
                        && let Some(update) = params.get("update").and_then(|u| u.as_object()) {
                            // Check if this is a session start (has availableCommands)
                            if update.contains_key("availableCommands") {
                                return Some(UnifiedEvent::SessionStarted {
                                    session_id: session_id.clone(),
                                    agent_id,
                                });
                            }

                            // Check for agent message chunks and thought chunks
                            if let Some(session_update_type) =
                                update.get("sessionUpdate").and_then(|s| s.as_str())
                            {
                                if session_update_type == "agent_message_chunk" {
                                    // Extract content from update.content.text
                                    if let Some(content_obj) =
                                        update.get("content").and_then(|c| c.as_object())
                                        && let Some(text) =
                                            content_obj.get("text").and_then(|t| t.as_str())
                                        {
                                            // Check if this is the last chunk (we'll mark complete when we see stopReason)
                                            return Some(UnifiedEvent::MessageChunk {
                                                session_id: session_id.clone(),
                                                content: text.to_string(),
                                                is_complete: false, // Will be set to true when we see stopReason
                                            });
                                        }
                                } else if session_update_type == "agent_thought_chunk" {
                                    // Extract content from update.content.text
                                    if let Some(content_obj) =
                                        update.get("content").and_then(|c| c.as_object())
                                        && let Some(text) =
                                            content_obj.get("text").and_then(|t| t.as_str())
                                        {
                                            return Some(UnifiedEvent::ThoughtChunk {
                                                session_id: session_id.clone(),
                                                content: text.to_string(),
                                                is_complete: false,
                                            });
                                        }
                                }
                            }

                            // Tool execution updates (ACP rawInput/rawOutput)
                            let extract_tool_id = || -> Option<String> {
                                let direct_keys = ["toolCallId", "call_id", "callId"];
                                for key in direct_keys {
                                    if let Some(id) = update.get(key).and_then(|v| v.as_str()) {
                                        return Some(id.to_string());
                                    }
                                }

                                for nested_key in ["rawInput", "rawOutput"] {
                                    if let Some(obj) =
                                        update.get(nested_key).and_then(|v| v.as_object())
                                    {
                                        for key in direct_keys {
                                            if let Some(id) = obj.get(key).and_then(|v| v.as_str())
                                            {
                                                return Some(id.to_string());
                                            }
                                        }
                                    }
                                }
                                None
                            };

                            let extract_tool_name = || -> Option<String> {
                                let raw_input =
                                    update.get("rawInput").and_then(|v| v.as_object())?;
                                if let Some(cmd) = raw_input.get("command") {
                                    if let Some(parts) = cmd.as_array() {
                                        let joined = parts
                                            .iter()
                                            .filter_map(|v| v.as_str())
                                            .collect::<Vec<_>>()
                                            .join(" ");
                                        if !joined.is_empty() {
                                            return Some(joined);
                                        }
                                    } else if let Some(cmd_str) = cmd.as_str() {
                                        return Some(cmd_str.to_string());
                                    }
                                }
                                raw_input
                                    .get("toolName")
                                    .and_then(|v| v.as_str())
                                    .map(|v| v.to_string())
                            };

                            let extract_output = || -> Option<String> {
                                if let Some(raw_output) =
                                    update.get("rawOutput").and_then(|v| v.as_object())
                                {
                                    if let Some(text) =
                                        raw_output.get("aggregated_output").and_then(|v| v.as_str())
                                    {
                                        return Some(text.to_string());
                                    }
                                    if let Some(text) =
                                        raw_output.get("output").and_then(|v| v.as_str())
                                    {
                                        return Some(text.to_string());
                                    }
                                    if let Some(text) =
                                        raw_output.get("text").and_then(|v| v.as_str())
                                    {
                                        return Some(text.to_string());
                                    }
                                }
                                None
                            };

                            let is_complete = update
                                .get("isComplete")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            if (update.get("rawInput").is_some()
                                || update.get("kind").and_then(|k| k.as_str()) == Some("execute"))
                                && let Some(tool_id) = extract_tool_id() {
                                    let tool_name =
                                        extract_tool_name().unwrap_or_else(|| "tool".to_string());
                                    let arguments = update
                                        .get("rawInput")
                                        .cloned()
                                        .unwrap_or(serde_json::json!({}));
                                    return Some(UnifiedEvent::ToolCall {
                                        session_id: session_id.clone(),
                                        tool_id,
                                        tool_name,
                                        arguments,
                                    });
                                }

                            if update.get("rawOutput").is_some()
                                && let Some(tool_id) = extract_tool_id() {
                                    let output = extract_output().unwrap_or_default();
                                    return Some(UnifiedEvent::ToolCallUpdate {
                                        session_id: session_id.clone(),
                                        tool_id,
                                        output,
                                        is_complete,
                                    });
                                }
                        }
                }
            }

        // Check for response with stopReason (completion)
        // Responses have an "id" field and a "result" field (not "method")
        if inner_msg.contains_key("id") && !inner_msg.contains_key("method") {
            // Check for stopReason in result
            if let Some(result) = inner_msg.get("result").and_then(|r| r.as_object())
                && let Some(stop_reason) = result.get("stopReason").and_then(|s| s.as_str())
                    && stop_reason == "end_turn" {
                        return Some(UnifiedEvent::SessionCompleted {
                            session_id: session_id.clone(),
                            stop_reason: "end_turn".to_string(),
                        });
                    }
        }

        None
    }

    /// Static helper to map SessionNotification
    async fn map_session_notification_static(
        notification: &serde_json::Map<String, serde_json::Value>,
        session_id: &str,
        agent_id: AgentId,
    ) -> Option<UnifiedEvent> {
        // Same logic as map_session_notification but static
        let kind = notification.get("kind")?.as_str()?;

        match kind {
            "AgentMessageChunk" => {
                let content = notification
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                let is_complete = notification
                    .get("isComplete")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);

                Some(UnifiedEvent::MessageChunk {
                    session_id: session_id.to_string(),
                    content,
                    is_complete,
                })
            }
            "AgentThoughtChunk" => {
                let content = notification
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                let is_complete = notification
                    .get("isComplete")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);

                Some(UnifiedEvent::ThoughtChunk {
                    session_id: session_id.to_string(),
                    content,
                    is_complete,
                })
            }
            "ToolCall" => {
                let tool_id = notification
                    .get("toolCallId")
                    .and_then(|id| id.as_str())
                    .unwrap_or("")
                    .to_string();
                let tool_name = notification
                    .get("toolName")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments = notification
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                Some(UnifiedEvent::ToolCall {
                    session_id: session_id.to_string(),
                    tool_id,
                    tool_name,
                    arguments,
                })
            }
            "ToolCallUpdate" => {
                let tool_id = notification
                    .get("toolCallId")
                    .and_then(|id| id.as_str())
                    .unwrap_or("")
                    .to_string();
                let output = notification
                    .get("output")
                    .and_then(|o| o.as_str())
                    .unwrap_or("")
                    .to_string();
                let is_complete = notification
                    .get("isComplete")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);

                Some(UnifiedEvent::ToolCallUpdate {
                    session_id: session_id.to_string(),
                    tool_id,
                    output,
                    is_complete,
                })
            }
            "UserMessageChunk" | "Plan" => None,
            _ => {
                if kind.starts_with("codex/") {
                    Self::map_codex_extension_static(notification, session_id, kind, agent_id).await
                } else {
                    None
                }
            }
        }
    }

    /// Static helper to map Codex extensions
    async fn map_codex_extension_static(
        notification: &serde_json::Map<String, serde_json::Value>,
        session_id: &str,
        kind: &str,
        agent_id: AgentId,
    ) -> Option<UnifiedEvent> {
        match kind {
            "codex/tokenUsage" => {
                let input_tokens = notification
                    .get("inputTokens")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0);
                let output_tokens = notification
                    .get("outputTokens")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0);
                let total_tokens = input_tokens + output_tokens;

                Some(UnifiedEvent::TokenUsage {
                    session_id: session_id.to_string(),
                    input_tokens,
                    output_tokens,
                    total_tokens,
                })
            }
            "codex/rateLimits" => {
                let used_percent = notification
                    .get("usedPercent")
                    .and_then(|p| p.as_f64())
                    .unwrap_or(0.0);
                let resets_at = notification.get("resetsAt").and_then(|r| r.as_u64());

                Some(UnifiedEvent::RateLimitUpdate {
                    agent_id,
                    used_percent,
                    resets_at,
                })
            }
            _ => None,
        }
    }
}
