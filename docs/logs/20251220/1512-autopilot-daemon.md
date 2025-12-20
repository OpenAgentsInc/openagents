# Autopilot Daemon Implementation Plan

## Problem
Claude Code leaks memory over time, and the current autopilot has no external supervision. If it crashes from memory pressure, nothing restarts it.

## Solution
Create `autopilotd` - a supervisor daemon that spawns, monitors, and restarts the autopilot worker process.

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
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐ │
│  │Memory Monitor│  │Worker Super-│  │Control      │  │Health      │ │
│  │(sysinfo)    │  │visor        │  │Socket       │  │Check       │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
                                │ spawn/monitor
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│              AUTOPILOT WORKER (Child Process)                       │
│              cargo autopilot run --full-auto ...                    │
└─────────────────────────────────────────────────────────────────────┘
```

## Files to Create

### 1. `crates/autopilot/src/daemon/mod.rs`
Module exports for daemon components.

### 2. `crates/autopilot/src/daemon/config.rs`
```rust
pub struct DaemonConfig {
    pub worker_command: WorkerCommand,  // Cargo or Binary
    pub working_dir: PathBuf,
    pub project: Option<String>,
    pub memory: MemoryConfig,
    pub restart: RestartConfig,
    pub socket_path: PathBuf,           // ~/.autopilot/autopilotd.sock
    pub pid_file: PathBuf,              // ~/.autopilot/autopilotd.pid
}

pub struct MemoryConfig {
    pub min_available_bytes: u64,       // 2GB default
    pub critical_threshold_bytes: u64,  // 1GB - force restart
    pub poll_interval_ms: u64,          // 5000ms
    pub node_kill_threshold_bytes: u64, // 500MB
}

pub struct RestartConfig {
    pub initial_backoff_ms: u64,        // 1000
    pub max_backoff_ms: u64,            // 300000 (5 min)
    pub backoff_multiplier: f64,        // 2.0
    pub max_consecutive_restarts: u32,  // 10
}
```

### 3. `crates/autopilot/src/daemon/supervisor.rs`
- Spawns worker via `tokio::process::Command`
- Monitors worker exit status via `try_wait()`
- Handles crashes with exponential backoff
- Uses process groups to kill orphan Claude/Node processes

### 4. `crates/autopilot/src/daemon/memory.rs`
- Polls system memory via `sysinfo` crate
- On Low: kills node processes >500MB
- On Critical: forces worker restart

### 5. `crates/autopilot/src/daemon/control.rs`
- Unix socket server at `~/.autopilot/autopilotd.sock`
- Commands: status, restart, stop, start

### 6. `crates/autopilot/src/bin/autopilotd.rs`
CLI with subcommands:
- `autopilotd start` - run daemon (default)
- `autopilotd stop` - stop via control socket
- `autopilotd status` - get worker status
- `autopilotd restart-worker` - restart worker only

### 7. `crates/autopilot/systemd/autopilotd.service`
```ini
[Unit]
Description=Autopilot Supervisor Daemon
After=network.target

[Service]
Type=notify
ExecStart=/usr/local/bin/autopilotd --foreground
Restart=always
RestartSec=10
Environment=RUST_LOG=autopilot=info
EnvironmentFile=-/home/christopherdavid/.autopilot/env
WorkingDirectory=/home/christopherdavid/code/openagents
User=christopherdavid
MemoryMax=512M
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=default.target
```

## Files to Modify

### `crates/autopilot/Cargo.toml`
Add new binary and dependencies:
```toml
[[bin]]
name = "autopilotd"
path = "src/bin/autopilotd.rs"

[dependencies]
sd-notify = "0.4"  # systemd notification
```

## Implementation Steps

1. **Create daemon module structure**
   - Create `src/daemon/` directory with mod.rs, config.rs stubs
   - Add `[[bin]]` entry to Cargo.toml

2. **Implement configuration**
   - DaemonConfig with defaults
   - TOML config file loading from `~/.autopilot/daemon.toml`
   - Environment variable overrides

3. **Implement memory monitor**
   - Reuse logic from existing `check_memory()` and `check_and_kill_memory_hogs()`
   - Add critical threshold for forced restart

4. **Implement worker supervisor**
   - Spawn worker with `Command::new()`
   - Monitor via polling loop
   - Handle crashes with backoff
   - Use process groups for cleanup

5. **Implement control socket**
   - Unix socket server
   - JSON request/response protocol
   - status, restart, stop commands

6. **Create daemon binary**
   - Clap CLI with subcommands
   - Signal handlers (SIGTERM, SIGINT)
   - PID file management
   - systemd notify integration

7. **Create systemd service file**
   - User service at `~/.config/systemd/user/autopilotd.service`
   - Security hardening
   - Memory limits

8. **Add cargo alias**
   - `cargo daemon` -> runs autopilotd

## Key Design Decisions

- **Signals only**: No bidirectional channel needed. Daemon monitors process exit and memory.
- **Process groups**: Worker spawns in own group so we can kill all children.
- **Exponential backoff**: 1s -> 2s -> 4s -> ... -> 5min max, reset on 60s success.
- **Reactive memory**: Try killing hogs first, only restart worker if still critical.
- **Minimal daemon**: <50MB memory, just supervision logic.

## Usage After Implementation

```bash
# Development
cargo daemon start --project openagents --foreground

# Production (systemd)
systemctl --user enable autopilotd
systemctl --user start autopilotd

# Check status
autopilotd status
journalctl --user -u autopilotd -f

# Restart worker (not daemon)
autopilotd restart-worker
```

