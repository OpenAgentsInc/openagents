# Plan: Compute Provider Desktop App

A new desktop app (`crates/compute/`) for "Go Online to Sell Compute" via NIP-90 DVMs with Bitcoin payments.

## Summary

- **Purpose**: Let users sell spare compute (Ollama inference) for Bitcoin via Nostr NIP-90 protocol
- **Stack**: wgpui (GPU-accelerated UI) + winit + existing Nostr/Ollama infrastructure
- **Identity**: BIP32 seed phrase shared between Nostr (NIP-06) and Spark wallet
- **Payments**: Breez/Spark wallet for Lightning payments

## Crate Structure

```
crates/compute/
├── Cargo.toml
├── src/
│   ├── bin/compute.rs          # Entry point (ApplicationHandler)
│   ├── lib.rs
│   ├── app.rs                  # ComputeApp main struct
│   ├── state.rs                # Reactive state (Signal<T>)
│   │
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── identity.rs         # UnifiedIdentity (Nostr + Spark from same seed)
│   │   ├── job.rs              # Job, JobStatus types
│   │   ├── earnings.rs         # EarningsTracker
│   │   └── events.rs           # DomainEvent enum
│   │
│   ├── services/
│   │   ├── mod.rs
│   │   ├── dvm_service.rs      # NIP-90 job handler
│   │   ├── ollama_service.rs   # Wrap existing OllamaProvider
│   │   ├── wallet_service.rs   # Spark wallet wrapper
│   │   └── relay_service.rs    # Nostr relay pool
│   │
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── root.rs             # Main layout
│   │   ├── dashboard.rs        # Main dashboard screen
│   │   ├── onboarding.rs       # Seed generation/import
│   │   ├── wallet_panel.rs     # Balance display
│   │   ├── earnings_panel.rs   # Earnings stats
│   │   ├── models_panel.rs     # Ollama models list
│   │   ├── network_panel.rs    # Relay status
│   │   └── job_queue.rs        # Active jobs
│   │
│   └── storage/
│       ├── mod.rs
│       └── secure_store.rs     # Encrypted seed storage
```

## Key Dependencies

```toml
[dependencies]
# Internal
nostr = { path = "../nostr/core" }
nostr_client = { path = "../nostr/client" }
llm = { path = "../llm" }
wgpui = { path = "../wgpui", features = ["desktop"] }
hud = { path = "../hud" }
coder_ui_runtime = { path = "../coder/ui_runtime" }

# External - Spark wallet (git)
spark-wallet = { git = "https://github.com/anthropics/spark-sdk" }

# Core
tokio = { version = "1", features = ["rt-multi-thread", "sync"] }
winit = "0.30"
bip39 = "2.2"
bitcoin = { version = "0.32", features = ["serde"] }
```

## Key Types

### UnifiedIdentity (domain/identity.rs)

```rust
pub struct UnifiedIdentity {
    mnemonic: String,           // BIP39 seed phrase
    nostr_keypair: nip06::Keypair,  // Derived via m/44'/1237'/0'/0/0
    spark_signer: Arc<dyn Signer>,   // Spark wallet signer
}

impl UnifiedIdentity {
    pub fn generate() -> Result<Self>;
    pub fn from_mnemonic(mnemonic: &str, passphrase: &str) -> Result<Self>;
    pub fn npub(&self) -> String;
    pub fn nsec(&self) -> String;
}
```

### AppState (state.rs)

```rust
pub struct AppState {
    pub identity: Signal<Option<UnifiedIdentity>>,
    pub is_online: Signal<bool>,
    pub balance_sats: Signal<u64>,
    pub spark_address: Signal<String>,
    pub earnings: Signal<EarningsTracker>,
    pub available_models: Signal<Vec<OllamaModel>>,
    pub connected_relays: Signal<Vec<String>>,
    pub active_jobs: Signal<Vec<Job>>,
}
```

## UI Layout

```
+----------------------------------------------------------+
|  [COMPUTE]                              [npub1abc...xyz]  |
+----------------------------------------------------------+
|  +-------------+  +------------------------------------+  |
|  | WALLET      |  | EARNINGS                           |  |
|  | ⚡ 21,000   |  | Today:    500 sats  (12 jobs)     |  |
|  | sats        |  | Week:   3,200 sats  (78 jobs)     |  |
|  +-------------+  +------------------------------------+  |
|                                                           |
|  +----------------------------------------------------+  |
|  |  [ GO ONLINE ]  (Toggle)                           |  |
|  +----------------------------------------------------+  |
|                                                           |
|  +----------------------------------------------------+  |
|  | ACTIVE JOBS                                        |  |
|  | Job #abc123  |  text-generation  |  Processing     |  |
|  +----------------------------------------------------+  |
|                                                           |
|  +---------------------+  +---------------------------+  |
|  | MODELS (Ollama)     |  | NETWORK                   |  |
|  | * llama3:8b    [x]  |  | relay.damus.io      [OK]  |  |
|  | * mistral:7b   [ ]  |  | nos.lol             [OK]  |  |
|  +---------------------+  +---------------------------+  |
+----------------------------------------------------------+
```

## HUD Components to Use

From `crates/hud/`:
- `form/toggle.rs` - Go Online switch
- `button/hud_button.rs` - Action buttons
- `frame/corners.rs` - Panel borders
- `indicator/status_light.rs` - Relay status indicators
- `feedback/progress.rs` - Job progress
- `data/card.rs` - Panel containers
- `data/list.rs` - Jobs list
- `indicator/meter.rs` - Earnings visualization

## Data Flow

```
Nostr Relay → DvmService → OllamaService → Result
                 ↓              ↓
            JobFeedback    Inference
                 ↓              ↓
            Publish ←──────────┘
                 ↓
         PaymentReceived → WalletService → EarningsTracker
```

## Implementation Steps

### Phase 1: Foundation
1. Create `crates/compute/Cargo.toml` with dependencies
2. Implement `domain/identity.rs` - UnifiedIdentity with NIP-06 + Spark signer bridge
3. Implement `storage/secure_store.rs` - AES-GCM encrypted mnemonic storage
4. Create `bin/compute.rs` - ApplicationHandler skeleton following coder_app pattern
5. Create `app.rs` with ComputeApp struct

### Phase 2: Services
6. Implement `services/ollama_service.rs` - wrap existing OllamaProvider from `crates/llm/`
7. Implement `services/relay_service.rs` - wrap RelayPool with NIP-90 subscriptions
8. Implement `services/wallet_service.rs` - wrap SparkWallet
9. Implement `services/dvm_service.rs` - NIP-90 job handling loop

### Phase 3: UI
10. Implement `ui/root.rs` - main layout with HUD styling
11. Implement `ui/onboarding.rs` - seed generation/import screen
12. Implement `ui/dashboard.rs` - main screen
13. Implement panels: wallet, earnings, models, network, job_queue

### Phase 4: Integration
14. Wire up reactive state with Signal<T>/Memo<T>
15. Implement domain event processing
16. Add persistence for earnings history
17. Error handling and recovery

## Critical Files to Reference

| File | Purpose |
|------|---------|
| `crates/coder/app/src/main.rs` | ApplicationHandler pattern |
| `crates/nostr/core/src/nip06.rs` | Seed → Nostr keypair |
| `crates/nostr/core/src/nip90.rs` | DVM job types |
| `crates/nostr/client/src/pool.rs` | Relay pool |
| `crates/llm/src/provider/ollama.rs` | Ollama provider |
| `crates/hud/src/form/toggle.rs` | Toggle component |
| `crates/coder/ui_runtime/src/signal.rs` | Reactive state |

## Notes

- **No Maud/HTMX** - that's for web only. Desktop uses wgpui.
- **Spark SDK via git** - `{ git = "..." }` not path dependency
- **Seed phrase** generates both Nostr identity AND Spark wallet
- **NIP-90 kinds**: 5050 (text generation), 6050 (result), 7000 (feedback)
- **Default relays**: wss://relay.damus.io, wss://nos.lol, wss://relay.nostr.band
