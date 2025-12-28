//! Provider Agent Binary
//!
//! Run on Computer A to provide NIP-90 compute services.
//!
//! Usage:
//!   cargo run --bin agent-provider -- --create-channel
//!   cargo run --bin agent-provider -- --channel <CHANNEL_ID>

use clap::Parser;
use compute::backends::{BackendRegistry, CompletionRequest};
use nostr::{
    derive_keypair, finalize_event, ChannelMessageEvent, ChannelMetadata, Event, EventTemplate,
    Keypair, KIND_CHANNEL_CREATION, KIND_CHANNEL_MESSAGE, KIND_JOB_TEXT_GENERATION,
};
use nostr_client::RelayConnection;
use openagents::agents::{now, parse_agent_message, AgentMessage, Network as AgentNetwork, DEFAULT_RELAY, PROVIDER_MNEMONIC};
use openagents_spark::{Network as SparkNetwork, SparkSigner, SparkWallet, WalletConfig};
use std::collections::{HashMap, HashSet};
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

    /// Model to use for inference (default: auto-detect first available)
    #[arg(long)]
    model: Option<String>,

    /// Enable streaming responses (send tokens as they're generated)
    #[arg(long)]
    stream: bool,
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

    // Detect inference backends
    println!("[PROVIDER] Detecting inference backends...");
    let registry = BackendRegistry::detect().await;

    let available_models: Vec<String>;
    if registry.has_backends() {
        let backends = registry.available_backends();
        println!("[PROVIDER] Found backends: {:?}", backends);

        // List available models
        let models = registry.list_all_models().await;
        available_models = models.iter().map(|(_, m)| m.id.clone()).collect();
        if !available_models.is_empty() {
            println!("[PROVIDER] Available models: {:?}", available_models);
        }

        if let Some(default_id) = registry.default_id() {
            println!("[PROVIDER] Default backend: {}", default_id);
        }
    } else {
        println!("[PROVIDER] WARNING: No inference backends available!");
        println!("[PROVIDER] Install Ollama: https://ollama.ai");
        println!("[PROVIDER] Or run: ollama serve");
        available_models = vec![];
    }

    // Determine which model to use
    let model_to_use = args.model.clone().unwrap_or_else(|| {
        available_models.first().cloned().unwrap_or_else(|| "llama3.2".to_string())
    });
    println!("[PROVIDER] Will use model: {}", model_to_use);

    // Initialize wallet (optional)
    let wallet = if !args.no_wallet {
        println!("[PROVIDER] Connecting to Spark wallet...");
        let signer = SparkSigner::from_mnemonic(PROVIDER_MNEMONIC, "")?;
        let config = WalletConfig {
            network: SparkNetwork::Regtest,
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

    let provider_pubkey = hex::encode(keypair.public_key);
    let announce = AgentMessage::ServiceAnnouncement {
        kind: KIND_JOB_TEXT_GENERATION,
        price_msats: 10_000,
        spark_address,
        network: AgentNetwork::Regtest,
        provider_pubkey: Some(provider_pubkey.clone()),
        models: available_models.clone(),
        capabilities: vec!["text-generation".to_string()],
    };
    send_channel_message(&relay, &channel_id, &keypair, &announce).await?;
    println!("[PROVIDER] Service announced: kind=5050, price=10000 msats, network=regtest");
    println!("[PROVIDER] Models: {:?}", available_models);

    // Subscribe to channel
    let mut rx = subscribe_to_channel(&relay, &channel_id).await?;
    println!("[PROVIDER] Listening for job requests...\n");

    // Record start time to filter old messages
    let start_time = now();

    // Store pending jobs (job_id -> prompt)
    let mut pending_jobs: HashMap<String, String> = HashMap::new();

    // Track processed jobs to avoid duplicates
    let mut processed_jobs: HashSet<String> = HashSet::new();

    // Event loop
    while let Some(event) = rx.recv().await {
        // Skip old messages from before we started
        if event.created_at < start_time {
            continue;
        }

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
            AgentMessage::JobRequest { prompt, kind, max_tokens, target_provider } => {
                // Skip if targeted to another provider
                if let Some(ref target) = target_provider {
                    if target != &provider_pubkey {
                        continue;
                    }
                }

                println!("[PROVIDER] Got job request:");
                println!("           Kind: {}", kind);
                println!("           Prompt: {}", prompt);
                println!("           Max tokens: {}", max_tokens);

                let job_id = format!("job_{}", &event.id[..16]);

                // Store prompt for later processing
                pending_jobs.insert(job_id.clone(), prompt.clone());

                if let Some(ref w) = wallet {
                    // Create invoice
                    let invoice = w
                        .create_invoice(10, Some("NIP-90 Job".to_string()), Some(3600))
                        .await?;

                    // Use job_id as payment reference (payment_hash from bolt11 would require parsing)
                    let resp = AgentMessage::Invoice {
                        job_id: job_id.clone(),
                        bolt11: invoice.payment_request.clone(),
                        amount_msats: 10_000,
                        payment_hash: Some(job_id.clone()), // Use job_id as reference
                    };
                    send_channel_message(&relay, &channel_id, &keypair, &resp).await?;
                    println!("[PROVIDER] Invoice sent for job {}", job_id);
                } else {
                    // No wallet - send mock invoice
                    let resp = AgentMessage::Invoice {
                        job_id: job_id.clone(),
                        bolt11: "lnbcrt100n1mock".to_string(),
                        amount_msats: 10_000,
                        payment_hash: Some("mock_payment_hash".to_string()),
                    };
                    send_channel_message(&relay, &channel_id, &keypair, &resp).await?;
                    println!("[PROVIDER] Mock invoice sent for job {}", job_id);
                }
            }
            AgentMessage::PaymentSent { job_id, payment_id } => {
                // Skip if already processed
                if processed_jobs.contains(&job_id) {
                    println!("[PROVIDER] Skipping already processed job: {}", job_id);
                    continue;
                }
                processed_jobs.insert(job_id.clone());

                println!("[PROVIDER] Payment received for {}: {}", job_id, payment_id);

                // Get the stored prompt
                let prompt = pending_jobs.remove(&job_id).unwrap_or_else(|| "Hello".to_string());
                println!("[PROVIDER] Processing prompt: {}", prompt);

                // Run inference
                if let Some(backend) = registry.default() {
                    println!("[PROVIDER] Running inference with {}...", registry.default_id().unwrap_or("unknown"));

                    let request = CompletionRequest::new(&model_to_use, &prompt)
                        .with_max_tokens(256)
                        .with_temperature(0.7);

                    let backend = backend.read().await;

                    if args.stream {
                        // Streaming mode - send chunks as they arrive
                        match backend.complete_stream(request).await {
                            Ok(mut rx) => {
                                let mut accumulated = String::new();
                                let mut chunk_count = 0;

                                while let Some(chunk_result) = rx.recv().await {
                                    match chunk_result {
                                        Ok(chunk) => {
                                            accumulated.push_str(&chunk.delta);
                                            chunk_count += 1;

                                            // Send streaming chunk to channel
                                            let stream_msg = AgentMessage::StreamChunk {
                                                job_id: job_id.clone(),
                                                chunk: chunk.delta.clone(),
                                                is_final: chunk.finish_reason.is_some(),
                                            };
                                            send_channel_message(&relay, &channel_id, &keypair, &stream_msg).await?;

                                            if chunk.finish_reason.is_some() {
                                                break;
                                            }
                                        }
                                        Err(e) => {
                                            println!("[PROVIDER] Streaming error: {}", e);
                                            break;
                                        }
                                    }
                                }

                                println!("[PROVIDER] Streamed {} chunks", chunk_count);

                                // Send final complete result
                                let result = AgentMessage::JobResult {
                                    job_id: job_id.clone(),
                                    result: accumulated,
                                };
                                send_channel_message(&relay, &channel_id, &keypair, &result).await?;
                            }
                            Err(e) => {
                                println!("[PROVIDER] Streaming setup error: {}", e);
                                let result = AgentMessage::JobResult {
                                    job_id: job_id.clone(),
                                    result: format!("Error: {}", e),
                                };
                                send_channel_message(&relay, &channel_id, &keypair, &result).await?;
                            }
                        }
                    } else {
                        // Non-streaming mode - wait for complete response
                        match backend.complete(request).await {
                            Ok(response) => {
                                println!("[PROVIDER] Inference complete ({} tokens)",
                                    response.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0));
                                let result = AgentMessage::JobResult {
                                    job_id: job_id.clone(),
                                    result: response.text,
                                };
                                send_channel_message(&relay, &channel_id, &keypair, &result).await?;
                            }
                            Err(e) => {
                                println!("[PROVIDER] Inference error: {}", e);
                                let result = AgentMessage::JobResult {
                                    job_id: job_id.clone(),
                                    result: format!("Error: {}", e),
                                };
                                send_channel_message(&relay, &channel_id, &keypair, &result).await?;
                            }
                        }
                    }
                } else {
                    println!("[PROVIDER] No backend available, returning error");
                    let result = AgentMessage::JobResult {
                        job_id: job_id.clone(),
                        result: "Error: No inference backend available. Install Ollama: https://ollama.ai".to_string(),
                    };
                    send_channel_message(&relay, &channel_id, &keypair, &result).await?;
                }

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
            AgentMessage::StreamChunk { .. } => {
                // Ignore stream chunks (we send these)
            }
        }
    }

    Ok(())
}
