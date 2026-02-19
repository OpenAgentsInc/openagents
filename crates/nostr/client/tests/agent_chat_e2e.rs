//! Agent-to-Agent NIP-28 + NIP-90 E2E Test
//!
//! Two agents communicate via NIP-28 public chat, negotiate a NIP-90 job, and exchange Bitcoin.
//!
//! Run: cargo test -p nostr-client --test agent_chat_e2e -- --ignored --nocapture

use nostr::{
    ChannelMessageEvent, ChannelMetadata, Event, EventTemplate, KIND_CHANNEL_CREATION,
    KIND_CHANNEL_MESSAGE, KIND_JOB_TEXT_GENERATION, Keypair, derive_keypair, finalize_event,
};
use nostr_client::RelayConnection;
use openagents_spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use serde::{Deserialize, Serialize};
use std::env::temp_dir;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

const RELAY: &str = "wss://relay.damus.io";

// Fixed mnemonics for reproducibility
const PROVIDER_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const CUSTOMER_MNEMONIC: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

/// Get current unix timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Messages exchanged between agents in the channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum AgentMessage {
    /// Provider announces available service
    ServiceAnnouncement {
        kind: u16,
        price_msats: u64,
        spark_address: String,
    },
    /// Customer requests a job
    JobRequest {
        kind: u16,
        prompt: String,
        max_tokens: u32,
    },
    /// Provider sends invoice
    Invoice {
        job_id: String,
        bolt11: String,
        amount_msats: u64,
    },
    /// Customer confirms payment
    PaymentSent { job_id: String, preimage: String },
    /// Provider delivers result
    JobResult { job_id: String, result: String },
}

/// Result type for test functions
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Create a NIP-28 channel
async fn create_channel(relay: &RelayConnection, keypair: &Keypair) -> Result<String> {
    let metadata = ChannelMetadata::new(
        "OpenAgents Compute Marketplace",
        "Agents negotiate NIP-90 jobs with Bitcoin payments",
        "",
    )
    .with_relays(vec![RELAY.to_string()]);

    let template = EventTemplate {
        created_at: now(),
        kind: KIND_CHANNEL_CREATION,
        tags: vec![],
        content: metadata.to_json()?,
    };

    let event = finalize_event(&template, &keypair.private_key)?;
    let event_id = event.id.clone();

    relay.publish_event(&event, Duration::from_secs(10)).await?;

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

    let channel_msg = ChannelMessageEvent::new(channel_id, RELAY, &msg_json, now());

    let template = EventTemplate {
        created_at: now(),
        kind: KIND_CHANNEL_MESSAGE,
        tags: channel_msg.to_tags(),
        content: msg_json,
    };

    let event = finalize_event(&template, &keypair.private_key)?;
    relay.publish_event(&event, Duration::from_secs(10)).await?;

    Ok(())
}

/// Subscribe to channel messages
async fn subscribe_to_channel(
    relay: &RelayConnection,
    channel_id: &str,
    sub_id: &str,
) -> Result<mpsc::Receiver<Event>> {
    let filters = vec![serde_json::json!({
        "kinds": [KIND_CHANNEL_MESSAGE as u64],
        "#e": [channel_id]
    })];

    let rx = relay.subscribe_with_channel(sub_id, &filters).await?;
    Ok(rx)
}

/// Provider agent loop
async fn run_provider(
    relay: RelayConnection,
    channel_id: String,
    wallet: SparkWallet,
    keypair: Keypair,
    log_tx: mpsc::Sender<String>,
) -> Result<()> {
    // Announce service
    let spark_address = wallet.get_spark_address().await?;
    let announce = AgentMessage::ServiceAnnouncement {
        kind: KIND_JOB_TEXT_GENERATION,
        price_msats: 10_000,
        spark_address,
    };
    send_channel_message(&relay, &channel_id, &keypair, &announce).await?;
    log_tx
        .send("[PROVIDER] Service announced".into())
        .await
        .ok();

    // Subscribe to channel
    let mut rx = subscribe_to_channel(&relay, &channel_id, "provider-sub").await?;

    // Event loop
    while let Some(event) = rx.recv().await {
        // Skip our own messages
        if event.pubkey == hex::encode(keypair.public_key) {
            continue;
        }

        // Parse message
        let msg: AgentMessage = match serde_json::from_str(&event.content) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match msg {
            AgentMessage::JobRequest { prompt, .. } => {
                log_tx
                    .send(format!("[PROVIDER] Got job request: {}", prompt))
                    .await
                    .ok();

                // Create invoice
                let invoice = wallet
                    .create_invoice(10, Some("NIP-90 Job".to_string()), Some(3600))
                    .await?;
                let job_id = format!("job_{}", &event.id[..16]);

                let resp = AgentMessage::Invoice {
                    job_id,
                    bolt11: invoice.payment_request.clone(),
                    amount_msats: 10_000,
                };
                send_channel_message(&relay, &channel_id, &keypair, &resp).await?;
                log_tx.send("[PROVIDER] Invoice sent".into()).await.ok();
            }
            AgentMessage::PaymentSent { job_id, preimage } => {
                log_tx
                    .send(format!(
                        "[PROVIDER] Payment received for {}: {}",
                        job_id, preimage
                    ))
                    .await
                    .ok();

                // Process job (mock response)
                let result = AgentMessage::JobResult {
                    job_id,
                    result: "The meaning of life is 42. This is a mock response from the compute provider.".into(),
                };
                send_channel_message(&relay, &channel_id, &keypair, &result).await?;
                log_tx.send("[PROVIDER] Result delivered".into()).await.ok();

                // Done
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Customer agent loop
async fn run_customer(
    relay: RelayConnection,
    channel_id: String,
    wallet: SparkWallet,
    keypair: Keypair,
    log_tx: mpsc::Sender<String>,
) -> Result<()> {
    // Subscribe to channel
    let mut rx = subscribe_to_channel(&relay, &channel_id, "customer-sub").await?;

    // Event loop
    while let Some(event) = rx.recv().await {
        // Skip our own messages
        if event.pubkey == hex::encode(keypair.public_key) {
            continue;
        }

        // Parse message
        let msg: AgentMessage = match serde_json::from_str(&event.content) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match msg {
            AgentMessage::ServiceAnnouncement {
                kind, price_msats, ..
            } => {
                log_tx
                    .send(format!(
                        "[CUSTOMER] Found provider: kind={}, price={} msats",
                        kind, price_msats
                    ))
                    .await
                    .ok();

                // Request a job
                let request = AgentMessage::JobRequest {
                    kind: KIND_JOB_TEXT_GENERATION,
                    prompt: "What is the meaning of life?".into(),
                    max_tokens: 100,
                };
                send_channel_message(&relay, &channel_id, &keypair, &request).await?;
                log_tx.send("[CUSTOMER] Job requested".into()).await.ok();
            }
            AgentMessage::Invoice {
                bolt11,
                job_id,
                amount_msats,
            } => {
                log_tx
                    .send(format!(
                        "[CUSTOMER] Got invoice for {} msats, paying...",
                        amount_msats
                    ))
                    .await
                    .ok();

                // Pay the invoice
                let payment = wallet.send_payment_simple(&bolt11, None).await?;
                let payment_id = payment.payment.id.clone();

                log_tx
                    .send(format!("[CUSTOMER] Payment sent: {}", payment_id))
                    .await
                    .ok();

                // Confirm payment (use payment ID as proof)
                let confirm = AgentMessage::PaymentSent {
                    job_id,
                    preimage: payment_id,
                };
                send_channel_message(&relay, &channel_id, &keypair, &confirm).await?;
            }
            AgentMessage::JobResult { result, job_id } => {
                log_tx
                    .send(format!("[CUSTOMER] Got result for {}: {}", job_id, result))
                    .await
                    .ok();

                // Done
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Test connecting to relay
#[tokio::test]
#[ignore]
async fn test_relay_connect() {
    let relay = RelayConnection::new(RELAY).expect("should create relay");

    println!("Connecting to {}...", RELAY);
    relay.connect().await.expect("should connect");
    println!("Connected!");

    // Check we're connected
    assert!(relay.is_connected().await);

    relay.disconnect().await.ok();
}

/// Test creating a channel
#[tokio::test]
#[ignore]
async fn test_create_channel() {
    let keypair = derive_keypair(PROVIDER_MNEMONIC).expect("should derive keypair");

    let relay = RelayConnection::new(RELAY).expect("should create relay");
    relay.connect().await.expect("should connect");

    let channel_id = create_channel(&relay, &keypair)
        .await
        .expect("should create channel");

    println!("Channel created: {}", channel_id);

    relay.disconnect().await.ok();
}

/// Test Spark wallet connectivity
#[tokio::test]
#[ignore]
async fn test_spark_wallet_connect() {
    let signer = SparkSigner::from_mnemonic(CUSTOMER_MNEMONIC, "").expect("should create signer");

    let config = WalletConfig {
        network: Network::Regtest,
        api_key: None,
        storage_dir: temp_dir().join("spark_agent_test"),
    };

    println!("Connecting to Spark regtest...");
    let wallet = SparkWallet::new(signer, config)
        .await
        .expect("should connect wallet");

    let balance = wallet.get_balance().await.expect("should get balance");
    println!("Balance: {} sats", balance.total_sats());

    let address = wallet
        .get_bitcoin_address()
        .await
        .expect("should get address");
    println!("BTC address (for faucet): {}", address);

    let spark_address = wallet
        .get_spark_address()
        .await
        .expect("should get spark address");
    println!("Spark address: {}", spark_address);
}

/// Full E2E test: Two agents communicate via NIP-28, negotiate a job, exchange Bitcoin
#[tokio::test]
#[ignore]
async fn test_agent_chat_e2e() {
    println!("=== Agent Chat E2E Test ===\n");

    // Create logging channel
    let (log_tx, mut log_rx) = mpsc::channel::<String>(100);

    // Derive keypairs from mnemonics
    let provider_kp = derive_keypair(PROVIDER_MNEMONIC).expect("should derive provider keypair");
    let customer_kp = derive_keypair(CUSTOMER_MNEMONIC).expect("should derive customer keypair");

    println!("Provider pubkey: {}", hex::encode(provider_kp.public_key));
    println!("Customer pubkey: {}", hex::encode(customer_kp.public_key));

    // Create Spark wallets
    println!("\nConnecting Spark wallets to regtest...");

    let provider_wallet = SparkWallet::new(
        SparkSigner::from_mnemonic(PROVIDER_MNEMONIC, "").unwrap(),
        WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: temp_dir().join("spark_provider_agent"),
        },
    )
    .await
    .expect("should connect provider wallet");

    let customer_wallet = SparkWallet::new(
        SparkSigner::from_mnemonic(CUSTOMER_MNEMONIC, "").unwrap(),
        WalletConfig {
            network: Network::Regtest,
            api_key: None,
            storage_dir: temp_dir().join("spark_customer_agent"),
        },
    )
    .await
    .expect("should connect customer wallet");

    // Check customer has funds
    let balance = customer_wallet
        .get_balance()
        .await
        .expect("should get balance");
    println!("Customer balance: {} sats", balance.total_sats());

    if balance.total_sats() < 100 {
        let address = customer_wallet
            .get_bitcoin_address()
            .await
            .expect("should get address");
        println!("\n!!! Customer wallet needs funds !!!");
        println!("Send regtest sats to: {}", address);
        println!("Faucet: https://app.lightspark.com/regtest-faucet");
        return;
    }

    // Connect to relay
    println!("\nConnecting to relay: {}", RELAY);

    let provider_relay = RelayConnection::new(RELAY).expect("should create provider relay");
    provider_relay
        .connect()
        .await
        .expect("should connect provider relay");

    let customer_relay = RelayConnection::new(RELAY).expect("should create customer relay");
    customer_relay
        .connect()
        .await
        .expect("should connect customer relay");

    println!("Both agents connected to relay");

    // Provider creates channel
    let channel_id = create_channel(&provider_relay, &provider_kp)
        .await
        .expect("should create channel");
    println!("\nChannel created: {}", channel_id);

    // Spawn provider agent
    let provider_tx = log_tx.clone();
    let provider_channel = channel_id.clone();
    let provider_handle = tokio::spawn(async move {
        if let Err(e) = run_provider(
            provider_relay,
            provider_channel,
            provider_wallet,
            provider_kp,
            provider_tx,
        )
        .await
        {
            eprintln!("Provider error: {}", e);
        }
    });

    // Wait for provider to announce
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Spawn customer agent
    let customer_tx = log_tx.clone();
    let customer_channel = channel_id.clone();
    let customer_handle = tokio::spawn(async move {
        if let Err(e) = run_customer(
            customer_relay,
            customer_channel,
            customer_wallet,
            customer_kp,
            customer_tx,
        )
        .await
        {
            eprintln!("Customer error: {}", e);
        }
    });

    // Drop sender so receiver knows when to stop
    drop(log_tx);

    // Collect logs with timeout
    let mut logs = Vec::new();
    let timeout_result = tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(log) = log_rx.recv().await {
            println!("{}", log);
            logs.push(log.clone());
            if log.contains("Got result") {
                break;
            }
        }
    })
    .await;

    if timeout_result.is_err() {
        println!("\n!!! Test timed out !!!");
    }

    // Write logs to file
    let log_dir = std::path::Path::new("docs/logs/20251227");
    if let Err(e) = std::fs::create_dir_all(log_dir) {
        eprintln!("Failed to create log dir: {}", e);
    }
    let log_path = log_dir.join("agent-chat-results.log");
    if let Err(e) = std::fs::write(&log_path, logs.join("\n")) {
        eprintln!("Failed to write logs: {}", e);
    } else {
        println!("\nLogs written to: {}", log_path.display());
    }

    // Cleanup
    provider_handle.abort();
    customer_handle.abort();

    // Verify success
    assert!(
        logs.iter().any(|l| l.contains("Got result")),
        "Should have received job result"
    );

    println!("\n=== E2E Test Complete! ===");
}
