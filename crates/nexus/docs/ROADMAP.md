# Nexus v0.1 Implementation Roadmap

Step-by-step guide to building Nexus, the Nostr relay for the OpenAgents compute marketplace.

## What We're Building

Nexus v0.1 is a **Nostr relay** optimized for:
- NIP-90 job marketplace (kind 5xxx/6xxx/7000)
- NIP-89 handler discovery (kind 31990)
- NIP-42 agent authentication

**Not** a hosted agent runtime. That's a separate product. Nexus is infrastructure — the transport layer that connects Pylons.

---

## Current Status (2026-01-07)

- Cloudflare worker for Nexus is deployed and serving `nexus.openagents.com`.
- Phase 0-6 completed for the v0.1 worker path; adapter extraction into core traits is deferred.
- WebSocket AUTH challenge verified on the custom domain.
- End-to-end Pylon auth/subscribe and broadcast tests are still pending.

---

## Code Reuse Strategy

We have a working relay at `crates/relay-worker/`. Most code transfers directly to Nexus.

### Direct Copy (Zero Changes)

| Source | Target | Lines | Purpose |
|--------|--------|-------|---------|
| `relay-worker/src/nip01.rs` | `nexus/src/protocol/nip01.rs` | 178 | ClientMessage, RelayMessage parsing |
| `relay-worker/src/nip28.rs` | `nexus/src/protocol/nip28.rs` | 54 | Channel kind helpers |
| `relay-worker/src/nip32.rs` | `nexus/src/protocol/nip32.rs` | 112 | Labels, reputation scoring |
| `relay-worker/src/nip42.rs` | `nexus/src/protocol/nip42.rs` | 145 | Auth challenge/validation |
| `relay-worker/src/nip90.rs` | `nexus/src/protocol/nip90.rs` | 93 | DVM job kind helpers |
| `relay-worker/schema.sql` | `nexus/worker/schema.sql` | 22 | D1 events table |

### Extract & Adapt

| Source | Target | Changes |
|--------|--------|---------|
| `relay-worker/src/subscription.rs` | `nexus/src/filter.rs` | Keep `Filter` struct, remove DO-specific `SubscriptionManager` |
| `relay-worker/src/storage.rs` | `nexus/worker/src/storage.rs` | Reused for v0.1 worker; adapter extraction deferred |
| `relay-worker/src/relay_do.rs` | `nexus/worker/src/relay_do.rs` | Reused for v0.1 worker; core relay extraction deferred |
| `relay-worker/src/lib.rs` | `nexus/worker/src/lib.rs` | Update config, keep routing pattern |

### Config Files

| Source | Target | Changes |
|--------|--------|---------|
| `relay-worker/wrangler.toml` | `nexus/worker/wrangler.toml` | Update name, database_id, vars |
| `relay-worker/package.json` | `nexus/worker/package.json` | Update names |
| `relay-worker/Cargo.toml` | `nexus/worker/Cargo.toml` | Same deps |

---

## Target Directory Structure

```
crates/nexus/
├── Cargo.toml                    # Workspace member, feature flags
├── README.md                     # Updated: Nexus is a relay
├── src/
│   ├── lib.rs                    # Core exports
│   ├── relay.rs                  # RelayHandler trait + impl
│   ├── filter.rs                 # Filter struct (from subscription.rs)
│   └── protocol/
│       ├── mod.rs
│       ├── nip01.rs              # ← relay-worker (copy)
│       ├── nip28.rs              # ← relay-worker (copy)
│       ├── nip32.rs              # ← relay-worker (copy)
│       ├── nip42.rs              # ← relay-worker (copy)
│       └── nip90.rs              # ← relay-worker (copy)
├── docs/
│   ├── MVP.md                    # ✓ exists
│   ├── BACKENDS.md               # ✓ exists
│   └── ROADMAP.md                # ← this file
└── worker/                       # Cloudflare Workers entry point
    ├── Cargo.toml
    ├── wrangler.toml
    ├── schema.sql
    ├── package.json
    └── src/
        ├── lib.rs                # Worker main, routing
        ├── relay_do.rs           # NexusRelay DO
        ├── storage.rs            # D1 + DO cache adapter
        └── subscription.rs       # Subscription matching
```

---

## Implementation Phases

### Phase 0: Project Scaffold (1 hour)

**Goal:** Set up directory structure and build configuration.

```bash
# Create worker directory
mkdir -p crates/nexus/worker/src

# Create protocol directory
mkdir -p crates/nexus/src/protocol
```

**Files to create:**

1. **`crates/nexus/worker/Cargo.toml`**
```toml
[package]
name = "nexus-worker"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
worker = { version = "0.7", features = ["http", "d1"] }
nostr = { path = "../../nostr/core", default-features = false, features = ["minimal"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
getrandom = { version = "0.2", features = ["js"] }
hex = "0.4"
js-sys = "0.3"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
console_error_panic_hook = "0.1"
```

2. **`crates/nexus/worker/wrangler.toml`** (copy from relay-worker, update)
3. **`crates/nexus/worker/schema.sql`** (copy from relay-worker)
4. **`crates/nexus/worker/package.json`** (copy from relay-worker, update names)

**Verification:** `cd crates/nexus/worker && bun run build` compiles

**Status:** ✅ Completed (2026-01-07)

---

### Phase 1: Copy Protocol Code (30 min)

**Goal:** Get all reusable NIP implementations in place.

```bash
# Copy protocol files
cp crates/relay-worker/src/nip01.rs crates/nexus/src/protocol/
cp crates/relay-worker/src/nip28.rs crates/nexus/src/protocol/
cp crates/relay-worker/src/nip32.rs crates/nexus/src/protocol/
cp crates/relay-worker/src/nip42.rs crates/nexus/src/protocol/
cp crates/relay-worker/src/nip90.rs crates/nexus/src/protocol/
```

**Create `crates/nexus/src/protocol/mod.rs`:**
```rust
pub mod nip01;
pub mod nip28;
pub mod nip32;
pub mod nip42;
pub mod nip90;

pub use nip01::{ClientMessage, RelayMessage};
pub use nip42::{generate_challenge, validate_auth_event, AUTH_KIND};
pub use nip90::{is_job_request_kind, is_job_result_kind, JOB_FEEDBACK_KIND};
```

**Verification:** Code compiles with `cargo check -p nexus`

**Status:** ✅ Completed (2026-01-07)

---

### Phase 2: Extract Filter & Subscription Logic (1 hour)

**Goal:** Create adapter-agnostic filter matching.

**Create `crates/nexus/src/filter.rs`:**
- Copy `Filter` struct from `relay-worker/src/subscription.rs`
- Copy `Filter::matches()` method
- Copy `Filter::to_sql_conditions()` method
- Remove `SubscriptionManager` (DO-specific)

**Key code to extract:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<u16>>,
    #[serde(rename = "#e")]
    pub e_tags: Option<Vec<String>>,
    #[serde(rename = "#p")]
    pub p_tags: Option<Vec<String>>,
    // ... etc
}

impl Filter {
    pub fn matches(&self, event: &Event) -> bool { ... }
    pub fn to_sql_conditions(&self) -> (String, Vec<String>) { ... }
}
```

**Status:** ✅ Completed (2026-01-07)

---

### Phase 3: Create Storage Adapter (2 hours)

**Goal:** Abstract D1 storage behind a trait.

**Create `crates/nexus/worker/src/storage.rs`:**

```rust
use worker::*;
use nostr::Event;
use crate::filter::Filter;

pub struct D1Storage<'a> {
    db: &'a D1Database,
}

impl<'a> D1Storage<'a> {
    pub fn new(db: &'a D1Database) -> Self {
        Self { db }
    }

    pub async fn store_event(&self, event: &Event) -> Result<()> {
        // Adapt from relay-worker/src/storage.rs
    }

    pub async fn query_events(&self, filter: &Filter) -> Result<Vec<Event>> {
        // Adapt from relay-worker/src/storage.rs
    }

    pub async fn get_event(&self, id: &str) -> Result<Option<Event>> {
        // Adapt from relay-worker/src/storage.rs
    }
}
```

**Key adaptations from relay-worker:**
- Keep the D1 query building logic
- Keep the `EventRow` struct for deserialization
- Keep the f64 → i64 conversion (D1 quirk)
- Remove DO cache layer (handle separately)

**Status:** ✅ Completed for v0.1 worker (2026-01-07); adapter extraction deferred

---

### Phase 4: Create Durable Object (2 hours)

**Goal:** WebSocket handling with NIP-42 auth.

**Create `crates/nexus/worker/src/durable_object.rs`:**

```rust
use worker::*;

#[durable_object]
pub struct NexusRelay {
    state: State,
    env: Env,
}

#[durable_object]
impl DurableObject for NexusRelay {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        // WebSocket upgrade
        // Generate AUTH challenge
        // Store challenge in connection metadata
    }

    async fn websocket_message(
        &self,
        ws: WebSocket,
        msg: WebSocketIncomingMessage,
    ) -> Result<()> {
        // Get connection metadata (challenge, auth state)
        // Parse ClientMessage
        // If not authenticated:
        //   - AUTH → validate, mark authenticated
        //   - Other → reject with auth-required
        // If authenticated:
        //   - EVENT → validate, store, broadcast
        //   - REQ → store subscription, query, send results
        //   - CLOSE → remove subscription
    }
}
```

**Key patterns from relay-worker:**
- WebSocket hibernation API
- Connection metadata as attachment
- Subscription storage in DO state
- Broadcast to matching subscriptions

**Status:** ✅ Completed (2026-01-07)

---

### Phase 5: Worker Entry Point (1 hour)

**Goal:** HTTP routing and NIP-11 info document.

**Create `crates/nexus/worker/src/lib.rs`:**

```rust
use worker::*;

mod durable_object;
mod storage;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // CORS headers
    if req.method() == Method::Options {
        return Response::empty()?.with_headers(cors_headers());
    }

    let url = req.url()?;

    match (req.method(), url.path()) {
        // NIP-11 relay info
        (Method::Get, "/") if !is_websocket_upgrade(&req) => {
            serve_relay_info(&env)
        }

        // Health check
        (Method::Get, "/health") => {
            Response::ok("ok")
        }

        // WebSocket → route to Durable Object
        (Method::Get, "/") | (Method::Get, "/ws") => {
            let namespace = env.durable_object("NEXUS_RELAY")?;
            let stub = namespace.id_from_name("main")?.get_stub()?;
            stub.fetch_with_request(req).await
        }

        _ => Response::error("Not found", 404)
    }
}

fn serve_relay_info(env: &Env) -> Result<Response> {
    let info = serde_json::json!({
        "name": env.var("RELAY_NAME")?.to_string(),
        "description": "Nexus - Nostr relay for the OpenAgents compute marketplace",
        "supported_nips": [1, 11, 42, 89, 90],
        "software": "https://github.com/OpenAgentsInc/openagents",
        "version": "0.1.0",
        "limitation": {
            "auth_required": true
        }
    });

    Response::from_json(&info)
}
```

**Status:** ✅ Completed (2026-01-07)

---

### Phase 6: Build & Deploy (30 min)

**Goal:** Deploy to Cloudflare Workers.

```bash
cd crates/nexus/worker

# Create D1 database
npx wrangler d1 create nexus
# Copy database_id to wrangler.toml

# Run migrations
npx wrangler d1 execute nexus --file=schema.sql

# Build
bun run build

# Deploy
npx wrangler deploy
```

**Verification:**
1. `curl https://nexus.openagents.com/` returns NIP-11 info
2. `wscat -c wss://nexus.openagents.com/` connects, receives AUTH challenge
3. Pylon can connect and authenticate

**Status:** ✅ Deployed (2026-01-07)

---

## Testing Strategy

### Unit Tests
- Filter matching logic
- NIP-42 auth validation
- Protocol message parsing

### Integration Tests
- Full AUTH flow
- EVENT → store → query
- Subscription → broadcast

### End-to-End Tests
- Pylon provider connects, receives jobs
- Pylon buyer submits job, receives result
- Multi-relay scenario (Nexus + damus.io)

---

## Deployment Checklist

- [x] D1 database created and migrated
- [x] wrangler.toml configured with correct database_id
- [x] Environment variables set (RELAY_NAME, RELAY_URL)
- [x] Build succeeds (`bun run build`)
- [x] Deploy succeeds (`npx wrangler deploy`)
- [x] NIP-11 endpoint works (`curl https://nexus.openagents.com/`)
- [x] WebSocket connects and sends AUTH challenge
- [ ] Pylon can authenticate and subscribe
- [ ] Events are stored and queryable
- [ ] Subscriptions receive broadcasts

---

## Timeline Estimate

| Phase | Duration |
|-------|----------|
| Phase 0: Scaffold | 1 hour |
| Phase 1: Protocol code | 30 min |
| Phase 2: Filter logic | 1 hour |
| Phase 3: Storage adapter | 2 hours |
| Phase 4: Durable Object | 2 hours |
| Phase 5: Entry point | 1 hour |
| Phase 6: Deploy | 30 min |
| **Total** | **~8 hours** |

Most time is spent on Phase 3-4, adapting the existing relay-worker code to the new structure. The protocol code (Phase 1) is pure copy-paste.

---

## Related Documentation

- `MVP.md` — Feature requirements and success criteria
- `BACKENDS.md` — Multi-backend architecture (CF Workers vs native)
- `relay-worker/README.md` — Existing relay implementation details
