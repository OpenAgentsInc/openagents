//! Provider Agent Binary
//!
//! Run on Computer A to provide NIP-90 compute services.
//!
//! Usage:
//!   cargo run --bin agent-provider -- --create-channel
//!   cargo run --bin agent-provider -- --channel <CHANNEL_ID>

use clap::Parser;
use nostr::{
    derive_keypair, finalize_event, ChannelMessageEvent, ChannelMetadata, Event, EventTemplate,
    Keypair, KIND_CHANNEL_CREATION, KIND_CHANNEL_MESSAGE, KIND_JOB_TEXT_GENERATION,
};
use nostr_client::RelayConnection;
use openagents::agents::{now, parse_agent_message, AgentMessage, DEFAULT_RELAY, PROVIDER_MNEMONIC};
use openagents_spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use std::env::temp_dir;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "agent-provider")]
#[command(about = "NIP-90 Provider Agent - provides compute services via NIP-28 channels")]
struct Args {
    /// Create a new channel (prints channel ID for customer to use)
    #[arg(long)]
    create_channel: bool,

    /// Join existing channel by ID
    #[arg(long)]
    channel: Option<String>,

    /// Relay URL
    #[arg(long, default_value = DEFAULT_RELAY)]
    relay: String,

    /// Skip wallet initialization (for testing without Spark)
    #[arg(long)]
    no_wallet: bool,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Create a NIP-28 channel
async fn create_channel(relay: &RelayConnection, keypair: &Keypair) -> Result<String> {
    let metadata = ChannelMetadata::new(
        "OpenAgents Compute Marketplace",
        "Agents negotiate NIP-90 jobs with Bitcoin payments",
        "",
    )
    .with_relays(vec![DEFAULT_RELAY.to_string()]);

    let template = EventTemplate {
        created_at: now(),
        kind: KIND_CHANNEL_CREATION,
        tags: vec![],
        content: metadata.to_json()?,
    };

    let event = finalize_event(&template, &keypair.private_key)?;
    let event_id = event.id.clone();

    relay
        .publish_event(&event, Duration::from_secs(10))
        .await?;

    Ok(event_id)
}

/// Send a message to the channel
async fn send_channel_message(
    relay: &RelayConnection,
    channel_id: &str,
    keypair: &Keypair,
    msg: &AgentMessage,
) -> Result<()> {
    let msg_json = serde_json::to_string(msg)?;

    let channel_msg = ChannelMessageEvent::new(channel_id, DEFAULT_RELAY, &msg_json, now());

    let template = EventTemplate {
        created_at: now(),
        kind: KIND_CHANNEL_MESSAGE,
        tags: channel_msg.to_tags(),
        content: msg_json,
    };

    let event = finalize_event(&template, &keypair.private_key)?;
    relay
        .publish_event(&event, Duration::from_secs(10))
        .await?;

    Ok(())
}

/// Subscribe to channel messages
async fn subscribe_to_channel(
    relay: &RelayConnection,
    channel_id: &str,
) -> Result<mpsc::Receiver<Event>> {
    let filters = vec![serde_json::json!({
        "kinds": [KIND_CHANNEL_MESSAGE as u64],
        "#e": [channel_id]
    })];

    let rx = relay.subscribe_with_channel("provider-sub", &filters).await?;
    Ok(rx)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("=== OpenAgents Provider Agent ===\n");

    // Derive keypair
    let keypair = derive_keypair(PROVIDER_MNEMONIC)?;
    println!("[PROVIDER] Public key: {}", hex::encode(keypair.public_key));

    // Initialize wallet (optional)
    let wallet = if !args.no_wallet {
        println!("[PROVIDER] Connecting to Spark wallet...");
        let signer = SparkSigner::from_mnemonic(PROVIDER_MNEMONIC, "")?;
        let config = WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: temp_dir().join("agent_provider_wallet"),
        };
        let w = SparkWallet::new(signer, config).await?;
        let balance = w.get_balance().await?;
        println!("[PROVIDER] Wallet balance: {} sats", balance.total_sats());
        Some(w)
    } else {
        println!("[PROVIDER] Running without wallet (--no-wallet)");
        None
    };

    // Connect to relay
    println!("[PROVIDER] Connecting to relay: {}", args.relay);
    let relay = RelayConnection::new(&args.relay)?;
    relay.connect().await?;
    println!("[PROVIDER] Connected to relay");

    // Get or create channel
    let channel_id = if args.create_channel {
        let id = create_channel(&relay, &keypair).await?;
        println!("\n========================================");
        println!("CHANNEL CREATED: {}", id);
        println!("========================================");
        println!("\nShare this with customer:");
        println!("  cargo run --bin agent-customer -- --channel {} --prompt \"Your question here\"", id);
        println!("\nOr on another computer:");
        println!("  cargo run --bin agent-customer -- --channel {} --prompt \"...\"", id);
        println!("========================================\n");
        id
    } else if let Some(id) = args.channel {
        println!("[PROVIDER] Joining channel: {}", id);
        id
    } else {
        eprintln!("Error: Must provide --create-channel or --channel <ID>");
        std::process::exit(1);
    };

    // Announce service
    let spark_address = if let Some(ref w) = wallet {
        w.get_spark_address().await?
    } else {
        "mock-spark-address".to_string()
    };

    let announce = AgentMessage::ServiceAnnouncement {
        kind: KIND_JOB_TEXT_GENERATION,
        price_msats: 10_000,
        spark_address,
    };
    send_channel_message(&relay, &channel_id, &keypair, &announce).await?;
    println!("[PROVIDER] Service announced: kind=5050, price=10000 msats");

    // Subscribe to channel
    let mut rx = subscribe_to_channel(&relay, &channel_id).await?;
    println!("[PROVIDER] Listening for job requests...\n");

    // Event loop
    while let Some(event) = rx.recv().await {
        // Skip our own messages
        if event.pubkey == hex::encode(keypair.public_key) {
            continue;
        }

        // Parse message
        let msg = match parse_agent_message(&event.content) {
            Some(m) => m,
            None => continue,
        };

        match msg {
            AgentMessage::JobRequest { prompt, kind, max_tokens } => {
                println!("[PROVIDER] Got job request:");
                println!("           Kind: {}", kind);
                println!("           Prompt: {}", prompt);
                println!("           Max tokens: {}", max_tokens);

                if let Some(ref w) = wallet {
                    // Create invoice
                    let invoice = w
                        .create_invoice(10, Some("NIP-90 Job".to_string()), Some(3600))
                        .await?;
                    let job_id = format!("job_{}", &event.id[..16]);

                    let resp = AgentMessage::Invoice {
                        job_id: job_id.clone(),
                        bolt11: invoice.payment_request.clone(),
                        amount_msats: 10_000,
                    };
                    send_channel_message(&relay, &channel_id, &keypair, &resp).await?;
                    println!("[PROVIDER] Invoice sent for job {}", job_id);
                } else {
                    // No wallet - send mock invoice
                    let job_id = format!("job_{}", &event.id[..16]);
                    let resp = AgentMessage::Invoice {
                        job_id: job_id.clone(),
                        bolt11: "lnbcrt100n1mock".to_string(),
                        amount_msats: 10_000,
                    };
                    send_channel_message(&relay, &channel_id, &keypair, &resp).await?;
                    println!("[PROVIDER] Mock invoice sent for job {}", job_id);
                }
            }
            AgentMessage::PaymentSent { job_id, payment_id } => {
                println!("[PROVIDER] Payment received for {}: {}", job_id, payment_id);

                // Process job (mock response for now)
                let result = AgentMessage::JobResult {
                    job_id: job_id.clone(),
                    result: "The meaning of life is 42. This is a response from the compute provider.".into(),
                };
                send_channel_message(&relay, &channel_id, &keypair, &result).await?;
                println!("[PROVIDER] Result delivered for {}", job_id);
                println!("\n[PROVIDER] Job complete! Waiting for more requests...\n");
            }
            AgentMessage::ServiceAnnouncement { .. } => {
                // Ignore other providers
            }
            AgentMessage::Invoice { .. } => {
                // Ignore invoices (we send these)
            }
            AgentMessage::JobResult { .. } => {
                // Ignore results (we send these)
            }
        }
    }

    Ok(())
}
