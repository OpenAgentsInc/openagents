# Centralized Agent Definition Architecture (crates/agents)

**Date:** December 12, 2025
**Author:** Claude Code
**Status:** Complete, compiling, ready for integration

## Problem Statement

The OpenAgents codebase lacked a unified definition of "agent" across multiple contexts:
- SDK for programmatic agent control (`claude-agent-sdk`)
- Native UI implementations (`agent_ui`, `atif`)
- Swarm computing (`parallel`, `taskmaster`)
- Agent servers and infrastructure scattered across codebase

As we scale toward billions of agents (local, cloud, swarm), we need:
1. **Single source of truth** for agent identity and capabilities
2. **Portable manifest format** signed with Nostr keypairs
3. **Bitcoin/Lightning integration** for payments and revenue sharing
4. **Environment compatibility** matching for hybrid execution
5. **Declarative definitions** composable into agent stores

## Solution: crates/agents

Created a new Rust crate (`crates/agents`) containing the complete agent type system:

```
crates/agents/
├── Cargo.toml
└── src/
    ├── lib.rs (public API)
    └── core/
        ├── mod.rs (architecture overview)
        ├── id.rs (Nostr-based identity)
        ├── capabilities.rs (tools, skills, job kinds)
        ├── requirements.rs (execution environments, resources)
        ├── economics.rs (Bitcoin/Lightning, pricing)
        ├── state.rs (state machine, stats)
        ├── events.rs (lifecycle events)
        ├── manifest.rs (declarative, signable definitions)
        └── traits.rs (async executor/factory/registry traits)
```

## Key Components

### 1. Agent Identity (id.rs)

**Nostr-based global identity** using BIP32/BIP39:

```rust
pub struct AgentId(pub [u8; 32]);  // 32-byte public key
pub struct AgentKeypair { /* ... */ }
```

- Derives keypairs from BIP39 mnemonics (NIP-06 standard)
- Schnorr signing for manifest verification
- Converts to/from npub format for human readability
- **Zero signup required** — identity emerges from keypair

**Example:**
```rust
let keypair = AgentKeypair::from_mnemonic(mnemonic)?;
let agent_id = keypair.agent_id();  // Unique global identity
```

### 2. Capabilities (capabilities.rs)

Declares what an agent can do:

```rust
pub struct AgentCapabilities {
    pub tools: Vec<ToolCapability>,
    pub job_kinds: Vec<u16>,
    pub skills: Vec<String>,
}
```

- **Job Kinds** (5000-5999): TEXT_EXTRACTION (5000), SUMMARIZATION (5001), TEXT_GENERATION (5050), etc.
- **Skills**: CODE_GENERATION, DEBUGGING, RESEARCH, SUMMARIZATION, etc.
- **Tool Capability**: Discrete functions with schema, permissions, danger level

```rust
// Tools declare what they do
let summarization = ToolCapability {
    name: "summarize",
    description: "Reduce text to key points",
    input_schema: "{ text: string, length: u32 }",
    category: ToolCategory::Text,
    dangerous: false,
    requires_network: false,
    modifies_filesystem: false,
};
```

### 3. Requirements (requirements.rs)

Specify execution environment and resource needs:

```rust
pub enum ExecutionEnvironment {
    Local,
    Cloud { provider: CloudProvider },
    Swarm,
    Hybrid { prefer: ExecutionPreference },
    Oanix { namespace: OanixNamespace },
}
```

- **Environments**: Local device, cloud APIs (Anthropic/OpenAI/Google), swarm, hybrid with fallback, OANIX sandbox
- **Resources**: min_memory, GPU type/VRAM, network access, filesystem paths, disk space, max_execution_time
- **Models**: Specify model name patterns, provider, context length, vision/tools support
- **Sandbox**: Isolation levels (None, Light, Full, Custom) with resource limits

```rust
// Example: Cloud agent with fallback to local
let reqs = AgentRequirements {
    environment: ExecutionEnvironment::Hybrid {
        prefer: ExecutionPreference::PreferCloud,
    },
    resources: ResourceRequirements {
        min_memory: Some(2048),
        max_execution_time: Some(300_000),  // 5 minutes
        ..Default::default()
    },
    model: Some(ModelRequirement {
        model: "claude-*".to_string(),
        requires_vision: false,
        ..Default::default()
    }),
};
```

### 4. Economics (economics.rs)

Bitcoin/Lightning payment and revenue models:

```rust
pub struct AgentEconomics {
    pub wallet: Option<WalletConfig>,
    pub pricing: PricingModel,
    pub payment_methods: Vec<PaymentMethod>,
    pub refund_policy: RefundPolicy,
    pub revenue_sharing: Option<RevenueSharing>,
}
```

**Pricing Models:**
- `Free` — No payment required
- `PerJob { millisats }` — Fixed cost per job
- `PerToken { input_millisats, output_millisats }` — Usage-based
- `PerSecond { millisats }` — Time-based (streaming)
- `Tiered { tiers }` — Complexity-based tiers
- `Custom { description }` — Negotiated

**Wallets:** Lightning address, LNURL, NWC URI, Spark, On-chain Bitcoin
**Revenue Sharing:** Split payments among Creator, ComputeProvider, Platform, Referral
**Refund Policies:** NoRefunds, FullRefundOnFailure, PartialRefund

```rust
// Example: Agent that charges 10 sats per job, splits 50/50 with platform
let economics = AgentEconomics {
    pricing: PricingModel::per_job(10_000),  // 10,000 millisats = 10 sats
    payment_methods: vec![PaymentMethod::Lightning],
    wallet: Some(WalletConfig::lightning("agent@ln.example.com")),
    revenue_sharing: Some(RevenueSharing::new(vec![
        RevenueShare { recipient: "creator".to_string(), percentage: 50, .. },
        RevenueShare { recipient: "platform".to_string(), percentage: 50, .. },
    ])),
    ..Default::default()
};
```

### 5. State & Stats (state.rs)

State machine for agent lifecycle:

```rust
pub enum AgentState {
    Idle,
    Starting,
    Online { relays: Vec<String>, active_sessions: u32 },
    Working { job_id: String, progress: f32, step: String },
    Paused,
    ShuttingDown,
    Error { message: String, recoverable: bool },
}
```

**Statistics tracking:**
- jobs_completed, jobs_failed, total_earnings_millisats
- avg_job_duration_ms, uptime_secs, active_sessions
- success_rate() calculation

### 6. Events (events.rs)

Lifecycle event stream (15+ event types):

```rust
pub enum AgentEvent {
    StateChanged { from: AgentState, to: AgentState },
    Connected { relay: String },
    Disconnected { relay: String },
    JobReceived { job_id: String, kind: u16 },
    JobStarted { job_id: String },
    JobProgress { job_id: String, progress: f32, step: String },
    JobCompleted { job_id: String, content: String },
    JobFailed { job_id: String, error: String },
    PaymentReceived { job_id: String, amount_millisats: u64 },
    PaymentRequired { job_id: String, invoice: JobInvoice },
    ToolInvoked { tool: String, input: Value },
    ToolCompleted { tool: String, output: Value },
    AgentMessage { content: String },
    Error { message: String, recoverable: bool },
    // ... more
}
```

### 7. Manifest (manifest.rs)

**Declarative, signable agent definition** combining all above:

```rust
pub struct AgentManifest {
    pub schema_version: String,
    pub id: Option<AgentId>,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<AgentAuthor>,
    pub license: Option<String>,
    pub capabilities: AgentCapabilities,
    pub requirements: AgentRequirements,
    pub economics: Option<AgentEconomics>,
    pub tags: Vec<String>,
    pub category: AgentCategory,
    pub signature: Option<String>,  // Signed with keypair
}
```

**Signing workflow:**
```rust
let mut manifest = AgentManifest::builder()
    .name("my-agent")
    .version("1.0.0")
    .description("Does cool things")
    .build()?;

// Sign with keypair
manifest.sign(&keypair)?;
assert!(manifest.verify_signature()?);

// Serialize to JSON
let json = manifest.to_json()?;
```

**Categories:** Coding, Research, Writing, Data, Image, Audio, Video, Automation, Security, Finance, General, Custom

### 8. Execution & Registry (traits.rs)

Async traits for runtime integration:

```rust
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn execute(&self, request: JobRequest) -> AgentResult<JobResult>;
    fn can_handle(&self, kind: u16) -> bool;
    fn manifest(&self) -> &AgentManifest;
}

#[async_trait]
pub trait AgentFactory: Send + Sync {
    async fn create(&self, manifest: AgentManifest) -> AgentResult<Arc<dyn AgentExecutor>>;
}

#[async_trait]
pub trait AgentRegistry: Send + Sync {
    async fn find_by_capability(&self, kind: u16) -> AgentResult<Vec<AgentManifest>>;
    async fn get(&self, name: &str) -> AgentResult<Option<AgentManifest>>;
    async fn register(&self, manifest: AgentManifest) -> AgentResult<()>;
}
```

**Job Protocol:**
- `JobRequest` with id, kind, inputs, params, customer, bid, deadline
- `JobResult` with success flag, content/error, duration, cost
- Permission handler for tool access control
- Session management for multi-turn interactions

## Architecture Diagram

```
User/Customer
    |
    v
+------------------+
|  Job Request     |
|  (kind, inputs)  |
+------------------+
    |
    v
+------------------+
| Agent Registry   |
| find_by_kind()   |
+------------------+
    |
    v
+------------------+        +------------------+
| Agent Executor   |------->| Agent Manifest   |
| execute()        |        | (signed)         |
+------------------+        +------------------+
    |                       /   |   \   \
    |                      /    |    \   \
    v                     v     v     v   v
+--------+         +--------+-----+--+--+
| Result | <------ | Capabilities | Requirements | Economics | State
|        |         +--------+-----+--+--+
+--------+              ^
    |                  /
    |     Permission Handler
    |     (tool access control)
    v
Customer Payment
```

## Integration Points

### 1. Agent Registry
Implement for central discovery:
```rust
pub struct CentralRegistry {
    agents: HashMap<String, AgentManifest>,
}

#[async_trait]
impl AgentRegistry for CentralRegistry {
    async fn find_by_capability(&self, kind: u16) -> AgentResult<Vec<AgentManifest>> {
        Ok(self.agents.values()
            .filter(|m| m.can_handle_job(kind))
            .cloned()
            .collect())
    }
}
```

### 2. Agent Factory
Instantiate from manifest:
```rust
pub struct LocalAgentFactory;

#[async_trait]
impl AgentFactory for LocalAgentFactory {
    async fn create(&self, manifest: AgentManifest) -> AgentResult<Arc<dyn AgentExecutor>> {
        // Verify signature first
        if !manifest.verify_signature()? {
            return Err(AgentError::ExecutionFailed("Invalid signature".into()));
        }
        // Instantiate appropriate executor based on requirements
        // ...
    }
}
```

### 3. Payment Processing
Calculate job cost:
```rust
let cost = manifest.economics.calculate_cost(&job_estimate);
// Check if bid acceptable
if !manifest.economics.is_bid_acceptable(customer_bid, &job_estimate) {
    // Request payment
}
```

### 4. Environment Matching
Match agent to execution environment:
```rust
let can_run_here = manifest
    .requirements
    .environment
    .compatible_with(&host.environment);
```

## Next Steps

### Short Term (Integration)
1. **Agent Store**: SQLite/JSONL persistence for manifests
2. **Nostr Integration**: Publish/subscribe agents via NIP-90 DVM protocol
3. **Bitcoin/Lightning**: Hook economics.calculate_cost() to payment layer
4. **Executor Implementations**:
   - Local executor (run WASM, system binaries)
   - Cloud executor (route to Anthropic/OpenAI APIs)
   - Swarm executor (route to network participants)

### Medium Term (Scale)
1. **Agent Discovery**: Market/store UI for browsing agents by capability
2. **Marketplace Economics**: Revenue sharing settlement via Lightning
3. **Reputation System**: Agent success/failure rates, customer reviews
4. **Auto-Scaling**: Spawn instances based on job demand
5. **Version Management**: Manifest versioning, deprecation, migration

### Long Term (Ecosystem)
1. **Agent Composition**: Chain agents together (output of one → input of next)
2. **Skill Marketplace**: Shared skill libraries, version resolution
3. **Model Marketplace**: Agents bidding for compute from hardware providers
4. **Cross-Chain**: Bridge to other networks (Bitcoin, Solana, etc.)

## Testing

All components include unit tests. Verify:
```bash
cargo test -p agents --lib
```

**Test coverage:**
- `id.rs`: BIP39 derivation, Schnorr signing, NIP-06 vectors
- `manifest.rs`: Builder pattern, signing/verification, JSON serialization
- `economics.rs`: Pricing calculations, cost estimates
- `capabilities.rs`: Job kind matching, skill queries
- `requirements.rs`: Environment compatibility, resource validation

## Breaking Changes

**None.** This is a new crate. Existing codebases can adopt gradually:
1. Import from `agents::core::*`
2. Replace scatter ad-hoc agent definitions with `AgentManifest`
3. Implement `AgentExecutor` trait for specific runtime
4. Register with central `AgentRegistry`

## Design Principles

1. **Decentralized by default**: Identity via Nostr keypair, not central ID server
2. **Portable**: JSON-serializable, signature-verifiable across systems
3. **Extensible**: Tags, metadata, custom categories for future features
4. **Economically aware**: Built-in pricing, payments, revenue sharing
5. **Resource conscious**: Explicit environment compatibility, resource limits
6. **Async-first**: All I/O operations use async traits
7. **Type-safe**: Rust's type system prevents invalid state transitions

## Files Created

- `crates/agents/Cargo.toml` (41 lines) — Crate manifest
- `crates/agents/src/lib.rs` (120+ lines) — Public API, architecture docs
- `crates/agents/src/core/mod.rs` (80+ lines) — Module organization
- `crates/agents/src/core/id.rs` (450+ lines) — Nostr identity + signing
- `crates/agents/src/core/capabilities.rs` (300+ lines) — Tools, skills, job kinds
- `crates/agents/src/core/requirements.rs` (400+ lines) — Environments, resources, models
- `crates/agents/src/core/economics.rs` (280+ lines) — Pricing, payments, revenue
- `crates/agents/src/core/state.rs` (150+ lines) — State machine, stats
- `crates/agents/src/core/events.rs` (300+ lines) — Event stream, severity levels
- `crates/agents/src/core/manifest.rs` (350+ lines) — Declarative definitions
- `crates/agents/src/core/traits.rs` (250+ lines) — Async executor/factory/registry
- **Total**: ~3,500 lines of code + tests

## Compilation Status

✅ Compiling cleanly: `cargo check -p agents`

```
    Checking agents v0.1.0
    Finished `dev` profile [optimized + debuginfo] target(s) in 4.49s
```

---

**Next:** Integrate with existing codebase, replace scattered agent definitions with AgentManifest references, connect to Nostr/Bitcoin infrastructure.
