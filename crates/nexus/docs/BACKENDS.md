# Nexus Backend Adapters

Nexus supports multiple deployment backends. Cloudflare Workers is the initial target, but the architecture is designed to support native Rust deployments as well.

## Design Principles

1. **Core logic is backend-agnostic** — Protocol handling, NIP implementations, and business logic live in shared code
2. **Adapters for infrastructure** — Storage, WebSockets, and caching are abstracted behind traits
3. **Feature flags for backends** — Compile with `--features cloudflare` or `--features native`

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         NEXUS CORE                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   NIP-01    │  │   NIP-42    │  │   NIP-89/90 (DVM)       │ │
│  │  Protocol   │  │    Auth     │  │   Job Marketplace       │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Event Validation & Routing                  │   │
│  └─────────────────────────────────────────────────────────┘   │
└───────────────────────────┬─────────────────────────────────────┘
                            │
              ┌─────────────┴─────────────┐
              │      Adapter Traits       │
              │  ┌───────┐ ┌───────┐     │
              │  │Storage│ │  WS   │     │
              │  └───────┘ └───────┘     │
              │  ┌───────┐ ┌───────┐     │
              │  │ Cache │ │Runtime│     │
              │  └───────┘ └───────┘     │
              └─────────────┬─────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│  Cloudflare   │   │    Native     │   │    Future     │
│   Workers     │   │     Rust      │   │   (Fly.io,    │
│               │   │               │   │    Railway)   │
│ - D1 Database │   │ - SQLite      │   │               │
│ - DO Storage  │   │ - Redis       │   │               │
│ - WS Hibern.  │   │ - Tokio WS    │   │               │
└───────────────┘   └───────────────┘   └───────────────┘
```

---

## Adapter Traits

### StorageAdapter

Handles persistent event storage (events table).

```rust
#[async_trait]
pub trait StorageAdapter: Send + Sync {
    /// Store an event
    async fn store_event(&self, event: &Event) -> Result<(), StorageError>;

    /// Query events by filter
    async fn query_events(&self, filter: &Filter) -> Result<Vec<Event>, StorageError>;

    /// Delete an event by ID
    async fn delete_event(&self, id: &str) -> Result<bool, StorageError>;

    /// Get event by ID
    async fn get_event(&self, id: &str) -> Result<Option<Event>, StorageError>;

    /// Check if event exists
    async fn event_exists(&self, id: &str) -> Result<bool, StorageError>;
}
```

**Implementations:**
- `D1Storage` — Cloudflare D1 (SQLite-compatible)
- `SqliteStorage` — Native SQLite via rusqlite
- `PostgresStorage` — PostgreSQL for horizontal scaling (future)

### CacheAdapter

Handles hot cache for recent events and active subscriptions.

```rust
#[async_trait]
pub trait CacheAdapter: Send + Sync {
    /// Cache an event (with TTL)
    async fn cache_event(&self, event: &Event, ttl_secs: u64) -> Result<(), CacheError>;

    /// Get cached event
    async fn get_cached_event(&self, id: &str) -> Result<Option<Event>, CacheError>;

    /// Store subscription filters
    async fn store_subscription(
        &self,
        conn_id: &str,
        sub_id: &str,
        filters: &[Filter],
    ) -> Result<(), CacheError>;

    /// Get subscriptions for a connection
    async fn get_subscriptions(&self, conn_id: &str) -> Result<Vec<(String, Vec<Filter>)>, CacheError>;

    /// Remove subscription
    async fn remove_subscription(&self, conn_id: &str, sub_id: &str) -> Result<(), CacheError>;

    /// Get all active subscriptions (for broadcast)
    async fn get_all_subscriptions(&self) -> Result<Vec<(String, String, Vec<Filter>)>, CacheError>;
}
```

**Implementations:**
- `DOCache` — Cloudflare Durable Object transactional storage
- `InMemoryCache` — HashMap-based (single instance)
- `RedisCache` — Redis for distributed deployments (future)

### WebSocketAdapter

Handles WebSocket connections and message passing.

```rust
#[async_trait]
pub trait WebSocketAdapter: Send + Sync {
    /// Send message to a connection
    async fn send(&self, conn_id: &str, message: &str) -> Result<(), WsError>;

    /// Broadcast to all connections matching filter
    async fn broadcast(&self, message: &str, filter: impl Fn(&str) -> bool) -> Result<usize, WsError>;

    /// Close a connection
    async fn close(&self, conn_id: &str) -> Result<(), WsError>;

    /// Get connection metadata
    async fn get_meta(&self, conn_id: &str) -> Result<Option<ConnectionMeta>, WsError>;

    /// Set connection metadata
    async fn set_meta(&self, conn_id: &str, meta: ConnectionMeta) -> Result<(), WsError>;
}

pub struct ConnectionMeta {
    pub pubkey: Option<String>,
    pub challenge: String,
    pub authenticated: bool,
    pub subscriptions: Vec<String>,
}
```

**Implementations:**
- `CFWebSocket` — Cloudflare Workers WebSocket with hibernation
- `TokioWebSocket` — tokio-tungstenite for native

### RuntimeAdapter

Handles runtime-specific concerns (crypto, randomness, time).

```rust
pub trait RuntimeAdapter: Send + Sync {
    /// Generate random bytes
    fn random_bytes(&self, len: usize) -> Vec<u8>;

    /// Current Unix timestamp
    fn now(&self) -> u64;

    /// Verify event signature
    fn verify_signature(&self, event: &Event) -> Result<bool, CryptoError>;

    /// Log message (adapts to console.log vs tracing)
    fn log(&self, level: LogLevel, message: &str);
}
```

**Implementations:**
- `CFRuntime` — Uses `getrandom` (js feature), `js_sys::Date`, `console_log!`
- `NativeRuntime` — Uses `rand`, `std::time`, `tracing`

---

## Backend: Cloudflare Workers

### Components Used

| Component | Purpose | Config |
|-----------|---------|--------|
| **Workers** | Request routing, NIP-11 | `wrangler.toml` |
| **Durable Objects** | WebSocket state, hot cache | `[[durable_objects.bindings]]` |
| **D1** | Event persistence | `[[d1_databases]]` |
| **Secrets** | Auth keys, admin pubkeys | `wrangler secret put` |

### File Structure

```
crates/nexus/
├── src/
│   ├── lib.rs              # Core Nexus logic
│   ├── adapters/
│   │   ├── mod.rs
│   │   ├── cloudflare/
│   │   │   ├── mod.rs
│   │   │   ├── storage.rs  # D1Storage
│   │   │   ├── cache.rs    # DOCache
│   │   │   ├── websocket.rs # CFWebSocket
│   │   │   └── runtime.rs  # CFRuntime
│   │   └── native/
│   │       ├── mod.rs
│   │       ├── storage.rs  # SqliteStorage
│   │       ├── cache.rs    # InMemoryCache
│   │       └── websocket.rs # TokioWebSocket
│   ├── protocol/
│   │   ├── nip01.rs
│   │   ├── nip42.rs
│   │   ├── nip89.rs
│   │   └── nip90.rs
│   └── relay.rs            # Main relay logic
├── worker/                  # CF Workers entry point
│   ├── src/
│   │   ├── lib.rs          # Worker main + DO
│   │   └── durable_object.rs
│   ├── wrangler.toml
│   └── schema.sql
└── native/                  # Native binary entry point
    └── src/
        └── main.rs
```

### Build & Deploy

```bash
# Cloudflare Workers
cd crates/nexus/worker
bun run build && npx wrangler deploy

# With environment
npx wrangler deploy --env preview
```

### Configuration

**wrangler.toml:**
```toml
name = "nexus"
main = "build/index.js"
compatibility_date = "2024-01-01"

[vars]
RELAY_URL = "wss://nexus.openagents.com"
AUTH_REQUIRED = "true"

[[d1_databases]]
binding = "DB"
database_name = "nexus"
database_id = "xxx"

[[durable_objects.bindings]]
name = "NEXUS_RELAY"
class_name = "NexusRelay"

[[migrations]]
tag = "v1"
new_classes = ["NexusRelay"]
```

---

## Backend: Native Rust

### Components

| Component | Purpose | Crate |
|-----------|---------|-------|
| **HTTP/WS Server** | Connections | `axum` + `tokio-tungstenite` |
| **SQLite** | Event persistence | `rusqlite` or `sqlx` |
| **In-Memory Cache** | Hot cache | `dashmap` |
| **TLS** | HTTPS | `rustls` |

### Build & Run

```bash
# Build native binary
cargo build --release -p nexus --features native

# Run
./target/release/nexus --config nexus.toml
```

### Configuration

**nexus.toml:**
```toml
[server]
bind = "0.0.0.0:443"
relay_url = "wss://nexus.openagents.com"

[storage]
type = "sqlite"
path = "./data/nexus.db"

[cache]
type = "memory"
max_events = 10000
ttl_secs = 300

[auth]
required = true

[tls]
cert = "/etc/letsencrypt/live/nexus.openagents.com/fullchain.pem"
key = "/etc/letsencrypt/live/nexus.openagents.com/privkey.pem"
```

---

## Migration Path

### Phase 1: Cloudflare (v0.1)
- Deploy to CF Workers with existing relay-worker patterns
- D1 for storage, DO for cache/state
- Single region, simple scaling

### Phase 2: Multi-Backend (v0.2)
- Extract adapters into traits
- Add native backend option
- Support self-hosted deployments

### Phase 3: Distributed (v0.3)
- PostgreSQL storage adapter
- Redis cache adapter
- Multi-region deployment
- Event replication between instances

---

## Existing Code to Reuse

From `crates/relay-worker/`:

| File | Reuse | Notes |
|------|-------|-------|
| `nip01.rs` | ✅ Full | ClientMessage/RelayMessage parsing |
| `nip42.rs` | ✅ Full | Auth challenge/validation |
| `nip90.rs` | ✅ Full | DVM kind helpers |
| `subscription.rs` | ✅ Full | Filter matching |
| `storage.rs` | 🔄 Adapt | Extract D1-specific into adapter |
| `relay_do.rs` | 🔄 Adapt | Extract DO-specific into adapter |
| `lib.rs` | 🔄 Adapt | Extract routing logic |

From `crates/nostr/core/`:

| Module | Reuse | Notes |
|--------|-------|-------|
| `nip42.rs` | ✅ Full | `create_auth_event_template`, `validate_auth_event` |
| `nip01.rs` | ✅ Full | Event, EventTemplate, finalize_event |

---

## Testing Strategy

### Unit Tests
- Protocol parsing (NIP-01 messages)
- Filter matching
- Auth validation
- Each adapter implementation

### Integration Tests
- Full flow with mock adapters
- Storage round-trip
- Subscription broadcast

### Backend-Specific Tests
- CF Workers: `wrangler dev` + local D1
- Native: In-process SQLite + test WebSocket client

```bash
# Run all tests
cargo test -p nexus

# Run with specific backend
cargo test -p nexus --features cloudflare
cargo test -p nexus --features native
```
