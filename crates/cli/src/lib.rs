//! CLI: Command-line interface for OpenAgents
//!
//! Provides commands for:
//! - Task management (list, add, start, complete, block)
//! - MechaCoder agent execution (run, parallel, safe-mode)
//! - Session management (list, resume, replay)
//!
//! # User Stories
//!
//! ## Task CLI (CLI-001..007)
//! - CLI-001: List tasks with filtering
//! - CLI-002: Add new task
//! - CLI-003: Start task (mark in-progress)
//! - CLI-004: Complete task
//! - CLI-005: Block task with reason
//! - CLI-006: Show task details
//! - CLI-007: Delete task
//!
//! ## MechaCoder CLI (CLI-010..015)
//! - CLI-010: Run single agent
//! - CLI-011: Run parallel agents
//! - CLI-012: Safe mode execution
//! - CLI-013: Dry run mode
//! - CLI-014: Set max tasks limit
//! - CLI-015: Watch mode (continuous)
//!
//! ## Session CLI (CLI-020..026)
//! - CLI-020: List sessions
//! - CLI-021: Show session details
//! - CLI-022: Resume session
//! - CLI-023: Replay session
//! - CLI-024: Delete session
//! - CLI-025: Export session
//! - CLI-026: Session statistics

mod error;
mod tasks_cmd;
mod mechacoder_cmd;
mod session_cmd;
mod output;

pub use error::*;
pub use output::*;

// Re-export command enums (but not their execute functions to avoid conflicts)
pub use tasks_cmd::TasksCommand;
pub use mechacoder_cmd::MechaCommand;
pub use session_cmd::SessionCommand;

use clap::{Parser, Subcommand};

/// OpenAgents CLI - Autonomous coding agent toolkit
#[derive(Parser, Debug)]
#[command(name = "oa")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,

    /// Working directory (defaults to current directory)
    #[arg(short, long, global = true)]
    pub workdir: Option<String>,

    /// Output format (text, json)
    #[arg(short, long, global = true, default_value = "text")]
    pub format: OutputFormat,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

/// Available commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Task management commands
    #[command(subcommand)]
    Tasks(TasksCommand),

    /// MechaCoder agent commands
    #[command(subcommand)]
    Mecha(MechaCommand),

    /// Session management commands
    #[command(subcommand)]
    Session(SessionCommand),
}

/// Output format for CLI results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(OutputFormat::Text),
            "json" => Ok(OutputFormat::Json),
            _ => Err(format!("Unknown format: {}", s)),
        }
    }
}

/// Run the CLI with the given arguments
pub async fn run(cli: Cli) -> CliResult<()> {
    let workdir = cli.workdir
        .unwrap_or_else(|| std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string()));

    match cli.command {
        Commands::Tasks(cmd) => tasks_cmd::execute(cmd, &workdir, cli.format).await,
        Commands::Mecha(cmd) => mechacoder_cmd::execute(cmd, &workdir, cli.format).await,
        Commands::Session(cmd) => session_cmd::execute(cmd, &workdir, cli.format).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_parse() {
        assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!("JSON".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert!("invalid".parse::<OutputFormat>().is_err());
    }
}
