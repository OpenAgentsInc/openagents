//! FM Bridge agent implementation
//!
//! Agent-level abstraction that wraps FMClient and provides tool execution
//! and trajectory recording, similar to the GPT-OSS agent.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use fm_bridge::{CompletionOptions, FMClient};
use gpt_oss_agent::tools::{
    Tool, ToolRequest, ToolResult,
    apply_patch::ApplyPatchTool,
    browser::BrowserTool,
    python::PythonTool,
    ui_pane::{PaneManager, UiPaneTool},
};

use crate::error::{FmBridgeAgentError, Result};
use crate::session::FmBridgeSession;

/// FM Bridge agent configuration
#[derive(Debug, Clone)]
pub struct FmBridgeAgentConfig {
    /// Base URL for the Foundation Models bridge
    pub base_url: String,
    /// Default model to use
    pub model: String,
    /// Workspace root for file operations
    pub workspace_root: PathBuf,
    /// Enable trajectory recording
    pub record_trajectory: bool,
}

impl Default for FmBridgeAgentConfig {
    fn default() -> Self {
        let base_url =
            std::env::var("FM_BRIDGE_URL").unwrap_or_else(|_| "http://localhost:3030".to_string());
        Self {
            base_url,
            model: "gpt-4o-mini-2024-07-18".to_string(),
            workspace_root: std::env::current_dir().unwrap_or_default(),
            record_trajectory: false,
        }
    }
}

/// FM Bridge agent
pub struct FmBridgeAgent {
    client: Arc<FMClient>,
    config: FmBridgeAgentConfig,
    tools: RwLock<Vec<Arc<dyn Tool>>>,
}

impl FmBridgeAgent {
    /// Create a new FM Bridge agent
    pub async fn new(config: FmBridgeAgentConfig) -> Result<Self> {
        let client = FMClient::builder()
            .base_url(&config.base_url)
            .default_model(&config.model)
            .build()?;

        let pane_manager = Arc::new(std::sync::RwLock::new(PaneManager::new()));
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(BrowserTool::new()),
            Arc::new(PythonTool::new()),
            Arc::new(ApplyPatchTool::new(config.workspace_root.clone())),
            Arc::new(UiPaneTool::new(pane_manager)),
        ];

        Ok(Self {
            client: Arc::new(client),
            config,
            tools: RwLock::new(tools),
        })
    }

    /// Execute a completion request
    pub async fn complete(&self, prompt: &str) -> Result<String> {
        let options = CompletionOptions {
            model: Some(self.config.model.clone()),
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
        };
        let response = self.client.complete(prompt, Some(options)).await?;
        let content = response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .unwrap_or_default();
        Ok(content)
    }

    /// Execute a tool request
    pub async fn execute_tool(&self, request: ToolRequest) -> Result<ToolResult> {
        let tools = self.tools.read().await;

        let tool = tools
            .iter()
            .find(|t| t.name() == request.tool)
            .ok_or_else(|| {
                FmBridgeAgentError::ToolError(format!("Tool not found: {}", request.tool))
            })?;

        let result = tool.execute(request.parameters).await?;
        Ok(result)
    }

    /// List available tools
    pub async fn list_tools(&self) -> Vec<String> {
        let tools = self.tools.read().await;
        tools.iter().map(|t| t.name().to_string()).collect()
    }

    /// Get tool schema
    pub async fn get_tool_schema(&self, tool_name: &str) -> Option<serde_json::Value> {
        let tools = self.tools.read().await;
        tools
            .iter()
            .find(|t| t.name() == tool_name)
            .map(|t| t.parameter_schema())
    }

    /// Check if the agent is ready
    pub async fn is_ready(&self) -> bool {
        self.client.health().await.unwrap_or(false)
    }

    /// Create a new session for multi-turn conversation
    pub async fn create_session(&self) -> FmBridgeSession {
        let tools = self.tools.read().await;
        FmBridgeSession::new(self.client.clone(), self.config.clone(), tools.clone())
    }

    /// Get the configuration
    pub fn config(&self) -> &FmBridgeAgentConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation() {
        let config = FmBridgeAgentConfig::default();
        let agent = FmBridgeAgent::new(config).await;
        assert!(agent.is_ok());
    }

    #[tokio::test]
    async fn test_list_tools() {
        let config = FmBridgeAgentConfig::default();
        let agent = FmBridgeAgent::new(config).await.unwrap();
        let tools = agent.list_tools().await;
        assert_eq!(tools.len(), 4);
        assert!(tools.contains(&"browser".to_string()));
        assert!(tools.contains(&"python".to_string()));
        assert!(tools.contains(&"apply_patch".to_string()));
        assert!(tools.contains(&"ui_pane".to_string()));
    }
}
