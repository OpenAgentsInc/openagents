# Quick Start Guide

Get Pylon running in 5 minutes.

## Prerequisites

- Rust 1.75+ (for building from source)
- An inference backend (Ollama recommended)
- Bitcoin wallet (for provider mode payments)

## Step 1: Install Pylon

```bash
# From the OpenAgents repository
cd crates/pylon
cargo build --release

# The binary is at target/release/pylon
# Optionally install to PATH:
cargo install --path .
```

## Step 2: Install an Inference Backend

Pylon needs a local inference backend. The easiest is Ollama:

```bash
# macOS
brew install ollama

# Linux
curl -fsSL https://ollama.com/install.sh | sh

# Start Ollama
ollama serve

# Pull a model
ollama pull llama3.2
```

## Step 3: Initialize Pylon

```bash
# Create identity and default configuration
pylon init

# This creates:
# - ~/.config/pylon/config.toml
# - ~/.config/pylon/identity.mnemonic (BACK THIS UP!)
```

## Step 4: Start the Daemon

```bash
# Start in background (default)
pylon start

# Or start in foreground for debugging
pylon start -f

# Check status
pylon status
```

## Step 5: Connect Local Claude (Optional)

If you want Claude to run on your machine for the web UI, connect the tunnel session:

```bash
# From the web UI, copy the tunnel URL and run:
pylon connect --tunnel-url wss://openagents-web.openagents.workers.dev/api/tunnel/ws/tunnel?session_id=...&token=...
```

Keep this running while you use the Claude chat overlay.

## Step 6: Verify Everything Works

```bash
# Run diagnostics
pylon doctor

# Expected output:
# Identity: OK (npub1...)
# Backends:
#   ollama: Available (default)
# Relays:
#   wss://nexus.openagents.com: OK
```

## What's Next?

### Run Your Own Agents (Host Mode)

```bash
# Spawn a new agent
pylon agent spawn --name myagent

# List agents
pylon agent list

# View agent details
pylon agent info myagent
```

### Earn Bitcoin (Provider Mode)

Provider mode is enabled by default. As jobs come in from the Nostr network, Pylon will process them and earn sats.

```bash
# View earnings
pylon earnings

# Check status (shows jobs completed, earnings)
pylon status
```

### Run Both Modes

```bash
# Default: both modes active
pylon start

# Or explicitly:
pylon start --mode both

# Provider only:
pylon start --mode provider

# Host only:
pylon start --mode host
```

## Stopping Pylon

```bash
# Graceful shutdown
pylon stop

# Force kill (if graceful fails)
pylon stop --force
```

## Troubleshooting

### "No inference backends detected"

Make sure Ollama (or another backend) is running:
```bash
ollama serve
pylon doctor
```

### "Identity not initialized"

Run `pylon init` first.

### "Daemon is already running"

Either stop it first (`pylon stop`) or check if a stale PID file exists:
```bash
cat ~/.openagents/pylon/pylon.pid
# If the process doesn't exist, remove the file:
rm ~/.openagents/pylon/pylon.pid
```

## Next Steps

- Read the [Architecture](./ARCHITECTURE.md) to understand how Pylon works
- Learn about [Host Mode](./HOST_MODE.md) for running agents
- Learn about [Provider Mode](./PROVIDER_MODE.md) for earning Bitcoin
- See the full [CLI Reference](./CLI.md) for all commands
