# OANIX - OpenAgents NIX

The agent operating system runtime.

OANIX is the OS layer that wraps the OpenAgents runtime. When you run `oanix`, it discovers your environment (hardware, compute backends, network, identity) and prepares for autonomous operation.

## Quick Start

```bash
# Build
cargo build -p oanix

# Run
./target/debug/oanix
```

## Output Example

```
OANIX v0.1.0 - OpenAgents NIX
═══════════════════════════════════════════════════════════════

Discovering environment...

Hardware
  CPU: Apple M2 Pro (12 cores)
  RAM: 16 GB (12 GB available)
  GPU: Apple Silicon GPU (Metal)

Compute Backends
  [OK] Ollama (localhost:11434) - 3 models
       - llama3.2:latest
       - qwen2.5:7b
       - deepseek-coder:6.7b

Network
  [OK] Internet connectivity
  [OK] Nostr relays: 3 connected
       - wss://nexus.openagents.com (42ms)
       - wss://relay.damus.io (68ms)
       - wss://nos.lol (85ms)
  [--] Swarm: not connected

Identity
  [OK] Pubkey: npub1abc...xyz

Situation Assessment
  Environment: Developer (macos)
  Compute: Medium (can run 7B models)
  Connectivity: Full (internet + nostr)

Recommended: Awaiting user direction

Ready. What would you like to do?
```

## Architecture

```
┌─────────────────────────────────────────────┐
│  OANIX = Operating System                   │
│  "What am I? What should I do?"             │
│  ├── Boot sequence                          │
│  ├── Hardware discovery                     │
│  ├── Situation assessment                   │
│  └── Autonomous decision loop               │
├─────────────────────────────────────────────┤
│  Runtime = Execution Engine                 │
│  "How do agents run?"                       │
│  ├── Tick model                             │
│  ├── Filesystem abstraction                 │
│  └── /compute, /containers, /claude         │
└─────────────────────────────────────────────┘
```

## Discovery Phases

1. **Hardware** - CPU, RAM, GPU detection via sysinfo
2. **Compute** - Probe Ollama, Apple FM, llama.cpp endpoints
3. **Network** - Check internet, Nostr relays, swarm peers
4. **Identity** - Check for pylon identity and wallet

## See Also

- [docs/OANIX.md](../../docs/OANIX.md) - Full vision document
- [crates/runtime/](../runtime/) - The underlying execution engine
- [crates/pylon/](../pylon/) - Node software that can host OANIX
