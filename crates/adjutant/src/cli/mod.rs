//! Autopilot CLI commands
//!
//! These are the user-facing commands for the autopilot system.

pub mod blocker;
pub mod boot;
pub mod directive;
pub mod dspy;
pub mod issue;
pub mod run;
pub mod stream;
pub mod status;

use clap::{Parser, Subcommand};

/// Autopilot CLI - AI-powered coding assistant
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

    /// DSPy training and optimization
    Dspy {
        #[command(subcommand)]
        command: dspy::DspyCommand,
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
            status::run(status::StatusArgs { full_boot: false }).await
        }
        Some(Commands::Run(args)) => run::run(args).await,
        Some(Commands::Status(args)) => status::run(args).await,
        Some(Commands::Issue { command }) => match command {
            IssueCommand::List(args) => issue::list(args).await,
            IssueCommand::Claim(args) => issue::claim(args).await,
            IssueCommand::Complete(args) => issue::complete(args).await,
            IssueCommand::Show(args) => issue::show(args).await,
        },
        Some(Commands::Dspy { command }) => match command {
            dspy::DspyCommand::Status(args) => dspy::status(args).await,
            dspy::DspyCommand::Optimize(args) => dspy::optimize(args).await,
            dspy::DspyCommand::Export(args) => dspy::export(args).await,
            dspy::DspyCommand::Sessions(args) => dspy::sessions(args).await,
            dspy::DspyCommand::Performance(args) => dspy::performance(args).await,
            dspy::DspyCommand::AutoOptimize(args) => dspy::auto_optimize(args).await,
        },
    }
}
