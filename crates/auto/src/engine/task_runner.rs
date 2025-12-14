//! Task runner - executes a single task using the selected backend.

use crate::config::AutoConfig;
use crate::detection::Detection;
use crate::discovery::{DiscoveredTask, TaskDiscoverySource};
use crate::{AutoError, Result};
use mechacoder::router::Backend;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Runs a single task using the selected backend.
pub struct TaskRunner {
    config: AutoConfig,
    detection: Detection,
}

impl TaskRunner {
    /// Create a new task runner.
    pub fn new(config: AutoConfig, detection: Detection) -> Self {
        Self { config, detection }
    }

    /// Run a task and return commits made.
    pub async fn run(&self, task: &DiscoveredTask) -> Result<Vec<String>> {
        let backend = self
            .detection
            .selected_backend()
            .ok_or(AutoError::NoBackend)?;

        match backend {
            Backend::ClaudeCode => self.run_with_claude_code(task).await,
            Backend::Anthropic | Backend::OpenRouter | Backend::OpenAI => {
                self.run_with_api(task, backend).await
            }
            Backend::Ollama => self.run_with_ollama(task).await,
            Backend::Pi => self.run_with_pi(task).await,
            Backend::OpenAgentsCloud => {
                Err(AutoError::Backend("OpenAgents Cloud not yet implemented".to_string()))
            }
        }
    }

    /// Run task using Claude Code CLI.
    async fn run_with_claude_code(&self, task: &DiscoveredTask) -> Result<Vec<String>> {
        // Build the prompt
        let prompt = self.build_prompt(task);

        // Find claude CLI
        let claude_path = self.find_claude_cli()?;

        // Run claude with the prompt
        let mut cmd = Command::new(&claude_path);
        cmd.arg("--print")
            .arg("-p")
            .arg(&prompt)
            .current_dir(&self.config.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            AutoError::Backend(format!("Failed to spawn claude: {}", e))
        })?;

        // Read output
        let stdout = child.stdout.take().expect("stdout not captured");
        let mut reader = BufReader::new(stdout).lines();

        let mut output = String::new();
        while let Some(line) = reader.next_line().await? {
            output.push_str(&line);
            output.push('\n');
        }

        let status = child.wait().await?;
        if !status.success() {
            return Err(AutoError::Backend(format!(
                "Claude CLI exited with status: {}",
                status
            )));
        }

        // Extract commits from git log
        let commits = self.get_recent_commits().await?;

        Ok(commits)
    }

    /// Run task using API provider (Anthropic, OpenRouter, OpenAI).
    async fn run_with_api(
        &self,
        task: &DiscoveredTask,
        _backend: Backend,
    ) -> Result<Vec<String>> {
        // For now, fall back to Claude CLI if available
        // Full API implementation would use coder_service
        let prompt = self.build_prompt(task);

        tracing::info!(
            task_id = %task.id,
            prompt_len = prompt.len(),
            "API backend execution not yet implemented, task prompt prepared"
        );

        // Return empty commits - full implementation would:
        // 1. Create a coder_service::ChatService
        // 2. Send the prompt
        // 3. Handle tool calls
        // 4. Track commits
        Ok(vec![])
    }

    /// Run task using Ollama.
    async fn run_with_ollama(&self, task: &DiscoveredTask) -> Result<Vec<String>> {
        tracing::info!(task_id = %task.id, "Ollama backend not yet implemented");
        Ok(vec![])
    }

    /// Run task using Pi agent.
    async fn run_with_pi(&self, task: &DiscoveredTask) -> Result<Vec<String>> {
        tracing::info!(task_id = %task.id, "Pi backend not yet implemented");
        Ok(vec![])
    }

    /// Build the prompt for a task.
    fn build_prompt(&self, task: &DiscoveredTask) -> String {
        let mut prompt = String::new();

        // Task title
        prompt.push_str(&format!("# Task: {}\n\n", task.title));

        // Task description
        if let Some(desc) = &task.description {
            prompt.push_str(desc);
            prompt.push_str("\n\n");
        }

        // Source-specific context
        match &task.source {
            TaskDiscoverySource::Taskmaster { issue_id } => {
                prompt.push_str(&format!("Task ID: {}\n", issue_id));
            }
            TaskDiscoverySource::Plan { plan_path } => {
                prompt.push_str(&format!("Plan file: {}\n", plan_path.display()));
            }
            TaskDiscoverySource::Explicit => {
                prompt.push_str("Explicitly requested task.\n");
            }
        }

        // Labels
        if !task.labels.is_empty() {
            prompt.push_str(&format!("\nLabels: {}\n", task.labels.join(", ")));
        }

        // Instructions
        prompt.push_str("\n## Instructions\n\n");
        prompt.push_str("Please complete this task. ");
        if self.config.auto_commit {
            prompt.push_str("Create appropriate git commits for your changes. ");
        }
        prompt.push_str("Explain your approach and implementation.\n");

        prompt
    }

    /// Find the Claude CLI path.
    fn find_claude_cli(&self) -> Result<String> {
        let home = std::env::var("HOME").unwrap_or_default();
        let known_paths = [
            format!("{}/.claude/local/claude", home),
            format!("{}/.npm-global/bin/claude", home),
            format!("{}/.local/bin/claude", home),
            "/usr/local/bin/claude".to_string(),
            "/opt/homebrew/bin/claude".to_string(),
        ];

        for path in &known_paths {
            if std::path::Path::new(path).exists() {
                return Ok(path.clone());
            }
        }

        // Try which
        let output = std::process::Command::new("which")
            .arg("claude")
            .output()
            .ok();

        if let Some(output) = output {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Ok(path);
                }
            }
        }

        Err(AutoError::Backend("Claude CLI not found".to_string()))
    }

    /// Get recent commits from git log.
    async fn get_recent_commits(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(["log", "--oneline", "-5", "--format=%H"])
            .current_dir(&self.config.working_directory)
            .output()
            .await?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let commits: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(commits)
    }
}
