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

Jeff Lindsay's WebAssembly runtime inspired by Plan 9. WANIX brings Plan 9 concepts to the browser with a focus on **browser-first execution**:

### Core Philosophy

"A virtual environment runtime for the web, inspired by Plan 9."

- **Run WASI and x86 programs on web pages** — Not just Node, actual browser tabs
- **Apply Plan 9 ideas in the browser** — Namespaces, filesystem abstraction, tasks
- **Build a web-native operating system** — The browser becomes the OS

### Key Insights

1. **Per-process namespaces in browser**
   - Each process gets isolated view of capabilities
   - Security through capability mounting, not sandboxing

2. **Tasks as virtual processes**
   - POSIX process "shape" (args, env, stdin/stdout, exit code)
   - Backed by Web Workers, VMs, or remote jobs
   - Created and managed via filesystem (`task/new/wasi`)

3. **Capabilities via filesystem**
   ```
   #task      — Process/task management
   #cap       — Mounted capabilities
   #bundle    — Loaded application bundle
   #console   — Browser console
   ```

4. **Browser APIs as files**
   - DOM, fetch, WebSocket exposed through FileService
   - Universal interface across all platforms

### Browser-First Example

```javascript
// Create WASI task in browser
const tid = (await wanix.readText("task/new/wasi")).trim();
await wanix.writeFile(`task/${tid}/cmd`, `#bundle/agent.wasm`);
const stdout = await wanix.openReadable(`task/${tid}/fd/1`);
await wanix.writeFile(`task/${tid}/ctl`, "start");
stdout.pipeTo(logStream);  // Real WASI execution in browser
```

### What We Learn

- **WASI portability is real** — Same binary runs server and browser
- **Filesystem abstraction scales** — Works for capabilities, not just files
- **Browser is a first-class target** — Not a fallback, a primary deployment

**Reference**: https://github.com/tractordev/wanix
**Demo**: https://wanix.run

---

## Apptron

Apptron extends WANIX concepts to create a **full Linux environment in the browser**:

### What It Is

"Runs entirely in the browser and does not depend on the cloud."

- VSCode-based editor in browser
- Full Alpine Linux via v86 x86 emulator
- WANIX for native WASM and DOM filesystem access
- "Similar to Smalltalk" — self-contained compute environment

### Key Features

1. **Full Linux in Browser**
   - Alpine Linux with custom kernel via v86
   - Install packages with `apk`
   - Run git, make, esbuild out of the box

2. **Persistence via Browser Storage**
   - Project, home, public directories persisted
   - Cloud synced automatically
   - Changes outside persist directories reset on reload

3. **Virtual Network**
   - Virtual DHCP assigns session IP
   - Session IPs routable across browser tabs/devices
   - Bind TCP port → get public HTTPS endpoint (like ngrok)

4. **DOM APIs via Filesystem**
   - WANIX integration for "native Wasm executable support"
   - Access browser APIs through filesystem abstraction

### Why This Matters for Agents

Apptron proves:
- **Full compute environments run in browser** — Not toy demos
- **Network presence from browser** — Agents can expose HTTP APIs
- **Cross-device communication** — Session IPs enable agent coordination
- **Heavy workloads are possible** — Linux toolchain, x86 emulation

### Virtual Network for Agents

```
Browser Agent A                     Browser Agent B
      │                                    │
      ├── Session IP: 10.0.0.1            ├── Session IP: 10.0.0.2
      │                                    │
      └── WebSocket tunnel ───────────────►│
            to public endpoint             │
                                           │
          Both agents can communicate via session IPs
          or through public HTTPS endpoints
```

**Reference**: https://github.com/progrium/apptron

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

| Aspect | Plan 9 | WANIX | Apptron | OANIX | Our Runtime |
|--------|--------|-------|---------|-------|-------------|
| Language | C | TS/WASM | Go/TS | Rust | Rust |
| Execution | Processes | WASI | WASI + x86 | WASI | Native + WASI |
| Namespace | 9P mounts | Service mounts | WANIX + Linux | FileService | FileService |
| Network | IP/9P | Browser APIs | Virtual network | Executor bridge | Drivers |
| Target | OS | Browser | Browser | Server/Browser | Everywhere |
| Focus | General OS | Sandbox | Full dev env | Agent sandbox | Agent runtime |

---

## What We Take Forward

From Plan 9:
- Everything is a file
- Per-agent namespaces
- Mount tables as capability map
- Plumber-style event routing

From WANIX:
- Browser as first-class target, not fallback
- WASI portability across server/desktop/browser
- FileService abstraction for capabilities
- Tasks with POSIX process shape

From Apptron:
- Virtual network for browser-to-browser communication
- Public HTTPS endpoints from browser agents
- Heavy compute is possible in browser (x86 emulation)
- Persistence via browser storage with cloud sync

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
- Multiple backend support (Browser, Cloudflare, Local, K8s)
- Nostr-native identity and communication
- Autonomy levels and transparency
- True write-once-run-anywhere via WASI
