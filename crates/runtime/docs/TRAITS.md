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

    /// Agent's cryptographic identity.
    identity: AgentIdentity,

    /// Storage backend.
    storage: Box<dyn AgentStorage>,

    /// Message transport.
    transport: Box<dyn MessageTransport>,

    /// Connected clients (for broadcast).
    connections: Vec<ConnectionHandle>,

    /// Resource tracker.
    budget: BudgetTracker,

    /// Scheduled alarms.
    alarms: Vec<ScheduledAlarm>,

    /// Emitted events this tick.
    events: Vec<Event>,
}

impl<S: AgentState> AgentContext<S> {
    // === Identity ===

    /// Agent's public key.
    pub fn pubkey(&self) -> &PublicKey;

    /// Sign data with agent's key.
    pub fn sign(&self, data: &[u8]) -> Signature;

    /// Verify signature from another key.
    pub fn verify(&self, pubkey: &PublicKey, data: &[u8], sig: &Signature) -> bool;

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
/// What caused an agent tick.
pub enum Trigger {
    /// Incoming message from another agent or user.
    Message(IncomingMessage),

    /// Scheduled alarm fired.
    Alarm(AlarmTrigger),

    /// External event (webhook, file change, etc.).
    Event(ExternalEvent),

    /// Manual invocation (API call, CLI).
    Manual(ManualTrigger),

    /// First tick after creation.
    Initialize,
}

pub struct AlarmTrigger {
    pub alarm_id: AlarmId,
    pub scheduled_at: Timestamp,
    pub fired_at: Timestamp,
    pub payload: Option<Vec<u8>>,
}

pub struct ExternalEvent {
    pub source: String,
    pub event_type: String,
    pub payload: serde_json::Value,
}

pub struct ManualTrigger {
    pub invoked_by: Option<String>,
    pub reason: Option<String>,
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
