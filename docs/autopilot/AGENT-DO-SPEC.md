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

## GTM Relevance

This architecture directly enables key GTM objectives from the [Autopilot Go-To-Market Strategy](../../../backroom/live/GTM.md):

### Personal HUD URLs → Personal Agent URLs
GTM envisions `openagents.com/hud/@username/repo` as the viral loop. With Agent-as-DO, this becomes even more personal: each user has a named, persistent agent with its own Nostr identity. The HUD shows not just "Autopilot on repo" but "YOUR Autopilot" - an entity with memory, personality, and history.

### The Hero Moment: "Wake Up to PRs"
GTM's hero moment requires an agent that works overnight and remembers context. Without persistent memory, each session starts cold. Agent-as-DO enables:
- **Continuous operation** via DO alarms (heartbeat ticks)
- **Context preservation** via SQLite memory (conversations, file context, patterns)
- **Goal tracking** that persists across sessions

### Live Fishbowl
The landing page fishbowl showing "Autopilot working on issue #847" needs a persistent agent. Agent-as-DO provides:
- Long-lived agent identity for the public fishbowl
- Memory of what it's worked on (for context continuity)
- Nostr presence so viewers can follow the agent's work

### Multi-Agent Grid (Future)
GTM mentions "Multi-agent grid with several agents working" as a screenshot-worthy moment. This architecture enables:
- Multiple agents per user
- Agents that can coordinate via Nostr DMs
- Each agent with distinct identity and specialization

### Viral Mechanics Beyond Repos
With Nostr identity, agents become shareable entities:
- Follow an agent's npub to see its work across repos
- Agents can publish their own "accomplishments" as Nostr events
- Agent reputation becomes portable (trust scores, completed tasks)

### "47 Autopilots Coding Right Now"
The GTM gallery showing active autopilots requires persistent, identifiable agents. Agent-as-DO provides the data model: each agent tracked in D1, status queryable, activity publishable to Nostr.

## Relevant Files

### Existing DO Implementations (patterns to follow)
- `crates/web/worker/src/autopilot_container.rs` - Current container DO, shows DO lifecycle and container spawning
- `crates/web/worker/src/relay.rs` - TunnelRelay DO, shows WebSocket hibernation patterns

### Nostr Core (to extend)
- `crates/nostr/core/src/nip06.rs` - BIP39/44 key derivation, add `derive_agent_keypair`
- `crates/nostr/core/src/nip_sa/mod.rs` - Sovereign Agent types (AgentProfile, AgentState, goals)
- `crates/nostr/core/src/nip04.rs` - Encrypted DMs for inter-agent messaging
- `crates/nostr/core/src/nip44.rs` - Modern encryption for state export

### Nostr Client (for relay connections)
- `crates/nostr/client/src/relay.rs` - WebSocket relay connection patterns
- `crates/nostr/client/src/pool.rs` - Multi-relay pool management
- `crates/nostr/client/src/dvm.rs` - DVM client for NIP-90 job requests

### Web Worker (to modify)
- `crates/web/worker/src/lib.rs` - Add agent routes and DO binding
- `crates/web/worker/src/routes/container.rs` - Pattern for session management with KV
- `crates/web/worker/src/routes/hud.rs` - Pattern for proxying to DOs
- `crates/web/worker/src/db/users.rs` - D1 operations pattern, identity material access
- `crates/web/wrangler.toml` - Add AGENT_DO binding and migration

### Identity & Encryption
- `crates/web/worker/src/identity.rs` - User identity encryption/decryption helpers

### Existing Migrations
- `crates/web/migrations/0001_initial.sql` - Users, billing tables
- `crates/web/migrations/0002_identity_keys.sql` - Nostr identity columns

## Connection to the Big Picture

This architecture is a critical piece of the OpenAgents vision described in [SYNTHESIS.md](../../SYNTHESIS.md). Here's how Agent-as-DO connects to the larger system:

### Sovereign Identity (FROSTR Foundation)

SYNTHESIS describes agents as **sovereign economic actors** with threshold-protected identity via FROST/FROSTR—keys that no operator can extract. Agent-as-DO is the runtime instantiation of this:

- Each agent derives its own Nostr keypair from the user's mnemonic
- The BIP44 path (`m/44'/1237'/{n+1}'/0/0`) creates cryptographically separate identities
- Future: These keys can be threshold-protected (2-of-3) where the agent, marketplace signer, and guardian must cooperate

The DO provides the persistent container where this identity lives and operates.

### NIP-SA Protocol Implementation

SYNTHESIS defines NIP-SA (Sovereign Agent Protocol) with event kinds 39200-39231. Agent-as-DO implements these:

| NIP-SA Concept | Agent-as-DO Implementation |
|----------------|---------------------------|
| AgentProfile (kind:39200) | Published on agent creation with threshold config |
| AgentState (kind:39201) | SQLite memory exported/synced to Nostr |
| AgentSchedule (kind:39202) | DO alarm() configuration |
| TickRequest/Result (kind:39210/39211) | Heartbeat execution cycle |
| TrajectorySession (kind:39230) | Conversation and tool call logging |

The SQLite schema (`goals`, `learned_patterns`, `agent_config`) maps directly to NIP-SA's AgentState structure.

### Reed's Law and Coalition Formation

SYNTHESIS emphasizes that agent networks exhibit **Reed's Law** dynamics (2^N possible coalitions vs Metcalfe's N² connections). Agent-as-DO enables this through:

1. **Peer Discovery** - `agent_peers` table tracks known agents
2. **Inter-Agent Messaging** - NIP-04 encrypted DMs with typed `AgentMessage` enum
3. **Coalition Operations** - Agents can coordinate on tasks, share patterns, delegate work

The key insight: agents don't have Dunbar's number. An agent can participate in thousands of coalitions simultaneously. Agent-as-DO provides the infrastructure for forming, tracking, and operating within coalitions.

### Economic Layer Integration

SYNTHESIS describes the Neobank treasury layer and Exchange for agent payments. Agent-as-DO connects to this:

- Agents derive Bitcoin keys from the same mnemonic (BIP44 `m/44'/0'/...`)
- `reward_sats` in `TaskRequest` messages enables bounty-driven coordination
- Agents can purchase skills and compute via NIP-90, settling in sats
- Memory export includes trajectory data valuable for training (can earn sats via contribution)

The architecture positions agents as **first-class economic actors** who can earn, spend, and budget autonomously.

### The Agent Mesh Vision

SYNTHESIS envisions a "global agent mesh" with OpenAgents infrastructure at its core. Agent-as-DO is one node in that mesh:

```
                    ┌──────────────────────────────┐
                    │     Global Agent Mesh        │
                    │  (Nostr relays, NIP-90 DVMs) │
                    └──────────────────────────────┘
                                  │
           ┌──────────────────────┼──────────────────────┐
           │                      │                      │
    ┌──────┴──────┐        ┌──────┴──────┐        ┌──────┴──────┐
    │  User A's   │        │  User B's   │        │  Provider   │
    │  Agent DO   │◄──────►│  Agent DO   │◄──────►│  DVM Node   │
    └──────┬──────┘        └──────┬──────┘        └─────────────┘
           │                      │
    ┌──────┴──────┐        ┌──────┴──────┐
    │  Agent A.1  │        │  Agent B.1  │
    │  Agent A.2  │        │  Agent B.2  │
    └─────────────┘        └─────────────┘
```

Each Agent-DO is a node in this mesh—participating in markets, coordinating with peers, executing work, and building reputation.

### From Tools to Entities

SYNTHESIS's Part Ten describes the shift "From Tools to Entities"—AI systems that persist across sessions, accumulate resources, and have career arcs. Agent-as-DO implements this transition:

| Tool Paradigm | Entity Paradigm (Agent-as-DO) |
|---------------|------------------------------|
| Stateless sessions | Persistent SQLite memory |
| No identity | Own Nostr keypair |
| No resources | Can hold/earn sats |
| No reputation | Trust scores, completed tasks |
| No learning | Patterns extracted and stored |

The architecture enables agents that **improve over time**—not through retraining, but through accumulated context, learned patterns, and reputation building.

### Trajectory Data Flywheel

SYNTHESIS describes trajectory contribution as a revenue stream where developers earn sats for training data. Agent-as-DO generates this data:

1. Every tick produces trajectory events (tool calls, reasoning, outcomes)
2. These are published to Nostr (kind:39230/39231)
3. Quality scoring evaluates completeness, complexity, reward signal
4. High-quality trajectories can be contributed for sats

The `conversations`, `messages`, and `file_context` tables capture the raw material for trajectory export.

### Why This Architecture Matters

Agent-as-DO is not just a technical choice—it's the **minimum viable infrastructure** for the OpenAgents vision:

- Without persistent identity → no reputation, no economic participation
- Without memory → no learning, no context continuity
- Without Nostr integration → no mesh participation, no coalition formation
- Without DO isolation → no multi-agent operation, no user-bound agents

This spec implements the substrate upon which sovereign AI agents become possible.
