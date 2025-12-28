# nostr/core

Nostr protocol implementation for OpenAgents, providing cryptographic primitives, NIP implementations, and marketplace identity types.

## Overview

This crate implements the core Nostr protocol functionality needed by the OpenAgents platform:

- **NIP-01**: Basic event structure, signing, and verification
- **NIP-06**: Deterministic key derivation from BIP39 mnemonic seed phrases
- **NIP-28**: Public chat channels with moderation
- **NIP-77**: Negentropy protocol for efficient range-based set reconciliation
- **NIP-89**: Application handler discovery (social discovery of skills/agents)
- **NIP-90**: Data Vending Machine (DVM) protocol for job requests and results
- **NIP-SA**: Sovereign Agents protocol for autonomous agents with their own identity
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

## NIP-77: Negentropy Protocol

Efficient range-based set reconciliation for syncing Nostr events between clients and relays.

### Overview

Negentropy enables efficient event syncing with O(log N) round trips instead of O(N) for traditional filter-based approaches. It uses fingerprints to identify differences and recursively narrows down to missing events.

### Protocol Flow

```
Client                           Relay
  |                                |
  |--NEG-OPEN (filter + ranges)--->|
  |                                |
  |<--NEG-MSG (diff ranges)--------|
  |                                |
  |--NEG-MSG (narrowed ranges)---->|
  |                                |
  |<--NEG-MSG (event IDs)----------|
  |                                |
  |--NEG-CLOSE----------------->   |
```

### Varint Encoding

Base-128 variable-length unsigned integers for compact representation:

```rust
use nostr::{encode_varint, decode_varint};

// Encode unsigned integers
let encoded = encode_varint(300)?;
assert_eq!(encoded, vec![0xAC, 0x02]);

// Decode back
let (value, bytes_read) = decode_varint(&encoded)?;
assert_eq!(value, 300);
assert_eq!(bytes_read, 2);

// High-bit indicates continuation
// 300 = 0b100101100 → 0xAC 0x02
//     = [0b10101100, 0b00000010]
//     = [172, 2]
```

### Fingerprint Calculation

16-byte SHA-256 based fingerprint of event ID set:

```rust
use nostr::{calculate_fingerprint, EventId};

let ids = vec![
    "abc...".to_string(),
    "def...".to_string(),
];

// Fingerprint = SHA-256(sum(IDs mod 2^256) || varint(count)).take(16)
let fingerprint = calculate_fingerprint(&ids);
assert_eq!(fingerprint.len(), 16);
```

**Algorithm:**
1. Sum all event IDs modulo 2^256
2. Concatenate with varint-encoded count
3. SHA-256 hash the result
4. Take first 16 bytes

### Record Sorting

Events must be sorted by timestamp, then by ID lexically:

```rust
use nostr::{sort_records, Record};

let mut records = vec![
    Record { timestamp: 1000, id: "bbb".to_string() },
    Record { timestamp: 1000, id: "aaa".to_string() },
    Record { timestamp: 500, id: "zzz".to_string() },
];

sort_records(&mut records);

assert_eq!(records[0].timestamp, 500);  // Earliest timestamp first
assert_eq!(records[1].id, "aaa");       // Then lexical by ID
assert_eq!(records[2].id, "bbb");
```

### Protocol Messages

#### NEG-OPEN: Start Sync Session

```rust
use nostr::{NegOpen, NegentropyMessage, PROTOCOL_VERSION_1};

// Create initial message with ranges
let message = NegentropyMessage {
    version: PROTOCOL_VERSION_1,
    ranges: vec![/* ranges */],
};

// Start session
let neg_open = NegOpen {
    subscription_id: "sub-123".to_string(),
    filter: serde_json::json!({"kinds": [1]}),
    initial_message: message.to_hex()?,
};
```

#### NEG-MSG: Exchange Ranges

```rust
use nostr::{NegMsg, Range, RangeMode, RangePayload, Bound};

// Build range with fingerprint
let range = Range {
    upper_bound: Bound {
        timestamp: 1703000000,
        id_prefix: vec![0xFF],
    },
    payload: RangePayload::Fingerprint(vec![0x12, 0x34, /* 16 bytes */]),
};

// Send message
let neg_msg = NegMsg {
    subscription_id: "sub-123".to_string(),
    message: NegentropyMessage {
        version: PROTOCOL_VERSION_1,
        ranges: vec![range],
    }.to_hex()?,
};
```

#### NEG-ERR: Report Error

```rust
use nostr::NegErr;

let neg_err = NegErr {
    subscription_id: "sub-123".to_string(),
    reason: "invalid fingerprint".to_string(),
};
```

#### NEG-CLOSE: End Session

```rust
use nostr::NegClose;

let neg_close = NegClose {
    subscription_id: "sub-123".to_string(),
};
```

### Range Payloads

```rust
use nostr::{RangeMode, RangePayload};

// Fingerprint: 16-byte hash of IDs in range
let fp = RangePayload::Fingerprint(vec![0u8; 16]);

// HaveIds: Client has these IDs
let have = RangePayload::HaveIds(vec!["id1".into(), "id2".into()]);

// NeedIds: Client needs these IDs
let need = RangePayload::NeedIds(vec!["id3".into(), "id4".into()]);
```

### Bounds and Timestamps

```rust
use nostr::{Bound, TIMESTAMP_INFINITY};

// Range from beginning to timestamp
let bound = Bound {
    timestamp: 1703000000,
    id_prefix: vec![],
};

// Open-ended range (to infinity)
let infinity_bound = Bound {
    timestamp: TIMESTAMP_INFINITY,  // u64::MAX
    id_prefix: vec![0xFF; 32],
};
```

### Integration Example

```rust
use nostr::{
    NegOpen, NegMsg, NegClose, calculate_fingerprint,
    sort_records, Record, NegentropyMessage, PROTOCOL_VERSION_1,
};

// 1. Client: Collect local events
let mut local_events = vec![
    Record { timestamp: 1000, id: "event1".into() },
    Record { timestamp: 2000, id: "event2".into() },
];
sort_records(&mut local_events);

// 2. Client: Calculate fingerprint
let ids: Vec<String> = local_events.iter().map(|r| r.id.clone()).collect();
let fp = calculate_fingerprint(&ids);

// 3. Client: Start sync
let initial_msg = NegentropyMessage {
    version: PROTOCOL_VERSION_1,
    ranges: vec![/* ranges with fp */],
};
let open = NegOpen {
    subscription_id: "sync-1".into(),
    filter: serde_json::json!({"kinds": [1]}),
    initial_message: initial_msg.to_hex()?,
};

// 4. Exchange NEG-MSG until converged

// 5. Client: Close session
let close = NegClose {
    subscription_id: "sync-1".into(),
};
```

### Performance Characteristics

- **Round Trips**: O(log N) where N is number of differences
- **Bandwidth**: O(D) where D is actual number of differences
- **Memory**: O(1) - streaming-friendly, no need to load all events
- **CPU**: Dominated by fingerprint calculation (SHA-256)

### Testing

```bash
# Run NIP-77 tests
cargo test --test nip77

# Test varint encoding
cargo test nip77::tests::test_varint_encode_decode

# Test fingerprint calculation
cargo test nip77::tests::test_calculate_fingerprint

# Test record sorting
cargo test nip77::tests::test_sort_records
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
├── nip77.rs         # Negentropy protocol (range-based set reconciliation)
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

## NIP-SA: Sovereign Agents

Protocol for autonomous agents with their own identity, encrypted state, and transparent decision-making.

### Overview

NIP-SA enables agents to operate as sovereign actors on Nostr:
- **Threshold Signatures**: Agent keys protected by 2-of-3 FROSTR scheme
- **Encrypted State**: Goals, memory, and beliefs stored with NIP-44 encryption
- **Public Transparency**: Optional public goals and decision trajectories
- **Skill Licensing**: Marketplace-based capability acquisition

### Agent Identity

Agents have their own Nostr identity (pubkey) but can't unilaterally sign events. The threshold signature scheme requires cooperation from both the marketplace and the runner.

### Event Kinds

| Kind | Type | Event |
|------|------|-------|
| 39200 | Addressable | Agent Profile |
| 39201 | Addressable | Agent State (encrypted) |
| 39202 | Addressable | Agent Schedule |
| 39203 | Addressable | Public Goals |
| 39210 | Regular | Tick Request |
| 39211 | Regular | Tick Result |
| 39220 | Addressable | Skill License |
| 39221 | Ephemeral | Skill Delivery |
| 39230 | Addressable | Trajectory Session |
| 39231 | Regular | Trajectory Event |

### Agent Profile (kind:39200)

Describes the agent's identity, capabilities, and threshold signature configuration.

```rust
use nostr::{
    AgentProfile, AgentProfileContent, ThresholdConfig,
    AutonomyLevel, KIND_AGENT_PROFILE,
};

// Configure threshold signature (2-of-3 FROSTR)
let threshold = ThresholdConfig {
    threshold: 2,
    total_signers: 3,
    signer_pubkeys: vec![
        "marketplace_pubkey".to_string(),
        "runner_pubkey".to_string(),
        "recovery_pubkey".to_string(),
    ],
};

// Create agent profile content
let content = AgentProfileContent {
    name: "Code Review Bot".to_string(),
    description: Some("Reviews Rust code for best practices".to_string()),
    avatar: Some("https://example.com/bot.jpg".to_string()),
    threshold_config: threshold,
    autonomy_level: AutonomyLevel::SemiAutonomous,
    runner_pubkey: "runner_pubkey".to_string(),
    model: "claude-sonnet-4.5".to_string(),
    created_at: 1703000000,
};

// Build profile event
let profile = AgentProfile::new(content);
let tags = profile.build_tags();
```

**Autonomy Levels:**
- `FullyAutonomous`: Agent makes all decisions independently
- `SemiAutonomous`: Agent requires approval for high-stakes actions
- `Supervised`: Agent requires human review for most actions

### Agent State (kind:39201)

Encrypted storage for agent's internal state: goals, memory, beliefs, wallet balance.

```rust
use nostr::{
    AgentState, AgentStateContent, Goal, GoalStatus, MemoryEntry,
    KIND_AGENT_STATE,
};

// Create goals
let goal1 = Goal::new("goal-1", "Review 10 PRs daily", 1);
let goal2 = Goal::new("goal-2", "Maintain 4.5+ rating", 2);

// Create memory entries
let memory = MemoryEntry::new(
    "observation",
    "Last review received positive feedback",
);

// Build state content
let mut content = AgentStateContent::new();
content.add_goal(goal1);
content.add_goal(goal2);
content.add_memory(memory);
content.update_balance(50000); // 50k sats
content.record_tick(1703001000);

// Create state (must be encrypted with NIP-44)
let state = AgentState::new(content);
let tags = state.build_tags();

// Encrypt before publishing
#[cfg(feature = "full")]
let encrypted_content = state.encrypt(&sender_sk, &agent_pk)?;
```

**State Fields:**
- `goals`: Active goals with progress tracking
- `memory`: Agent observations and reflections
- `pending_tasks`: Queued actions
- `beliefs`: Key-value store for agent knowledge
- `wallet_balance_sats`: Available satoshis
- `tick_count`: Number of execution cycles

### Agent Schedule (kind:39202)

Defines when and how the agent should be triggered.

```rust
use nostr::{
    AgentSchedule, TriggerType, KIND_AGENT_SCHEDULE,
};

// Create schedule with heartbeat + event triggers
let schedule = AgentSchedule::new()
    .with_heartbeat(900)?  // Every 15 minutes
    .add_trigger(TriggerType::Mention)
    .add_trigger(TriggerType::Dm)
    .add_trigger(TriggerType::Zap);

let tags = schedule.build_tags();
```

**Trigger Types:**
- `Heartbeat`: Regular interval (seconds)
- `Mention`: When agent is mentioned
- `Dm`: When agent receives DM
- `Zap`: When agent receives zap
- `Custom(u32)`: Custom event kind

### Public Goals (kind:39203)

Optional public exposure of agent goals for transparency.

```rust
use nostr::{
    PublicGoals, PublicGoalsContent, Goal, KIND_PUBLIC_GOALS,
};

// Create public goals
let goal1 = Goal::new("goal-1", "Be helpful to developers", 1);
let goal2 = Goal::new("goal-2", "Improve code quality", 2);

let content = PublicGoalsContent::with_goals(vec![goal1, goal2]);
let goals = PublicGoals::new(content);
let tags = goals.build_tags();

// Filter active goals
let active = content.active_goals();

// Sort by priority
let prioritized = content.goals_by_priority();
```

### Tick Events (kinds:39210, 39211)

Track agent execution cycles: inputs, processing, outputs.

```rust
use nostr::{
    TickRequest, TickResult, TickResultContent,
    TickAction, TickTrigger, TickStatus,
    KIND_TICK_REQUEST, KIND_TICK_RESULT,
};

// Tick Request (kind:39210) - marks start of execution
let request = TickRequest::new("runner_pubkey", TickTrigger::Heartbeat);
let tags = request.build_tags();

// Tick Result (kind:39211) - reports outcome
let action1 = TickAction::new("post")
    .with_id("event-id-1");
let action2 = TickAction::new("dm")
    .with_metadata("recipient", serde_json::json!("npub..."));

let content = TickResultContent::new(
    1000,  // tokens_in
    500,   // tokens_out
    0.05,  // cost_usd
    2,     // goals_updated
)
.add_action(action1)
.add_action(action2);

let result = TickResult::new(
    "request-id",
    "runner_pubkey",
    TickStatus::Success,
    1234,  // duration_ms
    content,
);

let tags = result.build_tags();
```

**Tick Metrics:**
- Token usage (input/output)
- USD cost
- Goals updated
- Actions taken
- Duration

### Trajectory Events (kinds:39230, 39231)

Transparent record of agent decision-making process.

```rust
use nostr::{
    TrajectorySession, TrajectorySessionContent,
    TrajectoryEvent, TrajectoryEventContent,
    TrajectoryVisibility, StepType,
    KIND_TRAJECTORY_SESSION, KIND_TRAJECTORY_EVENT,
};

// Session (kind:39230) - describes complete trajectory
let session_content = TrajectorySessionContent::new(
    "session-123",
    1703000000,
    "claude-sonnet-4.5",
)
.with_end_time(1703001000)
.with_total_events(42)
.with_hash("sha256-of-all-events");

let session = TrajectorySession::new(
    session_content,
    "tick-456",
    TrajectoryVisibility::Public,
);

// Individual events (kind:39231) - each step in trajectory
let event_content = TrajectoryEventContent::new(StepType::ToolUse)
    .with_data("tool", serde_json::json!("Read"))
    .with_data("input", serde_json::json!({"file_path": "/path"}));

let event = TrajectoryEvent::new(
    event_content,
    "session-123",
    "tick-456",
    5,  // sequence number
);
```

**Step Types:**
- `ToolUse`: Tool invocation
- `ToolResult`: Tool output
- `Message`: Agent response
- `Thinking`: Agent reasoning (may be redacted)

**Visibility:**
- `Public`: NIP-28 channel (fully transparent)
- `Private`: NIP-EE group (encrypted, controlled access)

### Skill Events (kinds:39220, 39221)

Marketplace-based capability acquisition with licenses.

```rust
use nostr::{
    SkillLicense, SkillLicenseContent,
    SkillDelivery, SkillDeliveryContent,
    KIND_SKILL_LICENSE, KIND_SKILL_DELIVERY,
};

// License (kind:39220) - marketplace issues license
let license_content = SkillLicenseContent::new(
    "skill-123",
    "web-scraper",
    "1.0.0",
    1703000000,
    vec!["fetch".to_string(), "parse".to_string()],
)
.with_expires_at(1703086400)
.with_restrictions({
    let mut r = HashMap::new();
    r.insert("max_requests_per_day".into(), serde_json::json!(1000));
    r
});

let license = SkillLicense::new(
    license_content,
    "agent-pubkey",
    1000,  // price_sats
);

// Check license validity
license.validate(current_time)?;

// Check capabilities
assert!(license.content.has_capability("fetch"));

// Delivery (kind:39221) - encrypted skill content
let delivery_content = SkillDeliveryContent::new(
    "skill-123",
    "fn fetch(url: &str) { ... }",
    "rust",
    "sha256-hash",
);

let delivery = SkillDelivery::new(delivery_content, "license-event-id");

// Verify content integrity
delivery.content.verify_hash("sha256-hash")?;
```

**License Features:**
- Expiration or perpetual
- Capability checking
- Usage restrictions (rate limits, quotas)
- Price tracking

**Delivery Features:**
- Multiple content types (rust, python, prompt)
- Hash verification
- Encrypted with NIP-59 gift wrap
- Threshold ECDH for license-gated decryption

### Security Model

**Threshold Signatures (FROSTR):**
- Agent pubkey is deterministic from 3 signer pubkeys
- Requires 2-of-3 signatures for any agent action
- Marketplace verifies license/state before signing
- Runner executes agent and provides signature
- Recovery key held offline for key rotation

**Encrypted State:**
- State encrypted to agent pubkey using NIP-44
- Decryption requires threshold ECDH participation
- Marketplace verifies authorized tick before cooperating
- Prevents unauthorized state access

**Public Transparency:**
- Optional public goals for coordination
- Trajectory events show decision process
- Balances privacy (encrypted state) with transparency (public actions)

## Related NIPs

- [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md): Basic protocol
- [NIP-06](https://github.com/nostr-protocol/nips/blob/master/06.md): Key derivation from mnemonic
- [NIP-28](https://github.com/nostr-protocol/nips/blob/master/28.md): Public Chat
- [NIP-44](https://github.com/nostr-protocol/nips/blob/master/44.md): Versioned Encryption
- [NIP-59](https://github.com/nostr-protocol/nips/blob/master/59.md): Gift Wrap
- [NIP-77](https://github.com/nostr-protocol/nips/blob/master/77.md): Negentropy Protocol
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
