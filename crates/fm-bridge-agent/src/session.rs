//! FM Bridge agent session management
//!
//! Provides session tracking with conversation history and optional
//! trajectory recording for reproducibility.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use fm_bridge::{CompletionOptions, FMClient};
use gpt_oss_agent::tools::{Tool, ToolRequest, ToolResult};

use crate::FmBridgeAgentConfig;
use crate::error::{FmBridgeAgentError, Result};

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

/// An active FM Bridge agent session with conversation history
pub struct FmBridgeSession {
    /// Unique session ID
    pub id: String,
    /// Client for inference
    client: Arc<FMClient>,
    /// Session configuration
    config: FmBridgeAgentConfig,
    /// Available tools
    tools: Vec<Arc<dyn Tool>>,
    /// Conversation history
    history: RwLock<Vec<Message>>,
    /// Session state
    state: RwLock<SessionState>,
    /// Trajectory file path (if recording)
    trajectory_path: Option<PathBuf>,
}

impl FmBridgeSession {
    /// Create a new session
    pub fn new(
        client: Arc<FMClient>,
        config: FmBridgeAgentConfig,
        tools: Vec<Arc<dyn Tool>>,
    ) -> Self {
        let id = format!("fm-bridge-{}", Uuid::new_v4());

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
        {
            let mut history = self.history.write().await;
            history.push(Message {
                role: "user".to_string(),
                content: message.to_string(),
                tool_calls: Vec::new(),
            });
        }

        self.record_line(&format!("u: {}", message)).await?;

        let prompt = self.build_prompt().await;

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

        {
            let mut history = self.history.write().await;
            history.push(Message {
                role: "assistant".to_string(),
                content: content.clone(),
                tool_calls: Vec::new(),
            });
        }

        {
            let mut state = self.state.write().await;
            state.turn += 1;
        }

        self.record_line(&format!("a: {}", content)).await?;

        Ok(content)
    }

    /// Execute a tool and record the result
    pub async fn execute_tool(&self, request: ToolRequest) -> Result<ToolResult> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == request.tool)
            .ok_or_else(|| {
                FmBridgeAgentError::ToolError(format!("Tool not found: {}", request.tool))
            })?;

        self.record_line(&format!(
            "t!:{} {} -> [running]",
            request.tool,
            serde_json::to_string(&request.parameters).unwrap_or_default()
        ))
        .await?;

        let result = tool.execute(request.parameters.clone()).await?;

        let result_str = if result.success {
            format!("[ok] {}", result.output)
        } else {
            format!("[error] {}", result.error.as_deref().unwrap_or("unknown"))
        };
        self.record_line(&format!("t:{} -> {}", request.tool, result_str))
            .await?;

        {
            let mut history = self.history.write().await;
            if let Some(last) = history.last_mut()
                && last.role == "assistant"
            {
                last.tool_calls.push(ToolCall {
                    tool: request.tool,
                    parameters: request.parameters,
                    result: Some(result.clone()),
                });
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
            let prefix = if msg.role == "user" {
                "User"
            } else {
                "Assistant"
            };
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

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let needs_header = !path.exists();

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        if needs_header {
            let header = format!(
                r#"---
format: rlog/1
id: {}
agent: fm-bridge
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
        let config = FmBridgeAgentConfig::default();
        let client = FMClient::builder()
            .base_url("http://localhost:3030")
            .build()
            .unwrap();

        let session = FmBridgeSession::new(Arc::new(client), config, Vec::new());

        assert!(session.session_id().starts_with("fm-bridge-"));
        assert!(session.history().await.is_empty());
    }
}
