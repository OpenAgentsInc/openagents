//! pylon start - Start the provider daemon

use clap::Args;
use compute::domain::UnifiedIdentity;

use crate::config::PylonConfig;
use crate::provider::PylonProvider;

/// Arguments for the start command
#[derive(Args)]
pub struct StartArgs {
    /// Run in foreground (don't daemonize)
    #[arg(long, short)]
    pub foreground: bool,

    /// Config file path (default: ~/.config/pylon/config.toml)
    #[arg(long, short)]
    pub config: Option<String>,
}

/// Run the start command
pub async fn run(args: StartArgs) -> anyhow::Result<()> {
    // Load config
    let config = if let Some(ref path) = args.config {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content)?
    } else {
        PylonConfig::load()?
    };

    // Load identity
    let data_dir = config.data_path()?;
    let identity_file = data_dir.join("identity.mnemonic");

    if !identity_file.exists() {
        println!("No identity found. Run 'pylon init' first.");
        return Err(anyhow::anyhow!("Identity not initialized"));
    }

    // Load mnemonic from file
    let mnemonic = std::fs::read_to_string(&identity_file)?;
    let mnemonic = mnemonic.trim();

    let identity = UnifiedIdentity::from_mnemonic(mnemonic, "")
        .map_err(|e| anyhow::anyhow!("Failed to load identity: {}", e))?;

    let npub = identity
        .npub()
        .map_err(|e| anyhow::anyhow!("Failed to get npub: {}", e))?;
    println!("Loaded identity: {}", npub);

    // Create provider
    let mut provider = PylonProvider::new(config).await?;
    provider.init_with_identity(identity).await?;

    // Check backends
    let status = provider.status().await;
    if status.backends.is_empty() {
        println!("\n⚠️  No inference backends detected!");
        println!("   Install Ollama or start a llama.cpp server to begin earning.");
        return Err(anyhow::anyhow!("No backends available"));
    }

    println!("\nAvailable backends: {}", status.backends.join(", "));
    if let Some(ref default) = status.default_backend {
        println!("Default backend: {}", default);
    }

    // Start provider
    provider.start().await?;

    println!("\n🚀 Pylon provider started!");
    println!("   Listening for NIP-90 job requests...");
    println!("   Press Ctrl+C to stop.\n");

    // Subscribe to events
    let mut events = provider.events();

    if args.foreground {
        // Run in foreground, print events
        loop {
            tokio::select! {
                event = events.recv() => {
                    match event {
                        Ok(event) => {
                            let timestamp = event.timestamp().format("%H:%M:%S");
                            println!("[{}] {}", timestamp, event.description());
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("Dropped {} events", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\nShutting down...");
                    break;
                }
            }
        }

        provider.stop().await?;
        println!("Provider stopped.");
    } else {
        // TODO: Daemonize properly
        // For now, just run in foreground
        println!("Note: Background mode not yet implemented. Running in foreground.");
        loop {
            tokio::select! {
                event = events.recv() => {
                    if let Ok(event) = event {
                        let timestamp = event.timestamp().format("%H:%M:%S");
                        println!("[{}] {}", timestamp, event.description());
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\nShutting down...");
                    break;
                }
            }
        }
        provider.stop().await?;
    }

    Ok(())
}

use tokio::sync::broadcast;
