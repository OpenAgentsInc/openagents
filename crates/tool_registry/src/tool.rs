//! Core Tool trait for implementing tools.

use crate::{ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// Information about a tool for display purposes.
#[derive(Debug, Clone, Serialize)]
pub struct ToolInfo {
    /// Unique identifier for the tool.
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
    /// Categories/tags for the tool.
    pub tags: Vec<String>,
    /// Whether the tool requires permission to execute.
    pub requires_permission: bool,
}

/// The result of executing a tool.
#[derive(Debug, Clone, Serialize)]
pub struct ToolOutput {
    /// Whether the tool execution was successful.
    pub success: bool,
    /// The output content (text or structured data).
    pub content: String,
    /// Optional structured data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Error message if the tool failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Additional metadata about the execution.
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
}

impl ToolOutput {
    /// Create a successful output with text content.
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            data: None,
            error: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Create a successful output with structured data.
    pub fn success_with_data<T: Serialize>(content: impl Into<String>, data: T) -> Self {
        Self {
            success: true,
            content: content.into(),
            data: serde_json::to_value(data).ok(),
            error: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Create a failed output.
    pub fn failure(error: impl Into<String>) -> Self {
        let error_msg = error.into();
        Self {
            success: false,
            content: error_msg.clone(),
            data: None,
            error: Some(error_msg),
            metadata: serde_json::Value::Null,
        }
    }

    /// Add metadata to the output.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

impl From<ToolError> for ToolOutput {
    fn from(err: ToolError) -> Self {
        Self::failure(err.user_message())
    }
}

/// Permission request for tool execution.
#[derive(Debug, Clone, Serialize)]
pub struct PermissionRequest {
    /// Type of permission being requested.
    pub permission_type: String,
    /// Human-readable title for the permission dialog.
    pub title: String,
    /// Detailed description of what will be done.
    pub description: String,
    /// Patterns that this permission would allow (for "Always Allow").
    pub patterns: Vec<String>,
    /// Additional context for the permission decision.
    pub metadata: serde_json::Value,
}

impl PermissionRequest {
    /// Create a new permission request.
    pub fn new(
        permission_type: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            permission_type: permission_type.into(),
            title: title.into(),
            description: description.into(),
            patterns: Vec::new(),
            metadata: serde_json::Value::Null,
        }
    }

    /// Add patterns for "Always Allow" matching.
    pub fn with_patterns(mut self, patterns: Vec<String>) -> Self {
        self.patterns = patterns;
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Trait for implementing tools that can be executed by the agent.
///
/// Tools should be stateless - all state should be passed via the input
/// or retrieved during execution.
#[async_trait]
pub trait Tool: Send + Sync + Debug {
    /// The input type for this tool.
    type Input: DeserializeOwned + Send;

    /// Get information about this tool.
    fn info(&self) -> ToolInfo;

    /// Execute the tool with the given input and context.
    async fn execute(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult<ToolOutput>;

    /// Check if this tool requires permission for the given input.
    ///
    /// Returns `Some(PermissionRequest)` if permission is needed,
    /// or `None` if the tool can execute without permission.
    fn check_permission(&self, _input: &Self::Input, _ctx: &ToolContext) -> Option<PermissionRequest> {
        // Default: no permission required
        None
    }

    /// Validate the input before execution.
    ///
    /// This is called before `check_permission` and `execute`.
    fn validate(&self, _input: &Self::Input, _ctx: &ToolContext) -> ToolResult<()> {
        Ok(())
    }
}

/// A boxed, type-erased tool that can be stored in a registry.
pub type BoxedTool = Box<dyn DynTool>;

/// Object-safe version of the Tool trait for dynamic dispatch.
#[async_trait]
pub trait DynTool: Send + Sync + Debug {
    /// Get information about this tool.
    fn info(&self) -> ToolInfo;

    /// Execute the tool with JSON input.
    async fn execute_json(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> ToolResult<ToolOutput>;

    /// Check if this tool requires permission for the given input.
    fn check_permission_json(
        &self,
        input: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Option<PermissionRequest>;

    /// Validate the input before execution.
    fn validate_json(&self, input: &serde_json::Value, ctx: &ToolContext) -> ToolResult<()>;
}

/// Wrapper to convert a typed Tool into a DynTool.
#[derive(Debug)]
pub struct ToolWrapper<T: Tool>(pub T);

#[async_trait]
impl<T: Tool + 'static> DynTool for ToolWrapper<T> {
    fn info(&self) -> ToolInfo {
        self.0.info()
    }

    async fn execute_json(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> ToolResult<ToolOutput> {
        let typed_input: T::Input = serde_json::from_value(input).map_err(|e| {
            ToolError::InvalidInput(format!("Failed to parse input: {}", e))
        })?;

        self.0.validate(&typed_input, ctx)?;
        self.0.execute(typed_input, ctx).await
    }

    fn check_permission_json(
        &self,
        input: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Option<PermissionRequest> {
        let typed_input: T::Input = serde_json::from_value(input.clone()).ok()?;
        self.0.check_permission(&typed_input, ctx)
    }

    fn validate_json(&self, input: &serde_json::Value, ctx: &ToolContext) -> ToolResult<()> {
        let typed_input: T::Input = serde_json::from_value(input.clone()).map_err(|e| {
            ToolError::InvalidInput(format!("Failed to parse input: {}", e))
        })?;
        self.0.validate(&typed_input, ctx)
    }
}

/// Helper trait to convert a Tool into a boxed DynTool.
pub trait IntoBoxedTool: Tool + Sized + 'static {
    fn into_boxed(self) -> BoxedTool {
        Box::new(ToolWrapper(self))
    }
}

impl<T: Tool + 'static> IntoBoxedTool for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct EchoInput {
        message: String,
    }

    #[derive(Debug)]
    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        type Input = EchoInput;

        fn info(&self) -> ToolInfo {
            ToolInfo {
                name: "echo".to_string(),
                description: "Echoes the input message".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "The message to echo"
                        }
                    },
                    "required": ["message"]
                }),
                tags: vec!["test".to_string()],
                requires_permission: false,
            }
        }

        async fn execute(&self, input: Self::Input, _ctx: &ToolContext) -> ToolResult<ToolOutput> {
            Ok(ToolOutput::success(input.message))
        }
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let tool = EchoTool;
        let ctx = ToolContext::default();
        let input = EchoInput {
            message: "Hello, world!".to_string(),
        };

        let output = tool.execute(input, &ctx).await.unwrap();
        assert!(output.success);
        assert_eq!(output.content, "Hello, world!");
    }

    #[tokio::test]
    async fn test_dyn_tool() {
        let tool: BoxedTool = EchoTool.into_boxed();
        let ctx = ToolContext::default();
        let input = serde_json::json!({ "message": "Hello from DynTool!" });

        let output = tool.execute_json(input, &ctx).await.unwrap();
        assert!(output.success);
        assert_eq!(output.content, "Hello from DynTool!");
    }

    #[test]
    fn test_tool_output() {
        let success = ToolOutput::success("Done!");
        assert!(success.success);
        assert!(success.error.is_none());

        let failure = ToolOutput::failure("Something went wrong");
        assert!(!failure.success);
        assert!(failure.error.is_some());
    }
}
