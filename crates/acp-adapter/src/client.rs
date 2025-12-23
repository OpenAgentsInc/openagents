//! ACP Client implementation for OpenAgents
//!
//! This module implements the client-side protocol handlers that respond
//! to requests from the AI agent (permission requests, file operations, etc.)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol_schema as acp;
use tokio::sync::RwLock;

use crate::error::{AcpError, Result};
use crate::session::AcpAgentSession;

/// Permission handler trait for tool authorization
///
/// Implement this trait to customize how permissions are handled.
#[async_trait::async_trait]
pub trait PermissionHandler: Send + Sync {
    /// Check if a tool can be used
    async fn can_use_tool(
        &self,
        tool_call: &acp::ToolCallUpdate,
        options: &[acp::PermissionOption],
    ) -> Result<acp::RequestPermissionOutcome>;
}

/// Default permission handler that allows all operations
pub struct AllowAllPermissions;

#[async_trait::async_trait]
impl PermissionHandler for AllowAllPermissions {
    async fn can_use_tool(
        &self,
        _tool_call: &acp::ToolCallUpdate,
        options: &[acp::PermissionOption],
    ) -> Result<acp::RequestPermissionOutcome> {
        // Find the "allow once" option and use it
        for option in options {
            if matches!(option.kind, acp::PermissionOptionKind::AllowOnce) {
                return Ok(acp::RequestPermissionOutcome::Selected(
                    acp::SelectedPermissionOutcome::new(option.option_id.clone()),
                ));
            }
        }
        // Default to first option if available
        if let Some(first) = options.first() {
            Ok(acp::RequestPermissionOutcome::Selected(
                acp::SelectedPermissionOutcome::new(first.option_id.clone()),
            ))
        } else {
            Ok(acp::RequestPermissionOutcome::Cancelled)
        }
    }
}

/// Default permission handler that denies all operations
pub struct DenyAllPermissions;

#[async_trait::async_trait]
impl PermissionHandler for DenyAllPermissions {
    async fn can_use_tool(
        &self,
        _tool_call: &acp::ToolCallUpdate,
        options: &[acp::PermissionOption],
    ) -> Result<acp::RequestPermissionOutcome> {
        // Find a reject option
        for option in options {
            if matches!(
                option.kind,
                acp::PermissionOptionKind::RejectOnce | acp::PermissionOptionKind::RejectAlways
            ) {
                return Ok(acp::RequestPermissionOutcome::Selected(
                    acp::SelectedPermissionOutcome::new(option.option_id.clone()),
                ));
            }
        }
        Ok(acp::RequestPermissionOutcome::Cancelled)
    }
}

/// OpenAgents ACP client delegate
///
/// Handles requests from the agent including:
/// - Permission requests for tool use
/// - File system operations (read/write)
/// - Terminal creation and management
/// - Session notifications
pub struct OpenAgentsClient {
    /// Active sessions indexed by session ID
    sessions: Arc<RwLock<HashMap<String, AcpAgentSession>>>,

    /// Permission handler for tool authorization
    permission_handler: Arc<dyn PermissionHandler>,

    /// Root directory for file operations
    root_dir: PathBuf,
}

impl OpenAgentsClient {
    /// Create a new client with the given permission handler
    pub fn new(
        sessions: Arc<RwLock<HashMap<String, AcpAgentSession>>>,
        permission_handler: Arc<dyn PermissionHandler>,
        root_dir: PathBuf,
    ) -> Self {
        Self {
            sessions,
            permission_handler,
            root_dir,
        }
    }

    /// Create a new client that allows all operations
    pub fn allow_all(
        sessions: Arc<RwLock<HashMap<String, AcpAgentSession>>>,
        root_dir: PathBuf,
    ) -> Self {
        Self::new(sessions, Arc::new(AllowAllPermissions), root_dir)
    }

    /// Handle a permission request from the agent
    pub async fn request_permission(
        &self,
        request: acp::RequestPermissionRequest,
    ) -> Result<acp::RequestPermissionResponse> {
        let outcome = self
            .permission_handler
            .can_use_tool(&request.tool_call, &request.options)
            .await?;

        Ok(acp::RequestPermissionResponse::new(outcome))
    }

    /// Handle a file read request
    pub async fn read_text_file(
        &self,
        request: acp::ReadTextFileRequest,
    ) -> Result<acp::ReadTextFileResponse> {
        let path = &request.path;

        // Security: Ensure path is within root_dir
        let canonical = path
            .canonicalize()
            .map_err(|e| AcpError::FileError(format!("Invalid path: {}", e)))?;

        if !canonical.starts_with(&self.root_dir) {
            return Err(AcpError::PermissionDenied(format!(
                "Path {} is outside allowed directory",
                path.display()
            )));
        }

        let content = tokio::fs::read_to_string(&canonical)
            .await
            .map_err(|e| AcpError::FileError(format!("Failed to read {}: {}", path.display(), e)))?;

        Ok(acp::ReadTextFileResponse::new(content))
    }

    /// Handle a file write request
    pub async fn write_text_file(
        &self,
        request: acp::WriteTextFileRequest,
    ) -> Result<acp::WriteTextFileResponse> {
        let path = &request.path;

        // Security: Ensure path is within root_dir (for new files, check parent)
        let parent = path.parent().ok_or_else(|| {
            AcpError::FileError("Invalid path: no parent directory".to_string())
        })?;

        let canonical_parent = parent
            .canonicalize()
            .map_err(|e| AcpError::FileError(format!("Invalid parent path: {}", e)))?;

        if !canonical_parent.starts_with(&self.root_dir) {
            return Err(AcpError::PermissionDenied(format!(
                "Path {} is outside allowed directory",
                path.display()
            )));
        }

        tokio::fs::write(path, &request.content)
            .await
            .map_err(|e| AcpError::FileError(format!("Failed to write {}: {}", path.display(), e)))?;

        Ok(acp::WriteTextFileResponse::new())
    }

    /// Handle a session notification from the agent
    pub async fn session_notification(
        &self,
        notification: acp::SessionNotification,
    ) -> Result<()> {
        let session_id = notification.session_id.to_string();

        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            session.handle_notification(notification).await;
        } else {
            tracing::warn!(
                session_id = %session_id,
                "Received notification for unknown session"
            );
        }

        Ok(())
    }

    /// Handle terminal creation request
    pub async fn create_terminal(
        &self,
        request: acp::CreateTerminalRequest,
    ) -> Result<acp::CreateTerminalResponse> {
        // For now, we create a simple pseudo-terminal
        // Full PTY support can be added later
        let terminal_id = acp::TerminalId::new(uuid::Uuid::new_v4().to_string());

        tracing::debug!(
            terminal_id = %terminal_id,
            command = ?request.command,
            "Creating terminal"
        );

        // TODO: Actually spawn the command and manage the terminal
        // For now, just return the ID

        Ok(acp::CreateTerminalResponse::new(terminal_id))
    }
}
