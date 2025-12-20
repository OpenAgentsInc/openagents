# nostr/core

Nostr protocol implementation for OpenAgents, providing cryptographic primitives, NIP implementations, and marketplace identity types.

## Overview

This crate implements the core Nostr protocol functionality needed by the OpenAgents platform:

- **NIP-01**: Basic event structure, signing, and verification
- **NIP-06**: Deterministic key derivation from BIP39 mnemonic seed phrases
- **NIP-28**: Public chat channels with moderation
- **NIP-89**: Application handler discovery (social discovery of skills/agents)
- **NIP-90**: Data Vending Machine (DVM) protocol for job requests and results
- **Identity Types**: Marketplace participants (agents, creators, providers)
- **Provider Types**: Compute marketplace with pricing and reputation

## Features

### `full` (default)

Includes complete cryptographic operations:
- Key generation and signing
- BIP39 mnemonic support
- Event verification
- DateTime types for profiles

### `minimal`

Event types and serialization only, suitable for:
- WASM compilation
- Relay implementations
- Environments without crypto dependencies

## Quick Start

```rust
use nostr::{
    Event, EventTemplate, finalize_event, verify_event,
    derive_keypair, KIND_SHORT_TEXT_NOTE,
};

// Derive keypair from mnemonic (NIP-06)
let mnemonic = "leader monkey parrot ring guide accident before fence cannon height naive bean";
let keypair = derive_keypair(mnemonic)?;

// Create and sign an event (NIP-01)
let template = EventTemplate {
    kind: KIND_SHORT_TEXT_NOTE,
    tags: vec![],
    content: "Hello, Nostr!".to_string(),
    created_at: chrono::Utc::now().timestamp() as u64,
};

let event = finalize_event(&template, &keypair.private_key)?;

// Verify the event
assert!(verify_event(&event)?);
```

## NIP-01: Basic Protocol

The foundation of all Nostr interactions.

### Event Structure

```rust
pub struct Event {
    pub id: String,         // 32-byte sha256 of serialized event
    pub pubkey: String,     // 32-byte public key (hex)
    pub created_at: u64,    // Unix timestamp
    pub kind: u16,          // Event type
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,        // 64-byte Schnorr signature (hex)
}
```

### Creating Events

```rust
use nostr::{EventTemplate, finalize_event, KIND_SHORT_TEXT_NOTE};

let private_key = [0u8; 32]; // Your 32-byte private key

let template = EventTemplate {
    kind: KIND_SHORT_TEXT_NOTE,
    tags: vec![
        vec!["e".to_string(), "event_id_to_reply_to".to_string()],
        vec!["p".to_string(), "pubkey_to_mention".to_string()],
    ],
    content: "This is a reply!".to_string(),
    created_at: 1234567890,
};

let event = finalize_event(&template, &private_key)?;
```

### Event Kinds Classification

```rust
use nostr::{classify_kind, KindClassification};

// Regular events (stored by relays)
assert_eq!(classify_kind(1), KindClassification::Regular);

// Replaceable (only latest per pubkey+kind)
assert_eq!(classify_kind(0), KindClassification::Replaceable);  // Metadata
assert_eq!(classify_kind(3), KindClassification::Replaceable);  // Contacts

// Ephemeral (not stored)
assert_eq!(classify_kind(20000), KindClassification::Ephemeral);

// Addressable (latest per pubkey+kind+d-tag)
assert_eq!(classify_kind(30000), KindClassification::Addressable);
```

### Verification

```rust
use nostr::verify_event;

let is_valid = verify_event(&event)?;
if !is_valid {
    eprintln!("Invalid signature or event ID!");
}
```

## NIP-06: Key Derivation

Deterministic key generation from BIP39 mnemonic seed phrases.

### Derivation Path

Uses BIP32 path: `m/44'/1237'/<account>'/0/0`

- Coin type 1237 is registered for Nostr in SLIP-0044
- Account index allows multiple identities from one mnemonic
- First two derivation steps (44' and 1237') are hardened

### Basic Usage

```rust
use nostr::{derive_keypair, Keypair};

// Generate from 12 or 24-word mnemonic
let mnemonic = "leader monkey parrot ring guide accident before fence cannon height naive bean";
let keypair = derive_keypair(mnemonic)?;

// Access keys
println!("Private key (hex): {}", keypair.private_key_hex());
println!("Public key (hex): {}", keypair.public_key_hex());
println!("nsec: {}", keypair.nsec()?);
println!("npub: {}", keypair.npub()?);
```

### Multiple Accounts

```rust
use nostr::derive_keypair_with_account;

// Derive different accounts from same mnemonic
let account_0 = derive_keypair_with_account(mnemonic, 0)?;
let account_1 = derive_keypair_with_account(mnemonic, 1)?;
let account_2 = derive_keypair_with_account(mnemonic, 2)?;

// Each account has different keys
assert_ne!(account_0.public_key, account_1.public_key);
```

### Passphrase Support

```rust
use nostr::derive_keypair_full;

// Optional BIP39 passphrase (not for encryption, part of seed derivation)
let keypair = derive_keypair_full(mnemonic, "my_passphrase", 0)?;
```

### Bech32 Encoding

```rust
use nostr::{private_key_to_nsec, public_key_to_npub};

let private_key = [0u8; 32];
let public_key = [0u8; 32];

let nsec = private_key_to_nsec(&private_key)?;  // "nsec1..."
let npub = public_key_to_npub(&public_key)?;    // "npub1..."
```

## NIP-28: Public Chat

Channel creation, messaging, and moderation.

### Channel Events

```rust
use nostr::{
    ChannelMetadata, KIND_CHANNEL_CREATION, KIND_CHANNEL_MESSAGE,
    KIND_CHANNEL_METADATA, KIND_CHANNEL_MUTE_USER,
};

// Channel kinds
const CREATION: u16 = 40;         // Create channel
const METADATA: u16 = 41;         // Update metadata
const MESSAGE: u16 = 42;          // Chat message
const HIDE_MESSAGE: u16 = 43;     // Moderator hides message
const MUTE_USER: u16 = 44;        // Moderator mutes user

// Channel metadata
let metadata = ChannelMetadata {
    name: "General Discussion".to_string(),
    about: Some("A place for general chat".to_string()),
    picture: Some("https://example.com/channel.jpg".to_string()),
    creator: "pubkey_hex".to_string(),
};
```

### Moderation

```rust
use nostr::{ModerationReason, is_moderation_kind};

// Moderators can hide messages or mute users
let reasons = vec![
    ModerationReason::Spam,
    ModerationReason::Illegal,
    ModerationReason::Offensive,
];

// Check if kind is moderation-related
assert!(is_moderation_kind(KIND_CHANNEL_HIDE_MESSAGE));
assert!(is_moderation_kind(KIND_CHANNEL_MUTE_USER));
```

## NIP-89: Application Handlers

Social discovery of skills, agents, and applications.

### Handler Recommendation

```rust
use nostr::{
    HandlerRecommendation, HandlerType,
    KIND_HANDLER_RECOMMENDATION, KIND_HANDLER_INFO,
};

// Recommend a handler for a specific event kind
let recommendation = HandlerRecommendation {
    handler_type: HandlerType::Web,
    kind: Some(5050),  // Recommend handler for text generation
    handler_pubkey: "handler_pubkey_hex".to_string(),
    relay_url: Some("wss://relay.example.com".to_string()),
};
```

### Handler Information

```rust
use nostr::{HandlerInfo, PricingInfo, SocialTrustScore};

// Publish handler info (kind 31990)
let handler_info = HandlerInfo {
    name: "Text Generation Agent".to_string(),
    display_name: Some("GPT-4 Helper".to_string()),
    about: Some("Fast text generation using GPT-4".to_string()),
    picture: Some("https://example.com/avatar.jpg".to_string()),
    website: Some("https://example.com".to_string()),
    kind: 5050,  // Handles NIP-90 text generation
    pricing: Some(PricingInfo {
        per_request: Some(1000),  // 1000 sats per request
        per_token: None,
    }),
    trust_score: Some(SocialTrustScore {
        followers: 1500,
        endorsements: 42,
    }),
};
```

## NIP-90: Data Vending Machine (DVM)

Protocol for on-demand computation: money in, data out.

### Job Request Flow

1. Customer publishes **job request** (kind 5000-5999)
2. Service providers may send **feedback** (kind 7000)
3. Provider completes work and publishes **result** (kind 6000-6999)
4. Customer pays via bolt11 or zap

### Job Request

```rust
use nostr::{
    JobRequest, JobInput, JobParam,
    KIND_JOB_TEXT_GENERATION,
};

// Create a text generation request
let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
    .add_input(JobInput::text("What is the capital of France?"))
    .add_param("model", "gpt-4")
    .add_param("temperature", "0.7")
    .with_output("text/plain")
    .with_bid(1000)  // Max 1000 sats
    .add_relay("wss://relay.damus.io");

// Convert to tags for event
let tags = request.to_tags();
```

### Input Types

```rust
use nostr::{JobInput, InputType};

// Text input
let input = JobInput::text("Translate this text");

// URL input
let input = JobInput::url("https://example.com/audio.mp3");

// Event reference
let input = JobInput::event("event_id", Some("wss://relay.com".to_string()));

// Job chaining (use output from previous job)
let input = JobInput::job("job_event_id", Some("wss://relay.com".to_string()));
```

### Job Result

```rust
use nostr::JobResult;

// Service provider sends result
let result = JobResult::new(
    5050,  // Request kind (text generation)
    "request_event_id",
    "customer_pubkey",
    "The capital of France is Paris.",
)?
.with_request_relay("wss://relay.damus.io")
.with_amount(500, Some("lnbc500n1...".to_string()));

// Result kind is always request_kind + 1000
assert_eq!(result.kind, 6050);
```

### Job Feedback

```rust
use nostr::{JobFeedback, JobStatus};

// Provider sends processing update
let feedback = JobFeedback::new(
    JobStatus::Processing,
    "request_event_id",
    "customer_pubkey",
);

// Request payment
let feedback = JobFeedback::new(
    JobStatus::PaymentRequired,
    "request_event_id",
    "customer_pubkey",
)
.with_status_extra("Please pay 1000 sats to continue")
.with_amount(1000, Some("lnbc1000n1...".to_string()));
```

### Common Job Kinds

```rust
use nostr::*;

// NIP-90 defines common job types
const TEXT_EXTRACTION: u16 = 5000;      // OCR
const SUMMARIZATION: u16 = 5001;        // Text summarization
const TRANSLATION: u16 = 5002;          // Language translation
const TEXT_GENERATION: u16 = 5050;      // LLM text generation
const IMAGE_GENERATION: u16 = 5100;     // Image generation
const SPEECH_TO_TEXT: u16 = 5250;       // Transcription

// Results are always kind + 1000
assert_eq!(get_result_kind(5050), Some(6050));
```

## Identity Types

### NostrIdentity

Base identity type for all marketplace participants.

```rust
use nostr::NostrIdentity;

// From hex pubkey
let identity = NostrIdentity::new("a".repeat(64))?;

// From npub
let identity = NostrIdentity::new("npub1...")?;

println!("Public key: {}", identity.pubkey());
```

### CreatorProfile

Identity for creators publishing skills and agents.

```rust
use nostr::CreatorProfile;

let identity = NostrIdentity::new(pubkey)?;
let profile = CreatorProfile::new(identity, "Alice")
    .with_bio("AI developer and agent creator")
    .with_website("https://alice.dev")
    .with_lightning_address("alice@getalby.com")?
    .with_avatar("https://example.com/alice.jpg")
    .verify();  // Mark as verified

assert!(profile.verified);
```

### AgentIdentity

Identity for autonomous agents in the marketplace.

```rust
use nostr::{AgentIdentity, WalletInfo, ReputationScore};

let identity = NostrIdentity::new(pubkey)?;
let wallet = WalletInfo::connected("agent@ln.address");

let agent = AgentIdentity::new(identity, "Code Review Bot")
    .with_wallet(wallet)
    .with_description("Reviews Rust code for best practices")
    .add_job_kind(5001)  // Summarization
    .add_job_kind(5050); // Text generation

// Check capabilities
assert!(agent.supports_job_kind(5001));

// Reputation tracking
let mut reputation = ReputationScore::default();
reputation.jobs_completed = 100;
reputation.jobs_successful = 95;
reputation.rating = 4.5;

println!("Success rate: {}%", reputation.success_rate());
assert!(reputation.is_reputable());
```

## Compute Provider Types

### ComputeProvider

Represents a compute provider in the decentralized marketplace.

```rust
use nostr::{
    ComputeProvider, ComputePricing, ComputeCapabilities,
    Region, NostrIdentity,
};

let identity = NostrIdentity::new(pubkey)?;

// Configure pricing
let pricing = ComputePricing::new(
    10,   // 10 sats per 1k input tokens
    20,   // 20 sats per 1k output tokens
    100,  // 100 sat minimum
)?;

// Configure capabilities
let capabilities = ComputeCapabilities::new(
    vec!["llama-70b".to_string(), "mistral-7b".to_string()],
    8192,  // Max context window
    2048,  // Max output tokens
)?;

// Create provider
let provider = ComputeProvider::new(
    identity,
    "provider@getalby.com",
    Region::UsWest,
    pricing,
    capabilities,
)?
.with_name("Fast Inference Provider")
.with_description("Low-latency LLM inference");

// Calculate job cost
let cost = provider.calculate_job_cost(1000, 500);
println!("Job cost: {} sats", cost);
```

### Reputation System

```rust
use nostr::{ProviderReputation, ReputationTier};

let mut rep = ProviderReputation::default();
rep.jobs_completed = 1500;
rep.success_rate = 0.99;
rep.avg_latency_ms = 400;
rep.uptime_pct = 0.99;

// Automatic tier calculation
match rep.tier() {
    ReputationTier::New => println!("New provider"),
    ReputationTier::Established => println!("Established provider"),
    ReputationTier::Trusted => println!("Trusted provider"),
    ReputationTier::Premium => println!("Premium provider"),
}

assert!(rep.is_reliable());  // >95% success rate
assert!(rep.is_fast());      // <1000ms avg latency
```

### Geographic Regions

```rust
use nostr::Region;

let regions = vec![
    Region::UsWest,
    Region::UsEast,
    Region::EuCentral,
    Region::AsiaPacific,
];

for region in regions {
    println!("{}: {}", region, region.display_name());
}
```

## Testing

The crate includes comprehensive tests matching the nostr-tools test suite:

```bash
# Run all tests (requires 'full' feature)
cargo test

# Run minimal feature tests
cargo test --no-default-features --features minimal

# Run specific NIP tests
cargo test --test nip01
cargo test --test nip06
cargo test --test nip90
```

### Test Vectors

NIP-06 includes official test vectors from the spec:

```rust
// Test vector from NIP-06 specification
let mnemonic = "leader monkey parrot ring guide accident before fence cannon height naive bean";
let keypair = derive_keypair(mnemonic)?;

assert_eq!(
    keypair.private_key_hex(),
    "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a"
);
assert_eq!(
    keypair.public_key_hex(),
    "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917"
);
```

## Architecture

### Module Organization

```
nostr/core/src/
├── lib.rs           # Public API and feature gates
├── nip01.rs         # Basic protocol (events, signing)
├── nip06.rs         # Key derivation from mnemonic
├── nip28.rs         # Public chat channels
├── nip89.rs         # Application handlers
├── nip90.rs         # Data Vending Machine
├── identity.rs      # Marketplace identity types
├── provider.rs      # Compute provider types
├── payments.rs      # Lightning payment types
└── compute_job.rs   # Job routing and selection
```

### Feature Flags

| Feature | Dependencies | Use Case |
|---------|-------------|----------|
| `full` (default) | bip39, bitcoin, bech32, rand, chrono | Full node, CLI tools, wallets |
| `minimal` | serde, serde_json, hex, sha2 | WASM, relays, lightweight clients |

## Security Considerations

### Private Key Handling

```rust
// ✅ Good: Private keys are stored securely
let keypair = derive_keypair(mnemonic)?;
// Private key never exposed in Debug output
println!("{:?}", keypair);  // Shows [redacted]

// ❌ Bad: Never log or print private keys
println!("{}", hex::encode(keypair.private_key));  // DON'T DO THIS
```

### Event Verification

Always verify events before trusting their content:

```rust
use nostr::verify_event;

// Verify signature and event ID match
if !verify_event(&event)? {
    return Err("Invalid event signature");
}

// Check event is recent (prevent replay attacks)
let now = chrono::Utc::now().timestamp() as u64;
if event.created_at > now + 60 {
    return Err("Event from the future");
}
```

### Mnemonic Security

```rust
use nostr::derive_keypair;

// ✅ Good: Load from secure storage
let mnemonic = std::fs::read_to_string("/secure/path/mnemonic.txt")?;
let keypair = derive_keypair(&mnemonic)?;

// ❌ Bad: Hardcoded mnemonics in source code
let keypair = derive_keypair("abandon abandon abandon...")?;  // DON'T
```

## Related NIPs

- [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md): Basic protocol
- [NIP-06](https://github.com/nostr-protocol/nips/blob/master/06.md): Key derivation from mnemonic
- [NIP-28](https://github.com/nostr-protocol/nips/blob/master/28.md): Public Chat
- [NIP-89](https://github.com/nostr-protocol/nips/blob/master/89.md): Application Handlers
- [NIP-90](https://github.com/nostr-protocol/nips/blob/master/90.md): Data Vending Machine

## Dependencies

- `serde`/`serde_json`: Event serialization
- `hex`: Hex encoding for keys and signatures
- `sha2`: SHA-256 hashing for event IDs
- `thiserror`: Error handling

### Full Feature Dependencies

- `bip39`: BIP39 mnemonic phrase support
- `bitcoin`: BIP32 key derivation and secp256k1 signatures
- `bech32`: Bech32 encoding (nsec/npub)
- `rand`: Random number generation for keys
- `chrono`: DateTime support for profiles

## License

Same as the OpenAgents workspace.
