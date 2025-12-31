# Control Plane

The portable API for managing agents across runtimes.

---

## Overview

Every runtime implements the same control plane API, enabling:
- List, create, delete agents
- Route messages to agents
- Move agents between runtimes
- Monitor and debug agents

This makes "portable" operational, not theoretical.

---

## Control API

### HTTP Endpoints

```
# Agent Lifecycle
POST   /agents                    Create agent
GET    /agents                    List agents
GET    /agents/{id}               Get agent info
DELETE /agents/{id}               Delete agent

# Agent Control
POST   /agents/{id}/send          Send envelope to inbox
POST   /agents/{id}/tick          Trigger manual tick
POST   /agents/{id}/hibernate     Force hibernation
POST   /agents/{id}/wake          Force wake

# Agent State
GET    /agents/{id}/status        Current status
GET    /agents/{id}/goals         List goals
POST   /agents/{id}/goals         Add goal
GET    /agents/{id}/memory        Memory summary
GET    /agents/{id}/export        Export full bundle
POST   /agents/{id}/import        Import/restore bundle

# Observability
GET    /agents/{id}/logs          Recent logs
GET    /agents/{id}/ticks         Tick history
GET    /agents/{id}/stream        SSE event stream
GET    /agents/{id}/metrics       Prometheus metrics

# Runtime Info
GET    /runtime/status            Runtime health
GET    /runtime/agents/count      Agent count
GET    /runtime/metrics           Runtime metrics
```

### CLI Commands

```bash
# Agent lifecycle
agentctl create <name> [--config config.yaml]
agentctl list [--status active|dormant|all]
agentctl info <agent-id>
agentctl delete <agent-id>

# Agent control
agentctl send <agent-id> <message>
agentctl tick <agent-id>
agentctl hibernate <agent-id>
agentctl wake <agent-id>

# Agent state
agentctl status <agent-id>
agentctl goals <agent-id> [add|remove|list]
agentctl memory <agent-id> [--format json|summary]
agentctl export <agent-id> > bundle.tar
agentctl import < bundle.tar

# Observability
agentctl logs <agent-id> [--follow]
agentctl ticks <agent-id> [--limit N]
agentctl stream <agent-id>

# Runtime
agentctl runtime status
agentctl runtime metrics
```

---

## Create Agent

```http
POST /agents
Content-Type: application/json

{
  "name": "my-agent",
  "config": {
    "autonomy_level": "semi_autonomous",
    "budget": {
      "max_daily_sats": 50000,
      "max_per_tick_sats": 1000
    },
    "mounts": {
      "/repo": {
        "type": "git",
        "url": "https://github.com/user/repo"
      }
    }
  },
  "initial_goals": [
    {
      "description": "Monitor repository for issues",
      "priority": 1
    }
  ]
}
```

Response:
```json
{
  "id": "agent_abc123",
  "name": "my-agent",
  "pubkey": "npub1...",
  "status": "creating",
  "created_at": "2025-01-01T00:00:00Z"
}
```

---

## Agent Status

```http
GET /agents/{id}/status
```

Response:
```json
{
  "id": "agent_abc123",
  "name": "my-agent",
  "state": "active",
  "pubkey": "npub1...",
  "created_at": "2025-01-01T00:00:00Z",
  "last_tick_at": "2025-01-01T12:00:00Z",
  "last_tick_cause": "alarm",
  "tick_count": 1423,
  "queue_depth": 2,
  "budget": {
    "daily_limit_sats": 50000,
    "daily_spent_sats": 1200,
    "remaining_sats": 48800
  },
  "memory": {
    "conversations": 12,
    "goals_active": 3,
    "goals_completed": 47,
    "patterns_learned": 23,
    "storage_bytes": 1048576
  },
  "connections": {
    "websocket": 2,
    "nostr_relays": 3
  }
}
```

---

## Export/Import (Portability)

### Export Bundle

```http
GET /agents/{id}/export
Accept: application/x-tar
```

Bundle contents:
```
agent-abc123.bundle/
├── manifest.json       # Bundle metadata
├── state.sqlite        # Full SQLite database
├── config.json         # Agent configuration
├── mounts.json         # Mount table + access levels
├── identity/
│   └── pubkey          # Public key (not private!)
├── runtime/            # Runtime state for safe migration
│   ├── seen_envelopes.json     # Bounded dedup cache
│   ├── idempotency_journal.json # Recent effect idempotency keys
│   ├── budget_state.json       # Current budget counters
│   └── plumber_rules.yaml      # Event routing rules
├── deadletter/         # Dead-lettered envelopes (if any)
│   └── envelopes.jsonl
└── logs/
    └── recent.jsonl    # Recent trajectory (optional)
```

The `runtime/` directory ensures safe migration:
- **seen_envelopes** — Prevents reprocessing on restore
- **idempotency_journal** — Prevents duplicate external effects
- **budget_state** — Preserves daily spend counters
- **plumber_rules** — Preserves custom routing

Manifest (with versioning for portability):
```json
{
  "manifest_version": 1,
  "agent_id": "agent_abc123",
  "agent_name": "my-agent",
  "pubkey": "npub1...",
  "exported_at": "2025-01-01T12:00:00Z",
  "source_runtime": "cloudflare",

  "versioning": {
    "agent_code_digest": "sha256:abc123...",
    "runtime_api_version": "0.1.0",
    "state_schema_version": 3,
    "state_version": 42
  },

  "capabilities": {
    "required_mounts": ["/wallet", "/nostr", "/compute"],
    "required_access": {
      "/wallet": "budgeted",
      "/nostr": "read_write",
      "/compute": "read_only"
    }
  },

  "includes_logs": true
}
```

### Import Compatibility

Before importing, the runtime checks compatibility:

| Check | Action if Failed |
|-------|------------------|
| `runtime_api_version` incompatible | Reject import |
| `state_schema_version` newer than target | Reject or migrate |
| `required_mounts` not available | Warn, import without those mounts |
| `agent_code_digest` mismatch | Warn (code may have changed) |

Import modes:
- **restore** — Requires exact version match
- **clone** — New ID, allows version differences with migration
- **migrate** — Explicit schema migration (prompts user)

### Import Bundle

```http
POST /agents/import
Content-Type: application/x-tar

<bundle data>
```

Import options:
```http
POST /agents/import?mode=restore     # Restore exact state (same ID)
POST /agents/import?mode=clone       # Create new agent with new ID
POST /agents/import?mode=merge       # Merge into existing agent
```

Response:
```json
{
  "id": "agent_abc123",
  "status": "imported",
  "state_version": 42,
  "warnings": []
}
```

---

## Event Stream

Real-time events via SSE:

```http
GET /agents/{id}/stream
Accept: text/event-stream
```

Events:
```
event: tick_start
data: {"tick_id": "t_123", "cause": "alarm", "timestamp": "..."}

event: tool_call
data: {"tick_id": "t_123", "tool": "read_file", "params": {...}}

event: tool_result
data: {"tick_id": "t_123", "tool": "read_file", "success": true}

event: message_sent
data: {"tick_id": "t_123", "to": "npub1...", "type": "dm"}

event: tick_end
data: {"tick_id": "t_123", "duration_ms": 1234, "success": true}

event: state_change
data: {"field": "goals", "operation": "add", "value": {...}}
```

---

## Multi-Runtime Coordination

When agents can move between runtimes, the control plane enables:

### Agent Discovery

```http
GET /agents/{id}/location
```

Response:
```json
{
  "agent_id": "agent_abc123",
  "runtime": "cloudflare",
  "endpoint": "https://agents.example.com",
  "last_seen": "2025-01-01T12:00:00Z"
}
```

### Agent Migration

```bash
# Export from source
agentctl --runtime cloudflare export agent_abc123 > agent.bundle

# Import to target
agentctl --runtime local import < agent.bundle

# Update registry (if using central discovery)
agentctl registry update agent_abc123 --runtime local
```

### Cross-Runtime Messaging

The control plane can route messages across runtimes:

```http
POST /agents/{id}/send
X-Source-Runtime: local
X-Target-Runtime: cloudflare

{
  "envelope": { ... }
}
```

---

## Authentication

Control plane endpoints require authentication:

### API Key

```http
GET /agents
Authorization: Bearer <api-key>
```

### Signed Requests (Agent-to-Agent)

```http
POST /agents/{id}/send
X-Agent-Pubkey: npub1...
X-Agent-Signature: <signature of request body>
```

### User Session

```http
GET /agents
Cookie: session=<session-token>
```

---

## Rate Limiting

Control plane endpoints are rate-limited:

```http
HTTP/1.1 429 Too Many Requests
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1704067200
Retry-After: 60
```

---

## Webhook Notifications

Register webhooks for agent events:

```http
POST /webhooks
Content-Type: application/json

{
  "url": "https://example.com/webhook",
  "events": ["agent.created", "agent.deleted", "agent.error"],
  "secret": "webhook-secret"
}
```

Events delivered:
```json
{
  "event": "agent.error",
  "agent_id": "agent_abc123",
  "timestamp": "2025-01-01T12:00:00Z",
  "data": {
    "error": "Budget exceeded",
    "tick_id": "t_123"
  }
}
```

---

## Implementation per Backend

| Endpoint | Browser | Local | Cloudflare | Server |
|----------|---------|-------|------------|--------|
| Create/Delete | IndexedDB | In-memory registry | D1 + DO | Postgres |
| Status | postMessage | Direct call | DO fetch | Direct call |
| Export | IndexedDB dump | SQLite dump | DO SQL export | SQLite/Postgres dump |
| Stream | BroadcastChannel | Tokio broadcast | DO WebSocket | Tokio broadcast |
| Metrics | Performance API | prometheus crate | Workers Analytics | prometheus crate |

---

## Control Plane as Agent Filesystem

The control API maps to the agent filesystem (see [PLAN9.md](./PLAN9.md)):

| API Endpoint | Filesystem Path |
|--------------|-----------------|
| `GET /agents/{id}/status` | `cat /agents/{id}/status` |
| `POST /agents/{id}/send` | `echo msg > /agents/{id}/inbox` |
| `GET /agents/{id}/export` | `cat /agents/{id}/memory/export` |
| `GET /agents/{id}/stream` | `tail -f /agents/{id}/logs/trace` |

The filesystem is the canonical interface; HTTP is one transport.
