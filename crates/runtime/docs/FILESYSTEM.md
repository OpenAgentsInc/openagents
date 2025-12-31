# Agent Filesystem Implementation

Concrete trait definitions for the agent filesystem abstraction.

---

## Overview

Every capability in the runtime is exposed as a **mountable filesystem**. This isn't metaphor—it's the actual implementation pattern. Services implement `FileService`, get mounted into an agent's namespace, and become accessible through uniform file operations.

This pattern originates from Plan 9 and was explored in OANIX (OpenAgents Nix), our experimental agent-centric runtime.

---

## Namespace Scopes

There are **two views** of an agent's filesystem:

### Agent-Local Namespace

What code **inside the agent** sees. Root `/` is the agent itself:

```
/                       # Agent's root
├── status              # Read: agent state JSON
├── inbox/              # Write to enqueue, read to peek
├── goals/              # CRUD goal files
├── identity/
│   ├── pubkey          # Read: hex pubkey
│   └── sign            # Call (write+read same handle) → signature
├── wallet/
│   └── balance         # Read: balance JSON
├── compute/
│   ├── new             # Call (write+read same handle) → job_id
│   ├── providers/      # Available AI providers
│   └── usage           # Budget tracking (reserved/spent)
└── ...
```

Agents use paths like `/status`, `/inbox`, `/wallet/balance`.

### Global Admin Namespace

What **operators, CLI tools, and HTTP clients** see. Prefixed with agent ID:

```
/agents/<agent-id>/
├── status
├── inbox/
├── goals/
└── ...
```

This is just a namespacing wrapper—the underlying services are the same.

### HTTP Mapping

The global namespace maps to HTTP:

| Filesystem Operation | HTTP Equivalent |
|---------------------|-----------------|
| `cat /agents/abc/status` | `GET /agents/abc/status` |
| `echo msg > /agents/abc/inbox` | `POST /agents/abc/inbox` |
| `ls /agents/abc/goals/` | `GET /agents/abc/goals/` |
| `tail -f /agents/abc/logs/trace` | `GET /agents/abc/logs/trace?watch=1` (SSE) |

---

## Core Traits

### FileService

The fundamental trait all capability providers implement:

```rust
/// A capability exposed as a filesystem
pub trait FileService: Send + Sync {
    /// Open a file or directory at the given path
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>>;

    /// List directory contents
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>>;

    /// Get file/directory metadata
    fn stat(&self, path: &str) -> Result<Stat>;

    /// Create a directory
    fn mkdir(&self, path: &str) -> Result<()>;

    /// Remove a file or directory
    fn remove(&self, path: &str) -> Result<()>;

    /// Rename/move a file
    fn rename(&self, from: &str, to: &str) -> Result<()>;

    /// Watch for changes (optional, returns None if not supported)
    fn watch(&self, path: &str) -> Result<Option<Box<dyn WatchHandle>>>;

    /// Service name for debugging
    fn name(&self) -> &str;
}

/// Handle for watching file/directory changes
pub trait WatchHandle: Send {
    /// Block until next change event (or timeout)
    fn next(&mut self, timeout: Option<Duration>) -> Result<Option<WatchEvent>>;

    /// Close the watch
    fn close(&mut self) -> Result<()>;
}

/// Watch event types
pub enum WatchEvent {
    Modified { path: String },
    Created { path: String },
    Deleted { path: String },
    Data(Vec<u8>),  // For streaming files like logs/trace
}

/// Flags for opening files
#[derive(Clone, Copy)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub truncate: bool,
    pub append: bool,
}

/// Directory entry
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<Timestamp>,
}

/// File metadata
pub struct Stat {
    pub size: u64,
    pub is_dir: bool,
    pub created: Option<Timestamp>,
    pub modified: Option<Timestamp>,
    pub permissions: Permissions,
}
```

### FileHandle

Returned by `open()`, represents an open file:

```rust
/// An open file handle
pub trait FileHandle: Send + Sync {
    /// Read bytes from current position
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    /// Write bytes at current position
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    /// Seek to position
    fn seek(&mut self, pos: SeekFrom) -> Result<u64>;

    /// Get current position
    fn position(&self) -> u64;

    /// Flush any buffered writes
    fn flush(&mut self) -> Result<()>;

    /// Close the handle (called on drop, but explicit is cleaner)
    fn close(&mut self) -> Result<()>;
}

/// Seek position
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}
```

---

## Namespace

The namespace manages mount points and routes paths to services:

```rust
/// Agent's view of mounted capabilities
pub struct Namespace {
    /// Mounts ordered by path length (longest first for matching)
    mounts: Vec<Mount>,
}

struct Mount {
    path: String,           // e.g., "/wallet"
    service: Arc<dyn FileService>,
    access: AccessLevel,
}

impl Namespace {
    /// Mount a service at a path
    pub fn mount(&mut self, path: &str, service: Arc<dyn FileService>, access: AccessLevel) {
        self.mounts.push(Mount { path: path.to_string(), service, access });
        // Sort by path length descending for longest-prefix matching
        self.mounts.sort_by(|a, b| b.path.len().cmp(&a.path.len()));
    }

    /// Resolve a path to its service and relative path
    pub fn resolve(&self, path: &str) -> Option<(&dyn FileService, &str, AccessLevel)> {
        for mount in &self.mounts {
            if path.starts_with(&mount.path) {
                let relative = &path[mount.path.len()..];
                let relative = if relative.starts_with('/') { &relative[1..] } else { relative };
                return Some((mount.service.as_ref(), relative, mount.access));
            }
        }
        None
    }

    /// Unmount a path
    pub fn unmount(&mut self, path: &str) {
        self.mounts.retain(|m| m.path != path);
    }

    /// List all mount points
    pub fn mounts(&self) -> Vec<&str> {
        self.mounts.iter().map(|m| m.path.as_str()).collect()
    }
}

/// Access level for mounted services (canonical definition in TRAITS.md)
#[derive(Clone)]
pub enum AccessLevel {
    /// Read-only access
    ReadOnly,
    /// Read and write access
    ReadWrite,
    /// Sign-only (for /identity; key ops allowed, private keys never exposed)
    SignOnly,
    /// Budgeted access with spending limits
    Budgeted(BudgetPolicy),
    /// Disabled (mount exists but access denied)
    Disabled,
}

/// Static budget policy (canonical definition in TRAITS.md)
pub struct BudgetPolicy {
    pub per_tick_usd: u64,
    pub per_day_usd: u64,
    pub approval_threshold_usd: u64,
    pub approvers: Vec<PublicKey>,
}

// Note: BudgetState (reserved/spent counters) is tracked by runtime,
// not stored in AccessLevel. See TRAITS.md for BudgetState definition.
```

### Longest-Prefix Matching

Path resolution uses longest-prefix matching:

```
Mounts: /tools/core, /tools, /wallet/lightning, /wallet

Path: /tools/core/search → matches /tools/core, relative: search
Path: /tools/user/custom → matches /tools, relative: user/custom
Path: /wallet/lightning/pay → matches /wallet/lightning, relative: pay
Path: /wallet/balance → matches /wallet, relative: balance
```

This allows both broad and specific mounts to coexist.

---

## Standard Services

### StatusFs

Agent status as a file:

```rust
pub struct StatusFs {
    agent_id: AgentId,
    state: Arc<RwLock<AgentState>>,
}

impl FileService for StatusFs {
    fn name(&self) -> &str { "status" }

    fn open(&self, path: &str, _flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        match path {
            "" | "status" => {
                let state = self.state.read();
                let json = serde_json::to_string_pretty(&*state)?;
                Ok(Box::new(StringHandle::new(json)))
            }
            _ => Err(Error::NotFound),
        }
    }

    fn stat(&self, path: &str) -> Result<Stat> {
        match path {
            "" => Ok(Stat::dir()),
            "status" => Ok(Stat::file(self.state.read().to_json().len() as u64)),
            _ => Err(Error::NotFound),
        }
    }

    // ... other methods
}
```

### InboxFs

Message inbox as a directory:

```rust
pub struct InboxFs {
    queue: Arc<Mutex<VecDeque<Envelope>>>,
}

impl FileService for InboxFs {
    fn name(&self) -> &str { "inbox" }

    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        if flags.write {
            // Writing to inbox = enqueue message
            Ok(Box::new(InboxWriter { queue: self.queue.clone() }))
        } else {
            // Reading = peek at messages
            let queue = self.queue.lock();
            let json = serde_json::to_string_pretty(&*queue)?;
            Ok(Box::new(StringHandle::new(json)))
        }
    }

    fn readdir(&self, _path: &str) -> Result<Vec<DirEntry>> {
        let queue = self.queue.lock();
        Ok(queue.iter().enumerate().map(|(i, env)| {
            DirEntry {
                name: format!("{}.json", i),
                is_dir: false,
                size: env.to_json().len() as u64,
                modified: Some(env.timestamp),
            }
        }).collect())
    }
}

struct InboxWriter {
    queue: Arc<Mutex<VecDeque<Envelope>>>,
}

impl FileHandle for InboxWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let envelope: Envelope = serde_json::from_slice(buf)?;
        self.queue.lock().push_back(envelope);
        Ok(buf.len())
    }
    // ... other methods return errors (write-only handle)
}
```

### GoalsFs

Goals as individual files:

```rust
pub struct GoalsFs {
    storage: Arc<dyn AgentStorage>,
    agent_id: AgentId,
}

impl FileService for GoalsFs {
    fn name(&self) -> &str { "goals" }

    fn readdir(&self, _path: &str) -> Result<Vec<DirEntry>> {
        let goals = self.storage.list_goals(&self.agent_id)?;
        Ok(goals.iter().map(|g| DirEntry {
            name: format!("{}.json", g.id),
            is_dir: false,
            size: g.to_json().len() as u64,
            modified: Some(g.updated_at),
        }).collect())
    }

    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        if flags.create {
            // Create new goal
            Ok(Box::new(GoalWriter { storage: self.storage.clone(), agent_id: self.agent_id.clone() }))
        } else {
            // Read existing goal
            let goal_id = path.trim_end_matches(".json");
            let goal = self.storage.get_goal(&self.agent_id, goal_id)?;
            Ok(Box::new(StringHandle::new(goal.to_json())))
        }
    }
}
```

### IdentityFs

Cryptographic operations as files:

```rust
pub struct IdentityFs {
    signer: Arc<dyn SigningService>,
    agent_id: AgentId,
}

impl FileService for IdentityFs {
    fn name(&self) -> &str { "identity" }

    fn readdir(&self, _path: &str) -> Result<Vec<DirEntry>> {
        Ok(vec![
            DirEntry::file("pubkey"),
            DirEntry::file("sign"),
            DirEntry::file("verify"),
            DirEntry::file("encrypt"),
            DirEntry::file("decrypt"),
        ])
    }

    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        match path {
            "pubkey" => {
                let pubkey = self.signer.pubkey(&self.agent_id)?;
                Ok(Box::new(StringHandle::new(pubkey.to_hex())))
            }
            "sign" => Ok(Box::new(SignHandle::new(self.signer.clone(), self.agent_id.clone()))),
            "verify" => Ok(Box::new(VerifyHandle::new(self.signer.clone()))),
            "encrypt" => Ok(Box::new(EncryptHandle::new(self.signer.clone(), self.agent_id.clone()))),
            "decrypt" => Ok(Box::new(DecryptHandle::new(self.signer.clone(), self.agent_id.clone()))),
            _ => Err(Error::NotFound),
        }
    }
}

/// Write data → read signature
struct SignHandle {
    signer: Arc<dyn SigningService>,
    agent_id: AgentId,
    data: Vec<u8>,
    signature: Option<Vec<u8>>,
}

impl FileHandle for SignHandle {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.signature.is_none() {
            let sig = self.signer.sign(&self.agent_id, &self.data)?;
            self.signature = Some(sig.to_bytes());
        }
        let sig = self.signature.as_ref().unwrap();
        let len = std::cmp::min(buf.len(), sig.len());
        buf[..len].copy_from_slice(&sig[..len]);
        Ok(len)
    }
}
```

### WalletFs

Payment operations as files:

```rust
pub struct WalletFs {
    wallet: Arc<dyn WalletService>,
    budget: Budget,
}

impl FileService for WalletFs {
    fn name(&self) -> &str { "wallet" }

    fn readdir(&self, _path: &str) -> Result<Vec<DirEntry>> {
        Ok(vec![
            DirEntry::file("balance"),
            DirEntry::file("invoice"),
            DirEntry::file("pay"),
            DirEntry::dir("history"),
        ])
    }

    fn open(&self, path: &str, _flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        match path {
            "balance" => {
                let balance = self.wallet.balance()?;
                let json = serde_json::json!({
                    "balance_usd": balance,  // micro-USD
                    "budget": {
                        "daily_limit_usd": self.budget.daily_limit,
                        "daily_spent_usd": self.budget.daily_spent,
                        "remaining_usd": self.budget.daily_limit - self.budget.daily_spent,
                    }
                });
                Ok(Box::new(StringHandle::new(json.to_string())))
            }
            "invoice" => Ok(Box::new(InvoiceHandle::new(self.wallet.clone()))),
            "pay" => Ok(Box::new(PayHandle::new(self.wallet.clone(), self.budget.clone()))),
            _ => Err(Error::NotFound),
        }
    }
}
```

### LogsFs

Streaming logs as files:

```rust
pub struct LogsFs {
    logs: Arc<RwLock<RingBuffer<LogEntry>>>,
    traces: broadcast::Sender<TraceEvent>,
}

impl FileService for LogsFs {
    fn name(&self) -> &str { "logs" }

    fn open(&self, path: &str, _flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        match path {
            "trace" => {
                // Streaming handle - reads block until new events
                let rx = self.traces.subscribe();
                Ok(Box::new(StreamHandle::new(rx)))
            }
            "recent" => {
                let logs = self.logs.read();
                let json = serde_json::to_string_pretty(&logs.as_slice())?;
                Ok(Box::new(StringHandle::new(json)))
            }
            _ => Err(Error::NotFound),
        }
    }
}
```

---

## Executor Bridge

Services are synchronous (for simplicity), but network I/O is async. The executor bridges this:

```rust
/// Bridges sync FileService to async network operations
pub struct ExecutorManager {
    /// Runtime handle for spawning async tasks
    runtime: Handle,

    /// Pending async operations
    pending: Arc<Mutex<HashMap<OperationId, PendingOp>>>,
}

impl ExecutorManager {
    /// Execute an async operation from sync context
    pub fn execute<F, T>(&self, op: F) -> Result<T>
    where
        F: Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        // Block on the async operation (safe because we're in a dedicated thread)
        self.runtime.block_on(op)
    }

    /// Start an async operation, return handle for polling
    pub fn spawn<F, T>(&self, op: F) -> OperationHandle<T>
    where
        F: Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        let id = OperationId::new();
        let (tx, rx) = oneshot::channel();

        self.runtime.spawn(async move {
            let result = op.await;
            let _ = tx.send(result);
        });

        OperationHandle { rx }
    }
}

/// Used by services that need network access
pub struct NostrFs {
    relays: Vec<String>,
    executor: Arc<ExecutorManager>,
    client: Arc<NostrClient>,
}

impl FileService for NostrFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        match path {
            "publish" => Ok(Box::new(PublishHandle::new(
                self.client.clone(),
                self.executor.clone(),
            ))),
            _ => Err(Error::NotFound),
        }
    }
}

struct PublishHandle {
    client: Arc<NostrClient>,
    executor: Arc<ExecutorManager>,
    event_json: Vec<u8>,
}

impl FileHandle for PublishHandle {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.event_json.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        let event: NostrEvent = serde_json::from_slice(&self.event_json)?;
        // Use executor to run async publish
        self.executor.execute(async {
            self.client.publish(event).await
        })
    }
}
```

---

## Agent Environment

The complete environment an agent sees:

```rust
/// Complete agent environment
pub struct AgentEnv {
    /// Agent identity
    pub id: AgentId,

    /// Mounted capabilities
    pub namespace: Namespace,

    /// Configuration
    pub config: AgentConfig,

    /// Current tick context
    pub tick: Option<TickContext>,
}

impl AgentEnv {
    /// Create environment for an agent
    pub fn new(id: AgentId, config: AgentConfig) -> Self {
        let mut namespace = Namespace::new();

        // Mount standard services
        namespace.mount("/status", Arc::new(StatusFs::new(&id)), AccessLevel::ReadOnly);
        namespace.mount("/inbox", Arc::new(InboxFs::new(&id)), AccessLevel::ReadWrite);
        namespace.mount("/goals", Arc::new(GoalsFs::new(&id)), AccessLevel::ReadWrite);
        namespace.mount("/identity", Arc::new(IdentityFs::new(&id)), AccessLevel::SignOnly);
        namespace.mount("/logs", Arc::new(LogsFs::new(&id)), AccessLevel::ReadOnly);

        // Mount configured capabilities
        for (path, mount_config) in &config.mounts {
            let service = create_service(mount_config);
            namespace.mount(path, service, mount_config.access);
        }

        Self {
            id,
            namespace,
            config,
            tick: None,
        }
    }

    /// File operations
    pub fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        let (service, relative, access) = self.namespace.resolve(path)
            .ok_or(Error::NotFound)?;

        // Check access level - mount table is the security boundary
        match access {
            AccessLevel::Disabled => {
                return Err(Error::PermissionDenied);  // Mount exists but access denied
            }
            AccessLevel::ReadOnly if flags.write => {
                return Err(Error::PermissionDenied);
            }
            AccessLevel::SignOnly if flags.write || flags.read => {
                // SignOnly: only sign/verify operations allowed (via special paths)
                if !relative.starts_with("sign") && !relative.starts_with("verify") {
                    return Err(Error::PermissionDenied);
                }
            }
            _ => {}  // ReadWrite and Budgeted allow access (budget checked by service)
        }

        service.open(relative, flags)
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let mut handle = self.open(path, OpenFlags::read())?;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = handle.read(&mut chunk)?;
            if n == 0 { break; }
            buf.extend_from_slice(&chunk[..n]);
        }
        Ok(buf)
    }

    pub fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let mut handle = self.open(path, OpenFlags::write())?;
        handle.write(data)?;
        handle.flush()?;
        Ok(())
    }
}
```

---

## WASI Compatibility

The filesystem abstraction maps cleanly to WASI:

```rust
/// WASI host implementation using our FileService
pub struct WasiHost {
    env: Arc<AgentEnv>,
}

impl wasi::WasiSnapshotPreview1 for WasiHost {
    fn path_open(
        &mut self,
        dirfd: Fd,
        dirflags: Lookupflags,
        path: &str,
        oflags: Oflags,
        fs_rights_base: Rights,
        fs_rights_inheriting: Rights,
        fdflags: Fdflags,
    ) -> Result<Fd, Errno> {
        let full_path = self.resolve_path(dirfd, path)?;
        let flags = OpenFlags::from_wasi(oflags, fs_rights_base);

        match self.env.open(&full_path, flags) {
            Ok(handle) => {
                let fd = self.allocate_fd(handle);
                Ok(fd)
            }
            Err(Error::NotFound) => Err(Errno::Noent),
            Err(Error::PermissionDenied) => Err(Errno::Acces),
            Err(_) => Err(Errno::Io),
        }
    }

    fn fd_read(&mut self, fd: Fd, iovs: &[Iovec]) -> Result<Size, Errno> {
        let handle = self.get_handle(fd)?;
        // ... read into iovecs
    }

    // ... other WASI functions
}
```

This enables:
- **Same agent binary** runs on server (native), desktop (native), browser (WASM)
- **Sandboxed execution** — agent can only access mounted capabilities
- **Portable skills** — WASM modules work everywhere

---

## Implementation Notes

### From OANIX

Key lessons from OANIX experimentation:

1. **Keep FileService sync** — Async makes the trait complex; use executor bridge instead
2. **Longest-prefix matching is essential** — Allows both `/tools` and `/tools/core` mounts
3. **StringHandle covers 80% of cases** — Most reads return JSON strings
4. **Write-then-read pattern** — For operations like sign: write data, read result
5. **Budget enforcement at mount level** — Cleaner than per-operation checks

### Sync vs Async Reality (Critical for Browser/WASM)

`FileService` is **sync by design** for simplicity. However, `ExecutorManager.block_on()` cannot exist in browser/WASM (no threads to block).

**The Official Story:**

1. **Sync FileService is the universal interface** — All backends implement it
2. **Buffer + Flush is the WASM pattern** — For network operations in browser
3. **AsyncFileService is NOT used** — Adds complexity, breaks uniformity

**Backend-specific async handling:**

| Backend | Async Strategy |
|---------|----------------|
| Native (Local/Server) | `ExecutorManager.block_on()` — sync call blocks on async work |
| Cloudflare Workers | Single-threaded event loop — naturally async internally |
| Browser WASM | **Buffer + Flush + Callback** (see below) |

**The Buffer + Flush Pattern (Canonical for WASM)**

For WASM targets, network operations buffer synchronously and execute asynchronously on flush:

```rust
#[cfg(target_arch = "wasm32")]
impl FileHandle for PublishHandle {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        // Buffer synchronously — never blocks
        self.event_json.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        // Queue async operation, return immediately
        // Actual publish happens via JS callback
        let event: NostrEvent = serde_json::from_slice(&self.event_json)?;
        self.queue_publish(event);  // Non-blocking, queues to JS event loop
        Ok(())
    }
}
```

**Completion notification** comes via:
- Watch on `/nostr/events` for published event confirmation
- Watch on `/logs/trace` for operation result
- Callback registered in service initialization

**Capability availability:**

Some services cannot work in browser:
- `/compute/exec` — No arbitrary code execution
- `/secrets/raw` — No HSM access

Document unavailable capabilities in mount manifest:

```json
{
  "required_capabilities": ["/nostr", "/wallet"],
  "optional_capabilities": ["/compute"],
  "unavailable_on": {
    "browser": ["/compute/exec", "/secrets/raw"]
  }
}
```

**Key principle:** FileService stays sync everywhere. Browser backend uses buffer+flush+callback. Services that can't work this way are unavailable on browser.

### Thread Safety

Services are `Send + Sync` but individual handles are `Send` only:
- Services can be shared across threads (namespace lookup)
- Handles have mutable state (position, buffers) so not `Sync`

### Error Mapping

Consistent error types across all services:

```rust
pub enum Error {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    NotDirectory,
    IsDirectory,
    InvalidPath,
    BudgetExceeded,
    Io(std::io::Error),
    Other(String),
}
```
