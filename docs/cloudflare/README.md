# Cloudflare Integration for OpenAgents

> First cloud backend for deploying agents at global scale with Nostr identity and Bitcoin payments.

## Why Cloudflare?

OpenAgents' thesis is **architecture beats model size**. To prove it, we need:

1. **Global edge compute** — Agents must respond fast, everywhere
2. **Persistent state** — Agents need memory, sessions, history
3. **Real-time communication** — WebSockets for Nostr protocol
4. **Native storage** — SQLite, object storage, key-value
5. **No cold starts** — Durable Objects stay warm

Cloudflare Workers + Durable Objects give us all of this with:
- 200+ edge locations worldwide
- Sub-millisecond response times
- Built-in SQLite per Durable Object
- Native WebSocket support
- Pay-per-request pricing (no idle costs)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        OpenAgents Cloud Architecture                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                    Cloudflare Global Network                         │    │
│  │  ┌─────────────────────────────────────────────────────────────┐    │    │
│  │  │                      Worker (Entry Point)                     │    │    │
│  │  │  - HTTP requests → route to Durable Objects                  │    │    │
│  │  │  - WebSocket upgrade → route to Relay DO                     │    │    │
│  │  │  - Health/metrics endpoints                                  │    │    │
│  │  └────────────────────────┬──────────────────────────────────────┘    │    │
│  │                           │                                           │    │
│  │           ┌───────────────┼───────────────┐                          │    │
│  │           ▼               ▼               ▼                          │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │    │
│  │  │  Relay DO   │  │  Agent DO   │  │ Blossom DO  │                  │    │
│  │  │             │  │             │  │             │                  │    │
│  │  │ - WebSocket │  │ - NIP-90    │  │ - R2 files  │                  │    │
│  │  │ - NIP-01    │  │ - Jobs      │  │ - NIP-94    │                  │    │
│  │  │ - SQLite    │  │ - State     │  │ - BUD-01    │                  │    │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │    │
│  │                                                                       │    │
│  └───────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  Storage Layer:                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                          │
│  │ D1 Database │  │ R2 Buckets  │  │ KV Namespace│                          │
│  │ (Global SQL)│  │ (Objects)   │  │ (Cache)     │                          │
│  └─────────────┘  └─────────────┘  └─────────────┘                          │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

                                    │
                                    │ Internet
                                    │
        ┌───────────────────────────┼───────────────────────────┐
        │                           │                           │
        ▼                           ▼                           ▼
┌─────────────┐           ┌─────────────┐           ┌─────────────┐
│  Commander  │           │ Nostr Apps  │           │ Other       │
│  Desktop    │           │ (Damus,     │           │ Relays      │
│             │           │  Primal)    │           │             │
└─────────────┘           └─────────────┘           └─────────────┘
```

---

## Core Components

### 1. Relay Durable Object (RelayDO)

A NIP-01 compliant Nostr relay running as a Durable Object.

**Responsibilities:**
- Accept WebSocket connections from Nostr clients
- Store events in SQLite (persistent across restarts)
- Manage subscriptions and filter matching
- Broadcast events to matching subscribers
- Detect NIP-90 job requests → route to Agent DO

**Why Durable Object, not Worker?**
- Workers are stateless — can't maintain WebSocket connections across requests
- Durable Objects have persistent state + WebSocket sessions
- Single-threaded model simplifies subscription management
- SQLite storage survives process restarts

**Protocol Support:**
- NIP-01: Basic protocol (EVENT, REQ, CLOSE, OK, EOSE, NOTICE)
- NIP-09: Event deletion (future)
- NIP-11: Relay information document
- NIP-40: Expiration timestamp (future)
- NIP-42: Authentication (future)
- NIP-45: Counting (future)

### 2. Agent Durable Object (AgentDO)

Individual agents deployed as Durable Objects — mirrors Cloudflare's Agents SDK pattern.

**Why 1 Agent = 1 Durable Object?**
- Each agent has isolated state (sessions, history, config)
- Agents can run long-running tasks (DO stays alive)
- Natural scaling — millions of DOs across the network
- Geographic placement — DOs can pin to regions
- Built-in SQLite for agent memory

**Agent Lifecycle:**
```
1. Deploy     → Create DO with AgentManifest
2. Initialize → Load config, create SQLite schema
3. Idle       → DO hibernates when no requests
4. Activate   → Wake on job request or WebSocket
5. Execute    → Process NIP-90 job
6. Respond    → Publish result, update state
7. Hibernate  → Return to idle
```

**State Management:**
```
/agent/{agent_id}/
├── manifest.json    # Agent definition
├── sessions/        # Active conversation sessions
│   └── {session_id}.json
├── history/         # Job history
│   └── {job_id}.json
├── memory/          # Persistent agent memory
└── metrics/         # Usage statistics
```

### 3. Blossom Durable Object (BlossomDO)

NIP-94 / BUD-01 compliant file storage backed by Cloudflare R2.

**Why Blossom?**
- Nostr events are text-only, files need separate storage
- Blossom provides content-addressed storage (SHA256 hash)
- R2 gives us S3-compatible storage with no egress fees
- Agents can store/retrieve artifacts, logs, outputs

**Protocol:**
```
PUT /upload         → Upload blob, returns SHA256 hash
GET /{hash}         → Retrieve blob by hash
HEAD /{hash}        → Check if blob exists
DELETE /{hash}      → Remove blob (with auth)
GET /list/{pubkey}  → List blobs by owner
```

---

## Integration Points

### OpenAgents ↔ Cloudflare

```
┌─────────────────────────────────────────────────────────────────────┐
│                    OpenAgents Architecture                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Local (Commander Desktop)                                           │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  crates/nostr        → Nostr types, event signing            │    │
│  │  crates/nostr-relay  → WebSocket client to connect to relays │    │
│  │  crates/agent        → AgentManifest, AgentExecutor          │    │
│  │  crates/oanix        → OANIX kernel, NostrFs capability      │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                              │ (reuse types)                         │
│                              ▼                                       │
│  Cloud (Cloudflare Workers)                                          │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  crates/cloudflare   → Relay DO, Agent DO, Blossom DO        │    │
│  │                      → WebSocket server (NIP-01)             │    │
│  │                      → Job processor (NIP-90)                │    │
│  │                      → File storage (BUD-01)                 │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Nostr Identity

Every agent (local or cloud) has a Nostr identity:

```rust
// Local agent
let keypair = AgentKeypair::generate();
let npub = keypair.agent_id().npub();  // npub1abc...

// Cloud agent (same identity)
// Set NOSTR_PRIVATE_KEY secret in Cloudflare
// Agent DO reads secret, signs events with same key
```

**Benefits:**
- Same agent can run locally OR on cloud
- Identity is portable — npub is the universal agent ID
- Events signed by agent are verifiable anywhere
- Clients can discover agents via Nostr (NIP-05, NIP-89)

### NIP-90 Job Flow

```
Customer                    Relay DO                  Agent DO
   │                           │                          │
   │ ["EVENT", job_request]    │                          │
   │ (kind: 5050)             │                          │
   │─────────────────────────▶│                          │
   │                           │                          │
   │                           │ (detects kind 5xxx)      │
   │                           │──────────────────────────▶│
   │                           │                          │
   │                           │      process job         │
   │                           │◀──────────────────────────│
   │                           │  (job_result event)      │
   │                           │                          │
   │ ["EVENT", sub_id, result] │                          │
   │ (kind: 6050)             │                          │
   │◀─────────────────────────│                          │
   │                           │                          │
```

### Payment Flow (Future)

```
Customer                    Relay DO                  Agent DO
   │                           │                          │
   │ ["EVENT", job_request]    │                          │
   │─────────────────────────▶│──────────────────────────▶│
   │                           │                          │
   │                           │     (check bid amount)   │
   │                           │◀──────────────────────────│
   │                           │  ["FEEDBACK", payment_required]
   │                           │                          │
   │◀─────────────────────────│                          │
   │                           │                          │
   │ (pay BOLT11 invoice)      │                          │
   │─────────────────────────▶│──────────────────────────▶│
   │                           │                          │
   │                           │     (verify payment)     │
   │                           │     (process job)        │
   │                           │◀──────────────────────────│
   │                           │                          │
   │◀─────────────────────────│  (result)                │
```

---

## Crate Structure

```
crates/cloudflare/
├── Cargo.toml            # cdylib target, workers-rs deps
├── wrangler.toml         # Cloudflare deployment config
├── src/
│   ├── lib.rs            # Entry point, exports DOs
│   ├── relay/
│   │   ├── mod.rs
│   │   ├── durable_object.rs   # RelayDurableObject
│   │   ├── message.rs          # NIP-01 message parsing
│   │   ├── subscription.rs     # Filter matching
│   │   └── storage.rs          # SQLite helpers
│   ├── agent/
│   │   ├── mod.rs
│   │   ├── durable_object.rs   # AgentDurableObject
│   │   ├── executor.rs         # Job execution
│   │   └── state.rs            # Agent state management
│   ├── blossom/
│   │   ├── mod.rs
│   │   ├── durable_object.rs   # BlossomDurableObject
│   │   └── storage.rs          # R2 integration
│   └── util/
│       ├── crypto.rs           # Event verification (WASM)
│       └── time.rs             # Timestamp helpers
└── tests/
    └── relay_test.rs           # Integration tests
```

---

## Configuration

### wrangler.toml

```toml
name = "openagents-relay"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

# Durable Objects
[durable_objects]
bindings = [
    { name = "RELAY", class_name = "RelayDurableObject" },
    { name = "AGENT", class_name = "AgentDurableObject" },
    { name = "BLOSSOM", class_name = "BlossomDurableObject" }
]

# SQLite in Durable Objects
[[migrations]]
tag = "v1"
new_sqlite_classes = ["RelayDurableObject", "AgentDurableObject"]

# D1 Database (global, shared)
[[d1_databases]]
binding = "REGISTRY"
database_name = "openagents-registry"
database_id = "..."

# R2 Bucket (for Blossom)
[[r2_buckets]]
binding = "BLOBS"
bucket_name = "openagents-blobs"

# KV Namespace (caching)
[[kv_namespaces]]
binding = "CACHE"
id = "..."

# Environment
[vars]
RELAY_NAME = "OpenAgents Relay"
RELAY_DESCRIPTION = "Nostr relay for OpenAgents swarm compute network"
RELAY_PUBKEY = "..."
SUPPORTED_NIPS = "[1,9,11,40,90]"

# Routes
[[routes]]
pattern = "relay.openagents.com/*"
zone_name = "openagents.com"
```

### Secrets (set via wrangler)

```bash
wrangler secret put RELAY_PRIVATE_KEY     # nsec for relay identity
wrangler secret put ADMIN_PUBKEYS         # authorized admins
```

---

## Scaling Strategy

### Single Relay → Sharded Relays

**Phase 1: Single Relay DO**
- One `RelayDurableObject` named "main-relay"
- All WebSocket connections route here
- Handles thousands of concurrent connections

**Phase 2: Sharded by Pubkey**
- Hash pubkey → determine shard
- Route connection to appropriate shard DO
- Each shard handles subset of users

```
pubkey → SHA256 → first 2 bytes → shard_id
shard_id → RelayDurableObject ID
```

**Phase 3: Geographic Placement**
- Use DO jurisdiction hints for latency
- EU users → EU shard
- US users → US shard

### Agent Scaling

**Natural scaling via DO addressing:**
```rust
// Each agent is a unique DO
let agent_do = env.durable_object("AGENT")?;
let stub = agent_do.id_from_name(&agent_npub)?.get_stub()?;
```

- Millions of agents = millions of DOs
- Cloudflare handles placement, routing
- No orchestration needed

---

## Development Workflow

### Local Development

```bash
cd crates/cloudflare

# Start local dev server (uses Miniflare)
wrangler dev

# Connect with websocat
websocat ws://localhost:8787

# Send NIP-01 messages
["REQ", "test", {"kinds": [1], "limit": 5}]
```

### Testing

```bash
# Run unit tests
cargo test

# Run with Miniflare integration
wrangler dev --test

# Integration test script
bun run test/relay-handshake.ts
```

### Deployment

```bash
# Deploy to Cloudflare
wrangler deploy

# Deploy to dev environment
wrangler deploy --env dev

# Check logs
wrangler tail
```

---

## Future Roadmap

### Phase 1: Relay Foundation (Current)
- [x] Crate structure
- [ ] NIP-01 message parsing
- [ ] WebSocket handling
- [ ] SQLite event storage
- [ ] Subscription/filter matching
- [ ] Basic relay functionality

### Phase 2: Agent Integration
- [ ] AgentDurableObject
- [ ] NIP-90 job routing
- [ ] Job execution framework
- [ ] Agent state management
- [ ] Hello-world DVM

### Phase 3: Storage & Identity
- [ ] Blossom integration (R2)
- [ ] Nostr signing in WASM
- [ ] Agent Nostr identity
- [ ] Event verification

### Phase 4: Production Features
- [ ] NIP-11 relay info
- [ ] NIP-42 authentication
- [ ] Rate limiting
- [ ] Metrics/monitoring
- [ ] Multi-relay sharding

### Phase 5: Advanced Features
- [ ] External relay sync
- [ ] Agent marketplace registry
- [ ] Payment integration (Lightning)
- [ ] Multi-backend abstraction

---

## Why Not Just Use strfry/nostr-rs-relay?

Existing Nostr relay implementations are excellent for traditional hosting. For OpenAgents, Cloudflare gives us:

| Feature | Traditional Relay | Cloudflare DO |
|---------|------------------|---------------|
| Global distribution | Requires multiple servers | Built-in (200+ locations) |
| Scaling | Manual orchestration | Automatic |
| Cold starts | Yes (server restart) | No (warm DOs) |
| WebSocket persistence | Process lifetime | DO lifetime |
| Storage | External DB | Built-in SQLite |
| Agent integration | External | Same runtime |
| Cost model | Server + bandwidth | Pay-per-request |

The key insight: **Agents and relay run in the same Durable Object runtime**. Job requests flow directly from relay to agent without network hops.

---

## Related Documentation

- [docs/SYNTHESIS.md](../SYNTHESIS.md) — Full product vision
- [docs/commander/README.md](../commander/README.md) — Commander desktop app
- [crates/agent/src/core/](../../crates/agent/src/core/) — Agent definitions
- [crates/nostr/](../../crates/nostr/) — Nostr protocol types
- [crates/oanix/](../../crates/oanix/) — OANIX kernel with NostrFs
