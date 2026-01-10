//! Claude Code backend implementation
//!
//! Wraps the claude-agent-sdk to implement AgentBackend.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use claude_agent_sdk::permissions::{AllowAllPermissions, CallbackPermissionHandler, PermissionRequest};
use claude_agent_sdk::protocol::{PermissionMode, PermissionResult};
use claude_agent_sdk::{query_with_permissions, QueryOptions, SdkMessage, Query};

use super::backend::{
    AgentAvailability, AgentBackend, AgentConfig, AgentKind, AgentSession, ModelInfo,
};
use crate::app::events::ResponseEvent;
use crate::app::permissions::PermissionPending;
use crate::app::tools::tool_result_output;

/// Claude Code backend
pub struct ClaudeBackend {
    /// Cached availability status
    availability: std::sync::RwLock<Option<AgentAvailability>>,
}

impl ClaudeBackend {
    pub fn new() -> Self {
        Self {
            availability: std::sync::RwLock::new(None),
        }
    }
}

impl Default for ClaudeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentBackend for ClaudeBackend {
    fn kind(&self) -> AgentKind {
        AgentKind::Claude
    }

    fn check_availability(&self) -> AgentAvailability {
        // Check cache first
        if let Some(cached) = self.availability.read().unwrap().as_ref() {
            return cached.clone();
        }

        let result = check_claude_availability();

        // Cache the result
        *self.availability.write().unwrap() = Some(result.clone());

        result
    }

    fn available_models(&self) -> Pin<Box<dyn Future<Output = Vec<ModelInfo>> + Send + '_>> {
        Box::pin(async move {
            // Claude models are well-known, return static list
            // In the future, we could query the agent for available models
            vec![
                ModelInfo {
                    id: "claude-opus-4-5-20251101".to_string(),
                    name: "Claude Opus 4.5".to_string(),
                    description: Some("Most capable for complex work".to_string()),
                    is_default: true,
                },
                ModelInfo {
                    id: "claude-sonnet-4-5-20250929".to_string(),
                    name: "Claude Sonnet 4.5".to_string(),
                    description: Some("Best for everyday tasks".to_string()),
                    is_default: false,
                },
                ModelInfo {
                    id: "claude-haiku-4-5-20251001".to_string(),
                    name: "Claude Haiku 4.5".to_string(),
                    description: Some("Fastest for quick answers".to_string()),
                    is_default: false,
                },
            ]
        })
    }

    fn default_model_id(&self) -> Option<&str> {
        Some("claude-opus-4-5-20251101")
    }

    fn connect(
        &self,
        config: AgentConfig,
        response_tx: mpsc::UnboundedSender<ResponseEvent>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Box<dyn AgentSession>>> + Send + '_>> {
        Box::pin(async move {
            let session = ClaudeSession::new(config, response_tx);
            Ok(Box::new(session) as Box<dyn AgentSession>)
        })
    }
}

/// Check if Claude CLI is available
fn check_claude_availability() -> AgentAvailability {
    // Try to find claude executable
    match which::which("claude") {
        Ok(path) => AgentAvailability {
            available: true,
            executable_path: Some(path),
            version: None, // Could run `claude --version` but adds startup latency
            error: None,
        },
        Err(_) => {
            // Check common installation paths
            let home = std::env::var("HOME").unwrap_or_default();
            let paths = [
                format!("{}/.claude/local/claude", home),
                format!("{}/.npm-global/bin/claude", home),
                format!("{}/.local/bin/claude", home),
                "/usr/local/bin/claude".to_string(),
                "/opt/homebrew/bin/claude".to_string(),
            ];

            for path in &paths {
                let path = std::path::PathBuf::from(path);
                if path.exists() {
                    return AgentAvailability {
                        available: true,
                        executable_path: Some(path),
                        version: None,
                        error: None,
                    };
                }
            }

            AgentAvailability {
                available: false,
                executable_path: None,
                version: None,
                error: Some("Claude CLI not found. Install with: npm install -g @anthropic-ai/claude-code".to_string()),
            }
        }
    }
}

/// Claude session implementation
pub struct ClaudeSession {
    config: AgentConfig,
    response_tx: mpsc::UnboundedSender<ResponseEvent>,
    session_id: Option<String>,
    stream: Option<Query>,
}

impl ClaudeSession {
    fn new(config: AgentConfig, response_tx: mpsc::UnboundedSender<ResponseEvent>) -> Self {
        Self {
            config,
            response_tx,
            session_id: None,
            stream: None,
        }
    }
}

#[async_trait]
impl AgentSession for ClaudeSession {
    async fn prompt(&mut self, text: &str) -> anyhow::Result<()> {
        let mut options = QueryOptions::new()
            .cwd(&self.config.cwd)
            .include_partial_messages(true);

        if let Some(model) = &self.config.model_id {
            options = options.model(model);
        }

        if let Some(mode) = &self.config.permission_mode {
            let sdk_mode = match mode.as_str() {
                "bypassPermissions" => PermissionMode::BypassPermissions,
                "plan" => PermissionMode::Plan,
                "acceptEdits" => PermissionMode::AcceptEdits,
                "dontAsk" => PermissionMode::DontAsk,
                _ => PermissionMode::Default,
            };
            options = options.permission_mode(sdk_mode);
        }

        if let Some(resume) = &self.config.resume_session {
            options = options.resume(resume.clone());
        }

        if self.config.fork_session {
            options = options.fork_session(true);
        }

        options.max_thinking_tokens = self.config.max_thinking_tokens;
        options.persist_session = self.config.persist_session;

        // Note: mcp_servers and agents in AgentConfig are Vec<Value>
        // but QueryOptions expects HashMap<String, McpServerConfig/AgentDefinition>.
        // Conversion will be added when we fully integrate the agent backends.
        // For now, these are passed through the main coder_actions.rs flow.

        // Use bypass permissions for now - the full permission handling
        // will be added when we refactor coder_actions.rs
        let permissions = Arc::new(AllowAllPermissions);

        let mut stream = query_with_permissions(text, options, permissions).await?;

        // Store the stream for interrupt/abort
        // Note: We'll process events in the main event loop
        self.stream = Some(stream);

        Ok(())
    }

    async fn interrupt(&mut self) -> anyhow::Result<()> {
        if let Some(stream) = &mut self.stream {
            stream.interrupt().await?;
        }
        Ok(())
    }

    async fn abort(&mut self) -> anyhow::Result<()> {
        if let Some(stream) = &mut self.stream {
            stream.abort().await?;
        }
        Ok(())
    }

    fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    async fn mcp_server_status(&mut self) -> anyhow::Result<serde_json::Value> {
        if let Some(stream) = &mut self.stream {
            stream.mcp_server_status().await.map_err(|e| anyhow::anyhow!("{}", e))
        } else {
            Ok(serde_json::json!({}))
        }
    }

    async fn rewind_files(&mut self, user_message_id: &str) -> anyhow::Result<()> {
        if let Some(stream) = &mut self.stream {
            stream.rewind_files(user_message_id).await.map_err(|e| anyhow::anyhow!("{}", e))
        } else {
            Ok(())
        }
    }
}
