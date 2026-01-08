//! Autopilot CLI commands
//!
//! These are the user-facing commands for the autopilot system.

pub mod issue;
pub mod run;
pub mod status;

use clap::{Parser, Subcommand};

/// Autopilot - AI-powered coding assistant
#[derive(Parser)]
#[command(name = "autopilot")]
#[command(about = "AI-powered coding assistant that works on your issues")]
pub struct AutopilotCli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    /// Start the autopilot loop (default if no command given)
    Run(run::RunArgs),

    /// Show current status
    Status(status::StatusArgs),

    /// Issue management
    Issue {
        #[command(subcommand)]
        command: IssueCommand,
    },
}

/// Issue subcommands
#[derive(Subcommand)]
pub enum IssueCommand {
    /// List open issues
    List(issue::ListArgs),

    /// Claim an issue to work on
    Claim(issue::ClaimArgs),

    /// Mark current issue as complete
    Complete(issue::CompleteArgs),

    /// Show issue details
    Show(issue::ShowArgs),
}

/// Execute a CLI command
pub async fn execute(cli: AutopilotCli) -> anyhow::Result<()> {
    match cli.command {
        None => {
            // Default: show status and await direction
            status::run(status::StatusArgs {}).await
        }
        Some(Commands::Run(args)) => run::run(args).await,
        Some(Commands::Status(args)) => status::run(args).await,
        Some(Commands::Issue { command }) => match command {
            IssueCommand::List(args) => issue::list(args).await,
            IssueCommand::Claim(args) => issue::claim(args).await,
            IssueCommand::Complete(args) => issue::complete(args).await,
            IssueCommand::Show(args) => issue::show(args).await,
        },
    }
}
