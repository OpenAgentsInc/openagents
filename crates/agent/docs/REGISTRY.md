# Agent Registry

This document describes how agent configurations are stored and managed.

## Overview

The Agent Registry persists agent configurations to disk, enabling:

- Agent survival across restarts
- Lookup by npub or name
- Enumeration of all agents
- State persistence

## Directory Structure

```
~/.config/openagents/agents/          # Linux
~/Library/Preferences/openagents/agents/  # macOS
├── npub1abc123....toml              # Agent config files
├── npub1def456....toml
└── npub1xyz789....toml
```

**Note**: The path uses `dirs::config_dir()` which varies by platform.

| Platform | Path |
|----------|------|
| Linux | `~/.config/openagents/agents/` |
| macOS | `~/Library/Preferences/openagents/agents/` |
| Windows | `C:\Users\<user>\AppData\Roaming\openagents\agents\` |

## File Format

Each agent is stored as a TOML file named `{npub}.toml`:

```toml
# ~/.config/openagents/agents/npub1abc123....toml

name = "MyAgent"
npub = "npub1abc123..."
pubkey = "abc123def456..."
mnemonic_encrypted = "word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12"
spark_address = "sp1xyz..."
state = "active"
created_at = 1703980800
last_active_at = 1703984400
tick_count = 42

[network]
bitcoin = "regtest"

[relays]
urls = ["wss://relay.damus.io", "wss://nos.lol"]

[profile]
name = "MyAgent"
about = "I research topics and provide summaries"
autonomy = "bounded"
capabilities = ["research", "summarization"]
version = "0.1.0"
picture = ""
nip05 = ""

[schedule]
heartbeat_seconds = 900
triggers = ["mention", "dm", "zap"]
active = true

[runway]
low_balance_days = 7
daily_burn_sats = 100
hibernate_threshold_sats = 50
daily_limit_sats = 500
per_tick_limit_sats = 50
```

## Registry API

### Creating a Registry

```rust
use agent::registry::AgentRegistry;

// Use default path
let registry = AgentRegistry::new()?;

// Or use custom path (for testing)
let registry = AgentRegistry::with_path("/custom/path".into())?;
```

### Saving Agents

```rust
use agent::config::AgentConfig;

let config = AgentConfig {
    name: "MyAgent".to_string(),
    npub: "npub1...".to_string(),
    // ... other fields
};

registry.save(&config)?;
// Creates ~/.config/openagents/agents/npub1....toml
```

### Loading Agents

```rust
// By npub (exact match)
let config = registry.load("npub1abc...")?;

// By name (searches all files)
let config = registry.load("MyAgent")?;
```

### Listing All Agents

```rust
let agents = registry.list_all()?;
// Returns Vec<AgentConfig>

for agent in agents {
    println!("{}: {}", agent.name, agent.npub);
}
```

### Checking Existence

```rust
if registry.exists("MyAgent")? {
    println!("Agent exists");
}
```

### Deleting Agents

```rust
registry.delete("npub1abc...")?;
// Or by name
registry.delete("MyAgent")?;
```

## File Naming

Agent files are named by their npub with sanitization:

```rust
fn agent_path(&self, npub: &str) -> PathBuf {
    // Sanitize for filesystem safety
    let safe_name = npub.replace(['/', '\\', ':'], "_");
    self.agents_dir.join(format!("{}.toml", safe_name))
}
```

This ensures:
- No path traversal attacks
- Cross-platform compatibility
- Predictable file locations

## Configuration Fields

### Core Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | String | Human-readable agent name |
| `npub` | String | Nostr public key (bech32) |
| `pubkey` | String | Nostr public key (hex) |
| `mnemonic_encrypted` | String | BIP39 mnemonic (TODO: encryption) |
| `spark_address` | String | Spark wallet address |
| `state` | String | Lifecycle state |
| `created_at` | u64 | Unix timestamp |
| `last_active_at` | u64 | Last activity timestamp |
| `tick_count` | u32 | Total ticks executed |

### Network Configuration

```toml
[network]
bitcoin = "regtest"  # mainnet | testnet | signet | regtest
```

### Relay Configuration

```toml
[relays]
urls = [
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band"
]
```

### Profile Configuration

```toml
[profile]
name = "MyAgent"              # Display name
about = "Description"          # Bio
autonomy = "bounded"           # full | bounded | supervised
capabilities = ["research"]    # Agent capabilities
version = "0.1.0"             # Agent version
picture = ""                   # Profile picture URL
nip05 = ""                     # NIP-05 identifier
```

### Schedule Configuration

```toml
[schedule]
heartbeat_seconds = 900        # Tick interval (15 min default)
triggers = ["mention", "dm", "zap"]  # Event triggers
active = true                  # Whether scheduling is enabled
```

**Triggers**:
- `mention` - Wake on @mention
- `dm` - Wake on direct message
- `zap` - Wake on Lightning zap
- `reply` - Wake on reply to agent's post

### Runway Configuration

```toml
[runway]
low_balance_days = 7           # Days before low balance warning
daily_burn_sats = 100          # Estimated daily cost
hibernate_threshold_sats = 50  # Pause at this balance
daily_limit_sats = 500         # Max spend per day
per_tick_limit_sats = 50       # Max spend per tick
```

## Lifecycle States

| State | Description | File Exists |
|-------|-------------|-------------|
| `spawning` | Created, awaiting funding | Yes |
| `active` | Fully operational | Yes |
| `low_balance` | Running with warnings | Yes |
| `hibernating` | Paused to conserve funds | Yes |
| `dormant` | Zero balance, waiting | Yes |
| `terminated` | Permanently stopped | Yes (may be deleted) |

## Security Considerations

### Mnemonic Storage

**Current State**: Mnemonics stored in plaintext (`mnemonic_encrypted` is misleading)

**Recommended**: Set restrictive permissions:
```bash
chmod 600 ~/.config/openagents/agents/*.toml
chmod 700 ~/.config/openagents/agents/
```

**Future**: Encrypt with user password or system keychain

### File Permissions

The registry does not set permissions explicitly. Users should:

```bash
# Restrict directory access
chmod 700 ~/.config/openagents/agents/

# Restrict individual files
chmod 600 ~/.config/openagents/agents/*.toml
```

## Error Handling

```rust
use agent::registry::{AgentRegistry, RegistryError};

match registry.load("MyAgent") {
    Ok(config) => println!("Loaded: {}", config.name),
    Err(RegistryError::NotFound(name)) => {
        println!("Agent '{}' not found", name);
    }
    Err(RegistryError::Serialization(e)) => {
        println!("Config parse error: {}", e);
    }
    Err(e) => println!("Error: {}", e),
}
```

### Error Types

| Error | Cause | Resolution |
|-------|-------|------------|
| `NoConfigDir` | Can't determine config directory | Check `$HOME` is set |
| `NotFound` | Agent doesn't exist | Check name/npub spelling |
| `AlreadyExists` | Agent with same npub exists | Use different identity |
| `Io` | File system error | Check permissions |
| `Serialization` | Invalid TOML format | Check config syntax |

## Backup and Restore

### Backup All Agents

```bash
# Backup entire agents directory
cp -r ~/.config/openagents/agents/ ~/agents-backup-$(date +%Y%m%d)/
```

### Backup Single Agent

```bash
cp ~/.config/openagents/agents/npub1abc123....toml ~/myagent-backup.toml
```

### Restore

```bash
# Restore entire directory
cp -r ~/agents-backup-20240115/* ~/.config/openagents/agents/

# Restore single agent
cp ~/myagent-backup.toml ~/.config/openagents/agents/npub1abc123....toml
```

## Integration with Pylon

When running in Pylon's host mode:

1. Pylon daemon scans the registry on startup
2. Agents in `active` state are started as subprocesses
3. Agent state changes are persisted back to registry
4. Registry is re-scanned periodically for new agents

```
Pylon Daemon
     │
     ├── Scan Registry
     │   └── ~/.config/openagents/agents/*.toml
     │
     ├── For each active agent:
     │   └── Spawn agent-runner subprocess
     │
     └── Monitor & Update
         └── Persist state changes to registry
```

## Future Enhancements

1. **Mnemonic Encryption** - Password-protected storage
2. **Watch Mode** - React to file changes
3. **Remote Sync** - Sync registry across devices
4. **Compression** - For large agent histories
5. **Migration** - Version-aware config upgrades
