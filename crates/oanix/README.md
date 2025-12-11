# OANIX: OpenAgents Agent Operating Environment

**A Rust-native agent OS inspired by Plan 9 from Bell Labs and WANIX**

---

## What is OANIX? (Plain English)

OANIX is like a **mini operating system for AI agents**. Instead of letting an agent loose on your real computer, you create a fake "world" for it to live in - and that world only contains exactly what you want the agent to see.

### Everything is a File

This is the Plan 9 philosophy. Instead of having different APIs for different things, everything looks like files and folders:

- Want to give an agent access to your project? Mount it at `/workspace`
- Want it to be able to send Nostr messages? Mount a Nostr service at `/cap/nostr`
- Want logs? They appear at `/logs/stdout.log`

The agent just reads and writes files. It doesn't know or care if `/workspace` is a real folder, an in-memory filesystem, or something synced from the cloud.

### Why This is Cool

**1. Security by Default**
- Agent can't access your home folder unless you mount it
- Agent can't hit the network unless you give it a network capability
- No "oops it deleted my files" scenarios

**2. Portable Sandboxes**
- Same WASM binary runs on Mac, Linux, Windows, browser
- Same namespace definition works everywhere
- Test locally, deploy anywhere

**3. Composable Capabilities**
```
Agent A: /workspace + /cap/nostr + /cap/payments
Agent B: /workspace + /cap/http (read-only)
Agent C: /task (read-only) - can only see its instructions
```
Each agent gets exactly what it needs. No more, no less.

**4. Reproducible Runs**
- Namespace is explicit and serializable
- Can replay an agent's run with the same inputs
- Perfect for benchmarking (Terminal-Bench!)

### Quick Demo

**Browser Namespace Explorer:**
```bash
cd crates/oanix
wasm-pack build --target web --features browser
python -m http.server 8080
# Open http://localhost:8080/examples/browser/
```

**Run a WASI binary in a namespace:**
```bash
# Build the test binary
cd examples/hello-wasi && cargo build --target wasm32-wasip1 --release && cd ../..

# Run it with OANIX
cargo run --features wasi --example run_wasi -- \
    examples/hello-wasi/target/wasm32-wasip1/release/hello-wasi.wasm
```

The WASI program can read `/workspace`, write to `/tmp`, and list directories - all within the isolated namespace.

### The One-Liner

> OANIX lets you create isolated, portable worlds for AI agents where they can only see and do what you explicitly allow - and the same agent runs identically on any platform.

---

## Overview (Technical)

OANIX is a **Rust-native agent operating environment** designed to execute WebAssembly (WASI) workloads in secure, composable, and portable sandboxes. Inspired by **Plan 9 from Bell Labs** and **WANIX** (Jeff Lindsay's WebAssembly runtime), OANIX adapts their core architectural insights for modern agent systems:

- **Everything is a file/service** - All capabilities exposed as mountable filesystems
- **Per-process namespaces** - Each agent has its own isolated view of the world
- **Capability-based security** - Access is granted by what you mount, not by global permissions
- **WASI-first execution** - Portable, deterministic, sandboxed workloads

OANIX is not a general-purpose Unix clone. It is an **agent OS** whose primary job is to define precise execution environments by assembling services into namespaces, then running WASI binaries inside those namespaces.

---

## Goals

1. **Isolated, composable environments for agents**
   - Each agent/task runs in a private OANIX namespace
   - All state and capabilities exposed as mountable services

2. **Plan 9-style namespace model**
   - Capabilities granted by mounting services (`/workspace`, `/cap/nostr`)
   - The "API surface" is entirely defined by the namespace

3. **WASM/WASI-first execution**
   - Same binaries run across browser, server, and edge
   - Deterministic, reproducible workloads

4. **High observability**
   - Structured logs and ATIF trajectories under `/logs`
   - Easy to capture and replay agent runs

5. **Host-controlled capabilities**
   - No default network access
   - External capabilities exposed via higher-level mounted services

---

## Plan 9 Inspirations

### Everything is a File (or Service)

In Plan 9, devices, system services, and remote resources are all accessed as files. OANIX exposes core constructs - tasks, workspaces, logs, capabilities - as **virtual filesystems** behind a uniform `FileService` trait.

### Per-Process Namespaces

Plan 9 allows each process to build its own view of the system by mounting services at arbitrary locations. OANIX gives each environment a dedicated **namespace** composed of mounts:

```
/task           - Task specification & metadata
/workspace      - Code or data snapshot
/logs           - Structured logs, ATIF trajectories
/cap/nostr      - Nostr/NIP-90 capability
/cap/ws         - WebSocket capability
/cap/payments   - Lightning payment capability
```

### Service-Oriented Design

System functionality is decomposed into services implementing `FileService`. The environment definition is simply: *which services are mounted, and where*.

### Uniformity Across Contexts

Plan 9 used 9P to unify local/remote resources. OANIX uses WASI + host capabilities to unify execution across browser, edge, and servers. From the agent's perspective, the world is defined solely by the namespace.

---

## Architecture

```
              +-------------------------+
              |  Rust Web App / API     |
              +------------+------------+
                           |
                      [OANIX Manager]
                           |
      +--------------------+---------------------+
      |                                          |
+-----v------------------+            +----------v--------------+
| OANIX Environment #1   |            | OANIX Environment #N    |
|  - Namespace           |            |  - Namespace            |
|  - WASI runtime        |            |  - WASI runtime         |
+-----+------------------+            +----------+--------------+
      |                                          |
  [WASI modules]                            [WASI modules]
```

### Key Components

- **oanix-core** - Namespace, Mount, FileService, basic filesystems
- **oanix-wasi** - Integration with Wasmtime/Wasmer
- **oanix-scheduler** - Job abstraction and lifecycle
- **oanix-web** - HTTP/WebSocket APIs for environments and jobs

---

## Core Abstractions

### FileService Trait

```rust
pub trait FileService: Send + Sync {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError>;
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;
    fn stat(&self, path: &str) -> Result<Metadata, FsError>;
}
```

### Namespace & Mount

```rust
pub struct Mount {
    pub path: String,                  // e.g., "/task"
    pub service: Arc<dyn FileService>, // e.g., TaskFs
}

#[derive(Clone)]
pub struct Namespace {
    mounts: Arc<Vec<Mount>>,
}

impl Namespace {
    pub fn builder() -> NamespaceBuilder { /* ... */ }

    pub fn resolve(&self, full_path: &str) -> Option<(&dyn FileService, &str)> {
        // longest-prefix match over mounts
    }
}
```

### OANIX Environment

```rust
pub struct OanixEnv {
    pub id: Uuid,
    namespace: Namespace,
    wasi_runtime: WasiRuntime,
}

impl OanixEnv {
    pub fn new(namespace: Namespace, wasi_runtime: WasiRuntime) -> Self { /* ... */ }

    pub async fn run_wasi(
        &self,
        wasm_bytes: &[u8],
        cfg: RunConfig,
    ) -> anyhow::Result<RunResult> {
        let instance = self.wasi_runtime
            .instantiate_with_namespace(wasm_bytes, &self.namespace, &cfg)
            .await?;
        instance.run().await
    }
}
```

### Job Abstraction

```rust
pub enum JobKind {
    TerminalBench { task_id: String },
    Script { script_path: String },
    AgentTool { name: String, args: Vec<String> },
}

pub struct JobSpec {
    pub id: Uuid,
    pub env_id: Uuid,
    pub kind: JobKind,
    pub created_at: DateTime<Utc>,
}

pub enum JobStatus {
    Pending,
    Running,
    Succeeded(RunResult),
    Failed { error: String },
}
```

---

## Standard Services

### TaskFs (`/task`)

Exposes task specification and metadata:
- `/task/spec.json` - Full task definition
- `/task/meta.json` - Metadata, tags, creation time

### WorkspaceFs (`/workspace`)

POSIX-like directory tree for project files. Can be:
- Read-only for benchmarking tasks
- Read-write for development sessions

### LogsFs (`/logs`)

Structured logging:
- `/logs/stdout.log`, `/logs/stderr.log`
- `/logs/atif/` - ATIF trajectory JSON files
- `/logs/metrics.json` - Aggregated metrics

### Capability Services (`/cap/*`)

External capabilities as file services:

**NostrFs** (`/cap/nostr`):
- `/cap/nostr/submit` - Write request JSON to schedule jobs
- `/cap/nostr/events` - Stream of response events

**WsFs** (`/cap/ws`):
- `/cap/ws/control` - Open/close connections
- `/cap/ws/conns/{id}/in` - Incoming frames
- `/cap/ws/conns/{id}/out` - Outgoing frames

**PaymentsFs** (`/cap/payments`):
- `/cap/payments/invoices/new` - Create invoice
- `/cap/payments/invoices/{id}` - Status inspection

---

## Security Model

1. **No global APIs** - WASI modules have no inherent right to host OS or network
2. **Capabilities via namespaces** - All access mediated by mounted services
3. **Per-environment policies** - Which services mount, read-only vs read-write, limits
4. **Controlled networking** - External access via `/cap/*` services, not raw sockets

---

## Implementation: `crates/oanix/`

Proposed crate structure:

```
crates/oanix/
  Cargo.toml
  src/
    lib.rs
    namespace.rs       # Namespace, Mount, NamespaceBuilder
    service.rs         # FileService trait, FileHandle, FsError
    env.rs             # OanixEnv, RunConfig, RunResult
    scheduler.rs       # JobSpec, JobStatus, Scheduler trait
    services/
      mod.rs
      mem_fs.rs        # In-memory filesystem
      task_fs.rs       # Task specification service
      workspace_fs.rs  # Workspace/project files
      logs_fs.rs       # Logging and ATIF
      pty_fs.rs        # Pseudo-terminal support
    cap/
      mod.rs
      nostr_fs.rs      # Nostr/NIP-90 capability
      ws_fs.rs         # WebSocket capability
      payments_fs.rs   # Lightning payments
    wasi/
      mod.rs
      runtime.rs       # Wasmtime/Wasmer integration
      bridge.rs        # WASI syscalls -> FileService
    web/
      mod.rs
      api.rs           # HTTP endpoints
      stream.rs        # WebSocket log streaming
```

### Build Targets

- **Native** (`cargo build`) - Server/CLI usage with Wasmtime
- **WASM** (`cargo build --target wasm32-unknown-unknown`) - Browser kernel

---

## Native First, Browser Later

OANIX is designed to work in Commander (native Rust) first, then extract to browser later. The architecture supports this because **the core abstractions are platform-agnostic**.

The `FileService` trait doesn't know or care if it's running on macOS or in a browser:

```rust
pub trait FileService: Send + Sync {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError>;
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;
    fn stat(&self, path: &str) -> Result<Metadata, FsError>;
}
```

### Same Code, Different Implementations

| Service | Native (Commander) | Browser |
|---------|-------------------|---------|
| WorkspaceFs | Real filesystem via `std::fs` | IndexedDB or in-memory |
| WsFs | `tokio-tungstenite` | Browser `WebSocket` API |
| LogsFs | Write to disk | In-memory + sync to server |
| WASI runtime | Wasmtime | Browser's WASM runtime |

### Development Flow

1. **Now in Commander**: OANIX runs as a Rust library. Mount `/workspace` pointing at real directories. Run WASI modules via Wasmtime. Vibe renders in GPUI.

2. **Later in browser**: Same `Namespace`, `Mount`, `FileService` code compiles to WASM. Swap in browser-compatible service implementations. Vibe renders to DOM.

### Why This Works

The Plan 9 philosophy is the key - by making everything a mountable file service, you're forced into abstractions that don't leak platform details. The "what can this agent see" question is answered by the namespace, not by what OS you're on.

Build it in Commander first, prove the model works, then extract to browser. The architecture explicitly supports this.

---

## Implementation Phases

### Phase 1: Core Kernel
- `Namespace`, `Mount`, `FileService` traits
- `MemFs` and `LogsFs` implementations
- Basic Wasmtime integration, run simple WASI binary

### Phase 2: Environment & Scheduler
- `OanixEnv` abstraction with `run_wasi`
- In-memory scheduler with `JobSpec`/`JobStatus`
- API for create environments, submit jobs, query status

### Phase 3: Plan 9-Style Services
- `TaskFs` and `WorkspaceFs`
- Standard namespace profiles (`terminalbench`, `sandbox-dev`)
- Integration with OpenAgents task storage

### Phase 4: Capabilities
- `NostrFs`, `WsFs`, `PaymentsFs`
- Policy layer for capability access control

### Phase 5: Web Integration
- HTTP/WebSocket endpoints for environment lifecycle
- Streaming logs from `/logs/*`
- UI components for namespace visualization

---

## Why WASM?

Benefits beyond "runs in browser":

| Benefit | Impact |
|---------|--------|
| Universal sandbox format | Same agent tools everywhere |
| Strong isolation | Safe execution of untrusted code |
| Sub-ms startup | Perfect for agent micro-tasks |
| Deterministic | Reliable ATIF + benchmarking |
| Language-agnostic | Universal plugin system |
| Low overhead | Replace containers for agent tasks |
| Capability-based | Direct match for OANIX namespaces |
| Near-native speed | Heavy workloads without native code |

---

## References

- [Plan 9 from Bell Labs](https://9p.io/plan9/)
- [WANIX](https://github.com/tractordev/wanix) - WebAssembly runtime inspired by Plan 9
- [WASI](https://wasi.dev/) - WebAssembly System Interface
