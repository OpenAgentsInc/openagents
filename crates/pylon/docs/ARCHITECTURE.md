# System Architecture

This document describes the internal architecture of Pylon.

## Overview

Pylon is structured as a self-daemonizing binary with two operating modes:

```
┌──────────────────────────────────────────────────────────────────────────┐
│                              PYLON BINARY                                │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌────────────────┐  ┌─────────────────┐  ┌────────────────────────────┐ │
│  │   CLI Layer    │  │  Daemon Layer   │  │     Persistence Layer      │ │
│  │                │  │                 │  │                            │ │
│  │  init          │  │  PID File       │  │  SQLite Database           │ │
│  │  start         │  │  Control Socket │  │    - jobs                  │ │
│  │  stop          │  │  Signal Handler │  │    - earnings              │ │
│  │  status        │  │  Event Loop     │  │    - agents                │ │
│  │  doctor        │  │                 │  │    - tick_history          │ │
│  │  agent         │  │                 │  │                            │ │
│  │  earnings      │  │                 │  │                            │ │
│  └────────────────┘  └─────────────────┘  └────────────────────────────┘ │
│                                                                          │
│  ┌───────────────────────────────┐  ┌──────────────────────────────────┐ │
│  │         HOST MODE             │  │        PROVIDER MODE             │ │
│  │                               │  │                                  │ │
│  │  AgentRunner                  │  │  PylonProvider                   │ │
│  │    └─ Subprocess Management   │  │    ├─ BackendRegistry            │ │
│  │    └─ Agent Registry          │  │    ├─ RelayService               │ │
│  │    └─ State Persistence       │  │    ├─ DvmService                 │ │
│  │                               │  │    └─ Event Broadcasting         │ │
│  └───────────────────────────────┘  └──────────────────────────────────┘ │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

## Component Details

### CLI Layer (`src/cli/`)

The CLI provides the user interface to Pylon:

| Module | Purpose |
|--------|---------|
| `mod.rs` | Command routing and execution |
| `init.rs` | Identity initialization |
| `start.rs` | Daemon startup with mode selection |
| `stop.rs` | Graceful daemon shutdown |
| `status.rs` | Status display with daemon querying |
| `doctor.rs` | Diagnostic checks |
| `agent.rs` | Agent management commands |
| `earnings.rs` | Earnings display |

### Daemon Layer (`src/daemon/`)

Manages the background process lifecycle:

| Module | Purpose |
|--------|---------|
| `mod.rs` | Path helpers and module exports |
| `pid.rs` | PID file management |
| `process.rs` | Unix fork/daemonization |
| `control.rs` | Unix socket IPC |

#### Daemonization Process

```
pylon start
    │
    ▼
┌─────────────────┐
│  Check if       │
│  already running│
│  (PID file)     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   First fork()  │──────────────────┐
└────────┬────────┘                  │
         │                           │
         ▼                           ▼
┌─────────────────┐          ┌──────────────┐
│   setsid()      │          │ Parent exits │
│   (new session) │          │ immediately  │
└────────┬────────┘          └──────────────┘
         │
         ▼
┌─────────────────┐
│  Second fork()  │──────────────────┐
└────────┬────────┘                  │
         │                           │
         ▼                           ▼
┌─────────────────┐          ┌──────────────┐
│ Write PID file  │          │ Session      │
│ Open control    │          │ leader exits │
│ socket          │          └──────────────┘
│ Redirect stdio  │
│ to /dev/null    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Main daemon   │
│   event loop    │
└─────────────────┘
```

#### Control Socket Protocol

The daemon accepts commands via Unix socket at `~/.pylon/control.sock`:

**Commands (JSON-encoded):**
```json
{"Ping": null}
{"Status": null}
{"Shutdown": null}
```

**Responses (JSON-encoded):**
```json
{"Pong": null}
{"Ok": null}
{"Error": "message"}
{"Status": {
    "running": true,
    "uptime_secs": 3600,
    "provider_active": true,
    "host_active": true,
    "jobs_completed": 42,
    "earnings_msats": 50000
}}
```

### Persistence Layer (`src/db/`)

SQLite database for durable storage:

| Module | Purpose |
|--------|---------|
| `mod.rs` | Database wrapper, migrations |
| `jobs.rs` | Job CRUD operations |
| `earnings.rs` | Earnings tracking |
| `agents.rs` | Agent state persistence |

See [Database Schema](./DATABASE.md) for full schema documentation.

### Host Mode (`src/host/`)

Manages sovereign agent subprocesses:

| Module | Purpose |
|--------|---------|
| `mod.rs` | Module exports |
| `runner.rs` | Subprocess spawning and management |

**Agent Lifecycle:**

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Spawning   │────▶│    Active    │────▶│   Dormant    │
│  (no funds)  │     │  (running)   │     │ (zero funds) │
└──────────────┘     └──────────────┘     └──────────────┘
                            │                    │
                            │                    │
                            ▼                    ▼
                     ┌──────────────┐     ┌──────────────┐
                     │  Low Balance │     │  (fund to    │
                     │  (< 7 days)  │     │   revive)    │
                     └──────────────┘     └──────────────┘
```

### Provider Mode (`src/provider.rs`)

Implements NIP-90 Data Vending Machine:

**Job Processing Flow:**

```
┌─────────────┐
│ Nostr Relay │
└──────┬──────┘
       │ kind:5xxx (job request)
       ▼
┌─────────────┐
│ DVM Service │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────────┐
│             Backend Registry            │
│                                         │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  │
│  │ Ollama  │  │ llamacpp│  │Apple FM │  │
│  └────┬────┘  └────┬────┘  └────┬────┘  │
│       │            │            │       │
│       └────────────┴────────────┘       │
│                    │                    │
└────────────────────┼────────────────────┘
                     │
                     ▼
              ┌─────────────┐
              │   Result    │
              └──────┬──────┘
                     │
                     ▼
┌─────────────────────────────────────────┐
│              Event Types                │
│                                         │
│  JobReceived → JobStarted → JobProgress │
│       │              │           │      │
│       ▼              ▼           ▼      │
│  JobCompleted ← ─ ─ ─ ─ ─ ─ ─ ─ ─      │
│       │                                 │
│       ▼                                 │
│  PaymentReceived                        │
└─────────────────────────────────────────┘
```

## Configuration

See [Configuration](./CONFIGURATION.md) for config file format.

## Data Flow

### Startup Sequence

```
1. Load configuration (~/.config/pylon/config.toml)
2. Load identity (~/.config/pylon/identity.mnemonic)
3. Check for existing daemon (PID file)
4. Fork to background (if not -f)
5. Write PID file
6. Open control socket
7. Open database
8. Load historical stats from database
9. Initialize provider (if mode includes provider)
   a. Detect backends
   b. Connect to relays
   c. Start DVM service
10. Initialize host (if mode includes host)
    a. Load agent registry
    b. Start active agent subprocesses
11. Enter main event loop
```

### Main Event Loop

```
loop {
    // 1. Check control socket for commands
    if control_socket.has_pending() {
        handle_command()
    }

    // 2. Process provider events
    if provider_events.has_pending() {
        match event {
            JobReceived => db.create_job()
            JobCompleted => {
                db.complete_job()
                db.record_earning()
            }
            JobFailed => db.fail_job()
            PaymentReceived => earnings += amount
        }
    }

    // 3. Check for shutdown signals
    select! {
        _ = ctrl_c => break
        _ = sleep(100ms) => continue
    }
}
```

### Shutdown Sequence

```
1. Receive shutdown signal (SIGTERM or control socket)
2. Stop accepting new jobs
3. Stop all agent subprocesses
4. Stop provider/DVM service
5. Disconnect from relays
6. Remove PID file
7. Exit
```

## Thread Model

Pylon uses Tokio for async I/O with a multi-threaded runtime:

- **Main Task**: Event loop, control socket handling
- **Provider Tasks**: Relay connections, job processing
- **Agent Subprocesses**: Separate OS processes (not threads)

## Security Considerations

### Identity Protection

- Mnemonic stored in plaintext at `~/.config/pylon/identity.mnemonic`
- Future: Password-protected encryption
- Recommendation: Set restrictive file permissions (`chmod 600`)

### Unix Socket

- Control socket at `~/.pylon/control.sock`
- Protected by filesystem permissions
- Only accessible to the user who started the daemon

### Agent Isolation

- Agents run as separate OS processes
- Each agent has its own identity and wallet
- Agent crashes don't affect the daemon

## Performance Characteristics

### Memory Usage

- Base daemon: ~20-50 MB
- Per-agent subprocess: ~50-100 MB (depends on model)
- Database: Minimal (SQLite with WAL mode)

### Disk Usage

- Database grows with job/earnings history
- Recommend periodic cleanup for long-running instances

### Network

- Maintains persistent WebSocket connections to relays
- Job results streamed back (not buffered)
