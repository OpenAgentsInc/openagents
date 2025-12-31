# Spec: Autopilot Agents as Durable Objects

## Vision

Each Autopilot agent is a persistent Durable Object with its own identity, memory, and Nostr presence. User-bound ("your Autopilot") but portable - memories can transfer between contexts. Agents are first-class Nostr citizens that can connect to relays, communicate with other agents, and optionally serve as relays themselves.

## Architecture Overview

```
User
  └── Agent 1 (DO) ──── SQLite Memory ──── Nostr Keypair
  │       │                                     │
  │       ├── Relay Pool (wss://...)  ←─────────┤
  │       ├── WebSocket to UI                   │
  │       └── Container (optional heavy compute)│
  │                                             │
  └── Agent 2 (DO) ──── SQLite Memory ──── Nostr Keypair
          │                                     │
          └── Can talk to Agent 1 via Nostr ────┘
```

## 1. DO Naming & Identity

**Naming scheme:** `agent:{user_id}:{agent_id}`
- Groups agents by user for easy enumeration
- Predictable stub creation: `env.AGENT_DO.idFromName(name)`

**Nostr keypair derivation (NIP-06):**
```
User mnemonic
  └── m/44'/1237'/0'/0/0  → User's personal Nostr key
  └── m/44'/1237'/1'/0/0  → Agent 0 (primary Autopilot)
  └── m/44'/1237'/2'/0/0  → Agent 1
  └── m/44'/1237'/n+1'/0/0 → Agent n
```

Deterministic, recoverable from user seed, cryptographically separate.

## 2. SQLite Memory Schema

```sql
-- Core config
CREATE TABLE agent_config (key TEXT PRIMARY KEY, value TEXT, updated_at INTEGER);

-- Conversations (persists across sessions)
CREATE TABLE conversations (id TEXT PRIMARY KEY, started_at INTEGER, summary TEXT, tokens_used INTEGER);
CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT, role TEXT, content TEXT, created_at INTEGER);

-- File context (files agent has read)
CREATE TABLE file_context (path TEXT PRIMARY KEY, content_hash TEXT, last_read_at INTEGER, summary TEXT);

-- Learned patterns (extracted knowledge)
CREATE TABLE learned_patterns (id TEXT PRIMARY KEY, pattern_type TEXT, description TEXT, confidence REAL);

-- Nostr events cache
CREATE TABLE nostr_events (id TEXT PRIMARY KEY, pubkey TEXT, kind INTEGER, created_at INTEGER, content TEXT);

-- Goals (NIP-SA compatible)
CREATE TABLE goals (id TEXT PRIMARY KEY, description TEXT, priority INTEGER, status TEXT, progress REAL);

-- Agent peers (for multi-agent coordination)
CREATE TABLE agent_peers (npub TEXT PRIMARY KEY, name TEXT, relationship TEXT, trust_score REAL);
```

## 3. Nostr Integration

**Relay connections:** WebSocket hibernation for efficient long-lived connections
```rust
// Connect to relays, subscribe to mentions
self.state.accept_websocket_with_tags(&ws, &[relay_url]);
ws.send(&json!(["REQ", "mentions", {"#p": [agent_pubkey]}]));
```

**Agent as relay (optional):** Serve own events to authorized peers
- Accept relay protocol connections on `/relay` endpoint
- Useful for offline-first, peer-to-peer agent coordination

**Inter-agent messaging:** NIP-04 encrypted DMs with typed content
```rust
enum AgentMessage {
    TaskRequest { task_id, description, deadline, reward_sats },
    TaskResponse { task_id, accepted, reason },
    TaskComplete { task_id, result, trajectory_hash },
    PatternShare { pattern_type, description, examples },
    Ping { timestamp },
    Pong { timestamp, status },
}
```

## 4. Agent Lifecycle

```
Created → Initializing → Active ⇄ Dormant → Terminated
                           │
                    alarm() triggers
                      heartbeat ticks
```

**Tick execution:** Heartbeat, mention, DM, or manual trigger
1. Build context (goals, recent memories, patterns)
2. Execute via Claude SDK or local model
3. Update memory with results
4. Publish NIP-SA tick result (kind:39211)

## 5. Memory Portability

**Export format:** JSON with conversations, patterns, goals, peers
**Export to Nostr:** NIP-SA state event (kind:39201), encrypted to self
**Import:** Merge strategy for conversations, patterns, goals

## 6. API Endpoints

```
GET  /api/agents                    List user's agents
POST /api/agents                    Create new agent
GET  /api/agents/:id                Get agent info
DELETE /api/agents/:id              Delete agent

# Proxy to Agent DO
GET  /api/agents/:id/do/status      Agent status
POST /api/agents/:id/do/tick        Trigger tick
GET  /api/agents/:id/do/goals       List goals
POST /api/agents/:id/do/goals       Add goal
GET  /api/agents/:id/do/memory/export   Export memory
POST /api/agents/:id/do/memory/import   Import memory
GET  /api/agents/:id/ws             WebSocket to agent
```

## 7. Files to Create/Modify

| File | Purpose |
|------|---------|
| `crates/web/worker/src/agent_do.rs` | New DO implementation |
| `crates/web/worker/src/routes/agents.rs` | Agent CRUD routes |
| `crates/web/worker/src/lib.rs` | Add DO binding & routes |
| `crates/web/wrangler.toml` | Add AGENT_DO binding |
| `crates/nostr/core/src/nip06.rs` | Add `derive_agent_keypair` |
| `crates/web/worker/migrations/` | D1 agents table |

## 8. Relationship to AutopilotContainer

**Phase 1:** Coexist - AgentDO for persistent features, AutopilotContainer for legacy
**Phase 2:** AgentDO can spawn containers for heavy compute
**Phase 3:** Deprecate AutopilotContainer

## 9. Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| DO naming | `agent:{user_id}:{agent_id}` | Groups by user, predictable |
| Keypair derivation | BIP44 path m/44'/1237'/{n+1}'/0/0 | Deterministic, recoverable |
| Relay connection | WebSocket hibernation | Efficient idle handling |
| Inter-agent protocol | NIP-04 DMs with typed JSON | Secure, standard Nostr |
| Memory export | JSON + NIP-SA kind:39201 | Multiple formats |

## Implementation Order

1. D1 schema - Create agents table
2. Agent DO skeleton - Basic init, status, fetch routing
3. SQLite schema - Create memory tables on init
4. Keypair derivation - Extend nip06.rs
5. CRUD routes - Create, list, delete agents
6. Memory operations - Store/retrieve conversations, patterns
7. Nostr relay connection - Connect, subscribe, publish
8. Tick execution - Heartbeat, context building
9. Inter-agent messaging - DM protocol
10. Memory export/import - Portability
