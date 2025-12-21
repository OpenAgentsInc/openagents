//! OpenAgents - Unified CLI and Desktop Application
//!
//! Single binary that provides:
//! - GUI by default (tabbed view of all apps)
//! - CLI subcommands for all functionality:
//!   - `openagents wallet ...`
//!   - `openagents marketplace ...`
//!   - `openagents autopilot ...`
//!   - `openagents agentgit ...`
//!   - `openagents daemon ...`

use clap::{Parser, Subcommand};
use std::process;

mod cli;
mod gui;

#[derive(Parser)]
#[command(name = "openagents")]
#[command(version, about = "OpenAgents - Unified desktop application and CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Wallet commands (identity, payments)
    #[command(subcommand)]
    Wallet(cli::wallet::WalletCommands),

    /// Marketplace commands (compute, skills, data, trajectories)
    #[command(subcommand)]
    Marketplace(cli::marketplace::MarketplaceCommands),

    /// Autopilot commands (autonomous task runner)
    #[command(subcommand)]
    Autopilot(cli::autopilot::AutopilotCommands),

    /// AgentGit commands (Nostr-native git)
    #[command(subcommand)]
    Agentgit(cli::agentgit::AgentgitCommands),

    /// Daemon commands (background supervisor)
    #[command(subcommand)]
    Daemon(cli::daemon::DaemonCommands),
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();

    // Run command
    let result = match cli.command {
        None => gui::run(),
        Some(Commands::Wallet(cmd)) => cli::wallet::run(cmd),
        Some(Commands::Marketplace(cmd)) => cli::marketplace::run(cmd),
        Some(Commands::Autopilot(cmd)) => cli::autopilot::run(cmd),
        Some(Commands::Agentgit(cmd)) => cli::agentgit::run(cmd),
        Some(Commands::Daemon(cmd)) => cli::daemon::run(cmd),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
