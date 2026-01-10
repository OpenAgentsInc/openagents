//! Codex backend implementation
//!
//! Wraps the codex-agent-sdk to implement AgentBackend.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use codex_agent_sdk::{
    Codex, ThreadEvent, ThreadOptions, TurnOptions, StreamedTurn, SandboxMode, ApprovalMode,
};

use super::backend::{
    AgentAvailability, AgentBackend, AgentConfig, AgentKind, AgentSession, ModelInfo,
};
use crate::app::events::ResponseEvent;

/// Codex backend
pub struct CodexBackend {
    /// Cached availability status
    availability: std::sync::RwLock<Option<AgentAvailability>>,
}

impl CodexBackend {
    pub fn new() -> Self {
        Self {
            availability: std::sync::RwLock::new(None),
        }
    }
}

impl Default for CodexBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentBackend for CodexBackend {
    fn kind(&self) -> AgentKind {
        AgentKind::Codex
    }

    fn check_availability(&self) -> AgentAvailability {
        // Check cache first
        if let Some(cached) = self.availability.read().unwrap().as_ref() {
            return cached.clone();
        }

        let result = check_codex_availability();

        // Cache the result
        *self.availability.write().unwrap() = Some(result.clone());

        result
    }

    fn available_models(&self) -> Pin<Box<dyn Future<Output = Vec<ModelInfo>> + Send + '_>> {
        Box::pin(async move {
            // Codex models - these are OpenAI models
            vec![
                ModelInfo {
                    id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    description: Some("Latest GPT-4 Omni model".to_string()),
                    is_default: true,
                },
                ModelInfo {
                    id: "gpt-4o-mini".to_string(),
                    name: "GPT-4o Mini".to_string(),
                    description: Some("Faster, cost-effective GPT-4o".to_string()),
                    is_default: false,
                },
                ModelInfo {
                    id: "o1".to_string(),
                    name: "O1".to_string(),
                    description: Some("Reasoning-optimized model".to_string()),
                    is_default: false,
                },
                ModelInfo {
                    id: "o1-mini".to_string(),
                    name: "O1 Mini".to_string(),
                    description: Some("Faster O1 variant".to_string()),
                    is_default: false,
                },
                ModelInfo {
                    id: "o3-mini".to_string(),
                    name: "O3 Mini".to_string(),
                    description: Some("Latest reasoning model".to_string()),
                    is_default: false,
                },
            ]
        })
    }

    fn default_model_id(&self) -> Option<&str> {
        Some("gpt-4o")
    }

    fn connect(
        &self,
        config: AgentConfig,
        response_tx: mpsc::UnboundedSender<ResponseEvent>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Box<dyn AgentSession>>> + Send + '_>> {
        Box::pin(async move {
            let session = CodexSession::new(config, response_tx);
            Ok(Box::new(session) as Box<dyn AgentSession>)
        })
    }
}

/// Check if Codex CLI is available
fn check_codex_availability() -> AgentAvailability {
    // Try to find codex executable
    match which::which("codex") {
        Ok(path) => AgentAvailability {
            available: true,
            executable_path: Some(path),
            version: None,
            error: None,
        },
        Err(_) => {
            // Check common installation paths
            let home = std::env::var("HOME").unwrap_or_default();
            let paths = [
                format!("{}/.npm-global/bin/codex", home),
                format!("{}/.local/bin/codex", home),
                "/usr/local/bin/codex".to_string(),
                "/opt/homebrew/bin/codex".to_string(),
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
                error: Some("Codex CLI not found. Install with: npm install -g @openai/codex".to_string()),
            }
        }
    }
}

/// Codex session implementation
pub struct CodexSession {
    config: AgentConfig,
    response_tx: mpsc::UnboundedSender<ResponseEvent>,
    thread_id: Option<String>,
    codex: Codex,
    streamed_turn: Option<StreamedTurn>,
}

impl CodexSession {
    fn new(config: AgentConfig, response_tx: mpsc::UnboundedSender<ResponseEvent>) -> Self {
        let codex = Codex::new();
        Self {
            config,
            response_tx,
            thread_id: None,
            codex,
            streamed_turn: None,
        }
    }

    fn build_thread_options(&self) -> ThreadOptions {
        let mut options = ThreadOptions::new()
            .working_directory(&self.config.cwd)
            .skip_git_repo_check(true); // Coder handles git checks itself

        if let Some(model) = &self.config.model_id {
            options = options.model(model);
        }

        // Map permission mode to sandbox mode
        if let Some(mode) = &self.config.permission_mode {
            let sandbox = match mode.as_str() {
                "bypassPermissions" => SandboxMode::DangerFullAccess,
                "plan" => SandboxMode::ReadOnly,
                _ => SandboxMode::WorkspaceWrite,
            };
            options = options.sandbox_mode(sandbox);

            // Also set approval policy based on mode
            let approval = match mode.as_str() {
                "bypassPermissions" => ApprovalMode::Never,
                "plan" => ApprovalMode::Never,
                _ => ApprovalMode::OnRequest,
            };
            options = options.approval_policy(approval);
        }

        options
    }
}

#[async_trait]
impl AgentSession for CodexSession {
    async fn prompt(&mut self, text: &str) -> anyhow::Result<()> {
        let thread_options = self.build_thread_options();

        // Create or resume thread
        let mut thread = if let Some(thread_id) = &self.config.resume_session {
            self.codex.resume_thread(thread_id, thread_options)
        } else {
            self.codex.start_thread(thread_options)
        };

        // Start streaming turn
        let turn_options = TurnOptions::default();
        let streamed = thread.run_streamed(text, turn_options).await?;

        // Store for later
        self.streamed_turn = Some(streamed);

        Ok(())
    }

    async fn interrupt(&mut self) -> anyhow::Result<()> {
        // Codex doesn't have a direct interrupt mechanism like Claude
        // The streamed turn will be dropped when we reset
        self.streamed_turn = None;
        Ok(())
    }

    async fn abort(&mut self) -> anyhow::Result<()> {
        self.streamed_turn = None;
        Ok(())
    }

    fn session_id(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }

    async fn mcp_server_status(&mut self) -> anyhow::Result<serde_json::Value> {
        // Codex doesn't have MCP integration in the same way as Claude
        Ok(serde_json::json!({}))
    }

    async fn rewind_files(&mut self, _user_message_id: &str) -> anyhow::Result<()> {
        // Codex doesn't support checkpoints in the same way
        Ok(())
    }
}
