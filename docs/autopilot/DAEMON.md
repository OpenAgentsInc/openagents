# Autopilot Daemon (autopilotd)

The autopilot daemon is a supervisor process that manages autonomous autopilot workers. It handles process lifecycle, crash recovery, memory management, and ensures continuous operation.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        AUTOPILOT DAEMON                              │
│                                                                      │
│  ┌───────────────┐    ┌──────────────┐    ┌───────────────────────┐ │
│  │   Supervisor  │───▶│    Worker    │───▶│   Claude Code CLI     │ │
│  │               │    │ (autopilot)  │    │   (child process)     │ │
│  └───────┬───────┘    └──────────────┘    └───────────────────────┘ │
│          │                                                           │
│          │            ┌──────────────┐                              │
│          └───────────▶│   Memory     │                              │
│                       │   Monitor    │                              │
│                       └──────────────┘                              │
│                                                                      │
│  Control Socket: ~/.autopilot/autopilotd.sock                       │
│  PID File: ~/.autopilot/autopilotd.pid                              │
│  Known-Good Binary: ~/.autopilot/bin/autopilot                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Quick Start

### First-Time Setup

```bash
# 1. Build the autopilot binary
cargo build -p autopilot

# 2. Install the known-good binary
mkdir -p ~/.autopilot/bin
cp target/debug/autopilot ~/.autopilot/bin/autopilot
chmod +x ~/.autopilot/bin/autopilot

# 3. Start the daemon
./target/debug/autopilotd --workdir /path/to/project --project myproject
```

### Running the Daemon

```bash
# Development mode (from workspace root)
cargo run -p autopilot --bin autopilotd -- --workdir . --project openagents

# Using pre-built binary
./target/debug/autopilotd --workdir /path/to/project --project myproject

# Background mode
nohup ./target/debug/autopilotd --workdir . --project openagents &
```

### Daemon Commands

```bash
# Check status
autopilotd status

# Restart worker (without restarting daemon)
autopilotd restart-worker

# Stop worker only
autopilotd stop-worker

# Stop daemon entirely
autopilotd stop
```

## Known-Good Binary System

### The Problem

The daemon originally used `cargo run` to spawn workers. This was fragile because:

1. Worker could modify code during execution
2. If worker crashed after breaking the build, daemon couldn't restart it
3. All restart attempts would fail until someone manually fixed the code

### The Solution

The daemon now uses a **pre-built binary** at `~/.autopilot/bin/autopilot`:

- Binary is separate from working tree
- Code changes don't affect restart capability
- Workers can be restarted even if current code is broken

### How It Works

```
Worker Start Flow:

  Daemon
     │
     ▼
  Check: AUTOPILOT_WORKER_BINARY env var set?
     │
     ├─ Yes ──▶ Use that path
     │
     ▼
  Check: ~/.autopilot/bin/autopilot exists?
     │
     ├─ Yes ──▶ Use known-good binary
     │
     ├─ No ───▶ Fall back to cargo run
     │          (only for first-time setup)
     ▼
  Spawn worker process
```

### Installing/Updating the Known-Good Binary

```bash
# After successful build, update the known-good binary
cargo build -p autopilot && \
cp target/debug/autopilot ~/.autopilot/bin/autopilot

# Verify
~/.autopilot/bin/autopilot --version
```

**Important:** Only update the known-good binary when you're confident the build works. This is your safety net.

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `AUTOPILOT_WORKER_BINARY` | Path to worker binary | `~/.autopilot/bin/autopilot` |
| `AUTOPILOT_STALL_TIMEOUT_MS` | Stall detection timeout | 300000 (5 min) |
| `AUTOPILOT_RECOVERY_COOLDOWN_MS` | Cooldown after max failures | 600000 (10 min) |

```bash
# Example: Use a different binary
AUTOPILOT_WORKER_BINARY=/path/to/custom/autopilot autopilotd start

# Example: Shorter stall timeout for testing
AUTOPILOT_STALL_TIMEOUT_MS=60000 autopilotd start
```

## Configuration

### Config File

The daemon reads from `~/.autopilot/daemon.toml`:

```toml
# Worker command - use Binary for reliability
[worker_command]
type = "Binary"
path = "/home/user/.autopilot/bin/autopilot"

# Working directory
working_dir = "/home/user/code/myproject"

# Project name
project = "myproject"

# Model to use
model = "sonnet"

# Budget constraints
max_budget = 0.0    # 0 = no limit
max_turns = 99999

# Memory configuration
[memory]
min_available_bytes = 2147483648      # 2 GB
critical_threshold_bytes = 1073741824  # 1 GB
poll_interval_ms = 5000                # 5 seconds
node_kill_threshold_bytes = 524288000  # 500 MB

# Restart policy
[restart]
initial_backoff_ms = 1000
max_backoff_ms = 300000               # 5 minutes max
backoff_multiplier = 2.0
success_threshold_ms = 60000          # Reset backoff after 1 min success
max_consecutive_restarts = 10
stall_timeout_ms = 300000             # Kill if no log activity for 5 min
recovery_cooldown_ms = 600000         # Wait 10 min before retry after max failures
```

### CLI Arguments

```bash
autopilotd [OPTIONS]

Options:
  -c, --config <FILE>      Config file [default: ~/.autopilot/daemon.toml]
  -w, --workdir <PATH>     Working directory for worker
  -p, --project <NAME>     Project name
      --model <MODEL>      Model to use [default: sonnet]
      --max-budget <USD>   Maximum budget [default: 0]
      --max-turns <N>      Maximum turns [default: 99999]
```

## Process Management

### Restart Behavior

The daemon implements exponential backoff for crashes:

```
Attempt 1: 1 second delay
Attempt 2: 2 second delay
Attempt 3: 4 second delay
Attempt 4: 8 second delay
...
Attempt N: min(2^N, 300) seconds

After 10 consecutive failures: Enter "Failed" state
After 10 minute cooldown: Reset counter and retry
```

### Success Detection

If a worker runs for more than 60 seconds (configurable), backoff is reset. This prevents permanent backoff escalation after a single failure.

### Stall Detection

The daemon monitors log file modification times:

1. Finds latest `.rlog` file in `docs/logs/YYYYMMDD/`
2. If not modified for 5 minutes, worker is considered stalled
3. Daemon kills and restarts stalled worker

Grace period: First 2 minutes after worker start are exempt (allows for compilation time).

### Memory Management

The daemon monitors system memory:

| Memory Available | Action |
|------------------|--------|
| > 2 GB | Normal operation |
| 1-2 GB | Kill node processes > 500MB |
| < 1 GB | Force restart worker |

```bash
# Check current memory status
autopilotd status
# Shows: Memory: 4.2 GB / 16.0 GB available
```

## Logs and Monitoring

### Worker Logs

Worker sessions are logged to `.rlog` files:

```bash
# Find today's logs
ls docs/logs/$(date +%Y%m%d)/*.rlog

# Watch latest log
tail -f docs/logs/$(date +%Y%m%d)/*.rlog

# Find most recently modified log
ls -t docs/logs/**/*.rlog | head -1
```

### Daemon Logs

When running in foreground, daemon logs to stderr:

```
Starting autopilotd...
  Working dir: /home/user/code/project
  Project: myproject
  Model: sonnet
  Socket: /home/user/.autopilot/autopilotd.sock
Spawning worker...
Worker started with PID 12345
```

### Status Check

```bash
# Via control socket
autopilotd status

# Output:
# Autopilot Daemon Status
# =======================
# Worker status:  running
# Worker PID:     12345
# Uptime:         3600 seconds
# Total restarts: 2
# Failures:       0
# Memory:         4.2 GB / 16.0 GB available
```

## Troubleshooting

### Worker Won't Start

**Symptom:** Daemon says "Worker started" but immediately shows restart attempts.

**Check:**
```bash
# Try running worker directly
~/.autopilot/bin/autopilot run --full-auto "test"

# If that fails, rebuild the known-good binary
cargo build -p autopilot
cp target/debug/autopilot ~/.autopilot/bin/autopilot
```

### Build Failures Block Restarts

**Symptom:** Daemon keeps trying to restart but logs show compile errors.

**This is the main reason for the known-good binary system.** If you're seeing this:

```bash
# 1. Fix the compile errors manually
cargo build -p autopilot

# 2. Update the known-good binary
cp target/debug/autopilot ~/.autopilot/bin/autopilot

# 3. Restart the daemon
autopilotd restart-worker
```

### High Memory Usage

**Symptom:** System becomes slow, daemon kills worker frequently.

**Check:**
```bash
# See what's using memory
ps aux --sort=-%mem | head -10

# Daemon automatically kills node processes > 500MB
# To adjust threshold, set in config:
[memory]
node_kill_threshold_bytes = 1073741824  # 1 GB
```

### Worker Detected as Stalled

**Symptom:** Worker killed after 5 minutes even though it's working.

This happens when the worker is stuck in a long operation that doesn't write to logs (e.g., large builds, network timeouts).

**Solutions:**
1. Increase stall timeout:
   ```bash
   AUTOPILOT_STALL_TIMEOUT_MS=600000 autopilotd start  # 10 minutes
   ```

2. Or in config:
   ```toml
   [restart]
   stall_timeout_ms = 600000
   ```

### Daemon Doesn't Restart Worker

**Symptom:** Worker exits but daemon doesn't restart it.

**Check:**
```bash
autopilotd status
```

If status shows "Failed", the daemon hit max consecutive restarts and is in cooldown. Wait 10 minutes or:

```bash
# Force restart
autopilotd stop
autopilotd start --workdir /path --project myproject
```

## Best Practices

### 1. Always Have a Known-Good Binary

Before making significant code changes:
```bash
# Save current working state
cp ~/.autopilot/bin/autopilot ~/.autopilot/bin/autopilot.backup
```

### 2. Update Binary After Successful Builds

Add to your workflow:
```bash
# After cargo build succeeds
cargo build -p autopilot && cp target/debug/autopilot ~/.autopilot/bin/
```

### 3. Use Separate Worktrees for Manual Work

If you need to work on autopilot code while autopilot is running:
```bash
git worktree add ../openagents-manual main
cd ../openagents-manual
# Make changes here, autopilot continues in original dir
```

### 4. Monitor Daemon Health

Set up a simple health check:
```bash
# In crontab
*/5 * * * * pgrep autopilotd || /path/to/start-daemon.sh
```

## Related Documentation

- [Autopilot README](./README.md) - Worker functionality
- [APM Tracking](./VELOCITY-TRACKING.md) - Actions Per Minute metrics
- [Postmortem: Daemon Worker Failure](../logs/20251223/postmortem-daemon-worker-failure.md) - Incident that led to known-good binary system
