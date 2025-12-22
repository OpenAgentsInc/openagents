//! Permission handling system for tool execution
//!
//! This module provides the core permission system that allows users to
//! approve, reject, or create rules for tool executions.

use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::sync::mpsc;

/// A request for permission to execute a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// Unique ID for this permission request
    pub id: String,

    /// Tool name (e.g., "Bash", "Edit", "Write")
    pub tool: String,

    /// Tool input parameters
    pub input: serde_json::Value,

    /// Optional description of what this tool call will do
    pub description: Option<String>,

    /// Timestamp when request was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl PermissionRequest {
    /// Create a new permission request
    pub fn new(tool: String, input: serde_json::Value, description: Option<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tool,
            input,
            description,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Get a pattern string for this request (e.g., "Bash:npm:*")
    pub fn pattern(&self) -> String {
        match self.tool.as_str() {
            "Bash" => {
                // Extract command from input
                if let Some(command) = self.input.get("command").and_then(|v| v.as_str()) {
                    // Get first word as the program
                    let program = command.split_whitespace().next().unwrap_or("*");
                    format!("Bash:{}", program)
                } else {
                    "Bash:*".to_string()
                }
            }
            "Edit" | "Write" | "Read" => {
                // Extract file path pattern
                if let Some(path) = self.input.get("file_path").and_then(|v| v.as_str()) {
                    // Extract file extension
                    if let Some(ext) = std::path::Path::new(path)
                        .extension()
                        .and_then(|e| e.to_str())
                    {
                        format!("{}:*.{}", self.tool, ext)
                    } else {
                        format!("{}:*", self.tool)
                    }
                } else {
                    format!("{}:*", self.tool)
                }
            }
            _ => self.tool.clone(),
        }
    }
}

/// User's response to a permission request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionAction {
    /// Allow this specific request once
    Allow,

    /// Always allow requests matching this pattern
    AlwaysAllow,

    /// Reject this specific request
    Reject,

    /// Always reject requests matching this pattern
    AlwaysReject,
}

impl fmt::Display for PermissionAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => write!(f, "Allow"),
            Self::AlwaysAllow => write!(f, "Always Allow"),
            Self::Reject => write!(f, "Reject"),
            Self::AlwaysReject => write!(f, "Always Reject"),
        }
    }
}

/// Response to a permission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponse {
    /// ID of the request this responds to
    pub request_id: String,

    /// Action taken by user
    pub action: PermissionAction,

    /// Pattern to save as a rule (for AlwaysAllow/AlwaysReject)
    pub pattern: Option<String>,

    /// Whether this should be a persistent rule (vs session-only)
    pub persistent: bool,
}

/// Trait for handling permission requests
///
/// Implementations can use channels, callbacks, or other mechanisms
/// to communicate with the UI layer.
#[async_trait::async_trait]
pub trait PermissionHandler: Send + Sync {
    /// Request permission to execute a tool
    ///
    /// Returns true if permission granted, false if denied
    async fn request_permission(&self, request: PermissionRequest) -> Result<bool, PermissionError>;

    /// Check if a tool execution matches an existing permission rule
    ///
    /// Returns Some(true) if allowed by rule, Some(false) if denied by rule,
    /// None if no matching rule exists
    async fn check_rules(&self, request: &PermissionRequest) -> Result<Option<bool>, PermissionError>;
}

/// Permission system errors
#[derive(Debug, thiserror::Error)]
pub enum PermissionError {
    #[error("Permission request timeout")]
    Timeout,

    #[error("Permission handler not available")]
    NotAvailable,

    #[error("Database error: {0}")]
    Database(String),

    #[error("Channel error: {0}")]
    Channel(String),
}

/// Channel-based permission handler that communicates with UI
pub struct ChannelPermissionHandler {
    /// Send permission requests to UI
    request_tx: mpsc::UnboundedSender<PermissionRequest>,

    /// Receive permission responses from UI
    response_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<PermissionResponse>>,

    /// Permission rule storage
    storage: std::sync::Arc<crate::storage::PermissionStorage>,
}

impl ChannelPermissionHandler {
    /// Create a new channel-based permission handler
    pub fn new(
        request_tx: mpsc::UnboundedSender<PermissionRequest>,
        response_rx: mpsc::UnboundedReceiver<PermissionResponse>,
        storage: std::sync::Arc<crate::storage::PermissionStorage>,
    ) -> Self {
        Self {
            request_tx,
            response_rx: tokio::sync::Mutex::new(response_rx),
            storage,
        }
    }
}

#[async_trait::async_trait]
impl PermissionHandler for ChannelPermissionHandler {
    async fn request_permission(&self, request: PermissionRequest) -> Result<bool, PermissionError> {
        // First check if we have a rule for this
        if let Some(allowed) = self.check_rules(&request).await? {
            return Ok(allowed);
        }

        // Send request to UI
        self.request_tx
            .send(request.clone())
            .map_err(|e| PermissionError::Channel(e.to_string()))?;

        // Wait for response (with timeout)
        let mut rx = self.response_rx.lock().await;

        let response = tokio::time::timeout(
            std::time::Duration::from_secs(300), // 5 minute timeout
            rx.recv()
        )
        .await
        .map_err(|_| PermissionError::Timeout)?
        .ok_or(PermissionError::NotAvailable)?;

        // If user selected "Always" option, save the rule
        if matches!(response.action, PermissionAction::AlwaysAllow | PermissionAction::AlwaysReject) {
            if let Some(pattern) = response.pattern {
                let allowed = response.action == PermissionAction::AlwaysAllow;
                self.storage
                    .save_rule(&pattern, allowed, response.persistent)
                    .await
                    .map_err(|e| PermissionError::Database(e.to_string()))?;
            }
        }

        Ok(matches!(response.action, PermissionAction::Allow | PermissionAction::AlwaysAllow))
    }

    async fn check_rules(&self, request: &PermissionRequest) -> Result<Option<bool>, PermissionError> {
        let pattern = request.pattern();

        self.storage
            .check_pattern(&pattern)
            .await
            .map_err(|e| PermissionError::Database(e.to_string()))
    }
}

/// Safe commands that can be auto-approved
pub const SAFE_COMMANDS: &[&str] = &[
    "ls",
    "pwd",
    "git status",
    "git log",
    "git diff",
    "git show",
    "git branch",
    "cargo check",
    "cargo build --dry-run",
    "npm list",
    "node --version",
    "python --version",
];

/// Check if a command is considered safe for auto-approval
pub fn is_safe_command(command: &str) -> bool {
    let normalized = command.trim().to_lowercase();
    SAFE_COMMANDS.iter().any(|safe| {
        let safe_normalized = safe.to_lowercase();
        normalized == safe_normalized || normalized.starts_with(&format!("{} ", safe_normalized))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_request_pattern() {
        // Bash command patterns
        let req = PermissionRequest::new(
            "Bash".to_string(),
            serde_json::json!({"command": "npm install axios"}),
            None,
        );
        assert_eq!(req.pattern(), "Bash:npm");

        // File operation patterns
        let req = PermissionRequest::new(
            "Edit".to_string(),
            serde_json::json!({"file_path": "/path/to/file.rs"}),
            None,
        );
        assert_eq!(req.pattern(), "Edit:*.rs");

        let req = PermissionRequest::new(
            "Write".to_string(),
            serde_json::json!({"file_path": "/path/to/config.toml"}),
            None,
        );
        assert_eq!(req.pattern(), "Write:*.toml");
    }

    #[test]
    fn test_safe_commands() {
        assert!(is_safe_command("ls"));
        assert!(is_safe_command("ls -la"));
        assert!(is_safe_command("git status"));
        assert!(is_safe_command("Git Status")); // Case insensitive
        assert!(!is_safe_command("rm -rf /"));
        assert!(!is_safe_command("git push --force"));
    }
}
