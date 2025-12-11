//! HillClimber Runner Module
//!
//! High-level runner that integrates all components:
//! - TestGen for comprehensive test generation
//! - MAP Orchestrator for task execution
//! - Store for persistence
//! - Scoring for result evaluation
//!
//! This module provides the main entry point for running HillClimber.

use crate::error::{HillClimberError, Result};
use crate::evaluator::parse_pytest_output;
use crate::orchestrator::{FMClient, MAPOrchestrator, NoopEmitter, ToolExecutor, WorkspaceExecutor};
use crate::scoring::score_result;
use crate::store::HillClimberStore;
use crate::types::{
    ActionResult, EvaluatorResult, HillClimberRun, HillClimberRunInput, MAPOrchestratorOptions,
    TerminalBenchTask, VerificationConfig, generate_run_id,
};
use fm_bridge::FMClient as FMBridgeClient;
use llm::{LlmClient, ChatOptions};
use sandbox::{ContainerBackend, ContainerConfig, detect_backend};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// FM Client Adapter
// ============================================================================

/// Adapter to use fm-bridge's FMClient with our FMClient trait.
pub struct FMBridgeAdapter {
    client: FMBridgeClient,
}

impl FMBridgeAdapter {
    /// Create a new FM bridge adapter.
    pub fn new() -> Self {
        Self {
            client: FMBridgeClient::new(),
        }
    }
}

impl Default for FMBridgeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl FMClient for FMBridgeAdapter {
    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        // Combine system and user prompts
        let full_prompt = format!("{}\n\n{}", system, user);
        let response = self.client
            .complete(full_prompt, None)
            .await
            .map_err(HillClimberError::FmBridge)?;

        // Extract content from first choice
        let content = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(content)
    }
}

// ============================================================================
// LLM Client Adapter
// ============================================================================

/// Model provider selection.
#[derive(Debug, Clone, Default)]
pub enum ModelProvider {
    /// Local Apple Foundation Model (default)
    #[default]
    FM,
    /// Anthropic Claude (claude-sonnet-4, claude-opus-4, etc.)
    Anthropic(String),
    /// OpenAI GPT (gpt-4o, gpt-4o-mini, o1, etc.)
    OpenAI(String),
}

impl ModelProvider {
    /// Parse from CLI string (e.g., "fm", "claude-sonnet-4", "gpt-4o")
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "fm" | "foundation" | "local" => ModelProvider::FM,
            s if s.starts_with("claude") || s.starts_with("anthropic") => {
                ModelProvider::Anthropic(s.to_string())
            }
            s if s.starts_with("gpt") || s.starts_with("o1") || s.starts_with("openai") => {
                ModelProvider::OpenAI(s.to_string())
            }
            // Default to treating unknown as Anthropic model
            _ => ModelProvider::Anthropic(s.to_string()),
        }
    }
}

/// Adapter to use LlmClient with our FMClient trait.
pub struct LlmClientAdapter {
    client: Arc<LlmClient>,
    model: Option<String>,
}

impl LlmClientAdapter {
    /// Create a new LLM client adapter for Anthropic.
    pub fn anthropic(model: Option<String>) -> Result<Self> {
        let client = LlmClient::from_env_for_provider("anthropic")
            .map_err(|e| HillClimberError::Configuration(format!("Failed to create Anthropic client: {}", e)))?;
        Ok(Self {
            client: Arc::new(client),
            model,
        })
    }

    /// Create a new LLM client adapter for OpenAI.
    pub fn openai(model: Option<String>) -> Result<Self> {
        let client = LlmClient::from_env_for_provider("openai")
            .map_err(|e| HillClimberError::Configuration(format!("Failed to create OpenAI client: {}", e)))?;
        Ok(Self {
            client: Arc::new(client),
            model,
        })
    }

    /// Create from any available provider (tries Anthropic first, then OpenAI).
    pub fn from_env(model: Option<String>) -> Result<Self> {
        let client = LlmClient::from_env()
            .map_err(|e| HillClimberError::Configuration(format!("Failed to create LLM client: {}", e)))?;
        Ok(Self {
            client: Arc::new(client),
            model,
        })
    }

    /// Create from a specific model provider.
    pub fn from_provider(provider: &ModelProvider) -> Result<Box<dyn FMClient>> {
        match provider {
            ModelProvider::FM => Ok(Box::new(FMBridgeAdapter::new())),
            ModelProvider::Anthropic(model) => {
                let adapter = Self::anthropic(Some(model.clone()))?;
                Ok(Box::new(adapter))
            }
            ModelProvider::OpenAI(model) => {
                let adapter = Self::openai(Some(model.clone()))?;
                Ok(Box::new(adapter))
            }
        }
    }
}

#[async_trait::async_trait]
impl FMClient for LlmClientAdapter {
    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        let options = ChatOptions::default()
            .system(system)
            .max_tokens(8192);

        // Set model if specified
        let options = if let Some(ref model) = self.model {
            ChatOptions {
                model: Some(model.clone()),
                ..options
            }
        } else {
            options
        };

        let messages = vec![llm::Message::user(user)];
        let response = self.client
            .chat(&messages, Some(options))
            .await
            .map_err(|e| HillClimberError::Configuration(format!("LLM error: {}", e)))?;

        Ok(response.text())
    }
}

// ============================================================================
// Sandbox Tool Executor
// ============================================================================

/// Tool executor that runs commands inside a container for isolation.
pub struct SandboxToolExecutor {
    backend: Arc<dyn ContainerBackend>,
    config: ContainerConfig,
    verification: VerificationConfig,
}

impl SandboxToolExecutor {
    /// Create a new sandbox executor with auto-detected backend.
    pub async fn new(
        workspace: PathBuf,
        verification: VerificationConfig,
        image: &str,
    ) -> Result<Self> {
        let backend = detect_backend().await;

        // Check if backend is available
        if !backend.is_available().await {
            return Err(HillClimberError::Configuration(
                "No container runtime available (Docker or macOS Container required)".to_string()
            ));
        }

        let config = ContainerConfig::new(image, workspace)
            .workdir("/workspace")
            .memory_limit("4G")
            .timeout_ms(300000); // 5 minute timeout

        Ok(Self {
            backend,
            config,
            verification,
        })
    }

    /// Create with a specific backend and config.
    pub fn with_backend(
        backend: Arc<dyn ContainerBackend>,
        config: ContainerConfig,
        verification: VerificationConfig,
    ) -> Self {
        Self {
            backend,
            config,
            verification,
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for SandboxToolExecutor {
    async fn read_file(&self, path: &str) -> Result<ActionResult> {
        let command = vec![
            "cat".to_string(),
            format!("/workspace/{}", path.trim_start_matches('/')),
        ];

        let result = self.backend.run(&command, &self.config)
            .await
            .map_err(|e| HillClimberError::Configuration(format!("Container error: {}", e)))?;

        Ok(ActionResult {
            success: result.success(),
            output: if result.success() { result.stdout } else { result.stderr },
            modified_file: None,
        })
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<ActionResult> {
        // Use printf for safe content writing
        let escaped_content = content
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`");

        let file_path = format!("/workspace/{}", path.trim_start_matches('/'));
        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            format!("mkdir -p \"$(dirname '{}')\" && printf '%s' \"{}\" > '{}'",
                    file_path, escaped_content, file_path),
        ];

        let result = self.backend.run(&command, &self.config)
            .await
            .map_err(|e| HillClimberError::Configuration(format!("Container error: {}", e)))?;

        let success = result.success();
        let output = if success {
            format!("Wrote {} bytes to {}", content.len(), path)
        } else {
            result.stderr.clone()
        };
        let modified_file = if success { Some(path.to_string()) } else { None };

        Ok(ActionResult {
            success,
            output,
            modified_file,
        })
    }

    async fn run_command(&self, command: &str) -> Result<ActionResult> {
        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            command.to_string(),
        ];

        let result = self.backend.run(&command, &self.config)
            .await
            .map_err(|e| HillClimberError::Configuration(format!("Container error: {}", e)))?;

        Ok(ActionResult {
            success: result.success(),
            output: result.combined_output().trim().to_string(),
            modified_file: None,
        })
    }

    async fn verify_progress(&self, verification: &VerificationConfig) -> Result<EvaluatorResult> {
        let cmd = verification
            .command
            .clone()
            .unwrap_or_else(|| "pytest -v".to_string());

        let command = vec![
            "sh".to_string(),
            "-c".to_string(),
            cmd,
        ];

        let start = std::time::Instant::now();
        let result = self.backend.run(&command, &self.config)
            .await
            .map_err(|e| HillClimberError::Configuration(format!("Container error: {}", e)))?;

        let combined = result.combined_output();
        let parse_result = parse_pytest_output(&combined);
        let progress = if parse_result.total > 0 {
            parse_result.passed as f64 / parse_result.total as f64
        } else {
            0.0
        };

        Ok(EvaluatorResult {
            passed: result.success() && parse_result.failed == 0,
            progress,
            tests_total: parse_result.total,
            tests_passing: parse_result.passed,
            failures: parse_result.failures,
            suggestion: None,
            raw_output: combined,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

// ============================================================================
// Run Options
// ============================================================================

/// Options for a single HillClimber run.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Task to run
    pub task: TerminalBenchTask,
    /// Config to use (if None, uses current or default)
    pub config_id: Option<i64>,
    /// Workspace directory
    pub workspace: PathBuf,
    /// Maximum turns
    pub max_turns: u32,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Whether to generate tests first
    pub generate_tests: bool,
    /// Whether to use parallel sampling
    pub use_sampling: bool,
    /// Verbose output
    pub verbose: bool,
    /// Model provider to use (fm, claude-*, gpt-*, etc.)
    pub model: ModelProvider,
    /// Whether to run in a sandboxed container
    pub sandbox: bool,
    /// Container image to use for sandbox (e.g., "python:3.11")
    pub sandbox_image: String,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            task: TerminalBenchTask {
                id: "unknown".to_string(),
                description: "".to_string(),
                source_path: None,
                verification: VerificationConfig::default(),
            },
            config_id: None,
            workspace: PathBuf::from("."),
            max_turns: 30,
            timeout_secs: 600,
            generate_tests: true,
            use_sampling: true,
            verbose: false,
            model: ModelProvider::FM,
            sandbox: false,
            sandbox_image: "python:3.11".to_string(),
        }
    }
}

// ============================================================================
// Runner
// ============================================================================

/// HillClimber Runner - orchestrates a single run.
pub struct HillClimberRunner {
    store: std::sync::Arc<HillClimberStore>,
}

impl HillClimberRunner {
    /// Create a new runner.
    pub fn new(store: std::sync::Arc<HillClimberStore>) -> Self {
        Self { store }
    }

    /// Create a new runner with a store (takes ownership).
    pub fn with_store(store: HillClimberStore) -> Self {
        Self {
            store: std::sync::Arc::new(store),
        }
    }

    /// Run a single task.
    pub async fn run(&self, options: RunOptions) -> Result<HillClimberRun> {
        let run_id = generate_run_id();

        // Ensure we have a config for this task
        let config = self.store.ensure_default_config(&options.task.id)?;

        // Set up orchestrator options
        let orchestrator_options = MAPOrchestratorOptions {
            workspace: options.workspace.clone(),
            timeout_secs: options.timeout_secs,
            max_turns: options.max_turns,
            task_description: options.task.description.clone(),
            verbose: options.verbose,
            use_sampling: options.use_sampling,
            generate_tests: options.generate_tests,
        };

        let emitter = NoopEmitter;

        // Run the task with the selected model and executor
        let start_time = std::time::Instant::now();

        // Create the appropriate FM client based on model selection
        let fm_client: Box<dyn FMClient> = match &options.model {
            ModelProvider::FM => Box::new(FMBridgeAdapter::new()),
            ModelProvider::Anthropic(model) => {
                Box::new(LlmClientAdapter::anthropic(Some(model.clone()))?)
            }
            ModelProvider::OpenAI(model) => {
                Box::new(LlmClientAdapter::openai(Some(model.clone()))?)
            }
        };

        // Run with appropriate tool executor based on sandbox mode
        let result = if options.sandbox {
            let sandbox_executor = SandboxToolExecutor::new(
                options.workspace.clone(),
                options.task.verification.clone(),
                &options.sandbox_image,
            ).await?;

            let orchestrator = MAPOrchestrator::new(
                fm_client,
                sandbox_executor,
                emitter,
                orchestrator_options,
            );
            orchestrator.run(&options.task).await?
        } else {
            let workspace_executor = WorkspaceExecutor::new(
                options.workspace.clone(),
                options.task.verification.clone(),
            );

            let orchestrator = MAPOrchestrator::new(
                fm_client,
                workspace_executor,
                emitter,
                orchestrator_options,
            );
            orchestrator.run(&options.task).await?
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Calculate score
        let score = score_result(result.passed, result.turns);

        // Create run record
        let run_input = HillClimberRunInput {
            run_id: run_id.clone(),
            task_id: options.task.id.clone(),
            config_id: config.id,
            passed: result.passed,
            turns: result.turns,
            duration_ms,
            step_summary: Some(
                result
                    .subtask_status
                    .iter()
                    .map(|s| format!("{}: {:?}", s.name, s.status))
                    .collect(),
            ),
            error_message: result.error,
            meta_model: None,
            proposed_change: None,
            change_accepted: false,
            score,
        };

        // Save run (this also updates best internally)
        let run = self.store.save_run(&run_input)?;

        Ok(run)
    }

    /// Run multiple tasks in round-robin.
    pub async fn run_loop(
        &self,
        tasks: Vec<TerminalBenchTask>,
        max_runs: u32,
        sleep_ms: u64,
        verbose: bool,
        max_turns: u32,
        model: ModelProvider,
        sandbox: bool,
        sandbox_image: String,
    ) -> Result<Vec<HillClimberRun>> {
        let mut runs = Vec::new();
        let mut task_index = 0;

        for run_num in 0..max_runs {
            let task = &tasks[task_index % tasks.len()];

            if verbose {
                let mode = if sandbox { "sandboxed" } else { "local" };
                println!("Run {}/{}: Task {} (model: {:?}, {})", run_num + 1, max_runs, task.id, model, mode);
            }

            // Use task's source_path as workspace if available
            let workspace = task.source_path.clone().unwrap_or_else(|| PathBuf::from("."));

            let options = RunOptions {
                task: task.clone(),
                workspace,
                verbose,
                max_turns,
                model: model.clone(),
                sandbox,
                sandbox_image: sandbox_image.clone(),
                ..Default::default()
            };

            match self.run(options).await {
                Ok(run) => {
                    if verbose {
                        println!(
                            "  Result: {} (score: {}, turns: {})",
                            if run.passed { "PASS" } else { "FAIL" },
                            run.score,
                            run.turns
                        );
                    }
                    runs.push(run);
                }
                Err(e) => {
                    if verbose {
                        println!("  Error: {}", e);
                    }
                }
            }

            task_index += 1;

            // Sleep between runs
            if sleep_ms > 0 && run_num < max_runs - 1 {
                sleep(Duration::from_millis(sleep_ms)).await;
            }
        }

        Ok(runs)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a task from a Terminal-Bench task file.
pub fn load_task(path: &PathBuf) -> Result<TerminalBenchTask> {
    let content = std::fs::read_to_string(path)
        .map_err(HillClimberError::Io)?;

    serde_json::from_str(&content)
        .map_err(HillClimberError::Serialization)
}

/// Create a task from ID and description.
pub fn create_task(id: &str, description: &str) -> TerminalBenchTask {
    TerminalBenchTask {
        id: id.to_string(),
        description: description.to_string(),
        source_path: None,
        verification: VerificationConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_task() {
        let task = create_task("test-task", "Write a hello world program");
        assert_eq!(task.id, "test-task");
        assert!(task.description.contains("hello world"));
    }

    #[test]
    fn test_run_options_default() {
        let options = RunOptions::default();
        assert_eq!(options.max_turns, 30);
        assert_eq!(options.timeout_secs, 600);
        assert!(options.generate_tests);
    }
}
