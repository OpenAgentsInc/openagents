//! GPT-OSS Agent session management
//!
//! Provides session tracking with conversation history and optional
//! trajectory recording for reproducibility.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use gpt_oss::{
    GptOssClient, GptOssRequest, HarmonyRenderer, HarmonyRole, HarmonyToolSpec, HarmonyTurn,
};

use crate::GptOssAgentConfig;
use crate::error::{GptOssAgentError, Result};
use crate::tools::{Tool, ToolRequest, ToolResult};

/// A message in the conversation history
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    /// Role: "user" or "assistant"
    pub role: String,
    /// Message content
    pub content: String,
    /// Optional tool calls made in this turn
    pub tool_calls: Vec<ToolCall>,
    /// Optional recipient (used for tool calls)
    pub recipient: Option<String>,
    /// Optional author name (used for tool responses)
    pub name: Option<String>,
    /// Optional channel (analysis/commentary/final)
    pub channel: Option<String>,
    /// Optional content type (e.g., "code")
    pub content_type: Option<String>,
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
    /// Override max tokens for completions
    max_tokens: Option<usize>,
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
            max_tokens: None,
            tools,
            history: RwLock::new(Vec::new()),
            state: RwLock::new(SessionState::default()),
            trajectory_path,
        }
    }

    /// Set a max token override for this session.
    pub fn with_max_tokens(mut self, max_tokens: Option<usize>) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.id
    }

    /// Send a user message and get a response
    pub async fn send(&self, message: &str) -> Result<String> {
        self.add_user_message(message).await?;
        let (assistant_text, _) = self.complete_once().await?;
        Ok(assistant_text)
    }

    /// Send a user message and execute tool calls until completion
    pub async fn send_with_tools(&self, message: &str, max_tool_turns: u32) -> Result<String> {
        self.add_user_message(message).await?;

        let mut tool_turns = 0;
        loop {
            let (assistant_text, tool_requests) = self.complete_once().await?;
            if tool_requests.is_empty() {
                return Ok(assistant_text);
            }

            if tool_turns >= max_tool_turns {
                return Ok(assistant_text);
            }

            tool_turns += 1;
            for request in tool_requests {
                let result = self.execute_tool(request.clone()).await?;
                self.append_tool_result(&request.tool, &result).await?;
            }
        }
    }

    async fn add_user_message(&self, message: &str) -> Result<()> {
        {
            let mut history = self.history.write().await;
            history.push(Message {
                role: "user".to_string(),
                content: message.to_string(),
                tool_calls: Vec::new(),
                recipient: None,
                name: None,
                channel: None,
                content_type: None,
            });
        }

        self.record_line(&format!("u: {}", message)).await?;
        Ok(())
    }

    async fn complete_once(&self) -> Result<(String, Vec<ToolRequest>)> {
        let prompt = self.build_prompt().await?;

        let request = GptOssRequest {
            model: self.config.model.clone(),
            prompt,
            max_tokens: self.max_tokens,
            temperature: None,
            top_p: None,
            stop: None,
            stream: false,
        };

        let response = self.client.complete(request).await?;
        let (assistant_text, parsed_messages) =
            self.parse_completion_messages(&response.text).await?;

        let tool_requests = self.extract_tool_requests(&parsed_messages)?;

        {
            let mut history = self.history.write().await;
            if parsed_messages.is_empty() {
                history.push(Message {
                    role: "assistant".to_string(),
                    content: assistant_text.clone(),
                    tool_calls: Vec::new(),
                    recipient: None,
                    name: None,
                    channel: None,
                    content_type: None,
                });
            } else {
                history.extend(parsed_messages);
            }
        }

        {
            let mut state = self.state.write().await;
            state.turn += 1;
        }

        self.record_line(&format!("a: {}", assistant_text)).await?;

        Ok((assistant_text, tool_requests))
    }

    async fn parse_completion_messages(&self, completion: &str) -> Result<(String, Vec<Message>)> {
        let renderer = HarmonyRenderer::gpt_oss()?;
        let parsed = renderer.parse_completion(completion, Some(HarmonyRole::Assistant));

        let messages = match parsed {
            Ok(messages) => messages,
            Err(_) => {
                return Ok((completion.to_string(), Vec::new()));
            }
        };

        let assistant_text = {
            let text = self.extract_assistant_text_from_harmony(&messages);
            if text.is_empty() {
                completion.to_string()
            } else {
                text
            }
        };
        let internal_messages = messages
            .iter()
            .map(Self::map_harmony_message)
            .collect::<Vec<_>>();

        Ok((assistant_text, internal_messages))
    }

    fn extract_assistant_text_from_harmony(
        &self,
        messages: &[gpt_oss::harmony::HarmonyMessage],
    ) -> String {
        for message in messages.iter().rev() {
            if message.author.role != HarmonyRole::Assistant {
                continue;
            }
            if message.recipient.is_some() {
                continue;
            }
            let content = Self::extract_text_content(message);
            if !content.is_empty() {
                return content;
            }
        }

        String::new()
    }

    fn map_harmony_message(message: &gpt_oss::harmony::HarmonyMessage) -> Message {
        Message {
            role: message.author.role.as_str().to_string(),
            content: Self::extract_text_content(message),
            tool_calls: Vec::new(),
            recipient: message.recipient.clone(),
            name: message.author.name.clone(),
            channel: message.channel.clone(),
            content_type: message.content_type.clone(),
        }
    }

    fn extract_text_content(message: &gpt_oss::harmony::HarmonyMessage) -> String {
        let mut content = String::new();
        for part in &message.content {
            if let gpt_oss::harmony::HarmonyContent::Text(gpt_oss::harmony::HarmonyTextContent {
                text,
            }) = part
            {
                content.push_str(text);
            }
        }
        content
    }

    fn extract_tool_requests(&self, messages: &[Message]) -> Result<Vec<ToolRequest>> {
        let mut requests = Vec::new();
        for message in messages {
            if message.role != "assistant" {
                continue;
            }
            let Some(tool) = message.recipient.as_ref() else {
                continue;
            };
            let content = message.content.trim();
            let parameters = if content.is_empty() {
                serde_json::Value::Object(serde_json::Map::new())
            } else {
                serde_json::from_str(content).map_err(|err| {
                    GptOssAgentError::ToolError(format!(
                        "Failed to parse tool call payload for {}: {}",
                        tool, err
                    ))
                })?
            };
            requests.push(ToolRequest {
                tool: tool.clone(),
                parameters,
            });
        }
        Ok(requests)
    }

    async fn append_tool_result(&self, tool: &str, result: &ToolResult) -> Result<()> {
        let payload = serde_json::json!({
            "success": result.success,
            "output": result.output,
            "error": result.error,
        });
        let content = serde_json::to_string(&payload)?;

        let mut history = self.history.write().await;
        history.push(Message {
            role: "tool".to_string(),
            content,
            tool_calls: Vec::new(),
            recipient: Some("assistant".to_string()),
            name: Some(tool.to_string()),
            channel: Some("commentary".to_string()),
            content_type: Some("code".to_string()),
        });
        Ok(())
    }

    /// Execute a tool and record the result
    pub async fn execute_tool(&self, request: ToolRequest) -> Result<ToolResult> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == request.tool)
            .ok_or_else(|| {
                GptOssAgentError::ToolError(format!("Tool not found: {}", request.tool))
            })?;

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
            let target_index = history
                .iter()
                .rposition(|msg| {
                    msg.role == "assistant" && msg.recipient.as_deref() == Some(&request.tool)
                })
                .or_else(|| history.iter().rposition(|msg| msg.role == "assistant"));

            if let Some(index) = target_index {
                history[index].tool_calls.push(ToolCall {
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
    async fn build_prompt(&self) -> Result<String> {
        let history = self.history.read().await;

        let mut turns = Vec::new();
        for msg in history.iter() {
            let role = match msg.role.as_str() {
                "user" => HarmonyRole::User,
                "assistant" => HarmonyRole::Assistant,
                "tool" => HarmonyRole::Tool,
                other => {
                    return Err(GptOssAgentError::SessionError(format!(
                        "Unsupported role in history: {}",
                        other
                    )));
                }
            };

            let mut turn = HarmonyTurn::new(role, msg.content.clone());
            if let Some(recipient) = &msg.recipient {
                turn = turn.with_recipient(recipient.clone());
            }
            if let Some(name) = &msg.name {
                turn = turn.with_name(name.clone());
            }
            if let Some(channel) = &msg.channel {
                turn = turn.with_channel(channel.clone());
            }
            if let Some(content_type) = &msg.content_type {
                turn = turn.with_content_type(content_type.clone());
            }
            turns.push(turn);
        }

        let tool_specs = self
            .tools
            .iter()
            .map(|tool| {
                HarmonyToolSpec::new(
                    tool.name(),
                    tool.description(),
                    Some(tool.parameter_schema()),
                )
            })
            .collect::<Vec<_>>();

        let renderer = HarmonyRenderer::gpt_oss()?;
        Ok(renderer.render_prompt(&turns, &tool_specs)?)
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
            .base_url("http://localhost:8000")
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
            .base_url("http://localhost:8000")
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
                recipient: None,
                name: None,
                channel: None,
                content_type: None,
            });
        }

        let history = session.history().await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].role, "user");
        assert_eq!(history[0].content, "Hello");
    }

    #[tokio::test]
    async fn test_extract_tool_requests_from_harmony_messages() {
        let config = GptOssAgentConfig::default();
        let client = GptOssClient::builder()
            .base_url("http://localhost:8000")
            .build()
            .unwrap();
        let session = GptOssSession::new(Arc::new(client), config, Vec::new());

        let messages = vec![Message {
            role: "assistant".to_string(),
            content: r#"{"path":"README.md"}"#.to_string(),
            tool_calls: Vec::new(),
            recipient: Some("apply_patch".to_string()),
            name: None,
            channel: Some("commentary".to_string()),
            content_type: Some("code".to_string()),
        }];

        let requests = session
            .extract_tool_requests(&messages)
            .expect("Should parse tool requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].tool, "apply_patch");
        assert_eq!(
            requests[0]
                .parameters
                .get("path")
                .and_then(|value| value.as_str()),
            Some("README.md")
        );
    }
}
