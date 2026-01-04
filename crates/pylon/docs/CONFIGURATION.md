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

### claude

**Type**: Table
**Required**: No

Claude tunnel settings for local Claude Agent SDK sessions.

```toml
[claude]
enabled = true
model = "claude-sonnet-4-20250514"
autonomy = "supervised"           # full | supervised | restricted | read_only
approval_required_tools = ["Write", "Edit", "Bash"]
allowed_tools = []                # empty = allow requested tools
blocked_tools = []
max_cost_usd = 250000             # micro-USD ($0.25) or omit
cwd = "/path/to/repo"
executable_path = "/usr/local/bin/claude"
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

# Claude tunnel defaults
[claude]
enabled = true
model = "claude-sonnet-4-20250514"
autonomy = "supervised"
approval_required_tools = ["Write", "Edit", "Bash"]

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

### GPT-OSS Metal (macOS)

Enable the GPT-OSS Metal backend (local `model.bin`) for Pylon providers.

1) Build the Metal libraries from the GPT-OSS repo:

```bash
cd ~/code/gpt-oss/gpt_oss/metal
cmake -B build
cmake --build build --config Release
```

2) Create or download Metal-format weights:

```bash
python gpt_oss/metal/scripts/create-local-model.py -s <model_dir> -d <output>/model.bin
```

3) Export environment variables:

```bash
export GPT_OSS_METAL_DIR=~/code/gpt-oss/gpt_oss/metal/build
export GPT_OSS_METAL_MODEL_PATH=/path/to/model.bin
export GPT_OSS_METAL_MODEL_ID=gpt-oss-20b

# Optional tuning
export GPT_OSS_METAL_CONTEXT_LENGTH=8192
export GPT_OSS_METAL_MAX_BATCH_TOKENS=512
export GPT_OSS_METAL_TEMPERATURE=0.7
export GPT_OSS_METAL_MAX_TOKENS=256
export GPT_OSS_METAL_SEED=42
```

4) Run Pylon with the feature enabled:

```bash
cargo run -p pylon --features gpt-oss-metal -- start -f
```

### GPT-OSS GGUF (cross-platform CPU)

Enable the GPT-OSS GGUF backend (local `.gguf`) for Pylon providers.

1) Export environment variables:

```bash
export GPT_OSS_GGUF_PATH=/path/to/gpt-oss-20b-Q8_0.gguf
export GPT_OSS_GGUF_MODEL_ID=gpt-oss-20b

# Optional tuning
export GPT_OSS_GGUF_CONTEXT_LENGTH=8192
export GPT_OSS_GGUF_MAX_KV=8192
export GPT_OSS_GGUF_MAX_TOKENS=256
export GPT_OSS_GGUF_LAYER_LIMIT=24
export GPT_OSS_GGUF_MOE_FALLBACK=1
export GPT_OSS_GGUF_HARMONY=1
```

2) Run Pylon with the feature enabled:

```bash
cargo run -p pylon --features gpt-oss-gguf -- start -f
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
