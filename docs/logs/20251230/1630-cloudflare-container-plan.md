# Plan: Add Cloudflare Container Support

## Overview

Add Cloudflare Container support to run autopilot in the cloud, alongside existing tunnel architecture. Users can choose:
- **Tunnel mode**: `openagents connect` (free, uses local machine)
- **Container mode**: Click "Start" in browser (paid, uses CF containers)

## Architecture

```
┌─────────────┐                  ┌─────────────────┐
│   Browser   │◄───WebSocket────►│  CF Worker      │
│  (WASM)     │                  │  (Routes)       │
└─────────────┘                  └────────┬────────┘
                                          │
                    ┌─────────────────────┼─────────────────────┐
                    │                     │                     │
                    ▼                     ▼                     ▼
         ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
         │  TunnelRelay DO  │  │ AutopilotDO      │  │ Container        │
         │  (existing)      │  │ (new)            │  │ (autopilot svc)  │
         │                  │  │                  │  │                  │
         │  - Relay to CLI  │  │  - Manages       │──►│  - Axum HTTP    │
         │  - Free tier     │  │    container     │  │  - Claude SDK   │
         └──────────────────┘  │  - WebSocket     │  │  - Git ops      │
                               └──────────────────┘  └──────────────────┘
```

## Implementation Steps

### Step 1: Create Autopilot Container Service

New crate that wraps autopilot in an HTTP API for containerized execution.

**Create:** `crates/autopilot-container/`

```
crates/autopilot-container/
├── Cargo.toml
├── Dockerfile
└── src/
    └── main.rs          # Axum HTTP server
```

**Dockerfile:**
```dockerfile
FROM rust:alpine AS builder
WORKDIR /app
COPY . .
RUN apk add musl-dev git openssl-dev
RUN cargo build --release -p autopilot-container

FROM alpine:latest
RUN apk add --no-cache git openssh-client
COPY --from=builder /app/target/release/autopilot-container /autopilot
ENTRYPOINT ["/autopilot"]
EXPOSE 8080
```

**HTTP Endpoints:**
| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/ping` | GET | Health check |
| `/api/start` | POST | Start task with repo + prompt |
| `/api/status` | GET | Current task status |
| `/ws` | GET | WebSocket for streaming events |

**main.rs skeleton:**
```rust
use axum::{Router, routing::{get, post}};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let router = Router::new()
        .route("/ping", get(|| async { "ok" }))
        .route("/api/start", post(start_task))
        .route("/api/status", get(get_status))
        .route("/ws", get(ws_handler));

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
```

### Step 2: Add AutopilotContainer Durable Object

New DO that manages container lifecycle and proxies requests.

**Create:** `crates/web/worker/src/autopilot_container.rs`

```rust
use worker::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[durable_object]
pub struct AutopilotContainer {
    state: State,
    env: Env,
    ready: Arc<AtomicBool>,
}

impl DurableObject for AutopilotContainer {
    fn new(state: State, env: Env) -> Self {
        let container = state.container().expect("no container");
        if !container.running() {
            let mut opts = ContainerStartupOptions::new();
            opts.enable_internet(true);
            container.start(Some(opts)).expect("failed to start");
        }

        // Health check polling
        let ready = Arc::new(AtomicBool::new(false));
        // ... poll /ping until ready ...

        Self { state, env, ready }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        // Wait for container ready
        while !self.ready.load(Ordering::Acquire) {
            Delay::from(Duration::from_millis(100)).await;
        }

        // Forward to container
        let container = self.state.container().ok_or("no container")?;
        let fetcher = container.get_tcp_port(8080)?;
        fetcher.fetch_request(req).await
    }
}
```

### Step 3: Update wrangler.toml

**Modify:** `crates/web/wrangler.toml`

```toml
# Existing tunnel relay
[durable_objects]
bindings = [
    { name = "TUNNEL_RELAY", class_name = "TunnelRelay" },
    { name = "AUTOPILOT_CONTAINER", class_name = "AutopilotContainer" }
]

[[migrations]]
tag = "v2"
new_classes = ["AutopilotContainer"]

# Container configuration
[[containers]]
class_name = "AutopilotContainer"
image = "../autopilot-container/Dockerfile"
max_instances = 10
```

### Step 4: Add Container Routes

**Modify:** `crates/web/worker/src/lib.rs`

```rust
// Container task routes (paid tier)
(Method::Post, "/api/container/start") => {
    let body = req.text().await?;
    with_auth(&req, &env, |user| {
        routes::container::start_task(user, env.clone(), body)
    }).await
}
(Method::Get, "/api/container/status") => {
    with_auth(&req, &env, |user| {
        routes::container::get_status(user, env.clone())
    }).await
}
(Method::Get, path) if path.starts_with("/api/container/ws/") => {
    routes::container::websocket(req, env).await
}
```

**Create:** `crates/web/worker/src/routes/container.rs`

```rust
pub async fn start_task(user: AuthenticatedUser, env: Env, body: String) -> Result<Response> {
    // 1. Check user has credits
    // 2. Create session in KV
    // 3. Get or create DO for user
    // 4. Forward start request to container
    // 5. Return session_id + WebSocket URL
}

pub async fn websocket(req: Request, env: Env) -> Result<Response> {
    // Route WebSocket to user's AutopilotContainer DO
}
```

### Step 5: Wire Up Exports

**Modify:** `crates/web/worker/src/lib.rs`

Add module and export:
```rust
mod autopilot_container;
pub use autopilot_container::AutopilotContainer;
```

**Modify:** `crates/web/worker/src/routes/mod.rs`

```rust
pub mod container;
```

---

## Files to Create

| File | Purpose |
|------|---------|
| `crates/autopilot-container/Cargo.toml` | Container service crate |
| `crates/autopilot-container/src/main.rs` | Axum HTTP server |
| `crates/autopilot-container/Dockerfile` | Container image |
| `crates/web/worker/src/autopilot_container.rs` | DO for container management |
| `crates/web/worker/src/routes/container.rs` | Container API routes |

## Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` (workspace) | Add `autopilot-container` to members |
| `crates/web/wrangler.toml` | Add container binding + migration |
| `crates/web/worker/src/lib.rs` | Add container routes + DO export |
| `crates/web/worker/src/routes/mod.rs` | Export container module |

---

## Execution Order

1. **Create autopilot-container crate** - Axum HTTP wrapper around autopilot
2. **Create Dockerfile** - Alpine-based minimal image
3. **Add AutopilotContainer DO** - Container lifecycle management
4. **Update wrangler.toml** - Container bindings + migrations
5. **Add container routes** - API endpoints for starting/streaming tasks
6. **Test locally** - Use miniflare with container support
7. **Deploy** - Push container image + worker update

---

## Key Decisions

1. **One container per user** - DO keyed by user_id, reuses container across tasks
2. **Container startup** - Lazy start on first request, health check polling
3. **Credentials** - Claude API key passed via env vars from user's OAuth tokens
4. **Billing** - Check credits before starting, meter container time

## Dependencies

Container service needs:
- `axum` - HTTP server
- `tokio` - Async runtime
- `tokio-tungstenite` - WebSocket support
- Autopilot crate dependencies (claude-agent-sdk, etc.)

Worker needs:
- `worker` crate with container feature (already have workers-rs)
