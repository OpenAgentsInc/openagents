# Runtime Traits

Core trait definitions for the OpenAgents Runtime.

---

## Agent Trait

The fundamental abstraction for all agents:

```rust
/// A persistent autonomous entity with identity, memory, and agency.
pub trait Agent: Send + Sync + 'static {
    /// Agent's persistent state type.
    type State: AgentState;

    /// Agent's configuration type.
    type Config: AgentConfig;

    /// Called once when agent is first created.
    fn on_create(&self, ctx: &mut AgentContext<Self::State>) -> Result<()>;

    /// Called when agent wakes from hibernation.
    fn on_wake(&self, ctx: &mut AgentContext<Self::State>) -> Result<()>;

    /// Called before agent hibernates.
    fn on_sleep(&self, ctx: &mut AgentContext<Self::State>) -> Result<()>;

    /// Called when agent receives a trigger (message, alarm, event).
    fn on_trigger(
        &self,
        ctx: &mut AgentContext<Self::State>,
        trigger: Trigger,
    ) -> Result<TickResult>;

    /// Called when an error occurs during tick execution.
    fn on_error(&self, ctx: &mut AgentContext<Self::State>, error: &AgentError);

    /// Called before agent is terminated.
    fn on_terminate(&self, ctx: &mut AgentContext<Self::State>) -> Result<()>;
}
```

---

## State Traits

```rust
/// Marker trait for agent state types.
pub trait AgentState: Serialize + DeserializeOwned + Default + Send + Sync {
    /// Version for schema migration.
    fn version() -> u32 { 1 }

    /// Migrate from previous version.
    fn migrate(from_version: u32, data: &[u8]) -> Result<Self> {
        if from_version == Self::version() {
            Ok(serde_json::from_slice(data)?)
        } else {
            Err(AgentError::StateMigrationRequired { from_version, to_version: Self::version() })
        }
    }
}

/// Configuration for agent behavior.
pub trait AgentConfig: Serialize + DeserializeOwned + Default + Send + Sync {
    /// Validate configuration.
    fn validate(&self) -> Result<()> { Ok(()) }
}
```

---

## Context

The execution context provided to agents during ticks:

```rust
/// Execution context for agent ticks.
pub struct AgentContext<S: AgentState> {
    /// Mutable access to agent state.
    pub state: S,

    /// Agent's cryptographic identity (public info only).
    identity: AgentIdentity,

    /// Signing service (Factotum) - keys never exposed.
    signer: Arc<dyn SigningService>,

    /// Storage backend.
    storage: Box<dyn AgentStorage>,

    /// Message transport (convenience over drivers).
    transport: Box<dyn MessageTransport>,

    /// Connected clients (for broadcast).
    connections: Vec<ConnectionHandle>,

    /// Resource tracker.
    budget: BudgetTracker,

    /// Scheduled alarms.
    alarms: Vec<ScheduledAlarm>,

    /// Emitted events this tick.
    events: Vec<Event>,

    /// Seen envelope IDs (bounded cache for dedup).
    seen_envelopes: SeenCache,
}

impl<S: AgentState> AgentContext<S> {
    // === Identity (via SigningService - keys never exposed) ===

    /// Agent's public key.
    pub fn pubkey(&self) -> &PublicKey {
        &self.identity.pubkey
    }

    /// Sign data (delegates to signer, may fail for remote KMS/HSM).
    pub fn sign(&self, data: &[u8]) -> Result<Signature> {
        self.signer.sign(&self.identity.agent_id, data)
    }

    /// Verify signature from any key (pure crypto, always sync).
    pub fn verify(&self, pubkey: &PublicKey, data: &[u8], sig: &Signature) -> bool {
        pubkey.verify(data, sig)
    }

    /// Encrypt to recipient (NIP-44, delegates to signer).
    pub fn encrypt_to(&self, recipient: &PublicKey, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.signer.encrypt(&self.identity.agent_id, recipient, plaintext)
    }

    /// Decrypt from sender (NIP-44, delegates to signer).
    pub fn decrypt_from(&self, sender: &PublicKey, ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.signer.decrypt(&self.identity.agent_id, sender, ciphertext)
    }

    // === Idempotency (for at-least-once delivery) ===

    /// Check if envelope was already processed (bounded cache).
    pub fn seen(&self, envelope_id: &EnvelopeId) -> bool {
        self.seen_envelopes.contains(envelope_id)
    }

    /// Mark envelope as processed.
    pub fn mark_seen(&mut self, envelope_id: &EnvelopeId) {
        self.seen_envelopes.insert(envelope_id.clone());
    }

    // === Storage ===

    /// Get value from storage.
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;

    /// Set value in storage.
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()>;

    /// Delete value from storage.
    pub async fn delete(&self, key: &str) -> Result<()>;

    /// List keys with prefix.
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>>;

    /// Execute atomic transaction.
    pub async fn transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Transaction) -> Result<R>;

    // === Communication ===

    /// Send message to another agent.
    pub async fn send(&self, to: &AgentId, message: impl Into<Message>) -> Result<()>;

    /// Broadcast event to all connected clients.
    pub fn broadcast(&self, event: &str, data: impl Serialize);

    /// Publish event to relay/pub-sub.
    pub async fn publish(&self, event: Event) -> Result<()>;

    /// Request/response to another agent.
    pub async fn request<R: DeserializeOwned>(
        &self,
        to: &AgentId,
        request: impl Into<Message>,
        timeout: Duration,
    ) -> Result<R>;

    // === Scheduling ===

    /// Schedule alarm to wake agent.
    pub fn schedule_alarm(&mut self, delay: Duration, payload: Option<Vec<u8>>);

    /// Schedule recurring alarm.
    pub fn schedule_recurring(&mut self, interval: Duration, payload: Option<Vec<u8>>);

    /// Cancel scheduled alarm.
    pub fn cancel_alarm(&mut self, alarm_id: &AlarmId);

    // === Resources ===

    /// Check remaining budget.
    pub fn remaining_budget(&self) -> &BudgetStatus;

    /// Check if operation is within budget.
    pub fn can_afford(&self, cost: ResourceCost) -> bool;

    /// Record resource consumption.
    pub fn record_usage(&mut self, usage: ResourceUsage);
}
```

---

## Signing Service Trait

The Factotum-style signing service. Private keys live here, never exposed to agent code.

```rust
/// Signing service (Factotum) - holds keys, never exposes them.
///
/// This is sync by design. Backends implement via:
/// - Native: blocking call to keychain/HSM
/// - Browser: Web Crypto API (may queue internally)
/// - Cloud: KMS API call
pub trait SigningService: Send + Sync {
    /// Get the public key for an agent.
    fn pubkey(&self, agent_id: &AgentId) -> Result<PublicKey>;

    /// Sign data (agent requests signature, never sees private key).
    fn sign(&self, agent_id: &AgentId, data: &[u8]) -> Result<Signature>;

    /// Encrypt to recipient (NIP-44).
    fn encrypt(
        &self,
        agent_id: &AgentId,
        recipient: &PublicKey,
        plaintext: &[u8],
    ) -> Result<Vec<u8>>;

    /// Decrypt from sender (NIP-44).
    fn decrypt(
        &self,
        agent_id: &AgentId,
        sender: &PublicKey,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>>;
}
```

Implementations:

| Backend | SigningService Implementation |
|---------|------------------------------|
| Local dev | In-memory keys (fast, not for production) |
| Local prod | OS keychain / secure enclave |
| Browser | Web Crypto API |
| Cloudflare | Workers secrets + DO storage |
| Cloud | AWS KMS / GCP Cloud KMS |
| Threshold | FROST/FROSTR signing quorum |

---

## Storage Trait

```rust
/// Abstraction over agent state storage.
#[async_trait]
pub trait AgentStorage: Send + Sync {
    /// Load raw state bytes.
    async fn load_state(&self, agent_id: &AgentId) -> Result<Option<Vec<u8>>>;

    /// Save raw state bytes.
    async fn save_state(&self, agent_id: &AgentId, state: &[u8]) -> Result<()>;

    /// Delete agent state.
    async fn delete_state(&self, agent_id: &AgentId) -> Result<()>;

    /// Get key-value item.
    async fn get(&self, agent_id: &AgentId, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set key-value item.
    async fn set(&self, agent_id: &AgentId, key: &str, value: &[u8]) -> Result<()>;

    /// Delete key-value item.
    async fn delete(&self, agent_id: &AgentId, key: &str) -> Result<()>;

    /// List keys with prefix.
    async fn list(&self, agent_id: &AgentId, prefix: &str) -> Result<Vec<String>>;

    /// Execute transactional operations.
    async fn transaction(
        &self,
        agent_id: &AgentId,
        ops: Vec<StorageOp>,
    ) -> Result<()>;
}

/// Storage operation for transactions.
pub enum StorageOp {
    Set { key: String, value: Vec<u8> },
    Delete { key: String },
    SetState { state: Vec<u8> },
}
```

---

## Transport Trait

```rust
/// Abstraction over message transport.
#[async_trait]
pub trait MessageTransport: Send + Sync {
    /// Send message to specific agent.
    async fn send(&self, from: &AgentId, to: &AgentId, message: Message) -> Result<()>;

    /// Publish event (broadcast).
    async fn publish(&self, from: &AgentId, event: Event) -> Result<()>;

    /// Subscribe to messages for an agent.
    async fn subscribe(&self, agent_id: &AgentId) -> Result<MessageStream>;

    /// Request with response (RPC pattern).
    async fn request(
        &self,
        from: &AgentId,
        to: &AgentId,
        request: Message,
        timeout: Duration,
    ) -> Result<Message>;
}

/// Stream of incoming messages.
pub type MessageStream = Pin<Box<dyn Stream<Item = IncomingMessage> + Send>>;

/// Incoming message with metadata.
pub struct IncomingMessage {
    pub from: AgentId,
    pub message: Message,
    pub received_at: Timestamp,
}
```

---

## Backend Trait

```rust
/// Runtime backend abstraction.
#[async_trait]
pub trait RuntimeBackend: Send + Sync {
    // === Agent Lifecycle ===

    /// Create a new agent.
    async fn create_agent(&self, id: AgentId, config: AgentConfig) -> Result<()>;

    /// Destroy an agent and its state.
    async fn destroy_agent(&self, id: &AgentId) -> Result<()>;

    /// List all agents.
    async fn list_agents(&self) -> Result<Vec<AgentId>>;

    /// Get agent status.
    async fn get_status(&self, id: &AgentId) -> Result<AgentStatus>;

    // === Execution ===

    /// Wake agent with trigger.
    async fn wake(&self, id: &AgentId, trigger: Trigger) -> Result<TickResult>;

    /// Force agent to hibernate.
    async fn hibernate(&self, id: &AgentId) -> Result<()>;

    // === Resources ===

    /// Get storage for agent.
    fn storage(&self) -> Arc<dyn AgentStorage>;

    /// Get transport for messaging.
    fn transport(&self) -> Arc<dyn MessageTransport>;

    // === Observability ===

    /// Get metrics for agent.
    async fn get_metrics(&self, id: &AgentId) -> Result<AgentMetrics>;

    /// Get recent ticks for agent.
    async fn get_tick_history(&self, id: &AgentId, limit: usize) -> Result<Vec<TickRecord>>;
}

/// Agent status information.
pub struct AgentStatus {
    pub id: AgentId,
    pub state: AgentLifecycleState,
    pub created_at: Timestamp,
    pub last_tick_at: Option<Timestamp>,
    pub tick_count: u64,
    pub storage_bytes: u64,
}

/// Agent lifecycle state.
pub enum AgentLifecycleState {
    Creating,
    Active,
    Dormant,
    Failed { error: String },
    Terminated,
}
```

---

## Trigger Types

```rust
/// What caused an agent tick. Every trigger carries envelope metadata for dedup + tracing.
pub enum Trigger {
    /// Incoming message from another agent or user.
    Message(MessageTrigger),

    /// Scheduled alarm fired.
    Alarm(AlarmTrigger),

    /// External event (webhook, file change, etc.).
    Event(EventTrigger),

    /// Manual invocation (API call, CLI).
    Manual(ManualTrigger),

    /// First tick after creation.
    Initialize(InitializeTrigger),
}

/// Common metadata on all triggers (for dedup + trace correlation).
pub struct TriggerMeta {
    /// Envelope ID for idempotency (use with ctx.seen/mark_seen).
    pub envelope_id: EnvelopeId,

    /// Source system (driver name, relay URL, etc.).
    pub source: String,

    /// Sequence number for ordering (if available).
    pub seq: Option<u64>,

    /// Timestamp when envelope was created.
    pub created_at: Timestamp,
}

pub struct MessageTrigger {
    pub meta: TriggerMeta,
    pub from: AgentId,
    pub message: Message,
    pub reply_to: Option<EnvelopeId>,
}

pub struct AlarmTrigger {
    pub meta: TriggerMeta,
    pub alarm_id: AlarmId,
    pub scheduled_at: Timestamp,
    pub fired_at: Timestamp,
    pub payload: Option<Vec<u8>>,
}

pub struct EventTrigger {
    pub meta: TriggerMeta,
    pub event_type: String,
    pub payload: serde_json::Value,
}

pub struct ManualTrigger {
    pub meta: TriggerMeta,
    pub invoked_by: Option<String>,
    pub reason: Option<String>,
}

pub struct InitializeTrigger {
    pub meta: TriggerMeta,
}

impl Trigger {
    /// Get envelope ID for deduplication.
    pub fn envelope_id(&self) -> &EnvelopeId {
        match self {
            Trigger::Message(t) => &t.meta.envelope_id,
            Trigger::Alarm(t) => &t.meta.envelope_id,
            Trigger::Event(t) => &t.meta.envelope_id,
            Trigger::Manual(t) => &t.meta.envelope_id,
            Trigger::Initialize(t) => &t.meta.envelope_id,
        }
    }
}
```

---

## Tick Result

```rust
/// Result of a tick execution.
pub struct TickResult {
    /// Whether tick completed successfully.
    pub success: bool,

    /// Duration of tick execution.
    pub duration: Duration,

    /// Resources consumed.
    pub usage: ResourceUsage,

    /// Messages sent during tick.
    pub messages_sent: usize,

    /// Events emitted during tick.
    pub events_emitted: usize,

    /// Next scheduled alarm (if any).
    pub next_alarm: Option<Timestamp>,

    /// Error if tick failed.
    pub error: Option<String>,

    /// Agent should hibernate after this tick.
    pub should_hibernate: bool,
}

pub struct ResourceUsage {
    pub compute_ms: u64,
    pub storage_reads: u64,
    pub storage_writes: u64,
    pub storage_bytes_written: u64,
    pub messages_sent: u64,
    pub api_calls: u64,
}
```

---

## Budget Types

Canonical budget definitions used across the runtime.

```rust
/// Static budget policy (configured at mount/agent level).
/// All amounts in micro-USD (1 micro-USD = $0.000001).
pub struct BudgetPolicy {
    /// Maximum spend per tick (micro-USD).
    pub per_tick_usd: u64,

    /// Maximum spend per day (micro-USD).
    pub per_day_usd: u64,

    /// Spend above this requires approval (micro-USD).
    pub approval_threshold_usd: u64,

    /// Who can approve large spends.
    pub approvers: Vec<PublicKey>,
}

/// Dynamic budget state (tracked by runtime, shown in /status).
/// All amounts in micro-USD (1 micro-USD = $0.000001).
pub struct BudgetState {
    /// Amount spent this tick (micro-USD).
    pub spent_tick_usd: u64,

    /// Amount spent today (micro-USD).
    pub spent_day_usd: u64,

    /// Day boundary (resets spent_day).
    pub day_start: Timestamp,

    /// Remaining budget this tick (micro-USD).
    pub remaining_tick(&self) -> u64;

    /// Remaining budget today (micro-USD).
    pub remaining_day(&self) -> u64;
}

/// Access level for mounted capabilities.
pub enum AccessLevel {
    /// Read-only access.
    ReadOnly,

    /// Read and write access.
    ReadWrite,

    /// Sign-only (for /identity - can sign, not extract keys).
    SignOnly,

    /// Budgeted access with spending limits.
    Budgeted(BudgetPolicy),

    /// Disabled (mount exists but access denied).
    Disabled,
}
```

Usage in mount table:

```rust
namespace.mount("/wallet", wallet_fs, AccessLevel::Budgeted(BudgetPolicy {
    per_tick_sats: 1000,
    per_day_sats: 50000,
    approval_threshold_sats: 5000,
    approvers: vec![owner_pubkey],
}));
```
