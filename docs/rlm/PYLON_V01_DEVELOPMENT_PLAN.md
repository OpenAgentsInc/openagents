# Pylon v0.1 "Testnet Alpha" Development Plan

**Target:** GitHub Release tomorrow
**Goal:** Prove the market loop works end-to-end with testnet Bitcoin payments

---

## Executive Summary

Ship a single `pylon` CLI binary that can run as either:
1. **Provider mode** (`pylon node up`) - Serve NIP-90 jobs, earn testnet sats
2. **Buyer mode** (`pylon job submit/status/results`) - Submit jobs, pay for results

All payments via Spark (Breez SDK) on **testnet/regtest**.

---

## What Already Exists

| Component | Status | Location |
|-----------|--------|----------|
| Spark wallet core | ✅ Complete | `crates/spark/src/wallet.rs` |
| NIP-90 protocol | ✅ Complete | `crates/nostr/core/src/nip90.rs` |
| RelayPool | ✅ Complete | `crates/nostr/client/src/pool.rs` |
| DvmClient | ✅ Complete | `crates/nostr/client/src/dvm.rs` |
| Pylon CLI skeleton | ✅ Exists | `crates/pylon/src/cli/mod.rs` |
| Provider runtimes | ✅ Exists | `crates/pylon-desktop/src/{nostr,fm,wallet}_runtime.rs` |
| UnifiedIdentity | ✅ Complete | `crates/compute/src/domain/identity.rs` |
| Backend registry | ✅ Complete | `crates/compute/src/infrastructure/backends/` |

---

## MVP Scope (Tonight)

### 1. Provider Node (`pylon node up`)

**What it does:**
- Connects to relay (default: `wss://relay.openagents.com`)
- Announces capabilities via NIP-89 (kind 31990)
- Subscribes to job requests (kind 5050 for text generation)
- Executes jobs via local backend (Apple FM, Ollama, or Llama.cpp)
- Publishes results (kind 6050) with payment invoice
- Tracks earnings

**Implementation:**
- Mostly exists in `crates/pylon-desktop/src/cli.rs`
- Wire up WalletRuntime to create invoices on job completion
- Add `--testnet` flag to force regtest network

**Files to modify:**
```
crates/pylon/src/cli/mod.rs          # Add 'node' subcommand group
crates/pylon/src/cli/node.rs         # NEW: node up/down/status
crates/pylon-desktop/src/cli.rs      # Refactor to be callable as library
crates/pylon-desktop/src/wallet_runtime.rs  # Wire invoice creation
```

### 2. Buyer CLI (`pylon job`)

**Commands:**
```bash
pylon job submit <type> --input <file|text> [--bid <sats>]
pylon job status <job_id>
pylon job results <job_id>
pylon job list [--limit N]
```

**Implementation:**
- Use existing `DvmClient` from `crates/nostr/client/src/dvm.rs`
- Store pending jobs in SQLite (`~/.pylon/jobs.db`)
- Subscribe to result/feedback events

**Files to create:**
```
crates/pylon/src/cli/job.rs          # NEW: job submit/status/results/list
crates/pylon/src/jobs/mod.rs         # NEW: job tracking database
crates/pylon/src/jobs/store.rs       # NEW: SQLite storage for jobs
```

### 3. Wallet CLI (`pylon wallet`)

**Commands:**
```bash
pylon wallet init                     # Generate/import mnemonic
pylon wallet whoami                   # Show pubkeys (Nostr + Bitcoin)
pylon wallet balance                  # Show Spark balance
pylon wallet send <invoice>           # Pay bolt11 invoice
pylon wallet receive [amount]         # Create invoice
pylon wallet history [--limit N]      # Payment history
```

**Implementation:**
- Wire existing `SparkWallet` to CLI
- Store encrypted mnemonic in `~/.pylon/identity.enc`
- Use `WalletConfig { network: Network::Testnet }` for alpha

**Files to create:**
```
crates/pylon/src/cli/wallet.rs       # NEW: wallet commands
crates/pylon/src/identity.rs         # NEW: encrypted identity storage
```

### 4. Stats Dashboard (`pylon stats`)

**Commands:**
```bash
pylon stats                           # Live dashboard (TUI)
pylon stats --json                    # JSON output for scripts
```

**Metrics:**
- Online nodes (from relay subscription)
- Jobs/minute (rolling window)
- Success rate
- p50/p95 latency
- Top providers by completed jobs
- Personal earnings (if provider)

**Implementation:**
- Subscribe to NIP-90 events on relay
- Aggregate in-memory with sliding windows
- Use `ratatui` for TUI or simple println for MVP

**Files to create:**
```
crates/pylon/src/cli/stats.rs        # NEW: stats command
crates/pylon/src/stats/mod.rs        # NEW: metrics aggregation
crates/pylon/src/stats/collector.rs  # NEW: event collection
```

### 5. Payment Integration

**Flow:**
```
1. Provider completes job
2. Provider creates Spark invoice: wallet.create_invoice(cost_sats, job_id)
3. Provider publishes JobResult with bolt11 in ["amount", "1000", "lnbc..."] tag
4. Buyer receives result, extracts bolt11
5. Buyer pays: wallet.send_payment_simple(bolt11)
6. Provider sees payment arrive (Spark event)
7. Both sides log receipt
```

**Files to modify:**
```
crates/pylon-desktop/src/nostr_runtime.rs  # Include bolt11 in result
crates/pylon/src/cli/job.rs                # Auto-pay option
```

---

## CLI Command Tree (Final)

```
pylon
├── init                    # Initialize identity (mnemonic + network)
├── node
│   ├── up                  # Start provider daemon
│   ├── down                # Stop provider daemon
│   └── status              # Show provider status
├── job
│   ├── submit              # Submit job request
│   ├── status              # Check job status
│   ├── results             # Get job results
│   └── list                # List submitted jobs
├── wallet
│   ├── init                # (alias for pylon init)
│   ├── whoami              # Show identities
│   ├── balance             # Show balance
│   ├── send                # Pay invoice
│   ├── receive             # Create invoice
│   └── history             # Payment history
├── stats                   # Live metrics dashboard
└── doctor                  # Diagnostics (existing)
```

---

## Data Storage Layout

```
~/.pylon/
├── identity.enc            # Encrypted BIP39 mnemonic
├── config.toml             # Network, relay URLs, preferences
├── jobs.db                 # SQLite: submitted jobs, statuses
├── receipts.db             # SQLite: payment receipts
├── spark/                  # Breez SDK storage
│   └── regtest/            # Testnet wallet data
└── logs/
    └── pylon.log           # Rotating logs
```

---

## Implementation Order (Tonight)

### Phase 1: Identity & Wallet (1-2 hours)
1. `pylon init` - Generate UnifiedIdentity, save encrypted
2. `pylon wallet whoami` - Display Nostr pubkey + Bitcoin pubkey
3. `pylon wallet balance` - Query Spark balance
4. `pylon wallet receive` - Create invoice
5. `pylon wallet send` - Pay invoice

**Test:** Create wallet, fund from faucet, send between two wallets

### Phase 2: Provider Node (2-3 hours)
1. Refactor pylon-desktop CLI mode to library
2. `pylon node up` - Start provider with NostrRuntime + FmRuntime
3. Wire WalletRuntime to create invoices on job completion
4. `pylon node status` - Show connection state, jobs served

**Test:** Start node, submit job via raw nostr event, see result

### Phase 3: Buyer CLI (2-3 hours)
1. `pylon job submit` - Create JobRequest, publish, subscribe
2. `pylon job status` - Query pending job state
3. `pylon job results` - Fetch and display result
4. `pylon job list` - Show all jobs from local DB

**Test:** Submit job to own provider, receive result, pay invoice

### Phase 4: Stats & Polish (1-2 hours)
1. `pylon stats` - Basic metrics display
2. `pylon doctor` - Verify all components working
3. Error handling, help text, version info

**Test:** Run demo script showing full flow

---

## Hero Demo Script

```bash
#!/bin/bash
# demo.sh - Pylon v0.1 Testnet Alpha Demo

# Terminal 1: Start provider
pylon init --testnet
pylon wallet balance  # Should be 0
# Fund from faucet...
pylon node up --relay wss://relay.openagents.com

# Terminal 2: Submit job as buyer
pylon init --testnet  # Different identity
# Fund from faucet...
pylon job submit text-generation \
  --input "Summarize the Bitcoin whitepaper in 3 sentences" \
  --bid 1000

# Watch job complete
pylon job status <job_id>
pylon job results <job_id>

# Check payments
pylon wallet history
```

---

## Testnet Bitcoin Setup

**Network:** Regtest (via Breez SDK)

**Funding Options:**
1. **Breez Regtest Faucet** - Automated in integration tests
2. **Manual funding** - Use `pylon wallet receive` + send from another regtest wallet

**Config:**
```rust
WalletConfig {
    network: Network::Testnet,  // Maps to Regtest internally
    api_key: None,              // Not required for testnet
    storage_dir: PathBuf::from("~/.pylon/spark/regtest"),
}
```

---

## Success Criteria (Tomorrow)

| Metric | Target |
|--------|--------|
| Providers online | ≥20 distinct |
| Jobs completed | ≥200 total |
| Median latency | <30s |
| Success rate | ≥95% |
| Payment flow | Works end-to-end |

---

## Files to Create/Modify Summary

### New Files
```
crates/pylon/src/cli/node.rs         # Provider node commands
crates/pylon/src/cli/job.rs          # Buyer job commands
crates/pylon/src/cli/wallet.rs       # Wallet commands
crates/pylon/src/cli/stats.rs        # Stats dashboard
crates/pylon/src/identity.rs         # Encrypted identity storage
crates/pylon/src/jobs/mod.rs         # Job tracking module
crates/pylon/src/jobs/store.rs       # SQLite job storage
crates/pylon/src/stats/mod.rs        # Stats module
crates/pylon/src/stats/collector.rs  # Event collection
```

### Modified Files
```
crates/pylon/src/cli/mod.rs          # Add new subcommands
crates/pylon/src/lib.rs              # Export new modules
crates/pylon/Cargo.toml              # Add dependencies (rusqlite, dirs)
crates/pylon-desktop/src/cli.rs      # Refactor for library use
crates/pylon-desktop/src/wallet_runtime.rs  # Invoice on job complete
crates/pylon-desktop/src/nostr_runtime.rs   # Include bolt11 in results
```

### Dependencies to Add
```toml
# crates/pylon/Cargo.toml
rusqlite = { version = "0.31", features = ["bundled"] }
dirs = "5.0"
ratatui = "0.26"  # Optional, for TUI stats
chacha20poly1305 = "0.10"  # For identity encryption
```

---

## Relay Infrastructure

**Primary relay:** `wss://relay.openagents.com`

**NIP-90 subscription filters:**
```json
// Provider subscribes to job requests
{ "kinds": [5050], "since": <now> }

// Buyer subscribes to results for their jobs
{ "kinds": [6050, 7000], "#e": ["<job_event_id>"] }

// Stats subscribes to all DVM traffic
{ "kinds": [5050, 6050, 7000], "since": <now - 1h> }
```

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breez SDK testnet issues | Fall back to mock payments for demo |
| Relay connectivity | Bundle local relay as fallback |
| Backend unavailable | Graceful degradation, clear error messages |
| Identity loss | Mnemonic backup prompt on init |

---

## Post-MVP (v0.2+)

- [ ] Multi-job-type marketplace
- [ ] Provider reputation system
- [ ] Mainnet payments (with API key)
- [ ] Multi-relay routing
- [ ] Job verification tiers
- [ ] Async sub-call fanout (FRLM)
- [ ] Web dashboard

---

## Appendix: Key Code References

### Spark Wallet Usage
```rust
// crates/spark/src/wallet.rs
let signer = SparkSigner::from_mnemonic(mnemonic, "")?;
let config = WalletConfig {
    network: Network::Testnet,
    ..Default::default()
};
let wallet = SparkWallet::new(signer, config).await?;

// Create invoice for job payment
let invoice = wallet.create_invoice(1000, Some(job_id.clone()), Some(3600)).await?;

// Pay invoice
let result = wallet.send_payment_simple(&bolt11, None).await?;
```

### NIP-90 Job Submission
```rust
// crates/nostr/client/src/dvm.rs
let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
    .add_input(JobInput::text("Summarize this..."))
    .with_bid(1000)
    .add_relay("wss://relay.openagents.com");

let submission = client.submit_job(&request, &relays).await?;
let result = client.await_result(&submission.event_id, Duration::from_secs(60)).await?;
```

### UnifiedIdentity
```rust
// crates/compute/src/domain/identity.rs
let identity = UnifiedIdentity::from_mnemonic(mnemonic, "")?;
let nostr_pubkey = identity.nostr_keypair().public_key();
let bitcoin_pubkey = identity.spark_public_key_hex();
```
