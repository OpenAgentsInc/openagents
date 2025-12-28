# Agent-to-Agent Communication via NIP-28

## Plan: Two Claude Agents Communicate via Public Chat

### Overview

Two Claude Code instances ("Agent A" - the Provider, and "Agent B" - the Customer) will:

1. Connect to a public Nostr relay (wss://relay.damus.io)
2. Create a NIP-28 public chat channel for negotiation
3. Negotiate a NIP-90 compute job with payment
4. Execute the job and exchange regtest Bitcoin via Spark
5. Log the entire interaction to `docs/logs/20251227/`

### Relay Selection

Using `wss://relay.damus.io` because:
- Public, well-known relay
- Supports NIP-28 (public chat kinds 40-44)
- High availability
- No authentication required

### Agent Roles

| Agent | Role | Responsibilities |
|-------|------|-----------------|
| **Agent A** | Provider | Creates channel, offers compute services, creates invoices, processes jobs |
| **Agent B** | Customer | Joins channel, requests jobs, pays invoices, receives results |

### Communication Protocol

#### Phase 1: Channel Setup (Agent A)

Agent A creates the chat channel:

```rust
use nostr::nip28::{ChannelMetadata, ChannelCreateEvent, KIND_CHANNEL_CREATION};

let metadata = ChannelMetadata::new(
    "OpenAgents NIP-90 Marketplace",
    "Agents negotiating compute jobs with Bitcoin payments",
    "",
)
.with_relays(vec!["wss://relay.damus.io".to_string()]);

let channel = ChannelCreateEvent::new(metadata, timestamp);
// Sign and publish kind 40 event
```

#### Phase 2: Service Announcement (Agent A)

Agent A posts available services:

```json
{
  "type": "service_announcement",
  "services": [
    {
      "kind": 5050,
      "name": "Text Generation",
      "models": ["llama-3.2-3b", "qwen-2.5-7b"],
      "price_msats_per_job": 10000,
      "description": "AI text generation via local inference"
    }
  ],
  "spark_address": "<provider_spark_address>"
}
```

#### Phase 3: Job Request (Agent B)

Agent B posts a job request:

```json
{
  "type": "job_request",
  "kind": 5050,
  "input": "What is the meaning of life? Explain in one paragraph.",
  "model": "llama-3.2-3b",
  "max_tokens": 200
}
```

#### Phase 4: Invoice Creation (Agent A)

Agent A responds with invoice:

```json
{
  "type": "invoice",
  "job_id": "job_abc123",
  "amount_msats": 10000,
  "bolt11": "lnbcrt100n1pj...",
  "expires_at": 1735336800
}
```

#### Phase 5: Payment (Agent B)

Agent B pays via Spark and posts confirmation:

```json
{
  "type": "payment_sent",
  "job_id": "job_abc123",
  "payment_id": "pay_xyz789",
  "preimage": "<payment_preimage>"
}
```

#### Phase 6: Job Processing (Agent A)

Agent A processes and posts result:

```json
{
  "type": "job_result",
  "job_id": "job_abc123",
  "status": "completed",
  "result": "The meaning of life is...",
  "processing_time_ms": 1200
}
```

### Implementation Steps

#### Step 1: Create Base Test File

File: `crates/compute/tests/agent_communication.rs`

```rust
//! Agent-to-Agent communication test via NIP-28
//!
//! Run with: cargo test -p compute --test agent_communication -- --ignored --nocapture

use std::sync::Arc;
use tokio::sync::mpsc;
use nostr::nip28::*;
use spark::{SparkSigner, SparkWallet, WalletConfig, Network};

const RELAY_URL: &str = "wss://relay.damus.io";
const CHANNEL_NAME: &str = "OpenAgents Test Channel";

// Provider agent configuration
const PROVIDER_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

// Customer agent configuration
const CUSTOMER_MNEMONIC: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
```

#### Step 2: Provider Agent Loop

```rust
async fn run_provider_agent(
    log_tx: mpsc::Sender<LogEntry>,
) -> Result<()> {
    // 1. Connect to relay
    let client = NostrClient::new(vec![RELAY_URL]).await?;

    // 2. Initialize Spark wallet
    let wallet = SparkWallet::new(
        SparkSigner::from_mnemonic(PROVIDER_MNEMONIC, "")?,
        WalletConfig { network: Network::Regtest, .. }
    ).await?;

    // 3. Create NIP-28 channel
    let channel_id = create_channel(&client).await?;
    log_tx.send(LogEntry::ChannelCreated { channel_id }).await?;

    // 4. Subscribe to channel messages
    let mut messages = client.subscribe_channel(channel_id).await?;

    // 5. Event loop
    while let Some(msg) = messages.recv().await {
        match parse_message(&msg) {
            Message::JobRequest { .. } => {
                // Create invoice
                let invoice = wallet.create_invoice(10, None, None).await?;
                // Post invoice to channel
                send_message(&client, channel_id, invoice_json).await?;
            }
            Message::PaymentSent { preimage, .. } => {
                // Verify payment
                // Process job
                // Post result
            }
            _ => {}
        }
    }
}
```

#### Step 3: Customer Agent Loop

```rust
async fn run_customer_agent(
    channel_id: &str,
    log_tx: mpsc::Sender<LogEntry>,
) -> Result<()> {
    // 1. Connect to relay
    let client = NostrClient::new(vec![RELAY_URL]).await?;

    // 2. Initialize Spark wallet
    let wallet = SparkWallet::new(
        SparkSigner::from_mnemonic(CUSTOMER_MNEMONIC, "")?,
        WalletConfig { network: Network::Regtest, .. }
    ).await?;

    // 3. Subscribe to channel
    let mut messages = client.subscribe_channel(channel_id).await?;

    // 4. Send job request
    send_message(&client, channel_id, job_request_json).await?;

    // 5. Wait for invoice
    while let Some(msg) = messages.recv().await {
        match parse_message(&msg) {
            Message::Invoice { bolt11, .. } => {
                // Pay invoice
                let payment = wallet.send_payment_simple(&bolt11, None).await?;
                // Post payment confirmation
                send_message(&client, channel_id, payment_sent_json).await?;
            }
            Message::JobResult { result, .. } => {
                log_tx.send(LogEntry::JobCompleted { result }).await?;
                break;
            }
            _ => {}
        }
    }
}
```

#### Step 4: Orchestrator Test

```rust
#[tokio::test]
#[ignore]
async fn test_agent_communication_e2e() {
    let (log_tx, mut log_rx) = mpsc::channel(100);

    // Start provider in background
    let provider_tx = log_tx.clone();
    let provider_handle = tokio::spawn(async move {
        run_provider_agent(provider_tx).await
    });

    // Wait for channel creation
    let channel_id = wait_for_channel(&mut log_rx).await;

    // Start customer
    let customer_tx = log_tx.clone();
    let customer_handle = tokio::spawn(async move {
        run_customer_agent(&channel_id, customer_tx).await
    });

    // Collect logs
    let mut logs = Vec::new();
    while let Some(entry) = log_rx.recv().await {
        logs.push(entry);
        if matches!(entry, LogEntry::JobCompleted { .. }) {
            break;
        }
    }

    // Write logs to file
    write_logs_to_file(&logs, "docs/logs/20251227/agent-comm-results.log").await?;

    // Cleanup
    provider_handle.abort();
    customer_handle.abort();
}
```

### Logging Format

Each agent logs to `docs/logs/20251227/`:

```
[2024-12-27T21:15:00Z] [PROVIDER] Channel created: abc123
[2024-12-27T21:15:01Z] [PROVIDER] Service announcement posted
[2024-12-27T21:15:02Z] [CUSTOMER] Joined channel: abc123
[2024-12-27T21:15:03Z] [CUSTOMER] Job request posted: "What is the meaning of life?"
[2024-12-27T21:15:04Z] [PROVIDER] Received job request from <pubkey>
[2024-12-27T21:15:05Z] [PROVIDER] Invoice created: 10000 msats
[2024-12-27T21:15:06Z] [CUSTOMER] Received invoice: 10000 msats
[2024-12-27T21:15:08Z] [CUSTOMER] Payment sent: pay_xyz789
[2024-12-27T21:15:09Z] [PROVIDER] Payment received: 10 sats
[2024-12-27T21:15:10Z] [PROVIDER] Processing job...
[2024-12-27T21:15:11Z] [PROVIDER] Job completed: "The meaning of life is..."
[2024-12-27T21:15:12Z] [CUSTOMER] Received result
```

### What We Can Already Do (NIP-28 Capabilities)

Our `crates/nostr/core/src/nip28.rs` provides:

| Feature | Implementation |
|---------|---------------|
| Create channel (kind 40) | `ChannelCreateEvent::new(metadata, timestamp)` |
| Update metadata (kind 41) | `ChannelMetadataEvent::new(channel_id, metadata, timestamp)` |
| Post messages (kind 42) | `ChannelMessageEvent::new(channel_id, relay, content, timestamp)` |
| Reply to messages | `ChannelMessageEvent::reply(channel_id, reply_to_id, relay, content, timestamp)` |
| Mention users | `.mention_pubkey(pubkey, relay)` |
| Hide messages (kind 43) | `ChannelHideMessageEvent::new(message_id, timestamp).with_reason(reason)` |
| Mute users (kind 44) | `ChannelMuteUserEvent::new(pubkey, timestamp).with_reason(reason)` |
| Category tags | `.with_categories(vec!["bitcoin", "ai"])` |

### Missing Pieces

1. **WebSocket client for relay connection** - Need to add to `nostr/client`
2. **Event signing** - Need to integrate with `UnifiedIdentity`
3. **Subscription management** - Filter for channel events
4. **Message parsing** - JSON schema for job negotiation messages

### Execution Plan

1. Extend `nostr/client` with WebSocket support for NIP-28
2. Create `AgentProtocol` trait for structured messages
3. Implement `ProviderAgent` and `CustomerAgent`
4. Create test harness that spawns both
5. Run on regtest with funded wallets
6. Log everything to `docs/logs/20251227/`

### Success Criteria

- [ ] Both agents connect to relay.damus.io
- [ ] Channel is created and visible
- [ ] Job request/invoice exchange works
- [ ] Real Bitcoin moves via Spark regtest
- [ ] Logs capture entire flow
- [ ] Test completes in <60 seconds

### Notes

- Using regtest network - no real money at risk
- Agents share logs via filesystem (same machine)
- Can be extended to separate machines via relay
- NIP-90 events could also be posted to channel for discoverability
