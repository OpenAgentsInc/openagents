//! Claude Code backend for Bazaar agent jobs
//!
//! Executes PatchGen, CodeReview, SandboxRun jobs using Claude Code.
//! Supports multiple isolation modes:
//! - `local`: Direct execution (requires trust)
//! - `container`: Docker container isolation
//! - `gvisor`: gVisor sandbox (highest security)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::agent::{AgentBackend, AgentCapabilities, AgentError, JobProgress, Result};
use crate::domain::{
    CodeReviewRequest, CodeReviewResult, CommandResult, PatchGenRequest, PatchGenResult,
    PatchVerification, RepoIndexRequest, RepoIndexResult, ResourceUsage, SandboxRunRequest,
    SandboxRunResult, TokenUsage,
};

// ============================================================================
// Configuration
// ============================================================================

/// Isolation mode for Claude Code execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IsolationMode {
    /// Direct execution on host (requires trust)
    Local,
    /// Docker container isolation
    #[default]
    Container,
    /// gVisor sandbox (highest security)
    Gvisor,
}

impl IsolationMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            IsolationMode::Local => "local",
            IsolationMode::Container => "container",
            IsolationMode::Gvisor => "gvisor",
        }
    }
}

/// Configuration for the Claude Code backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeConfig {
    /// Isolation mode for execution
    pub isolation: IsolationMode,
    /// Maximum concurrent jobs
    pub max_workers: u32,
    /// Default model to use
    pub model: String,
    /// Maximum time limit per job in seconds
    pub max_time_limit_secs: u32,
    /// Directory for cloning repositories
    pub work_dir: PathBuf,
    /// Whether to run tests after generating patches
    pub run_tests_by_default: bool,
    /// Container image to use (for container/gvisor modes)
    pub container_image: Option<String>,
    /// Additional environment variables
    pub env: HashMap<String, String>,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            isolation: IsolationMode::Container,
            max_workers: 3,
            model: "claude-sonnet-4".to_string(),
            max_time_limit_secs: 1800, // 30 minutes
            work_dir: PathBuf::from("/tmp/claude-code-jobs"),
            run_tests_by_default: true,
            container_image: None,
            env: HashMap::new(),
        }
    }
}

// ============================================================================
// Backend Implementation
// ============================================================================

/// Claude Code backend for Bazaar jobs
pub struct ClaudeCodeBackend {
    config: ClaudeCodeConfig,
    active_jobs: AtomicU32,
    api_key_available: bool,
    cli_available: bool,
}

impl ClaudeCodeBackend {
    /// Create a new Claude Code backend with the given configuration
    pub fn new(config: ClaudeCodeConfig) -> Self {
        let api_key_available = std::env::var("ANTHROPIC_API_KEY").is_ok();
        let cli_available = check_claude_cli();

        Self {
            config,
            active_jobs: AtomicU32::new(0),
            api_key_available,
            cli_available,
        }
    }

    /// Auto-detect and create a Claude Code backend if available
    pub async fn detect() -> Option<Self> {
        let api_key_available = std::env::var("ANTHROPIC_API_KEY").is_ok();
        let cli_available = check_claude_cli();

        if !api_key_available && !cli_available {
            info!("Claude Code not available: no API key or CLI found");
            return None;
        }

        let config = ClaudeCodeConfig::default();
        let backend = Self::new(config);

        if backend.is_ready().await {
            Some(backend)
        } else {
            None
        }
    }

    /// Get the model to use for a request
    fn resolve_model(&self, requested: Option<&str>) -> String {
        requested
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.config.model.clone())
    }

    /// Clone a repository to the work directory
    async fn clone_repo(
        &self,
        repo: &str,
        git_ref: &str,
        job_id: &str,
        progress: &Option<mpsc::Sender<JobProgress>>,
    ) -> Result<PathBuf> {
        let work_dir = self.config.work_dir.join(job_id);
        tokio::fs::create_dir_all(&work_dir)
            .await
            .map_err(|e| AgentError::RepositoryError(format!("Failed to create work dir: {}", e)))?;

        // Send progress update
        if let Some(tx) = progress {
            let _ = tx
                .send(JobProgress::CloningRepo {
                    repo: repo.to_string(),
                    progress_pct: 0,
                })
                .await;
        }

        // Clone the repository
        let output = Command::new("git")
            .args(["clone", "--depth", "1", "--branch", git_ref, repo, "."])
            .current_dir(&work_dir)
            .output()
            .await
            .map_err(|e| AgentError::RepositoryError(format!("Git clone failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // If branch clone failed, try without --branch (for commit SHAs)
            let output = Command::new("git")
                .args(["clone", repo, "."])
                .current_dir(&work_dir)
                .output()
                .await
                .map_err(|e| AgentError::RepositoryError(format!("Git clone failed: {}", e)))?;

            if !output.status.success() {
                return Err(AgentError::RepositoryError(format!(
                    "Git clone failed: {}",
                    stderr
                )));
            }

            // Checkout specific ref
            let output = Command::new("git")
                .args(["checkout", git_ref])
                .current_dir(&work_dir)
                .output()
                .await
                .map_err(|e| {
                    AgentError::RepositoryError(format!("Git checkout failed: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AgentError::RepositoryError(format!(
                    "Git checkout failed: {}",
                    stderr
                )));
            }
        }

        if let Some(tx) = progress {
            let _ = tx
                .send(JobProgress::CloningRepo {
                    repo: repo.to_string(),
                    progress_pct: 100,
                })
                .await;
        }

        Ok(work_dir)
    }

    /// Run Claude with the given prompt
    async fn run_claude(
        &self,
        work_dir: &Path,
        prompt: &str,
        model: &str,
        time_limit_secs: u32,
        progress: &Option<mpsc::Sender<JobProgress>>,
    ) -> Result<ClaudeOutput> {
        if let Some(tx) = progress {
            let _ = tx
                .send(JobProgress::Thinking {
                    message: "Starting Claude...".to_string(),
                })
                .await;
        }

        // Build the command based on isolation mode
        let output = match self.config.isolation {
            IsolationMode::Local => {
                self.run_claude_local(work_dir, prompt, model, time_limit_secs)
                    .await?
            }
            IsolationMode::Container | IsolationMode::Gvisor => {
                self.run_claude_container(work_dir, prompt, model, time_limit_secs)
                    .await?
            }
        };

        Ok(output)
    }

    /// Run Claude locally (direct execution)
    async fn run_claude_local(
        &self,
        work_dir: &Path,
        prompt: &str,
        model: &str,
        time_limit_secs: u32,
    ) -> Result<ClaudeOutput> {
        let mut cmd = Command::new("claude");
        cmd.current_dir(work_dir)
            .arg("--print")
            .arg("--model")
            .arg(model)
            .arg("--max-turns")
            .arg("50")
            .arg(prompt);

        // Add environment variables
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(time_limit_secs.into()),
            cmd.output(),
        )
        .await
        .map_err(|_| AgentError::Timeout(time_limit_secs))?
        .map_err(|e| AgentError::ExecutionError(format!("Claude execution failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && stdout.is_empty() {
            return Err(AgentError::ExecutionError(format!(
                "Claude failed with exit code {:?}: {}",
                output.status.code(),
                stderr
            )));
        }

        Ok(ClaudeOutput {
            content: stdout,
            stderr,
            exit_code: output.status.code().unwrap_or(-1),
            usage: None, // CLI doesn't provide token usage easily
        })
    }

    /// Run Claude in a container
    async fn run_claude_container(
        &self,
        work_dir: &Path,
        prompt: &str,
        model: &str,
        time_limit_secs: u32,
    ) -> Result<ClaudeOutput> {
        let image = self
            .config
            .container_image
            .as_deref()
            .unwrap_or("ghcr.io/anthropics/claude-code:latest");

        let runtime = match self.config.isolation {
            IsolationMode::Gvisor => "--runtime=runsc",
            _ => "",
        };

        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| AgentError::Unavailable("ANTHROPIC_API_KEY not set".to_string()))?;

        let work_dir_str = work_dir.to_string_lossy();

        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "-v".to_string(),
            format!("{}:/workspace", work_dir_str),
            "-w".to_string(),
            "/workspace".to_string(),
            "-e".to_string(),
            format!("ANTHROPIC_API_KEY={}", api_key),
        ];

        if !runtime.is_empty() {
            args.push(runtime.to_string());
        }

        args.push(image.to_string());
        args.push("claude".to_string());
        args.push("--print".to_string());
        args.push("--model".to_string());
        args.push(model.to_string());
        args.push("--max-turns".to_string());
        args.push("50".to_string());
        args.push(prompt.to_string());

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(time_limit_secs.into()),
            Command::new("docker").args(&args).output(),
        )
        .await
        .map_err(|_| AgentError::Timeout(time_limit_secs))?
        .map_err(|e| AgentError::SandboxError(format!("Docker execution failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && stdout.is_empty() {
            return Err(AgentError::SandboxError(format!(
                "Container execution failed: {}",
                stderr
            )));
        }

        Ok(ClaudeOutput {
            content: stdout,
            stderr,
            exit_code: output.status.code().unwrap_or(-1),
            usage: None,
        })
    }

    /// Extract patch from Claude's output
    fn extract_patch(&self, output: &str) -> Option<String> {
        // Look for diff blocks in the output
        let mut in_diff = false;
        let mut patch_lines = Vec::new();

        for line in output.lines() {
            if line.starts_with("diff --git") || line.starts_with("--- a/") {
                in_diff = true;
            }

            if in_diff {
                patch_lines.push(line);
            }

            // End of diff (empty line or new section)
            if in_diff && line.is_empty() && !patch_lines.is_empty() {
                // Check if this looks like a complete diff
                if patch_lines.iter().any(|l| l.starts_with("@@")) {
                    break;
                }
            }
        }

        if patch_lines.is_empty() {
            None
        } else {
            Some(patch_lines.join("\n"))
        }
    }

    /// Generate git diff from work directory
    async fn get_git_diff(&self, work_dir: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["diff", "HEAD"])
            .current_dir(work_dir)
            .output()
            .await
            .map_err(|e| AgentError::RepositoryError(format!("Git diff failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgentError::RepositoryError(format!(
                "Git diff failed: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get list of modified files
    async fn get_modified_files(&self, work_dir: &Path) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD"])
            .current_dir(work_dir)
            .output()
            .await
            .map_err(|e| AgentError::RepositoryError(format!("Git diff failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }

    /// Run tests in the work directory
    async fn run_tests(
        &self,
        work_dir: &Path,
        test_command: Option<&str>,
        progress: &Option<mpsc::Sender<JobProgress>>,
    ) -> Result<(i32, String)> {
        if let Some(tx) = progress {
            let _ = tx
                .send(JobProgress::Verifying {
                    check: "Running tests...".to_string(),
                })
                .await;
        }

        // Auto-detect test command if not provided
        let cmd = if let Some(cmd) = test_command {
            cmd.to_string()
        } else {
            self.detect_test_command(work_dir).await
        };

        let output = Command::new("sh")
            .args(["-c", &cmd])
            .current_dir(work_dir)
            .output()
            .await
            .map_err(|e| AgentError::ExecutionError(format!("Test execution failed: {}", e)))?;

        let combined_output = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        Ok((output.status.code().unwrap_or(-1), combined_output))
    }

    /// Auto-detect test command based on project files
    async fn detect_test_command(&self, work_dir: &Path) -> String {
        // Check for common project files
        if work_dir.join("Cargo.toml").exists() {
            return "cargo test".to_string();
        }
        if work_dir.join("package.json").exists() {
            return "npm test".to_string();
        }
        if work_dir.join("pyproject.toml").exists() || work_dir.join("setup.py").exists() {
            return "pytest".to_string();
        }
        if work_dir.join("go.mod").exists() {
            return "go test ./...".to_string();
        }
        if work_dir.join("Makefile").exists() {
            return "make test".to_string();
        }

        // Default: no tests
        "true".to_string()
    }

    /// Clean up job work directory
    async fn cleanup(&self, job_id: &str) {
        let work_dir = self.config.work_dir.join(job_id);
        if work_dir.exists() {
            if let Err(e) = tokio::fs::remove_dir_all(&work_dir).await {
                warn!("Failed to clean up work dir {}: {}", work_dir.display(), e);
            }
        }
    }
}

#[async_trait]
impl AgentBackend for ClaudeCodeBackend {
    fn id(&self) -> &str {
        "claude_code"
    }

    async fn is_ready(&self) -> bool {
        self.api_key_available || self.cli_available
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            patch_gen: true,
            code_review: true,
            sandbox_run: true,
            repo_index: false, // Not implemented yet
            max_concurrent_jobs: self.config.max_workers,
            supported_models: vec![
                "claude-sonnet-4".to_string(),
                "claude-opus-4".to_string(),
                "claude-haiku-3-5".to_string(),
            ],
            isolation_mode: self.config.isolation.as_str().to_string(),
            max_time_limit_secs: self.config.max_time_limit_secs,
        }
    }

    async fn patch_gen(
        &self,
        request: PatchGenRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> Result<PatchGenResult> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let start_time = std::time::Instant::now();

        // Track active job
        self.active_jobs.fetch_add(1, Ordering::SeqCst);

        // Send start progress
        if let Some(ref tx) = progress {
            let _ = tx
                .send(JobProgress::Started {
                    job_id: job_id.clone(),
                    estimated_duration_secs: Some(request.time_limit_secs),
                })
                .await;
        }

        // Clone repository
        let work_dir = self
            .clone_repo(&request.repo, &request.git_ref, &job_id, &progress)
            .await?;

        // Build prompt for Claude
        let prompt = format!(
            "You are working in a git repository. Your task is to implement the following:\n\n{}\n\n{}Please make the necessary changes to fix/implement this. After making changes, verify they work.",
            request.issue,
            request.context.as_deref().map(|c| format!("Additional context: {}\n\n", c)).unwrap_or_default()
        );

        // Run Claude
        let model = self.resolve_model(request.model.as_deref());
        let output = self
            .run_claude(
                &work_dir,
                &prompt,
                &model,
                request.time_limit_secs,
                &progress,
            )
            .await?;

        // Get the diff
        let patch = self.get_git_diff(&work_dir).await?;
        let files_modified = self.get_modified_files(&work_dir).await?;

        // Compute patch hash
        let mut hasher = Sha256::new();
        hasher.update(patch.as_bytes());
        let patch_sha256 = format!("{:x}", hasher.finalize());

        // Count lines
        let mut lines_added = 0u32;
        let mut lines_removed = 0u32;
        for line in patch.lines() {
            if line.starts_with('+') && !line.starts_with("+++") {
                lines_added += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                lines_removed += 1;
            }
        }

        // Run tests if requested
        let (test_exit_code, test_output) = if request.run_tests {
            let (code, out) = self
                .run_tests(&work_dir, request.test_command.as_deref(), &progress)
                .await?;
            (Some(code), Some(out))
        } else {
            (None, None)
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Build verification
        let verification = PatchVerification {
            applies_cleanly: true, // We generated it in-place
            test_exit_code,
            test_output,
            files_changed: files_modified.len() as u32,
            lines_added,
            lines_removed,
        };

        // Build result
        let mut result = PatchGenResult::new(&patch, &patch_sha256)
            .with_trajectory(job_id.clone())
            .with_verification(verification)
            .with_duration(duration_ms)
            .with_model(&model)
            .with_summary(format!(
                "Modified {} files: +{} -{} lines",
                files_modified.len(),
                lines_added,
                lines_removed
            ));

        for file in files_modified {
            result = result.add_file(file);
        }

        // Clean up
        self.cleanup(&job_id).await;
        self.active_jobs.fetch_sub(1, Ordering::SeqCst);

        // Send completion
        if let Some(ref tx) = progress {
            let _ = tx.send(JobProgress::Completed { duration_ms }).await;
        }

        Ok(result)
    }

    async fn code_review(
        &self,
        request: CodeReviewRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> Result<CodeReviewResult> {
        use crate::domain::{ApprovalStatus, ReviewStats};

        let job_id = uuid::Uuid::new_v4().to_string();
        let start_time = std::time::Instant::now();

        self.active_jobs.fetch_add(1, Ordering::SeqCst);

        if let Some(ref tx) = progress {
            let _ = tx
                .send(JobProgress::Started {
                    job_id: job_id.clone(),
                    estimated_duration_secs: Some(request.time_limit_secs),
                })
                .await;
        }

        // Get the diff content
        let diff = match &request.input {
            crate::domain::ReviewInput::Diff(d) => d.clone(),
            crate::domain::ReviewInput::PullRequest { url } => {
                // For now, just use the URL as context
                format!("Review the changes in PR: {}", url)
            }
            _ => {
                return Err(AgentError::InvalidRequest(
                    "Unsupported review input type".to_string(),
                ));
            }
        };

        // Build review prompt
        let focus = if request.focus_areas.is_empty() {
            "all aspects".to_string()
        } else {
            request
                .focus_areas
                .iter()
                .map(|c| c.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };

        let prompt = format!(
            "Please review the following code changes. Focus on: {}.\n\n{}\n\n```diff\n{}\n```\n\nProvide your review in a structured format with:\n1. Overall assessment (APPROVE, REQUEST_CHANGES, or COMMENT)\n2. Summary of your review\n3. Specific issues found (if any) with file, line, severity, and description",
            focus,
            request.guidelines.as_deref().unwrap_or(""),
            diff
        );

        // Run Claude
        let model = self.resolve_model(request.model.as_deref());

        // For code review, we don't need a repo, just run Claude directly
        let temp_dir = self.config.work_dir.join(&job_id);
        tokio::fs::create_dir_all(&temp_dir).await?;

        // Write the diff to a file for context
        tokio::fs::write(temp_dir.join("changes.diff"), &diff).await?;

        let output = self
            .run_claude(
                &temp_dir,
                &prompt,
                &model,
                request.time_limit_secs,
                &progress,
            )
            .await?;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Parse the review output (simplified - in production would use structured output)
        let status = if output.content.to_lowercase().contains("approve")
            && !output.content.to_lowercase().contains("request_changes")
        {
            ApprovalStatus::Approve
        } else if output.content.to_lowercase().contains("request_changes")
            || output.content.to_lowercase().contains("request changes")
        {
            ApprovalStatus::RequestChanges
        } else {
            ApprovalStatus::Comment
        };

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(output.content.as_bytes());
        let review_sha256 = format!("{:x}", hasher.finalize());

        let result = CodeReviewResult {
            status,
            summary: output.content.clone(),
            issues: Vec::new(), // Would parse from structured output
            stats: ReviewStats::default(),
            review_sha256,
            input_tokens: 0,
            output_tokens: 0,
            duration_ms,
            model_used: model,
        };

        // Clean up
        self.cleanup(&job_id).await;
        self.active_jobs.fetch_sub(1, Ordering::SeqCst);

        if let Some(ref tx) = progress {
            let _ = tx.send(JobProgress::Completed { duration_ms }).await;
        }

        Ok(result)
    }

    async fn sandbox_run(
        &self,
        request: SandboxRunRequest,
        progress: Option<mpsc::Sender<JobProgress>>,
    ) -> Result<SandboxRunResult> {

        let job_id = uuid::Uuid::new_v4().to_string();
        let start_time = std::time::Instant::now();

        self.active_jobs.fetch_add(1, Ordering::SeqCst);

        if let Some(ref tx) = progress {
            let _ = tx
                .send(JobProgress::Started {
                    job_id: job_id.clone(),
                    estimated_duration_secs: Some(request.limits.max_time_secs),
                })
                .await;
        }

        // Clone repository
        let work_dir = self
            .clone_repo(&request.repo, &request.git_ref, &job_id, &progress)
            .await?;

        // Change to workdir if specified
        let exec_dir = if let Some(ref subdir) = request.workdir {
            work_dir.join(subdir)
        } else {
            work_dir.clone()
        };

        // Execute commands
        let mut command_results = Vec::new();
        let mut overall_exit_code = 0;
        let mut combined_stdout = String::new();
        let mut combined_stderr = String::new();

        for cmd in &request.commands {
            if let Some(ref tx) = progress {
                let _ = tx
                    .send(JobProgress::ToolUse {
                        tool: "bash".to_string(),
                        input_preview: cmd.chars().take(100).collect(),
                    })
                    .await;
            }

            let cmd_start = std::time::Instant::now();

            let output = Command::new("sh")
                .args(["-c", cmd])
                .current_dir(&exec_dir)
                .envs(&request.env)
                .output()
                .await
                .map_err(|e| AgentError::ExecutionError(format!("Command failed: {}", e)))?;

            let cmd_duration = cmd_start.elapsed().as_millis() as u64;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            combined_stdout.push_str(&stdout);
            combined_stderr.push_str(&stderr);

            if exit_code != 0 {
                overall_exit_code = exit_code;
            }

            command_results.push(CommandResult {
                command: cmd.clone(),
                exit_code,
                stdout,
                stderr,
                duration_ms: cmd_duration,
            });

            if let Some(ref tx) = progress {
                let _ = tx
                    .send(JobProgress::ToolResult {
                        tool: "bash".to_string(),
                        success: exit_code == 0,
                        output_preview: None,
                    })
                    .await;
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        let result = SandboxRunResult {
            exit_code: overall_exit_code,
            stdout: combined_stdout,
            stderr: combined_stderr,
            command_results,
            artifacts: Vec::new(),
            usage: ResourceUsage {
                cpu_time_ms: duration_ms,
                ..Default::default()
            },
        };

        // Clean up
        self.cleanup(&job_id).await;
        self.active_jobs.fetch_sub(1, Ordering::SeqCst);

        if let Some(ref tx) = progress {
            let _ = tx.send(JobProgress::Completed { duration_ms }).await;
        }

        Ok(result)
    }

    async fn repo_index(
        &self,
        _request: RepoIndexRequest,
        _progress: Option<mpsc::Sender<JobProgress>>,
    ) -> Result<RepoIndexResult> {
        Err(AgentError::Unavailable(
            "RepoIndex not implemented for Claude Code backend".to_string(),
        ))
    }

    async fn cancel(&self, job_id: &str) -> Result<()> {
        // Clean up the job directory
        self.cleanup(job_id).await;
        Ok(())
    }

    fn active_jobs(&self) -> u32 {
        self.active_jobs.load(Ordering::SeqCst)
    }
}

/// Output from running Claude
struct ClaudeOutput {
    content: String,
    stderr: String,
    exit_code: i32,
    usage: Option<TokenUsage>,
}

/// Check if the Claude CLI is available
fn check_claude_cli() -> bool {
    std::process::Command::new("which")
        .arg("claude")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_mode() {
        assert_eq!(IsolationMode::Local.as_str(), "local");
        assert_eq!(IsolationMode::Container.as_str(), "container");
        assert_eq!(IsolationMode::Gvisor.as_str(), "gvisor");
    }

    #[test]
    fn test_config_default() {
        let config = ClaudeCodeConfig::default();
        assert_eq!(config.isolation, IsolationMode::Container);
        assert_eq!(config.max_workers, 3);
        assert_eq!(config.model, "claude-sonnet-4");
    }

    #[test]
    fn test_backend_capabilities() {
        let config = ClaudeCodeConfig::default();
        let backend = ClaudeCodeBackend::new(config);
        let caps = backend.capabilities();

        assert!(caps.patch_gen);
        assert!(caps.code_review);
        assert!(caps.sandbox_run);
        assert!(!caps.repo_index);
        assert!(caps.supported_models.contains(&"claude-sonnet-4".to_string()));
    }

    #[test]
    fn test_resolve_model() {
        let config = ClaudeCodeConfig::default();
        let backend = ClaudeCodeBackend::new(config);

        assert_eq!(backend.resolve_model(None), "claude-sonnet-4");
        assert_eq!(
            backend.resolve_model(Some("claude-opus-4")),
            "claude-opus-4"
        );
    }

    #[test]
    fn test_extract_patch() {
        let config = ClaudeCodeConfig::default();
        let backend = ClaudeCodeBackend::new(config);

        let output = r#"
Here are the changes:

diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
+// New comment
 fn main() {
     println!("Hello");
 }

Done!
"#;

        let patch = backend.extract_patch(output);
        assert!(patch.is_some());
        assert!(patch.unwrap().contains("diff --git"));
    }
}
