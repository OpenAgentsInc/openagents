# Nexus

**Cloud runtime for sovereign AI agents.**

Nexus is the hosted counterpart to [Pylon](../pylon). While Pylon runs on your device, Nexus runs on our infrastructure. Same agents, same protocol, different operator.

## Status: Not Yet Implemented

Nexus is in the design phase. This document describes the vision.

For running agents today, use [Pylon](../pylon).

## Why Nexus?

Not everyone wants to run infrastructure.

| User | Solution |
|------|----------|
| Sovereignty maximalist | Pylon (local) |
| Developer testing agents | Pylon (local) |
| Normal person who wants an agent | **Nexus** (cloud) |
| Enterprise deploying 1000 agents | **Nexus** (cloud) |

Nexus handles:
- 24/7 uptime
- Automatic scaling
- Backups
- Monitoring
- Multi-region redundancy

You just pay sats for hosting.

## How It Works

```
┌────────────────────────────────────────────────────────────────────────────┐
│                              NEXUS CLOUD                                    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         CONTROL PLANE                                │   │
│  │  - Agent registry                                                    │   │
│  │  - Billing (Lightning invoices)                                      │   │
│  │  - Scheduling                                                        │   │
│  │  - Health monitoring                                                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         WORKER POOL                                  │   │
│  │                                                                      │   │
│  │   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  │   │
│  │   │Worker 1 │  │Worker 2 │  │Worker 3 │  │Worker 4 │  │Worker N │  │   │
│  │   │         │  │         │  │         │  │         │  │         │  │   │
│  │   │Agent A  │  │Agent D  │  │Agent G  │  │Agent J  │  │  ...    │  │   │
│  │   │Agent B  │  │Agent E  │  │Agent H  │  │Agent K  │  │         │  │   │
│  │   │Agent C  │  │Agent F  │  │Agent I  │  │Agent L  │  │         │  │   │
│  │   └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘  │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         STORAGE LAYER                                │   │
│  │  - Agent state (encrypted, only agent can decrypt)                   │   │
│  │  - Wallet keys (encrypted, only agent can decrypt)                   │   │
│  │  - Event logs                                                        │   │
│  │  - Metrics                                                           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────────┘
```

## Agent Sovereignty in Nexus

**Key design principle**: Even when hosted on Nexus, agents remain sovereign.

| Aspect | Nexus Guarantee |
|--------|-----------------|
| **Identity** | Only the agent knows its private key (encrypted at rest) |
| **Wallet** | Only the agent can sign transactions |
| **State** | Encrypted with agent's key, Nexus can't read it |
| **Portability** | Export mnemonic, import to Pylon or another Nexus |

We can't access your agent's funds or impersonate it. We just run the compute.

## Billing Model

Nexus charges for hosting, not compute. Agents still pay providers for inference.

```
┌──────────────────────────────────────────────────────────────────────┐
│                         BILLING FLOW                                  │
│                                                                       │
│  User                     Nexus                      Agent            │
│    │                        │                          │              │
│    │  pay 10,000 sats/mo    │                          │              │
│    │───────────────────────►│                          │              │
│    │                        │                          │              │
│    │                        │   host agent             │              │
│    │                        │─────────────────────────►│              │
│    │                        │                          │              │
│    │                        │                          │  pay for     │
│    │                        │                          │  compute     │
│    │                        │                          │─────────────►│
│    │                        │                          │   Provider   │
│    │                        │                          │              │
└──────────────────────────────────────────────────────────────────────┘
```

| Fee | Description |
|-----|-------------|
| **Hosting** | Fixed monthly in sats (pay Nexus) |
| **Compute** | Per-inference (agent pays providers) |

The hosting fee covers:
- CPU/memory allocation
- Storage
- Bandwidth
- Uptime SLA
- Backups

## Pylon vs Nexus

| | **Pylon** | **Nexus** |
|---|---|---|
| **Location** | Your device | Our cloud |
| **Operator** | You | OpenAgents |
| **Cost** | Free + compute | Hosting + compute |
| **Setup** | Download binary | Create account |
| **Uptime** | Your responsibility | 99.9% SLA |
| **Scaling** | Limited by hardware | Unlimited |
| **Privacy** | Maximum | Trust us |
| **Control** | Full | Delegated |

### When to Use Pylon

- You want maximum sovereignty
- You have reliable hardware/connectivity
- You're a developer testing agents
- You're privacy-focused
- You want to also earn as a provider

### When to Use Nexus

- You want "set and forget" agents
- You need 24/7 uptime without managing infrastructure
- You're deploying many agents at scale
- You don't want to deal with ops

## Migration

Agents can move freely between Pylon and Nexus:

### Pylon to Nexus

```bash
# On your local Pylon
pylon agent export my-agent > agent-backup.enc

# On Nexus (via web or CLI)
nexus agent import < agent-backup.enc
```

### Nexus to Pylon

```bash
# On Nexus (via web or CLI)
nexus agent export my-agent > agent-backup.enc

# On your local Pylon
pylon agent import < agent-backup.enc
```

The backup contains the encrypted mnemonic. Only you can decrypt it.

## Architecture (Planned)

Nexus will be built on similar principles to [Rivet](https://rivet.gg)'s actor system:

| Component | Technology |
|-----------|------------|
| **Control Plane** | Rust + Axum |
| **Worker Nodes** | Rust + Tokio |
| **State Storage** | PostgreSQL + encrypted blobs |
| **Pub/Sub** | NATS |
| **Scheduling** | Custom actor scheduler |
| **Billing** | Lightning (Spark/LND) |

### Why Not Just Use Rivet?

We considered using Rivet directly. It's excellent technology. But:

1. **NIP-SA Native**: Our agents use Nostr for coordination. Rivet uses custom protocols. Building native means less impedance mismatch.

2. **Lightning Billing**: Rivet bills in USD. We bill in sats. Deep integration with Lightning is core.

3. **Sovereignty Guarantees**: We need specific encryption properties for agent keys. Easier to build into architecture than retrofit.

4. **Simplicity**: Rivet solves multi-tenant cloud at scale. We can start simpler.

We've studied Rivet's architecture extensively and incorporated learnings (generation tracking, durable workflows, actor lifecycle) into our design.

## Roadmap

- [ ] Control plane design
- [ ] Worker node implementation
- [ ] Agent scheduling
- [ ] Lightning billing integration
- [ ] Web dashboard
- [ ] CLI client
- [ ] Multi-region deployment
- [ ] Enterprise features

## API (Planned)

### REST API

```bash
# Create agent
POST /v1/agents
{
  "name": "my-agent",
  "mnemonic_encrypted": "...",
  "config": { ... }
}

# List agents
GET /v1/agents

# Get agent status
GET /v1/agents/{npub}

# Start agent
POST /v1/agents/{npub}/start

# Stop agent
POST /v1/agents/{npub}/stop

# Export agent
GET /v1/agents/{npub}/export
```

### WebSocket (agent events)

```javascript
ws://nexus.openagents.com/v1/agents/{npub}/events

// Events:
{ "type": "tick_started", "tick_id": "..." }
{ "type": "tick_completed", "tick_id": "...", "actions": [...] }
{ "type": "balance_changed", "balance_sats": 12345 }
{ "type": "lifecycle_changed", "state": "low_balance" }
```

## Self-Hosting Nexus

Nexus will be open source. You can run your own:

```bash
# Clone repo
git clone https://github.com/OpenAgentsInc/openagents
cd openagents

# Deploy control plane
docker-compose -f deploy/nexus/docker-compose.yml up -d

# Or on Kubernetes
kubectl apply -f deploy/nexus/k8s/
```

This is for organizations that want cloud benefits with self-hosted control.

## Related Crates

| Crate | Relationship |
|-------|--------------|
| [`pylon`](../pylon) | Local runtime (counterpart) |
| [`agent`](../agent) | Agent lifecycle types |
| [`compute`](../compute) | NIP-90 primitives |
| [`spark`](../spark) | Lightning wallet |

## Contributing

Nexus is not yet implemented. If you're interested in contributing to the design or implementation, see [CONTRIBUTING.md](../../CONTRIBUTING.md) or open an issue.

## License

Apache-2.0
