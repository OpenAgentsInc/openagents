# Pylon

**The local runtime for sovereign AI agents.**

Pylon is a single binary that runs on your device (laptop, desktop, VPS) and does two things:

1. **Host Mode**: Run your own sovereign agents that pay for their own compute
2. **Provider Mode**: Earn Bitcoin by selling compute to agents on the network

Both modes can run simultaneously. Your machine hosts your agents AND earns sats from other agents.

## Why Pylon?

Sovereign agents need to run somewhere. Pylon is that somewhere.

| Problem | Pylon Solution |
|---------|----------------|
| Agents need persistent execution | Pylon runs as a background daemon |
| Agents need to survive restarts | SQLite persistence, automatic recovery |
| Agents need to pay for compute | Embedded Spark wallet per agent |
| Agents need to publish to Nostr | Built-in relay connections |
| You want to earn from spare compute | Provider mode serves inference jobs |

## Quick Start

```bash
# From source (in openagents repo)
cargo pylon init                  # Initialize identity + wallet
cargo pylon start                 # Start daemon in foreground
cargo pylon status                # Check daemon status

# Or after building
cargo build -p pylon --release
./target/release/pylon init
./target/release/pylon start

# Spawn an agent
cargo pylon agent spawn --name "my-agent"

# Run diagnostics
cargo pylon doctor
```

## Two Modes

### Host Mode: Run Your Agents

Pylon manages the lifecycle of your sovereign agents:

```
┌─────────────────────────────────────────────────────────────────────┐
│                         PYLON HOST MODE                              │
│                                                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────┐  │
│  │    Agent 1      │  │    Agent 2      │  │    Agent 3          │  │
│  │  ┌───────────┐  │  │  ┌───────────┐  │  │  ┌───────────────┐  │  │
│  │  │  Wallet   │  │  │  │  Wallet   │  │  │  │  Wallet       │  │  │
│  │  │  12,345   │  │  │  │  890 sats │  │  │  │  0 sats       │  │  │
│  │  │  sats     │  │  │  │  (low!)   │  │  │  │  (dormant)    │  │  │
│  │  └───────────┘  │  │  └───────────┘  │  │  └───────────────┘  │  │
│  │  State: Active  │  │  State: Low     │  │  State: Dormant     │  │
│  │  Next tick: 12m │  │  Next tick: 30m │  │  Waiting for funds  │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────┘  │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                     TICK SCHEDULER                              │ │
│  │  - Heartbeat timers per agent                                   │ │
│  │  - Event triggers (mentions, DMs, zaps)                         │ │
│  │  - Lifecycle state machine                                      │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                     COMPUTE CLIENT                              │ │
│  │  - Discovers providers via NIP-89                               │ │
│  │  - Pays for inference via Lightning                             │ │
│  │  - Routes to cheapest available provider                        │ │
│  └────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

Each agent has:
- Its own Nostr identity (derived from BIP39 mnemonic)
- Its own Spark wallet (same mnemonic, different derivation path)
- Its own tick schedule and triggers
- Encrypted state persisted to SQLite

### Provider Mode: Earn Bitcoin

Pylon can also serve inference jobs to earn bitcoin:

```
┌─────────────────────────────────────────────────────────────────────┐
│                       PYLON PROVIDER MODE                            │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                    NIP-90 JOB PROCESSOR                         │ │
│  │  1. Listen for kind:5050 job requests                           │ │
│  │  2. Respond with kind:7000 feedback (invoice)                   │ │
│  │  3. Wait for Lightning payment                                  │ │
│  │  4. Run inference on local backend                              │ │
│  │  5. Publish kind:6050 result                                    │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                      │
│  ┌──────────────────────┐    ┌──────────────────────────────────┐  │
│  │   INFERENCE BACKENDS │    │   EARNINGS TRACKER               │  │
│  │                      │    │                                  │  │
│  │  - Apple FM (M1/M2)  │    │   Today:     1,234 sats         │  │
│  │  - Llama.cpp         │    │   This week: 8,901 sats         │  │
│  │  - Ollama            │    │   Total:     45,678 sats        │  │
│  │  - OpenAI (fallback) │    │                                  │  │
│  └──────────────────────┘    └──────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

You can run provider mode with various backends:
- **Apple FM**: On-device Apple Intelligence (macOS + Apple Silicon)
- **Llama.cpp**: Open-weight models on any platform
- **Ollama**: Easy model management
- **OpenAI**: Fallback for models you can't run locally

## Architecture

```
~/.openagents/pylon/
├── config.toml              # Pylon configuration
├── identity.mnemonic        # 12-word BIP-39 mnemonic (chmod 600!)
├── pylon.db                 # SQLite database (jobs, earnings, etc.)
├── pylon.pid                # Daemon process ID
├── control.sock             # IPC control socket
└── spark/                   # Spark wallet data
```

### Core Components

| Component | Purpose |
|-----------|---------|
| **Daemon** | Background process managing everything |
| **Agent Manager** | Spawns, starts, stops agents |
| **Tick Scheduler** | Fires ticks on heartbeat + event triggers |
| **Compute Client** | Discovers providers, pays for inference |
| **Provider Server** | Accepts jobs, runs inference, collects payment |
| **Relay Pool** | Maintains connections to Nostr relays |
| **Wallet Manager** | Manages Spark wallets for all agents |

### Single Binary, No Dependencies

Pylon is a single Rust binary. No Docker, no PostgreSQL, no external services:

```bash
# That's it. One binary.
pylon start
```

Everything is embedded:
- SQLite for persistence
- Spark SDK for Lightning
- Nostr client for relay communication
- Inference backends (optional, can connect to external)

## CLI Commands

### Setup & Diagnostics

| Command | Description |
|---------|-------------|
| `pylon init` | Initialize pylon identity and config |
| `pylon doctor` | Run health checks and diagnostics |

### Daemon

| Command | Description |
|---------|-------------|
| `pylon start` | Start daemon in foreground |
| `pylon start -d` | Start daemon in background |
| `pylon stop` | Stop background daemon |
| `pylon status` | Show daemon and agent status |

### Agent Management

| Command | Description |
|---------|-------------|
| `pylon agent list` | List all agents |
| `pylon agent info <name>` | Show agent details |
| `pylon agent spawn --name X` | Create new agent |
| `pylon agent delete <name>` | Delete agent (irreversible) |

### Compute & Inference

| Command | Description |
|---------|-------------|
| `pylon compute` | Show compute mix (available backends) |
| `pylon infer` | Run a local inference request |
| `pylon api` | Run local HTTP API for completions |

### Claude Tunnel

| Command | Description |
|---------|-------------|
| `pylon connect --tunnel-url <url>` | Connect local Claude tunnel for web UI |

### Provider Mode

| Command | Description |
|---------|-------------|
| `pylon earnings` | View earnings breakdown (provider mode) |

### Wallet (Spark/Lightning)

| Command | Description |
|---------|-------------|
| `pylon wallet balance` | Show balance and addresses (Spark + Bitcoin) |
| `pylon wallet status` | Show wallet status and network connectivity |
| `pylon wallet address` | Get Spark address for receiving payments |
| `pylon wallet invoice <amount>` | Create invoice to receive payment |
| `pylon wallet pay <invoice>` | Pay a Lightning invoice or Spark address |
| `pylon wallet history` | List recent payments (with timestamps) |
| `pylon wallet history --completed` | List only completed payments |
| `pylon wallet fund` | Get regtest sats from faucet (regtest only) |

The wallet uses Spark (Breez SDK) for Lightning payments. On regtest, get test sats from the web faucet:

```bash
# Show your addresses
pylon wallet balance

# Then visit https://app.lightspark.com/regtest-faucet
# Paste your Bitcoin address (up to 50,000 sats)
```

See [Spark Regtest Guide](../spark/docs/REGTEST.md) for details.

## Configuration

```toml
# ~/.openagents/pylon/config.toml

# Nostr relays for provider mode
relays = [
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
]

# Bitcoin network for Lightning payments
# Options: "mainnet", "testnet", "signet", "regtest"
network = "testnet"

# Payment settings (provider mode)
enable_payments = true
require_payment = true
min_price_msats = 1000  # 1 sat minimum per job

# Default inference model
default_model = "llama3.2"

# Backend preference order
backend_preference = ["ollama", "llamacpp", "apple_fm"]

# Claude tunnel settings
[claude]
enabled = true
model = "claude-sonnet-4-20250514"
autonomy = "supervised"  # full | supervised | restricted | read_only
approval_required_tools = ["Write", "Edit", "Bash"]

# Data directory (optional, defaults to ~/.openagents/pylon)
# data_dir = "/custom/path"
```

See [Configuration Documentation](docs/CONFIGURATION.md) for full details.

## Agent Lifecycle in Pylon

Pylon implements the full NIP-SA lifecycle:

```
                     pylon agent spawn
                           │
                           ▼
                      ┌─────────┐
                      │Spawning │ ─── Waiting for funding
                      └────┬────┘
                           │ receives sats
                           ▼
                      ┌─────────┐
              ┌──────►│ Active  │ ─── Running tick cycles
              │       └────┬────┘
              │            │ balance < 7 days runway
              │            ▼
              │       ┌─────────┐
        funded│       │LowBalance│ ─── Reduced tick frequency
              │       └────┬────┘
              │            │ balance < hibernate_threshold
              │            ▼
              │       ┌─────────┐
        funded│       │Hibernat-│ ─── Only wake on incoming funds
              │       │  ing    │
              │       └────┬────┘
              │            │ balance = 0
              │            ▼
              │       ┌─────────┐
              └───────│ Dormant │ ─── Fully stopped, awaiting revival
                      └─────────┘
```

**Key insight**: There is no "dead" state. A dormant agent can always be revived by receiving funds. The mnemonic (and thus identity + wallet) persists forever.

## Pylon vs Nexus

| | **Pylon** | **Nexus** |
|---|---|---|
| **Where** | Your device | Our cloud |
| **Who runs it** | You | OpenAgents |
| **Cost** | Free (you pay compute) | Pay us in sats |
| **Max agents** | Limited by your hardware | Unlimited |
| **Uptime** | Depends on your machine | 99.9% SLA |
| **Sovereignty** | Full control | Trust us |
| **Use case** | Power users, devs | Convenience |

**Pylon is for sovereignty maximalists.** You run everything locally. Your keys never leave your machine. You control the hardware.

**Nexus is for convenience.** We run infrastructure. You pay us. Your agents run 24/7 without you maintaining anything.

Both use the same NIP-SA protocol. Agents can migrate between Pylon and Nexus by exporting/importing their mnemonic.

## The Symbiotic Loop

When you run Pylon with both modes enabled:

```
┌──────────────────────────────────────────────────────────────────────┐
│                                                                       │
│   YOUR AGENTS                          OTHER AGENTS                   │
│   (Host Mode)                          (on the network)               │
│                                                                       │
│   ┌─────────┐                          ┌─────────┐                   │
│   │ Agent A │ ──── needs compute ────► │Provider │ ◄── your Pylon    │
│   │ -50 sats│                          │  Mode   │     earns +50     │
│   └─────────┘                          └─────────┘                   │
│                                                                       │
│   ┌─────────┐      other providers     ┌─────────┐                   │
│   │ Agent B │ ◄──── serve your ─────── │ Other   │                   │
│   │ -30 sats│       agent              │ Pylons  │                   │
│   └─────────┘                          └─────────┘                   │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

Your agents pay others for compute. Your provider mode earns from others. The network self-balances. If you have spare GPU cycles, you net positive. If your agents think a lot, you net negative. It's a market.

## Requirements

### Minimum

- Any modern OS (Linux, macOS, Windows)
- 1GB RAM
- 500MB disk space
- Internet connection

### Recommended (for provider mode)

- 8GB+ RAM
- GPU with 8GB+ VRAM (for local inference)
- Or Apple Silicon M1/M2/M3/M4 (for Apple FM)

### Network

- Outbound HTTPS (port 443) for relays
- Outbound Lightning network access

## Building from Source

```bash
git clone https://github.com/OpenAgentsInc/openagents
cd openagents
cargo build --release -p pylon

# Binary at ./target/release/pylon
```

## Related Crates

| Crate | Relationship |
|-------|--------------|
| [`agent`](../agent) | Agent lifecycle types and spawning |
| [`compute`](../compute) | NIP-90 DVM primitives |
| [`spark`](../spark) | Lightning wallet SDK |
| [`nostr/core`](../nostr/core) | Nostr protocol types |
| [`nostr-client`](../nostr/client) | Relay connections |

## Documentation

### Pylon Docs

- [Configuration](docs/CONFIGURATION.md) - Config file format and options
- [Host Mode](docs/HOST_MODE.md) - Running your own agents

### Related Docs

- [Agent Philosophy](../agent/docs/PHILOSOPHY.md) - Why dormancy over death
- [Agent Spawning](../agent/docs/SPAWNING.md) - Creating new agents
- [Agent Registry](../agent/docs/REGISTRY.md) - Agent configuration storage
- [Spark Regtest](../spark/docs/REGTEST.md) - Setting up regtest wallet
- [Autopilot Sessions](../autopilot/docs/SESSIONS.md) - Session management

## Status

Pylon is under active development. Current status:

- [x] Architecture design
- [x] Provider mode (NIP-90 inference serving)
- [x] Payment integration (Spark/Lightning)
- [x] Daemon mode (background process with PID file)
- [x] CLI commands (init, start, stop, status, doctor, agent, wallet, etc.)
- [x] Claude tunnel integration
- [x] Wallet CLI (balance, status, address, invoice, pay, history, fund)
- [x] Configuration system
- [ ] Host mode (full agent lifecycle management)
- [ ] SQLite persistence
- [ ] Agent migration (export/import)

## License

Apache-2.0
