# Configuration

This document describes Pylon's configuration file format and options.

## Configuration File Location

The main configuration file is located at:

```
~/.config/pylon/config.toml
```

On first run (`pylon init`), a default configuration is created.

## File Format

Pylon uses TOML format for configuration.

```toml
# ~/.config/pylon/config.toml

# Nostr relays for provider mode
relays = [
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
]

# Data directory (optional, defaults to ~/.config/pylon)
# data_dir = "/custom/path"
```

## Configuration Options

### relays

**Type**: Array of strings
**Default**: `["wss://relay.damus.io"]`
**Required**: No

List of Nostr relay WebSocket URLs to connect to.

```toml
relays = [
    "wss://relay.damus.io",
    "wss://nos.lol",
]
```

**Considerations**:
- More relays = wider reach but more bandwidth
- Use reliable, well-known relays
- Order matters: first relay is primary

### data_dir

**Type**: String (path)
**Default**: `~/.config/pylon`
**Required**: No

Directory for Pylon data files (identity, etc.).

```toml
data_dir = "/home/user/.pylon-data"
```

## Complete Example

```toml
# Pylon Configuration
# ~/.config/pylon/config.toml

# Nostr Relays
# Connect to these relays for NIP-90 job requests
relays = [
    "wss://relay.damus.io",      # Primary relay
    "wss://nos.lol",             # Backup
    "wss://relay.nostr.band",    # Additional coverage
]

# Data directory (uncomment to customize)
# data_dir = "/custom/path"
```

## Related Files

### Identity File

```
~/.config/pylon/identity.mnemonic
```

Contains the 12-word BIP-39 mnemonic for the provider identity.

**Format**: Plain text, one line, space-separated words

```
word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12
```

**Security**:
- Keep this file private (chmod 600)
- Back it up securely
- Losing this loses your provider identity

### Runtime Files

| File | Location | Purpose |
|------|----------|---------|
| PID file | `~/.pylon/pylon.pid` | Process tracking |
| Control socket | `~/.pylon/control.sock` | IPC |
| Database | `~/.pylon/pylon.db` | Persistence |

## Agent Configuration

Agent configurations are stored separately in the agent registry:

```
~/.config/openagents/agents/<name>/config.json
```

See [Host Mode](./HOST_MODE.md) for agent configuration details.

## Environment Variables

Environment variables can override or supplement configuration:

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Logging level |
| `PYLON_AGENT_RUNNER` | Path to agent-runner binary |

### RUST_LOG

Controls logging verbosity:

```bash
# Minimal
RUST_LOG=warn pylon start -f

# Normal
RUST_LOG=info pylon start -f

# Debug
RUST_LOG=debug pylon start -f

# Trace (very verbose)
RUST_LOG=trace pylon start -f

# Module-specific
RUST_LOG=pylon=debug,compute=trace pylon start -f
```

### PYLON_AGENT_RUNNER

Override the agent-runner binary location:

```bash
PYLON_AGENT_RUNNER=/path/to/agent-runner pylon start
```

## Default Configuration

When `pylon init` runs, it creates:

```toml
# Default config.toml

relays = [
    "wss://relay.damus.io",
]
```

## Configuration Loading

Configuration is loaded in this order:

1. Built-in defaults
2. Configuration file (`~/.config/pylon/config.toml`)
3. Command-line arguments (e.g., `--config`)

## Validation

Pylon validates configuration on startup:

| Check | Error |
|-------|-------|
| Config file parse error | "Failed to parse config: ..." |
| No relays configured | Warning only |
| Invalid relay URL | "Invalid relay URL: ..." |
| Missing identity | "No identity found. Run 'pylon init' first." |

## Reload

Configuration is loaded once at startup. To apply changes:

```bash
pylon stop
pylon start
```

Future versions may support hot reload via control socket.

## Example Configurations

### Minimal (Default)

```toml
relays = ["wss://relay.damus.io"]
```

### Multiple Relays

```toml
relays = [
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band",
    "wss://nostr.wine",
]
```

### Custom Data Directory

```toml
relays = ["wss://relay.damus.io"]
data_dir = "/data/pylon"
```

## Troubleshooting

### Config Not Found

```
Error: Config file not found
```

Solution: Run `pylon init` first.

### Parse Error

```
Error: Failed to parse config: ...
```

Solution: Check TOML syntax. Common issues:
- Missing quotes around strings
- Trailing commas
- Invalid escape sequences

### Invalid Relay

```
Warning: Invalid relay URL: not-a-url
```

Solution: Use proper WebSocket URLs (`wss://...` or `ws://...`).

## Future Configuration Options

Planned additions (not yet implemented):

```toml
# Provider settings
[provider]
max_concurrent_jobs = 4
default_model = "llama3.2"

# Backend configuration
[backends.ollama]
url = "http://localhost:11434"
enabled = true

[backends.llamacpp]
url = "http://localhost:8080"
enabled = false

# Host settings
[host]
auto_start_agents = true
agent_log_dir = "~/.pylon/agent-logs"

# Pricing (for provider mode)
[pricing]
base_rate_msats_per_token = 1
model_multipliers = { "gpt4" = 10.0, "llama3.2" = 1.0 }
```
