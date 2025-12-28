//! pylon agent - Manage sovereign agents
//!
//! Commands for spawning, listing, and managing agents in host mode.

use agent::{AgentRegistry, AgentSpawner, AutonomyLevel, NetworkConfig, SpawnRequest};
use clap::{Args, Subcommand};

use crate::db::PylonDb;
use crate::daemon::db_path;

/// Arguments for the agent command
#[derive(Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommands,
}

/// Agent subcommands
#[derive(Subcommand)]
pub enum AgentCommands {
    /// List all agents
    List(ListArgs),
    /// Show agent details
    Info(InfoArgs),
    /// Spawn a new agent
    Spawn(SpawnArgs),
    /// Delete an agent
    Delete(DeleteArgs),
}

/// Arguments for list command
#[derive(Args)]
pub struct ListArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// Arguments for info command
#[derive(Args)]
pub struct InfoArgs {
    /// Agent name or npub
    pub agent: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// Arguments for spawn command
#[derive(Args)]
pub struct SpawnArgs {
    /// Agent name
    #[arg(long, short)]
    pub name: String,

    /// Custom mnemonic (optional, will generate if not provided)
    #[arg(long)]
    pub mnemonic: Option<String>,

    /// Network (mainnet, testnet, signet, regtest)
    #[arg(long, default_value = "regtest")]
    pub network: String,

    /// Heartbeat interval in seconds
    #[arg(long, default_value = "900")]
    pub heartbeat: u64,

    /// Relay URL
    #[arg(long)]
    pub relay: Option<String>,
}

/// Arguments for delete command
#[derive(Args)]
pub struct DeleteArgs {
    /// Agent name
    pub agent: String,

    /// Force delete without confirmation
    #[arg(long, short)]
    pub force: bool,
}

/// Run the agent command
pub async fn run(args: AgentArgs) -> anyhow::Result<()> {
    match args.command {
        AgentCommands::List(list_args) => run_list(list_args).await,
        AgentCommands::Info(info_args) => run_info(info_args).await,
        AgentCommands::Spawn(spawn_args) => run_spawn(spawn_args).await,
        AgentCommands::Delete(delete_args) => run_delete(delete_args).await,
    }
}

async fn run_list(args: ListArgs) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;
    let agents = registry.list()?;

    if args.json {
        let json = serde_json::to_string_pretty(&agents)?;
        println!("{}", json);
    } else {
        if agents.is_empty() {
            println!("No agents found.");
            println!("\nSpawn a new agent with:");
            println!("  pylon agent spawn --name myagent");
        } else {
            println!("Agents:\n");
            println!("{:<20} {:<12} {:<64}", "NAME", "STATE", "NPUB");
            println!("{}", "-".repeat(96));

            for agent in agents {
                let state = format!("{:?}", agent.state).to_lowercase();
                println!("{:<20} {:<12} {}", agent.name, state, agent.npub);
            }
        }
    }

    Ok(())
}

async fn run_info(args: InfoArgs) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;
    let config = registry.load(&args.agent)?;

    // Also try to get database info
    let db_info = if let Ok(db) = PylonDb::open(db_path()?) {
        db.get_agent(&config.npub).ok().flatten()
    } else {
        None
    };

    if args.json {
        let json = serde_json::json!({
            "name": config.name,
            "npub": config.npub,
            "state": format!("{:?}", config.state).to_lowercase(),
            "network": format!("{:?}", config.network).to_lowercase(),
            "relays": config.relays,
            "schedule": {
                "heartbeat_seconds": config.schedule.heartbeat_seconds,
                "triggers": config.schedule.triggers,
            },
            "db_stats": db_info.as_ref().map(|a| serde_json::json!({
                "balance_sats": a.balance_sats,
                "tick_count": a.tick_count,
                "last_tick_at": a.last_tick_at,
            })),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("Agent: {}", config.name);
        println!("======{}", "=".repeat(config.name.len()));
        println!();
        println!("Npub:    {}", config.npub);
        println!("State:   {:?}", config.state);
        println!("Network: {:?}", config.network);
        println!();
        println!("Schedule:");
        println!("  Heartbeat: {} seconds", config.schedule.heartbeat_seconds);
        println!("  Triggers:  {}", config.schedule.triggers.join(", "));
        println!();
        println!("Relays:");
        for relay in &config.relays {
            println!("  {}", relay);
        }

        if let Some(db_agent) = db_info {
            println!();
            println!("Stats:");
            println!("  Balance:    {} sats", db_agent.balance_sats);
            println!("  Tick count: {}", db_agent.tick_count);
            if let Some(last_tick) = db_agent.last_tick_at {
                let ago = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - last_tick;
                println!("  Last tick:  {} seconds ago", ago);
            }
        }
    }

    Ok(())
}

async fn run_spawn(args: SpawnArgs) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;

    // Check if agent already exists
    if registry.load(&args.name).is_ok() {
        println!("Agent '{}' already exists.", args.name);
        return Err(anyhow::anyhow!("Agent already exists"));
    }

    // Parse network
    let network = match args.network.to_lowercase().as_str() {
        "mainnet" => NetworkConfig::Mainnet,
        "testnet" => NetworkConfig::Testnet,
        "signet" => NetworkConfig::Signet,
        "regtest" => NetworkConfig::Regtest,
        other => return Err(anyhow::anyhow!("Invalid network: {}", other)),
    };

    // Create spawn request
    let relays = args.relay
        .map(|r| vec![r])
        .unwrap_or_else(|| vec!["wss://relay.damus.io".to_string()]);

    let request = SpawnRequest {
        name: args.name.clone(),
        about: None,
        capabilities: vec!["general".to_string()],
        autonomy: AutonomyLevel::Bounded,
        heartbeat_seconds: args.heartbeat,
        triggers: vec!["mention".to_string(), "dm".to_string(), "zap".to_string()],
        network,
        relays,
    };

    // Use spawner to create agent
    let spawner = AgentSpawner::new()?;
    let result = spawner.spawn(request).await?;

    println!("Agent '{}' spawned successfully!", args.name);
    println!();
    println!("Npub: {}", result.npub);
    println!("State: spawning (awaiting funding)");
    println!();
    println!("Fund address: {}", result.spark_address);
    println!();
    println!("The agent wallet needs Bitcoin to operate.");
    println!("Send Bitcoin to the address above to activate the agent.");
    println!();
    println!("IMPORTANT: Back up the mnemonic phrase:");
    println!("  {}", result.mnemonic);

    Ok(())
}

async fn run_delete(args: DeleteArgs) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;

    // Load agent to confirm it exists
    let config = registry.load(&args.agent)?;

    if !args.force {
        println!("Are you sure you want to delete agent '{}'?", config.name);
        println!("This will permanently remove the agent configuration.");
        println!();
        println!("Type the agent name to confirm: ");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim() != config.name {
            println!("Deletion cancelled.");
            return Ok(());
        }
    }

    // Delete from registry
    registry.delete(&args.agent)?;

    // Also delete from database if exists
    if let Ok(db) = PylonDb::open(db_path()?) {
        let _ = db.delete_agent(&config.npub);
    }

    println!("Agent '{}' deleted.", config.name);

    Ok(())
}
