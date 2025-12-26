//! OpenAgents - Unified CLI and Desktop Application
//!
//! Single binary that provides:
//! - CLI subcommands for all functionality
//!   - `openagents wallet ...`
//!   - `openagents marketplace ...`
//!   - `openagents autopilot ...`
//!   - `openagents gitafter ...`
//!   - `openagents daemon ...`

use clap::{Parser, Subcommand};
use std::{env, process};

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
    /// Wallet commands (identity, payments)
    #[command(subcommand)]
    Wallet(cli::wallet::WalletCommands),

    /// Marketplace commands (compute, skills, data, trajectories)
    #[command(subcommand)]
    Marketplace(cli::marketplace::MarketplaceCommands),

    /// Autopilot commands (autonomous task runner)
    #[command(subcommand)]
    Autopilot(cli::autopilot::AutopilotCommands),

    /// GitAfter commands (Nostr-native git)
    #[command(subcommand)]
    Gitafter(cli::gitafter::GitafterCommands),

    /// Daemon commands (background supervisor)
    #[command(subcommand)]
    Daemon(cli::daemon::DaemonCommands),

    /// Auth commands (import credentials)
    #[command(subcommand)]
    Auth(cli::auth::AuthCommands),
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
        None => {
            if env::var_os("OPENAGENTS_HEADLESS").is_some() {
                println!("OpenAgents GUI disabled (OPENAGENTS_HEADLESS=1)");
                Ok(())
            } else {
                if let Err(err) = autopilot_gui::run() {
                    exit_with_error(err);
                }
                Ok(())
            }
        }
        Some(Commands::Wallet(cmd)) => cli::wallet::run(cmd),
        Some(Commands::Marketplace(cmd)) => cli::marketplace::run(cmd),
        Some(Commands::Autopilot(cmd)) => cli::autopilot::run(cmd),
        Some(Commands::Gitafter(cmd)) => cli::gitafter::run(cmd),
        Some(Commands::Daemon(cmd)) => cli::daemon::run(cmd),
        Some(Commands::Auth(cmd)) => cli::auth::run(cmd),
    };

    if let Err(e) = result {
        exit_with_error(e);
    }
}

fn exit_with_error(err: anyhow::Error) -> ! {
    eprintln!("Error: {}", err);
    process::exit(1);
}
