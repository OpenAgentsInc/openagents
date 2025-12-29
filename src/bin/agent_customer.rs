//! Customer Agent Binary
//!
//! Run on Computer B to request NIP-90 compute services.
//!
//! Primary flow uses direct NIP-90 events (no channel required):
//!   cargo run --bin agent-customer -- --prompt "Your question"
//!
//! Optional: Use NIP-28 channel for coordination:
//!   cargo run --bin agent-customer -- --channel <CHANNEL_ID> --prompt "Your question"

use clap::Parser;
use nostr::{derive_keypair, Event, HandlerInfo, Keypair, KIND_HANDLER_INFO};
use openagents::agents::{
    now, parse_agent_message, parse_job_feedback, parse_job_result, publish_job_request,
    send_channel_message, subscribe_job_responses, subscribe_to_channel, AgentMessage, JobStatus,
    RelayApi, RelayHub, SharedRelay, CUSTOMER_MNEMONIC, DEFAULT_RELAY, KIND_JOB_FEEDBACK,
    KIND_JOB_REQUEST_TEXT, KIND_JOB_RESULT_TEXT,
};
use openagents_spark::{Network as SparkNetwork, SparkSigner, SparkWallet, WalletConfig};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env::temp_dir;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Provider info discovered via NIP-89 (kind 31990)
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for metadata display
struct DiscoveredProvider {
    pubkey: String,
    name: String,
    /// NIP-28 channel ID (optional - only if provider uses channels)
    channel_id: Option<String>,
    relay_url: String,
    network: String,
    price_msats: u64,
    models: Vec<String>,
}

#[derive(Parser)]
#[command(name = "agent-customer")]
#[command(about = "NIP-90 Customer Agent - requests compute services via direct events or optional NIP-28 channels")]
struct Args {
    /// NIP-28 channel ID for legacy/coordination (optional - primary flow uses direct events)
    #[arg(long)]
    channel: Option<String>,

    /// Job prompt - the question or task to send to the provider
    #[arg(long)]
    prompt: String,

    /// Relay URL (repeat or comma-delimit to use multiple relays)
    #[arg(long = "relay", value_delimiter = ',', default_value = DEFAULT_RELAY)]
    relays: Vec<String>,

    /// Skip wallet initialization (for testing without Spark)
    #[arg(long)]
    no_wallet: bool,

    /// Time to wait for provider discovery (seconds)
    #[arg(long, default_value = "3")]
    discovery_time: u64,

    /// Select provider by: cheapest, first, or specific pubkey
    #[arg(long, default_value = "first")]
    select: String,

    /// Filter by max price in msats
    #[arg(long)]
    max_price: Option<u64>,

    /// Use HTLC escrow for trustless payments (experimental, channel mode only)
    #[arg(long)]
    htlc: bool,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Discover providers via NIP-89 (kind 31990)
async fn discover_providers(
    relay: &dyn RelayApi,
    discovery_time: u64,
    max_price: Option<u64>,
) -> Result<Vec<DiscoveredProvider>> {
    println!("[CUSTOMER] Discovering providers via NIP-89 (kind 31990)...");

    let filters = vec![serde_json::json!({
        "kinds": [KIND_HANDLER_INFO as u64],
        "limit": 50
    })];

    let subscription_id = format!("nip89-discovery-{}", Uuid::new_v4());
    let mut discovery_rx = relay
        .subscribe_with_channel(&subscription_id, &filters)
        .await?;

    // Collect events during discovery period
    let mut events: Vec<Event> = Vec::new();
    let discovery_deadline = std::time::Instant::now() + Duration::from_secs(discovery_time);

    while std::time::Instant::now() < discovery_deadline {
        let remaining = discovery_deadline.saturating_duration_since(std::time::Instant::now());
        match tokio::time::timeout(remaining.max(Duration::from_millis(100)), discovery_rx.recv())
            .await
        {
            Ok(Some(event)) => events.push(event),
            Ok(None) => break,
            Err(_) => break,
        }
    }

    println!("[CUSTOMER] Found {} handler info events", events.len());

    // Parse and filter providers
    let mut discovered: Vec<DiscoveredProvider> = Vec::new();

    for event in events {
        let handler = match HandlerInfo::from_event(&event) {
            Ok(h) => h,
            Err(_) => continue,
        };

        // Only want compute providers
        if handler.handler_type != nostr::HandlerType::ComputeProvider {
            continue;
        }

        // Extract channel_id from custom tags (optional)
        let channel_id = handler
            .custom_tags
            .iter()
            .find(|(k, _)| k == "channel")
            .map(|(_, v)| v.clone());

        // Extract other custom tags
        let relay_url = handler
            .custom_tags
            .iter()
            .find(|(k, _)| k == "relay")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| DEFAULT_RELAY.to_string());

        let network = handler
            .custom_tags
            .iter()
            .find(|(k, _)| k == "network")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // Extract models from custom tags
        let models: Vec<String> = handler
            .custom_tags
            .iter()
            .filter(|(k, _)| k == "model")
            .map(|(_, v)| v.clone())
            .collect();

        // Get price
        let price_msats = handler.pricing.as_ref().map(|p| p.amount).unwrap_or(0);

        // Filter by max price if specified
        if let Some(max) = max_price {
            if price_msats > max {
                continue;
            }
        }

        // Filter by network (we want regtest)
        if network != "regtest" {
            println!(
                "[CUSTOMER] Skipping provider on {} (we need regtest)",
                network
            );
            continue;
        }

        discovered.push(DiscoveredProvider {
            pubkey: handler.pubkey.clone(),
            name: handler.metadata.name.clone(),
            channel_id,
            relay_url,
            network,
            price_msats,
            models,
        });
    }

    Ok(discovered)
}

/// Request compute via direct NIP-90 events (primary flow)
async fn request_via_direct_events(
    relay: &dyn RelayApi,
    keypair: &Keypair,
    provider: &DiscoveredProvider,
    prompt: &str,
    wallet: Option<&SparkWallet>,
    budget_sats: u64,
) -> Result<String> {
    let customer_pubkey = hex::encode(keypair.public_key);

    // Publish job request (kind:5050)
    let job_request_id = publish_job_request(
        relay,
        keypair,
        &provider.pubkey,
        prompt,
        256,
        KIND_JOB_REQUEST_TEXT,
    )
    .await
    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
        format!("Failed to publish job request: {}", e).into()
    })?;

    println!("[CUSTOMER] Job request published: {}", job_request_id);
    println!("[CUSTOMER] Waiting for provider response...");

    // Subscribe to responses for this job
    let mut rx = subscribe_job_responses(relay, &job_request_id)
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            format!("Failed to subscribe to responses: {}", e).into()
        })?;

    // Wait for feedback and result
    let timeout = Duration::from_secs(120);
    let job_start = std::time::Instant::now();
    let mut paid = false;

    loop {
        if job_start.elapsed() > timeout {
            return Err("Timeout waiting for compute result".into());
        }

        let event = match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
            Ok(Some(e)) => e,
            Ok(None) => return Err("Channel closed".into()),
            Err(_) => continue,
        };

        // Skip our own messages
        if event.pubkey == customer_pubkey {
            continue;
        }

        // Handle feedback events (kind:7000)
        if event.kind == KIND_JOB_FEEDBACK {
            if let Some((job_id, status, bolt11, amount)) = parse_job_feedback(&event) {
                if job_id != job_request_id {
                    continue;
                }

                match status {
                    JobStatus::PaymentRequired => {
                        if paid {
                            continue;
                        }

                        let bolt11 =
                            bolt11.ok_or::<Box<dyn std::error::Error + Send + Sync>>(
                                "No invoice in feedback".into(),
                            )?;
                        let amount_msats = amount.unwrap_or(provider.price_msats);
                        let amount_sats = amount_msats / 1000;

                        println!("[CUSTOMER] Got invoice: {} sats", amount_sats);

                        if amount_sats > budget_sats {
                            return Err(format!(
                                "Invoice amount {} sats exceeds budget {} sats",
                                amount_sats, budget_sats
                            )
                            .into());
                        }

                        // Pay the invoice
                        if let Some(w) = wallet {
                            println!("[CUSTOMER] Paying invoice...");
                            let payment = w.send_payment_simple(&bolt11, None).await?;
                            println!("[CUSTOMER] Payment sent: {}", payment.payment.id);
                        } else {
                            println!("[CUSTOMER] Mock payment (no wallet)");
                        }
                        paid = true;
                    }
                    JobStatus::Processing => {
                        println!("[CUSTOMER] Job is processing...");
                    }
                    JobStatus::Success => {
                        println!("[CUSTOMER] Job completed successfully");
                    }
                    JobStatus::Error => {
                        return Err("Job failed with error".into());
                    }
                    JobStatus::Cancelled => {
                        return Err("Job was cancelled".into());
                    }
                }
            }
        }

        // Handle result events (kind:6050)
        if event.kind == KIND_JOB_RESULT_TEXT {
            if let Some((job_id, result)) = parse_job_result(&event) {
                if job_id == job_request_id {
                    println!("\n========================================");
                    println!("JOB RESULT RECEIVED");
                    println!("========================================");
                    println!("Job ID: {}", job_id);
                    println!("Result: {}", result);
                    println!("========================================\n");
                    return Ok(result);
                }
            }
        }
    }
}

/// Request compute via NIP-28 channel (legacy flow for backward compatibility)
async fn request_via_channel(
    relay: &dyn RelayApi,
    keypair: &Keypair,
    channel_id: &str,
    provider_pubkey: &str,
    prompt: &str,
    wallet: Option<&SparkWallet>,
    htlc_mode: bool,
) -> Result<String> {
    let customer_pubkey = hex::encode(keypair.public_key);

    // Subscribe to channel
    let subscription_id = format!("customer-channel-{}", Uuid::new_v4());
    let mut rx: mpsc::Receiver<Event> =
        subscribe_to_channel(relay, channel_id, &subscription_id)
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                format!("Failed to subscribe to channel: {}", e).into()
            })?;

    // Record start time (accept messages from last 5 minutes)
    let start_time = now().saturating_sub(300);

    // Send job request
    let request = AgentMessage::JobRequest {
        kind: KIND_JOB_REQUEST_TEXT,
        prompt: prompt.to_string(),
        max_tokens: 100,
        target_provider: Some(provider_pubkey.to_string()),
    };
    send_channel_message(relay, channel_id, keypair, &request)
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            format!("Failed to send job request: {}", e).into()
        })?;

    println!("[CUSTOMER] Job requested via channel: {}", prompt);

    // Wait for invoice and result
    let mut our_job_id: Option<String> = None;
    let mut htlc_preimages: HashMap<String, String> = HashMap::new();
    let timeout = Duration::from_secs(120);
    let job_start = std::time::Instant::now();

    loop {
        if job_start.elapsed() > timeout {
            return Err("Timeout waiting for response".into());
        }

        let event = match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
            Ok(Some(e)) => e,
            Ok(None) => return Err("Channel closed".into()),
            Err(_) => continue,
        };

        // Skip old messages
        if event.created_at < start_time {
            continue;
        }

        // Skip our own messages
        if event.pubkey == customer_pubkey {
            continue;
        }

        let msg = match parse_agent_message(&event.content) {
            Some(m) => m,
            None => continue,
        };

        match msg {
            AgentMessage::Invoice {
                bolt11,
                job_id,
                amount_msats,
                payment_hash,
            } => {
                // Skip invoices for other jobs
                if our_job_id.is_some() && our_job_id.as_ref() != Some(&job_id) {
                    continue;
                }

                println!("[CUSTOMER] Got invoice:");
                println!("           Job ID: {}", job_id);
                println!("           Amount: {} msats", amount_msats);
                if let Some(ref hash) = payment_hash {
                    println!(
                        "           Payment Hash: {}...",
                        &hash[..16.min(hash.len())]
                    );
                }

                our_job_id = Some(job_id.clone());

                if htlc_mode {
                    // HTLC escrow mode
                    let preimage_bytes: [u8; 32] = rand::rng().random();
                    let preimage = hex::encode(&preimage_bytes);

                    let mut hasher = Sha256::new();
                    hasher.update(&preimage_bytes);
                    let hash_result = hasher.finalize();
                    let htlc_payment_hash = hex::encode(hash_result);

                    htlc_preimages.insert(job_id.clone(), preimage);

                    if let Some(w) = wallet {
                        let amount_sats = amount_msats / 1000;
                        let expiry_secs: u64 = 3600;

                        match w
                            .send_htlc_payment(&bolt11, amount_sats, &htlc_payment_hash, expiry_secs, None)
                            .await
                        {
                            Ok(payment) => {
                                let payment_id = payment.payment.id.clone();

                                let locked = AgentMessage::HtlcLocked {
                                    job_id: job_id.clone(),
                                    payment_hash: htlc_payment_hash,
                                    amount_msats,
                                    expiry_secs,
                                };
                                let _ = send_channel_message(relay, channel_id, keypair, &locked).await;

                                let confirm = AgentMessage::PaymentSent { job_id, payment_id };
                                let _ = send_channel_message(relay, channel_id, keypair, &confirm).await;
                            }
                            Err(e) => {
                                println!("[CUSTOMER] HTLC payment failed: {}", e);
                                htlc_preimages.remove(&job_id);
                            }
                        }
                    } else {
                        let locked = AgentMessage::HtlcLocked {
                            job_id: job_id.clone(),
                            payment_hash: htlc_payment_hash,
                            amount_msats,
                            expiry_secs: 3600,
                        };
                        let _ = send_channel_message(relay, channel_id, keypair, &locked).await;

                        let confirm = AgentMessage::PaymentSent {
                            job_id,
                            payment_id: "mock-htlc-payment".to_string(),
                        };
                        let _ = send_channel_message(relay, channel_id, keypair, &confirm).await;
                    }
                } else {
                    // Regular payment mode
                    if let Some(w) = wallet {
                        println!("[CUSTOMER] Paying invoice...");
                        let payment = w.send_payment_simple(&bolt11, None).await?;
                        let payment_id = payment.payment.id.clone();
                        println!("[CUSTOMER] Payment sent: {}", payment_id);

                        let confirm = AgentMessage::PaymentSent { job_id, payment_id };
                        let _ = send_channel_message(relay, channel_id, keypair, &confirm).await;
                    } else {
                        println!("[CUSTOMER] Mock payment (no wallet)");
                        let confirm = AgentMessage::PaymentSent {
                            job_id,
                            payment_id: "mock-payment-id".to_string(),
                        };
                        let _ = send_channel_message(relay, channel_id, keypair, &confirm).await;
                    }
                }
            }
            AgentMessage::JobResult { job_id, result } => {
                if our_job_id.as_ref() != Some(&job_id) {
                    continue;
                }

                println!("\n========================================");
                println!("JOB RESULT RECEIVED");
                println!("========================================");
                println!("Job ID: {}", job_id);
                println!("Result: {}", result);
                println!("========================================\n");

                // Release preimage in HTLC mode
                if htlc_mode {
                    if let Some(preimage) = htlc_preimages.remove(&job_id) {
                        let release = AgentMessage::PreimageRelease {
                            job_id: job_id.clone(),
                            preimage,
                        };
                        let _ = send_channel_message(relay, channel_id, keypair, &release).await;
                        println!("[CUSTOMER] Preimage released");
                    }
                }

                return Ok(result);
            }
            AgentMessage::StreamChunk {
                job_id,
                chunk,
                is_final,
            } => {
                if our_job_id.as_ref() == Some(&job_id) {
                    print!("{}", chunk);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                    if is_final {
                        println!();
                    }
                }
            }
            _ => {}
        }
    }
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

    // Connect to relays
    let relay_urls = if args.relays.is_empty() {
        vec![DEFAULT_RELAY.to_string()]
    } else {
        args.relays.clone()
    };
    println!(
        "[CUSTOMER] Connecting to relays: {}",
        relay_urls.join(", ")
    );
    let relay_hub = Arc::new(RelayHub::new(relay_urls)?);
    relay_hub.connect_all().await?;
    println!("[CUSTOMER] Connected to relays");
    let relay: SharedRelay = relay_hub.clone();

    // Discover providers via NIP-89
    let discovered =
        discover_providers(relay.as_ref(), args.discovery_time, args.max_price).await?;

    if discovered.is_empty() {
        println!("[CUSTOMER] No providers discovered via NIP-89!");
        println!("[CUSTOMER] Make sure providers are running and have published handler info.");
        relay.disconnect().await.ok();
        return Ok(());
    }

    // Display discovered providers
    println!(
        "\n[CUSTOMER] Discovered {} provider(s) via NIP-89:",
        discovered.len()
    );
    for (i, p) in discovered.iter().enumerate() {
        println!("  [{}] {}", i, p.name);
        println!(
            "      Pubkey: {}...",
            &p.pubkey[..16.min(p.pubkey.len())]
        );
        println!("      Price: {} msats", p.price_msats);
        if let Some(ref ch) = p.channel_id {
            println!("      Channel: {}...", &ch[..16.min(ch.len())]);
        } else {
            println!("      Channel: none (direct events only)");
        }
        println!("      Models: {:?}", p.models);
    }

    // Select provider
    let selected = match args.select.as_str() {
        "cheapest" => discovered.iter().min_by_key(|p| p.price_msats).cloned(),
        "first" => discovered.first().cloned(),
        pubkey => discovered
            .iter()
            .find(|p| p.pubkey.starts_with(pubkey))
            .cloned()
            .or_else(|| discovered.first().cloned()),
    };

    let selected = match selected {
        Some(p) => p,
        None => {
            println!("[CUSTOMER] Failed to select provider");
            return Ok(());
        }
    };

    println!(
        "\n[CUSTOMER] Selected: {} ({}...)",
        selected.name,
        &selected.pubkey[..16.min(selected.pubkey.len())]
    );

    // Determine which flow to use
    let use_channel = args.channel.is_some() || (args.htlc && selected.channel_id.is_some());

    let result = if use_channel {
        // Use channel flow (legacy or for HTLC)
        let channel_id = args.channel.as_ref().or(selected.channel_id.as_ref());

        match channel_id {
            Some(ch_id) => {
                println!("[CUSTOMER] Using channel flow: {}", ch_id);
                request_via_channel(
                    relay.as_ref(),
                    &keypair,
                    ch_id,
                    &selected.pubkey,
                    &args.prompt,
                    wallet.as_ref(),
                    args.htlc,
                )
                .await
            }
            None => {
                println!("[CUSTOMER] Error: --htlc requires a channel, but provider has none");
                return Ok(());
            }
        }
    } else {
        // Use direct NIP-90 events (primary flow)
        println!("[CUSTOMER] Using direct NIP-90 events (kind:5050 -> 7000 -> 6050)");
        let budget_sats = args.max_price.map(|p| p / 1000).unwrap_or(1000);
        request_via_direct_events(
            relay.as_ref(),
            &keypair,
            &selected,
            &args.prompt,
            wallet.as_ref(),
            budget_sats,
        )
        .await
    };

    match result {
        Ok(_) => println!("[CUSTOMER] Job complete!"),
        Err(e) => println!("[CUSTOMER] Error: {}", e),
    }

    println!("[CUSTOMER] Disconnecting...");
    relay.disconnect().await.ok();

    Ok(())
}
