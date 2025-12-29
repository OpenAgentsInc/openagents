//! Agent Runner Binary
//!
//! Runs a sovereign agent that:
//! - Executes tick cycles autonomously
//! - Pays for compute with its Bitcoin wallet
//! - Goes dormant when funds run out (can be revived by funding)
//!
//! Usage:
//!   cargo run --bin agent-runner -- --agent <name_or_npub>
//!   cargo run --bin agent-runner -- --agent myagent --single-tick
//!   cargo run --bin agent-runner -- --mnemonic "12 words..."

use agent::{AgentConfig, AgentRegistry, AutonomyLevel, LifecycleState};
use anyhow::Result;
use clap::Parser;
use compute::domain::UnifiedIdentity;
use nostr::nip_sa::{
    AgentProfile, AgentProfileContent, AgentSchedule, AutonomyLevel as NipSaAutonomyLevel,
    ThresholdConfig, TriggerType, KIND_AGENT_PROFILE, KIND_AGENT_SCHEDULE,
};
use nostr::{finalize_event, EventTemplate, KIND_CHANNEL_MESSAGE};
use openagents::agents::{ComputeClient, RelayHub, Scheduler, SharedRelay, StateManager, TickExecutor};
use openagents_spark::{Network as SparkNetwork, SparkWallet, WalletConfig};
use std::sync::Arc;
use std::time::Duration;

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

    /// Relay URL (repeat or comma-delimit to use multiple relays)
    #[arg(long = "relay", value_delimiter = ',')]
    relays: Vec<String>,

    /// NIP-28 channel ID for agent chat (optional)
    #[arg(long)]
    channel: Option<String>,

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

async fn publish_agent_profile_schedule(
    relay: &SharedRelay,
    identity: &UnifiedIdentity,
    config: &AgentConfig,
    triggers: &[String],
) -> Result<()> {
    let now = chrono::Utc::now().timestamp() as u64;

    let autonomy = match config.profile.autonomy {
        AutonomyLevel::Supervised => NipSaAutonomyLevel::Supervised,
        AutonomyLevel::Bounded => NipSaAutonomyLevel::Bounded,
        AutonomyLevel::Autonomous => NipSaAutonomyLevel::Autonomous,
    };

    let profile_content = AgentProfileContent::new(
        &config.profile.name,
        &config.profile.about,
        autonomy,
        &config.profile.version,
    )
    .with_capabilities(config.profile.capabilities.clone());

    let threshold = ThresholdConfig::new(1, 1, &identity.public_key_hex())?;
    let profile = AgentProfile::new(profile_content, threshold, &identity.public_key_hex());

    let profile_event = EventTemplate {
        created_at: now,
        kind: KIND_AGENT_PROFILE,
        tags: profile.build_tags(),
        content: profile
            .content
            .to_json()
            .map_err(|e| anyhow::anyhow!("Profile serialization failed: {}", e))?,
    };

    let event = finalize_event(&profile_event, identity.private_key_bytes())?;
    relay
        .publish_event(&event, Duration::from_secs(10))
        .await?;

    let mut schedule = AgentSchedule::new();
    if config.schedule.heartbeat_seconds > 0 {
        schedule = schedule.with_heartbeat(config.schedule.heartbeat_seconds)?;
    }
    for trigger in triggers {
        let trigger_type = match trigger.as_str() {
            "mention" => Some(TriggerType::Mention),
            "dm" => Some(TriggerType::Dm),
            "zap" => Some(TriggerType::Zap),
            "channel" => Some(TriggerType::Custom(KIND_CHANNEL_MESSAGE as u32)),
            _ => None,
        };

        if let Some(trigger_type) = trigger_type {
            schedule = schedule.add_trigger(trigger_type);
        }
    }

    let schedule_event = EventTemplate {
        created_at: now,
        kind: KIND_AGENT_SCHEDULE,
        tags: schedule.build_tags(),
        content: String::new(),
    };

    let event = finalize_event(&schedule_event, identity.private_key_bytes())?;
    relay
        .publish_event(&event, Duration::from_secs(10))
        .await?;

    Ok(())
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
    let (identity, config, relay_urls) = if let Some(agent_id) = &args.agent {
        // Load from registry
        let registry = AgentRegistry::new()?;
        let config = registry.load(agent_id)?;

        println!("Loaded agent: {}", config.name);
        println!("Npub: {}", config.npub);
        println!("State: {:?}", config.state);

        if config.state == LifecycleState::Dormant {
            println!("Agent {} is dormant (zero balance).", config.name);
            println!("Fund it first to revive: openagents agent fund {}", config.name);
            return Ok(());
        }

        // Decrypt mnemonic (in production, would require password)
        let mnemonic = &config.mnemonic_encrypted; // TODO: decrypt
        let identity = UnifiedIdentity::from_mnemonic(mnemonic, "")?;

        let relay_urls = if !args.relays.is_empty() {
            args.relays.clone()
        } else if config.relays.is_empty() {
            vec!["wss://relay.damus.io".to_string()]
        } else {
            config.relays.clone()
        };

        (identity, Some(config), relay_urls)
    } else if let Some(mnemonic) = &args.mnemonic {
        // Run from mnemonic directly
        let identity = UnifiedIdentity::from_mnemonic(mnemonic, "")?;
        let relay_urls = if !args.relays.is_empty() {
            args.relays.clone()
        } else {
            vec!["wss://relay.damus.io".to_string()]
        };

        println!("Running from mnemonic");
        println!(
            "Pubkey: {}",
            identity.npub().unwrap_or_else(|_| "unknown".to_string())
        );

        (identity, None, relay_urls)
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
    println!("\nConnecting to relays: {}", relay_urls.join(", "));
    let relay_hub = Arc::new(RelayHub::new(relay_urls)?);
    relay_hub.connect_all().await?;
    println!("Connected!");

    let relay: SharedRelay = relay_hub.clone();

    // Initialize components using the shared relay hub
    let state_manager = StateManager::new(
        UnifiedIdentity::from_mnemonic(identity.mnemonic(), "")?,
        relay.clone(),
    );

    let compute_client = ComputeClient::new(
        UnifiedIdentity::from_mnemonic(identity.mnemonic(), "")?,
        relay.clone(),
        wallet.clone(),
    );

    let mut executor = TickExecutor::new(
        UnifiedIdentity::from_mnemonic(identity.mnemonic(), "")?,
        state_manager,
        compute_client,
        relay.clone(),
        relay.clone(),
        identity.public_key_hex(),
        agent_name.clone(),
        args.channel.clone(),
    );

    // Get schedule from config
    let heartbeat_seconds = config
        .as_ref()
        .map(|c| c.schedule.heartbeat_seconds)
        .unwrap_or(900);

    let mut triggers = config
        .as_ref()
        .map(|c| c.schedule.triggers.clone())
        .unwrap_or_else(|| vec!["mention".to_string(), "dm".to_string(), "zap".to_string()]);
    if args.channel.is_some() && !triggers.iter().any(|t| t == "channel") {
        triggers.push("channel".to_string());
    }

    if let Some(ref config) = config {
        if let Err(e) =
            publish_agent_profile_schedule(&relay, &identity, config, &triggers).await
        {
            tracing::warn!("Failed to publish agent profile/schedule: {}", e);
        }
    }

    let scheduler = Scheduler::new(
        heartbeat_seconds,
        triggers,
        identity.public_key_hex(),
        relay.clone(),
        args.channel.clone(),
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
                if let Some(hash) = &result.trajectory_hash {
                    println!("Trajectory: {}...", &hash[..16]);
                }
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

        // Run scheduler (will run until agent goes dormant or interrupted)
        if let Err(e) = scheduler.run(&mut executor).await {
            eprintln!("Scheduler error: {}", e);
        }
    }

    // Cleanup
    relay.disconnect().await.ok();
    println!("\nAgent runner stopped.");

    Ok(())
}
