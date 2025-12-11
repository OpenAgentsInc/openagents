# OANIX Roadmap

## Current Status

**Sprint 1: Complete** ✅
- [x] `MemFs` - In-memory filesystem with full read/write support
- [x] `FileService` trait with `Serialize` support
- [x] Browser WASM build with wasm-bindgen
- [x] Namespace Explorer demo (Tokyo Night themed)
- [x] 8 passing unit tests

**Sprint 2: Complete** ✅
- [x] Wasmtime integration with WASI preview1
- [x] Namespace-to-WASI bridge (sync to temp directories)
- [x] `WasiRuntime` with `RunConfig` and `RunResult`
- [x] CLI example (`run_wasi`)
- [x] Test WASI binary (`hello-wasi`)

**Sprint 3: Complete** ✅
- [x] `MapFs` - Static/immutable filesystem (~270 lines)
- [x] `FuncFs` - Computed/dynamic files via closures (~330 lines)
- [x] `CowFs` - Copy-on-Write overlay filesystem (~340 lines)
- [x] 20 new tests (29 total)
- [x] `FsError::ReadOnly` error variant

---

## Sprint 4: Standard Services (Next)

### 4.1 TaskFs

Task specification filesystem:

```
/task/
├── spec.json      # Full task definition (MapFs)
├── meta.json      # Metadata (MapFs)
├── status         # Live status (FuncFs)
└── result.json    # Final result (MemFs)
```

```rust
pub struct TaskFs {
    spec: TaskSpec,
    status: Arc<RwLock<TaskStatus>>,
}
```

### 4.2 LogsFs

Structured logging filesystem:

```
/logs/
├── stdout.log     # Append-only
├── stderr.log     # Append-only
├── events/        # Structured events (JSONL)
│   ├── 001.jsonl
│   └── ...
└── atif/          # ATIF trajectory files
    └── trajectory.json
```

```rust
pub struct LogsFs {
    stdout: AppendOnlyFile,
    stderr: AppendOnlyFile,
    events: EventLog,
}
```

### 4.3 WorkspaceFs

Real filesystem wrapper with path restrictions:

```rust
pub struct WorkspaceFs {
    root: PathBuf,
    readonly: bool,
}
```

---

## Sprint 5: Capabilities

### 5.1 NostrFs

Nostr/NIP-90 capability:

```
/cap/nostr/
├── config.json    # Relay configuration
├── submit         # Write request JSON → publishes event
├── events/        # Read response events
└── status         # Connection status
```

### 5.2 WsFs

WebSocket capability:

```
/cap/ws/
├── control        # Write: {"open": "wss://..."} or {"close": "conn-id"}
└── conns/
    └── {id}/
        ├── in     # Read incoming frames
        ├── out    # Write outgoing frames
        └── status # Connection state
```

### 5.3 HttpFs

HTTP client capability:

```
/cap/http/
├── request        # Write request JSON → response in /response
└── response       # Read response (blocks until complete)
```

---

## Sprint 6: OanixEnv & Scheduler

### 6.1 OanixEnv

Complete environment abstraction:

```rust
pub struct OanixEnv {
    pub id: Uuid,
    namespace: Namespace,
    wasi_runtime: WasiRuntime,
    status: Arc<RwLock<EnvStatus>>,
}

impl OanixEnv {
    pub async fn run_wasi(
        &self,
        wasm_bytes: &[u8],
        config: RunConfig,
    ) -> Result<RunResult, OanixError>;

    pub fn namespace(&self) -> &Namespace;
    pub fn status(&self) -> EnvStatus;
}
```

### 6.2 Job Scheduler

```rust
pub struct JobSpec {
    pub id: Uuid,
    pub env_id: Uuid,
    pub kind: JobKind,
    pub priority: i32,
}

pub enum JobKind {
    Wasi { wasm_path: String, args: Vec<String> },
    Script { script: String },
}

pub struct Scheduler {
    jobs: VecDeque<JobSpec>,
    running: HashMap<Uuid, JoinHandle<RunResult>>,
}
```

---

## Future Considerations

### Browser WASI Runtime

Currently Sprint 2 targets native wasmtime. Browser needs different approach:
- Use browser's WebAssembly API
- Implement WASI preview1 in JavaScript
- Consider existing solutions (browser_wasi_shim)

### Persistence

- Save/restore namespaces
- IndexedDB backing for browser MemFs
- SQLite backing for native

### Networking

- 9P protocol for remote mounts
- Distributed namespaces across nodes
- P2P file sharing

### Observability

- OpenTelemetry integration
- Metrics export via `/metrics`
- Distributed tracing

---

## Milestones

| Milestone | Description | Target |
|-----------|-------------|--------|
| **M1** | Run "Hello World" WASI in namespace | Sprint 2 |
| **M2** | Terminal-Bench task in OANIX env | Sprint 4 |
| **M3** | Agent with Nostr capability | Sprint 5 |
| **M4** | Multi-job scheduling | Sprint 6 |
| **M5** | Browser WASI execution | Future |

---

## Contributing

Focus areas where help is welcome:

1. **FileService implementations** - New filesystem types
2. **WASI compatibility** - Edge cases, preview2 support
3. **Browser runtime** - WASI shim, IndexedDB persistence
4. **Documentation** - Examples, tutorials
5. **Testing** - Property-based tests, fuzzing
