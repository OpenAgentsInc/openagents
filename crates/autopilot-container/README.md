# Autopilot Container

HTTP service wrapper for Autopilot, designed to run in Cloudflare Containers.

## Overview

This crate provides an HTTP API that wraps the Claude Agent SDK, enabling cloud-based code analysis and task execution. It's the backend for the "paid tier" container mode in OpenAgents.

```
Browser → POST /api/start → Container clones repo → Runs Claude SDK → Streams results via WebSocket
```

## API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/ping` | GET | Health check |
| `/api/start` | POST | Start task with repo URL + prompt |
| `/api/status` | GET | Current task status |
| `/ws` | GET | WebSocket for streaming events |

## Quick Start

```bash
# Local development
WORKSPACE_DIR=/tmp/workspace cargo run -p autopilot-container

# Test
curl http://localhost:8080/ping  # → "ok"

# Start a task
curl -X POST http://localhost:8080/api/start \
  -H 'Content-Type: application/json' \
  -d '{"repo":"https://github.com/owner/repo","prompt":"Analyze this code"}'
```

## WebSocket Events

Connect to `/ws` to receive real-time events:

- `status` - Task status changes (cloning, running, completed, failed)
- `chunk` - Streaming text from Claude
- `tool_start` / `tool_done` - Tool execution events
- `tool_progress` - Long-running tool progress
- `usage` - Token counts and cost
- `done` / `error` - Task completion

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `WORKSPACE_DIR` | `/workspace` | Directory for cloned repos |
| `ANTHROPIC_API_KEY` | (required) | Claude API key |

## Documentation

See [docs/README.md](docs/README.md) for detailed API documentation and architecture.

## Related

- [autopilot-core](../autopilot-core/) - Core autopilot logic
- [claude-agent-sdk](../claude-agent-sdk/) - Rust SDK for Claude Code
- [web/worker](../web/worker/) - Cloudflare Worker that manages containers
