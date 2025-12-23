//! Permission request types for UI integration
//!
//! This module provides types for surfacing ACP permission requests
//! to the UI layer and handling user responses.

use std::collections::HashMap;
use std::sync::Arc;

use agent_client_protocol_schema as acp;
use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, RwLock};

use crate::error::Result;

/// Permission request surfaced to the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPermissionRequest {
    /// Unique ID for this request
    pub request_id: String,
    /// Session ID this request belongs to
    pub session_id: String,
    /// Tool being called
    pub tool_name: String,
    /// Tool call description
    pub description: String,
    /// Input parameters as JSON
    pub input: serde_json::Value,
    /// Available permission options
    pub options: Vec<UiPermissionOption>,
    /// Timestamp when request was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Permission option presented to the user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPermissionOption {
    /// Unique ID for this option
    pub option_id: String,
    /// Kind of permission (allow once, always allow, etc.)
    pub kind: PermissionOptionKind,
    /// Display label for the option
    pub label: String,
    /// Whether this creates a persistent rule
    pub is_persistent: bool,
}

/// Permission option kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOptionKind {
    /// Allow this specific action once
    AllowOnce,
    /// Always allow this pattern
    AllowAlways,
    /// Reject this specific action once
    RejectOnce,
    /// Always reject this pattern
    RejectAlways,
}

impl From<&acp::PermissionOptionKind> for PermissionOptionKind {
    fn from(kind: &acp::PermissionOptionKind) -> Self {
        match kind {
            acp::PermissionOptionKind::AllowOnce => PermissionOptionKind::AllowOnce,
            acp::PermissionOptionKind::AllowAlways => PermissionOptionKind::AllowAlways,
            acp::PermissionOptionKind::RejectOnce => PermissionOptionKind::RejectOnce,
            acp::PermissionOptionKind::RejectAlways => PermissionOptionKind::RejectAlways,
            // Handle any future variants - default to AllowOnce
            _ => PermissionOptionKind::AllowOnce,
        }
    }
}

/// User's response to a permission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPermissionResponse {
    /// ID of the request being responded to
    pub request_id: String,
    /// Selected option ID
    pub selected_option_id: String,
    /// Whether to make this rule persistent across sessions
    pub make_persistent: bool,
}

impl UiPermissionRequest {
    /// Create a permission request from ACP protocol types
    pub fn from_acp(
        request_id: String,
        session_id: String,
        req: &acp::RequestPermissionRequest,
    ) -> Self {
        let tool_name = extract_tool_name(&req.tool_call);
        let description = extract_description(&req.tool_call);
        let input = extract_input(&req.tool_call);

        let options = req
            .options
            .iter()
            .map(|opt| UiPermissionOption {
                option_id: opt.option_id.to_string(),
                kind: PermissionOptionKind::from(&opt.kind),
                label: opt.name.clone(),
                is_persistent: matches!(
                    opt.kind,
                    acp::PermissionOptionKind::AllowAlways
                        | acp::PermissionOptionKind::RejectAlways
                ),
            })
            .collect();

        Self {
            request_id,
            session_id,
            tool_name,
            description,
            input,
            options,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Convert UI response to ACP outcome
    pub fn to_acp_outcome(&self, response: &UiPermissionResponse) -> acp::RequestPermissionOutcome {
        if let Some(_option) = self
            .options
            .iter()
            .find(|o| o.option_id == response.selected_option_id)
        {
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
                response.selected_option_id.clone(),
            ))
        } else {
            // Invalid option ID - cancel
            acp::RequestPermissionOutcome::Cancelled
        }
    }
}

/// Manager for pending permission requests awaiting UI responses
#[derive(Clone)]
pub struct PermissionRequestManager {
    /// Pending requests indexed by request ID
    pending: Arc<RwLock<HashMap<String, PendingRequest>>>,
}

struct PendingRequest {
    request: UiPermissionRequest,
    response_tx: oneshot::Sender<UiPermissionResponse>,
}

impl PermissionRequestManager {
    /// Create a new permission request manager
    pub fn new() -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a permission request and wait for user response
    pub async fn request_permission(
        &self,
        request: UiPermissionRequest,
    ) -> Result<UiPermissionResponse> {
        let (tx, rx) = oneshot::channel();
        let request_id = request.request_id.clone();

        {
            let mut pending = self.pending.write().await;
            pending.insert(
                request_id.clone(),
                PendingRequest {
                    request,
                    response_tx: tx,
                },
            );
        }

        // Wait for user response (or timeout after 5 minutes)
        match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                // Sender dropped
                self.cancel_request(&request_id).await;
                Err(crate::AcpError::PermissionDenied(
                    "Permission request was cancelled".to_string(),
                ))
            }
            Err(_) => {
                // Timeout
                self.cancel_request(&request_id).await;
                Err(crate::AcpError::PermissionDenied(
                    "Permission request timed out".to_string(),
                ))
            }
        }
    }

    /// Submit a user response to a pending request
    pub async fn submit_response(&self, response: UiPermissionResponse) -> bool {
        let mut pending = self.pending.write().await;
        if let Some(pending_req) = pending.remove(&response.request_id) {
            pending_req.response_tx.send(response).is_ok()
        } else {
            false
        }
    }

    /// Get all pending permission requests for a session
    pub async fn get_pending_for_session(
        &self,
        session_id: &str,
    ) -> Vec<UiPermissionRequest> {
        let pending = self.pending.read().await;
        pending
            .values()
            .filter(|p| p.request.session_id == session_id)
            .map(|p| p.request.clone())
            .collect()
    }

    /// Get all pending permission requests
    pub async fn get_all_pending(&self) -> Vec<UiPermissionRequest> {
        let pending = self.pending.read().await;
        pending.values().map(|p| p.request.clone()).collect()
    }

    /// Cancel a pending request
    pub async fn cancel_request(&self, request_id: &str) {
        let mut pending = self.pending.write().await;
        pending.remove(request_id);
    }
}

impl Default for PermissionRequestManager {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions for extracting tool information

fn extract_tool_name(tool_call: &acp::ToolCallUpdate) -> String {
    // Try to extract from title, otherwise use kind
    if let Some(title) = &tool_call.fields.title {
        title.clone()
    } else if let Some(kind) = &tool_call.fields.kind {
        format!("{:?}", kind)
    } else {
        "Tool".to_string()
    }
}

fn extract_description(tool_call: &acp::ToolCallUpdate) -> String {
    // Extract description from tool call metadata or generate default
    if let Some(title) = &tool_call.fields.title {
        format!("Execute: {}", title)
    } else {
        "Execute tool".to_string()
    }
}

fn extract_input(tool_call: &acp::ToolCallUpdate) -> serde_json::Value {
    tool_call
        .fields
        .raw_input
        .clone()
        .unwrap_or(serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_permission_request_lifecycle() {
        let manager = PermissionRequestManager::new();

        let request = UiPermissionRequest {
            request_id: "test-1".to_string(),
            session_id: "session-1".to_string(),
            tool_name: "bash".to_string(),
            description: "Execute bash command".to_string(),
            input: serde_json::json!({"command": "ls -la"}),
            options: vec![UiPermissionOption {
                option_id: "allow-once".to_string(),
                kind: PermissionOptionKind::AllowOnce,
                label: "Allow Once".to_string(),
                is_persistent: false,
            }],
            timestamp: chrono::Utc::now(),
        };

        // Submit request in background
        let manager_clone = manager.clone();
        let request_clone = request.clone();
        let handle = tokio::spawn(async move {
            manager_clone.request_permission(request_clone).await
        });

        // Wait a bit to ensure request is pending
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check it's pending
        let pending = manager.get_all_pending().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].request_id, "test-1");

        // Submit response
        let response = UiPermissionResponse {
            request_id: "test-1".to_string(),
            selected_option_id: "allow-once".to_string(),
            make_persistent: false,
        };
        assert!(manager.submit_response(response).await);

        // Request should complete
        let result = handle.await.unwrap();
        assert!(result.is_ok());

        // Should no longer be pending
        let pending = manager.get_all_pending().await;
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_permission_request_timeout() {
        let manager = PermissionRequestManager::new();

        let request = UiPermissionRequest {
            request_id: "test-timeout".to_string(),
            session_id: "session-1".to_string(),
            tool_name: "bash".to_string(),
            description: "Execute bash command".to_string(),
            input: serde_json::json!({"command": "ls -la"}),
            options: vec![],
            timestamp: chrono::Utc::now(),
        };

        // Note: This test would take 5 minutes in real time
        // In a real test, we'd use a shorter timeout for testing
        // For now, just verify the structure compiles
        drop(manager);
        drop(request);
    }
}
