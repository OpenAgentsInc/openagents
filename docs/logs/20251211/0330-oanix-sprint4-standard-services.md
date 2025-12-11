# OANIX Sprint 4: Standard Services

**Date:** 2025-12-11

---

## Summary

Implemented Sprint 4 of the OANIX roadmap: three standard services that provide the core abstractions for agent execution environments.

---

## New Services

### 1. TaskFs - Task Specification Service

**File:** `src/services/task_fs.rs` (~320 lines)

A composite FileService that exposes task data as files:

```
/task/
├── spec.json      # Task specification (read-only)
├── meta.json      # Metadata (read-only)
├── status         # Live status (computed on read)
└── result.json    # Execution result (read-write)
```

**Key types:**

```rust
pub struct TaskSpec {
    pub id: String,
    pub task_type: String,
    pub description: String,
    pub input: serde_json::Value,
}

pub struct TaskMeta {
    pub created_at: u64,
    pub tags: Vec<String>,
    pub timeout_secs: Option<u64>,
    pub version: u32,
}

pub enum TaskStatus {
    Pending,
    Running { started_at: u64 },
    Completed { finished_at: u64 },
    Failed { finished_at: u64, error: String },
}
```

**API:**

```rust
let task = TaskFs::new(spec, meta);

// Read spec via file interface
let content = read_file(&task, "/spec.json");

// Status management
task.set_running();
task.set_completed();
task.set_failed("Error message");
task.is_finished() // true if completed or failed
```

### 2. LogsFs - Structured Logging Service

**File:** `src/services/logs_fs.rs` (~400 lines)

Provides structured logging with both file and programmatic interfaces:

```
/logs/
├── stdout.log     # Standard output (append-only)
├── stderr.log     # Standard error (append-only)
└── events.jsonl   # Structured events (JSON Lines)
```

**Key types:**

```rust
pub enum LogLevel { Debug, Info, Warn, Error }

pub struct LogEvent {
    pub timestamp: u64,
    pub level: LogLevel,
    pub message: String,
    pub data: Option<serde_json::Value>,
}
```

**API:**

```rust
let logs = LogsFs::new();

// Programmatic logging
logs.write_stdout(b"Output text\n");
logs.write_stderr(b"Error text\n");
logs.info("Task started");
logs.log_event(LogEvent::with_data(LogLevel::Debug, "Config", json!({...})));

// File interface
let mut handle = logs.open("/stdout.log", OpenFlags::write_only())?;
handle.write(b"Output via file\n")?;

// Read logs
let stdout = logs.stdout();
let events = logs.events();
```

### 3. WorkspaceFs - Real Filesystem Wrapper

**File:** `src/services/workspace_fs.rs` (~380 lines)

Wraps a real host directory with path security (native only, not WASM):

```rust
// Wrap a project directory
let workspace = WorkspaceFs::new("/home/user/project")?;

// Read-only mode
let workspace_ro = WorkspaceFs::readonly("/home/user/project")?;
```

**Security features:**

- All paths canonicalized to prevent `..` escapes
- Symlinks that escape root are rejected
- Root directory must exist and be accessible
- Optional read-only mode

```rust
// These are rejected:
workspace.open("/../../../etc/passwd", flags);  // Error
workspace.open("/src/../../etc/passwd", flags); // Error
```

---

## Test Results

```
running 59 tests (unit)
...all pass...

running 12 tests (integration)
test test_taskfs_service ... ok
test test_logsfs_service ... ok
test test_complete_agent_environment ... ok
test test_task_failure_handling ... ok
...all pass...

test result: ok. 71 passed; 0 failed
```

---

## Integration Example: Complete Agent Environment

```rust
// Create services
let task = TaskFs::new(spec, meta);
let logs = LogsFs::new();
let workspace = CowFs::new(MapFs::builder()...build());
let tmp = MemFs::new();

// Build namespace
let ns = Namespace::builder()
    .mount("/task", task)
    .mount("/logs", logs)
    .mount("/workspace", workspace)
    .mount("/tmp", tmp)
    .build();

// Agent can now:
// - Read task from /task/spec.json
// - Update status via task.set_running()
// - Modify workspace files
// - Write logs to /logs/stdout.log
// - Store intermediate results in /tmp
// - Write final result to /task/result.json
```

---

## Files Created/Modified

- `src/services/task_fs.rs` - NEW (~320 lines)
- `src/services/logs_fs.rs` - NEW (~400 lines)
- `src/services/workspace_fs.rs` - NEW (~380 lines)
- `src/services/mod.rs` - Updated exports
- `src/lib.rs` - Updated re-exports
- `tests/integration.rs` - Added 4 new tests
- `docs/ROADMAP.md` - Updated status

---

## Dependencies Added

- `tempfile` (dev dependency) - For WorkspaceFs tests

---

## Next: Sprint 5 - Capabilities

With standard services complete, we can now build capability services:

1. **NostrFs** - Nostr/NIP-90 event publishing
2. **WsFs** - WebSocket connections
3. **HttpFs** - HTTP client

These will enable agents to interact with external services.
