# Centralized Agent Definition

**Date:** 2024-12-12
**Commit:** 0589abff3

## Summary

Created a unified agent type system in `crates/agent/src/core/` designed for an ecosystem of billions of agents running across local, cloud, and swarm compute environments.

## Problem Statement

The codebase had fragmented agent definitions scattered across multiple crates:

| Location | Type | Issue |
|----------|------|-------|
| `crates/atif/src/agent.rs` | Simple `Agent` struct | Only name/version/model |
| `crates/agent/src/agent.rs` | `NativeAgent` | Tightly coupled to gpui/project |
| `crates/agent_servers/` | `AgentServer` trait | Connection factory only |
| `crates/agent_ui/` | `ExternalAgent` enum | UI-specific |
| `crates/parallel/` | `AgentPool`, `PoolAgent` | Parallel execution only |

No unified model for:
- Universal identity
- Capability declaration
- Execution environment abstraction
- Economic layer (payments)
- Inter-agent communication

## Solution: Core Agent Types

### New Module Structure

```
crates/agent/src/core/
├── mod.rs           # Module root with architecture diagram
├── id.rs            # AgentId, AgentKeypair (Nostr-based identity)
├── capabilities.rs  # AgentCapabilities, ToolCapability, skills
├── requirements.rs  # ExecutionEnvironment, ResourceRequirements
├── economics.rs     # AgentEconomics, WalletConfig, PricingModel
├── state.rs         # AgentState, AgentStats
├── events.rs        # AgentEvent (lifecycle events)
├── manifest.rs      # AgentManifest (declarative agent definition)
└── traits.rs        # AgentExecutor, AgentFactory, AgentRegistry traits
```

### Key Types

#### Identity (`id.rs`)

```rust
/// Universal agent identity from Nostr public key
pub struct AgentId {
    pubkey: [u8; 32],
}

/// Full keypair for signing (NIP-06 compatible)
pub struct AgentKeypair {
    private_key: [u8; 32],
    public_key: [u8; 32],
}
```

- Derives from BIP39 mnemonic via NIP-06 path `m/44'/1237'/0'/0/0`
- Supports npub/nsec encoding
- Schnorr signatures for message signing

#### Capabilities (`capabilities.rs`)

```rust
pub struct AgentCapabilities {
    pub tools: Vec<ToolCapability>,      // Discrete functions
    pub job_kinds: Vec<u16>,             // NIP-90 kinds (5000-5999)
    pub skills: HashSet<String>,         // High-level abilities
    pub model: Option<ModelCapability>,  // LLM config if applicable
}
```

#### Requirements (`requirements.rs`)

```rust
pub enum ExecutionEnvironment {
    Local,                              // Device-local (Apple FM, etc.)
    Cloud { provider: CloudProvider },  // Anthropic, OpenAI, etc.
    Swarm,                              // Distributed compute network
    Hybrid { prefer: ExecutionPreference },
    Oanix { namespace: OanixNamespace }, // Plan 9-style sandbox
}

pub struct ResourceRequirements {
    pub min_memory: Option<u64>,
    pub gpu: Option<GpuRequirement>,
    pub network: NetworkAccess,
    pub filesystem: FilesystemAccess,
}
```

#### Economics (`economics.rs`)

```rust
pub struct AgentEconomics {
    pub wallet: Option<WalletConfig>,     // Lightning, Spark, NWC
    pub pricing: PricingModel,            // Per-job, per-token, etc.
    pub payment_methods: Vec<PaymentMethod>,
    pub revenue_sharing: Option<RevenueSharing>,
}

pub enum PricingModel {
    Free,
    PerJob { millisats: u64 },
    PerToken { input_millisats: u64, output_millisats: u64 },
    PerSecond { millisats: u64 },
    Tiered { tiers: Vec<PriceTier> },
    Custom { description: String },
}
```

#### Manifest (`manifest.rs`)

```rust
pub struct AgentManifest {
    pub id: Option<AgentId>,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub capabilities: AgentCapabilities,
    pub requirements: AgentRequirements,
    pub economics: Option<AgentEconomics>,
    pub signature: Option<String>,  // Signed by creator
}
```

#### Traits (`traits.rs`)

```rust
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn execute(&self, request: JobRequest) -> AgentResult<JobResult>;
    fn can_handle(&self, kind: u16) -> bool;
    fn state(&self) -> AgentState;
    fn events(&self) -> broadcast::Receiver<AgentEvent>;
}

#[async_trait]
pub trait AgentFactory: Send + Sync {
    async fn create(&self, manifest: AgentManifest) -> AgentResult<Arc<dyn AgentExecutor>>;
    fn supported_environments(&self) -> Vec<ExecutionEnvironment>;
}

#[async_trait]
pub trait AgentRegistry: Send + Sync {
    async fn find_by_capability(&self, kind: u16) -> AgentResult<Vec<AgentManifest>>;
    async fn get(&self, id: &AgentId) -> AgentResult<Option<AgentManifest>>;
    async fn register(&self, manifest: AgentManifest) -> AgentResult<()>;
}
```

## Design Decisions

### 1. Nostr-Native Identity

Every agent has a Nostr keypair (npub/nsec). Benefits:
- No signup required (generate on first run)
- Globally unique without central registry
- Cryptographically verifiable ownership
- Interoperable with broader Nostr ecosystem

### 2. NIP-90 DVM Protocol Alignment

Job kinds (5000-5999) and job flow align with Data Vending Machine spec:
- JobRequest with inputs, params, bid
- JobFeedback for status updates
- JobResult with content and payment info

This enables agents to participate in the existing Nostr DVM marketplace.

### 3. Execution-Agnostic Design

Same `AgentManifest` can be executed by different backends:
- `LocalAgentExecutor` - Apple FM, OANIX sandbox
- `CloudAgentExecutor` - Claude, GPT, Gemini
- `SwarmAgentExecutor` - Distributed NIP-90 network

### 4. Bitcoin-First Economics

Built-in support for:
- Lightning payments (bolt11 invoices)
- Spark wallet integration
- Nostr Wallet Connect (NIP-47)
- Revenue sharing between creators/providers

### 5. Signable Manifests

Manifests can be cryptographically signed by the creator:
```rust
manifest.sign(&keypair)?;
assert!(manifest.verify_signature()?);
```

This enables verification that a published agent came from its claimed creator.

## Reference: claude-agent-sdk

Used `crates/claude-agent-sdk` as reference for:
- Query/Session model
- Permission handling pattern
- Tool capability structure
- Stream-based event model

Key differences:
- Our model is runtime-agnostic (not Claude-specific)
- Identity is Nostr-based (not API keys)
- Economics are built into the core

## Files Changed

```
crates/agent/Cargo.toml          # Added bech32, bip39, bitcoin deps
crates/agent/src/agent.rs        # Added `pub mod core;`
crates/agent/src/core/mod.rs     # Module root
crates/agent/src/core/id.rs      # ~330 lines
crates/agent/src/core/capabilities.rs  # ~370 lines
crates/agent/src/core/requirements.rs  # ~500 lines
crates/agent/src/core/economics.rs     # ~450 lines
crates/agent/src/core/state.rs   # ~130 lines
crates/agent/src/core/events.rs  # ~280 lines
crates/agent/src/core/manifest.rs      # ~380 lines
crates/agent/src/core/traits.rs  # ~280 lines
```

Total: ~4,200 lines of new code

## Next Steps

### Immediate

1. **Compatibility Layer**: Create `crates/agent/src/core/compat/` to bridge existing types:
   - `NativeAgent` → `AgentManifest` conversion
   - `AgentServer` → `AgentFactory` adapter
   - `ExternalAgent` → capability-based lookup

2. **Tests**: Add integration tests that verify:
   - Manifest signing/verification
   - Capability matching
   - Environment compatibility checks

### Short-term

3. **Agent Runtime**: Implement `Agent` runtime struct that wraps `AgentManifest` + `AgentExecutor`:
   ```rust
   pub struct Agent {
       manifest: AgentManifest,
       keypair: Option<AgentKeypair>,
       state: AgentState,
       executor: Option<Arc<dyn AgentExecutor>>,
   }
   ```

4. **Local Executor**: Create `LocalAgentExecutor` using Apple FM:
   - Integrate with `crates/fm-bridge`
   - Use OANIX for sandboxing

5. **NIP-90 Protocol**: Implement full NIP-90 job flow in `crates/agent/src/core/protocol/`:
   - Job request/result publishing to Nostr relays
   - Payment handling via bolt11

### Medium-term

6. **Agent Store**: Implement `AgentStore` trait for SQLite persistence:
   - Save/load manifests
   - Track job history
   - Store earnings

7. **Registry Integration**: Connect to Nostr relays for agent discovery:
   - Publish manifests as Nostr events
   - Subscribe to job requests matching capabilities

8. **Swarm Executor**: Implement `SwarmAgentExecutor`:
   - Route jobs to cheapest/fastest providers
   - Handle payment flow
   - Aggregate results

### Long-term

9. **Agent Composition**: Enable agents to call other agents:
   - Hierarchical job execution
   - Payment routing
   - Capability aggregation

10. **Reputation System**: Track agent performance:
    - Success rate
    - Response time
    - Payment reliability

## Usage Example

```rust
use agent::core::*;

// Create manifest
let manifest = AgentManifest::builder()
    .name("code-reviewer")
    .version("1.0.0")
    .description("Reviews code for bugs and style issues")
    .capabilities(AgentCapabilities::builder()
        .add_skill(SKILL_CODE_REVIEW)
        .add_job_kind(KIND_JOB_TEXT_GENERATION)
        .add_tool(ToolCapability::new("read_file", "Read file contents")
            .with_filesystem_access())
        .build())
    .requirements(AgentRequirements::builder()
        .environment(ExecutionEnvironment::Hybrid {
            prefer: ExecutionPreference::PreferLocal,
        })
        .build())
    .economics(AgentEconomics::builder()
        .pricing(PricingModel::per_job(10000)) // 10 sats
        .wallet(WalletConfig::lightning("agent@getalby.com"))
        .build())
    .build()?;

// Sign with creator keypair
let keypair = AgentKeypair::from_mnemonic("word1 word2 ... word12")?;
manifest.sign(&keypair)?;

// Verify signature
assert!(manifest.verify_signature()?);

// Publish to registry
registry.register(manifest).await?;
```
