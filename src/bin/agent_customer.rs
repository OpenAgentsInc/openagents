//! Customer Agent Binary
//!
//! Run on Computer B to request NIP-90 compute services.
//!
//! Usage:
//!   cargo run --bin agent-customer -- --channel <CHANNEL_ID> --prompt "Your question"

use clap::Parser;
use nostr::{
    derive_keypair, finalize_event, ChannelMessageEvent, Event, EventTemplate, Keypair,
    KIND_CHANNEL_MESSAGE, KIND_JOB_TEXT_GENERATION,
};
use nostr_client::RelayConnection;
use openagents::agents::{now, parse_agent_message, AgentMessage, Network as AgentNetwork, CUSTOMER_MNEMONIC, DEFAULT_RELAY};
use openagents_spark::{Network as SparkNetwork, SparkSigner, SparkWallet, WalletConfig};
use std::collections::HashMap;
use std::env::temp_dir;
use std::time::Duration;
use tokio::sync::mpsc;

/// Provider info collected from ServiceAnnouncement
#[derive(Debug, Clone)]
struct ProviderInfo {
    pubkey: String,
    kind: u16,
    price_msats: u64,
    spark_address: String,
    network: AgentNetwork,
    models: Vec<String>,
    capabilities: Vec<String>,
}

#[derive(Parser)]
#[command(name = "agent-customer")]
#[command(about = "NIP-90 Customer Agent - requests compute services via NIP-28 channels")]
struct Args {
    /// Channel ID to join (get from provider)
    #[arg(long)]
    channel: String,

    /// Job prompt - the question or task to send to the provider
    #[arg(long)]
    prompt: String,

    /// Relay URL
    #[arg(long, default_value = DEFAULT_RELAY)]
    relay: String,

    /// Skip wallet initialization (for testing without Spark)
    #[arg(long)]
    no_wallet: bool,

    /// Time to wait for provider discovery (seconds)
    #[arg(long, default_value = "3")]
    discovery_time: u64,

    /// Select provider by: cheapest, first, or specific pubkey
    #[arg(long, default_value = "first")]
    select: String,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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

    let rx = relay
        .subscribe_with_channel("customer-sub", &filters)
        .await?;
    Ok(rx)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("=== OpenAgents Customer Agent ===\n");

    // Derive keypair
    let keypair = derive_keypair(CUSTOMER_MNEMONIC)?;
    println!("[CUSTOMER] Public key: {}", hex::encode(keypair.public_key));

    // Initialize wallet (optional)
    let wallet = if !args.no_wallet {
        println!("[CUSTOMER] Connecting to Spark wallet...");
        let signer = SparkSigner::from_mnemonic(CUSTOMER_MNEMONIC, "")?;
        let config = WalletConfig {
            network: SparkNetwork::Regtest,
            api_key: None,
            storage_dir: temp_dir().join("agent_customer_wallet"),
        };
        let w = SparkWallet::new(signer, config).await?;
        let balance = w.get_balance().await?;
        println!("[CUSTOMER] Wallet balance: {} sats", balance.total_sats());

        if balance.total_sats() < 100 {
            let address = w.get_bitcoin_address().await?;
            println!("\n!!! Customer wallet needs funds !!!");
            println!("Send regtest sats to: {}", address);
            println!("Faucet: https://app.lightspark.com/regtest-faucet");
            println!("\nRe-run after funding.");
            return Ok(());
        }

        Some(w)
    } else {
        println!("[CUSTOMER] Running without wallet (--no-wallet)");
        None
    };

    // Connect to relay
    println!("[CUSTOMER] Connecting to relay: {}", args.relay);
    let relay = RelayConnection::new(&args.relay)?;
    relay.connect().await?;
    println!("[CUSTOMER] Connected to relay");

    // Subscribe to channel
    println!("[CUSTOMER] Joining channel: {}", args.channel);
    let mut rx = subscribe_to_channel(&relay, &args.channel).await?;

    // Record start time to filter old messages
    let start_time = now();

    // ============================================
    // PHASE 1: Provider Discovery
    // ============================================
    println!("[CUSTOMER] Discovering providers for {} seconds...", args.discovery_time);
    let mut providers: HashMap<String, ProviderInfo> = HashMap::new();
    let discovery_deadline = std::time::Instant::now() + Duration::from_secs(args.discovery_time);

    while std::time::Instant::now() < discovery_deadline {
        let remaining = discovery_deadline.saturating_duration_since(std::time::Instant::now());
        let event = match tokio::time::timeout(remaining.max(Duration::from_millis(100)), rx.recv()).await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => break, // Discovery timeout reached
        };

        // Skip old messages
        if event.created_at < start_time {
            continue;
        }

        // Skip our own messages
        if event.pubkey == hex::encode(keypair.public_key) {
            continue;
        }

        if let Some(AgentMessage::ServiceAnnouncement {
            kind,
            price_msats,
            spark_address,
            network,
            provider_pubkey,
            models,
            capabilities,
        }) = parse_agent_message(&event.content) {
            // Use provider_pubkey or event.pubkey as key
            let pubkey = provider_pubkey.clone().unwrap_or_else(|| event.pubkey.clone());

            // Skip wrong network
            if network != AgentNetwork::Regtest {
                println!("[CUSTOMER] Skipping provider on {} (we need regtest)", network);
                continue;
            }

            providers.insert(pubkey.clone(), ProviderInfo {
                pubkey,
                kind,
                price_msats,
                spark_address,
                network,
                models,
                capabilities,
            });
        }
    }

    // ============================================
    // PHASE 2: Provider Selection
    // ============================================
    if providers.is_empty() {
        println!("[CUSTOMER] No providers found! Make sure a provider is running.");
        println!("[CUSTOMER] Disconnecting...");
        relay.disconnect().await.ok();
        return Ok(());
    }

    println!("\n[CUSTOMER] Found {} provider(s):", providers.len());
    for (i, (pubkey, info)) in providers.iter().enumerate() {
        println!("  [{}] {}...", i, &pubkey[..16.min(pubkey.len())]);
        println!("      Price: {} msats", info.price_msats);
        println!("      Models: {:?}", info.models);
        println!("      Capabilities: {:?}", info.capabilities);
    }

    // Select provider based on --select flag
    let selected = match args.select.as_str() {
        "cheapest" => {
            providers.values().min_by_key(|p| p.price_msats).cloned()
        }
        "first" => {
            providers.values().next().cloned()
        }
        pubkey => {
            // Try to find by pubkey prefix
            providers.values()
                .find(|p| p.pubkey.starts_with(pubkey))
                .cloned()
                .or_else(|| providers.values().next().cloned())
        }
    };

    let selected = match selected {
        Some(p) => p,
        None => {
            println!("[CUSTOMER] Failed to select provider");
            return Ok(());
        }
    };

    println!("\n[CUSTOMER] Selected provider: {}...", &selected.pubkey[..16.min(selected.pubkey.len())]);
    println!("           Price: {} msats", selected.price_msats);

    // ============================================
    // PHASE 3: Job Request
    // ============================================
    let prompt = args.prompt.clone();
    let request = AgentMessage::JobRequest {
        kind: KIND_JOB_TEXT_GENERATION,
        prompt: prompt.clone(),
        max_tokens: 100,
        target_provider: Some(selected.pubkey.clone()),
    };
    send_channel_message(&relay, &args.channel, &keypair, &request).await?;
    println!("[CUSTOMER] Job requested: {}", prompt);

    // ============================================
    // PHASE 4: Wait for Invoice and Result
    // ============================================
    let mut our_job_id: Option<String> = None;
    let timeout = Duration::from_secs(120);
    let job_start = std::time::Instant::now();

    loop {
        if job_start.elapsed() > timeout {
            println!("\n[CUSTOMER] Timeout waiting for response");
            break;
        }

        let event = match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
            Ok(Some(e)) => e,
            Ok(None) => {
                println!("[CUSTOMER] Channel closed");
                break;
            }
            Err(_) => continue,
        };

        // Skip old messages
        if event.created_at < start_time {
            continue;
        }

        // Skip our own messages
        if event.pubkey == hex::encode(keypair.public_key) {
            continue;
        }

        let msg = match parse_agent_message(&event.content) {
            Some(m) => m,
            None => continue,
        };

        match msg {
            AgentMessage::ServiceAnnouncement { .. } => {
                // Already handled in discovery phase
            }
            AgentMessage::Invoice {
                bolt11,
                job_id,
                amount_msats,
                payment_hash,
            } => {
                // Skip invoices for other customers' jobs
                if our_job_id.is_some() && our_job_id.as_ref() != Some(&job_id) {
                    continue;
                }

                println!("[CUSTOMER] Got invoice:");
                println!("           Job ID: {}", job_id);
                println!("           Amount: {} msats", amount_msats);
                if let Some(ref hash) = payment_hash {
                    println!("           Payment Hash: {}...", &hash[..16.min(hash.len())]);
                }

                // Track this as our job
                our_job_id = Some(job_id.clone());

                if let Some(ref w) = wallet {
                    println!("[CUSTOMER] Paying invoice...");
                    let payment = w.send_payment_simple(&bolt11, None).await?;
                    let payment_id = payment.payment.id.clone();
                    println!("[CUSTOMER] Payment sent: {}", payment_id);

                    // Confirm payment
                    let confirm = AgentMessage::PaymentSent {
                        job_id,
                        payment_id,
                    };
                    send_channel_message(&relay, &args.channel, &keypair, &confirm).await?;
                    println!("[CUSTOMER] Payment confirmation sent");
                } else {
                    // No wallet - send mock payment
                    let payment_id = "mock-payment-id".to_string();
                    println!("[CUSTOMER] Mock payment (no wallet): {}", payment_id);

                    let confirm = AgentMessage::PaymentSent {
                        job_id,
                        payment_id,
                    };
                    send_channel_message(&relay, &args.channel, &keypair, &confirm).await?;
                }
            }
            AgentMessage::JobResult { job_id, result } => {
                // Skip results for other customers' jobs
                if our_job_id.as_ref() != Some(&job_id) {
                    continue;
                }

                println!("\n========================================");
                println!("JOB RESULT RECEIVED");
                println!("========================================");
                println!("Job ID: {}", job_id);
                println!("Result: {}", result);
                println!("========================================\n");

                println!("[CUSTOMER] Job complete!");
                break;
            }
            AgentMessage::JobRequest { .. } => {
                // Ignore other customers' requests
            }
            AgentMessage::PaymentSent { .. } => {
                // Ignore (we send these)
            }
            AgentMessage::StreamChunk { job_id, chunk, is_final } => {
                // Only process chunks for our job
                if our_job_id.as_ref() == Some(&job_id) {
                    print!("{}", chunk);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                    if is_final {
                        println!();  // Newline after streaming
                    }
                }
            }
        }
    }

    println!("[CUSTOMER] Disconnecting...");
    relay.disconnect().await.ok();

    Ok(())
}
