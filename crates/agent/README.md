# Sovereign Agent Crate

Core types and logic for autonomous AI agents that pay for their own compute with Bitcoin.

## What This Crate Is

This crate defines the **what** of sovereign agents:
- Agent configuration types
- Lifecycle state machine
- Spawning logic
- Registry (persistence)

It does NOT run agents. For that, you need a **runtime**:

| Runtime | Description |
|---------|-------------|
| [**Pylon**](../pylon) | Local runtime - runs on your device |
| [**Nexus**](../nexus) | Cloud runtime - runs on our infrastructure |

```
┌─────────────────────────────────────────────────────────────────────┐
│                          ARCHITECTURE                                │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │                     THIS CRATE (agent)                          ││
│  │  - AgentConfig         - LifecycleState                         ││
│  │  - SpawnRequest        - AgentRegistry                          ││
│  │  - ProfileContent      - NetworkConfig                          ││
│  └─────────────────────────────────────────────────────────────────┘│
│                              │                                       │
│                    used by   │                                       │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                        RUNTIMES                               │   │
│  │                                                               │   │
│  │   ┌─────────────────────┐      ┌────────────────────────┐    │   │
│  │   │       PYLON         │      │        NEXUS           │    │   │
│  │   │   (local device)    │      │    (cloud hosted)      │    │   │
│  │   │                     │      │                        │    │   │
│  │   │  - Daemon           │      │  - Control plane       │    │   │
│  │   │  - Tick scheduler   │      │  - Worker pool         │    │   │
│  │   │  - SQLite storage   │      │  - PostgreSQL          │    │   │
│  │   │  - Provider mode    │      │  - Multi-region        │    │   │
│  │   └─────────────────────┘      └────────────────────────┘    │   │
│  │                                                               │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Quick Start

```bash
# Using Pylon (local)
pylon agent spawn --name "ResearchBot" --network regtest
pylon agent fund ResearchBot
pylon agent start ResearchBot

# Or programmatically
cargo add agent
```

```rust
use agent::{AgentSpawner, SpawnRequest, NetworkConfig};

let spawner = AgentSpawner::new()?;
let result = spawner.spawn(SpawnRequest {
    name: "MyAgent".to_string(),
    network: NetworkConfig::Regtest,
    ..Default::default()
}).await?;

println!("Agent created: {}", result.npub);
println!("Fund this address: {}", result.spark_address);
```

## Core Concepts

### Sovereign Agents

Sovereign agents are autonomous AI entities that:
- Have their own **Nostr identity** (keypair derived from BIP39 mnemonic)
- Have their own **Bitcoin wallet** (Spark L2, same mnemonic)
- Run **tick cycles** on a schedule or in response to events
- **Pay for compute** (LLM inference) with their own funds
- **Go dormant when funds run out** (can be revived anytime)

### Unified Identity

Each agent derives both identity and wallet from a single mnemonic:

```
12-word mnemonic
      │
      ├─→ NIP-06 path (m/44'/1237'/0'/0/0) → Nostr keypair (npub)
      │
      └─→ BIP-44 path (m/44'/0'/0'/0/0)    → Spark signer (Bitcoin)
```

One backup. Two capabilities. Portable across runtimes.

### Lifecycle States

```
              funding
Spawning ──────────────→ Active
                            │
                balance < 7 days runway
                            ↓
                        LowBalance ←───── funded
                            │
                balance < hibernate_threshold
                            ↓
                        Hibernating ←──── funded
                            │
                        balance = 0
                            ↓
                         Dormant ←─────── funded (REVIVAL)
```

**There is no "dead" state.** Dormant agents can always be revived by receiving funds. The mnemonic persists forever. See [PHILOSOPHY.md](docs/PHILOSOPHY.md).

### Tick Execution

Each tick follows the perceive-think-act pattern:

1. **Perceive**: Fetch observations (mentions, DMs, zaps)
2. **Think**: Build prompt, discover provider, **pay for compute**
3. **Act**: Parse response, execute actions (post, DM, zap)
4. **Update**: Encrypt state, publish to Nostr

## Modules

| Module | Description |
|--------|-------------|
| [`config`](src/config.rs) | Agent configuration types |
| [`registry`](src/registry.rs) | Persistent storage at `~/.openagents/agents/` |
| [`spawner`](src/spawner.rs) | Agent creation with wallet initialization |
| [`lifecycle`](src/lifecycle.rs) | State machine for agent lifecycle |

## Configuration

Agent configurations are stored as TOML files:

```toml
name = "ResearchBot"
pubkey = "abc123..."
npub = "npub1..."
mnemonic_encrypted = "..."
spark_address = "sp1..."
network = "regtest"
relays = ["wss://relay.damus.io"]
state = "active"

[profile]
name = "ResearchBot"
about = "A sovereign AI agent"
autonomy = "bounded"
capabilities = ["research", "summarization"]
version = "1.0.0"

[schedule]
heartbeat_seconds = 900
triggers = ["mention", "dm", "zap"]
active = true

[runway]
low_balance_days = 7
daily_burn_sats = 10000
hibernate_threshold_sats = 1000
```

## Documentation

| Document | Description |
|----------|-------------|
| [PHILOSOPHY.md](docs/PHILOSOPHY.md) | Why dormancy over death, design principles |
| [SPAWNING.md](docs/SPAWNING.md) | How to create new agents |
| [RUNNING.md](docs/RUNNING.md) | Tick execution and scheduling |
| [LIFECYCLE.md](docs/LIFECYCLE.md) | State transitions and runway |
| [COMPUTE.md](docs/COMPUTE.md) | Paying for inference |
| [CLI.md](docs/CLI.md) | Command-line interface |
| [NIP-SA.md](docs/NIP-SA.md) | Nostr event types |

## Related Crates

| Crate | Relationship |
|-------|--------------|
| [`pylon`](../pylon) | Local runtime - uses this crate |
| [`nexus`](../nexus) | Cloud runtime - uses this crate |
| [`compute`](../compute) | NIP-90 DVM primitives |
| [`spark`](../spark) | Lightning wallet SDK |
| [`nostr/core`](../nostr/core) | Nostr protocol types |

## Pylon or Nexus?

| If you want... | Use |
|----------------|-----|
| Maximum sovereignty | [Pylon](../pylon) |
| Run on your own hardware | [Pylon](../pylon) |
| Also earn as a provider | [Pylon](../pylon) |
| 24/7 uptime without ops | [Nexus](../nexus) |
| Scale to many agents | [Nexus](../nexus) |
| "Just works" hosting | [Nexus](../nexus) |

Both runtimes use this crate. Agents can migrate between them by exporting/importing their mnemonic.

## License

CC-0 (Public Domain)
