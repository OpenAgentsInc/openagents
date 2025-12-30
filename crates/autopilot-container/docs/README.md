# Autopilot Container Service

HTTP API wrapper for Autopilot, designed to run in Cloudflare Containers. Provides cloud-based code analysis and task execution as an alternative to the local tunnel mode.

## Architecture

```
┌─────────────┐     POST /api/start      ┌───────────────────────────────────┐
│   Browser   │ ─────────────────────────▶│   Autopilot Container Service     │
│   (Client)  │                           │                                   │
│             │ ◀──── WebSocket ──────────│   ┌─────────────────────────────┐ │
│             │      (streaming events)   │   │  1. Clone repo              │ │
└─────────────┘                           │   │  2. Run Claude SDK query    │ │
                                          │   │  3. Stream results via WS   │ │
                                          │   │  4. Cleanup                 │ │
                                          │   └─────────────────────────────┘ │
                                          └───────────────────────────────────┘
```

## Compute Tiers

| Mode | Command | Cost | Where |
|------|---------|------|-------|
| Tunnel (free) | `openagents connect` | Free | Your machine |
| Container (paid) | Click "Start" in browser | Credits | Cloudflare edge |

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/ping` | GET | Health check, returns "ok" |
| `/api/start` | POST | Start a new task |
| `/api/status` | GET | Get current task status |
| `/ws` | GET | WebSocket for streaming events |

### POST /api/start

Start a new autopilot task.

**Request:**
```json
{
  "repo": "https://github.com/owner/repo",
  "prompt": "Analyze this codebase and suggest improvements"
}
```

**Response:**
```json
{
  "task_id": "uuid-here",
  "status": "cloning"
}
```

### GET /api/status

Get the current task state.

**Response:**
```json
{
  "task": {
    "task_id": "uuid-here",
    "repo": "https://github.com/owner/repo",
    "prompt": "...",
    "status": "running",
    "started_at": "2025-12-30T23:00:00Z",
    "working_dir": "/workspace/repo_uuid"
  }
}
```

**Status values:** `cloning`, `running`, `completed`, `failed`

### GET /ws

WebSocket endpoint for real-time event streaming.

## WebSocket Events

All events are JSON with a `type` field:

### status
Task status change.
```json
{"type": "status", "task_id": "...", "status": "running"}
```

### chunk
Text chunk from Claude (streaming response).
```json
{"type": "chunk", "task_id": "...", "text": "I'll analyze..."}
```

### tool_start
Tool execution started.
```json
{
  "type": "tool_start",
  "task_id": "...",
  "tool_name": "Read",
  "tool_id": "toolu_...",
  "params": {"file_path": "/workspace/..."}
}
```

### tool_done
Tool execution completed.
```json
{
  "type": "tool_done",
  "task_id": "...",
  "tool_id": "toolu_...",
  "output": "file contents...",
  "is_error": false
}
```

### tool_progress
Long-running tool progress update.
```json
{
  "type": "tool_progress",
  "task_id": "...",
  "tool_id": "toolu_...",
  "elapsed_secs": 5.2
}
```

### usage
Token usage and cost statistics.
```json
{
  "type": "usage",
  "task_id": "...",
  "input_tokens": 1234,
  "output_tokens": 567,
  "total_cost_usd": 0.043
}
```

### done
Task completed successfully.
```json
{"type": "done", "task_id": "...", "summary": "Analysis complete..."}
```

### error
Task failed.
```json
{"type": "error", "task_id": "...", "error": "Failed to clone repo"}
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WORKSPACE_DIR` | `/workspace` | Directory for cloned repos |
| `ANTHROPIC_API_KEY` | (required) | Claude API key |

### Claude SDK Options

The service uses these defaults:
- Model: `claude-sonnet-4-5-20250929`
- Max turns: 50
- Max budget: $10 USD
- Partial messages: enabled

## Local Development

```bash
# Create workspace directory
mkdir -p /tmp/autopilot-workspace

# Run with local workspace
WORKSPACE_DIR=/tmp/autopilot-workspace cargo run -p autopilot-container

# Test health
curl http://localhost:8080/ping

# Start a task
curl -X POST http://localhost:8080/api/start \
  -H 'Content-Type: application/json' \
  -d '{"repo":"https://github.com/octocat/Hello-World","prompt":"List files"}'

# Connect to WebSocket (using websocat)
websocat ws://localhost:8080/ws
```

## Docker

The service is designed to run in Cloudflare Containers:

```dockerfile
FROM rust:alpine AS builder
WORKDIR /app
COPY . .
RUN apk add musl-dev git openssl-dev pkgconfig
RUN cargo build --release -p autopilot-container

FROM alpine:latest
RUN apk add --no-cache git openssh-client ca-certificates
COPY --from=builder /app/target/release/autopilot-container /usr/local/bin/autopilot
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/autopilot"]
```

## Integration with Web Worker

The container is managed by the `AutopilotContainer` Durable Object in the web worker:

1. User calls `POST /api/container/start` on the worker
2. Worker creates/gets DO instance keyed by user_id
3. DO starts container if not running
4. DO health-checks container via `/ping`
5. DO forwards requests to container
6. WebSocket events stream back to browser

See [web/docs/README.md](../../web/docs/README.md) for worker API documentation.

## Security

- Repos are cloned with `--depth 1` (shallow clone)
- Working directories are cleaned up after task completion
- Each task gets a unique directory (repo_taskid)
- Container runs with minimal privileges

## Related Documentation

- [Autopilot README](../../autopilot/README.md) - Core autopilot logic
- [Claude Agent SDK](../../claude-agent-sdk/README.md) - SDK for running Claude queries
- [Web Docs](../../web/docs/README.md) - Worker API and container routes
