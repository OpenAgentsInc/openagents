# Plan: Two Separate Computers Running Agents via NIP-28

## Goal

Two **separate Claude Code instances** on **different machines** communicate via NIP-28 public chat, negotiate a NIP-90 job, and exchange real Bitcoin on Spark regtest.

This is NOT a single test with two threads - this is two independent processes that discover each other via Nostr relay.

---

## Architecture

```
┌─────────────────────────┐         ┌─────────────────────────┐
│    Computer A           │         │    Computer B           │
│    (Provider)           │         │    (Customer)           │
│                         │         │                         │
│  Claude Code Instance   │         │  Claude Code Instance   │
│         │               │         │         │               │
│         ▼               │         │         ▼               │
│  cargo run --bin        │         │  cargo run --bin        │
│    agent-provider       │         │    agent-customer       │
│         │               │         │         │               │
│         ▼               │         │         ▼               │
│  Spark Wallet           │         │  Spark Wallet           │
│  (Provider Mnemonic)    │         │  (Customer Mnemonic)    │
└─────────┬───────────────┘         └─────────┬───────────────┘
          │                                   │
          │         wss://relay.damus.io      │
          └──────────────┬────────────────────┘
                         │
                    NIP-28 Channel
                    (Public Chat)
```

---

## Implementation: Two Separate Binaries

### 1. Provider Binary (`src/bin/agent_provider.rs`)

```rust
//! Provider Agent - Run on Computer A
//!
//! Usage: cargo run --bin agent_provider -- --channel <CHANNEL_ID>
//!        cargo run --bin agent_provider -- --create-channel

use clap::Parser;

#[derive(Parser)]
struct Args {
    /// Create a new channel (prints channel ID for customer to use)
    #[arg(long)]
    create_channel: bool,

    /// Join existing channel by ID
    #[arg(long)]
    channel: Option<String>,

    /// Relay URL
    #[arg(long, default_value = "wss://relay.damus.io")]
    relay: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // 1. Initialize provider identity and wallet
    let keypair = derive_keypair(PROVIDER_MNEMONIC)?;
    let wallet = SparkWallet::new(...).await?;

    // 2. Connect to relay
    let relay = RelayConnection::new(&args.relay)?;
    relay.connect().await?;

    // 3. Create or join channel
    let channel_id = if args.create_channel {
        let id = create_channel(&relay, &keypair).await?;
        println!("=== CHANNEL CREATED ===");
        println!("Share this with customer:");
        println!("  cargo run --bin agent_customer -- --channel {}", id);
        println!("========================");
        id
    } else {
        args.channel.expect("Must provide --channel or --create-channel")
    };

    // 4. Subscribe and announce service
    let mut rx = subscribe_to_channel(&relay, &channel_id).await?;
    announce_service(&relay, &channel_id, &keypair, &wallet).await?;

    println!("[PROVIDER] Listening for job requests...");

    // 5. Event loop - wait for jobs
    while let Some(event) = rx.recv().await {
        // Skip own messages
        if event.pubkey == hex::encode(keypair.public_key) {
            continue;
        }

        match parse_message(&event.content) {
            AgentMessage::JobRequest { prompt, .. } => {
                println!("[PROVIDER] Got job: {}", prompt);

                // Create invoice
                let invoice = wallet.create_invoice(10, Some("Job"), None).await?;
                send_invoice(&relay, &channel_id, &keypair, &event.id, &invoice).await?;
                println!("[PROVIDER] Invoice sent, waiting for payment...");
            }
            AgentMessage::PaymentSent { job_id, .. } => {
                println!("[PROVIDER] Payment received!");

                // Process job (call real LLM or mock)
                let result = process_job(&job_id).await?;
                send_result(&relay, &channel_id, &keypair, &job_id, &result).await?;
                println!("[PROVIDER] Result delivered!");
            }
            _ => {}
        }
    }

    Ok(())
}
```

### 2. Customer Binary (`src/bin/agent_customer.rs`)

```rust
//! Customer Agent - Run on Computer B
//!
//! Usage: cargo run --bin agent_customer -- --channel <CHANNEL_ID> --prompt "Your question"

use clap::Parser;

#[derive(Parser)]
struct Args {
    /// Channel ID to join (get from provider)
    #[arg(long)]
    channel: String,

    /// Job prompt
    #[arg(long)]
    prompt: String,

    /// Relay URL
    #[arg(long, default_value = "wss://relay.damus.io")]
    relay: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // 1. Initialize customer identity and wallet
    let keypair = derive_keypair(CUSTOMER_MNEMONIC)?;
    let wallet = SparkWallet::new(...).await?;

    // Check balance
    let balance = wallet.get_balance().await?;
    println!("[CUSTOMER] Wallet balance: {} sats", balance.total_sats());

    // 2. Connect to relay
    let relay = RelayConnection::new(&args.relay)?;
    relay.connect().await?;

    // 3. Subscribe to channel
    let mut rx = subscribe_to_channel(&relay, &args.channel).await?;
    println!("[CUSTOMER] Joined channel: {}", args.channel);

    // 4. Send job request
    send_job_request(&relay, &args.channel, &keypair, &args.prompt).await?;
    println!("[CUSTOMER] Job requested: {}", args.prompt);

    // 5. Wait for invoice, pay, get result
    while let Some(event) = rx.recv().await {
        if event.pubkey == hex::encode(keypair.public_key) {
            continue;
        }

        match parse_message(&event.content) {
            AgentMessage::Invoice { bolt11, job_id, amount_msats } => {
                println!("[CUSTOMER] Got invoice for {} msats", amount_msats);

                // Pay
                let payment = wallet.send_payment_simple(&bolt11, None).await?;
                println!("[CUSTOMER] Paid! ID: {}", payment.payment.id);

                // Confirm
                send_payment_confirmation(&relay, &args.channel, &keypair, &job_id, &payment).await?;
            }
            AgentMessage::JobResult { result, .. } => {
                println!("[CUSTOMER] === RESULT ===");
                println!("{}", result);
                println!("==========================");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
```

---

## Execution Flow

### Step 1: Provider starts on Computer A

```bash
# Computer A (Provider)
cd /path/to/openagents
cargo run --bin agent_provider -- --create-channel
```

Output:
```
[PROVIDER] Connected to wss://relay.damus.io
=== CHANNEL CREATED ===
Share this with customer:
  cargo run --bin agent_customer -- --channel abc123def456...
========================
[PROVIDER] Service announced: kind=5050, price=10000 msats
[PROVIDER] Listening for job requests...
```

### Step 2: Customer joins on Computer B

```bash
# Computer B (Customer)
cd /path/to/openagents
cargo run --bin agent_customer -- \
  --channel abc123def456... \
  --prompt "What is the meaning of life?"
```

Output:
```
[CUSTOMER] Wallet balance: 675 sats
[CUSTOMER] Connected to wss://relay.damus.io
[CUSTOMER] Joined channel: abc123def456...
[CUSTOMER] Job requested: What is the meaning of life?
[CUSTOMER] Got invoice for 10000 msats
[CUSTOMER] Paid! ID: 019b6303-cc36-7482-aeb8-a5b005a44438
[CUSTOMER] === RESULT ===
The meaning of life is 42.
==========================
```

### Step 3: Provider sees the interaction

```
[PROVIDER] Got job: What is the meaning of life?
[PROVIDER] Invoice sent, waiting for payment...
[PROVIDER] Payment received!
[PROVIDER] Result delivered!
```

---

## Logging

Both binaries log to `docs/logs/YYYYMMDD/`:

**Provider log**: `agent-provider-<timestamp>.log`
**Customer log**: `agent-customer-<timestamp>.log`

Format:
```
[2024-12-27T21:35:00Z] [PROVIDER] Connected to relay
[2024-12-27T21:35:01Z] [PROVIDER] Channel created: abc123...
[2024-12-27T21:35:02Z] [PROVIDER] Service announced
[2024-12-27T21:36:15Z] [PROVIDER] Job request from ed6b4c44...
[2024-12-27T21:36:16Z] [PROVIDER] Invoice created: lnbcrt...
[2024-12-27T21:36:20Z] [PROVIDER] Payment confirmed
[2024-12-27T21:36:21Z] [PROVIDER] Result sent
```

---

## Files to Create

| File | Purpose |
|------|---------|
| `src/bin/agent_provider.rs` | Provider binary |
| `src/bin/agent_customer.rs` | Customer binary |
| `src/agents/mod.rs` | Shared agent logic |
| `src/agents/protocol.rs` | Message types (AgentMessage enum) |
| `src/agents/channel.rs` | NIP-28 channel operations |

---

## Shared Message Protocol

Both binaries use the same `AgentMessage` enum:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentMessage {
    ServiceAnnouncement { kind: u16, price_msats: u64, spark_address: String },
    JobRequest { kind: u16, prompt: String, max_tokens: u32 },
    Invoice { job_id: String, bolt11: String, amount_msats: u64 },
    PaymentSent { job_id: String, preimage: String },
    JobResult { job_id: String, result: String },
}
```

---

## Running with Two Claude Instances

### Option A: Two terminals on same machine (simulates two computers)

Terminal 1:
```bash
cargo run --bin agent_provider -- --create-channel
```

Terminal 2:
```bash
cargo run --bin agent_customer -- --channel <ID> --prompt "Hello"
```

### Option B: Actually two different computers

Computer A:
```bash
git clone https://github.com/OpenAgentsInc/openagents
cd openagents
cargo run --bin agent_provider -- --create-channel
# Share the channel ID with Computer B
```

Computer B:
```bash
git clone https://github.com/OpenAgentsInc/openagents
cd openagents
cargo run --bin agent_customer -- --channel <ID> --prompt "Hello"
```

### Option C: Two Claude Code sessions (what user actually wants)

**Session 1** (Provider Claude):
```
User: Run the provider agent and create a channel
Claude: *runs agent_provider --create-channel*
Claude: Channel created: abc123... Share this with the customer.
```

**Session 2** (Customer Claude):
```
User: Join channel abc123 and ask "What is the meaning of life?"
Claude: *runs agent_customer --channel abc123 --prompt "..."*
Claude: Got result: The meaning of life is 42.
```

---

## Implementation Steps

1. Create `src/agents/` module with shared code
2. Create `src/bin/agent_provider.rs`
3. Create `src/bin/agent_customer.rs`
4. Add binaries to `Cargo.toml`
5. Test with two terminals
6. Document usage

---

## Success Criteria

- [ ] Provider can create channel and announce service
- [ ] Customer can discover provider via channel
- [ ] Real Bitcoin payment flows between wallets
- [ ] Both agents log everything
- [ ] Works on actual separate machines
- [ ] Channel visible on public Nostr clients (like Damus)
