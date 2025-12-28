# Pylon End-to-End Test Instructions

## Overview

This document provides step-by-step instructions for testing the Pylon MVP - a self-daemonizing binary that runs both **Provider Mode** (earn sats by processing NIP-90 jobs) and **Host Mode** (run sovereign agents).

---

## Prerequisites

### Build Pylon

```bash
# From workspace root
cargo build -p pylon --release

# Or for debugging
cargo build -p pylon
```

### Build Agent Runner (for Host Mode)

```bash
cargo build --bin agent-runner --release
```

### Start Ollama Backend (for Provider Mode)

```bash
# Install ollama if needed
brew install ollama  # macOS
# or
curl -fsSL https://ollama.com/install.sh | sh  # Linux

# Start ollama
ollama serve

# Pull a model
ollama pull llama3.2
```

---

## Test 1: Basic Daemon Lifecycle

### Initialize Pylon

```bash
# Create identity and config
cargo run -p pylon --bin pylon -- init

# Expected output:
# Initializing Pylon...
#
# Generated new identity:
#   npub: npub1abc123...
#
# IMPORTANT: Back up your seed phrase:
#   word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
#
# Configuration written to ~/.config/pylon/config.toml
```

### Verify Files Created

```bash
# Config file
cat ~/.config/pylon/config.toml

# Identity file
cat ~/.config/pylon/identity.mnemonic
```

### Start Daemon (Foreground)

```bash
# Run in foreground for debugging
cargo run -p pylon --bin pylon -- start -f

# Expected output:
# Starting Pylon in foreground...
# Mode: Both (provider + host)
# Identity: npub1abc...
# Relays: wss://relay.damus.io
# Ollama: Available (default backend)
# Listening for NIP-90 jobs...
```

**To stop**: Press `Ctrl+C`

### Start Daemon (Background)

```bash
# Start daemonized
cargo run -p pylon --bin pylon -- start

# Expected output:
# Pylon daemon started (PID: 12345)

# Check status
cargo run -p pylon --bin pylon -- status

# Expected output:
# Pylon Status
# ============
#
# Daemon: Running (PID: 12345)
# Uptime: 5 seconds
# Modes:  provider, host
#
# Session Stats:
#   Jobs completed: 0
#   Earnings: 0 sats (0 msats)
#
# Identity:
#   Configured
#
# Backends:
#   Available: ollama (default)
#
# Relays:
#   wss://relay.damus.io
```

### Stop Daemon

```bash
# Graceful stop
cargo run -p pylon --bin pylon -- stop

# Expected output:
# Sending shutdown command...
# Pylon daemon stopped

# Verify stopped
cargo run -p pylon --bin pylon -- status
# Expected: "Daemon: Not running"
```

---

## Test 2: Provider Mode (NIP-90 Jobs)

### Terminal 1: Start Provider

```bash
RUST_LOG=info cargo run -p pylon --bin pylon -- start -f --mode provider
```

### Terminal 2: Send Test Job

Use the agent-customer binary or send a manual NIP-90 event:

```bash
# Option A: Use agent-customer (if available)
cargo run --bin agent-customer -- \
  --discover \
  --prompt "What is 2+2?" \
  --no-wallet

# Option B: Manual NIP-90 event (requires nostr-cli or similar)
# Create a kind:5100 text generation request targeting your provider's pubkey
```

### Expected Provider Output

```
Received job: abc123 (kind:5100)
Processing with ollama (llama3.2)...
Job completed in 1.2s
```

### Check Earnings

```bash
cargo run -p pylon --bin pylon -- earnings

# Expected output:
# Pylon Earnings
# ==============
#
# Summary:
#   Total earned: 0 sats (0 msats)
#   Jobs completed: 1
#
# By Source:
#   job: 0 sats
#
# Recent Earnings (last 10):
# (none - payment not received yet)
```

---

## Test 3: Host Mode (Sovereign Agents)

### Terminal 1: Start Host

```bash
RUST_LOG=info cargo run -p pylon --bin pylon -- start -f --mode host
```

### Terminal 2: Spawn an Agent

```bash
# Spawn a test agent
cargo run -p pylon --bin pylon -- agent spawn --name testbot --network regtest

# Expected output:
# Agent 'testbot' spawned successfully!
#
# Npub: npub1xyz...
# State: spawning (awaiting funding)
#
# Fund address: sp1abc...
#
# The agent wallet needs Bitcoin to operate.
# Send Bitcoin to the address above to activate the agent.
#
# IMPORTANT: Back up the mnemonic phrase:
#   word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
```

### List Agents

```bash
cargo run -p pylon --bin pylon -- agent list

# Expected output:
# Agents:
#
# NAME                 STATE        NPUB
# -------------------- ------------ ----------------------------------------------------------------
# testbot              spawning     npub1xyz...
```

### View Agent Details

```bash
cargo run -p pylon --bin pylon -- agent info testbot

# Expected output:
# Agent: testbot
# ===================
#
# Npub:    npub1xyz...
# State:   spawning
# Network: regtest
#
# Schedule:
#   Heartbeat: 900 seconds
#   Triggers:  mention, dm, zap
#
# Relays:
#   wss://relay.damus.io
#
# Stats:
#   Balance:    0 sats
#   Tick count: 0
#   Last tick:  never
```

### Delete Agent

```bash
cargo run -p pylon --bin pylon -- agent delete testbot

# Prompts for confirmation
# Type 'testbot' to confirm
# Expected: Agent 'testbot' deleted
```

---

## Test 4: Both Modes Simultaneously

### Start Both Modes

```bash
RUST_LOG=info cargo run -p pylon --bin pylon -- start -f --mode both
```

### Expected Output

```
Starting Pylon in foreground...
Mode: Both (provider + host)
Identity: npub1abc...
Relays: wss://relay.damus.io
Ollama: Available (default backend)

Loading agents from registry...
  Found 0 active agents

Listening for NIP-90 jobs...
Host mode active, monitoring agents...
```

### Verify Status

```bash
cargo run -p pylon --bin pylon -- status

# Should show both modes active:
# Modes:  provider, host
```

---

## Test 5: Database Persistence

### Check Database Location

```bash
ls -la ~/.pylon/pylon.db
```

### Query Database Directly

```bash
# List tables
sqlite3 ~/.pylon/pylon.db ".tables"
# Expected: agents  earnings  jobs  migrations  tick_history

# Check migrations
sqlite3 ~/.pylon/pylon.db "SELECT * FROM migrations"

# Check jobs (after running provider)
sqlite3 ~/.pylon/pylon.db "SELECT * FROM jobs"

# Check agents (after spawning)
sqlite3 ~/.pylon/pylon.db "SELECT * FROM agents"
```

---

## Test 6: Diagnostics

### Run Doctor

```bash
cargo run -p pylon --bin pylon -- doctor

# Expected output:
# Pylon Diagnostics
# =================
#
# Identity:
#   ✓ Configured (npub1abc123...)
#
# Backends:
#   ✓ ollama: Available
#   ✗ apple_fm: Not available
#   ✗ llamacpp: Not available
#
# Relays:
#   wss://relay.damus.io
#
# Warnings:
#   None
#
# Status: OK
```

---

## Test 7: Control Socket IPC

### Check Socket Exists

```bash
ls -la ~/.pylon/control.sock
```

### Test Socket Communication

The control socket uses a simple JSON protocol. You can test it manually:

```bash
# While daemon is running
echo '{"command":"Status"}' | nc -U ~/.pylon/control.sock
```

---

## Common Issues

### "Identity not initialized"

```bash
# Solution: Run init first
cargo run -p pylon --bin pylon -- init
```

### "Already running"

```bash
# Solution: Stop existing daemon
cargo run -p pylon --bin pylon -- stop --force

# Or manually
kill $(cat ~/.pylon/pylon.pid)
rm ~/.pylon/pylon.pid
```

### "No backends available"

```bash
# Solution: Start ollama
ollama serve
```

### "Failed to connect to relay"

```bash
# Solution: Check network and relay URL
# Edit ~/.config/pylon/config.toml to use a working relay
```

---

## Log Levels

```bash
# Minimal logging
RUST_LOG=warn cargo run -p pylon --bin pylon -- start -f

# Normal logging
RUST_LOG=info cargo run -p pylon --bin pylon -- start -f

# Debug logging
RUST_LOG=debug cargo run -p pylon --bin pylon -- start -f

# Trace all DVM messages
RUST_LOG=pylon=debug,compute=trace cargo run -p pylon --bin pylon -- start -f
```

---

## File Locations

| File | Location | Purpose |
|------|----------|---------|
| Config | `~/.config/pylon/config.toml` | TOML configuration |
| Identity | `~/.config/pylon/identity.mnemonic` | 12-word seed phrase |
| PID file | `~/.pylon/pylon.pid` | Process tracking |
| Control socket | `~/.pylon/control.sock` | IPC communication |
| Database | `~/.pylon/pylon.db` | SQLite persistence |
| Agent registry | `~/.config/openagents/agents/` | Agent configurations |

---

## Two-Computer Test (Provider + Customer)

### Computer A (Provider)

```bash
# Initialize and start provider
cargo run -p pylon --bin pylon -- init
RUST_LOG=info cargo run -p pylon --bin pylon -- start -f --mode provider

# Note the npub shown in output
```

### Computer B (Customer)

```bash
# Send job request to provider
cargo run --bin agent-customer -- \
  --channel <PROVIDER_NPUB> \
  --prompt "Explain quantum computing in one sentence" \
  --no-wallet
```

### Verify on Provider

The provider should show:
```
Received job: <job_id> (kind:5100)
Processing with ollama...
Job completed, result published
```

---

## Cleanup

```bash
# Stop daemon
cargo run -p pylon --bin pylon -- stop

# Remove all data (DESTRUCTIVE)
rm -rf ~/.pylon
rm -rf ~/.config/pylon
rm -rf ~/.config/openagents/agents

# Re-initialize
cargo run -p pylon --bin pylon -- init
```
