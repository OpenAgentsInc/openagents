# Daemon Infrastructure

This document describes Pylon's self-daemonizing infrastructure in detail.

## Overview

Pylon implements Unix-style daemonization, allowing it to:
- Run as a background process
- Survive terminal disconnection
- Be controlled via a Unix socket
- Manage its own lifecycle

## File Locations

| File | Purpose | Created By |
|------|---------|------------|
| `~/.openagents/pylon/pylon.pid` | Process ID file | `daemonize()` or `run_daemon()` |
| `~/.openagents/pylon/control.sock` | Unix domain socket for IPC | `run_daemon()` |
| `~/.openagents/pylon/pylon.db` | SQLite database | `run_daemon()` |

The `~/.openagents/pylon/` directory is created automatically if it doesn't exist.

## PID File Management

### Structure

The PID file contains a single line with the process ID:

```
12345
```

### Operations

```rust
// PidFile API (src/daemon/pid.rs)

pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    /// Create a new PidFile handle
    pub fn new(path: PathBuf) -> Self;

    /// Write current process PID to file
    pub fn write(&self) -> io::Result<()>;

    /// Read PID from file
    pub fn read(&self) -> io::Result<u32>;

    /// Check if file exists
    pub fn exists(&self) -> bool;

    /// Check if the process in the PID file is running
    pub fn is_running(&self) -> bool;

    /// Remove the PID file
    pub fn remove(&self) -> io::Result<()>;

    /// Send SIGTERM to the process
    pub fn terminate(&self) -> anyhow::Result<()>;

    /// Send SIGKILL to the process
    pub fn kill(&self) -> anyhow::Result<()>;
}
```

### Stale PID Detection

The `is_running()` method uses `kill(pid, 0)` to check if the process exists:

```rust
pub fn is_running(&self) -> bool {
    if let Ok(pid) = self.read() {
        let pid = Pid::from_raw(pid as i32);
        signal::kill(pid, None).is_ok()
    } else {
        false
    }
}
```

## Daemonization Process

### Double Fork Pattern

Pylon uses the classic Unix double-fork pattern to fully detach from the terminal:

```rust
// src/daemon/process.rs

pub fn daemonize() -> anyhow::Result<bool> {
    // First fork - creates child process
    match unsafe { unistd::fork() }? {
        ForkResult::Parent { .. } => {
            // Parent returns false (not the daemon)
            return Ok(false);
        }
        ForkResult::Child => {
            // Child continues
        }
    }

    // Create new session (detach from terminal)
    unistd::setsid()?;

    // Second fork - prevents session leader from
    // accidentally acquiring a controlling terminal
    match unsafe { unistd::fork() }? {
        ForkResult::Parent { .. } => {
            // Intermediate process exits
            std::process::exit(0);
        }
        ForkResult::Child => {
            // This is the actual daemon
        }
    }

    // Change to root directory
    unistd::chdir("/")?;

    // Redirect standard file descriptors to /dev/null
    redirect_stdio()?;

    // Write PID file
    let pid_file = PidFile::new(pid_path()?);
    pid_file.write()?;

    // Return true (we are the daemon)
    Ok(true)
}
```

### Why Double Fork?

1. **First fork**: Creates a child that's not a process group leader
2. **setsid()**: Creates a new session, makes child the session leader
3. **Second fork**: Creates a process that's not a session leader, preventing it from ever acquiring a controlling terminal

### Standard I/O Redirection

After daemonizing, stdin/stdout/stderr are redirected to `/dev/null`:

```rust
fn redirect_stdio() -> anyhow::Result<()> {
    let dev_null = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")?;

    let fd = dev_null.as_raw_fd();

    // Redirect stdin, stdout, stderr to /dev/null
    unistd::dup2(fd, 0)?; // stdin
    unistd::dup2(fd, 1)?; // stdout
    unistd::dup2(fd, 2)?; // stderr

    Ok(())
}
```

## Control Socket

### Protocol

The control socket uses a simple JSON-based request/response protocol:

```
Client                           Server (Daemon)
   │                                  │
   │──── Connect ────────────────────▶│
   │                                  │
   │──── JSON Command ───────────────▶│
   │                                  │
   │◀─── JSON Response ───────────────│
   │                                  │
   │──── Close ──────────────────────▶│
```

### Commands

```rust
// src/daemon/control.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonCommand {
    /// Check if daemon is responsive
    Ping,

    /// Get daemon status
    Status,

    /// Request graceful shutdown
    Shutdown,
}
```

### Responses

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    /// Response to Ping
    Pong,

    /// Generic success
    Ok,

    /// Error with message
    Error(String),

    /// Detailed status
    Status {
        running: bool,
        uptime_secs: u64,
        provider_active: bool,
        host_active: bool,
        jobs_completed: u64,
        earnings_msats: u64,
    },
}
```

### Socket Implementation

**Server Side (Daemon):**

```rust
pub struct ControlSocket {
    listener: UnixListener,
    path: PathBuf,
}

impl ControlSocket {
    /// Create and bind the control socket
    pub fn new(path: PathBuf) -> io::Result<Self>;

    /// Non-blocking accept (returns None if no pending connection)
    pub fn try_accept(&self) -> Option<ControlConnection>;
}

pub struct ControlConnection {
    stream: UnixStream,
}

impl ControlConnection {
    /// Read a command from the connection
    pub fn read_command(&mut self) -> io::Result<DaemonCommand>;

    /// Write a response to the connection
    pub fn write_response(&mut self, response: &DaemonResponse) -> io::Result<()>;
}
```

**Client Side:**

```rust
pub struct ControlClient {
    path: PathBuf,
}

impl ControlClient {
    /// Create a new client for the socket at path
    pub fn new(path: PathBuf) -> Self;

    /// Send a command and receive response
    pub fn send(&self, cmd: DaemonCommand) -> io::Result<DaemonResponse>;
}
```

### Socket Lifecycle

1. **Creation**: Socket created when daemon starts
2. **Cleanup**: Old socket file removed before creating new one
3. **Permissions**: Inherits umask (typically user-only access)
4. **Removal**: Socket file removed on daemon shutdown

## Signal Handling

### Supported Signals

| Signal | Action |
|--------|--------|
| SIGTERM | Graceful shutdown |
| SIGINT (Ctrl+C) | Graceful shutdown (foreground mode) |
| SIGKILL | Immediate termination (cannot be caught) |

### Signal Integration

In the main event loop:

```rust
tokio::select! {
    _ = tokio::signal::ctrl_c() => {
        tracing::info!("Received SIGINT, shutting down...");
        break;
    }
    _ = tokio::time::sleep(Duration::from_millis(100)) => {
        // Continue processing
    }
}
```

## Daemon Checking

### Is Daemon Running?

```rust
// src/daemon/process.rs

pub fn is_daemon_running() -> bool {
    let pid_file = match pid_path() {
        Ok(path) => PidFile::new(path),
        Err(_) => return false,
    };
    pid_file.is_running()
}
```

This function:
1. Checks if PID file exists
2. Reads the PID
3. Sends signal 0 to check if process exists
4. Returns true only if all checks pass

### Starting When Already Running

```rust
// In start.rs
if is_daemon_running() {
    println!("Pylon daemon is already running.");
    println!("Use 'pylon stop' to stop it first.");
    return Ok(());
}
```

## Stopping the Daemon

### Graceful Shutdown

```
pylon stop
    │
    ▼
┌─────────────────────────────────────────┐
│ Try control socket                       │
│   └─ Send Shutdown command               │
│   └─ Wait for Ok response                │
└─────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────┐
│ Send SIGTERM                             │
└─────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────┐
│ Wait for process to exit (with timeout) │
│   └─ Check is_running() every 100ms     │
│   └─ Default timeout: 10 seconds        │
└─────────────────────────────────────────┘
    │
    ├── Process exited ──▶ Success
    │
    ▼
┌─────────────────────────────────────────┐
│ Send SIGKILL (force)                     │
└─────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────┐
│ Remove PID file                          │
└─────────────────────────────────────────┘
```

### Force Stop

```bash
pylon stop --force
```

Skips graceful shutdown, immediately sends SIGKILL.

## Error Handling

### PID File Errors

| Error | Cause | Resolution |
|-------|-------|------------|
| Can't create directory | Permission denied | Check ~/.openagents/pylon permissions |
| Can't write PID file | Disk full or permissions | Free space or fix permissions |
| Stale PID file | Previous crash | Remove manually or use `pylon stop --force` |

### Socket Errors

| Error | Cause | Resolution |
|-------|-------|------------|
| Address already in use | Old socket file exists | Remove `~/.openagents/pylon/control.sock` |
| Permission denied | Wrong ownership | Fix permissions on ~/.openagents/pylon |
| Connection refused | Daemon not running | Start daemon first |

## Best Practices

### Monitoring

```bash
# Check if daemon is running
pylon status

# View daemon process
ps aux | grep pylon

# Watch logs (foreground mode only)
pylon start -f 2>&1 | tee pylon.log
```

### Recovery

```bash
# If daemon is unresponsive
pylon stop --force

# Clean up stale files
rm ~/.openagents/pylon/pylon.pid ~/.openagents/pylon/control.sock

# Restart
pylon start
```

### Logging

When running in foreground mode, logs go to stderr:

```bash
# Run with debug logging
RUST_LOG=debug pylon start -f

# Run with trace logging (very verbose)
RUST_LOG=trace pylon start -f
```

## Implementation Files

| File | Purpose |
|------|---------|
| `src/daemon/mod.rs` | Module exports and path helpers |
| `src/daemon/pid.rs` | PID file management |
| `src/daemon/process.rs` | Fork/daemonization |
| `src/daemon/control.rs` | Unix socket IPC |
