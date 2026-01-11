# Autopilot Core

Core logic for Autopilot - the autonomous engine that powers AI coding tasks.

## Overview

Autopilot Core runs Codex/Codex SDK queries (plus local LLM/tool fallbacks) against your codebase to analyze code, fix bugs, implement features, and more. It supports two execution modes:

| Mode | Command | Cost | Execution |
|------|---------|------|-----------|
| **Tunnel** (free) | `openagents connect` | Free | Your machine |
| **Container** (paid) | Click "Start" in browser | Credits | Cloudflare edge |

## Getting Started

### Local Mode (Tunnel)

Run Autopilot locally with your own compute:

```bash
# Connect your local machine to the browser UI
openagents connect

# Or run directly in terminal
cargo autopilot run "Fix the failing tests in src/auth"
# (or `autopilot run ...` if the binary is on your PATH)
# Note: `cargo autopilot` with no args launches the GPU desktop UI.
```

### Cloud Mode (Container)

Use cloud compute via the web interface:
1. Log in at https://openagents.com
2. Select a repository
3. Click "Start" and enter your prompt
4. Watch real-time streaming results

## Preflight Checks

Before running, Autopilot verifies:

- **Config**: Reads `~/.openagents/` for settings
- **Auth**: Checks Codex/Codex/OpenCode authentication status
- **Repository**: Validates git repo access in current folder
- **Projects**: Detects issue tracking (GitHub Issues, Linear, etc.)
- **Inference**: Codex/Codex availability and selected model
- **Usage**: Current credit balance and limits

Configuration is stored in `~/.openagents/folders/<path-hash>/` (not committed to git).

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Autopilot                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐  │
│  │  Preflight  │───▶│   Runner    │───▶│   Codex SDK        │  │
│  │   Checks    │    │   Loop      │    │   Query             │  │
│  └─────────────┘    └─────────────┘    └─────────────────────┘  │
│         │                  │                      │              │
│         ▼                  ▼                      ▼              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐  │
│  │   Config    │    │   Tools     │    │   Streaming         │  │
│  │   Store     │    │ (Read,Edit) │    │   Results           │  │
│  └─────────────┘    └─────────────┘    └─────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## CLI Commands

```bash
# Run Autopilot with a prompt
cargo autopilot run "Implement user authentication"
# (or `autopilot run ...` if the binary is on your PATH)

# Force a backend (auto/codex/codex/local-llm/local-tools)
cargo autopilot run "Implement user authentication" --backend codex

# Check status
cargo autopilot status

# Issue workflows
cargo autopilot issue list
cargo autopilot issue claim <id>
```

## Configuration

### Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | Codex API key (required for cloud mode) |
| `OPENAGENTS_CONFIG` | Custom config directory path |
| `AUTOPILOT_BACKEND` | Override backend selection for CLI runs |

### Config File

Located at `~/.openagents/config.toml`:

```toml
[inference]
provider = "codex"  # or "codex"
model = "codex-sonnet-4-5-20250929"

[limits]
max_turns = 50
max_budget_usd = 10.0
```

CLI runs can override the backend using `--backend` or `AUTOPILOT_BACKEND`.

## Related Crates

- [autopilot-container](../autopilot-container/) - HTTP service wrapper for Cloudflare Containers
- [autopilot](../autopilot/) - GPU-accelerated desktop UI
- [autopilot-service](../autopilot-service/) - Background service and daemon
- [autopilot-shell](../autopilot-shell/) - Interactive shell interface
- [codex-agent-sdk](../codex-agent-sdk/) - Rust SDK for Codex CLI

## Documentation

- [DAEMON.md](../../docs/autopilot/DAEMON.md) - Daemon architecture and management
- [PROJECT-SPEC.md](../../docs/autopilot/PROJECT-SPEC.md) - Full product specification
