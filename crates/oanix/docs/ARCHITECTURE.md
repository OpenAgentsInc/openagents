# OANIX Architecture

OANIX (OpenAgents Agent Operating Environment) is a Rust-native agent OS inspired by [WANIX](https://github.com/tractordev/wanix) by Jeff Lindsay and Plan 9 from Bell Labs. It provides isolated, composable execution environments for WebAssembly workloads.

## Core Philosophy

### Everything is a File

Like Plan 9, OANIX exposes all capabilities as filesystems. Instead of system calls or APIs, agents interact with their environment by reading and writing files:

```
/task/spec.json     # Read task definition
/workspace/src/     # Access project files
/logs/stdout.log    # Write output
/cap/nostr/submit   # Send Nostr request by writing JSON
```

### Per-Process Namespaces

Each agent gets its own namespace - a custom view of the filesystem constructed by mounting services at specific paths. This provides:

- **Isolation**: Agents can't access anything not explicitly mounted
- **Composition**: Mix and match services for different use cases
- **Capability-based security**: Access granted by what you mount, not global permissions

### WASI-First Execution

Agents run as WebAssembly modules using WASI (WebAssembly System Interface):

- Same binary runs on server, desktop, and browser
- Strong sandboxing with no escape hatches
- Deterministic execution for reproducibility
- Near-native performance

## Architecture Layers

```
┌─────────────────────────────────────────────────────┐
│                    Application                       │
│         (Commander, Vibe, Agent Tools)              │
└─────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│                   OANIX Manager                      │
│         (Environment lifecycle, scheduling)          │
└─────────────────────────────────────────────────────┘
                         │
          ┌──────────────┼──────────────┐
          ▼              ▼              ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│  OanixEnv   │  │  OanixEnv   │  │  OanixEnv   │
│  Namespace  │  │  Namespace  │  │  Namespace  │
│  WASI Rt    │  │  WASI Rt    │  │  WASI Rt    │
└─────────────┘  └─────────────┘  └─────────────┘
      │                │                │
      ▼                ▼                ▼
┌─────────────────────────────────────────────────────┐
│                  FileService Layer                   │
│   MemFs │ TaskFs │ WorkspaceFs │ LogsFs │ CapFs    │
└─────────────────────────────────────────────────────┘
```

## Core Abstractions

### FileService Trait

The foundation of OANIX. Every mountable service implements this trait:

```rust
pub trait FileService: Send + Sync {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError>;
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;
    fn stat(&self, path: &str) -> Result<Metadata, FsError>;
    fn mkdir(&self, path: &str) -> Result<(), FsError>;
    fn remove(&self, path: &str) -> Result<(), FsError>;
    fn rename(&self, from: &str, to: &str) -> Result<(), FsError>;
}
```

### Namespace

A collection of mounts that define an environment's view of the world:

```rust
let namespace = Namespace::builder()
    .mount("/task", TaskFs::new(task_spec))
    .mount("/workspace", WorkspaceFs::new("/path/to/project"))
    .mount("/logs", MemFs::new())
    .mount("/cap/nostr", NostrFs::new(relay_config))
    .build();
```

Path resolution uses longest-prefix matching:
- `/workspace/src/main.rs` → WorkspaceFs handles `src/main.rs`
- `/cap/nostr/submit` → NostrFs handles `submit`

### Mount

A binding between a path prefix and a FileService:

```rust
pub struct Mount {
    pub path: String,              // e.g., "/workspace"
    pub service: Arc<dyn FileService>,
}
```

### FileHandle

An open file with read/write/seek operations:

```rust
pub trait FileHandle: Send + Sync {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError>;
    fn seek(&mut self, pos: u64) -> Result<(), FsError>;
    fn position(&self) -> u64;
    fn flush(&mut self) -> Result<(), FsError>;
}
```

## Standard Namespace Layout

OANIX environments use a consistent structure:

```
/
├── task/                    # Task definition
│   ├── spec.json           # Full task specification
│   ├── meta.json           # Metadata, tags, timestamps
│   └── status              # Live status (computed)
│
├── workspace/              # Project files
│   ├── src/
│   ├── Cargo.toml
│   └── ...
│
├── logs/                   # Structured output
│   ├── stdout.log         # Standard output
│   ├── stderr.log         # Standard error
│   └── atif/              # ATIF trajectory files
│
└── cap/                    # Capabilities
    ├── nostr/             # Nostr/NIP-90
    │   ├── submit         # Write request JSON
    │   └── events         # Read response stream
    │
    ├── ws/                # WebSocket
    │   ├── control        # Open/close connections
    │   └── conns/{id}/    # Per-connection streams
    │
    └── payments/          # Lightning
        ├── invoices/new   # Create invoice
        └── invoices/{id}  # Check status
```

## Data Flow

### Reading a File

```
Agent                    OANIX                    FileService
  │                        │                          │
  │  read("/workspace/x")  │                          │
  │───────────────────────>│                          │
  │                        │  resolve("/workspace/x") │
  │                        │─────────────────────────>│
  │                        │  (WorkspaceFs, "x")      │
  │                        │<─────────────────────────│
  │                        │                          │
  │                        │  open("x", READ)         │
  │                        │─────────────────────────>│
  │                        │  FileHandle              │
  │                        │<─────────────────────────│
  │                        │                          │
  │  file contents         │  handle.read()           │
  │<───────────────────────│─────────────────────────>│
```

### Writing to a Capability

```
Agent                    OANIX                    NostrFs
  │                        │                          │
  │  write("/cap/nostr/    │                          │
  │        submit", json)  │                          │
  │───────────────────────>│                          │
  │                        │  resolve(...)            │
  │                        │  open("submit", WRITE)   │
  │                        │─────────────────────────>│
  │                        │                          │
  │                        │  handle.write(json)      │
  │                        │─────────────────────────>│
  │                        │        │                 │
  │                        │        │ (publishes to   │
  │                        │        │  Nostr relay)   │
  │                        │        ▼                 │
  │  OK                    │                          │
  │<───────────────────────│                          │
```

## Platform Support

OANIX is designed to run on multiple platforms with the same abstractions:

| Component | Native (Desktop) | Browser (WASM) |
|-----------|-----------------|----------------|
| WorkspaceFs | Real filesystem (`std::fs`) | IndexedDB or in-memory |
| WsFs | `tokio-tungstenite` | Browser WebSocket API |
| LogsFs | Disk files | In-memory + server sync |
| WASI Runtime | Wasmtime | Browser WASM runtime |

The `FileService` trait doesn't know or care which platform it's running on.

## Security Model

1. **No default capabilities**: WASI modules have no inherent access to host resources
2. **Explicit mounting**: All access mediated through namespace mounts
3. **Read-only by default**: WorkspaceFs can be mounted read-only for benchmarking
4. **Controlled networking**: No raw sockets; network access via `/cap/*` services
5. **Audit trail**: All operations logged to `/logs/`

## Thread Safety

- `Namespace` is immutable after construction (wrapped in `Arc`)
- `FileService` implementations must be `Send + Sync`
- `MemFs` uses `RwLock` for interior mutability
- `FileHandle` instances are not shared across threads
