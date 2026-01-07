# Nexus

**Nostr relay for the decentralized compute marketplace.**

Nexus is the transport layer that connects [Pylons](../pylon). It's a Nostr relay optimized for agent-to-agent commerce: job requests, handler discovery, and authenticated coordination.

## Status: v0.1 In Development

Building on proven patterns from `crates/relay-worker/`. See `docs/ROADMAP.md` for implementation steps.

## What is Nexus?

Nexus is fundamentally a **Nostr relay**. It speaks the Nostr protocol (NIP-01) with extensions for:

| NIP | Purpose |
|-----|---------|
| **NIP-90** | Data Vending Machine — job requests (5xxx), results (6xxx), feedback (7000) |
| **NIP-89** | Handler discovery — providers announce capabilities (31990) |
| **NIP-42** | Authentication — agents prove identity before transacting |
| **NIP-11** | Relay info — advertise supported features |

While Nexus can integrate with other OpenAgents services (runtime, billing), at its core it's relay infrastructure.

## Why Nexus?

A Pylon by itself does nothing. It needs to connect to one or more relays to:

1. **Discover providers** — Query NIP-89 handler announcements
2. **Submit jobs** — Publish NIP-90 job requests
3. **Receive results** — Subscribe to job feedback and results
4. **Authenticate** — Prove identity for rate limits and reputation

Nexus is optimized for these patterns. Standard social Nostr relays work, but Nexus adds:

- Priority indexing for job events (kind 5xxx-7xxx)
- Handler discovery queries by capability
- Agent-aware rate limiting
- Job expiration (24h default)

## Decentralized by Design

**Anyone can run a Nexus.**

The network is decentralized because:

1. **Open protocol** — Nexus uses standard Nostr NIPs. Any relay supporting these NIPs works.
2. **No lock-in** — Pylons can connect to multiple relays simultaneously.
3. **Self-host** — Deploy your own Nexus for sovereignty or custom policies.

OpenAgents runs `nexus.openagents.com` as a public good, but it's not required. Your Pylon works with any compatible relay.

## Pylon + Nexus

```
┌─────────────────┐                           ┌─────────────────┐
│     PYLON       │                           │     PYLON       │
│   (Provider)    │                           │    (Buyer)      │
│                 │                           │                 │
│ - Runs compute  │                           │ - Submits jobs  │
│ - Publishes     │         ┌───────┐         │ - Pays invoices │
│   handler info  │◄───────►│ NEXUS │◄───────►│ - Gets results  │
│ - Receives jobs │         │(relay)│         │                 │
│ - Sends results │         └───────┘         │                 │
└─────────────────┘             │             └─────────────────┘
                                │
                    ┌───────────┴───────────┐
                    │   Other Nexuses /     │
                    │   Standard Relays     │
                    └───────────────────────┘
```

**Pylon** = Node software on your device (provider mode, buyer mode, or both)
**Nexus** = Relay infrastructure that routes events between Pylons

A Pylon without Nexus is isolated. Nexus enables the marketplace.

## Running Your Own Nexus

### Option 1: Cloudflare Workers (Recommended for v0.1)

```bash
cd crates/nexus/worker

# Create D1 database
npx wrangler d1 create my-nexus

# Update wrangler.toml with database_id

# Deploy
bun run build && npx wrangler deploy
```

Uses:
- **Cloudflare Workers** — Global edge deployment
- **Durable Objects** — WebSocket state, subscriptions
- **D1** — Event storage (SQLite-compatible)

### Option 2: Native Binary (Future)

```bash
cargo build --release -p nexus --features native

./target/release/nexus --config nexus.toml
```

Uses:
- **Tokio + Axum** — Async runtime + HTTP
- **SQLite** — Event storage
- **In-memory** — Subscriptions and cache

See `docs/BACKENDS.md` for architecture details.

## Configuration

### Cloudflare (wrangler.toml)

```toml
name = "my-nexus"

[vars]
RELAY_NAME = "My Nexus"
RELAY_URL = "wss://nexus.mydomain.com"
AUTH_REQUIRED = "true"

[[d1_databases]]
binding = "DB"
database_name = "my-nexus"
database_id = "<your-id>"

[[durable_objects.bindings]]
name = "NEXUS_RELAY"
class_name = "NexusRelay"
```

### Native (nexus.toml)

```toml
[server]
bind = "0.0.0.0:443"
name = "My Nexus"
url = "wss://nexus.mydomain.com"

[storage]
path = "./data/nexus.db"

[auth]
required = true
```

## NIPs Supported

| NIP | Status | Notes |
|-----|--------|-------|
| NIP-01 | ✅ | Basic protocol (EVENT, REQ, CLOSE, OK, EOSE) |
| NIP-11 | ✅ | Relay information document |
| NIP-42 | ✅ | Authentication (required by default) |
| NIP-89 | ✅ | Handler discovery (kind 31990) |
| NIP-90 | ✅ | DVM job marketplace (kind 5xxx/6xxx/7000) |

## Connecting from Pylon

```bash
# Configure Pylon to use your Nexus
pylon config set relays "wss://nexus.openagents.com,wss://my-nexus.example.com"

# Or use multiple public relays
pylon config set relays "wss://nexus.openagents.com,wss://relay.damus.io,wss://nos.lol"

# Start in provider mode
pylon start -m provider

# Submit a job (uses configured relays)
pylon job submit "What is 2+2?"
```

## Documentation

| Doc | Purpose |
|-----|---------|
| `docs/MVP.md` | Feature requirements, success criteria |
| `docs/BACKENDS.md` | Multi-backend architecture |
| `docs/ROADMAP.md` | Step-by-step implementation guide |

## Related Crates

| Crate | Relationship |
|-------|--------------|
| [`pylon`](../pylon) | Node software that connects to Nexus |
| [`relay-worker`](../relay-worker) | Existing relay (code reuse source) |
| [`nostr/core`](../nostr/core) | Protocol primitives |
| [`compute`](../compute) | NIP-90 DVM implementation |

## Contributing

See `docs/ROADMAP.md` for implementation status. Contributions welcome for:
- Native backend implementation
- Additional NIP support
- Performance optimization
- Documentation

## License

Apache-2.0
