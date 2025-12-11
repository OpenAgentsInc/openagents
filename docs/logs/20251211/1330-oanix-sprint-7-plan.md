# OANIX Sprint 7: External Executors & Integration

## Overview

Add network executors that bridge OANIX capability services (HttpFs, WsFs, NostrFs) to actual I/O. Currently these services queue requests but perform no network operations.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
└──────────────────────────┬──────────────────────────────────┘
                           │
       ┌───────────────────┴───────────────────┐
       │                                       │
       ▼                                       ▼
┌──────────────┐                     ┌──────────────────────────┐
│   OANIX      │◄──── Arc<*Fs> ────►│    ExecutorManager       │
│  (Sync)      │                     │       (Async/Tokio)      │
│              │                     │                          │
│ HttpFs       │                     │  HttpExecutor            │
│ WsFs         │                     │  WsConnector             │
│ NostrFs      │                     │  NostrRelayConnector     │
└──────────────┘                     └──────────────────────────┘
```

**Key Decision**: ExecutorManager runs on tokio runtime, shares `Arc<*Fs>` with sync OANIX. Executors poll queues and perform actual I/O.

## Module Structure

```
crates/oanix/src/
├── executor/                    # NEW
│   ├── mod.rs                   # ExecutorManager, exports
│   ├── config.rs                # ExecutorConfig, RetryPolicy
│   ├── http.rs                  # HttpExecutor (reqwest)
│   ├── ws.rs                    # WsConnector (tokio-tungstenite)
│   ├── nostr.rs                 # NostrRelayConnector (NIP-01)
│   └── error.rs                 # ExecutorError
```

## Implementation Phases

### Phase 1: Foundation
**Files to create:**
- `crates/oanix/src/executor/mod.rs`
- `crates/oanix/src/executor/config.rs`
- `crates/oanix/src/executor/error.rs`

**Changes:**
- `crates/oanix/Cargo.toml` - Add `net-executor` feature with deps
- `crates/oanix/src/lib.rs` - Add conditional `pub mod executor;`

**Key types:**
```rust
pub struct ExecutorConfig {
    pub http_timeout: Duration,        // default: 30s
    pub http_retry: RetryPolicy,
    pub ws_ping_interval: Duration,    // default: 30s
    pub ws_reconnect: RetryPolicy,
}

pub struct ExecutorManager {
    runtime: tokio::runtime::Runtime,
    shutdown_tx: broadcast::Sender<()>,
    // handles for each executor task
}

impl ExecutorManager {
    pub fn new(config: ExecutorConfig) -> Result<Self, ExecutorError>;
    pub fn attach_http(&mut self, http_fs: Arc<HttpFs>);
    pub fn attach_ws(&mut self, ws_fs: Arc<WsFs>);
    pub fn attach_nostr(&mut self, nostr_fs: Arc<NostrFs>);
    pub fn start(&mut self) -> Result<(), ExecutorError>;
    pub fn shutdown(self) -> Result<(), ExecutorError>;
}
```

### Phase 2: HttpExecutor
**Files to create:**
- `crates/oanix/src/executor/http.rs`

**Files to modify:**
- `crates/oanix/src/services/http_fs.rs` - Add helper methods

**Implementation:**
```rust
struct HttpExecutor {
    http_fs: Arc<HttpFs>,
    client: reqwest::Client,
    config: ExecutorConfig,
}

// Main loop: poll pending -> execute -> complete/fail
async fn run(&mut self) {
    loop {
        select! {
            _ = shutdown => break,
            _ = sleep(50ms) => self.process_pending().await,
        }
    }
}
```

**HttpFs additions:**
- `has_pending() -> bool`
- `take_pending_batch(limit: usize) -> Vec<HttpRequest>`

### Phase 3: WsConnector
**Files to create:**
- `crates/oanix/src/executor/ws.rs`

**Files to modify:**
- `crates/oanix/src/services/ws_fs.rs` - Add helper methods

**Implementation:**
```rust
struct WsConnector {
    ws_fs: Arc<WsFs>,
    connections: HashMap<String, WsConnectionHandle>,
}

struct WsConnectionHandle {
    sink: SplitSink<WebSocketStream, Message>,
    outbox_task: JoinHandle<()>,
    inbox_task: JoinHandle<()>,
}
```

**WsFs additions:**
- `connecting_connections() -> Vec<(String, String)>` - (id, url) pairs
- `closing_connections() -> Vec<String>`
- `peek_outbox(id) -> Vec<Vec<u8>>`

### Phase 4: NostrRelayConnector
**Files to create:**
- `crates/oanix/src/executor/nostr.rs`

**Files to modify:**
- `crates/oanix/src/services/nostr_fs.rs` - Add subscription API

**Implementation:**
- Uses WsConnector internally for relay connections
- Implements NIP-01 protocol (EVENT, REQ, CLOSE, EOSE)
- Routes outbox events to relays, inbox events from relays

**NostrFs additions:**
```rust
pub struct Filter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<u16>>,
    pub e_tags: Option<Vec<String>>,
    pub p_tags: Option<Vec<String>>,
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub limit: Option<u64>,
}

impl NostrFs {
    pub fn add_subscription(&self, sub_id: String, filters: Vec<Filter>);
    pub fn subscriptions(&self) -> Vec<(String, Vec<Filter>)>;
    pub fn remove_subscription(&self, sub_id: &str);
    pub fn mark_sent(&self, event_id: &str, relay: &str);
}
```

### Phase 5: Integration & Tests
- End-to-end tests with all executors
- Mock servers (wiremock for HTTP, echo server for WS, mock relay for Nostr)
- Update README with executor usage examples
- Update `crates/oanix/docs/ROADMAP.md`

## Cargo.toml Changes

```toml
[features]
net-executor = ["dep:reqwest", "dep:tokio-tungstenite", "dep:futures-util"]

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
reqwest = { version = "0.12", features = ["json"], optional = true }
tokio-tungstenite = { version = "0.24", features = ["native-tls"], optional = true }
futures-util = { version = "0.3", optional = true }

[dev-dependencies]
wiremock = "0.6"
```

## Critical Files

| File | Action |
|------|--------|
| `crates/oanix/Cargo.toml` | Add feature + deps |
| `crates/oanix/src/lib.rs` | Add `pub mod executor` |
| `crates/oanix/src/executor/mod.rs` | Create ExecutorManager |
| `crates/oanix/src/executor/config.rs` | Create config types |
| `crates/oanix/src/executor/error.rs` | Create error types |
| `crates/oanix/src/executor/http.rs` | Create HttpExecutor |
| `crates/oanix/src/executor/ws.rs` | Create WsConnector |
| `crates/oanix/src/executor/nostr.rs` | Create NostrRelayConnector |
| `crates/oanix/src/services/http_fs.rs` | Add helper methods |
| `crates/oanix/src/services/ws_fs.rs` | Add helper methods |
| `crates/oanix/src/services/nostr_fs.rs` | Add Filter + subscription API |
| `crates/oanix/docs/ROADMAP.md` | Mark Sprint 7 complete |

## Example Usage

```rust
use oanix::{HttpFs, WsFs, NostrFs, executor::{ExecutorManager, ExecutorConfig}};
use std::sync::Arc;

let http_fs = Arc::new(HttpFs::new());
let ws_fs = Arc::new(WsFs::new());
let nostr_fs = Arc::new(NostrFs::generate()?);
nostr_fs.add_relay("wss://relay.damus.io");

let mut executor = ExecutorManager::new(ExecutorConfig::default())?;
executor.attach_http(Arc::clone(&http_fs));
executor.attach_ws(Arc::clone(&ws_fs));
executor.attach_nostr(Arc::clone(&nostr_fs));
executor.start()?;

// Now agents writing to /cap/http/request get real HTTP responses
// Agents writing to /cap/ws/control get real WebSocket connections
// Agents writing to /cap/nostr/submit get events sent to relays

executor.shutdown()?;
```

## Test Strategy

1. **Unit tests**: Mock reqwest/tungstenite, test executor logic
2. **Integration tests**: wiremock for HTTP, echo server for WS, mock relay for Nostr
3. **E2E tests**: Full flow from FileService write to network response

## WANIX Patterns Applied

From `~/code/wanix`:
- **Bidirectional pipes** (vfs/pipe/): Used for WsConnector bridge tasks
- **Resource allocation** (task/service.go): Dynamic pipe creation pattern
- **WebSocket adapter** (cmd/wanix/serve.go): Length-prefix framing, goroutine bridging
- **Capability registration** (cap/service.go): Allocator/Mounter pattern

