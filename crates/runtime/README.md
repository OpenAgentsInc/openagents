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
/agents/<id>/                    # Global admin view (operators, CLI, HTTP)
├── ctl                 # control: tick, hibernate, wake
├── status              # agent state, last_tick, queue_depth
├── inbox/              # incoming messages (write to enqueue)
├── outbox/             # emitted events (streaming)
├── goals/              # active goals (CRUD)
├── memory/
│   ├── conversations/  # conversation files
│   ├── patterns/       # learned patterns
│   └── export          # full state bundle
├── identity/
│   ├── pubkey          # agent's public key
│   ├── sign            # write data, read signature
│   ├── verify          # verify signatures
│   ├── encrypt         # encrypt to recipient pubkey
│   └── decrypt         # decrypt from sender
├── wallet/
│   ├── balance         # current balance
│   ├── pay             # write bolt11 to pay
│   ├── invoice         # create invoice to receive
│   └── history/        # transaction history
├── nostr/
│   ├── relays          # connected relay list
│   └── publish         # write event to publish
├── compute/
│   ├── providers/      # available AI providers
│   ├── new             # Call (write+read) → job_id
│   ├── jobs/<id>/      # status, result, stream
│   ├── policy          # compute policy (agents: read-only)
│   └── usage           # reserved/spent (micro-USD)
├── containers/
│   ├── providers/      # local, cloudflare, daytona, dvm
│   ├── new             # Call (write+read) → session_id
│   ├── sessions/       # per-session status, output, files
│   ├── policy          # container policy (agents: read-only)
│   ├── usage           # reserved/spent (micro-USD)
│   └── auth/           # challenge: agent writes; token/config: admin
├── claude/
│   ├── providers/      # tunnel, cloud, local
│   ├── new             # Call (write+read) → session_id
│   ├── sessions/       # status, prompt, output, tools, fork, ctl
│   ├── policy          # claude policy (agents: read-only)
│   ├── usage           # reserved/spent (micro-USD)
│   ├── auth/           # challenge: agent writes; tunnels config: admin
│   ├── workers/        # worker pool status (admin-only)
│   └── proxy/          # proxy status/allowlist (admin-only)
├── hud/
│   ├── stream          # redacted event stream (public viewers)
│   └── settings        # public/private, embed_allowed
├── metrics/
│   ├── apm             # actions per minute
│   └── queue           # issue queue depth
├── deadletter/         # overflow envelopes (when inbox full)
├── logs/
│   ├── trace           # streaming trace (owner only)
│   └── trajectory      # tick history (replay source)
└── mounts              # show mount table
```

**Two namespace scopes:**
- **Agent-local** (what code inside the agent sees): root `/` is that agent. Paths like `/status`, `/inbox`.
- **Global admin** (what operators/CLI see): `/agents/<id>/...` prefix. Just a namespacing wrapper.

The same interface works locally (FUSE), in the cloud (HTTP), or in a UI.

**Policies are admin-only:** Agents can read `/compute/policy`, `/containers/policy`, `/claude/policy` but cannot modify them; changes are applied through the control plane.

### /compute vs /containers vs /claude

| Mount | Purpose | Stateful | Tool Use |
|-------|---------|----------|----------|
| `/compute` | Stateless inference jobs (LLM calls, embeddings) | No | No |
| `/containers` | Sandboxed code execution (build, test, run) | Session | No |
| `/claude` | Autonomous Claude Agent SDK sessions | Yes | Yes |

**When to use each:**
- `/compute`: Quick LLM queries, embeddings, simple inference
- `/containers`: Run untrusted code, CI builds, shell commands
- `/claude`: Multi-turn conversations, complex multi-step tasks with tool use

**Security posture:** `/claude` workers should prefer running in sandboxed containers with proxy-only networking when processing untrusted content. Credentials stay outside the sandbox via the tunnel/proxy pattern.

## Backends

Same agent code runs on any of the four backends:

| Backend | Cold Start | Max Agents | Best For |
|---------|------------|------------|----------|
| **Browser** | <10ms | Single | Privacy, offline, zero cost |
| **Cloudflare** | 10-50ms | Millions | Global scale, zero ops |
| **Local** | <100ms | Hundreds | Privacy, offline, dev |
| **Server** | 100ms-5s | Thousands | Self-hosting, enterprise |

The **Server** backend can be deployed on bare metal, Docker, or Kubernetes—these are deployment modes, not separate backends.

The Browser backend is inspired by [WANIX](https://github.com/tractordev/wanix) (Plan 9 in browser) and [Apptron](https://github.com/progrium/apptron) (full Linux in browser). Same WASI binary runs on server (native), desktop (native), or browser (WASM).

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
| [COMPUTE.md](docs/COMPUTE.md) | AI compute abstraction (providers, budgets, streaming) |
| [CONTAINERS.md](docs/CONTAINERS.md) | Container spawning (Local, Cloudflare, Daytona, DVMs) |
| [CLAUDE.md](docs/CLAUDE.md) | Claude Agent SDK integration (tunnels, workers, security) |
| [HUD.md](docs/HUD.md) | HUD integration (events, redaction, public access) |
| [ROADMAP.md](docs/ROADMAP.md) | Implementation roadmap (build order, milestones) |
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
├── runtime-browser/      # Browser backend (WASM + IndexedDB)
├── runtime-cloudflare/   # Cloudflare Workers backend (DO)
├── runtime-local/        # Local device backend (SQLite + tokio)
├── runtime-server/       # Server backend (Docker/K8s/bare metal)
├── agent-memory/         # Structured memory schema
├── agent-identity/       # Identity and signing (SigningService)
└── agent-drivers/        # Shared driver implementations
```

## Feature Flags

- `browser` — Browser backend (WASM + IndexedDB)
- `cloudflare` — Cloudflare Workers backend (Durable Objects)
- `local` — Local device backend (SQLite + tokio)
- `server` — Server backend (for Docker/K8s/bare metal deployment)
- `full` — Enable all optional features (tracing, metrics)

## Prior Art

This design builds on:

- **Plan 9** — Everything is a file, per-process namespaces
- **WANIX** — Browser-first runtime with Plan 9 concepts (WASI in browser)
- **Apptron** — Full Linux environment in browser (virtual network, heavy compute)
- **OANIX** — Our experimental Rust-native agent OS
- **Cloudflare Durable Objects** — Tick model, SQLite, hibernation

See [PRIOR-ART.md](docs/PRIOR-ART.md) for details.

## Status

**Implementation in progress.** Milestones 0-14 complete (core tick engine, filesystem, control plane, budgets, /compute, /containers, HUD, `/claude`). See [ROADMAP.md](docs/ROADMAP.md) for details.

## License

MIT
