//! Provider Agent Binary
//!
//! Run on Computer A to provide NIP-90 compute services.
//!
//! Primary flow uses direct NIP-90 events (no channel required):
//!   cargo run --bin agent-provider
//!
//! Optional: Create a NIP-28 channel for coordination:
//!   cargo run --bin agent-provider -- --create-channel
//!
//! Optional: Join existing channel for coordination:
//!   cargo run --bin agent-provider -- --channel <CHANNEL_ID>

use clap::Parser;
use compute::backends::{BackendRegistry, CompletionRequest};
use nostr::{
    derive_keypair, finalize_event, Event, EventTemplate, HandlerInfo, HandlerMetadata,
    HandlerType, Keypair, PricingInfo, KIND_HANDLER_INFO,
};
use openagents::agents::{
    create_channel, now, parse_agent_message, parse_job_request, publish_job_feedback,
    publish_job_result, send_channel_message, subscribe_job_requests, subscribe_to_channel,
    AgentMessage, JobStatus, Network as AgentNetwork, RelayApi, RelayHub, SharedRelay,
    DEFAULT_RELAY, KIND_JOB_REQUEST_TEXT, KIND_JOB_RESULT_TEXT, PROVIDER_MNEMONIC,
};
use openagents_spark::{Network as SparkNetwork, SparkSigner, SparkWallet, WalletConfig};
use std::collections::{HashMap, HashSet};
use std::env::temp_dir;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "agent-provider")]
#[command(about = "NIP-90 Provider Agent - provides compute services via direct events or optional NIP-28 channels")]
struct Args {
    /// Create a new NIP-28 channel for coordination (optional)
    #[arg(long)]
    create_channel: bool,

    /// Join existing NIP-28 channel for coordination (optional)
    #[arg(long)]
    channel: Option<String>,

    /// Relay URL (repeat or comma-delimit to use multiple relays)
    #[arg(long = "relay", value_delimiter = ',', default_value = DEFAULT_RELAY)]
    relays: Vec<String>,

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

/// Run the inference and return the result text
async fn run_inference(
    registry: &BackendRegistry,
    model: &str,
    prompt: &str,
    stream: bool,
    relay: &dyn RelayApi,
    keypair: &Keypair,
    job_id: &str,
    _customer_pubkey: &str,
    channel_id: Option<&str>,
) -> Result<String> {
    if let Some(backend) = registry.default() {
        let request = CompletionRequest::new(model, prompt)
            .with_max_tokens(256)
            .with_temperature(0.7);

        let backend = backend.read().await;

        if stream {
            // Streaming mode
            match backend.complete_stream(request).await {
                Ok(mut rx) => {
                    let mut accumulated = String::new();

                    while let Some(chunk_result) = rx.recv().await {
                        match chunk_result {
                            Ok(chunk) => {
                                accumulated.push_str(&chunk.delta);

                                // If using channel, send streaming chunk
                                if let Some(ch_id) = channel_id {
                                    let stream_msg = AgentMessage::StreamChunk {
                                        job_id: job_id.to_string(),
                                        chunk: chunk.delta.clone(),
                                        is_final: chunk.finish_reason.is_some(),
                                    };
                                    let _ =
                                        send_channel_message(relay, ch_id, keypair, &stream_msg)
                                            .await;
                                }

                                if chunk.finish_reason.is_some() {
                                    break;
                                }
                            }
                            Err(e) => {
                                return Err(format!("Streaming error: {}", e).into());
                            }
                        }
                    }

                    Ok(accumulated)
                }
                Err(e) => Err(format!("Streaming setup error: {}", e).into()),
            }
        } else {
            // Non-streaming mode
            match backend.complete(request).await {
                Ok(response) => {
                    println!(
                        "[PROVIDER] Inference complete ({} tokens)",
                        response.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0)
                    );
                    Ok(response.text)
                }
                Err(e) => Err(format!("Inference error: {}", e).into()),
            }
        }
    } else {
        Err("No inference backend available. Install Ollama: https://ollama.ai".into())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("=== OpenAgents Provider Agent ===\n");

    // Derive keypair
    let keypair = derive_keypair(PROVIDER_MNEMONIC)?;
    let provider_pubkey = hex::encode(keypair.public_key);
    println!("[PROVIDER] Public key: {}", provider_pubkey);

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
    let model_to_use = args
        .model
        .clone()
        .unwrap_or_else(|| available_models.first().cloned().unwrap_or_else(|| "llama3.2".to_string()));
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

    // Connect to relays
    let relay_urls = if args.relays.is_empty() {
        vec![DEFAULT_RELAY.to_string()]
    } else {
        args.relays.clone()
    };
    println!(
        "[PROVIDER] Connecting to relays: {}",
        relay_urls.join(", ")
    );
    let relay_hub = Arc::new(RelayHub::new(relay_urls)?);
    relay_hub.connect_all().await?;
    println!("[PROVIDER] Connected to relays");
    let relay: SharedRelay = relay_hub.clone();

    // Optional: Get or create channel for coordination
    let channel_id: Option<String> = if args.create_channel {
        let id = create_channel(
            relay.as_ref(),
            &keypair,
            "OpenAgents Compute Marketplace",
            "Agents negotiate NIP-90 jobs with Bitcoin payments",
        )
        .await?;
        println!("\n========================================");
        println!("CHANNEL CREATED: {}", id);
        println!("========================================");
        println!("\nChannel is OPTIONAL. The primary flow uses direct NIP-90 events.");
        println!("Share this channel ID for coordination/discussion:");
        println!(
            "  cargo run --bin agent-customer -- --channel {} --prompt \"...\"",
            id
        );
        println!("========================================\n");
        Some(id)
    } else if let Some(id) = args.channel {
        println!("[PROVIDER] Joining channel for coordination: {}", id);
        Some(id)
    } else {
        println!("[PROVIDER] No channel specified - using direct NIP-90 events only");
        None
    };

    // Build NIP-89 handler info for global discovery
    let handler_metadata = HandlerMetadata::new(
        "OpenAgents Compute Provider",
        "NIP-90 text generation service with Lightning payments",
    );
    let mut handler_info = HandlerInfo::new(
        provider_pubkey.clone(),
        HandlerType::ComputeProvider,
        handler_metadata,
    )
    .add_capability("text-generation")
    .with_pricing(
        PricingInfo::new(10_000)
            .with_model("per-request")
            .with_currency("msats"),
    )
    .add_custom_tag("relay", relay.relay_url())
    .add_custom_tag("network", "regtest");

    // Only add channel tag if we have a channel
    if let Some(ref ch_id) = channel_id {
        handler_info = handler_info.add_custom_tag("channel", ch_id);
    }

    // Add models as capabilities
    for model in &available_models {
        handler_info = handler_info.add_custom_tag("model", model);
    }

    // Publish handler info (kind 31990)
    let handler_tags = handler_info.to_tags();
    let handler_content = serde_json::to_string(&serde_json::json!({
        "name": "OpenAgents Compute Provider",
        "description": "NIP-90 text generation service with Lightning payments",
    }))?;

    let handler_template = EventTemplate {
        created_at: now(),
        kind: KIND_HANDLER_INFO,
        tags: handler_tags,
        content: handler_content,
    };

    let handler_event = finalize_event(&handler_template, &keypair.private_key)?;
    relay
        .publish_event(&handler_event, Duration::from_secs(10))
        .await?;
    println!("[PROVIDER] Published NIP-89 handler info (kind 31990) for global discovery");

    // If we have a channel, announce service there too (legacy support)
    if let Some(ref ch_id) = channel_id {
        let spark_address = if let Some(ref w) = wallet {
            w.get_spark_address().await?
        } else {
            "mock-spark-address".to_string()
        };

        let announce = AgentMessage::ServiceAnnouncement {
            kind: KIND_JOB_REQUEST_TEXT,
            price_msats: 10_000,
            spark_address,
            network: AgentNetwork::Regtest,
            provider_pubkey: Some(provider_pubkey.clone()),
            models: available_models.clone(),
            capabilities: vec!["text-generation".to_string()],
        };
        send_channel_message(relay.as_ref(), ch_id, &keypair, &announce).await?;
        println!("[PROVIDER] Service announced in channel");
    }

    // Subscribe to direct NIP-90 job requests (kind:5050 tagged with our pubkey)
    let mut job_rx =
        subscribe_job_requests(relay.as_ref(), &provider_pubkey, &[KIND_JOB_REQUEST_TEXT]).await?;
    println!("[PROVIDER] Listening for direct NIP-90 job requests (kind:5050)...");

    // Also subscribe to channel if we have one (for legacy/coordination)
    let mut channel_rx: Option<mpsc::Receiver<Event>> = if let Some(ref ch_id) = channel_id {
        let subscription_id = format!("provider-channel-{}", Uuid::new_v4());
        let rx = subscribe_to_channel(relay.as_ref(), ch_id, &subscription_id).await?;
        println!("[PROVIDER] Also listening on channel for legacy support");
        Some(rx)
    } else {
        None
    };

    // Record start time to filter old messages
    let start_time = now();

    // Store pending jobs (job_id -> (prompt, customer_pubkey, is_channel_job))
    let mut pending_jobs: HashMap<String, (String, String, bool)> = HashMap::new();

    // Track processed jobs to avoid duplicates
    let mut processed_jobs: HashSet<String> = HashSet::new();

    println!("\n[PROVIDER] Ready! Waiting for job requests...\n");

    loop {
        tokio::select! {
            // Handle direct NIP-90 job requests
            Some(event) = job_rx.recv() => {
                // Skip old messages
                if event.created_at < start_time {
                    continue;
                }

                // Skip our own messages
                if event.pubkey == provider_pubkey {
                    continue;
                }

                // Parse the job request
                if let Some((prompt, max_tokens, _target)) = parse_job_request(&event) {
                    let job_id = event.id.clone();
                    let customer_pubkey = event.pubkey.clone();

                    println!("[PROVIDER] Got direct NIP-90 job request:");
                    println!("           Job ID: {}", job_id);
                    println!("           From: {}...", &customer_pubkey[..16.min(customer_pubkey.len())]);
                    println!("           Prompt: {}", prompt);
                    println!("           Max tokens: {}", max_tokens);

                    // Store for later processing
                    pending_jobs.insert(job_id.clone(), (prompt.clone(), customer_pubkey.clone(), false));

                    if let Some(ref w) = wallet {
                        // Create invoice
                        let invoice = w
                            .create_invoice(10, Some("NIP-90 Job".to_string()), Some(3600))
                            .await?;

                        // Send feedback with invoice (kind:7000)
                        publish_job_feedback(
                            relay.as_ref(),
                            &keypair,
                            &job_id,
                            &customer_pubkey,
                            JobStatus::PaymentRequired,
                            Some(&invoice.payment_request),
                            Some(10_000),
                        ).await?;

                        println!("[PROVIDER] Invoice sent via NIP-90 feedback (kind:7000)");
                    } else {
                        // No wallet - send mock invoice via feedback
                        publish_job_feedback(
                            relay.as_ref(),
                            &keypair,
                            &job_id,
                            &customer_pubkey,
                            JobStatus::PaymentRequired,
                            Some("lnbcrt100n1mock"),
                            Some(10_000),
                        ).await?;

                        println!("[PROVIDER] Mock invoice sent via NIP-90 feedback (kind:7000)");

                        // For testing without wallet, process immediately
                        if !processed_jobs.contains(&job_id) {
                            processed_jobs.insert(job_id.clone());

                            // Send processing status
                            publish_job_feedback(
                                relay.as_ref(),
                                &keypair,
                                &job_id,
                                &customer_pubkey,
                                JobStatus::Processing,
                                None,
                                None,
                            ).await?;

                            // Run inference
                            let result_text = match run_inference(
                                &registry,
                                &model_to_use,
                                &prompt,
                                args.stream,
                                relay.as_ref(),
                                &keypair,
                                &job_id,
                                &customer_pubkey,
                                channel_id.as_deref(),
                            ).await {
                                Ok(text) => text,
                                Err(e) => format!("Error: {}", e),
                            };

                            // Send result via NIP-90 (kind:6050)
                            publish_job_result(
                                relay.as_ref(),
                                &keypair,
                                &job_id,
                                &customer_pubkey,
                                &result_text,
                                KIND_JOB_RESULT_TEXT,
                            ).await?;

                            println!("[PROVIDER] Result delivered via NIP-90 (kind:6050)");
                            pending_jobs.remove(&job_id);
                        }
                    }
                }
            }

            // Handle channel messages (legacy/coordination)
            Some(event) = async {
                if let Some(ref mut rx) = channel_rx {
                    rx.recv().await
                } else {
                    std::future::pending::<Option<Event>>().await
                }
            } => {
                // Skip old messages
                if event.created_at < start_time {
                    continue;
                }

                // Skip our own messages
                if event.pubkey == provider_pubkey {
                    continue;
                }

                // Parse channel message
                let msg = match parse_agent_message(&event.content) {
                    Some(m) => m,
                    None => continue,
                };

                match msg {
                    AgentMessage::JobRequest { prompt, kind: _, max_tokens, target_provider } => {
                        // Skip if targeted to another provider
                        if let Some(ref target) = target_provider {
                            if target != &provider_pubkey {
                                continue;
                            }
                        }

                        println!("[PROVIDER] Got channel job request:");
                        println!("           Prompt: {}", prompt);
                        println!("           Max tokens: {}", max_tokens);

                        let job_id = format!("job_{}", &event.id[..16]);
                        let customer_pubkey = event.pubkey.clone();

                        // Store prompt for later processing
                        pending_jobs.insert(job_id.clone(), (prompt.clone(), customer_pubkey, true));

                        if let Some(ref w) = wallet {
                            // Create invoice
                            let invoice = w
                                .create_invoice(10, Some("NIP-90 Job".to_string()), Some(3600))
                                .await?;

                            let resp = AgentMessage::Invoice {
                                job_id: job_id.clone(),
                                bolt11: invoice.payment_request.clone(),
                                amount_msats: 10_000,
                                payment_hash: Some(job_id.clone()),
                            };
                            send_channel_message(
                                relay.as_ref(),
                                channel_id.as_ref().unwrap(),
                                &keypair,
                                &resp,
                            )
                            .await?;
                            println!("[PROVIDER] Invoice sent via channel for job {}", job_id);
                        } else {
                            // No wallet - send mock invoice
                            let resp = AgentMessage::Invoice {
                                job_id: job_id.clone(),
                                bolt11: "lnbcrt100n1mock".to_string(),
                                amount_msats: 10_000,
                                payment_hash: Some("mock_payment_hash".to_string()),
                            };
                            send_channel_message(
                                relay.as_ref(),
                                channel_id.as_ref().unwrap(),
                                &keypair,
                                &resp,
                            )
                            .await?;
                            println!("[PROVIDER] Mock invoice sent via channel for job {}", job_id);
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
                        let (prompt, customer_pubkey, is_channel) = pending_jobs
                            .remove(&job_id)
                            .unwrap_or_else(|| ("Hello".to_string(), event.pubkey.clone(), true));

                        println!("[PROVIDER] Processing prompt: {}", prompt);

                        // Run inference
                        let result_text = match run_inference(
                            &registry,
                            &model_to_use,
                            &prompt,
                            args.stream,
                            relay.as_ref(),
                            &keypair,
                            &job_id,
                            &customer_pubkey,
                            channel_id.as_deref(),
                        ).await {
                            Ok(text) => text,
                            Err(e) => format!("Error: {}", e),
                        };

                        if is_channel {
                            // Send result via channel
                            let result = AgentMessage::JobResult {
                                job_id: job_id.clone(),
                                result: result_text,
                            };
                            send_channel_message(
                                relay.as_ref(),
                                channel_id.as_ref().unwrap(),
                                &keypair,
                                &result,
                            )
                            .await?;
                            println!("[PROVIDER] Result delivered via channel for {}", job_id);
                        } else {
                            // Send result via direct NIP-90 event
                            publish_job_result(
                                relay.as_ref(),
                                &keypair,
                                &job_id,
                                &customer_pubkey,
                                &result_text,
                                KIND_JOB_RESULT_TEXT,
                            ).await?;
                            println!("[PROVIDER] Result delivered via NIP-90 (kind:6050)");
                        }

                        println!("\n[PROVIDER] Job complete! Waiting for more requests...\n");
                    }
                    AgentMessage::HtlcLocked { job_id, payment_hash, amount_msats, expiry_secs } => {
                        println!("[PROVIDER] HTLC locked for job {}:", job_id);
                        println!("           Payment hash: {}...", &payment_hash[..16.min(payment_hash.len())]);
                        println!("           Amount: {} msats", amount_msats);
                        println!("           Expiry: {} secs", expiry_secs);
                        println!("[PROVIDER] HTLC escrow noted. Waiting for PaymentSent confirmation...");
                    }
                    AgentMessage::PreimageRelease { job_id, preimage } => {
                        println!("[PROVIDER] Preimage released for job {}", job_id);
                        println!("           Preimage: {}...", &preimage[..16.min(preimage.len())]);

                        if let Some(ref w) = wallet {
                            println!("[PROVIDER] Claiming HTLC payment...");
                            match w.claim_htlc_payment(&preimage).await {
                                Ok(_) => {
                                    println!("[PROVIDER] HTLC payment claimed successfully!");
                                }
                                Err(e) => {
                                    println!("[PROVIDER] Failed to claim HTLC: {}", e);
                                }
                            }
                        } else {
                            println!("[PROVIDER] No wallet - would claim HTLC with preimage");
                        }
                    }
                    // Ignore messages we send
                    AgentMessage::ServiceAnnouncement { .. } => {}
                    AgentMessage::Invoice { .. } => {}
                    AgentMessage::JobResult { .. } => {}
                    AgentMessage::StreamChunk { .. } => {}
                }
            }
        }
    }
}
