pub mod apply_patch;
pub mod browser;
pub mod python;
pub mod ui_pane;

use serde::{Deserialize, Serialize};

/// Tool execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    pub tool: String,
    pub parameters: serde_json::Value,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Tool trait that all tools must implement
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Execute the tool with the given parameters
    async fn execute(&self, params: serde_json::Value) -> crate::Result<ToolResult>;

    /// Get the tool's name
    fn name(&self) -> &str;

    /// Get the tool's description
    fn description(&self) -> &str;

    /// Get the tool's parameter schema
    fn parameter_schema(&self) -> serde_json::Value;
}
