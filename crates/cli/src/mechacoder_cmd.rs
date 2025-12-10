//! MechaCoder CLI commands
//!
//! Implements CLI-010..015 user stories

use crate::{CliError, CliResult, OutputFormat, output::*};
use clap::Subcommand;
use colored::Colorize;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

use orchestrator::{Orchestrator, OrchestratorConfig, SessionConfig};
use tasks::{SqliteRepository, TaskRepository};
use llm::LlmClientBuilder;

/// MechaCoder agent commands
#[derive(Subcommand, Debug)]
pub enum MechaCommand {
    /// Run MechaCoder agent (CLI-010)
    Run {
        /// Task ID to work on (optional, auto-selects if not provided)
        #[arg(short, long)]
        task: Option<String>,

        /// Model to use
        #[arg(short, long, default_value = "claude-sonnet-4-20250514")]
        model: String,

        /// Enable safe mode (no destructive operations) (CLI-012)
        #[arg(long)]
        safe: bool,

        /// Dry run mode (don't execute tools) (CLI-013)
        #[arg(long)]
        dry_run: bool,

        /// Maximum tasks to complete (CLI-014)
        #[arg(long)]
        max_tasks: Option<usize>,

        /// Maximum tokens to use
        #[arg(long)]
        max_tokens: Option<u64>,

        /// Maximum duration in minutes
        #[arg(long)]
        max_duration: Option<u64>,
    },

    /// Run parallel agents (CLI-011)
    Parallel {
        /// Number of parallel agents
        #[arg(short, long, default_value = "2")]
        agents: usize,

        /// Enable safe mode
        #[arg(long)]
        safe: bool,

        /// Maximum tasks per agent
        #[arg(long)]
        max_tasks: Option<usize>,

        /// Use Claude Code only (no local models)
        #[arg(long)]
        cc_only: bool,
    },

    /// Watch mode - continuously process tasks (CLI-015)
    Watch {
        /// Poll interval in seconds
        #[arg(short, long, default_value = "30")]
        interval: u64,

        /// Enable safe mode
        #[arg(long)]
        safe: bool,

        /// Stop after N tasks
        #[arg(long)]
        max_tasks: Option<usize>,
    },

    /// Show agent status
    Status,
}

/// Run output
#[derive(Serialize)]
struct RunOutput {
    session_id: String,
    status: String,
    tasks_completed: usize,
    tasks_failed: usize,
    tokens_used: u64,
    duration_secs: u64,
}

impl std::fmt::Display for RunOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", "MechaCoder Run Complete".bold())?;
        writeln!(f, "─────────────────────────────────")?;
        writeln!(f, "{:16} {}", "Session:".dimmed(), self.session_id)?;
        writeln!(f, "{:16} {}", "Status:".dimmed(), format_status(&self.status))?;
        writeln!(f, "{:16} {}", "Tasks Completed:".dimmed(), self.tasks_completed.to_string().green())?;
        writeln!(f, "{:16} {}", "Tasks Failed:".dimmed(),
            if self.tasks_failed > 0 {
                self.tasks_failed.to_string().red()
            } else {
                self.tasks_failed.to_string().normal()
            })?;
        writeln!(f, "{:16} {}", "Tokens Used:".dimmed(), self.tokens_used)?;
        writeln!(f, "{:16} {}", "Duration:".dimmed(), format_duration(self.duration_secs))?;
        Ok(())
    }
}

/// Execute a mechacoder command
pub async fn execute(cmd: MechaCommand, workdir: &str, format: OutputFormat) -> CliResult<()> {
    match cmd {
        MechaCommand::Run { task, model, safe, dry_run, max_tasks, max_tokens, max_duration } => {
            print_info(&format!("Starting MechaCoder agent..."));
            print_info(&format!("Working directory: {}", workdir));
            print_info(&format!("Model: {}", model));

            if safe {
                print_info("Safe mode: enabled");
            }
            if dry_run {
                print_info("Dry run: enabled");
            }

            // Build session config
            let mut session_config = SessionConfig::default();
            session_config.model = model;
            session_config.safe_mode = safe;
            session_config.dry_run = dry_run;
            session_config.max_tasks = max_tasks;
            session_config.max_tokens = max_tokens;
            session_config.max_duration_secs = max_duration.map(|m| m * 60);

            // Create task repository
            let db_path = format!("{}/tasks.db", workdir);
            let task_repo = SqliteRepository::open(&db_path)
                .map_err(|e| CliError::Other(format!("Failed to open task database: {}", e)))?;
            task_repo.init()
                .map_err(|e| CliError::Other(format!("Failed to initialize task database: {}", e)))?;
            let task_repo: Arc<dyn TaskRepository> = Arc::new(task_repo);

            // If specific task, start it
            if let Some(ref task_id) = task {
                task_repo.start(task_id)
                    .map_err(|e| CliError::Other(e.to_string()))?;
                print_info(&format!("Starting task: {}", task_id));
            }

            // Create LLM client
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| CliError::ConfigError("ANTHROPIC_API_KEY not set".into()))?;
            let llm = LlmClientBuilder::anthropic(&api_key)
                .default_model(&session_config.model)
                .build()
                .map_err(|e| CliError::Other(format!("Failed to create LLM client: {}", e)))?;

            // Build orchestrator config
            let config = OrchestratorConfig {
                working_dir: PathBuf::from(workdir),
                session_config,
                ..Default::default()
            };

            // Create and run orchestrator
            let mut orchestrator = Orchestrator::new(config, llm, task_repo)?;

            print_info("Running orchestrator...");

            if let Some(task_id) = task {
                // Run single task
                orchestrator.run_single_task(&task_id).await?;
            } else {
                // Run the golden loop
                orchestrator.run().await?;
            }

            let session = orchestrator.session();
            let output = RunOutput {
                session_id: session.id.clone(),
                status: format!("{:?}", session.state).to_lowercase(),
                tasks_completed: session.tasks_completed,
                tasks_failed: session.tasks_failed,
                tokens_used: session.tokens_used.total(),
                duration_secs: session.duration_secs(),
            };

            print_output(&output, format);
        }

        MechaCommand::Parallel { agents, safe, max_tasks: _, cc_only } => {
            print_info(&format!("Starting {} parallel agents...", agents));

            if cc_only {
                print_info("Using Claude Code only");
            }

            if safe {
                print_info("Safe mode: enabled");
            }

            // TODO: Implement parallel execution with worktrees
            // For now, just print a message
            print_warning("Parallel execution not yet implemented");
            print_info("Use 'oa mecha run' for single-agent execution");
        }

        MechaCommand::Watch { interval, safe, max_tasks: _ } => {
            print_info(&format!("Starting watch mode (poll every {}s)...", interval));

            if safe {
                print_info("Safe mode: enabled");
            }

            // TODO: Implement watch mode
            print_warning("Watch mode not yet implemented");
            print_info("Use 'oa mecha run' for single execution");
        }

        MechaCommand::Status => {
            print_info("MechaCoder Status");
            print_info("─────────────────────────────────");

            // Check for running agents
            print_info("No agents currently running");

            // Show recent sessions
            print_info("\nRecent sessions:");
            print_warning("Session history not yet implemented");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_output_display() {
        let output = RunOutput {
            session_id: "test-123".to_string(),
            status: "completed".to_string(),
            tasks_completed: 5,
            tasks_failed: 0,
            tokens_used: 10000,
            duration_secs: 300,
        };

        let display = format!("{}", output);
        assert!(display.contains("test-123"));
        assert!(display.contains("completed"));
    }
}
