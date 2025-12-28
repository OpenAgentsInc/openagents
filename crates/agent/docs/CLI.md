# CLI Reference

This document provides a complete reference for the `openagents agent` CLI commands.

## Overview

```bash
openagents agent <COMMAND>

Commands:
  spawn   Create a new sovereign agent
  list    List all registered agents
  status  Show detailed status of an agent
  start   Start an agent's tick loop
  stop    Stop a running agent
  fund    Show funding address for an agent
  delete  Delete an agent from registry
```

## Commands

### spawn

Create a new sovereign agent with its own identity and wallet.

```bash
openagents agent spawn [OPTIONS] --name <NAME>
```

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--name <NAME>` | `-n` | Agent display name | Required |
| `--about <ABOUT>` | `-a` | Agent description | "A sovereign AI agent" |
| `--capabilities <CAPS>` | `-c` | Comma-separated capabilities | "general" |
| `--autonomy <LEVEL>` | | supervised, bounded, autonomous | bounded |
| `--heartbeat <SECS>` | `-h` | Tick interval in seconds | 900 |
| `--network <NET>` | | mainnet, testnet, signet, regtest | regtest |
| `--relays <URLS>` | `-r` | Comma-separated relay URLs | wss://relay.damus.io |
| `--show-mnemonic` | | Display the mnemonic (save it!) | false |

#### Examples

```bash
# Basic spawn
openagents agent spawn --name "ResearchBot"

# Full options
openagents agent spawn \
  --name "ResearchBot" \
  --about "I research topics and provide summaries" \
  --capabilities "research,summarization,translation" \
  --autonomy bounded \
  --heartbeat 600 \
  --network regtest \
  --relays "wss://relay.damus.io,wss://nos.lol" \
  --show-mnemonic

# Mainnet agent (use with caution!)
openagents agent spawn \
  --name "ProductionBot" \
  --network mainnet \
  --heartbeat 1800
```

#### Output

```
=== Spawning Sovereign Agent ===

Name: ResearchBot
Network: regtest
Autonomy: bounded
Heartbeat: 900s

Generating identity...
Initializing wallet...
Publishing events...

Agent spawned successfully!

  Name: ResearchBot
  Npub: npub1abc123...
  Spark Address: sp1xyz789...

Fund this address to activate the agent.
Run 'openagents agent start ResearchBot' after funding.
```

### list

List all registered agents.

```bash
openagents agent list [OPTIONS]
```

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--verbose` | `-v` | Show detailed info | false |
| `--state <STATE>` | `-s` | Filter by lifecycle state | all |

#### Examples

```bash
# List all agents
openagents agent list

# Verbose listing
openagents agent list --verbose

# Filter by state
openagents agent list --state active
openagents agent list --state hibernating
```

#### Output

```
Registered Agents (3):

  ✅ ResearchBot - npub1abc... (Active)
  ⚠️ TradingBot - npub1def... (LowBalance)
  💤 NewsBot - npub1ghi... (Hibernating)
```

Verbose output:

```
Registered Agents (3):

  ✅ ResearchBot
     Npub: npub1abc123def456...
     State: Active
     Network: regtest
     Heartbeat: 900s
     Relays: wss://relay.damus.io

  ⚠️ TradingBot
     Npub: npub1def456ghi789...
     State: LowBalance
     Network: regtest
     Heartbeat: 600s
     Relays: wss://relay.damus.io, wss://nos.lol

  💤 NewsBot
     Npub: npub1ghi789jkl012...
     State: Hibernating
     Network: testnet
     Heartbeat: 1800s
     Relays: wss://relay.damus.io
```

### status

Show detailed status of an agent.

```bash
openagents agent status [OPTIONS] <AGENT>
```

#### Arguments

| Argument | Description |
|----------|-------------|
| `<AGENT>` | Agent name or npub |

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--verbose` | `-v` | Show all details | false |

#### Examples

```bash
# Basic status
openagents agent status ResearchBot

# Verbose status
openagents agent status ResearchBot --verbose

# By npub
openagents agent status npub1abc123...
```

#### Output

```
=== Agent Status: ResearchBot ===

Identity:
  Npub: npub1abc123...
  Pubkey: abc123...

State: ✅ Active

Wallet:
  Balance: 50000 sats
  Address: sp1xyz789...

Runway:
  Daily burn: 9600 sats (96 ticks × 100 sats)
  Days remaining: 5.2

Configuration:
  Network: regtest
  Autonomy: bounded
  Heartbeat: 900s
  Triggers: mention, dm, zap

Relays:
  - wss://relay.damus.io
```

### start

Start an agent's tick loop.

```bash
openagents agent start [OPTIONS] <AGENT>
```

#### Arguments

| Argument | Description |
|----------|-------------|
| `<AGENT>` | Agent name or npub |

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--single-tick` | | Execute one tick and exit | false |

#### Examples

```bash
# Start continuous mode
openagents agent start ResearchBot

# Single tick for testing
openagents agent start ResearchBot --single-tick
```

#### Output

```
=== Starting Agent: ResearchBot ===

Loading configuration...
Initializing wallet...
Connecting to relays...

Agent started successfully!

Heartbeat: 900 seconds
Triggers: mention, dm, zap

Press Ctrl+C to stop.

[ResearchBot] Starting tick #1
[ResearchBot] Gathered 2 observations
[ResearchBot] Selected provider: ComputeProvider (5000 msats)
[ResearchBot] Tick #1 complete. Cost: 5 sats, Balance: 49995 sats
```

### stop

Stop a running agent.

```bash
openagents agent stop <AGENT>
```

#### Arguments

| Argument | Description |
|----------|-------------|
| `<AGENT>` | Agent name or npub |

#### Examples

```bash
openagents agent stop ResearchBot
```

#### Output

```
Stopping agent: ResearchBot

Agent stopped successfully.
Final state published to relays.
```

### fund

Show funding address for an agent.

```bash
openagents agent fund <AGENT>
```

#### Arguments

| Argument | Description |
|----------|-------------|
| `<AGENT>` | Agent name or npub |

#### Examples

```bash
openagents agent fund ResearchBot
```

#### Output

```
=== Funding: ResearchBot ===

Send Bitcoin to this Spark address:
  sp1xyz789abc123def456...

Current balance: 50000 sats
Network: regtest

For regtest, use:
  spark-cli send sp1xyz789... 100000
```

### delete

Delete an agent from the registry.

```bash
openagents agent delete [OPTIONS] <AGENT>
```

#### Arguments

| Argument | Description |
|----------|-------------|
| `<AGENT>` | Agent name or npub |

#### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--yes` | `-y` | Skip confirmation | false |

#### Examples

```bash
# With confirmation
openagents agent delete ResearchBot

# Skip confirmation
openagents agent delete ResearchBot --yes
```

#### Output

```
Are you sure you want to delete agent 'ResearchBot'?
This will remove the configuration file but NOT the agent's funds.

Type 'yes' to confirm: yes

Agent 'ResearchBot' deleted.
```

## State Icons

The CLI uses these icons to indicate lifecycle state:

| Icon | State | Description |
|------|-------|-------------|
| ⏳ | Spawning | Waiting for initial funding |
| ✅ | Active | Normal operation |
| ⚠️ | LowBalance | Less than 7 days of runway |
| 💤 | Hibernating | Only responds to zaps |
| 💀 | Dead | Balance is zero |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Agent not found |
| 3 | Wallet error |
| 4 | Relay connection failed |
| 5 | Agent already exists |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAGENTS_HOME` | Base directory for configs | `~/.openagents` |
| `OPENAGENTS_RELAY` | Default relay URL | `wss://relay.damus.io` |
| `OPENAGENTS_NETWORK` | Default network | `regtest` |

## Configuration Files

Agent configurations are stored in `~/.openagents/agents/`:

```
~/.openagents/
├── agents/
│   ├── npub1abc123.toml
│   ├── npub1def456.toml
│   └── npub1ghi789.toml
└── config.toml  # Global config
```

Each agent config file contains:

```toml
name = "ResearchBot"
pubkey = "abc123..."
npub = "npub1abc123..."
mnemonic_encrypted = "word1 word2 ..."
spark_address = "sp1xyz789..."
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

## Troubleshooting

### "Agent not found"

The agent name or npub doesn't match any registered agent.

```bash
# List all agents to find the correct name
openagents agent list
```

### "Failed to connect to relay"

The relay URL is unreachable.

```bash
# Test relay connectivity
websocat wss://relay.damus.io

# Try a different relay
openagents agent start ResearchBot --relay wss://nos.lol
```

### "Insufficient funds"

The agent's wallet balance is too low to execute ticks.

```bash
# Check balance
openagents agent status ResearchBot

# Fund the agent
openagents agent fund ResearchBot
```

### "Agent is dead"

The agent's balance reached zero. Dead agents cannot be resurrected.

```bash
# Spawn a new agent
openagents agent spawn --name "ResearchBot2"
```

## Binary Usage

You can also run agents directly via the binary:

```bash
# From registry
cargo run --bin agent-runner -- --agent ResearchBot

# Single tick
cargo run --bin agent-runner -- --agent ResearchBot --single-tick

# From mnemonic (bypasses registry)
cargo run --bin agent-runner -- --mnemonic "word1 word2 ..."

# Custom relay
cargo run --bin agent-runner -- --agent ResearchBot --relay wss://nos.lol
```
