# Autopilot Daemon (autopilotd)

A supervisor daemon that spawns, monitors, and restarts autopilot worker processes.

## Problem

Claude Code leaks memory over time, and the autopilot process can crash from memory pressure. Without external supervision, crashed processes stay down until manually restarted.

## Solution

`autopilotd` provides:

- **Process supervision**: Spawns and monitors the autopilot worker
- **Memory monitoring**: Tracks system memory, kills memory hogs
- **Crash recovery**: Automatic restart with exponential backoff
- **Systemd integration**: Proper service management and logging

## Quick Start

```bash
# Development (foreground)
cargo daemon --workdir /path/to/project --project myproject

# Or directly
cargo run -p autopilot --bin autopilotd -- --workdir . --project openagents
```

## Commands

```bash
# Start daemon (default)
autopilotd start

# Get status
autopilotd status

# Restart worker (not daemon)
autopilotd restart-worker

# Stop worker
autopilotd stop-worker

# Stop daemon
autopilotd stop
```

## Configuration

### CLI Arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `--workdir` | `.` | Working directory for worker |
| `--project` | none | Project name for `--project` flag |
| `--model` | `sonnet` | Model to use |
| `--max-budget` | `300` | Max budget in USD |
| `--max-turns` | `99999` | Max conversation turns |
| `--config` | `~/.autopilot/daemon.toml` | Config file path |

### Config File

Create `~/.autopilot/daemon.toml`:

```toml
[worker_command]
type = "Cargo"
# type = "Binary"
# path = "/usr/local/bin/autopilot"

working_dir = "/home/user/code/openagents"
project = "openagents"
model = "sonnet"
max_budget = 300.0
max_turns = 99999

[memory]
min_available_bytes = 2147483648      # 2 GB
critical_threshold_bytes = 1073741824 # 1 GB
poll_interval_ms = 5000               # 5 seconds
node_kill_threshold_bytes = 524288000 # 500 MB

[restart]
initial_backoff_ms = 1000
max_backoff_ms = 300000               # 5 minutes
backoff_multiplier = 2.0
success_threshold_ms = 60000          # 1 minute
max_consecutive_restarts = 10
```

### Environment File

Create `~/.autopilot/env`:

```bash
ANTHROPIC_API_KEY=your-key-here
RUST_LOG=autopilot=info
```

## Systemd Setup

### Install Service

```bash
mkdir -p ~/.config/systemd/user
cp crates/autopilot/systemd/autopilotd.service ~/.config/systemd/user/
systemctl --user daemon-reload
```

### Enable and Start

```bash
systemctl --user enable autopilotd
systemctl --user start autopilotd
```

### Monitor

```bash
systemctl --user status autopilotd
journalctl --user -u autopilotd -f
```

## Memory Management

### Strategy

1. **Poll every 5 seconds**: Check available system memory
2. **On Low (<2GB)**: Kill node processes using >500MB
3. **On Critical (<1GB)**: Kill hogs, then restart worker if still critical

### Why Node Processes?

Claude Code runs on Node.js. Long sessions accumulate orphaned Node processes that consume memory. The daemon aggressively kills these.

## Crash Recovery

### Backoff Strategy

- Initial: 1 second
- Multiplier: 2x per failure
- Maximum: 5 minutes
- Reset: After 60 seconds of successful running

Example: 1s -> 2s -> 4s -> 8s -> 16s -> 32s -> 64s -> 128s -> 256s -> 300s (capped)

### Max Restarts

After 10 consecutive failures, the daemon enters "failed" state and stops restarting. Manual intervention required.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           SYSTEMD                                   │
│            (manages autopilotd, journald logging)                   │
└─────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       AUTOPILOTD                                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │
│  │Memory Monitor│  │Worker Super-│  │Control      │                 │
│  │(sysinfo)    │  │visor        │  │Socket       │                 │
│  └─────────────┘  └─────────────┘  └─────────────┘                 │
└─────────────────────────────────────────────────────────────────────┘
                                │ spawn/monitor
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│              AUTOPILOT WORKER (Child Process)                       │
│              cargo autopilot run --full-auto ...                    │
└─────────────────────────────────────────────────────────────────────┘
```

## Control Socket

The daemon listens on `~/.autopilot/autopilotd.sock` for control commands.

### Protocol

JSON request/response over Unix socket.

Request:
```json
{"type": "Status"}
{"type": "RestartWorker"}
{"type": "StopWorker"}
{"type": "StartWorker"}
{"type": "Shutdown"}
```

Response:
```json
{
  "success": true,
  "message": "Worker is running",
  "data": {
    "worker_status": "running",
    "worker_pid": 12345,
    "uptime_seconds": 3600,
    "total_restarts": 2,
    "consecutive_failures": 0,
    "memory_available_bytes": 4294967296,
    "memory_total_bytes": 17179869184
  }
}
```

## Troubleshooting

### Daemon won't start

Check for stale PID file:
```bash
rm ~/.autopilot/autopilotd.pid
```

Check for stale socket:
```bash
rm ~/.autopilot/autopilotd.sock
```

### Worker keeps crashing

Check memory:
```bash
autopilotd status
free -h
```

Check logs:
```bash
journalctl --user -u autopilotd --since "1 hour ago"
```

### Memory not being freed

Kill all Node processes manually:
```bash
pkill -f node
```

Then restart worker:
```bash
autopilotd restart-worker
```
