//! OANIX - OpenAgents NIX
//!
//! The agent operating system runtime.

use clap::{Parser, Subcommand};
use oanix::{
    boot_with_config, display::print_manifest, run_tick_loop, BootConfig, OanixState, TickConfig,
};
use std::time::Duration;
use tokio::signal;

#[derive(Parser)]
#[command(name = "oanix")]
#[command(about = "OANIX - OpenAgents NIX agent operating system")]
#[command(version = "0.1.0")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Skip hardware discovery
    #[arg(long)]
    skip_hardware: bool,

    /// Skip compute backend discovery
    #[arg(long)]
    skip_compute: bool,

    /// Skip network discovery
    #[arg(long)]
    skip_network: bool,

    /// Skip identity/wallet discovery
    #[arg(long)]
    skip_identity: bool,

    /// Skip workspace discovery
    #[arg(long)]
    skip_workspace: bool,

    /// Timeout for network operations in seconds
    #[arg(long, default_value = "5")]
    timeout: u64,

    /// Number of retries for transient failures
    #[arg(long, default_value = "2")]
    retries: u32,

    /// Minimal mode (skip network, identity, compute)
    #[arg(long)]
    minimal: bool,

    /// Offline mode (skip network only)
    #[arg(long)]
    offline: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run boot discovery (default)
    Boot,
    /// Start autonomous tick loop
    Run {
        /// Auto-pick issues from workspace
        #[arg(long)]
        auto_pick: bool,

        /// Accept swarm jobs (provider mode)
        #[arg(long)]
        provider: bool,

        /// Idle duration between ticks (seconds)
        #[arg(long, default_value = "5")]
        idle: u64,
    },
    /// Show current status (if session exists)
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Build boot config from args
    let boot_config = if args.minimal {
        BootConfig::minimal()
    } else if args.offline {
        BootConfig::offline()
    } else {
        BootConfig {
            skip_hardware: args.skip_hardware,
            skip_compute: args.skip_compute,
            skip_network: args.skip_network,
            skip_identity: args.skip_identity,
            skip_workspace: args.skip_workspace,
            timeout: Duration::from_secs(args.timeout),
            retries: args.retries,
        }
    };

    // Print banner
    println!("OANIX v0.1.0 - OpenAgents NIX");
    println!("═══════════════════════════════════════════════════════════════\n");

    match args.command {
        Some(Command::Run {
            auto_pick,
            provider,
            idle,
        }) => {
            println!("Discovering environment...\n");
            let manifest = boot_with_config(boot_config).await?;
            print_manifest(&manifest);

            // Create state and tick config
            let state = OanixState::load_or_create(manifest).await?;
            let tick_config = TickConfig {
                manifest_refresh_interval: 60,
                idle_duration: idle,
                auto_pick_issues: auto_pick,
                accept_swarm_jobs: provider,
            };

            // Setup shutdown signal
            let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

            // Spawn signal handler
            tokio::spawn(async move {
                signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
                println!("\nShutdown signal received, saving state...");
                let _ = shutdown_tx.send(true);
            });

            println!("\nStarting autonomous loop (Ctrl+C to stop)...\n");
            run_tick_loop(state, tick_config, shutdown_rx).await?;
            println!("OANIX shutdown complete.");
        }
        Some(Command::Status) => {
            let state_path = OanixState::default_state_path();
            if state_path.exists() {
                let content = tokio::fs::read_to_string(&state_path).await?;
                let state: oanix::PersistedState = serde_json::from_str(&content)?;
                println!("Session: {}", state.session_id);
                println!("Mode: {:?}", state.mode);
                if let Some(task) = state.active_task {
                    println!("Active task: {} ({}%)", task.description, task.progress);
                } else {
                    println!("No active task");
                }
            } else {
                println!("No session found. Run 'oanix run' to start.");
            }
        }
        Some(Command::Boot) | None => {
            println!("Discovering environment...\n");
            let manifest = boot_with_config(boot_config).await?;
            print_manifest(&manifest);
            println!("\nReady. Use 'oanix run' to start autonomous loop.");
        }
    }

    Ok(())
}
