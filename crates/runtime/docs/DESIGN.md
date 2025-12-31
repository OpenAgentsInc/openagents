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

| Concern | Cloudflare | Local | Kubernetes |
|---------|------------|-------|------------|
| **Process model** | Isolate per request | Daemon | Pod |
| **Storage** | DO SQLite | File SQLite | PostgreSQL |
| **Wake trigger** | HTTP/WS | IPC/file | HTTP/gRPC |
| **Hibernation** | Automatic | Manual | Automatic |
| **Scaling** | Automatic | N/A | HPA |

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
