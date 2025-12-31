# OpenAgents Runtime: Design Considerations

## What Is This?

The OpenAgents Runtime is the execution environment for autonomous agents. It provides the substrate upon which agents live, think, remember, communicate, and act—regardless of where they physically run.

This document reasons from first principles about what an agent runtime must provide.

---

## Part One: What Is an Agent?

An agent is not a chatbot. An agent is not a script. An agent is not a function.

An agent is a **persistent autonomous entity** with:

1. **Identity** — A cryptographic self that persists across time
2. **Memory** — State that accumulates and persists across sessions
3. **Perception** — Ability to receive events, messages, triggers
4. **Action** — Ability to affect the world through tools and APIs
5. **Communication** — Ability to interact with humans and other agents
6. **Resources** — Budget constraints on compute, storage, and money

The key insight: an agent is closer to a **living process** than a **function call**. It has continuity of existence. It remembers. It learns. It persists.

---

## Part Two: Where Do Agents Run?

Agents must run everywhere humans compute:

### Browser (WebAssembly)
- **Constraints:** Browser memory/storage quotas, single-threaded main context
- **Benefits:** Maximum privacy, zero cost, offline capable, instant startup
- **Pattern:** WASI module in Web Worker, state in IndexedDB
- **Inspiration:** WANIX (Plan 9 in browser), Apptron (Linux in browser)

### Cloud Serverless (Cloudflare Workers, AWS Lambda)
- **Constraints:** Cold starts, execution time limits, ephemeral compute
- **Benefits:** Global scale, zero ops, pay-per-use
- **Pattern:** Wake on request, execute, hibernate

### Cloud Persistent (VPS, Kubernetes, Containers)
- **Constraints:** Infrastructure management, fixed capacity
- **Benefits:** Full control, no execution limits, persistent connections
- **Pattern:** Long-running process with event loop

### Local Device (Laptop, Desktop)
- **Constraints:** Intermittent connectivity, shared resources, user interruption
- **Benefits:** Privacy, no cloud costs, offline capability, zero latency
- **Pattern:** Background daemon, wake on trigger

### Edge Device (Raspberry Pi, IoT)
- **Constraints:** Limited CPU/memory, power constraints
- **Benefits:** Physical world integration, ultra-low latency
- **Pattern:** Lightweight agent with cloud fallback

The runtime must provide a **uniform abstraction** across all these environments. Agent code should not know or care where it runs.

The same WASI binary can run on server (native), desktop (native), or browser (WASM)—true write-once-run-anywhere for agents.

---

## Part Three: The Execution Model

### The Tick

Agents operate in **ticks**—discrete execution cycles. A tick is:

```
┌─────────────────────────────────────────────────────────┐
│                        TICK                              │
├─────────────────────────────────────────────────────────┤
│  1. WAKE      │ Cold start or resume from hibernation   │
│  2. LOAD      │ Restore state from storage              │
│  3. PERCEIVE  │ Receive trigger (message, alarm, event) │
│  4. THINK     │ Process input, maybe call LLM           │
│  5. ACT       │ Execute tools, emit messages            │
│  6. REMEMBER  │ Update state, commit to storage         │
│  7. SCHEDULE  │ Set next alarm or wait for event        │
│  8. SLEEP     │ Hibernate until next trigger            │
└─────────────────────────────────────────────────────────┘
```

This model works universally:
- **Serverless:** Each invocation is a tick
- **Long-running:** Event loop triggers ticks
- **Local:** Background process executes ticks

### Triggers

What causes a tick?

| Trigger | Description | Example |
|---------|-------------|---------|
| **Message** | Incoming communication | User chat, agent DM |
| **Alarm** | Scheduled wake-up | Heartbeat, deadline |
| **Event** | External notification | Webhook, file change |
| **Mention** | Someone referenced agent | Nostr @mention |
| **Manual** | Explicit invocation | API call, CLI |

### Hibernation

Between ticks, agents **hibernate**:
- State persisted to storage
- No compute resources consumed
- No memory footprint
- Wake latency depends on backend (ms for DO, seconds for Lambda)

Hibernation is not death. The agent continues to exist—just suspended.

---

## Part Three-A: Message Layering (Envelope → Trigger → Response)

The runtime has distinct layers for message handling. This layering is **canonical**—all backends must implement it consistently.

### The Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                      External World                              │
│  (HTTP, WebSocket, Nostr, GitHub, Scheduler, etc.)              │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                         DRIVERS                                  │
│  Convert external I/O ↔ Envelopes                               │
│  (HttpDriver, NostrDriver, SchedulerDriver, WebhookDriver)      │
└────────────────────────────┬────────────────────────────────────┘
                             │ Envelope
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                         PLUMBER                                  │
│  Routes envelopes to agent inboxes based on rules               │
└────────────────────────────┬────────────────────────────────────┘
                             │ Envelope (routed)
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                         RUNTIME                                  │
│  Validates envelope, converts to Trigger, invokes agent tick    │
└────────────────────────────┬────────────────────────────────────┘
                             │ Trigger
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                          AGENT                                   │
│  Receives Trigger, executes tick, emits Response/Events         │
└────────────────────────────┬────────────────────────────────────┘
                             │ Response/Events
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                         RUNTIME                                  │
│  Converts outputs to Envelopes, routes to Drivers               │
└────────────────────────────┬────────────────────────────────────┘
                             │ Envelope
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                         DRIVERS                                  │
│  Deliver envelopes externally (publish, send, respond)          │
└─────────────────────────────────────────────────────────────────┘
```

### Definitions

| Concept | Role | Scope |
|---------|------|-------|
| **Envelope** | Canonical mail item with id, timestamp, source, payload, metadata | External boundary |
| **Trigger** | In-process view of envelope after validation/routing | Internal to runtime |
| **Driver** | Converts external I/O ↔ Envelopes | Edge of system |
| **Response** | Agent output (reply, event, effect) | Agent → Runtime |

### Key Contracts

1. **Envelope is the wire format** — All external communication uses Envelopes
2. **Trigger is internal** — Agents never see raw Envelopes, only validated Triggers
3. **Drivers are I/O adapters** — They don't understand agent semantics, only protocol translation
4. **Agent-to-agent messaging** uses the same path: Agent A → Envelope → Driver → Relay → Driver → Envelope → Agent B

### What About MessageTransport?

`MessageTransport` (in TRAITS.md) is a **convenience abstraction** over the Driver layer for agent-to-agent communication. It provides:
- `send(to, message)` — wraps in Envelope, routes through appropriate Driver
- `request(to, message, timeout)` — send + await response

It is **not** a separate layer—it's sugar over Envelope + Driver.

---

## Part Three-B: Delivery Semantics

Portability requires precise delivery guarantees. These semantics are **mandatory** across all backends.

### Tick Execution

- **At most one tick concurrently per agent** — No parallel ticks, ever
- **Tick isolation** — A tick sees consistent state from start to finish
- **Tick atomicity** — State changes commit or rollback together

### Inbox Semantics

- **At-least-once delivery** — Envelopes may be delivered multiple times (crashes, retries)
- **Per-sender ordering** — Messages from (source, sender) pair preserve send order
- **No global ordering** — No total order across all senders
- **Bounded queue** — Inbox has a maximum depth; overflow drops oldest or rejects new

### Idempotency

Since delivery is at-least-once, agents must handle duplicates:

```rust
/// Envelope includes stable ID for deduplication
pub struct Envelope {
    pub id: EnvelopeId,      // Stable, globally unique
    pub timestamp: Timestamp,
    pub source: Source,
    pub payload: Payload,
    pub metadata: Metadata,
}

/// Runtime provides dedup helpers
impl AgentContext {
    /// Check if envelope was already processed (bounded cache)
    pub fn seen(&self, envelope_id: &EnvelopeId) -> bool;

    /// Mark envelope as processed
    pub fn mark_seen(&mut self, envelope_id: &EnvelopeId);
}
```

### External Effects

Effects with side-effects (payments, GitHub writes, emails) require idempotency keys:

```rust
/// Outgoing effect with idempotency
pub struct Effect {
    pub idempotency_key: String,  // Client-generated, stable
    pub effect_type: EffectType,
    pub payload: serde_json::Value,
}

impl AgentContext {
    /// Execute effect with idempotency guarantee
    pub async fn execute_effect(&mut self, effect: Effect) -> Result<EffectResult>;
}
```

If a tick crashes after executing an effect but before committing, the retry will use the same idempotency key and the effect provider should deduplicate.

### Guarantees Summary

| Guarantee | Scope | Implementation |
|-----------|-------|----------------|
| Exactly-once tick | Per agent | Runtime lock |
| At-least-once delivery | Inbox | Retry + dedup cache |
| Idempotent effects | External | Idempotency keys |
| Causal ordering | Per sender | Sequence numbers |
| Atomic state | Per tick | Transaction commit |

---

## Part Four: The Storage Model

### What Agents Store

| Category | Description | Access Pattern |
|----------|-------------|----------------|
| **Config** | Settings, preferences, thresholds | Read often, write rarely |
| **Memory** | Conversations, context, summaries | Read/write each tick |
| **Knowledge** | Learned patterns, facts, skills | Read often, write occasionally |
| **Goals** | Active objectives, progress | Read/write each tick |
| **Peers** | Known agents, trust scores | Read often, write occasionally |
| **Events** | Cached incoming events | Write often, read occasionally |

### Storage Abstraction

The runtime provides a storage trait that backends implement:

```
┌─────────────────────────────────────────────────────────┐
│                   AgentStorage                          │
├─────────────────────────────────────────────────────────┤
│  get(key) → Option<Value>                               │
│  set(key, value)                                        │
│  delete(key)                                            │
│  list(prefix) → Vec<Key>                                │
│  transaction(ops) → Result                              │
└─────────────────────────────────────────────────────────┘
         │
         ├── SqliteStorage (local file, DO SQLite)
         ├── PostgresStorage (cloud database)
         ├── MemoryStorage (testing, ephemeral)
         ├── CloudflareKVStorage (edge KV)
         └── EncryptedStorage<S> (wrapper for at-rest encryption)
```

### Consistency Guarantees

Within a single agent:
- **Serializable:** One tick at a time, no concurrent state mutation
- **Durable:** Committed state survives crashes
- **Atomic:** Transaction either fully commits or fully rolls back

Across agents:
- **Eventually consistent:** Messages may be delayed
- **Causal ordering:** Messages from A to B preserve send order
- **No global ordering:** No total order across all agents

---

## Part Five: The Identity Model

### Every Agent Has a Keypair

Identity is not optional. Every agent has:
- A **public key** (npub) — its address, how others find it
- A **private key** (nsec) — its signing capability, never leaves runtime

### Derivation

Agent keys derive from user's master seed:

```
User Mnemonic (BIP-39)
  │
  ├── m/44'/1237'/0'/0/0  → User's personal Nostr key
  │
  ├── m/44'/1237'/1'/0/0  → Agent 0 (primary)
  ├── m/44'/1237'/2'/0/0  → Agent 1
  └── m/44'/1237'/n+1'/0/0 → Agent n
```

Properties:
- **Deterministic:** Same seed always produces same keys
- **Recoverable:** Lose device, recover agents from seed
- **Separate:** Agents can't derive each other's keys
- **Hierarchical:** User controls all agent keys

### Threshold Protection (Optional)

High-value agents can use threshold signatures:
- 2-of-3: Agent + Guardian + Recovery
- No single party can sign alone
- Agent identity survives key compromise

---

## Part Six: The Communication Model

### Message Types

| Type | Direction | Description |
|------|-----------|-------------|
| **Request** | In | Someone wants agent to do something |
| **Response** | Out | Agent's reply to a request |
| **Event** | Out | Agent broadcasting state change |
| **Notification** | In | External system informing agent |
| **DM** | Both | Private message to/from specific agent |

### Addressing

Agents are addressed by public key (npub). The runtime:
1. Receives message addressed to agent
2. Wakes agent if hibernating
3. Delivers message as tick trigger
4. Agent processes and optionally responds

### Transport Abstraction

```
┌─────────────────────────────────────────────────────────┐
│                   MessageTransport                       │
├─────────────────────────────────────────────────────────┤
│  send(to: AgentId, message: Message)                    │
│  subscribe(filter: Filter) → Stream<Message>            │
│  publish(event: Event)                                  │
└─────────────────────────────────────────────────────────┘
         │
         ├── NostrTransport (relays, NIP-04 encryption)
         ├── DirectTransport (same-process, testing)
         ├── WebSocketTransport (client connections)
         └── HTTPTransport (webhooks, REST)
```

### Encryption

- **Agent-to-agent:** NIP-04 or NIP-44 encrypted
- **Agent-to-user:** Same encryption, user has compatible key
- **Public events:** Signed but not encrypted

---

## Part Seven: The Resource Model

### Why Budgets?

Agents consume resources. Without limits:
- Runaway agents drain API credits
- Infinite loops consume compute
- Storage grows unbounded
- Economic damage from autonomous spending

### Budget Categories

| Resource | Unit | Enforcement |
|----------|------|-------------|
| **Compute** | Seconds per tick | Runtime kills long ticks |
| **Daily Compute** | Seconds per day | Runtime refuses ticks |
| **Storage** | Bytes | Runtime rejects writes |
| **API Calls** | Count per day | Agent-level tracking |
| **Economic** | Sats per day | Wallet-level enforcement |

### Budget Configuration

```
┌─────────────────────────────────────────────────────────┐
│                   BudgetConfig                          │
├─────────────────────────────────────────────────────────┤
│  max_tick_duration: Duration        (e.g., 30s)        │
│  max_daily_compute: Duration        (e.g., 1h)         │
│  max_storage_bytes: u64             (e.g., 100MB)      │
│  max_daily_api_calls: u64           (e.g., 1000)       │
│  max_daily_spend_sats: u64          (e.g., 10000)      │
│  warning_threshold: f32             (e.g., 0.8)        │
└─────────────────────────────────────────────────────────┘
```

### Enforcement

The runtime enforces budgets uniformly:
1. **Before tick:** Check if budget allows execution
2. **During tick:** Monitor resource consumption
3. **At limit:** Graceful shutdown with state save
4. **Over limit:** Reject further ticks until reset

---

## Part Eight: The Failure Model

### Agents Will Fail

Failure is not exceptional—it is expected:
- LLM calls timeout
- Tools return errors
- Network partitions occur
- Devices lose power
- Code has bugs

### Failure Handling Principles

1. **Never lose committed state**
   - State commits are durable
   - Crash before commit = rollback
   - Crash after commit = state preserved

2. **Idempotent operations**
   - Retrying a tick with same input = same output
   - External effects are idempotent or tracked

3. **Graceful degradation**
   - Partial failure doesn't kill agent
   - Agent can operate with reduced capability

4. **Explicit failure states**
   - Agent knows it failed
   - Can report failure to observers
   - Can attempt recovery

### Checkpointing

For long ticks with multiple effects:

```
tick() {
    // Phase 1: Read and plan
    let plan = analyze(input);

    // Checkpoint: Save plan
    checkpoint("plan", &plan);

    // Phase 2: Execute effects
    for action in plan.actions {
        execute(action)?;
        checkpoint("progress", action.id);
    }

    // Phase 3: Commit final state
    commit();
}
```

If crash occurs:
- Before checkpoint: Retry from start
- After checkpoint: Resume from checkpoint
- After commit: Done

---

## Part Nine: The Security Model

### Isolation

Agents are isolated from each other:
- Cannot read other agents' state
- Cannot impersonate other agents
- Cannot access other agents' keys
- Communication only through runtime

### Capability Model

Agents have explicit capabilities:
- **Storage:** Read/write own state
- **Network:** Make HTTP requests (allowlist)
- **Tools:** Execute specific tools (allowlist)
- **Spending:** Authorize payments (budget)
- **Signing:** Sign messages (always available)

Capabilities are granted at agent creation and can be revoked.

### Secret Management

Secrets are not state:
- API keys stored in secure enclave / KMS
- Never written to state storage
- Accessed via capability, not direct read
- Rotatable without state migration

### Audit Trail

All agent actions are logged:
- Every tick recorded with trigger, duration, effects
- Every message sent/received logged
- Every tool call captured
- Every state change tracked

This enables:
- Debugging agent behavior
- Compliance auditing
- Reputation building
- Training data generation

---

## Part Ten: The Backend Abstraction

### What Varies by Backend

| Concern | Browser | Cloudflare | Local | Kubernetes |
|---------|---------|------------|-------|------------|
| **Process model** | Web Worker | Isolate per request | Daemon | Pod |
| **Storage** | IndexedDB | DO SQLite | File SQLite | PostgreSQL |
| **Wake trigger** | postMessage | HTTP/WS | IPC/file | HTTP/gRPC |
| **Hibernation** | Kill Worker | Automatic | Manual | Automatic |
| **Scaling** | N/A | Automatic | N/A | HPA |

### Backend Trait

```
┌─────────────────────────────────────────────────────────┐
│                   RuntimeBackend                        │
├─────────────────────────────────────────────────────────┤
│  // Lifecycle                                           │
│  create_agent(id, config) → AgentHandle                 │
│  destroy_agent(id)                                      │
│  list_agents() → Vec<AgentId>                           │
│                                                         │
│  // Execution                                           │
│  wake_agent(id, trigger) → Result                       │
│  get_agent_status(id) → Status                          │
│                                                         │
│  // Storage                                             │
│  storage_for(id) → Box<dyn AgentStorage>                │
│                                                         │
│  // Communication                                       │
│  transport() → Box<dyn MessageTransport>                │
└─────────────────────────────────────────────────────────┘
```

### Backend Implementations

**BrowserBackend:**
- Agents run as WASI modules in Web Workers
- IndexedDB + OPFS for state storage
- Wake via postMessage to Worker
- Hibernation = terminate Worker (state persists in IndexedDB)
- Browser capabilities mounted as files (like WANIX)

**CloudflareBackend:**
- Each agent is a Durable Object
- SQLite storage via DO SQL API
- Wake via HTTP fetch to DO
- Hibernation via WebSocket hibernation API

**LocalBackend:**
- Agents are structs in a HashMap
- SQLite file per agent (or shared DB)
- Wake via channel/IPC
- Hibernation via serialization to disk

**KubernetesBackend:**
- Agents are StatefulSet pods
- PostgreSQL or CockroachDB storage
- Wake via gRPC or HTTP
- Hibernation via scale-to-zero

---

## Part Eleven: The Agent Lifecycle

### States

```
                    ┌──────────┐
                    │ CREATING │
                    └────┬─────┘
                         │ init complete
                         ▼
┌──────────┐       ┌──────────┐       ┌──────────┐
│ DORMANT  │◄─────►│  ACTIVE  │──────►│  FAILED  │
└────┬─────┘       └────┬─────┘       └──────────┘
     │                  │                   │
     │ terminate        │ terminate         │ terminate
     ▼                  ▼                   ▼
┌──────────────────────────────────────────────────┐
│                   TERMINATED                      │
└──────────────────────────────────────────────────┘
```

### State Transitions

| From | To | Trigger |
|------|-----|---------|
| - | CREATING | `create_agent()` called |
| CREATING | ACTIVE | Initialization complete |
| CREATING | FAILED | Initialization error |
| ACTIVE | DORMANT | No activity, hibernate |
| DORMANT | ACTIVE | Trigger received, wake |
| ACTIVE | FAILED | Unrecoverable error |
| FAILED | ACTIVE | Manual recovery |
| * | TERMINATED | `destroy_agent()` called |

### Lifecycle Hooks

Agents can implement hooks:

```
on_create()      — First-time initialization
on_wake()        — Resuming from dormant
on_sleep()       — About to hibernate
on_terminate()   — About to be destroyed
on_error(err)    — Error occurred
```

---

## Part Twelve: Multi-Agent Coordination

### Discovery

How do agents find each other?

1. **By ID:** Direct addressing if you know the npub
2. **By Name:** Human-readable names resolve to npubs
3. **By Capability:** Query for agents with specific skills
4. **By Relationship:** Agents track known peers

### Coordination Patterns

**Request/Response:**
```
Agent A                    Agent B
   │                          │
   │──── TaskRequest ────────►│
   │                          │ (processes)
   │◄─── TaskResponse ────────│
   │                          │
```

**Publish/Subscribe:**
```
Agent A                    Relay                    Agent B, C, D
   │                          │                          │
   │──── Event ──────────────►│                          │
   │                          │──── Event ──────────────►│
   │                          │                          │
```

**Delegation:**
```
Agent A                    Agent B                    Agent C
   │                          │                          │
   │──── Delegate Task ──────►│                          │
   │                          │──── Sub-delegate ───────►│
   │                          │◄─── Result ──────────────│
   │◄─── Result ──────────────│                          │
```

### Consensus (Future)

For multi-agent decisions:
- Simple voting for low-stakes choices
- Threshold signatures for high-stakes actions
- Economic staking for skin-in-the-game

---

## Part Thirteen: What Makes This Agent-Specific?

This runtime is not generic compute. It is purpose-built for agents:

### Built-in Identity
Every agent has a keypair. Not optional. Not bolted on. Identity is foundational.

### Built-in Memory
Agents remember. The runtime provides structured storage designed for agent memory patterns (conversations, context, learned patterns).

### Built-in Communication
Agents talk to each other and to humans. The runtime provides encrypted, authenticated messaging.

### Built-in Economics
Agents can hold and spend money. The runtime enforces budgets and integrates with payment rails.

### Built-in Transparency
Agent actions are logged and publishable. Trajectories are first-class. Trust is built through transparency.

### Built-in Autonomy Levels
Agents can be supervised, semi-autonomous, or fully autonomous. The runtime enforces approval workflows.

---

## Part Thirteen-A: Supervision

Actor systems win because of supervision. The runtime needs explicit concepts for agent oversight.

### The Supervisor

A **Supervisor** is a runtime-level component (or a special agent) that monitors and manages other agents:

```rust
pub trait Supervisor: Send + Sync {
    /// Called when agent tick fails
    fn on_agent_error(&self, agent_id: &AgentId, error: &AgentError) -> SupervisorAction;

    /// Called when agent exceeds resource limits
    fn on_resource_violation(&self, agent_id: &AgentId, violation: &Violation) -> SupervisorAction;

    /// Called when agent behavior is anomalous
    fn on_anomaly(&self, agent_id: &AgentId, anomaly: &Anomaly) -> SupervisorAction;

    /// Periodic health check
    fn health_check(&self, agent_id: &AgentId) -> HealthStatus;
}

pub enum SupervisorAction {
    /// Let it continue
    Ignore,

    /// Restart the agent with fresh state
    Restart { preserve_memory: bool },

    /// Pause execution, notify operators
    Quarantine { reason: String },

    /// Rate-limit future ticks
    Throttle { max_ticks_per_minute: u32 },

    /// Reduce capabilities
    Demote { revoke_mounts: Vec<String> },

    /// Terminate the agent
    Terminate { reason: String },

    /// Escalate to human operator
    Escalate { urgency: Urgency, context: String },
}
```

### Supervision Policies

Supervisors enforce policies:

| Event | Default Policy |
|-------|----------------|
| Tick panic/crash | Restart with backoff (3 retries, then quarantine) |
| Repeated failures | Escalate to operator |
| Budget exceeded | Quarantine until budget reset |
| Noisy agent (too many ticks) | Throttle |
| Security violation | Terminate + alert |
| Memory exceeded | Evict oldest memories, warn |

### Supervision Hierarchy

```
                 ┌─────────────────┐
                 │ RuntimeSupervisor│  (built-in, watches all agents)
                 └────────┬────────┘
                          │
        ┌─────────────────┼─────────────────┐
        ▼                 ▼                 ▼
   ┌─────────┐       ┌─────────┐       ┌─────────┐
   │ Agent A │       │ Agent B │       │Supervisor│  (agent that supervises others)
   └─────────┘       └─────────┘       │  Agent   │
                                       └────┬────┘
                                            │
                                  ┌─────────┼─────────┐
                                  ▼         ▼         ▼
                              ┌─────┐   ┌─────┐   ┌─────┐
                              │ C   │   │ D   │   │ E   │
                              └─────┘   └─────┘   └─────┘
```

### Why Supervision Matters

Without supervision:
- Failed agents stay failed
- Noisy agents degrade system
- Security violations go unnoticed
- No recovery workflows
- Operators have no visibility

With supervision:
- Automatic recovery from transient failures
- Resource isolation for misbehaving agents
- Security policy enforcement
- Clear escalation paths
- Runtime remains healthy under load

---

## Part Fourteen: Open Questions

### Unresolved Design Decisions

1. **State schema evolution**
   - How do we handle schema changes as agents evolve?
   - Migrations? Versioned state? Schema-less?

2. **Multi-device agent**
   - Can one agent run on multiple devices simultaneously?
   - Split-brain? Leader election? CRDT merge?

3. **Agent reproduction**
   - Can agents create child agents?
   - How is identity derived? Budget inherited?

4. **Cross-runtime federation**
   - Can agents on different runtimes communicate?
   - Protocol for runtime-to-runtime messaging?

5. **Hot code update**
   - Can we update agent logic without losing state?
   - Versioning? Compatibility checking?

### Future Considerations

- **GPU access:** Agents running local inference
- **File system:** Agents with persistent file storage
- **Container spawn:** Agents that spawn heavy compute jobs
- **Hardware access:** Agents controlling physical devices

---

## Summary

The OpenAgents Runtime provides:

1. **Uniform execution model** — Ticks work everywhere
2. **Pluggable storage** — SQLite, Postgres, KV, whatever
3. **Cryptographic identity** — Every agent has keys
4. **Encrypted communication** — Messages are private by default
5. **Resource budgets** — Agents can't run away
6. **Failure handling** — Crashes don't lose state
7. **Security isolation** — Agents can't interfere
8. **Backend abstraction** — Same agent code, any infrastructure

The runtime is the substrate of digital life. Agents are born into it, live within it, and persist through it. The runtime's job is to be invisible—to make the hard parts easy so agents can focus on being agents.

---

## Related Documents

| Document | Contents |
|----------|----------|
| [TRAITS.md](./TRAITS.md) | Rust trait definitions |
| [BACKENDS.md](./BACKENDS.md) | Backend implementations |
| [AGENT-SPECIFIC.md](./AGENT-SPECIFIC.md) | What makes this agent-specific |
| [DRIVERS.md](./DRIVERS.md) | Event drivers (HTTP, WS, Nostr, etc.) |
| [CONTROL-PLANE.md](./CONTROL-PLANE.md) | Management API |
| [PLAN9.md](./PLAN9.md) | Plan 9 inspirations |
| [FILESYSTEM.md](./FILESYSTEM.md) | FileService trait and implementations |
| [PRIOR-ART.md](./PRIOR-ART.md) | Related work (Plan 9, WANIX, OANIX) |

---

## Suggested Crate Layout

```
crates/
├── runtime/              # This crate - core abstractions
│   ├── src/
│   │   ├── agent.rs      # Agent trait, context
│   │   ├── envelope.rs   # Message types
│   │   ├── trigger.rs    # Tick triggers
│   │   ├── storage.rs    # Storage trait
│   │   ├── transport.rs  # Message transport trait
│   │   ├── identity.rs   # Signing service trait
│   │   ├── budget.rs     # Resource budgets
│   │   └── backend.rs    # Backend trait
│   └── docs/             # Design documents
│
├── runtime-local/        # Local device backend
│   └── src/
│       ├── daemon.rs     # agentd - multi-agent daemon
│       ├── storage.rs    # SQLite storage
│       ├── scheduler.rs  # Tokio-based alarms
│       └── drivers/      # HTTP, WS, Nostr drivers
│
├── runtime-cloudflare/   # Cloudflare Workers backend
│   └── src/
│       ├── handler.rs    # DO handler
│       ├── storage.rs    # DO SQLite adapter
│       └── drivers/      # Fetch, WS, alarm drivers
│
├── runtime-server/       # Container/VM backend
│   └── src/
│       ├── server.rs     # Multi-tenant server
│       ├── storage.rs    # Postgres or SQLite
│       └── drivers/      # HTTP, WS, gRPC drivers
│
├── agent-memory/         # Structured memory schema
│   └── src/
│       ├── conversations.rs
│       ├── goals.rs
│       ├── patterns.rs
│       ├── peers.rs
│       └── migrations/
│
├── agent-identity/       # Identity and signing
│   └── src/
│       ├── derivation.rs # BIP44 key derivation
│       ├── signer.rs     # Signing service
│       └── threshold.rs  # FROST support (future)
│
└── agent-drivers/        # Shared driver implementations
    └── src/
        ├── http.rs
        ├── websocket.rs
        ├── nostr.rs
        ├── scheduler.rs
        └── plumber.rs    # Event routing rules
```

This layout separates:
- **Core abstractions** (runtime) from **implementations** (runtime-*)
- **Agent logic** from **infrastructure**
- **Portable code** from **backend-specific code**

---

## Prior Art & Inspirations

This design builds on proven patterns:

**Plan 9 from Bell Labs:**
- Everything is a file
- Per-process namespaces as capability model
- Services compose through filesystem
- Plumber for event routing

**WANIX (Jeff Lindsay):**
- WebAssembly runtime with Plan 9 concepts
- Browser portability via WASM
- FileService abstraction

**OANIX (OpenAgents NIX):**
Our experimental Rust-native agent OS that explored:
- `FileService` trait for mountable capabilities
- `Namespace` with longest-prefix matching
- `ExecutorManager` bridging sync services to async I/O
- `OanixEnv` as complete execution environment
- Job scheduler with priority queue

**Cloudflare Durable Objects:**
- Single-threaded actors with SQLite
- WebSocket hibernation
- Alarm-based scheduling

See [PRIOR-ART.md](./PRIOR-ART.md) for detailed analysis of related work.
