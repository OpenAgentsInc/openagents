//! Claude Code binary discovery and connection management.
//!
//! This module provides the ClaudeCode struct for discovering and connecting
//! to the Claude Code CLI via the ACP protocol.

use agent_client_protocol as acp;
use anyhow::{Context as _, Result};
use gpui::{App, SharedString, Task};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::connection::{AcpConnection, AgentServerCommand};
use crate::types::{AgentConnection, Project};

/// Claude Code agent server.
#[derive(Clone, Debug)]
pub struct ClaudeCode {
    /// Default mode to use.
    pub default_mode: Option<String>,
    /// Default model to use.
    pub default_model: Option<String>,
}

impl Default for ClaudeCode {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeCode {
    /// Create a new ClaudeCode instance.
    pub fn new() -> Self {
        Self {
            default_mode: None,
            default_model: None,
        }
    }

    /// Set the default mode.
    pub fn with_default_mode(mut self, mode: impl Into<String>) -> Self {
        self.default_mode = Some(mode.into());
        self
    }

    /// Set the default model.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Get the telemetry ID.
    pub fn telemetry_id(&self) -> &'static str {
        "claude-code"
    }

    /// Get the display name.
    pub fn name(&self) -> SharedString {
        "Claude Code".into()
    }

    /// Find the Claude Code binary.
    ///
    /// This searches for the Claude Code CLI in the following order:
    /// 1. CLAUDE_CODE_EXECUTABLE environment variable
    /// 2. npx @zed-industries/claude-code-acp
    /// 3. claude command in PATH
    pub fn find_binary() -> Result<AgentServerCommand> {
        // Check environment variable first
        if let Ok(path) = std::env::var("CLAUDE_CODE_EXECUTABLE") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Ok(AgentServerCommand {
                    path,
                    args: vec!["--acp".to_string()],
                    env: None,
                });
            }
        }

        // Try npx with the Zed ACP adapter
        if let Ok(npx_path) = which::which("npx") {
            return Ok(AgentServerCommand {
                path: npx_path,
                args: vec!["@zed-industries/claude-code-acp".to_string()],
                env: Some(
                    [
                        // Force no API key - user auth only
                        ("ANTHROPIC_API_KEY".to_string(), "".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
            });
        }

        // Try claude command directly
        if let Ok(claude_path) = which::which("claude") {
            return Ok(AgentServerCommand {
                path: claude_path,
                args: vec!["--acp".to_string()],
                env: None,
            });
        }

        anyhow::bail!(
            "Could not find Claude Code. Please either:\n\
             - Set CLAUDE_CODE_EXECUTABLE environment variable\n\
             - Install npx (npm) for automatic installation\n\
             - Install Claude Code CLI and ensure 'claude' is in PATH"
        )
    }

    /// Connect to Claude Code.
    pub fn connect(
        &self,
        root_dir: &Path,
        cx: &mut App,
    ) -> Task<Result<Rc<dyn AgentConnection>>> {
        let name = self.name();
        let telemetry_id = self.telemetry_id();
        let root_dir = root_dir.to_path_buf();
        let default_mode = self.default_mode.clone().map(acp::SessionModeId::new);
        let default_model = self.default_model.clone().map(acp::ModelId::new);

        cx.spawn(async move |mut cx| {
            let command = Self::find_binary()?;

            let connection = AcpConnection::stdio(
                name,
                telemetry_id,
                command,
                &root_dir,
                default_mode,
                default_model,
                &mut cx,
            )
            .await?;

            Ok(Rc::new(connection) as Rc<dyn AgentConnection>)
        })
    }

    /// Create a new thread with Claude Code.
    pub fn new_thread(
        connection: Rc<dyn AgentConnection>,
        project: Project,
        cx: &mut App,
    ) -> Task<Result<gpui::Entity<crate::AcpThread>>> {
        let cwd = project.root_path.clone();

        // Downcast to AcpConnection
        let acp_conn = match connection
            .clone()
            .into_any()
            .downcast::<AcpConnection>()
            .ok()
        {
            Some(conn) => conn,
            None => return Task::ready(Err(anyhow::anyhow!("Expected AcpConnection"))),
        };

        acp_conn.new_thread(project, &cwd, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_code_defaults() {
        let claude = ClaudeCode::new();
        assert_eq!(claude.telemetry_id(), "claude-code");
        assert_eq!(claude.name().as_ref(), "Claude Code");
        assert!(claude.default_mode.is_none());
        assert!(claude.default_model.is_none());
    }

    #[test]
    fn test_claude_code_with_defaults() {
        let claude = ClaudeCode::new()
            .with_default_mode("bypassPermissions")
            .with_default_model("claude-sonnet-4");

        assert_eq!(claude.default_mode, Some("bypassPermissions".to_string()));
        assert_eq!(claude.default_model, Some("claude-sonnet-4".to_string()));
    }
}
