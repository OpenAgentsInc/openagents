//! GPT-OSS Agent session management
//!
//! Provides session tracking with conversation history and optional
//! trajectory recording for reproducibility.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use gpt_oss::{GptOssClient, GptOssRequest};

use crate::error::{GptOssAgentError, Result};
use crate::tools::{Tool, ToolRequest, ToolResult};
use crate::GptOssAgentConfig;

/// A message in the conversation history
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    /// Role: "user" or "assistant"
    pub role: String,
    /// Message content
    pub content: String,
    /// Optional tool calls made in this turn
    pub tool_calls: Vec<ToolCall>,
}

/// A tool call record
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    /// Tool name
    pub tool: String,
    /// Tool parameters
    pub parameters: serde_json::Value,
    /// Tool result
    pub result: Option<ToolResult>,
}

/// Session state for trajectory recording
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionState {
    /// Current turn number
    pub turn: u32,
    /// Total tokens used (input)
    pub tokens_in: u64,
    /// Total tokens used (output)
    pub tokens_out: u64,
}

/// An active GPT-OSS agent session with conversation history
pub struct GptOssSession {
    /// Unique session ID
    pub id: String,
    /// Client for inference
    client: Arc<GptOssClient>,
    /// Session configuration
    config: GptOssAgentConfig,
    /// Available tools
    tools: Vec<Arc<dyn Tool>>,
    /// Conversation history
    history: RwLock<Vec<Message>>,
    /// Session state
    state: RwLock<SessionState>,
    /// Trajectory file path (if recording)
    trajectory_path: Option<PathBuf>,
}

impl GptOssSession {
    /// Create a new session
    pub fn new(
        client: Arc<GptOssClient>,
        config: GptOssAgentConfig,
        tools: Vec<Arc<dyn Tool>>,
    ) -> Self {
        let id = format!("gpt-oss-{}", Uuid::new_v4());

        let trajectory_path = if config.record_trajectory {
            Some(
                config
                    .workspace_root
                    .join("docs/logs")
                    .join(chrono::Local::now().format("%Y%m%d").to_string())
                    .join(format!("{}.rlog", id)),
            )
        } else {
            None
        };

        Self {
            id,
            client,
            config,
            tools,
            history: RwLock::new(Vec::new()),
            state: RwLock::new(SessionState::default()),
            trajectory_path,
        }
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.id
    }

    /// Send a user message and get a response
    pub async fn send(&self, message: &str) -> Result<String> {
        // Add user message to history
        {
            let mut history = self.history.write().await;
            history.push(Message {
                role: "user".to_string(),
                content: message.to_string(),
                tool_calls: Vec::new(),
            });
        }

        // Record to trajectory if enabled
        self.record_line(&format!("u: {}", message)).await?;

        // Build prompt from history
        let prompt = self.build_prompt().await;

        // Make completion request
        let request = GptOssRequest {
            model: self.config.model.clone(),
            prompt,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
        };

        let response = self.client.complete(request).await?;

        // Add assistant message to history
        {
            let mut history = self.history.write().await;
            history.push(Message {
                role: "assistant".to_string(),
                content: response.text.clone(),
                tool_calls: Vec::new(),
            });
        }

        // Update turn counter
        {
            let mut state = self.state.write().await;
            state.turn += 1;
        }

        // Record to trajectory
        self.record_line(&format!("a: {}", response.text)).await?;

        Ok(response.text)
    }

    /// Execute a tool and record the result
    pub async fn execute_tool(&self, request: ToolRequest) -> Result<ToolResult> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == request.tool)
            .ok_or_else(|| GptOssAgentError::ToolError(format!("Tool not found: {}", request.tool)))?;

        // Record tool start
        self.record_line(&format!(
            "t!:{} {} -> [running]",
            request.tool,
            serde_json::to_string(&request.parameters).unwrap_or_default()
        ))
        .await?;

        let result = tool.execute(request.parameters.clone()).await?;

        // Record tool result
        let result_str = if result.success {
            format!("[ok] {}", result.output)
        } else {
            format!("[error] {}", result.error.as_deref().unwrap_or("unknown"))
        };
        self.record_line(&format!("t:{} -> {}", request.tool, result_str))
            .await?;

        // Add tool call to last assistant message
        {
            let mut history = self.history.write().await;
            if let Some(last) = history.last_mut() {
                if last.role == "assistant" {
                    last.tool_calls.push(ToolCall {
                        tool: request.tool,
                        parameters: request.parameters,
                        result: Some(result.clone()),
                    });
                }
            }
        }

        Ok(result)
    }

    /// Get conversation history
    pub async fn history(&self) -> Vec<Message> {
        self.history.read().await.clone()
    }

    /// Get session state
    pub async fn state(&self) -> SessionState {
        self.state.read().await.clone()
    }

    /// Clear conversation history
    pub async fn clear(&self) {
        let mut history = self.history.write().await;
        history.clear();

        let mut state = self.state.write().await;
        state.turn = 0;
    }

    /// Check if trajectory recording is enabled
    pub fn is_recording(&self) -> bool {
        self.trajectory_path.is_some()
    }

    /// Build prompt from conversation history
    async fn build_prompt(&self) -> String {
        let history = self.history.read().await;

        let mut prompt = String::new();
        for msg in history.iter() {
            let prefix = if msg.role == "user" { "User" } else { "Assistant" };
            prompt.push_str(&format!("{}: {}\n", prefix, msg.content));
        }
        prompt.push_str("Assistant:");

        prompt
    }

    /// Record a line to the trajectory file
    async fn record_line(&self, line: &str) -> Result<()> {
        let Some(path) = &self.trajectory_path else {
            return Ok(());
        };

        use std::io::Write;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Check if file exists to determine if we need to write header
        let needs_header = !path.exists();

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        if needs_header {
            // Write YAML header
            let header = format!(
                r#"---
format: rlog/1
id: {}
agent: gpt-oss
model: {}
repo_sha: unknown
cwd: {}
---

"#,
                self.id,
                self.config.model,
                self.config.workspace_root.display()
            );
            file.write_all(header.as_bytes())?;
        }

        writeln!(file, "{}", line)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_creation() {
        let config = GptOssAgentConfig::default();
        let client = GptOssClient::builder()
            .base_url("http://localhost:8080")
            .build()
            .unwrap();

        let session = GptOssSession::new(Arc::new(client), config, Vec::new());

        assert!(session.session_id().starts_with("gpt-oss-"));
        assert!(session.history().await.is_empty());
    }

    #[tokio::test]
    async fn test_session_history() {
        let config = GptOssAgentConfig::default();
        let client = GptOssClient::builder()
            .base_url("http://localhost:8080")
            .build()
            .unwrap();

        let session = GptOssSession::new(Arc::new(client), config, Vec::new());

        // Manually add messages for testing
        {
            let mut history = session.history.write().await;
            history.push(Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
                tool_calls: Vec::new(),
            });
        }

        let history = session.history().await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].role, "user");
        assert_eq!(history[0].content, "Hello");
    }
}
