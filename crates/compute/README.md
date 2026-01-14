# Compute Provider

Sell compute power via **NIP-90 Data Vending Machines (DVMs)** with Bitcoin Lightning payments.

The compute crate implements a NIP-90 DVM provider that allows users to monetize their compute resources by processing AI inference jobs from the Nostr network and receiving Bitcoin payments.

## What is NIP-90?

[NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md) defines **Data Vending Machines (DVMs)** - a protocol for requesting and providing computational services over Nostr.

### How it Works

```
1. Customer publishes job request (kind 5000-5999) on Nostr
2. DVM providers see the request and bid on the job
3. Customer accepts a bid and pays the invoice
4. Provider processes the job (e.g., AI inference)
5. Provider publishes the result (kind 6000-6999)
6. Customer receives the result
```

### Supported Job Kinds

**Inference Jobs** (simple request/response):
- **5050**: Text generation (AI inference via Ollama, llama.cpp, Apple FM)

**Bazaar Jobs** (agentic, multi-step tasks):
- **5930**: SandboxRun - Execute code in isolated sandbox
- **5931**: RepoIndex - Index and analyze repositories
- **5932**: PatchGen - Generate patches from issues/requirements
- **5933**: CodeReview - Review code with structured feedback

Bazaar jobs require an **agent backend** (e.g., Codex) and support pay-after-verify semantics.

## Architecture

```
┌───────────────────────────────────────────────────────────────┐
│   Compute Provider (this crate)                               │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  UnifiedIdentity                                         │ │
│  │  - BIP39 seed phrase (12/24 words)                       │ │
│  │  - Nostr keypair (NIP-06)                                │ │
│  │  - Spark wallet signer                                   │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  DvmService                                              │ │
│  │  - Listens for job requests (5050, 5930-5933)            │ │
│  │  - Routes to inference or agent backends                 │ │
│  │  - Publishes results to Nostr                            │ │
│  │  - Tracks earnings                                       │ │
│  └─────────────────────────────────────────────────────────┘ │
│           │                    │                              │
│           ▼                    ▼                              │
│  ┌─────────────────┐  ┌─────────────────────────────────┐   │
│  │ BackendRegistry │  │ AgentRegistry                    │   │
│  │ (Inference)     │  │ (Bazaar Jobs)                    │   │
│  │ - Ollama        │  │ - CodexCodeBackend              │   │
│  │ - llama.cpp     │  │   └── PatchGen, CodeReview       │   │
│  │ - Apple FM      │  │   └── SandboxRun, RepoIndex      │   │
│  └─────────────────┘  └─────────────────────────────────┘   │
│           │                    │                              │
│  ┌─────────────────┐  ┌─────────────────────────────────┐   │
│  │ RelayService    │  │ Sandbox (container/gvisor)      │   │
│  │ (Nostr)         │  │                                  │   │
│  └─────────────────┘  └─────────────────────────────────┘   │
└───────────────────────────────────────────────────────────────┘
```

### Key Components

1. **UnifiedIdentity** (`domain/identity.rs`):
   - Manages BIP39 seed phrase (12 or 24 words)
   - Derives Nostr keypair via NIP-06 (m/44'/1237'/0'/0/0)
   - Derives Spark wallet signer for payments

2. **DvmService** (`services/dvm_service.rs`):
   - Subscribes to NIP-90 job requests on Nostr relays
   - Routes inference jobs (5050) to BackendRegistry
   - Routes Bazaar jobs (5930-5933) to AgentRegistry
   - Publishes NIP-89 handler info (announces capabilities)
   - Publishes job results to Nostr

3. **BackendRegistry** (`backends/registry.rs`):
   - Auto-detects inference backends (Ollama, llama.cpp, Apple FM)
   - Routes inference requests to appropriate backend

4. **AgentRegistry** (`backends/agent.rs`):
   - Manages agent backends for complex, multi-step tasks
   - Routes Bazaar jobs to appropriate agent (e.g., Codex)

5. **CodexCodeBackend** (`backends/codex.rs`):
   - Executes Codex in sandboxed environment
   - Supports PatchGen, CodeReview, SandboxRun jobs
   - Isolation modes: local, container, gvisor

6. **SecureStore** (`storage/secure_store.rs`):
   - Encrypts seed phrase using AES-256-GCM
   - Derives encryption key from password using Argon2
   - Stores encrypted data in `~/.config/openagents/compute/identity.enc`

## Quick Start

### Prerequisites

1. **Ollama**: Install and run Ollama for AI inference
   ```bash
   # Install Ollama
   curl -fsSL https://ollama.com/install.sh | sh

   # Pull a model (e.g., llama3.2)
   ollama pull llama3.2

   # Start Ollama server
   ollama serve
   ```

2. **Nostr Relays**: The provider needs access to Nostr relays to receive job requests. Default relays are configured in the code.

### Building

```bash
cd crates/compute
cargo build --release --features ui
```

### Running

```bash
# Run with UI (requires --features ui)
cargo run --release --features ui

# Run library only (no UI)
cargo run --release
```

### First Time Setup

When you first run the compute provider:

1. **Create or Import Identity**:
   - Generate a new 12-word seed phrase, OR
   - Import an existing BIP39 mnemonic

2. **Set Password**:
   - Choose a strong password to encrypt your seed phrase
   - This password is required every time you start the provider

3. **Verify Identity**:
   - Your Nostr public key (npub) will be displayed
   - Save your seed phrase in a safe place (it cannot be recovered!)

4. **Go Online**:
   - Start the DVM service to begin receiving job requests
   - The provider will publish a NIP-89 handler info event

## Configuration

### DvmConfig

Configure the DVM service:

```rust
pub struct DvmConfig {
    /// Minimum price in millisats per job
    pub min_price_msats: u64,
    /// Default model to use for inference
    pub default_model: String,
    /// Whether to require payment before processing
    pub require_payment: bool,
}
```

Default configuration:

```rust
DvmConfig {
    min_price_msats: 1000,        // 1 sat minimum
    default_model: "llama3.2".to_string(),
    require_payment: false,       // Testing mode
}
```

### Relay Configuration

Edit `services/relay_service.rs` to configure Nostr relays:

```rust
const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://relay.nostr.band",
    // Add more relays
];
```

## Identity Management

### Generate New Identity

```rust
use compute::domain::UnifiedIdentity;

// Generate 12-word seed
let identity = UnifiedIdentity::generate()?;

// Generate 24-word seed (more secure)
let identity = UnifiedIdentity::generate_24_words()?;

// Get the mnemonic
let mnemonic = identity.mnemonic();
println!("Seed phrase: {}", mnemonic);

// Get Nostr public key
let npub = identity.nostr_npub();
println!("Nostr public key: {}", npub);
```

### Import Existing Identity

```rust
// From mnemonic string
let mnemonic = "word1 word2 word3 ... word12";
let identity = UnifiedIdentity::from_mnemonic(mnemonic, "")?;

// With BIP39 passphrase (advanced)
let identity = UnifiedIdentity::from_mnemonic(mnemonic, "my-passphrase")?;
```

### Secure Storage

```rust
use compute::storage::SecureStore;

// Save encrypted identity
let store = SecureStore::new(SecureStore::default_path());
store.save(&mnemonic, "my-password").await?;

// Load encrypted identity
let recovered = store.load("my-password").await?;
assert_eq!(recovered, mnemonic);
```

Storage location: `~/.config/openagents/compute/identity.enc`

## Job Processing

### Processing Flow

1. **Receive Job Request**:
   ```rust
   // Job request event arrives from Nostr relay
   let job = Job {
       id: "job-123",
       kind: 5050,  // Text generation
       customer_pubkey: "npub1...",
       inputs: vec![JobInput { data: "Hello, AI!", ... }],
       params: { "model" => "llama3.2" },
       status: JobStatus::Pending,
       // ...
   };
   ```

2. **Process Job**:
   ```rust
   // DvmService processes the job
   let result = ollama_service.generate(&prompt, &model).await?;
   ```

3. **Publish Result**:
   ```rust
   // Create result event (kind 6000-6999)
   let result_event = EventTemplate {
       kind: 6050,  // Text generation result
       content: result,
       tags: vec![
           ["e", &job.request_event_id],
           ["p", &job.customer_pubkey],
           ["request", &job.request_event_id],
       ],
       // ...
   };
   ```

### Job Status States

```rust
pub enum JobStatus {
    Pending,           // Received, waiting to process
    Processing,        // Currently running
    AwaitingPayment,   // Waiting for Lightning invoice to be paid
    Completed,         // Successfully completed
    Failed(String),    // Failed with error message
}
```

## NIP-89 Handler Info

The provider publishes a NIP-89 handler info event to announce its capabilities:

```rust
pub fn publish_handler_info(&self) -> Result<()> {
    let handler_info = HandlerInfo {
        handler_type: HandlerType::Dvm,
        kinds: vec![5050],  // Supported job kinds
        metadata: HandlerMetadata {
            name: "OpenAgents Compute Provider".to_string(),
            about: "AI inference via Ollama".to_string(),
            picture: None,
            website: Some("https://openagents.com".to_string()),
        },
        pricing: vec![
            PricingInfo {
                kind: 5050,
                amount_msats: 1000,
                per_unit: Some("request".to_string()),
            }
        ],
    };

    // Sign and publish to relays
    let event = finalize_event(handler_info, &keypair)?;
    relay_service.publish(event).await?;
}
```

## Earnings Tracking

Track your earnings from completed jobs:

```rust
use compute::domain::EarningsTracker;

let tracker = EarningsTracker::new();

// Record a completed job
tracker.record_job(1000, "job-123").await;

// Get statistics
let stats = tracker.get_stats().await;
println!("Total earned: {} msats", stats.total_msats);
println!("Jobs completed: {}", stats.job_count);
println!("Average per job: {} msats", stats.average_msats);
```

## Integration with Other Crates

### Nostr Core

The compute crate depends on `nostr/core` for:
- NIP-06 key derivation
- NIP-90 job types and events
- NIP-89 handler info
- Event signing and verification

```rust
use nostr::{
    derive_keypair_full,
    EventTemplate,
    JobInput,
    HandlerInfo,
};
```

### Marketplace (Future)

The compute provider will integrate with the marketplace crate to:
- List available compute services
- Track reputation and reviews
- Handle escrow payments
- Manage provider profiles

## Development

### Running Tests

```bash
cargo test
```

Run only the end-to-end DVM job execution flow test (mock relay + mock backend):

```bash
cargo test -p compute dvm_executes_job_from_relay_and_publishes_result
```

### Logging

Enable debug logging:

```bash
RUST_LOG=compute=debug cargo run --features ui
```

Available log levels:
- `error`: Errors only
- `warn`: Warnings and errors
- `info`: General info (default)
- `debug`: Detailed debug info
- `trace`: Very detailed trace info

### Project Structure

```
crates/compute/
├── src/
│   ├── lib.rs                    # Library entry point
│   ├── app.rs                    # Application state
│   ├── state.rs                  # Shared state
│   ├── bin/
│   │   └── compute.rs            # Binary entry point (with UI)
│   ├── domain/
│   │   ├── mod.rs                # Domain exports
│   │   ├── identity.rs           # UnifiedIdentity
│   │   ├── job.rs                # Job types
│   │   ├── earnings.rs           # EarningsTracker
│   │   └── events.rs             # Domain events
│   ├── services/
│   │   ├── mod.rs                # Service exports
│   │   ├── dvm_service.rs        # DVM service
│   │   ├── relay_service.rs      # Nostr relay client
│   │   └── ollama_service.rs     # Ollama client
│   ├── storage/
│   │   ├── mod.rs                # Storage exports
│   │   └── secure_store.rs       # Encrypted storage
│   └── ui/                       # UI components (feature-gated)
│       ├── mod.rs
│       ├── root.rs
│       ├── dashboard.rs
│       ├── wallet_panel.rs
│       ├── job_queue.rs
│       ├── earnings_panel.rs
│       ├── network_panel.rs
│       ├── models_panel.rs
│       └── backup.rs
├── Cargo.toml
└── README.md
```

## Security Considerations

### Seed Phrase Security

- **NEVER** share your seed phrase with anyone
- **NEVER** commit seed phrases to version control
- **ALWAYS** use a strong password for encryption
- **BACKUP** your seed phrase in a safe, offline location
- Consider using a hardware wallet for production use

### Encryption

- Seed phrases are encrypted using AES-256-GCM
- Encryption key is derived from password using Argon2 (strong KDF)
- Each encryption uses a unique random nonce
- Salt is stored with the ciphertext

### Password Requirements

For production use, enforce strong passwords:
- Minimum 12 characters
- Mix of upper/lowercase, numbers, symbols
- Not a common dictionary word
- Use a password manager

## Payment Integration (TODO)

Lightning payment integration is planned but not yet implemented:

1. **Spark Wallet Integration**:
   - Derive Spark signer from the same BIP39 seed
   - Generate Lightning invoices for job requests
   - Detect invoice payments
   - Auto-process jobs upon payment confirmation

2. **Payment Flow**:
   ```
   1. Receive job request
   2. Generate Lightning invoice (bolt11)
   3. Publish invoice in job feedback event
   4. Wait for payment confirmation
   5. Process job
   6. Publish result
   ```

## Troubleshooting

### Ollama Connection Failed

```
Error: OllamaService failed to connect to http://localhost:11434
```

**Solution**: Ensure Ollama is running:
```bash
ollama serve
```

### Invalid Password

```
Error: SecureStore failed: InvalidPassword
```

**Solution**: Check that you're using the correct password. If you forgot your password, you'll need to regenerate your identity (and lose access to the old keypair).

### Relay Connection Issues

```
Error: RelayService failed to connect to wss://relay.damus.io
```

**Solution**:
- Check internet connection
- Try different relays
- Check firewall settings

### Model Not Found

```
Error: Ollama model 'llama3.2' not found
```

**Solution**: Pull the model first:
```bash
ollama pull llama3.2
```

## CLI Examples (Future)

Planned CLI commands:

```bash
# Generate new identity
compute identity new

# Import existing identity
compute identity import "word1 word2 ..."

# Show public key
compute identity show

# Start the provider
compute start

# List active jobs
compute jobs list

# Show earnings
compute earnings

# Backup seed phrase
compute backup
```

## API Documentation

Generate API docs:

```bash
cargo doc --open --features ui
```

## Contributing

See the main OpenAgents repository for contribution guidelines.

## Bazaar Jobs (Agent Backends)

### AgentBackend Trait

Agent backends handle complex, multi-step tasks that require:
- Repository checkout and file access
- Tool execution (shell commands, file edits)
- Multi-turn reasoning
- Sandboxed execution environments

```rust
use compute::backends::{AgentBackend, AgentCapabilities, JobProgress};

#[async_trait]
pub trait AgentBackend: Send + Sync {
    fn id(&self) -> &str;
    async fn is_ready(&self) -> bool;
    fn capabilities(&self) -> AgentCapabilities;

    async fn patch_gen(&self, req: PatchGenRequest, progress: Option<Sender<JobProgress>>) -> Result<PatchGenResult>;
    async fn code_review(&self, req: CodeReviewRequest, progress: Option<Sender<JobProgress>>) -> Result<CodeReviewResult>;
    async fn sandbox_run(&self, req: SandboxRunRequest, progress: Option<Sender<JobProgress>>) -> Result<SandboxRunResult>;
    async fn repo_index(&self, req: RepoIndexRequest, progress: Option<Sender<JobProgress>>) -> Result<RepoIndexResult>;
    async fn cancel(&self, job_id: &str) -> Result<()>;
}
```

### CodexCodeBackend

The primary agent backend, using Codex with sandbox isolation:

```rust
use compute::backends::{CodexCodeBackend, CodexCodeConfig, IsolationMode};

// Auto-detect Codex availability
if let Some(codex) = CodexCodeBackend::detect().await {
    println!("Codex available!");
    println!("Supported kinds: {:?}", codex.capabilities().supported_kinds());
}

// Manual configuration
let config = CodexCodeConfig {
    isolation_mode: IsolationMode::Container,
    max_workers: 3,
    model: "codex-sonnet-4".to_string(),
    default_time_limit_secs: 900,
    ..Default::default()
};
let backend = CodexCodeBackend::with_config(config).await?;
```

**Detection Requirements:**
- `OPENAI_API_KEY` environment variable, OR
- `codex` CLI in PATH

### PatchGen Jobs (kind 5932)

Generate patches from issues or requirements:

```rust
use compute::domain::{PatchGenRequest, PatchGenResult, PathFilter};

let request = PatchGenRequest {
    repo_url: "https://github.com/owner/repo".to_string(),
    git_ref: Some("main".to_string()),
    issue: "Add dark mode support".to_string(),
    path_filter: Some(PathFilter {
        include: vec!["src/**/*.rs".to_string()],
        exclude: vec!["src/tests/**".to_string()],
    }),
    max_time_secs: Some(600),
    model: Some("codex-sonnet-4".to_string()),
};

// Result includes patch and verification
let result: PatchGenResult = backend.patch_gen(request, None).await?;
println!("Patch:\n{}", result.patch);
println!("Verified: {:?}", result.verification);
```

### CodeReview Jobs (kind 5933)

Review code with structured feedback:

```rust
use compute::domain::{CodeReviewRequest, CodeReviewResult, ReviewInput};

let request = CodeReviewRequest {
    repo_url: "https://github.com/owner/repo".to_string(),
    git_ref: Some("feature-branch".to_string()),
    input: ReviewInput::Diff("--- a/file.rs\n+++ b/file.rs\n...".to_string()),
    review_focus: vec!["security".to_string(), "performance".to_string()],
    model: Some("codex-sonnet-4".to_string()),
    ..Default::default()
};

let result: CodeReviewResult = backend.code_review(request, None).await?;
println!("Approval: {:?}", result.approval);
for issue in result.issues {
    println!("[{}] {}: {}", issue.severity, issue.category, issue.message);
}
```

## Roadmap

- [x] BIP39 seed phrase generation
- [x] NIP-06 Nostr key derivation
- [x] AES-GCM encrypted storage
- [x] NIP-90 job processing (text generation)
- [x] NIP-89 handler info publishing
- [x] Ollama integration
- [x] Nostr relay integration
- [x] Spark wallet integration
- [x] Agent backend abstraction (AgentBackend trait)
- [x] Codex backend (PatchGen, CodeReview, SandboxRun)
- [x] Bazaar job kinds (5930-5933)
- [ ] Lightning invoice generation
- [ ] Payment verification
- [ ] Container isolation (Docker/gvisor)
- [ ] RepoIndex backend
- [ ] Job queue persistence
- [ ] Reputation system
- [ ] Multi-relay load balancing
- [ ] Rate limiting
- [ ] API server mode

## License

MIT

## Related Documentation

- [NIP-90: Data Vending Machines](https://github.com/nostr-protocol/nips/blob/master/90.md)
- [NIP-89: Handler Information](https://github.com/nostr-protocol/nips/blob/master/89.md)
- [NIP-06: Key Derivation](https://github.com/nostr-protocol/nips/blob/master/06.md)
- [BIP-39: Mnemonic Seed Phrases](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
- [Ollama Documentation](https://ollama.com/docs)
