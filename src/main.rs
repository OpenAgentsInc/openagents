//! OpenAgents - Unified CLI and Desktop Application
//!
//! Single binary that provides:
//! - CLI subcommands for all functionality
//!   - `openagents wallet ...`
//!   - `openagents marketplace ...`
//!   - `openagents gitafter ...`
//!
//! Note: Running `openagents` without args launches the Autopilot IDE.

use clap::{Parser, Subcommand};
use std::process;

mod cli;

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
    /// Autopilot commands (IDE and CLI)
    Autopilot(cli::autopilot::AutopilotArgs),

    /// Sovereign agent commands (spawn, manage, run autonomous agents)
    #[command(subcommand)]
    Agent(cli::agent::AgentCommands),

    /// Wallet commands (identity, payments)
    #[command(subcommand)]
    Wallet(cli::wallet::WalletCommands),

    /// Marketplace commands (compute, skills, data, trajectories)
    #[command(subcommand)]
    Marketplace(cli::marketplace::MarketplaceCommands),

    /// GitAfter commands (Nostr-native git)
    #[command(subcommand)]
    Gitafter(cli::gitafter::GitafterCommands),

    /// Auth commands (import credentials)
    #[command(subcommand)]
    Auth(cli::auth::AuthCommands),

    /// Pylon commands (NIP-90 compute provider)
    #[command(subcommand)]
    Pylon(cli::pylon::PylonCommands),
}

fn main() {
    let cli = Cli::parse();

    let init_logging = match &cli.command {
        None => false,
        Some(Commands::Autopilot(_)) => false,
        _ => true,
    };

    if init_logging {
        let log_level = if cli.verbose { "debug" } else { "info" };
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
            )
            .init();
    }

    // Run command
    let result = match cli.command {
        None => autopilot_app::run(),
        Some(Commands::Autopilot(cmd)) => cli::autopilot::run(cmd, cli.verbose),
        Some(Commands::Agent(cmd)) => cli::agent::run(cmd),
        Some(Commands::Wallet(cmd)) => cli::wallet::run(cmd),
        Some(Commands::Marketplace(cmd)) => cli::marketplace::run(cmd),
        Some(Commands::Gitafter(cmd)) => cli::gitafter::run(cmd),
        Some(Commands::Auth(cmd)) => cli::auth::run(cmd),
        Some(Commands::Pylon(cmd)) => cli::pylon::run(cmd),
    };

    if let Err(e) = result {
        exit_with_error(e);
    }
}

fn exit_with_error(err: anyhow::Error) -> ! {
    eprintln!("Error: {}", err);
    process::exit(1);
}
