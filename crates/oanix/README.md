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

### Namespace - Mount Points

```rust
let ns = Namespace::builder()
    .mount("/task", task_fs)
    .mount("/workspace", workspace_fs)
    .mount("/logs", logs_fs)
    .mount("/cap/nostr", nostr_fs)  // Nostr capability
    .mount("/tmp", MemFs::new())
    .build();

// Resolve paths to services
let (service, relative_path) = ns.resolve("/workspace/src/main.rs").unwrap();
```

---

## Current Status

- **89 tests passing** (72 unit + 17 integration)
- Native Rust with optional WASM browser support
- WASI execution via wasmtime
- Nostr/NIP-90 capability (with `nostr` feature)

See [docs/ROADMAP.md](docs/ROADMAP.md) for implementation progress.

---

## References

- [Plan 9 from Bell Labs](https://9p.io/plan9/) - The original "everything is a file" OS
- [WANIX](https://github.com/tractordev/wanix) - WebAssembly runtime inspired by Plan 9
- [WASI](https://wasi.dev/) - WebAssembly System Interface
