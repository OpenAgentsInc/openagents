//! Sovereign Agent CLI subcommands
//!
//! Commands for spawning, managing, and running autonomous agents.

use agent::{
    AgentRegistry, AgentSpawner, AutonomyLevel, LifecycleState, NetworkConfig, SpawnRequest,
};
use clap::Subcommand;

#[derive(Subcommand)]
pub enum AgentCommands {
    /// Spawn a new sovereign agent with its own wallet
    Spawn {
        /// Agent display name
        #[arg(short, long)]
        name: String,

        /// Agent description
        #[arg(short, long)]
        about: Option<String>,

        /// Capabilities (comma-separated, e.g., "research,coding")
        #[arg(short, long, value_delimiter = ',')]
        capabilities: Option<Vec<String>>,

        /// Autonomy level (supervised, bounded, autonomous)
        #[arg(long, default_value = "bounded")]
        autonomy: String,

        /// Heartbeat interval in seconds
        #[arg(long, default_value = "900")]
        heartbeat: u64,

        /// Network (mainnet, testnet, signet, regtest)
        #[arg(long, default_value = "regtest")]
        network: String,

        /// Relay URLs (comma-separated)
        #[arg(long, value_delimiter = ',')]
        relays: Option<Vec<String>>,

        /// Show the mnemonic (WARNING: save it securely!)
        #[arg(long)]
        show_mnemonic: bool,
    },

    /// List all registered agents
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,

        /// Filter by state (spawning, active, low_balance, hibernating, dead)
        #[arg(long)]
        state: Option<String>,
    },

    /// Show agent status
    Status {
        /// Agent identifier (npub or name)
        agent: String,

        /// Show detailed state information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Start an agent (run tick cycles)
    Start {
        /// Agent identifier (npub or name)
        agent: String,

        /// Run a single tick and exit
        #[arg(long)]
        single_tick: bool,

        /// Relay URL to use
        #[arg(long)]
        relay: Option<String>,
    },

    /// Stop a running agent
    Stop {
        /// Agent identifier (npub or name)
        agent: String,
    },

    /// Fund an agent's wallet
    Fund {
        /// Agent identifier (npub or name)
        agent: String,

        /// Show the agent's Spark address for funding
        #[arg(long)]
        show_address: bool,
    },

    /// Delete an agent (WARNING: irreversible)
    Delete {
        /// Agent identifier (npub or name)
        agent: String,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

pub fn run(cmd: AgentCommands) -> anyhow::Result<()> {
    match cmd {
        AgentCommands::Spawn {
            name,
            about,
            capabilities,
            autonomy,
            heartbeat,
            network,
            relays,
            show_mnemonic,
        } => spawn_agent(
            name,
            about,
            capabilities,
            autonomy,
            heartbeat,
            network,
            relays,
            show_mnemonic,
        ),
        AgentCommands::List { verbose, state } => list_agents(verbose, state),
        AgentCommands::Status { agent, verbose } => show_status(&agent, verbose),
        AgentCommands::Start {
            agent,
            single_tick,
            relay,
        } => start_agent(&agent, single_tick, relay),
        AgentCommands::Stop { agent } => stop_agent(&agent),
        AgentCommands::Fund { agent, show_address } => fund_agent(&agent, show_address),
        AgentCommands::Delete { agent, yes } => delete_agent(&agent, yes),
    }
}

fn spawn_agent(
    name: String,
    about: Option<String>,
    capabilities: Option<Vec<String>>,
    autonomy: String,
    heartbeat: u64,
    network: String,
    relays: Option<Vec<String>>,
    show_mnemonic: bool,
) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let autonomy = match autonomy.to_lowercase().as_str() {
            "supervised" => AutonomyLevel::Supervised,
            "bounded" => AutonomyLevel::Bounded,
            "autonomous" => AutonomyLevel::Autonomous,
            other => anyhow::bail!("Invalid autonomy level: {}. Use: supervised, bounded, or autonomous", other),
        };

        let network = match network.to_lowercase().as_str() {
            "mainnet" => NetworkConfig::Mainnet,
            "testnet" => NetworkConfig::Testnet,
            "signet" => NetworkConfig::Signet,
            "regtest" => NetworkConfig::Regtest,
            other => anyhow::bail!("Invalid network: {}. Use: mainnet, testnet, signet, or regtest", other),
        };

        let request = SpawnRequest {
            name: name.clone(),
            about,
            capabilities: capabilities.unwrap_or_else(|| vec!["general".to_string()]),
            autonomy,
            heartbeat_seconds: heartbeat,
            triggers: vec!["mention".to_string(), "dm".to_string(), "zap".to_string()],
            network,
            relays: relays.unwrap_or_else(|| vec!["wss://relay.damus.io".to_string()]),
        };

        let spawner = AgentSpawner::new()?;
        let result = spawner.spawn(request).await?;

        println!("Agent spawned successfully!");
        println!();
        println!("  Name:          {}", result.config.name);
        println!("  Npub:          {}", result.npub);
        println!("  Spark Address: {}", result.spark_address);
        println!("  Network:       {:?}", result.config.network);
        println!();
        println!("Fund the agent by sending Bitcoin to the Spark address above.");

        if show_mnemonic {
            println!();
            println!("MNEMONIC (save this securely - shown only once!):");
            println!("{}", result.mnemonic);
        } else {
            println!();
            println!("Use --show-mnemonic to display the backup phrase (save it!)");
        }

        Ok(())
    })
}

fn list_agents(verbose: bool, state_filter: Option<String>) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;
    let agents = registry.list()?;

    if agents.is_empty() {
        println!("No agents registered.");
        println!("Use 'openagents agent spawn --name <name>' to create one.");
        return Ok(());
    }

    // Filter by state if specified
    let agents: Vec<_> = if let Some(state_str) = state_filter {
        let filter_state = match state_str.to_lowercase().as_str() {
            "spawning" => LifecycleState::Spawning,
            "active" => LifecycleState::Active,
            "low_balance" | "lowbalance" => LifecycleState::LowBalance,
            "hibernating" => LifecycleState::Hibernating,
            "dead" => LifecycleState::Dead,
            other => anyhow::bail!("Invalid state filter: {}", other),
        };
        agents.into_iter().filter(|a| a.state == filter_state).collect()
    } else {
        agents
    };

    if agents.is_empty() {
        println!("No agents match the filter.");
        return Ok(());
    }

    println!("Registered Agents ({}):", agents.len());
    println!();

    for agent in agents {
        let state_icon = match agent.state {
            LifecycleState::Spawning => "⏳",
            LifecycleState::Active => "✅",
            LifecycleState::LowBalance => "⚠️",
            LifecycleState::Hibernating => "💤",
            LifecycleState::Dead => "💀",
        };

        if verbose {
            println!("{} {} ({:?})", state_icon, agent.name, agent.state);
            println!("    Npub:     {}", agent.npub);
            println!("    Spark:    {}", agent.spark_address);
            println!("    Network:  {:?}", agent.network);
            println!("    Ticks:    {}", agent.tick_count);
            println!("    Autonomy: {:?}", agent.profile.autonomy);
            println!();
        } else {
            println!("  {} {} - {} ({:?})", state_icon, agent.name, truncate_npub(&agent.npub), agent.state);
        }
    }

    Ok(())
}

fn show_status(identifier: &str, verbose: bool) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;
    let config = registry.load(identifier)?;

    let state_icon = match config.state {
        LifecycleState::Spawning => "⏳",
        LifecycleState::Active => "✅",
        LifecycleState::LowBalance => "⚠️",
        LifecycleState::Hibernating => "💤",
        LifecycleState::Dead => "💀",
    };

    println!("Agent: {} {}", state_icon, config.name);
    println!();
    println!("  State:           {:?}", config.state);
    println!("  Npub:            {}", config.npub);
    println!("  Spark Address:   {}", config.spark_address);
    println!("  Network:         {:?}", config.network);
    println!("  Tick Count:      {}", config.tick_count);

    if config.last_active_at > 0 {
        let last_active = chrono::DateTime::from_timestamp(config.last_active_at as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        println!("  Last Active:     {}", last_active);
    }

    if verbose {
        println!();
        println!("Profile:");
        println!("  About:           {}", config.profile.about);
        println!("  Autonomy:        {:?}", config.profile.autonomy);
        println!("  Capabilities:    {}", config.profile.capabilities.join(", "));
        println!("  Version:         {}", config.profile.version);
        println!();
        println!("Schedule:");
        println!("  Heartbeat:       {} seconds", config.schedule.heartbeat_seconds);
        println!("  Triggers:        {}", config.schedule.triggers.join(", "));
        println!("  Active:          {}", config.schedule.active);
        println!();
        println!("Runway:");
        println!("  Low Balance:     {} days", config.runway.low_balance_days);
        println!("  Hibernate:       {} sats", config.runway.hibernate_threshold_sats);
        println!("  Daily Burn:      {} sats", config.runway.daily_burn_sats);
        println!("  Per-Tick Limit:  {} sats", config.runway.per_tick_limit_sats);
        println!();
        println!("Relays:");
        for relay in &config.relays {
            println!("  - {}", relay);
        }
    }

    Ok(())
}

fn start_agent(identifier: &str, single_tick: bool, relay: Option<String>) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;
    let config = registry.load(identifier)?;

    if config.state == LifecycleState::Dead {
        anyhow::bail!("Agent {} is dead and cannot be started. Create a new agent.", config.name);
    }

    println!("Starting agent: {}", config.name);
    println!("Npub: {}", config.npub);

    if single_tick {
        println!("Mode: Single tick");
    } else {
        println!("Mode: Continuous (Ctrl+C to stop)");
    }

    let relay_url = relay.unwrap_or_else(|| {
        config.relays.first()
            .cloned()
            .unwrap_or_else(|| "wss://relay.damus.io".to_string())
    });
    println!("Relay: {}", relay_url);
    println!();

    // TODO: Actually start the agent runner
    // For now, just show what would happen
    println!("Agent runner not yet implemented.");
    println!("Use 'cargo run --bin agent-runner -- --agent {}' when available.", identifier);

    Ok(())
}

fn stop_agent(identifier: &str) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;
    let config = registry.load(identifier)?;

    println!("Stopping agent: {}", config.name);

    // TODO: Implement actual stop mechanism (signal to running process)
    println!("Agent stop not yet implemented.");
    println!("For now, use Ctrl+C on the running agent process.");

    Ok(())
}

fn fund_agent(identifier: &str, show_address: bool) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;
    let config = registry.load(identifier)?;

    if show_address || true {
        // Always show the address for now
        println!("Agent: {}", config.name);
        println!();
        println!("Spark Address (send Bitcoin here):");
        println!("  {}", config.spark_address);
        println!();
        println!("Network: {:?}", config.network);

        if matches!(config.network, NetworkConfig::Mainnet) {
            println!();
            println!("WARNING: Mainnet funds are real Bitcoin!");
        }
    }

    Ok(())
}

fn delete_agent(identifier: &str, yes: bool) -> anyhow::Result<()> {
    let registry = AgentRegistry::new()?;
    let config = registry.load(identifier)?;

    if !yes {
        println!("About to delete agent: {}", config.name);
        println!("Npub: {}", config.npub);
        println!();
        println!("WARNING: This action is irreversible!");
        println!("Any funds in the agent's wallet will require the mnemonic to recover.");
        println!();
        println!("Use --yes to confirm deletion.");
        return Ok(());
    }

    registry.delete(identifier)?;
    println!("Agent {} deleted.", config.name);

    Ok(())
}

fn truncate_npub(npub: &str) -> String {
    if npub.len() > 20 {
        format!("{}...{}", &npub[..10], &npub[npub.len()-8..])
    } else {
        npub.to_string()
    }
}
