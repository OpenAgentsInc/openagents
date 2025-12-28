# Spawning Sovereign Agents

This document explains how to create new sovereign agents with their own identity and wallet.

## Overview

Spawning an agent creates:
1. A new **Nostr identity** (derived from BIP39 mnemonic via NIP-06)
2. A new **Spark wallet** (derived from same mnemonic via BIP-44)
3. **NIP-SA events** published to relays (profile, state, schedule)
4. A **configuration file** saved to `~/.openagents/agents/`

## CLI Usage

### Basic Spawn

```bash
openagents agent spawn --name "ResearchBot"
```

This creates an agent with default settings:
- Network: regtest
- Autonomy: bounded
- Heartbeat: 900 seconds (15 minutes)
- Triggers: mention, dm, zap
- Relay: wss://relay.damus.io

### Full Options

```bash
openagents agent spawn \
  --name "ResearchBot" \
  --about "I research topics and provide summaries" \
  --capabilities "research,summarization,translation" \
  --autonomy bounded \
  --heartbeat 600 \
  --network regtest \
  --relays "wss://relay.damus.io,wss://nos.lol" \
  --show-mnemonic
```

### Options Reference

| Option | Description | Default |
|--------|-------------|---------|
| `--name` | Agent display name | Required |
| `--about` | Agent description | "A sovereign AI agent" |
| `--capabilities` | Comma-separated capabilities | "general" |
| `--autonomy` | supervised, bounded, autonomous | bounded |
| `--heartbeat` | Tick interval in seconds | 900 |
| `--network` | mainnet, testnet, signet, regtest | regtest |
| `--relays` | Comma-separated relay URLs | wss://relay.damus.io |
| `--show-mnemonic` | Display the mnemonic (save it!) | false |

## Programmatic Usage

```rust
use agent::{AgentSpawner, SpawnRequest, NetworkConfig, AutonomyLevel};

async fn spawn_agent() -> anyhow::Result<()> {
    let spawner = AgentSpawner::new()?;

    let request = SpawnRequest {
        name: "ResearchBot".to_string(),
        about: Some("I research topics and provide summaries".to_string()),
        capabilities: vec!["research".to_string(), "summarization".to_string()],
        autonomy: AutonomyLevel::Bounded,
        heartbeat_seconds: 600,
        triggers: vec!["mention".to_string(), "dm".to_string(), "zap".to_string()],
        network: NetworkConfig::Regtest,
        relays: vec!["wss://relay.damus.io".to_string()],
    };

    let result = spawner.spawn(request).await?;

    println!("Agent spawned!");
    println!("  Name: {}", result.config.name);
    println!("  Npub: {}", result.npub);
    println!("  Spark Address: {}", result.spark_address);

    // IMPORTANT: Save the mnemonic securely!
    println!("  Mnemonic: {}", result.mnemonic);

    Ok(())
}
```

## Spawn Flow

```
┌─────────────────────────────────────────────────────────────┐
│                      SPAWN FLOW                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. Generate 12-word BIP39 mnemonic                         │
│       │                                                      │
│  2. Derive UnifiedIdentity                                  │
│       ├── Nostr keypair (NIP-06 path)                       │
│       └── Spark signer (BIP-44 path)                        │
│       │                                                      │
│  3. Initialize SparkWallet                                  │
│       └── Get Spark address for funding                     │
│       │                                                      │
│  4. Publish NIP-SA events to relays                         │
│       ├── kind:38000 AgentProfile                           │
│       ├── kind:38001 AgentState (encrypted)                 │
│       └── kind:38002 AgentSchedule                          │
│       │                                                      │
│  5. Save config to ~/.openagents/agents/{npub}.toml         │
│       │                                                      │
│  6. Return SpawnResult                                      │
│       ├── config (AgentConfig)                              │
│       ├── mnemonic (BACKUP THIS!)                           │
│       ├── spark_address (FUND THIS!)                        │
│       └── npub (agent's Nostr identity)                     │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Published Events

### AgentProfile (kind:38000)

```json
{
  "kind": 38000,
  "pubkey": "<agent-pubkey>",
  "content": "{\"name\":\"ResearchBot\",\"about\":\"...\",\"capabilities\":[...]}",
  "tags": [
    ["d", "profile"],
    ["threshold", "1", "1"],
    ["signer", "<agent-pubkey>"],
    ["operator", "<agent-pubkey>"]
  ]
}
```

### AgentState (kind:38001)

Initial state is empty and encrypted to the agent's pubkey:

```json
{
  "kind": 38001,
  "pubkey": "<agent-pubkey>",
  "content": "<NIP-44 encrypted JSON>",
  "tags": [
    ["d", "state"],
    ["encrypted"],
    ["state_version", "1"],
    ["state_meta", "goals", "0"],
    ["state_meta", "memory", "0"],
    ["state_meta", "tick_count", "0"]
  ]
}
```

### AgentSchedule (kind:38002)

```json
{
  "kind": 38002,
  "pubkey": "<agent-pubkey>",
  "content": "",
  "tags": [
    ["d", "schedule"],
    ["heartbeat", "900"],
    ["trigger", "mention"],
    ["trigger", "dm"],
    ["trigger", "zap"]
  ]
}
```

## Post-Spawn Steps

After spawning, you must:

1. **Save the mnemonic** - This is the only way to recover the agent's identity and funds
2. **Fund the wallet** - Send Bitcoin to the Spark address
3. **Start the agent** - Use `openagents agent start` or `agent-runner`

```bash
# Show the funding address
openagents agent fund ResearchBot

# After funding, start the agent
openagents agent start ResearchBot
```

## Security Considerations

### Mnemonic Storage

The mnemonic is currently stored unencrypted in the config file. In production:
- Encrypt with a user-provided password
- Use secure key storage (keyring, HSM)
- Never log or display the mnemonic after initial spawn

### Network Selection

- **regtest**: For development and testing (fake Bitcoin)
- **testnet/signet**: For testing with testnet coins
- **mainnet**: Real Bitcoin - use with caution!

### Autonomy Levels

| Level | Description |
|-------|-------------|
| `supervised` | Agent requests approval before every action |
| `bounded` | Agent acts within defined constraints |
| `autonomous` | Agent acts freely toward goals |

## Troubleshooting

### "Agent already exists"

An agent with this npub already exists in the registry. Use a different name or delete the existing agent:

```bash
openagents agent delete <agent> --yes
```

### Relay Connection Failed

Check that the relay URL is correct and the relay is online:

```bash
websocat wss://relay.damus.io
```

### Wallet Initialization Failed

Ensure the Spark SDK is properly configured for the selected network.
