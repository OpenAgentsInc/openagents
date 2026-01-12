# Pylon Documentation

Pylon is the local runtime for sovereign AI agents in the OpenAgents ecosystem. It serves two purposes:

1. **Host Mode**: Run your own sovereign agents that pay for their own compute
2. **Provider Mode**: Earn Bitcoin by selling compute to agents on the network

Both modes can run simultaneously on the same machine.

## Documentation Index

### Getting Started
- [Quick Start Guide](./QUICKSTART.md) - Get up and running in 5 minutes
- [Installation](./INSTALLATION.md) - Detailed installation instructions

### Architecture
- [System Architecture](./ARCHITECTURE.md) - High-level system design
- [Daemon Infrastructure](./DAEMON.md) - Process management and IPC
- [Database Schema](./DATABASE.md) - Persistence layer and data model

### Operating Modes
- [Host Mode](./HOST_MODE.md) - Running sovereign agents
- [Provider Mode](./PROVIDER_MODE.md) - Earning Bitcoin as a compute provider

### Reference
- [CLI Reference](./CLI.md) - Complete command-line interface documentation
- [Configuration](./CONFIGURATION.md) - Config file format and options
- [Events and Actions](./EVENTS.md) - Domain events and agent actions

### Development
- [Development Guide](./DEVELOPMENT.md) - Building, testing, contributing
- [Troubleshooting](./TROUBLESHOOTING.md) - Common issues and solutions

## Quick Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         PYLON DAEMON                            │
│  ~/.openagents/pylon/pylon.pid    ~/.openagents/pylon/control.sock    ~/.openagents/pylon/pylon.db│
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────┐       ┌─────────────────────────────┐  │
│  │     HOST MODE       │       │      PROVIDER MODE          │  │
│  │                     │       │                             │  │
│  │  ┌───────────────┐  │       │  ┌───────────────────────┐  │  │
│  │  │ Agent Runner  │  │       │  │    DVM Service        │  │  │
│  │  │   Process 1   │  │       │  │                       │  │  │
│  │  └───────────────┘  │       │  │  NIP-90 Job Requests  │  │  │
│  │  ┌───────────────┐  │       │  │         ↓             │  │  │
│  │  │ Agent Runner  │  │       │  │  Backend Registry     │  │  │
│  │  │   Process 2   │  │       │  │    (Ollama, etc)      │  │  │
│  │  └───────────────┘  │       │  │         ↓             │  │  │
│  │        ...          │       │  │  Job Results + Payment│  │  │
│  └─────────────────────┘       │  └───────────────────────┘  │  │
│                                │                             │  │
│                                └─────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  Nostr Relays   │
                    │  (NIP-90 DVMs)  │
                    └─────────────────┘
```

## Key Concepts

### Sovereign Agents
Agents are autonomous entities with their own:
- **Identity**: Nostr keypair (npub/nsec)
- **Wallet**: Bitcoin wallet for paying compute costs
- **Lifecycle**: Spawning → Active → Dormant states

### NIP-90 Data Vending Machines
Provider mode implements NIP-90, allowing the daemon to:
- Listen for job requests on Nostr relays
- Process jobs using local inference backends
- Receive Lightning payments for completed work

### Self-Daemonizing
Pylon is a single binary that can:
- Fork into the background (`pylon start`)
- Manage its own PID file
- Accept control commands via Unix socket
- Gracefully shutdown (`pylon stop`)

## File Locations

| Path | Description |
|------|-------------|
| `~/.openagents/pylon/pylon.pid` | Daemon process ID |
| `~/.openagents/pylon/control.sock` | Unix socket for IPC |
| `~/.openagents/pylon/pylon.db` | SQLite database |
| `~/.openagents/pylon/config.toml` | Configuration file |
| `~/.openagents/pylon/identity.mnemonic` | Provider identity seed |
| `~/.openagents/agents/` | Agent configurations |

## Version

This documentation covers Pylon v0.1.0 as part of the OpenAgents project.
