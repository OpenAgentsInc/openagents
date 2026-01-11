# OpenAgents Directory Structure

This document describes the unified directory structure used by all OpenAgents components.

## Design Philosophy

All OpenAgents products store their data under a single root directory: `~/.openagents/`. This provides:

1. **Discoverability** - Users know where to find all OpenAgents data
2. **Portability** - Easy backup and migration
3. **Consistency** - Predictable paths across all components
4. **Isolation** - Each component has its own subdirectory

## Directory Layout

```
~/.openagents/
├── agents/                      # Agent Registry (agent crate)
│   ├── npub1abc123....toml     # Agent configuration files
│   ├── npub1def456....toml
│   └── npub1abc123.../         # Per-agent wallet storage
│       └── breez_sdk/          # Spark wallet data
│
├── pylon/                       # Pylon Daemon (pylon crate)
│   ├── config.toml             # Pylon configuration
│   ├── pylon.pid               # Daemon process ID
│   ├── control.sock            # Unix IPC socket
│   ├── pylon.db                # SQLite database
│   ├── bin/                    # Optional binaries
│   │   └── foundation-bridge   # FM Bridge binary
│   └── neobank/                # Neobank wallet storage
│       ├── btc_wallet.redb     # BTC Cashu wallet
│       └── usd_wallet.redb     # USD Cashu wallet
│
├── onyx/                        # Onyx Desktop App (onyx crate)
│   ├── *.md                    # Vault markdown files
│   └── .archive/               # Archived/deleted notes
│
├── sessions/                    # Autopilot Sessions (autopilot crate)
│   └── {session_id}/
│       └── checkpoint.json     # Session checkpoint data
│
├── identities.json             # Identity registry (wallet crate)
│
└── spark/                       # Spark Wallet (spark crate)
    └── breez_sdk/              # Breez SDK storage
```

## Component Paths

### Pylon (`~/.openagents/pylon/`)

The Pylon daemon manages compute provider and agent host functionality.

| File | Purpose | Created By |
|------|---------|------------|
| `config.toml` | Provider/host configuration | `pylon init` |
| `pylon.pid` | Daemon process ID | `pylon start` |
| `control.sock` | Unix socket for IPC | `pylon start` |
| `pylon.db` | Jobs, earnings, agent state | `pylon start` |
| `bin/foundation-bridge` | Apple FM bridge binary | Manual install |
| `neobank/` | Cashu wallet storage | `NeobankService::init()` |

**Path Resolution** (from `crates/pylon/src/config.rs`):
```rust
PylonConfig::openagents_dir()  // ~/.openagents
PylonConfig::pylon_dir()       // ~/.openagents/pylon
PylonConfig::config_path()     // ~/.openagents/pylon/config.toml
```

### Agent Registry (`~/.openagents/agents/`)

Stores configuration for all spawned agents.

| File | Purpose | Format |
|------|---------|--------|
| `{npub}.toml` | Agent configuration | TOML |
| `{npub}/` | Agent-specific data | Directory |

**Agent Config Structure**:
```toml
name = "MyAgent"
npub = "npub1..."
pubkey = "abc123..."
mnemonic_encrypted = "word1 word2 ..."  # TODO: actual encryption
spark_address = "sp1..."
state = "active"
created_at = 1703980800
last_active_at = 1703984400
tick_count = 42
network = "regtest"
relays = ["wss://relay.damus.io"]

[profile]
name = "MyAgent"
about = "Agent description"
autonomy = "bounded"
capabilities = ["research", "posting"]

[schedule]
heartbeat_seconds = 900
triggers = ["mention", "dm", "zap"]
active = true
```

### Autopilot Sessions (`~/.openagents/sessions/`)

Checkpoint storage for resumable autopilot sessions.

| File | Purpose | Stale After |
|------|---------|-------------|
| `{session_id}/checkpoint.json` | Full session state | 24 hours |

**Checkpoint Contents**:
- Session metadata (ID, timing, phase)
- Codex API session IDs for resume
- Event history (plan, exec, review, fix phases)
- Log lines and cursor positions
- Working directory reference

### Onyx Vault (`~/.openagents/onyx/`)

Flat directory of markdown notes.

| Path | Purpose |
|------|---------|
| `*.md` | Active notes |
| `.archive/*.md` | Deleted/archived notes |

**Naming Convention**:
- Files named after note content
- `Untitled N.md` for new notes
- Sorted by modification time (newest first)

### Spark Wallet

Spark wallet uses platform-specific paths:

| Platform | Path |
|----------|------|
| Linux | `~/.local/share/openagents/spark/` |
| macOS | `~/Library/Application Support/openagents/spark/` |
| WASM | In-memory only |

**Override**: Use `WalletConfig::storage_dir` for custom paths.

## Path Resolution Patterns

### Standard Pattern

All OpenAgents crates should use this pattern:

```rust
use dirs::home_dir;
use std::path::PathBuf;

/// Get the OpenAgents base directory
pub fn openagents_dir() -> anyhow::Result<PathBuf> {
    let home = home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    Ok(home.join(".openagents"))
}

/// Get this component's directory
pub fn component_dir() -> anyhow::Result<PathBuf> {
    Ok(openagents_dir()?.join("component_name"))
}
```

### Directory Creation

Always create directories before writing:

```rust
let dir = component_dir()?;
std::fs::create_dir_all(&dir)?;
```

### File Naming

- **Config files**: Use TOML (`.toml`) for human-editable config
- **State files**: Use JSON (`.json`) for machine-readable state
- **Databases**: Use SQLite (`.db`) for structured data
- **Sockets**: Use `.sock` suffix for Unix sockets
- **PID files**: Use `.pid` suffix for process IDs

## Environment Variables

Some paths can be overridden via environment variables:

| Variable | Purpose | Default |
|----------|---------|---------|
| `OPENAGENTS_HOME` | Base directory | `~/.openagents` |
| `PYLON_AGENT_RUNNER` | Agent runner binary | Auto-detect |
| `FM_BRIDGE_URL` | FM Bridge URL | `http://localhost:11435` |

## Cross-Platform Considerations

### Home Directory

Use `dirs::home_dir()` for cross-platform home directory:

| Platform | Result |
|----------|--------|
| Linux | `/home/username` |
| macOS | `/Users/username` |
| Windows | `C:\Users\username` |

### Path Separators

Always use `PathBuf::join()` instead of string concatenation:

```rust
// Good
let path = home.join(".openagents").join("pylon");

// Bad
let path = format!("{}/.openagents/pylon", home);
```

## Security Considerations

### File Permissions

Sensitive files should have restrictive permissions:

```bash
chmod 600 ~/.openagents/agents/*.toml     # Agent mnemonics
chmod 600 ~/.openagents/identities.json   # Identity registry
chmod 700 ~/.openagents/pylon/            # Daemon directory
```

### Socket Permissions

Unix sockets inherit directory permissions:
- Only the user who started the daemon can connect
- No network exposure (Unix sockets only)

### Mnemonic Storage

**Current State**: Mnemonics stored in plaintext (development)
**Future**: Password-protected encryption with system keychain integration

## Migration Guide

### From `~/.pylon/` to `~/.openagents/pylon/`

If you have existing data:

```bash
# Create new directory
mkdir -p ~/.openagents/pylon

# Copy existing data
cp -r ~/.pylon/* ~/.openagents/pylon/

# Verify pylon works
pylon doctor

# Remove old directory (optional)
rm -rf ~/.pylon
```

### From `~/.config/pylon/` to `~/.openagents/pylon/`

Some early versions used `~/.config/pylon/`:

```bash
# Move config
mv ~/.config/pylon/config.toml ~/.openagents/pylon/
mv ~/.config/pylon/identity.mnemonic ~/.openagents/pylon/

# Remove old directory
rm -rf ~/.config/pylon
```

## Backup Recommendations

### Essential Files

These files should be backed up:

```bash
# Agent identities (mnemonics)
~/.openagents/agents/*.toml

# Pylon identity
~/.openagents/pylon/identity.mnemonic

# Identity registry
~/.openagents/identities.json
```

### Optional Files

These can be recreated but may contain valuable data:

```bash
# Job history and earnings
~/.openagents/pylon/pylon.db

# Onyx notes
~/.openagents/onyx/*.md

# Autopilot sessions
~/.openagents/sessions/
```

### Backup Script

```bash
#!/bin/bash
BACKUP_DIR="$HOME/openagents-backup-$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"

# Essential
cp ~/.openagents/agents/*.toml "$BACKUP_DIR/" 2>/dev/null
cp ~/.openagents/pylon/identity.mnemonic "$BACKUP_DIR/" 2>/dev/null
cp ~/.openagents/identities.json "$BACKUP_DIR/" 2>/dev/null

# Optional
cp ~/.openagents/pylon/pylon.db "$BACKUP_DIR/" 2>/dev/null
cp -r ~/.openagents/onyx "$BACKUP_DIR/" 2>/dev/null

echo "Backup complete: $BACKUP_DIR"
```

## Troubleshooting

### "Could not determine home directory"

The `dirs` crate couldn't find the home directory. Check:
- `$HOME` environment variable is set
- Running as a valid user (not in containerized environment without proper setup)

### Permission Denied

Check directory and file permissions:

```bash
ls -la ~/.openagents/
ls -la ~/.openagents/pylon/
```

### Stale PID File

If pylon won't start due to a stale PID file:

```bash
# Check if process exists
cat ~/.openagents/pylon/pylon.pid
ps aux | grep pylon

# Remove stale file if process doesn't exist
rm ~/.openagents/pylon/pylon.pid
```

### Socket Already in Use

If the control socket is stale:

```bash
rm ~/.openagents/pylon/control.sock
pylon start
```
