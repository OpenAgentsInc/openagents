//! Agent Runner Binary
//!
//! Runs a sovereign agent that:
//! - Executes tick cycles autonomously
//! - Pays for compute with its Bitcoin wallet
//! - Dies when it runs out of money
//!
//! Usage:
//!   cargo run --bin agent-runner -- --agent <name_or_npub>
//!   cargo run --bin agent-runner -- --agent myagent --single-tick
//!   cargo run --bin agent-runner -- --mnemonic "12 words..."

use agent::{AgentRegistry, LifecycleState};
use anyhow::Result;
use clap::Parser;
use compute::domain::UnifiedIdentity;
use nostr_client::RelayConnection;
use openagents::agents::{ComputeClient, Scheduler, StateManager, TickExecutor};
use openagents_spark::{Network as SparkNetwork, SparkWallet, WalletConfig};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "agent-runner")]
#[command(about = "Run a sovereign agent that pays for its own compute")]
struct Args {
    /// Agent identifier (name or npub) - loads config from registry
    #[arg(long, conflicts_with = "mnemonic")]
    agent: Option<String>,

    /// Run from mnemonic directly (bypasses registry)
    #[arg(long, conflicts_with = "agent")]
    mnemonic: Option<String>,

    /// Relay URL (overrides config)
    #[arg(long)]
    relay: Option<String>,

    /// Run a single tick and exit
    #[arg(long)]
    single_tick: bool,

    /// Network (mainnet, testnet, signet, regtest)
    #[arg(long, default_value = "regtest")]
    network: String,
}

fn parse_network(network: &str) -> Result<SparkNetwork> {
    match network.to_lowercase().as_str() {
        "mainnet" => Ok(SparkNetwork::Mainnet),
        "testnet" => Ok(SparkNetwork::Testnet),
        "signet" => Ok(SparkNetwork::Signet),
        "regtest" => Ok(SparkNetwork::Regtest),
        other => anyhow::bail!("Invalid network: {}", other),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    println!("=== OpenAgents Sovereign Agent Runner ===\n");

    // Load agent config
    let (identity, config, relay_url) = if let Some(agent_id) = &args.agent {
        // Load from registry
        let registry = AgentRegistry::new()?;
        let config = registry.load(agent_id)?;

        println!("Loaded agent: {}", config.name);
        println!("Npub: {}", config.npub);
        println!("State: {:?}", config.state);

        if config.state == LifecycleState::Dead {
            anyhow::bail!("Agent {} is dead and cannot be started", config.name);
        }

        // Decrypt mnemonic (in production, would require password)
        let mnemonic = &config.mnemonic_encrypted; // TODO: decrypt
        let identity = UnifiedIdentity::from_mnemonic(mnemonic, "")?;

        let relay_url = args.relay.unwrap_or_else(|| {
            config
                .relays
                .first()
                .cloned()
                .unwrap_or_else(|| "wss://relay.damus.io".to_string())
        });

        (identity, Some(config), relay_url)
    } else if let Some(mnemonic) = &args.mnemonic {
        // Run from mnemonic directly
        let identity = UnifiedIdentity::from_mnemonic(mnemonic, "")?;
        let relay_url = args
            .relay
            .unwrap_or_else(|| "wss://relay.damus.io".to_string());

        println!("Running from mnemonic");
        println!(
            "Pubkey: {}",
            identity.npub().unwrap_or_else(|_| "unknown".to_string())
        );

        (identity, None, relay_url)
    } else {
        anyhow::bail!("Must provide --agent <name> or --mnemonic <phrase>");
    };

    let agent_name = config
        .as_ref()
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "Agent".to_string());

    // Initialize wallet
    println!("\nInitializing wallet...");
    let network = if let Some(ref c) = config {
        match c.network {
            agent::NetworkConfig::Mainnet => SparkNetwork::Mainnet,
            agent::NetworkConfig::Testnet => SparkNetwork::Testnet,
            agent::NetworkConfig::Signet => SparkNetwork::Signet,
            agent::NetworkConfig::Regtest => SparkNetwork::Regtest,
        }
    } else {
        parse_network(&args.network)?
    };

    let wallet_config = WalletConfig {
        network,
        api_key: None,
        storage_dir: dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("openagents")
            .join("agents")
            .join(
                identity
                    .npub()
                    .unwrap_or_else(|_| "unknown".to_string()),
            ),
    };

    let wallet = SparkWallet::new(identity.spark_signer().clone(), wallet_config).await?;
    let balance = wallet.get_balance().await?;
    println!("Wallet balance: {} sats", balance.total_sats());

    if balance.total_sats() == 0 {
        let address = wallet.get_spark_address().await?;
        println!("\n!!! Agent wallet is empty !!!");
        println!("Send Bitcoin to: {}", address);
        println!("The agent cannot run without funds.");
        return Ok(());
    }

    let wallet = Arc::new(wallet);

    // Connect to relay
    println!("\nConnecting to relay: {}", relay_url);
    let relay = RelayConnection::new(&relay_url)?;
    relay.connect().await?;
    println!("Connected!");

    // Initialize components - each needs its own relay connection
    let state_relay = RelayConnection::new(&relay_url)?;
    state_relay.connect().await?;
    let state_manager = StateManager::new(
        UnifiedIdentity::from_mnemonic(identity.mnemonic(), "")?,
        state_relay,
    );

    let compute_relay = RelayConnection::new(&relay_url)?;
    compute_relay.connect().await?;
    let compute_client = ComputeClient::new(
        UnifiedIdentity::from_mnemonic(identity.mnemonic(), "")?,
        compute_relay,
        wallet.clone(),
    );

    let tick_relay = RelayConnection::new(&relay_url)?;
    tick_relay.connect().await?;
    let mut executor = TickExecutor::new(
        state_manager,
        compute_client,
        tick_relay,
        identity.public_key_hex(),
        agent_name.clone(),
    );

    // Get schedule from config
    let heartbeat_seconds = config
        .as_ref()
        .map(|c| c.schedule.heartbeat_seconds)
        .unwrap_or(900);

    let triggers = config
        .as_ref()
        .map(|c| c.schedule.triggers.clone())
        .unwrap_or_else(|| vec!["mention".to_string(), "dm".to_string(), "zap".to_string()]);

    let scheduler_relay = RelayConnection::new(&relay_url)?;
    scheduler_relay.connect().await?;
    let scheduler = Scheduler::new(
        heartbeat_seconds,
        triggers,
        identity.public_key_hex(),
        scheduler_relay,
    );

    if args.single_tick {
        // Single tick mode
        println!("\nExecuting single tick...\n");
        match scheduler.run_single_tick(&mut executor).await {
            Ok(result) => {
                println!("\n=== Tick Result ===");
                println!("Tick #: {}", result.tick_number);
                println!("State: {:?}", result.lifecycle_state);
                println!("Cost: {} sats", result.compute_cost_sats);
                println!("Runway: {:.1} days", result.runway.days_remaining);
                println!("Actions: {}", result.actions.len());
                println!("\nReasoning:\n{}", result.reasoning);
            }
            Err(e) => {
                eprintln!("Tick failed: {}", e);
            }
        }
    } else {
        // Continuous mode
        println!("\nStarting continuous operation (Ctrl+C to stop)...");
        println!("Heartbeat: {} seconds", heartbeat_seconds);
        println!();

        // Handle Ctrl+C gracefully
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let r = running.clone();
        ctrlc::set_handler(move || {
            println!("\nShutting down...");
            r.store(false, std::sync::atomic::Ordering::SeqCst);
        })?;

        // Run scheduler (will run until agent dies or interrupted)
        if let Err(e) = scheduler.run(&mut executor).await {
            eprintln!("Scheduler error: {}", e);
        }
    }

    // Cleanup
    relay.disconnect().await.ok();
    println!("\nAgent runner stopped.");

    Ok(())
}
