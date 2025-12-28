# Plan: Two Agents Communicating via NIP-28 + NIP-90 with Spark Payments

## Summary

Create an integration test where two Claude-like agents communicate via NIP-28 public chat on `wss://relay.damus.io`, negotiate a NIP-90 compute job, and exchange real Bitcoin on Spark regtest.

## Current State: EVERYTHING IS ALREADY IMPLEMENTED

Our nostr crate is **production-ready**:

| Component | Status | Location |
|-----------|--------|----------|
| NIP-01 Events | ✅ Complete | `nostr/core/src/nip01.rs` |
| Event signing | ✅ Complete | `finalize_event()`, Schnorr sigs |
| WebSocket client | ✅ Complete | `nostr/client/src/relay.rs` |
| Relay pool | ✅ Complete | `nostr/client/src/pool.rs` |
| Subscriptions | ✅ Complete | `subscribe_with_channel()` |
| NIP-28 Chat | ✅ Complete | `nostr/core/src/nip28.rs` |
| NIP-90 DVM | ✅ Complete | `nostr/core/src/nip90.rs` |
| Spark wallet | ✅ Complete | `spark` crate |

**No new modules needed.** Just wire existing pieces together in a test.

---

## Implementation: Single Test File

**File**: `crates/nostr/client/tests/agent_chat_e2e.rs`

### Test Structure

```rust
//! Agent-to-Agent NIP-28 + NIP-90 E2E Test
//!
//! Two agents communicate via public chat, negotiate a job, exchange Bitcoin.
//! Run: cargo test -p nostr-client --test agent_chat_e2e -- --ignored --nocapture

use nostr::nip01::{finalize_event, EventTemplate};
use nostr::nip06::{derive_keypair, Keypair};
use nostr::nip28::*;
use nostr::nip90::KIND_JOB_TEXT_GENERATION;
use nostr_client::RelayConnection;
use spark::{SparkSigner, SparkWallet, WalletConfig, Network};
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc;
use std::time::Duration;
use std::env::temp_dir;

const RELAY: &str = "wss://relay.damus.io";

// Fixed mnemonics for reproducibility
const PROVIDER_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const CUSTOMER_MNEMONIC: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
```

### Phase 1: Provider Creates Channel

```rust
async fn provider_create_channel(
    relay: &RelayConnection,
    keypair: &Keypair,
) -> Result<String> {
    let metadata = ChannelMetadata::new(
        "OpenAgents Compute Marketplace",
        "Agents negotiate NIP-90 jobs here",
        "",
    ).with_relays(vec![RELAY.to_string()]);

    let template = EventTemplate {
        pubkey: hex::encode(&keypair.public_key),
        created_at: now(),
        kind: KIND_CHANNEL_CREATION,
        tags: vec![],
        content: metadata.to_json()?,
    };

    let event = finalize_event(&template, &keypair.private_key);
    relay.publish_event(&event, Duration::from_secs(5)).await?;

    Ok(event.id.clone())
}
```

### Phase 2: Both Agents Subscribe

```rust
async fn subscribe_to_channel(
    relay: &RelayConnection,
    channel_id: &str,
) -> Result<mpsc::Receiver<Event>> {
    let filters = vec![serde_json::json!({
        "kinds": [KIND_CHANNEL_MESSAGE],
        "#e": [channel_id]
    })];

    relay.subscribe_with_channel("agent-sub", &filters).await
}
```

### Phase 3: Message Protocol

JSON messages in channel content:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum AgentMessage {
    // Provider announces service
    ServiceAnnouncement {
        kind: u16,           // 5050 for text generation
        price_msats: u64,
        spark_address: String,
    },
    // Customer requests job
    JobRequest {
        kind: u16,
        prompt: String,
        max_tokens: u32,
    },
    // Provider sends invoice
    Invoice {
        job_id: String,
        bolt11: String,
        amount_msats: u64,
    },
    // Customer confirms payment
    PaymentSent {
        job_id: String,
        preimage: String,
    },
    // Provider delivers result
    JobResult {
        job_id: String,
        result: String,
    },
}
```

### Phase 4: Provider Loop

```rust
async fn run_provider(
    relay: RelayConnection,
    channel_id: String,
    wallet: SparkWallet,
    keypair: Keypair,
    log_tx: mpsc::Sender<String>,
) -> Result<()> {
    let mut rx = subscribe_to_channel(&relay, &channel_id).await?;

    // Announce service
    let announce = AgentMessage::ServiceAnnouncement {
        kind: KIND_JOB_TEXT_GENERATION,
        price_msats: 10_000,
        spark_address: wallet.get_spark_address().await?,
    };
    send_channel_message(&relay, &channel_id, &keypair, &announce).await?;
    log_tx.send("[PROVIDER] Service announced".into()).await?;

    while let Some(event) = rx.recv().await {
        let msg: AgentMessage = serde_json::from_str(&event.content)?;

        match msg {
            AgentMessage::JobRequest { prompt, .. } => {
                log_tx.send(format!("[PROVIDER] Got job: {}", prompt)).await?;

                // Create invoice
                let invoice = wallet.create_invoice(10, Some("Job"), None).await?;
                let job_id = format!("job_{}", &event.id[..16]);

                let resp = AgentMessage::Invoice {
                    job_id: job_id.clone(),
                    bolt11: invoice.payment_request.clone(),
                    amount_msats: 10_000,
                };
                send_channel_message(&relay, &channel_id, &keypair, &resp).await?;
                log_tx.send("[PROVIDER] Invoice sent".into()).await?;
            }
            AgentMessage::PaymentSent { job_id, preimage } => {
                log_tx.send(format!("[PROVIDER] Payment received: {}", preimage)).await?;

                // Process job (mock)
                let result = AgentMessage::JobResult {
                    job_id,
                    result: "The meaning of life is 42.".into(),
                };
                send_channel_message(&relay, &channel_id, &keypair, &result).await?;
                log_tx.send("[PROVIDER] Result delivered".into()).await?;
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
```

### Phase 5: Customer Loop

```rust
async fn run_customer(
    relay: RelayConnection,
    channel_id: String,
    wallet: SparkWallet,
    keypair: Keypair,
    log_tx: mpsc::Sender<String>,
) -> Result<()> {
    let mut rx = subscribe_to_channel(&relay, &channel_id).await?;

    // Wait for service announcement, then request job
    while let Some(event) = rx.recv().await {
        let msg: AgentMessage = serde_json::from_str(&event.content)?;

        match msg {
            AgentMessage::ServiceAnnouncement { .. } => {
                log_tx.send("[CUSTOMER] Found provider".into()).await?;

                let request = AgentMessage::JobRequest {
                    kind: KIND_JOB_TEXT_GENERATION,
                    prompt: "What is the meaning of life?".into(),
                    max_tokens: 100,
                };
                send_channel_message(&relay, &channel_id, &keypair, &request).await?;
                log_tx.send("[CUSTOMER] Job requested".into()).await?;
            }
            AgentMessage::Invoice { bolt11, job_id, .. } => {
                log_tx.send("[CUSTOMER] Got invoice, paying...".into()).await?;

                let payment = wallet.send_payment_simple(&bolt11, None).await?;

                let confirm = AgentMessage::PaymentSent {
                    job_id,
                    preimage: payment.payment.preimage.unwrap_or_default(),
                };
                send_channel_message(&relay, &channel_id, &keypair, &confirm).await?;
                log_tx.send("[CUSTOMER] Payment sent".into()).await?;
            }
            AgentMessage::JobResult { result, .. } => {
                log_tx.send(format!("[CUSTOMER] Got result: {}", result)).await?;
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
```

### Phase 6: Test Orchestrator

```rust
#[tokio::test]
#[ignore]
async fn test_agent_chat_e2e() {
    let (log_tx, mut log_rx) = mpsc::channel::<String>(100);

    // Derive keypairs
    let provider_kp = derive_keypair(PROVIDER_MNEMONIC).unwrap();
    let customer_kp = derive_keypair(CUSTOMER_MNEMONIC).unwrap();

    // Create wallets
    let provider_wallet = SparkWallet::new(
        SparkSigner::from_mnemonic(PROVIDER_MNEMONIC, "").unwrap(),
        WalletConfig { network: Network::Regtest, api_key: None, storage_dir: temp_dir().join("provider") },
    ).await.unwrap();

    let customer_wallet = SparkWallet::new(
        SparkSigner::from_mnemonic(CUSTOMER_MNEMONIC, "").unwrap(),
        WalletConfig { network: Network::Regtest, api_key: None, storage_dir: temp_dir().join("customer") },
    ).await.unwrap();

    // Check customer has funds
    let balance = customer_wallet.get_balance().await.unwrap();
    if balance.total_sats() < 100 {
        println!("Fund customer wallet first!");
        return;
    }

    // Connect to relay
    let provider_relay = RelayConnection::new(RELAY).unwrap();
    provider_relay.connect().await.unwrap();

    let customer_relay = RelayConnection::new(RELAY).unwrap();
    customer_relay.connect().await.unwrap();

    // Provider creates channel
    let channel_id = provider_create_channel(&provider_relay, &provider_kp).await.unwrap();
    println!("Channel created: {}", channel_id);

    // Spawn agents
    let provider_tx = log_tx.clone();
    let provider_handle = tokio::spawn(run_provider(
        provider_relay, channel_id.clone(), provider_wallet, provider_kp, provider_tx
    ));

    tokio::time::sleep(Duration::from_secs(1)).await; // Let provider announce

    let customer_tx = log_tx.clone();
    let customer_handle = tokio::spawn(run_customer(
        customer_relay, channel_id, customer_wallet, customer_kp, customer_tx
    ));

    // Collect logs
    let mut logs = Vec::new();
    let timeout = tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(log) = log_rx.recv().await {
            println!("{}", log);
            logs.push(log.clone());
            if log.contains("Got result") {
                break;
            }
        }
    });

    timeout.await.ok();

    // Write logs to file
    let log_path = "docs/logs/20251227/agent-chat-results.log";
    std::fs::write(log_path, logs.join("\n")).ok();

    // Cleanup
    provider_handle.abort();
    customer_handle.abort();

    assert!(logs.iter().any(|l| l.contains("Got result")));
}
```

---

## Files to Create/Modify

| File | Action |
|------|--------|
| `crates/nostr/client/tests/agent_chat_e2e.rs` | NEW - main test |
| `crates/nostr/client/Cargo.toml` | Add spark as dev-dependency |
| `docs/logs/20251227/agent-chat-results.log` | Generated by test |

---

## Helper Function

```rust
async fn send_channel_message(
    relay: &RelayConnection,
    channel_id: &str,
    keypair: &Keypair,
    msg: &AgentMessage,
) -> Result<()> {
    let event_template = ChannelMessageEvent::new(
        channel_id,
        RELAY,
        serde_json::to_string(msg)?,
        now(),
    );

    let template = EventTemplate {
        pubkey: hex::encode(&keypair.public_key),
        created_at: now(),
        kind: KIND_CHANNEL_MESSAGE,
        tags: event_template.to_tags(),
        content: serde_json::to_string(msg)?,
    };

    let event = finalize_event(&template, &keypair.private_key);
    relay.publish_event(&event, Duration::from_secs(5)).await?;
    Ok(())
}
```

---

## Run Instructions

```bash
# 1. Fund customer wallet (one-time)
cargo test -p nostr --test agent_chat_e2e test_regtest_wallet_connect -- --ignored --nocapture
# Copy the BTC address, fund via https://app.lightspark.com/regtest-faucet

# 2. Run the E2E test
cargo test -p nostr --test agent_chat_e2e test_agent_chat_e2e -- --ignored --nocapture

# 3. View logs
cat docs/logs/20251227/agent-chat-results.log
```

---

## Success Criteria

- [ ] Both agents connect to relay.damus.io
- [ ] Channel visible in any Nostr client
- [ ] Full message exchange visible in channel
- [ ] Real Bitcoin moves on regtest
- [ ] Logs written to `docs/logs/20251227/`
- [ ] Test completes in <60s
