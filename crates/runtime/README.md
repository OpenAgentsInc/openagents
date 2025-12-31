# OpenAgents Runtime

A pluggable runtime for autonomous AI agents that works across cloud and local deployments.

## Overview

The runtime provides the execution environment for agents, handling:

- **Lifecycle** — Create, wake, hibernate, terminate agents
- **Storage** — Persistent state across ticks (SQLite, Postgres, KV)
- **Identity** — Cryptographic keypairs for every agent (Nostr-compatible)
- **Communication** — Encrypted message passing between agents
- **Resources** — Budget enforcement and limits
- **Transparency** — Mandatory trajectory logging

## Key Concept: The Tick

Agents execute in discrete **ticks**:

```
WAKE → LOAD → PERCEIVE → THINK → ACT → REMEMBER → SCHEDULE → SLEEP
```

This model works universally across serverless, long-running, and local backends.

## Agent Filesystem

Inspired by Plan 9, every agent exposes a virtual filesystem:

```
/agents/<id>/
├── status          # agent state
├── inbox/          # incoming messages
├── outbox/         # emitted events
├── goals/          # active goals
├── memory/         # conversations, patterns
├── identity/       # pubkey, signing
└── wallet/         # balance, payments
```

The same interface works locally (FUSE), in the cloud (HTTP), or in a UI.

## Backends

Same agent code runs on any backend:

| Backend | Cold Start | Max Agents | Best For |
|---------|------------|------------|----------|
| **Cloudflare** | 10-50ms | Millions | Global scale, zero ops |
| **Local** | <100ms | Hundreds | Privacy, offline, free |
| **Docker** | 1-5s | Hundreds | Self-hosting |
| **Kubernetes** | 5-30s | Thousands | Enterprise scale |

## What Makes This Agent-Specific

This is not a generic actor framework. It's purpose-built for AI agents:

| Capability | Generic Actor | Agent Runtime |
|------------|---------------|---------------|
| Identity | Optional | Built-in keypairs |
| Memory | Arbitrary state | Structured (conversations, goals) |
| Economics | Not provided | Wallets, budgets, payments |
| Autonomy | Full by default | Graduated levels with approval |
| Transparency | Optional logging | Mandatory trajectories |

## Documentation

| Document | Description |
|----------|-------------|
| [DESIGN.md](docs/DESIGN.md) | Core architecture and principles (start here) |
| [TRAITS.md](docs/TRAITS.md) | Rust trait definitions |
| [BACKENDS.md](docs/BACKENDS.md) | Backend implementations and comparison |
| [AGENT-SPECIFIC.md](docs/AGENT-SPECIFIC.md) | What makes this agent-specific |
| [DRIVERS.md](docs/DRIVERS.md) | Event drivers (HTTP, WS, Nostr, Scheduler) |
| [CONTROL-PLANE.md](docs/CONTROL-PLANE.md) | Management API (HTTP + CLI) |
| [PLAN9.md](docs/PLAN9.md) | Plan 9 inspirations (filesystem, namespaces, plumber) |
| [FILESYSTEM.md](docs/FILESYSTEM.md) | FileService trait and implementations |
| [PRIOR-ART.md](docs/PRIOR-ART.md) | Related work (Plan 9, WANIX, OANIX) |

## Quick Example

```rust
use openagents_runtime::{Agent, AgentContext, Trigger, TickResult};

pub struct MyAgent;

impl Agent for MyAgent {
    type State = MyState;
    type Config = MyConfig;

    fn on_trigger(
        &self,
        ctx: &mut AgentContext<Self::State>,
        trigger: Trigger,
    ) -> Result<TickResult> {
        match trigger {
            Trigger::Message(msg) => {
                ctx.state.message_count += 1;
                ctx.broadcast("message_received", &msg);
                Ok(TickResult::success())
            }
            Trigger::Alarm(_) => {
                ctx.schedule_alarm(Duration::from_secs(60), None);
                Ok(TickResult::success())
            }
            _ => Ok(TickResult::default()),
        }
    }
}
```

## Crate Layout

```
crates/
├── runtime/              # This crate - core abstractions
├── runtime-local/        # Local device backend (SQLite + tokio)
├── runtime-cloudflare/   # Cloudflare Workers backend (DO)
├── runtime-server/       # Container/VM backend
├── agent-memory/         # Structured memory schema
├── agent-identity/       # Identity and signing
└── agent-drivers/        # Shared driver implementations
```

## Feature Flags

- `cloudflare` — Cloudflare Workers/Durable Objects backend
- `local` — Local device backend with SQLite
- `full` — Enable all optional features (tracing, metrics)

## Prior Art

This design builds on:

- **Plan 9** — Everything is a file, per-process namespaces
- **WANIX** — WebAssembly runtime with Plan 9 concepts
- **OANIX** — Our experimental Rust-native agent OS
- **Cloudflare Durable Objects** — Tick model, SQLite, hibernation

See [PRIOR-ART.md](docs/PRIOR-ART.md) for details.

## Status

**Design phase.** The `docs/` folder contains comprehensive design documents. Implementation is next.

## License

MIT
