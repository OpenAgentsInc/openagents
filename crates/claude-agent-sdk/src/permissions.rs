//! Permission handling for tool use requests.

use crate::error::Result;
use crate::protocol::{PermissionResult, PermissionUpdate};
use async_trait::async_trait;
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;

/// Trait for handling permission requests.
///
/// Implement this trait to customize how tool use permissions are handled.
/// The SDK will call `can_use_tool` before each tool execution.
#[async_trait]
pub trait PermissionHandler: Send + Sync {
    /// Check if a tool can be used.
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool being invoked
    /// * `input` - Tool input parameters
    /// * `suggestions` - Suggested permission updates for "always allow"
    /// * `blocked_path` - File path that triggered the request (if applicable)
    /// * `decision_reason` - Why this permission request was triggered
    /// * `tool_use_id` - Unique identifier for this tool call
    /// * `agent_id` - Sub-agent ID if running in a sub-agent context
    ///
    /// # Returns
    /// * `Ok(PermissionResult::Allow { .. })` - Allow the tool to execute
    /// * `Ok(PermissionResult::Deny { .. })` - Deny the tool execution
    /// * `Err(_)` - Error occurred during permission check
    #[allow(clippy::too_many_arguments)]
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &Value,
        suggestions: Option<Vec<PermissionUpdate>>,
        blocked_path: Option<String>,
        decision_reason: Option<String>,
        tool_use_id: &str,
        agent_id: Option<String>,
    ) -> Result<PermissionResult>;
}

/// A permission handler that allows all tool uses.
#[derive(Debug, Clone, Default)]
pub struct AllowAllPermissions;

#[async_trait]
impl PermissionHandler for AllowAllPermissions {
    async fn can_use_tool(
        &self,
        _tool_name: &str,
        input: &Value,
        _suggestions: Option<Vec<PermissionUpdate>>,
        _blocked_path: Option<String>,
        _decision_reason: Option<String>,
        _tool_use_id: &str,
        _agent_id: Option<String>,
    ) -> Result<PermissionResult> {
        Ok(PermissionResult::allow(input.clone()))
    }
}

/// A permission handler that denies all tool uses.
#[derive(Debug, Clone, Default)]
pub struct DenyAllPermissions {
    /// Message to include in denial.
    pub message: String,
}

impl DenyAllPermissions {
    /// Create a new deny-all handler with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[async_trait]
impl PermissionHandler for DenyAllPermissions {
    async fn can_use_tool(
        &self,
        _tool_name: &str,
        _input: &Value,
        _suggestions: Option<Vec<PermissionUpdate>>,
        _blocked_path: Option<String>,
        _decision_reason: Option<String>,
        _tool_use_id: &str,
        _agent_id: Option<String>,
    ) -> Result<PermissionResult> {
        Ok(PermissionResult::deny_and_interrupt(&self.message))
    }
}

/// A permission handler that uses a callback function.
pub struct CallbackPermissionHandler<F> {
    callback: F,
}

impl<F, Fut> CallbackPermissionHandler<F>
where
    F: Fn(PermissionRequest) -> Fut + Send + Sync,
    Fut: Future<Output = Result<PermissionResult>> + Send,
{
    /// Create a new callback-based permission handler.
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

/// Permission request information passed to callbacks.
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    /// Name of the tool being invoked.
    pub tool_name: String,
    /// Tool input parameters.
    pub input: Value,
    /// Suggested permission updates for "always allow".
    pub suggestions: Option<Vec<PermissionUpdate>>,
    /// File path that triggered the request.
    pub blocked_path: Option<String>,
    /// Why this permission request was triggered.
    pub decision_reason: Option<String>,
    /// Unique identifier for this tool call.
    pub tool_use_id: String,
    /// Sub-agent ID if running in a sub-agent context.
    pub agent_id: Option<String>,
}

#[async_trait]
impl<F, Fut> PermissionHandler for CallbackPermissionHandler<F>
where
    F: Fn(PermissionRequest) -> Fut + Send + Sync,
    Fut: Future<Output = Result<PermissionResult>> + Send,
{
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &Value,
        suggestions: Option<Vec<PermissionUpdate>>,
        blocked_path: Option<String>,
        decision_reason: Option<String>,
        tool_use_id: &str,
        agent_id: Option<String>,
    ) -> Result<PermissionResult> {
        let request = PermissionRequest {
            tool_name: tool_name.to_string(),
            input: input.clone(),
            suggestions,
            blocked_path,
            decision_reason,
            tool_use_id: tool_use_id.to_string(),
            agent_id,
        };
        (self.callback)(request).await
    }
}

/// Builder for creating permission handlers with tool-specific rules.
#[derive(Default)]
pub struct PermissionRules {
    /// Tools that are always allowed.
    allow_tools: Vec<String>,
    /// Tools that are always denied.
    deny_tools: Vec<String>,
    /// Default behavior for unlisted tools.
    default_allow: bool,
}

impl PermissionRules {
    /// Create new permission rules.
    pub fn new() -> Self {
        Self::default()
    }

    /// Always allow a specific tool.
    pub fn allow(mut self, tool_name: impl Into<String>) -> Self {
        self.allow_tools.push(tool_name.into());
        self
    }

    /// Always deny a specific tool.
    pub fn deny(mut self, tool_name: impl Into<String>) -> Self {
        self.deny_tools.push(tool_name.into());
        self
    }

    /// Set default behavior for unlisted tools.
    pub fn default_allow(mut self, allow: bool) -> Self {
        self.default_allow = allow;
        self
    }

    /// Build the permission handler.
    pub fn build(self) -> RulesPermissionHandler {
        RulesPermissionHandler { rules: self }
    }
}

/// Permission handler based on predefined rules.
pub struct RulesPermissionHandler {
    rules: PermissionRules,
}

#[async_trait]
impl PermissionHandler for RulesPermissionHandler {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &Value,
        _suggestions: Option<Vec<PermissionUpdate>>,
        _blocked_path: Option<String>,
        _decision_reason: Option<String>,
        _tool_use_id: &str,
        _agent_id: Option<String>,
    ) -> Result<PermissionResult> {
        // Check allow list
        if self.rules.allow_tools.iter().any(|t| t == tool_name) {
            return Ok(PermissionResult::allow(input.clone()));
        }

        // Check deny list
        if self.rules.deny_tools.iter().any(|t| t == tool_name) {
            return Ok(PermissionResult::deny(format!(
                "Tool '{}' is not allowed",
                tool_name
            )));
        }

        // Apply default
        if self.rules.default_allow {
            Ok(PermissionResult::allow(input.clone()))
        } else {
            Ok(PermissionResult::deny(format!(
                "Tool '{}' requires explicit permission",
                tool_name
            )))
        }
    }
}

/// Create a permission handler from a closure.
pub fn permission_handler<F, Fut>(callback: F) -> Arc<dyn PermissionHandler>
where
    F: Fn(PermissionRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<PermissionResult>> + Send + 'static,
{
    Arc::new(CallbackPermissionHandler::new(callback))
}
