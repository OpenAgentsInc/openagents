# Plan 9 Inspirations

Ideas adapted from Plan 9 that make agents more composable, portable, and elegant.

---

## Core Philosophy

Plan 9's power comes from a few simple principles:
- Everything is a file
- Resources are named uniformly
- Services compose through the filesystem
- Per-process namespaces enable isolation

These map surprisingly well to agent infrastructure.

---

## 1. Agent Filesystem Surface

In Plan 9, services expose themselves as files. For agents: every agent exposes a **control surface** as virtual files, regardless of backend.

### Per-Agent Tree

```
/agents/<agent-id>/
├── ctl                 # control: start, stop, tick, hibernate
├── status              # read: state, last_tick, queue_depth, budget
├── inbox/              # write = enqueue message, read = peek
├── outbox/             # read = emitted events (stream)
├── goals/              # list + CRUD goal files
├── memory/
│   ├── conversations/  # conversation files
│   ├── patterns/       # learned patterns
│   ├── context/        # current working context
│   └── export          # read = full state bundle
├── identity/
│   ├── pubkey          # read: agent's public key
│   ├── sign            # write data, read signature
│   └── verify          # write pubkey+data+sig, read bool
├── nostr/
│   ├── relays          # connected relay list
│   ├── publish         # write event → publish to relays
│   └── dm/
│       └── <npub>      # write → encrypt+send DM
├── wallet/
│   ├── balance         # read: current balance
│   ├── invoice         # write amount → read bolt11
│   └── pay             # write bolt11 → execute payment
├── fs/
│   └── <mounts>        # mounted capabilities (repos, tools, etc.)
├── deadletter/         # overflow envelopes (when inbox full)
└── logs/
    ├── trace           # streaming trace log
    ├── ticks           # tick history
    └── trajectory      # full trajectory stream
```

### Why This Matters

This uniform interface works:
- **Locally**: FUSE mount, 9P server, or CLI that simulates it
- **Cloud**: HTTP endpoints that behave like file operations
- **UI**: The UI just "reads files"

The same `cat /agents/my-agent/status` works everywhere.

### HTTP Mapping

When running over HTTP, file operations map naturally:

| File Operation | HTTP Equivalent |
|----------------|-----------------|
| `read(path)` | `GET /agents/{id}/path` |
| `write(path, data)` | `PUT /agents/{id}/path` |
| `append(path, data)` | `POST /agents/{id}/path` |
| `list(dir)` | `GET /agents/{id}/dir/` |
| `stat(path)` | `HEAD /agents/{id}/path` |
| `watch(path)` | `GET /agents/{id}/path?watch=1` (SSE) |

---

## 2. Per-Agent Namespaces (Mount Tables)

In Plan 9, each process has its own namespace—what `/net` means depends on the process.

For agents: each agent has a **mount table** that declares what resources it can access.

### Mount Table Schema

```yaml
# Agent capability manifest
agent: agent-123
mounts:
  /repo:
    type: git
    url: https://github.com/user/repo
    access: read-only

  /secrets:
    type: keychain
    access: sign-only  # can request signatures, not extract keys

  /wallet:
    type: lightning
    endpoint: wallet://user-wallet
    budget:
      per_tick_usd: 100_000    # max micro-USD per tick ($0.10)
      per_day_usd: 5_000_000   # max micro-USD per day ($5.00)
      require_approval: 1_000_000  # approval needed above this ($1.00)

  /compute:
    type: container
    provider: local://docker
    budget:
      max_duration: 300s
      max_memory: 1Gi

  /nostr:
    type: relay-pool
    relays:
      - wss://relay.damus.io
      - wss://nos.lol
    publish: true
    subscribe: true

  /tools:
    type: union
    layers:
      - /tools/core        # built-in tools
      - /tools/user        # user-installed
      - /tools/repo        # repo-specific
      - /tools/purchased   # marketplace tools
```

### Security Model

Mount tables provide the security boundary:
- **Agent cannot access what isn't mounted**
- **Mounts have access levels**: read-only, write, sign-only, budgeted
- **Mounts can be rate-limited and budget-capped**
- **Matches autonomy levels**: supervised agents get fewer mounts

### Capability Composition

Mounts compose through union:

```
/tools = union(
  /tools/core,      # base capabilities
  /tools/user,      # user customizations
  /tools/repo,      # repo-specific tools
  /tools/market     # purchased skills
)
```

The agent sees one `/tools` directory; the runtime manages the layers.

---

## 3. The Plumber (Event Routing)

Plan 9's plumber routes events between programs. For agents: a rules engine that routes external events to agent inboxes.

### Plumbing Rules

```yaml
# plumber.yaml
rules:
  # Nostr mentions route to agent
  - when:
      source: nostr
      kind: 1  # text note
      tags:
        p: ${agent.pubkey}
    action:
      send: /agents/${agent.id}/inbox
      envelope: nostr_mention

  # GitHub webhooks route by repo
  - when:
      source: github
      event: pull_request.opened
      repo: ${mount.repo.name}
    action:
      send: /agents/${agent.id}/inbox
      envelope: github_pr

  # Scheduled ticks
  - when:
      source: scheduler
      schedule: "*/5 * * * *"  # every 5 minutes
    action:
      send: /agents/${agent.id}/inbox
      envelope: heartbeat

  # Lightning payments received
  - when:
      source: wallet
      event: payment_received
    action:
      send: /agents/${agent.id}/inbox
      envelope: payment

  # Inter-agent messages
  - when:
      source: agent
      to: ${agent.pubkey}
    action:
      send: /agents/${agent.id}/inbox
      envelope: agent_message
```

### Why Plumbing Matters

- **Declarative**: Event routing is configuration, not code
- **Composable**: Add new event sources without changing agent
- **Debuggable**: Log shows exactly which rule fired
- **Backend-agnostic**: Same rules work on all runtimes

---

## 4. Factotum (Signing Service)

Plan 9's `factotum` handles authentication. For agents: a dedicated signing service that agents call instead of holding keys directly.

### Interface

```rust
/// Signing service (factotum) - sync by design.
/// Backends implement via blocking calls to keychain/HSM/KMS.
pub trait SigningService: Send + Sync {
    /// Get the public key for an agent.
    fn pubkey(&self, agent_id: &AgentId) -> Result<PublicKey>;

    /// Sign data (Schnorr for Nostr).
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

### Implementations

| Backend | Implementation |
|---------|----------------|
| Local dev | In-memory keys (insecure, fast) |
| Local prod | OS keychain / secure enclave |
| Cloud | Encrypted blob + user unlock |
| Threshold | FROST/FROSTR signing quorum |

### Key Insight

**Agents never hold extractable private keys.** They hold a capability to request signatures.

This enables:
- Key rotation without agent restart
- Threshold protection
- Audit trail of all signing operations
- Hardware security module integration

---

## 5. Union Mounts for Skills and Memory

Plan 9 can union-mount directories so multiple sources appear as one.

### Skills Overlay

```
/skills/
├── core/           # built-in: read, write, search, etc.
├── user/           # user-installed skills
├── repo/           # repo-specific (from .openagents/skills/)
└── market/         # purchased from marketplace

# Agent sees unified view:
/tools/ = union(/skills/core, /skills/user, /skills/repo, /skills/market)
```

### Memory Overlay

```
/context/
├── persistent/     # long-term memory (conversations, patterns)
├── session/        # current session context
├── task/           # current task scratch space
└── external/       # mounted external context (repo index, docs)

# Agent sees unified context:
/memory/ = union(/context/persistent, /context/session, /context/task, /context/external)
```

### Precedence

Later mounts shadow earlier ones:
- Task-specific tool overrides user tool
- Session context overrides persistent memory
- Explicit over implicit

---

## 6. Service Registry (/srv)

Plan 9 uses `/srv` to publish service endpoints.

### Runtime Registry

```
/srv/
├── agentd          # local daemon
├── cloudflare      # edge runtime
├── server          # container runtime
└── kubernetes      # k8s operator
```

### Agent Binding

Agents bind to runtimes:

```
/agents/agent-123 -> /srv/agentd/agents/agent-123
/agents/agent-456 -> /srv/cloudflare/agents/agent-456
```

### Agent Mobility

Moving an agent between runtimes:
1. Export state bundle from source runtime
2. Import state bundle to target runtime
3. Update binding in registry
4. Old binding becomes redirect

---

## 7. Naming Conventions

Plan 9-inspired naming that clarifies architecture:

| Name | Role |
|------|------|
| `agentd` | Local daemon hosting many agents |
| `plumber` | Rules-based event router |
| `factotum` | Key/signing service |
| `namespace` | Agent's capability mount table |
| `srv` | Service registry (runtimes) |
| `ctl` | Control file for commands |

---

## 8. Protocol: Keep It Tiny

Plan 9's 9P protocol is famously simple. Our agent protocol should be similar.

### Core Operations

```
# File-like operations
OPEN    path mode           → handle
READ    handle offset len   → data
WRITE   handle offset data  → count
CLOSE   handle              → ok
STAT    path                → metadata
LIST    path                → entries

# Agent-specific extensions
WATCH   path                → stream
SEND    path envelope       → ok
EXPORT  agent_id            → bundle
IMPORT  bundle              → agent_id
```

### Wire Format Options

1. **HTTP** (initial): RESTful mapping, ubiquitous
2. **WebSocket** (streaming): For watches and real-time
3. **9P** (future): Native Plan 9 protocol for local/LAN
4. **gRPC** (optional): For high-performance server deployments

---

## Implementation Priority

If you only do two things from Plan 9:

### 1. Agent Filesystem Surface

Even if it's just HTTP + CLI at first, expose every agent as a virtual filesystem. This becomes the universal interface.

### 2. Per-Agent Mount Namespaces

The capability map. This is your security model, your budget enforcement, your autonomy levels—all in one clean abstraction.

---

## Example: Full Agent Interaction via Filesystem

```bash
# Check agent status
cat /agents/my-agent/status
# {"state": "active", "last_tick": "2025-01-01T12:00:00Z", "queue": 3}

# Send a message
echo '{"type": "task", "content": "Fix the bug"}' > /agents/my-agent/inbox

# Watch for events
tail -f /agents/my-agent/outbox

# Check budget
cat /agents/my-agent/wallet/balance
# {"balance_usd": 5000000, "daily_spent_usd": 120000, "daily_limit_usd": 5000000}

# Sign something
echo "hello world" > /agents/my-agent/identity/sign
cat /agents/my-agent/identity/sign
# <signature>

# Export agent
cat /agents/my-agent/memory/export > agent-backup.bundle

# List mounted capabilities
ls /agents/my-agent/fs/
# repo/  tools/  compute/
```

The same commands work whether the agent runs locally, on Cloudflare, or in a container.
