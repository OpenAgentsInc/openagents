# OANIX: OpenAgents Agent Operating Environment

**A Rust-native agent OS inspired by Plan 9 from Bell Labs**

---

## What is OANIX? (Plain English)

OANIX is like a **mini operating system for AI agents**. Instead of letting an agent loose on your real computer, you create a controlled "world" for it to live in - and that world only contains exactly what you want the agent to see.

### Everything is a File

This is the Plan 9 philosophy. Instead of having different APIs for different things, everything looks like files and folders:

```
/task/spec.json     → The agent reads its assignment here
/task/status        → Shows "pending", "running", "completed", or "failed"
/task/result.json   → Agent writes its final answer here
/workspace/         → The code or data the agent works on
/logs/stdout.log    → Everything the agent prints
/logs/events.jsonl  → Structured log events (JSON)
/tmp/               → Scratch space
```

The agent just reads and writes files. It doesn't know if `/workspace` is a real folder, an in-memory snapshot, or something synced from the cloud.

### What Can You Build With This?

**Run a coding agent safely:**
```rust
let ns = Namespace::builder()
    .mount("/task", TaskFs::new(spec, meta))           // What to do
    .mount("/workspace", WorkspaceFs::new("./project")?) // Real project files
    .mount("/logs", LogsFs::new())                     // Capture everything
    .mount("/tmp", MemFs::new())                       // Scratch space
    .build();

// Agent reads /task/spec.json, modifies /workspace, writes /task/result.json
// You can review /logs/stdout.log to see what it did
```

**Benchmark agents reproducibly:**
```rust
// Snapshot the workspace so agent modifications don't affect the original
let workspace = CowFs::new(MapFs::builder()
    .file("/src/main.rs", include_bytes!("fixtures/main.rs"))
    .build());

// Run multiple agents against the same snapshot
// Compare their /task/result.json outputs
```

**Give different agents different capabilities:**
```rust
// Agent A: Can modify files and access network
Agent A: /workspace (read-write) + /cap/http

// Agent B: Read-only access, no network
Agent B: /workspace (read-only)

// Agent C: Can only see its task, nothing else
Agent C: /task (read-only)
```

**Connect to real networks (HTTP, WebSocket, Nostr):**
```rust
use oanix::executor::{ExecutorManager, ExecutorConfig};

// Create capability services
let http_fs = Arc::new(HttpFs::new());
let ws_fs = Arc::new(WsFs::new());
let nostr_fs = Arc::new(NostrFs::generate()?);

// Attach executors that do real network I/O
let mut executor = ExecutorManager::new(ExecutorConfig::default())?;
executor.attach_http(Arc::clone(&http_fs));
executor.attach_ws(Arc::clone(&ws_fs));
executor.attach_nostr(Arc::clone(&nostr_fs));
executor.start()?;

// Now mount in namespace - agent writes files, executors handle network
let ns = Namespace::builder()
    .mount("/cap/http", http_fs)
    .mount("/cap/ws", ws_fs)
    .mount("/cap/nostr", nostr_fs)
    .build();

// Agent writes to /cap/http/request → executor makes HTTP call
// Response appears in /cap/http/responses/{id}.json

executor.shutdown()?;
```

### Why This is Cool

**1. Security by Default**
- Agent can't access your home folder unless you explicitly mount it
- Agent can't hit the network unless you give it a network capability
- WorkspaceFs prevents path traversal attacks (`../../../etc/passwd` → rejected)

**2. Complete Observability**
- Every print statement captured in `/logs/stdout.log`
- Structured events in `/logs/events.jsonl` for machine processing
- Task lifecycle tracked: pending → running → completed/failed

**3. Portable & Reproducible**
- Same WASM binary runs on Mac, Linux, Windows, browser
- Namespace definition is explicit and serializable
- Perfect for benchmarking (Terminal-Bench!)

**4. Composable Building Blocks**
```rust
// Primitives (low-level)
MemFs      // In-memory read/write filesystem
MapFs      // Static/immutable data (bundled assets)
FuncFs     // Dynamic files computed on-the-fly
CowFs      // Copy-on-write snapshots

// Standard Services (high-level, built from primitives)
TaskFs     // Task spec + status + results
LogsFs     // Structured logging
WorkspaceFs // Real filesystem with security
```

---

## Quick Start

### Run a WASI Binary in a Namespace

```bash
# Build the test binary
cd examples/hello-wasi && cargo build --target wasm32-wasip1 --release && cd ../..

# Run it with OANIX
cargo run --features wasi --example run_wasi -- \
    examples/hello-wasi/target/wasm32-wasip1/release/hello-wasi.wasm
```

The WASI program can read `/workspace`, write to `/tmp`, and list directories - all within the isolated namespace.

### Browser Namespace Explorer

```bash
wasm-pack build --target web --features browser
python -m http.server 8080
# Open http://localhost:8080/examples/browser/
```

---

## What's Implemented

### Primitive Filesystems

| Service | Description |
|---------|-------------|
| **MemFs** | In-memory read/write filesystem |
| **MapFs** | Static/immutable from embedded data |
| **FuncFs** | Dynamic files via closures |
| **CowFs** | Copy-on-write overlay for snapshots |

### Standard Services

| Service | Description |
|---------|-------------|
| **TaskFs** | Task spec, status lifecycle, results |
| **LogsFs** | stdout/stderr + structured events |
| **WorkspaceFs** | Real filesystem wrapper with path security |

### Capability Services

| Service | Description |
|---------|-------------|
| **NostrFs** | Nostr event signing + NIP-90 DVM (requires `nostr` feature) |
| **WsFs** | WebSocket connection management (outbox/inbox pattern) |
| **HttpFs** | HTTP request/response client (queue-based) |

### Network Executors (requires `net-executor` feature)

| Executor | Description |
|----------|-------------|
| **ExecutorManager** | Coordinates all network executors, owns tokio runtime |
| **HttpExecutor** | Executes HTTP requests from HttpFs via reqwest |
| **WsConnector** | Manages WebSocket connections for WsFs |
| **NostrRelayConnector** | Connects to Nostr relays, routes events |

The executors bridge OANIX's sync filesystem APIs to real async network I/O. Your agent writes to `/cap/http/request`, the executor makes the actual HTTP call, and the response appears in `/cap/http/responses/`.

### Runtime

| Feature | Description |
|---------|-------------|
| **WasiRuntime** | Execute WASM binaries via wasmtime |
| **Namespace** | Mount services at paths, resolve routes |
| **Browser support** | wasm-bindgen API for browser usage |

---

## Example: Complete Agent Environment

```rust
use oanix::*;

// 1. Define the task
let task = TaskFs::new(
    TaskSpec {
        id: "review-001".into(),
        task_type: "code-review".into(),
        description: "Review the authentication module".into(),
        input: serde_json::json!({"files": ["src/auth.rs"]}),
    },
    TaskMeta::default(),
);

// 2. Set up logging
let logs = LogsFs::new();

// 3. Wrap real workspace with copy-on-write (agent changes don't affect original)
let workspace = CowFs::new(WorkspaceFs::readonly("./my-project")?);

// 4. Build the namespace
let ns = Namespace::builder()
    .mount("/task", task)
    .mount("/logs", logs)
    .mount("/workspace", workspace)
    .mount("/tmp", MemFs::new())
    .build();

// 5. Agent executes...
// - Reads /task/spec.json to understand the job
// - Reads files from /workspace/src/auth.rs
// - Writes analysis to /logs/stdout.log
// - Writes structured events via logs.info("Found issue: ...")
// - Writes final result to /task/result.json

// 6. Check results
let (task_svc, _) = ns.resolve("/task/result.json").unwrap();
let result = read_file(task_svc, "/result.json");
```

---

## API Overview

### TaskFs - Task Lifecycle

```rust
let task = TaskFs::new(spec, meta);

// Status management
task.set_running();              // Pending → Running
task.set_completed();            // Running → Completed
task.set_failed("Error msg");    // Running → Failed
task.is_finished();              // true if Completed or Failed

// File interface
read_file(&task, "/spec.json");   // Task specification
read_file(&task, "/meta.json");   // Metadata (tags, timeout, etc.)
read_file(&task, "/status");      // Current status as JSON
write_file(&task, "/result.json", result); // Final output
```

### LogsFs - Structured Logging

```rust
let logs = LogsFs::new();

// Programmatic API
logs.write_stdout(b"Output text\n");
logs.write_stderr(b"Error text\n");
logs.info("Task started");
logs.warn("Rate limit approaching");
logs.error("Connection failed");
logs.log_event(LogEvent::with_data(
    LogLevel::Debug,
    "Config loaded",
    serde_json::json!({"timeout": 300}),
));

// File interface (for agents)
// Write to /logs/stdout.log, /logs/stderr.log
// Read /logs/events.jsonl for structured events
```

### WorkspaceFs - Secure Real Filesystem

```rust
// Wrap a directory (read-write)
let workspace = WorkspaceFs::new("/path/to/project")?;

// Read-only mode
let workspace = WorkspaceFs::readonly("/path/to/project")?;

// Security: these are all rejected
workspace.open("/../../../etc/passwd", flags);  // Path escape
workspace.open("/src/../../etc/passwd", flags); // Sneaky escape
```

### NostrFs - Nostr Capability (requires `nostr` feature)

```rust
use oanix::NostrFs;

// Create with a secret key
let secret_key = [0u8; 32]; // Use a real key!
let nostr = NostrFs::new(secret_key)?;

// Or generate a new key
let nostr = NostrFs::generate()?;

// Get identity
println!("pubkey: {}", nostr.pubkey());
println!("npub: {}", nostr.npub());

// Add relays
nostr.add_relay("wss://relay.damus.io");

// Create NIP-90 job request
let event = nostr.create_job_request(
    5050, // Text generation
    "What is the capital of France?",
    HashMap::new(),
)?;

// Events are queued in outbox
let pending = nostr.outbox_events();

// File interface for agents:
// /identity/pubkey    - Read public key hex
// /identity/npub      - Read bech32 npub
// /submit             - Write event template JSON → signed event in outbox
// /request            - Write NIP-90 request JSON → signed event in outbox
// /outbox/{id}.json   - Read pending events
// /inbox/{id}.json    - Read received events
// /status             - Read service status
```

### WsFs - WebSocket Capability

```rust
use oanix::WsFs;

let ws = WsFs::new();

// Open connection programmatically
let conn_id = ws.open_connection("wss://relay.example.com")?;
ws.set_connected(&conn_id)?;  // Called by transport when connected

// Queue outgoing message
ws.send_message(&conn_id, b"Hello WebSocket!".to_vec())?;

// External transport drains outbox and sends
let pending = ws.drain_outbox(&conn_id)?;

// Transport adds received messages
ws.receive_message(&conn_id, b"Response".to_vec())?;

// Read from inbox (FIFO)
let msg = ws.read_message(&conn_id)?;

// File interface for agents:
// /control              - Write {"url": "wss://..."} or {"id": "conn-0"}
// /status               - Read overall service status
// /conns/{id}/out       - Write messages to send
// /conns/{id}/in        - Read received messages
// /conns/{id}/status    - Read connection state
// /conns/{id}/url       - Read connection URL
```

### HttpFs - HTTP Capability

```rust
use oanix::{HttpFs, HttpRequest, HttpResponse, HttpMethod};

let http = HttpFs::new();

// Submit request
let request = HttpRequest {
    method: HttpMethod::Get,
    url: "https://api.example.com/data".to_string(),
    headers: HashMap::from([("Authorization".into(), "Bearer token".into())]),
    ..Default::default()
};
let req_id = http.submit_request(request);

// External executor takes pending request
let pending = http.take_pending(&req_id)?;

// Executor completes request
http.complete_request(HttpResponse {
    request_id: req_id.clone(),
    status: 200,
    status_text: "OK".to_string(),
    headers: HashMap::new(),
    body: r#"{"result": "success"}"#.to_string(),
    duration_ms: 150,
    completed_at: now(),
});

// Read response
let response = http.get_response(&req_id)?;

// File interface for agents:
// /request              - Write request JSON → queued for execution
// /pending/{id}.json    - Read pending request details
// /responses/{id}.json  - Read completed response (or failure)
// /status               - Read service status
```

### ExecutorManager - Real Network I/O (requires `net-executor` feature)

```rust
use oanix::executor::{ExecutorManager, ExecutorConfig, RetryPolicy};
use std::sync::Arc;
use std::time::Duration;

// Configure execution behavior
let config = ExecutorConfig::builder()
    .poll_interval(Duration::from_millis(50))
    .http_timeout(Duration::from_secs(30))
    .http_retry(RetryPolicy::exponential(3, Duration::from_millis(100)))
    .ws_connect_timeout(Duration::from_secs(10))
    .ws_max_concurrent(100)
    .build();

// Create capability services
let http_fs = Arc::new(HttpFs::new());
let ws_fs = Arc::new(WsFs::new());

// Create executor manager
let mut executor = ExecutorManager::new(config)?;
executor.attach_http(Arc::clone(&http_fs));
executor.attach_ws(Arc::clone(&ws_fs));

// Start executors (spawns async tasks)
executor.start()?;

// Now http_fs and ws_fs are "live" - requests get executed
// Submit HTTP request via HttpFs
let req_id = http_fs.submit_request(HttpRequest {
    method: HttpMethod::Get,
    url: "https://api.example.com/data".to_string(),
    ..Default::default()
});

// Wait for response (executor handles it automatically)
// Response appears in http_fs.get_response(&req_id)

// Clean shutdown
executor.shutdown()?;
```

### Namespace - Mount Points

```rust
let ns = Namespace::builder()
    .mount("/task", task_fs)
    .mount("/workspace", workspace_fs)
    .mount("/logs", logs_fs)
    .mount("/cap/nostr", nostr_fs)  // Nostr capability
    .mount("/cap/ws", ws_fs)        // WebSocket capability
    .mount("/cap/http", http_fs)    // HTTP capability
    .mount("/tmp", MemFs::new())
    .build();

// Resolve paths to services
let (service, relative_path) = ns.resolve("/workspace/src/main.rs").unwrap();
```

### OanixEnv - Environment Abstraction

```rust
use oanix::{EnvBuilder, EnvStatus};

// Create a complete agent environment
let env = EnvBuilder::new()
    .mount("/task", task_fs)
    .mount("/logs", logs_fs)
    .mount("/workspace", workspace_fs)
    .mount("/cap/http", HttpFs::new())
    .mount("/tmp", MemFs::new())
    .build()?;

// Check status
println!("ID: {}", env.id());
println!("Status: {:?}", env.status());

// Lifecycle management
env.set_running();
// ... execution ...
env.set_completed(0);  // or env.set_failed("error message");

assert!(env.is_finished());
```

### Scheduler - Job Queue

```rust
use oanix::{Scheduler, JobSpec, JobKind, EnvBuilder};

let mut scheduler = Scheduler::with_max_concurrent(4);

// Register environments
let env = EnvBuilder::new()
    .mount("/workspace", MemFs::new())
    .build()?;
let env_id = scheduler.register_env(env);

// Submit jobs with priority
let job = JobSpec::new(env_id, JobKind::script("echo hello"))
    .with_priority(10)
    .env("DEBUG", "true")
    .with_timeout(300)
    .tag("urgent");
scheduler.submit(job)?;

// Process jobs
while let Some(job) = scheduler.next() {
    // Execute job...
    scheduler.complete(&job.id, 0);
}

// Check status
let status = scheduler.status();
println!("Completed: {}", status.completed_count);
```

---

## Current Status

- **180+ tests passing** (unit + integration + E2E executor tests)
- Native Rust with optional WASM browser support
- WASI execution via wasmtime
- Full capability suite: NostrFs, WsFs, HttpFs
- Network executors with real I/O (HTTP, WebSocket, Nostr relays)
- OanixEnv environment abstraction
- Priority-based job scheduler
- Comprehensive E2E test suite with mock servers

### Running Tests

```bash
# All tests (fast, uses mock servers)
cargo test --features "net-executor,nostr" -p oanix

# Include live network smoke tests (requires internet)
cargo test --features "net-executor,nostr" -p oanix -- --ignored
```

See [docs/ROADMAP.md](docs/ROADMAP.md) for implementation progress.

---

## References

- [Plan 9 from Bell Labs](https://9p.io/plan9/) - The original "everything is a file" OS
- [WANIX](https://github.com/tractordev/wanix) - WebAssembly runtime inspired by Plan 9
- [WASI](https://wasi.dev/) - WebAssembly System Interface
