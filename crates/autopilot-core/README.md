# Autopilot Core

Core logic for Autopilot - the autonomous engine that powers AI coding tasks.

## Overview

Autopilot Core runs Claude SDK queries against your codebase to analyze code, fix bugs, implement features, and more. It supports two execution modes:

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
autopilot run "Fix the failing tests in src/auth"
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
- **Auth**: Checks Claude/Codex/OpenCode authentication status
- **Repository**: Validates git repo access in current folder
- **Projects**: Detects issue tracking (GitHub Issues, Linear, etc.)
- **Inference**: Claude/Codex availability and selected model
- **Usage**: Current credit balance and limits

Configuration is stored in `~/.openagents/folders/<path-hash>/` (not committed to git).

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Autopilot                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐  │
│  │  Preflight  │───▶│   Runner    │───▶│   Claude SDK        │  │
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
autopilot run "Implement user authentication"

# Force a backend (auto/claude/codex/local-llm/local-tools)
autopilot run "Implement user authentication" --backend codex

# Check status
autopilot status

# Issue workflows
autopilot issue list
autopilot issue claim <id>
```

## Configuration

### Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Claude API key (required for cloud mode) |
| `OPENAGENTS_CONFIG` | Custom config directory path |
| `AUTOPILOT_BACKEND` | Override backend selection for CLI runs |

### Config File

Located at `~/.openagents/config.toml`:

```toml
[inference]
provider = "claude"  # or "codex"
model = "claude-sonnet-4-5-20250929"

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
- [claude-agent-sdk](../claude-agent-sdk/) - Rust SDK for Claude Code CLI

## Documentation

- [DAEMON.md](../../docs/autopilot/DAEMON.md) - Daemon architecture and management
- [PROJECT-SPEC.md](../../docs/autopilot/PROJECT-SPEC.md) - Full product specification
