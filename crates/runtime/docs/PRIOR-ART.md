# Prior Art & Related Work

References and inspirations for the OpenAgents Runtime.

---

## Plan 9 from Bell Labs

The foundational inspiration. Plan 9 reimagined Unix with radical consistency:

- **Everything is a file** — Devices, network, processes, all accessed as files
- **Per-process namespaces** — Each process builds its own view of the system
- **9P protocol** — Simple file protocol for local and remote resources
- **Uniform naming** — Resources named uniformly regardless of location

Key concepts adapted for agents:
- Agent filesystem surface (`/agents/<id>/status`, `/inbox`, etc.)
- Mount tables / capability namespaces for security
- Plumber for event routing
- Factotum for signing service

**Reference**: https://9p.io/plan9/

---

## WANIX

Jeff Lindsay's WebAssembly runtime inspired by Plan 9. WANIX brings Plan 9 concepts to the browser:

- Per-environment namespaces composed of mounted services
- WASI execution in sandboxed environments
- FileService abstraction for capabilities
- Runs on browser, server, and edge

WANIX demonstrated that Plan 9 concepts work well for sandboxed, portable execution environments.

**Reference**: https://github.com/tractordev/wanix

---

## OANIX (OpenAgents NIX)

Our experimental Rust-native agent operating environment, developed prior to this runtime crate. OANIX explored:

### Core Abstractions

```rust
/// A capability exposed as a filesystem
pub trait FileService: Send + Sync {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError>;
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;
    fn stat(&self, path: &str) -> Result<Metadata, FsError>;
    fn mkdir(&self, path: &str) -> Result<(), FsError>;
    fn remove(&self, path: &str) -> Result<(), FsError>;
    fn rename(&self, from: &str, to: &str) -> Result<(), FsError>;
}

/// Namespace with longest-prefix matching
pub struct Namespace {
    mounts: Arc<Vec<Mount>>,
}

impl Namespace {
    pub fn resolve(&self, path: &str) -> Option<(&dyn FileService, &str)> {
        // Longest-prefix match across mounts
    }
}
```

### Standard Services

| Service | Mount Point | Purpose |
|---------|-------------|---------|
| TaskFs | `/task` | Task spec, metadata, status, results |
| WorkspaceFs | `/workspace` | Project files (git repo, snapshot) |
| LogsFs | `/logs` | Structured logs, ATIF trajectories |
| MemFs | `/tmp` | In-memory scratch space |
| NostrFs | `/cap/nostr` | Nostr event signing and DVM |
| WsFs | `/cap/ws` | WebSocket connection management |
| HttpFs | `/cap/http` | HTTP request/response client |

### Executor Bridge

OANIX solved the sync FileService / async network I/O problem with `ExecutorManager`:

```rust
pub struct ExecutorManager {
    runtime: Runtime,
    shutdown_tx: broadcast::Sender<()>,
    http_fs: Option<Arc<HttpFs>>,
    ws_fs: Option<Arc<WsFs>>,
    nostr_fs: Option<Arc<NostrFs>>,
}

impl ExecutorManager {
    pub fn attach_http(&mut self, http_fs: Arc<HttpFs>);
    pub fn attach_ws(&mut self, ws_fs: Arc<WsFs>);
    pub fn start(&mut self) -> Result<()>;
    pub fn shutdown(self) -> Result<()>;
}
```

Services write to buffers synchronously; executors poll buffers and perform async I/O.

### Environment Abstraction

```rust
pub struct OanixEnv {
    id: Uuid,
    namespace: Namespace,
    status: Arc<RwLock<EnvStatus>>,
    wasi_runtime: Option<WasiRuntime>,
}

impl OanixEnv {
    pub fn run_wasi(&mut self, wasm_bytes: &[u8], config: RunConfig) -> Result<RunResult>;
}
```

### Job Scheduler

Priority-based job queue with concurrency limits:

```rust
pub struct Scheduler {
    environments: HashMap<Uuid, Arc<RwLock<OanixEnv>>>,
    pending: VecDeque<JobSpec>,  // Priority queue
    running: HashMap<Uuid, JobSpec>,
    max_concurrent: usize,
}

impl Scheduler {
    pub fn submit(&mut self, job: JobSpec) -> Result<Uuid>;
    pub fn next(&mut self) -> Option<JobSpec>;
    pub fn complete(&mut self, job_id: &Uuid, exit_code: i32);
    pub fn tick(&mut self) -> Result<Option<JobResult>>;  // Execute next job
}
```

### Key Lessons from OANIX

1. **Keep FileService sync** — Async traits complicate everything; use executor bridge
2. **Longest-prefix matching** — Essential for overlapping mounts like `/tools` and `/tools/core`
3. **Immutable namespaces** — Clone namespace, never mutate after construction
4. **Builder pattern for environments** — `EnvBuilder::new().mount(...).build()`
5. **Status tracking** — Environment lifecycle: Created → Running → Completed/Failed
6. **WASI enables portability** — Same binary runs on server, desktop, browser

### OANIX Crate Structure

```
crates/oanix/
├── src/
│   ├── lib.rs
│   ├── namespace.rs      # Namespace, Mount, NamespaceBuilder
│   ├── service.rs        # FileService, FileHandle traits
│   ├── error.rs          # FsError, OanixError
│   ├── env/              # OanixEnv, EnvBuilder
│   ├── scheduler/        # Job scheduler
│   ├── services/         # Standard services
│   │   ├── mem_fs.rs
│   │   ├── map_fs.rs     # Static/immutable filesystem
│   │   ├── func_fs.rs    # Dynamic files via closures
│   │   ├── cow_fs.rs     # Copy-on-write overlay
│   │   ├── task_fs.rs
│   │   ├── workspace_fs.rs
│   │   ├── logs_fs.rs
│   │   ├── http_fs.rs
│   │   ├── ws_fs.rs
│   │   └── nostr_fs.rs
│   ├── executor/         # Network I/O bridge
│   └── wasi/             # Wasmtime integration
```

OANIX was archived but its patterns inform this runtime design.

---

## Cloudflare Durable Objects

Cloudflare's actor model for edge computing:

- Single-threaded actors with SQLite storage
- Global uniqueness via naming scheme
- WebSocket hibernation for efficient connections
- Alarms for scheduling

Influences on our design:
- DO-like tick model for agent execution
- SQLite-per-agent storage pattern
- WebSocket hibernation concept for drivers
- Alarm-based scheduling

**Reference**: https://developers.cloudflare.com/durable-objects/

---

## Rivet

Open-source portable actor framework:

- Run on Cloudflare, Node.js, or custom backends
- Actor-to-actor RPC
- State management

We chose to build our own because:
- Rivet is TypeScript-first; we need Rust
- Agent-specific features (identity, memory, economics) not included
- Control over the core abstraction

**Reference**: https://github.com/rivet-gg/actor-core

---

## Actor Model (General)

The runtime adapts classic actor model concepts:

| Actor Concept | Agent Adaptation |
|---------------|------------------|
| Mailbox | Envelope queue (inbox) |
| Message passing | Envelopes via drivers |
| Isolated state | Agent storage (SQLite) |
| Location transparency | Backend abstraction |
| Supervision | Control plane, budgets |

Key differences from pure actors:
- Agents have **identity** (Nostr keypairs)
- Agents have **memory** (conversations, patterns)
- Agents have **economics** (budgets, payments)
- Agents are **transparent** (trajectories, logs)

---

## Comparison Matrix

| Aspect | Plan 9 | WANIX | OANIX | Our Runtime |
|--------|--------|-------|-------|-------------|
| Language | C | TS/WASM | Rust | Rust |
| Execution | Processes | WASI | WASI | Native + WASI |
| Namespace | 9P mounts | Service mounts | FileService | FileService |
| Network | IP/9P | Browser APIs | Executor bridge | Drivers |
| Target | OS | Browser | Server/Browser | Everywhere |
| Focus | General OS | Sandbox | Agent sandbox | Agent runtime |

---

## What We Take Forward

From Plan 9:
- Everything is a file
- Per-agent namespaces
- Mount tables as capability map
- Plumber-style event routing

From WANIX:
- Browser portability via WASM
- FileService abstraction

From OANIX:
- Rust trait definitions
- Executor bridge pattern
- Environment lifecycle
- Job scheduler

From Durable Objects:
- Tick execution model
- SQLite per-agent
- WebSocket hibernation
- Alarm scheduling

New in our runtime:
- Agent-specific traits (memory, identity, economics)
- Multiple backend support (not just Cloudflare)
- Nostr-native identity and communication
- Autonomy levels and transparency
