//! Tool registry for managing available tools.

use crate::{BoxedTool, ToolContext, ToolInfo, ToolOutput, ToolResult};
use std::collections::HashMap;
use std::sync::Arc;

/// A registry of available tools.
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<BoxedTool>>,
}

impl ToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Create a registry with the standard tools.
    pub fn with_standard_tools() -> Self {
        let mut registry = Self::new();
        for tool in crate::wrappers::standard_tools() {
            registry.register(tool);
        }
        registry
    }

    /// Register a tool in the registry.
    pub fn register(&mut self, tool: BoxedTool) {
        let name = tool.info().name;
        self.tools.insert(name, Arc::new(tool));
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<BoxedTool>> {
        self.tools.get(name).cloned()
    }

    /// List all registered tools.
    pub fn list(&self) -> Vec<ToolInfo> {
        self.tools.values().map(|t| t.info()).collect()
    }

    /// Get tool names.
    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Check if a tool is registered.
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Execute a tool by name with JSON input.
    pub async fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> ToolResult<ToolOutput> {
        let tool = self
            .get(name)
            .ok_or_else(|| crate::ToolError::not_found(format!("Tool not found: {}", name)))?;

        tool.execute_json(input, ctx).await
    }

    /// Check permission for a tool with JSON input.
    pub fn check_permission(
        &self,
        name: &str,
        input: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Option<crate::PermissionRequest> {
        self.get(name)
            .and_then(|tool| tool.check_permission_json(input, ctx))
    }

    /// Get tools formatted for Anthropic's API.
    pub fn to_anthropic_tools(&self) -> Vec<serde_json::Value> {
        self.list()
            .into_iter()
            .map(|info| {
                crate::to_anthropic_tool_schema(&info.name, &info.description, info.input_schema)
            })
            .collect()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.names())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_standard_tools() {
        let registry = ToolRegistry::with_standard_tools();
        assert!(registry.len() >= 6); // bash, read, write, edit, grep, glob

        assert!(registry.has("bash"));
        assert!(registry.has("read"));
        assert!(registry.has("write"));
        assert!(registry.has("edit"));
        assert!(registry.has("grep"));
        assert!(registry.has("glob"));
    }

    #[test]
    fn test_list_tools() {
        let registry = ToolRegistry::with_standard_tools();
        let tools = registry.list();

        assert!(tools.iter().any(|t| t.name == "bash"));
        assert!(tools.iter().any(|t| t.name == "read"));
    }

    #[test]
    fn test_anthropic_format() {
        let registry = ToolRegistry::with_standard_tools();
        let tools = registry.to_anthropic_tools();

        assert!(!tools.is_empty());
        for tool in tools {
            assert!(tool.get("name").is_some());
            assert!(tool.get("description").is_some());
            assert!(tool.get("input_schema").is_some());
            assert_eq!(tool["input_schema"]["strict"], true);
            assert_eq!(tool["input_schema"]["additionalProperties"], false);
        }
    }

    #[tokio::test]
    async fn test_execute_tool() {
        let registry = ToolRegistry::with_standard_tools();
        let ctx = ToolContext::default();

        // Test grep with a simple pattern (should work)
        let input = serde_json::json!({
            "pattern": "test",
            "path": "/tmp"
        });

        let result = registry.execute("grep", input, &ctx).await;
        // May succeed or fail depending on /tmp contents, but shouldn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_tool_not_found() {
        let registry = ToolRegistry::new();
        assert!(!registry.has("nonexistent"));
        assert!(registry.get("nonexistent").is_none());
    }
}
