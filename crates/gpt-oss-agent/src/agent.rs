//! GPT-OSS Agent implementation
//!
//! Agent-level abstraction that wraps GptOssClient and provides tool execution,
//! trajectory recording, and ACP integration.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use gpt_oss::{GptOssClient, GptOssRequest};

use crate::error::Result;
use crate::session::GptOssSession;
use crate::tools::{
    Tool, ToolRequest, ToolResult,
    apply_patch::ApplyPatchTool,
    browser::BrowserTool,
    python::PythonTool,
    ui_pane::{PaneManager, UiPaneTool},
};

/// GPT-OSS agent configuration
#[derive(Debug, Clone)]
pub struct GptOssAgentConfig {
    /// Base URL for GPT-OSS server
    pub base_url: String,
    /// Default model to use
    pub model: String,
    /// Workspace root for file operations
    pub workspace_root: PathBuf,
    /// Enable trajectory recording
    pub record_trajectory: bool,
}

impl Default for GptOssAgentConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8000".to_string(),
            model: "gpt-oss-20b".to_string(),
            workspace_root: std::env::current_dir().unwrap_or_default(),
            record_trajectory: false,
        }
    }
}

/// GPT-OSS agent session
pub struct GptOssAgent {
    client: Arc<GptOssClient>,
    config: GptOssAgentConfig,
    tools: RwLock<Vec<Arc<dyn Tool>>>,
}

impl GptOssAgent {
    /// Create a new GPT-OSS agent
    pub async fn new(config: GptOssAgentConfig) -> Result<Self> {
        let client = GptOssClient::builder()
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
        let request = GptOssRequest {
            model: self.config.model.clone(),
            prompt: prompt.to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
        };
        let response = self.client.complete(request).await?;
        Ok(response.text)
    }

    /// Execute a tool request
    pub async fn execute_tool(&self, request: ToolRequest) -> Result<ToolResult> {
        let tools = self.tools.read().await;

        let tool = tools
            .iter()
            .find(|t| t.name() == request.tool)
            .ok_or_else(|| {
                crate::GptOssAgentError::ToolError(format!("Tool not found: {}", request.tool))
            })?;

        tool.execute(request.parameters).await
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
    ///
    /// Sessions maintain conversation history and optionally record
    /// trajectory data for reproducibility.
    pub async fn create_session(&self) -> GptOssSession {
        let tools = self.tools.read().await;
        GptOssSession::new(self.client.clone(), self.config.clone(), tools.clone())
    }

    /// Get the configuration
    pub fn config(&self) -> &GptOssAgentConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation() {
        let config = GptOssAgentConfig::default();
        let agent = GptOssAgent::new(config).await;
        // Agent creation should succeed even if server is not available
        assert!(agent.is_ok());
    }

    #[tokio::test]
    async fn test_list_tools() {
        let config = GptOssAgentConfig::default();
        let agent = GptOssAgent::new(config).await.unwrap();
        let tools = agent.list_tools().await;
        assert_eq!(tools.len(), 4);
        assert!(tools.contains(&"browser".to_string()));
        assert!(tools.contains(&"python".to_string()));
        assert!(tools.contains(&"apply_patch".to_string()));
        assert!(tools.contains(&"ui_pane".to_string()));
    }
}
