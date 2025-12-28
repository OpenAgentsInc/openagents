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
use std::env::temp_dir;
use std::time::Duration;
use tokio::sync::mpsc;

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

    // Wait briefly for subscription to establish and receive service announcement
    println!("[CUSTOMER] Waiting for provider service announcement...");

    // Flag to track if we've sent our job request
    let mut job_requested = false;
    let prompt = args.prompt.clone();

    // Event loop with timeout
    let timeout = Duration::from_secs(120);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            println!("\n[CUSTOMER] Timeout waiting for response");
            break;
        }

        let event = match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(e)) => e,
            Ok(None) => {
                println!("[CUSTOMER] Channel closed");
                break;
            }
            Err(_) => {
                // Timeout on recv - if we haven't sent job yet and no announcement received,
                // send the job request anyway (provider may have already announced)
                if !job_requested {
                    println!("[CUSTOMER] No announcement received, sending job request anyway...");
                    let request = AgentMessage::JobRequest {
                        kind: KIND_JOB_TEXT_GENERATION,
                        prompt: prompt.clone(),
                        max_tokens: 100,
                    };
                    send_channel_message(&relay, &args.channel, &keypair, &request).await?;
                    println!("[CUSTOMER] Job requested: {}", prompt);
                    job_requested = true;
                }
                continue;
            }
        };

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
            AgentMessage::ServiceAnnouncement {
                kind,
                price_msats,
                spark_address,
                network,
            } => {
                println!("[CUSTOMER] Found provider:");
                println!("           Kind: {}", kind);
                println!("           Price: {} msats", price_msats);
                println!("           Spark: {}", spark_address);
                println!("           Network: {}", network);

                // Validate network matches our expectation (regtest)
                if network != AgentNetwork::Regtest {
                    println!("[CUSTOMER] WARNING: Provider is on {} but we expect regtest!", network);
                    println!("[CUSTOMER] Skipping this provider...");
                    continue;
                }

                if !job_requested {
                    // Request a job
                    let request = AgentMessage::JobRequest {
                        kind: KIND_JOB_TEXT_GENERATION,
                        prompt: prompt.clone(),
                        max_tokens: 100,
                    };
                    send_channel_message(&relay, &args.channel, &keypair, &request).await?;
                    println!("[CUSTOMER] Job requested: {}", prompt);
                    job_requested = true;
                }
            }
            AgentMessage::Invoice {
                bolt11,
                job_id,
                amount_msats,
            } => {
                println!("[CUSTOMER] Got invoice:");
                println!("           Job ID: {}", job_id);
                println!("           Amount: {} msats", amount_msats);

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
        }
    }

    println!("[CUSTOMER] Disconnecting...");
    relay.disconnect().await.ok();

    Ok(())
}
