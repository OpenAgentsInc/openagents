# Pylon

**Earn Bitcoin by running local AI inference.**

Pylon is a NIP-90 compute provider node that serves local inference jobs over Nostr and earns sats via Lightning payments. Run it on any machine with spare compute:

1. **Llama.cpp with GPT-OSS** — Open-weight models via llama.cpp (Linux, macOS, Windows)
2. **Apple Foundation Models** — On-device Apple Intelligence via `fm-bridge` (macOS + Apple Silicon only)

## What is Pylon?

Pylon turns your computer into an earning node in the OpenAgents compute marketplace. You run `openagents pylon start`, and your machine:

1. Publishes a NIP-89 handler announcement declaring its capabilities
2. Listens for NIP-90 job requests addressed to your provider
3. Responds with `payment-required` and a Lightning invoice
4. Runs inference locally upon payment confirmation
5. Publishes the result as a NIP-90 result event
6. Records earnings in local storage

**The MVP is focused on paid inference, not arbitrary code execution.**

## How It Works

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              PYLON PROVIDER                                   │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │  UnifiedIdentity (BIP39 → Nostr keypair + Lightning wallet)             │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
│  ┌───────────────────────┐    ┌───────────────────────────────────────────┐ │
│  │   Backend Selector    │    │           Job Processor                   │ │
│  │                       │    │                                           │ │
│  │  ┌─────────────────┐  │    │  1. Receive NIP-90 request                │ │
│  │  │  apple_fm       │◄─┼────┤  2. Validate backend availability         │ │
│  │  │  (fm-bridge)    │  │    │  3. Respond payment-required              │ │
│  │  └─────────────────┘  │    │  4. Wait for invoice payment              │ │
│  │                       │    │  5. Execute inference                     │ │
│  │  ┌─────────────────┐  │    │  6. Publish result event                  │ │
│  │  │  llamacpp:      │◄─┼────┤  7. Record earnings                       │ │
│  │  │  gpt_oss_20b    │  │    │                                           │ │
│  │  └─────────────────┘  │    └───────────────────────────────────────────┘ │
│  └───────────────────────┘                                                   │
│                                                                               │
│         ↓                           ↓                           ↓            │
│  ┌─────────────┐           ┌───────────────┐           ┌───────────────┐    │
│  │ Nostr Relays│           │ Local Backends│           │ Lightning     │    │
│  │ (NIP-90)    │           │ (FM/Llama.cpp)│           │ (Spark/LN)    │    │
│  └─────────────┘           └───────────────┘           └───────────────┘    │
└──────────────────────────────────────────────────────────────────────────────┘
```

### NIP-90 Job Flow

```
Buyer                          Nostr Relays                    Pylon Provider
  │                                 │                                │
  │  1. Publish job request         │                                │
  │  (kind 5935, backend=apple_fm)  │                                │
  │────────────────────────────────►│                                │
  │                                 │                                │
  │                                 │  2. Provider sees request      │
  │                                 │◄───────────────────────────────│
  │                                 │                                │
  │                                 │  3. payment-required feedback  │
  │                                 │  (includes Lightning invoice)  │
  │◄────────────────────────────────│◄───────────────────────────────│
  │                                 │                                │
  │  4. Pay Lightning invoice       │                                │
  │─────────────────────────────────┼───────────────────────────────►│
  │                                 │                                │
  │                                 │  5. Run local inference        │
  │                                 │                                │
  │                                 │  6. Publish result event       │
  │                                 │  (kind 6935)                   │
  │◄────────────────────────────────│◄───────────────────────────────│
  │                                 │                                │
```

## Requirements

### Platform

Pylon runs on **any platform** that supports at least one backend:

| Platform | Llama.cpp | Apple FM |
|----------|-----------|----------|
| Linux (x86_64, arm64) | ✅ | ❌ |
| macOS (Intel) | ✅ | ❌ |
| macOS (Apple Silicon) | ✅ | ✅ |
| Windows | ✅ | ❌ |

### Backend Requirements

**For Llama.cpp backend (any platform):**
- llama.cpp installed and accessible
- GPT-OSS weights downloaded (20B model for v1)
- Sufficient RAM/VRAM for the model

**For Apple FM backend (macOS + Apple Silicon only):**
- macOS 15.1+ (Sequoia)
- Apple Silicon (M1/M2/M3/M4)
- Apple Intelligence enabled in System Settings
- On-device model downloaded
- Foundation Models bridge running (see fm-bridge crate)

## Quick Start

### 1. Initialize Pylon

```bash
openagents pylon init
```

This will:
- Generate or import your BIP39 seed phrase
- Derive Nostr keypair (NIP-06 path m/44'/1237'/0'/0/0)
- Derive Lightning wallet signer
- Encrypt and store identity locally

### 2. Start the Provider

```bash
openagents pylon start
```

Pylon will:
- Detect available backends (Apple FM, Llama.cpp)
- Publish NIP-89 handler announcement to configured relays
- Start listening for NIP-90 job requests
- Display earnings dashboard

### 3. Check Status

```bash
openagents pylon status
```

Shows:
- Online/offline status
- Connected relays
- Available backends
- Jobs processed today
- Total sats earned

### 4. Verify Setup

```bash
openagents pylon doctor
```

Runs diagnostic checks:
- Backend availability (Apple FM model loaded? llama.cpp running?)
- Relay connectivity
- Identity configuration
- Payment channel status

## CLI Commands

| Command | Description |
|---------|-------------|
| `pylon init` | Initialize identity and configuration |
| `pylon start` | Start the provider daemon |
| `pylon stop` | Stop the provider daemon |
| `pylon status` | Show current provider status |
| `pylon doctor` | Run diagnostic checks |

## Backend Selection

Buyers specify which backend to use in their job request payload:

```json
{
  "backend": "apple_fm"
}
```

or

```json
{
  "backend": "llamacpp:gpt_oss_20b"
}
```

### Available Backends

| Backend | Identifier | Description |
|---------|------------|-------------|
| Apple FM | `apple_fm` | Apple Foundation Models via fm-bridge |
| GPT-OSS 20B | `llamacpp:gpt_oss_20b` | OpenAI's open-weight 21B model via llama.cpp |
| GPT-OSS 120B | `llamacpp:gpt_oss_120b` | (Future) Larger GPT-OSS model |

### Backend Validation

Pylon validates backend availability at runtime:

- If a buyer requests `apple_fm` but Apple FM isn't available, Pylon responds with a clear error status
- If a buyer requests an unknown backend, Pylon rejects the job
- The NIP-89 handler announcement only advertises backends that are actually available

## Job Kinds

The MVP supports a single job kind for chat completion:

| Kind | Request | Result | Description |
|------|---------|--------|-------------|
| 5935 | Request | — | Chat completion request |
| 6935 | — | Result | Chat completion result |

These are provisional kind numbers. The request includes:
- `backend` — Required backend selector
- `messages` — Chat messages array (OpenAI format)
- `params` — Optional parameters (temperature, max_tokens, etc.)

## Payment Flow

Pylon uses **invoice-gated prepay** to avoid disputes over subjective outputs:

```
1. Receive job request          → Validate inputs and backend
2. Respond payment-required     → Include Lightning invoice (bolt11)
3. Wait for payment             → Monitor invoice status
4. Process job                  → Run inference on selected backend
5. Publish result               → Sign and publish kind 6935 event
6. Record earnings              → Update local database
```

### Pricing

Pricing is configurable per job and per backend:

```rust
PylonConfig {
    // Base price in sats per request
    base_price_sats: 100,

    // Per-token pricing (optional)
    price_per_1k_tokens: 10,

    // Backend-specific multipliers
    backend_multipliers: {
        "apple_fm": 1.0,
        "llamacpp:gpt_oss_20b": 0.8,
    },
}
```

## Local Persistence

Pylon stores data locally in SQLite:

| Table | Description |
|-------|-------------|
| `jobs` | Job history with status, inputs, outputs |
| `invoices` | Lightning invoices and payment status |
| `earnings` | Sats earned per job, per backend |
| `config` | Provider configuration |

Database location: `~/.config/openagents/pylon/pylon.db`

## Configuration

### Default Configuration

```toml
# ~/.config/openagents/pylon/config.toml

[provider]
name = "My Pylon Node"
base_price_sats = 100

[relays]
urls = [
    "wss://relay.damus.io",
    "wss://relay.nostr.band",
    "wss://nos.lol",
]

[backends.apple_fm]
enabled = true
bridge_url = "http://localhost:11435"

[backends.llamacpp]
enabled = true
server_url = "http://localhost:8080"
model = "gpt_oss_20b"
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PYLON_CONFIG_DIR` | Configuration directory | `~/.config/openagents/pylon` |
| `PYLON_FM_BRIDGE_URL` | Apple FM bridge URL | `http://localhost:11435` |
| `PYLON_LLAMA_SERVER_URL` | Llama.cpp server URL | `http://localhost:8080` |

## Architecture

Pylon is a thin application layer on top of the `compute` crate's DVM primitives:

```
┌─────────────────────────────────────────────────────────────┐
│                     pylon (this crate)                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ CLI         │  │ Backends    │  │ Pylon-specific      │  │
│  │ (init/start │  │ - apple_fm  │  │ config & UX         │  │
│  │  stop/etc)  │  │ - llamacpp  │  │                     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                    uses primitives from
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    compute crate                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ DvmService  │  │ RelayService│  │ UnifiedIdentity     │  │
│  │ (NIP-90     │  │ (Nostr      │  │ (BIP39 → keys)      │  │
│  │  job loop)  │  │  relays)    │  │                     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Earnings    │  │ SecureStore │  │ NIP-89 Handler      │  │
│  │ Tracker     │  │             │  │ Info                │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

```
crates/pylon/
├── src/
│   ├── lib.rs                # Library entry point
│   ├── bin/
│   │   └── pylon.rs          # CLI binary
│   ├── config/
│   │   └── mod.rs            # Pylon-specific configuration
│   ├── backends/
│   │   ├── mod.rs            # Backend trait + registry
│   │   ├── apple_fm.rs       # Apple FM backend (via fm-bridge)
│   │   └── llamacpp.rs       # Llama.cpp backend (via gpt-oss)
│   └── cli/
│       └── mod.rs            # CLI commands (init, start, stop, status, doctor)
├── Cargo.toml
└── README.md
```

### Backend Trait

```rust
/// Trait for local inference backends
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// Backend identifier (e.g., "apple_fm", "llamacpp:gpt_oss_20b")
    fn identifier(&self) -> &str;

    /// Check if this backend is available and ready
    async fn is_available(&self) -> bool;

    /// Run inference on the given input
    async fn infer(&self, request: InferenceRequest) -> Result<InferenceResponse>;

    /// Estimate token count for pricing
    fn estimate_tokens(&self, input: &str) -> usize;
}
```

## Testing

### Unit Tests

```bash
cargo test -p pylon
cargo test -p compute
```

### Payment Integration Tests

The compute crate includes real payment integration tests using Lightspark's regtest network.
These tests verify the full paid job flow with actual Bitcoin transactions (no real value).

```bash
# Run all payment integration tests
cargo test -p compute --test payment_integration -- --ignored --nocapture
```

#### Available Tests

| Test | Description |
|------|-------------|
| `test_quick_connectivity` | Verify connection to Lightspark regtest |
| `test_regtest_wallet_connect` | Connect wallet and get deposit address |
| `test_spark_payment_between_wallets` | Real Spark payment between two wallets |
| `test_full_paid_job_e2e` | Complete NIP-90 job with real payment |

#### Example Output

```
=== Full Paid Job E2E Test ===

Customer has 690 sats available
DVM configured to require payment of 10 sats per job

--- Step 1: Customer requests job ---
Event: Job received: job_e2e_ (kind 5050)
Event: Invoice created: 10 sats

--- Step 2: Customer pays invoice ---
Payment sent! ID: 019b62ab-121a-74e0-853b-4cd2d9ff3f0b

--- Step 3: Confirm payment and process ---
Event: Payment received: 10 sats
Event: Job started: job_e2e_ (mock-model)
Event: Job completed: job_e2e_ (10 sats)

--- Job Completed! ---
Result: Response to 'What is the meaning of life?': 42 is the answer
Payment amount: 10000 msats

Provider final balance: 9626 sats

=== E2E Test Complete! ===
```

#### Funding Test Wallets

If test wallets need funding:
1. Run `test_regtest_wallet_connect` to get a Bitcoin deposit address
2. Send regtest sats via the Lightspark faucet: https://app.lightspark.com/regtest-faucet
3. Re-run the payment tests

The tests use fixed mnemonics for reproducibility:
- Provider: `abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about`
- Customer: `zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong`

**Note:** These are TEST mnemonics on regtest. Never use them for real funds.

### Stress Tests & Throughput Benchmarks

The compute crate includes stress tests to measure maximum throughput:

```bash
# Quick benchmark (1000 jobs, ~10 seconds)
cargo test -p compute --test payment_stress quick_throughput_benchmark -- --ignored --nocapture

# Full stress test with concurrency sweep
cargo test -p compute --test payment_stress stress_test_job_throughput_no_payment -- --ignored --nocapture

# Payment throughput (requires funded wallets)
cargo test -p compute --test payment_stress stress_test_payment_throughput -- --ignored --nocapture

# Full E2E with payments
cargo test -p compute --test payment_stress stress_test_full_e2e_throughput -- --ignored --nocapture
```

#### Benchmark Results

| Metric | Value |
|--------|-------|
| **Job Processing (no payment)** | **~15,000 jobs/sec** |
| **Avg job latency** | 76µs |
| **Full E2E (with payment)** | **~0.3 jobs/sec** |
| **E2E latency** | ~3,500ms/job |

#### Concurrency Scaling

| Concurrency | Throughput (jobs/sec) |
|-------------|----------------------|
| 1 worker | 14,783 |
| 10 workers | 14,743 |
| 50 workers | 14,287 |
| 100 workers | 14,805 |
| 500 workers | 14,866 |

Throughput is flat regardless of concurrency because the bottleneck is the RwLock on DvmService.

#### Where Time Goes (E2E with Payment)

| Phase | Time |
|-------|------|
| Invoice creation | ~500ms |
| Payment round-trip | ~2,500ms |
| Job processing | <1ms |
| Confirmation | ~500ms |

Lightning network latency dominates. Optimizations:
- Pre-generated invoice pools
- Parallel payment channels
- Payment batching
- Async confirmation

#### Theoretical Max Throughput

| Scenario | Throughput |
|----------|------------|
| Unpaid jobs | ~15,000/sec |
| Paid jobs (single channel) | ~0.3/sec |
| Paid jobs (10 parallel channels) | ~3/sec |
| Paid jobs (batched invoices) | TBD |

### Mock Testing

Backend trait abstraction enables testing with mock implementations:

```rust
#[cfg(test)]
struct MockBackend {
    response: String,
}

#[async_trait]
impl InferenceBackend for MockBackend {
    fn identifier(&self) -> &str { "mock" }
    async fn is_available(&self) -> bool { true }
    async fn infer(&self, _: InferenceRequest) -> Result<InferenceResponse> {
        Ok(InferenceResponse { content: self.response.clone(), .. })
    }
}
```

## Non-Goals (MVP)

- **No sandbox/test running** — Only inference, no code execution
- **No open bidding/market routing** — Directed jobs only
- **No TEEs/confidential compute** — Trust is invoice-based
- **No GUI/menubar** — CLI-only for MVP

## Roadmap

- [x] Architecture design
- [x] Backend trait abstraction (`InferenceBackend` trait in compute crate)
- [x] Ollama backend
- [x] Apple FM backend (via fm-bridge)
- [x] Llama.cpp backend
- [x] NIP-89 handler announcement
- [x] NIP-90 job request/result loop
- [x] Spark invoice generation
- [x] Payment verification
- [x] Payment integration tests (real Bitcoin on regtest)
- [x] CLI commands (init, start, stop, status, doctor)
- [x] Earnings tracking
- [ ] Local SQLite persistence
- [ ] GPT-OSS 120B support
- [ ] GUI dashboard

## Related Crates

| Crate | Relationship |
|-------|--------------|
| `compute` | **Core dependency** — provides NIP-90 DVM primitives (DvmService, RelayService, UnifiedIdentity, EarningsTracker) |
| `fm-bridge` | Apple FM client — provides on-device inference for Apple Silicon |
| `nostr/core` | NIP-90 and NIP-89 types and events |
| `gpt-oss` | GPT-OSS inference client (for llama.cpp integration) |

## Related Documentation

- [NIP-90: Data Vending Machines](https://github.com/nostr-protocol/nips/blob/master/90.md)
- [NIP-89: Handler Information](https://github.com/nostr-protocol/nips/blob/master/89.md)
- [FM Bridge Setup](../fm-bridge/README.md)
- [Compute Provider](../compute/README.md)
- [SYNTHESIS.md](../../SYNTHESIS.md) — The compute marketplace vision

## License

Apache 2.0
