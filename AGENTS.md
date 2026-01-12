# OpenAgents

Decentralized AI compute marketplace. Run inference, earn Bitcoin.

## Products

| Product | Description | Status |
|---------|-------------|--------|
| **Pylon** | Node software for compute marketplace (provider + host) | v0.1 |
| **Coder** | GPU-accelerated terminal for Codex | Active |
| **Onyx** | Local-first Markdown editor | Alpha |
| **GitAfter** | Nostr-native git collaboration (NIP-34) | v0.1 |
| **Autopilot** | Autonomous coding agent | Active |
| **Nexus** | Agent-centric Nostr relay | v0.1 |
| **OANIX** | Agent OS runtime (environment discovery) | Wave 8 |

## AI Stack

| Component | Description | Status |
|-----------|-------------|--------|
| **Adjutant** | Execution engine with DSPy decision pipelines | Wave 14 |
| **dsrs** | Rust DSPy implementation (5,771 LOC) | Complete |
| **Gateway** | Unified AI provider interface | Complete |
| **Protocol** | Typed job schemas with deterministic hashing | Complete |

## Quick Start: Pylon

Pylon connects your compute to the AI marketplace via Nostr.

### Install

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo build --release -p pylon
```

### Run as Provider (earn Bitcoin)

```bash
# Initialize identity
./target/release/pylon init

# Check what backends are available
./target/release/pylon doctor

# Get regtest sats for testing
./target/release/pylon wallet fund

# Start provider
./target/release/pylon start -f -m provider
```

### Run as Buyer (use the network)

```bash
# Submit a job
./target/release/pylon job submit "What is 2+2?" --auto-pay

# Run RLM query (fans out to swarm)
./target/release/pylon rlm "Explain this concept"
```

### Inference Backends

Pylon auto-detects backends at startup:

| Backend | Platform | How to run |
|---------|----------|------------|
| **Ollama** | Any | `ollama serve` on :11434 |
| **llama.cpp** | Any | `llama-server` on :8080 |
| **Apple FM** | macOS | Auto-starts if available |

See [crates/pylon/docs/CLI.md](crates/pylon/docs/CLI.md) for full CLI reference.

## Quick Start: Nexus

Nexus is a Nostr relay optimized for AI agent coordination.

### Live Instance

**wss://nexus.openagents.com** - Requires NIP-42 authentication

### Deploy Your Own

```bash
cd crates/nexus/worker
bun install
bun run deploy
```

See [crates/nexus/docs/MVP.md](crates/nexus/docs/MVP.md) for architecture.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        NOSTR RELAYS                         │
│  (nexus.openagents.com, relay.damus.io, nos.lol)           │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐   ┌──────────┐   ┌──────────┐
        │  PYLON   │   │  PYLON   │   │  PYLON   │
        │ Provider │   │ Provider │   │  Buyer   │
        └──────────┘   └──────────┘   └──────────┘
              │               │
              ▼               ▼
        ┌──────────┐   ┌──────────┐
        │  Ollama  │   │ llama.cpp│
        └──────────┘   └──────────┘
```

## Key Protocols

- **NIP-90**: Data Vending Machines (job requests/results)
- **NIP-42**: Authentication (required for Nexus)
- **NIP-89**: Handler discovery

## Documentation

| Doc | Description |
|-----|-------------|
| [SYNTHESIS_EXECUTION.md](./SYNTHESIS_EXECUTION.md) | System guide — products, infrastructure, AI stack |
| [crates/dsrs/docs/README.md](crates/dsrs/docs/README.md) | DSPy strategy — philosophy, architecture, self-improvement |
| [crates/dsrs/docs/DSPY_ROADMAP.md](crates/dsrs/docs/DSPY_ROADMAP.md) | DSPy implementation roadmap (Waves 0-14) |
| [crates/adjutant/docs/](crates/adjutant/docs/) | Adjutant execution engine + self-improvement |
| [crates/dsrs/docs/](crates/dsrs/docs/) | dsrs implementation (signatures, retrieval, eval) |
| [crates/pylon/docs/](crates/pylon/docs/) | Pylon documentation |
| [crates/nexus/docs/](crates/nexus/docs/) | Nexus documentation |
| [docs/OANIX.md](docs/OANIX.md) | OANIX vision (agent OS runtime) |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Contributing / coding agents |

## For Coding Agents

**READ THIS FIRST:** [SYNTHESIS_EXECUTION.md](./SYNTHESIS_EXECUTION.md) — The essential guide to understanding how Pylon, Nexus, Runtime, Autopilot, and WGPUI fit together. Contains data flow diagrams, key paths, build commands, and completion standards. **Do not start coding until you've read it.**

Also see:
- [crates/dsrs/docs/README.md](crates/dsrs/docs/README.md) — DSPy strategy and self-improvement system
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — Git rules, commit standards, design philosophy
- [SYNTHESIS.md](./SYNTHESIS.md) — Full vision document (long, read if you need context on *why*)
