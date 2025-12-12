# Cloudflare Workers Integration Plan

## Goal
Build a **Nostr Relay + DVM Agent** on Cloudflare Workers. The Worker IS a Nostr relay that also processes NIP-90 jobs internally.

## Architecture
```
┌─────────────────────────────────────────────────────────────┐
│                   Cloudflare Worker                          │
├─────────────────────────────────────────────────────────────┤
│  WebSocket Handler (NIP-01)                                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Client connects → upgrade to WebSocket              │    │
│  │  ["REQ", sub_id, filters] → subscribe                │    │
│  │  ["EVENT", event] → store & broadcast                │    │
│  │  ["CLOSE", sub_id] → unsubscribe                     │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                  │
│                           ▼                                  │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  RelayDurableObject                                  │    │
│  │  - SQLite: events table (NIP-01)                     │    │
│  │  - Manages WebSocket sessions                        │    │
│  │  - Broadcasts to subscribers                         │    │
│  │  - DVM: listens for kind 5xxx, processes, publishes  │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Hello-World Handshake Test (NIP-01)
1. Client connects via WebSocket to `wss://relay.openagents.com`
2. Client sends: `["REQ", "test-sub", {"kinds": [1], "limit": 10}]`
3. Relay responds: `["EOSE", "test-sub"]`
4. Client publishes: `["EVENT", {...signed event...}]`
5. Relay responds: `["OK", "event-id", true, ""]`
6. Client receives broadcast: `["EVENT", "test-sub", {...event...}]`

---

## Phase 1: Crate Setup

### Create `crates/cloudflare/`

```
crates/cloudflare/
├── Cargo.toml
├── wrangler.toml
├── src/
│   ├── lib.rs              # Entry point, WebSocket upgrade
│   ├── relay/
│   │   ├── mod.rs
│   │   ├── durable_object.rs  # RelayDurableObject
│   │   ├── protocol.rs        # NIP-01 message parsing
│   │   ├── subscription.rs    # Filter matching
│   │   └── storage.rs         # SQLite event storage
│   ├── dvm/
│   │   ├── mod.rs
│   │   └── processor.rs       # NIP-90 job processing
│   └── nostr/
│       └── wasm_compat.rs     # WASM signing helpers
```

### Cargo.toml
```toml
[package]
name = "openagents-cloudflare"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
worker = { version = "0.7", features = ["d1"] }
worker-macros = "0.7"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
futures = "0.3"
hex = "0.4"
sha2 = "0.10"
# Local crates
nostr = { path = "../nostr", features = ["wasm"] }
```

### wrangler.toml
```toml
name = "openagents-relay"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

[durable_objects]
bindings = [{ name = "RELAY", class_name = "RelayDurableObject" }]

[[migrations]]
tag = "v1"
new_sqlite_classes = ["RelayDurableObject"]
```

---

## Phase 2: NIP-01 Protocol Types

### `src/relay/protocol.rs`
```rust
use serde::{Deserialize, Serialize};

/// Client → Relay messages
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ClientMessage {
    Event(EventMessage),      // ["EVENT", event]
    Req(ReqMessage),          // ["REQ", sub_id, ...filters]
    Close(CloseMessage),      // ["CLOSE", sub_id]
}

#[derive(Debug, Deserialize)]
pub struct EventMessage(pub String, pub NostrEvent);

#[derive(Debug, Deserialize)]
pub struct ReqMessage {
    pub sub_id: String,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Deserialize)]
pub struct CloseMessage(pub String, pub String);

/// Relay → Client messages
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RelayMessage {
    Event(Vec<serde_json::Value>),  // ["EVENT", sub_id, event]
    Ok(Vec<serde_json::Value>),     // ["OK", event_id, accepted, message]
    Eose(Vec<serde_json::Value>),   // ["EOSE", sub_id]
    Notice(Vec<serde_json::Value>), // ["NOTICE", message]
}

impl RelayMessage {
    pub fn event(sub_id: &str, event: &NostrEvent) -> Self {
        Self::Event(vec![
            "EVENT".into(),
            sub_id.into(),
            serde_json::to_value(event).unwrap(),
        ])
    }

    pub fn ok(event_id: &str, accepted: bool, message: &str) -> Self {
        Self::Ok(vec![
            "OK".into(),
            event_id.into(),
            accepted.into(),
            message.into(),
        ])
    }

    pub fn eose(sub_id: &str) -> Self {
        Self::Eose(vec!["EOSE".into(), sub_id.into()])
    }
}

/// NIP-01 Filter
#[derive(Debug, Clone, Deserialize)]
pub struct Filter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<u16>>,
    #[serde(rename = "#e")]
    pub e_tags: Option<Vec<String>>,
    #[serde(rename = "#p")]
    pub p_tags: Option<Vec<String>>,
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub limit: Option<usize>,
}
```

---

## Phase 3: Durable Object Relay

### `src/relay/durable_object.rs`
```rust
use worker::*;
use std::collections::HashMap;

#[durable_object]
pub struct RelayDurableObject {
    state: State,
    env: Env,
    /// Active subscriptions: ws_id -> (sub_id -> filters)
    subscriptions: HashMap<String, HashMap<String, Vec<Filter>>>,
    /// WebSocket sessions
    sessions: Vec<WebSocket>,
}

#[durable_object]
impl DurableObject for RelayDurableObject {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            subscriptions: HashMap::new(),
            sessions: Vec::new(),
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        // Initialize SQLite schema on first request
        self.init_db().await?;

        // Handle WebSocket upgrade
        if req.headers().get("Upgrade")?.as_deref() == Some("websocket") {
            return self.handle_websocket(req).await;
        }

        // HTTP endpoints (NIP-11 relay info, health)
        match req.path().as_str() {
            "/" | "/.well-known/nostr.json" => self.relay_info().await,
            "/health" => Response::ok("OK"),
            _ => Response::error("Not found", 404),
        }
    }
}

impl RelayDurableObject {
    async fn init_db(&self) -> Result<()> {
        self.state.storage().sql().exec(
            "CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                pubkey TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                kind INTEGER NOT NULL,
                tags TEXT NOT NULL,
                content TEXT NOT NULL,
                sig TEXT NOT NULL
            )",
            vec![],
        )?;
        self.state.storage().sql().exec(
            "CREATE INDEX IF NOT EXISTS idx_kind ON events(kind)",
            vec![],
        )?;
        self.state.storage().sql().exec(
            "CREATE INDEX IF NOT EXISTS idx_pubkey ON events(pubkey)",
            vec![],
        )?;
        Ok(())
    }

    async fn handle_websocket(&mut self, req: Request) -> Result<Response> {
        let pair = WebSocketPair::new()?;
        let server = pair.server;
        let client = pair.client;

        server.accept()?;
        self.sessions.push(server.clone());

        // Spawn message handler
        wasm_bindgen_futures::spawn_local(async move {
            self.handle_ws_messages(server).await;
        });

        Response::from_websocket(client)
    }

    async fn handle_ws_messages(&mut self, ws: WebSocket) {
        let mut events = ws.events().expect("events");

        while let Some(event) = events.next().await {
            match event {
                Ok(WebsocketEvent::Message(msg)) => {
                    if let Some(text) = msg.text() {
                        self.handle_message(&ws, &text).await;
                    }
                }
                Ok(WebsocketEvent::Close(_)) => break,
                Err(_) => break,
            }
        }
    }

    async fn handle_message(&mut self, ws: &WebSocket, text: &str) {
        let msg: Result<ClientMessage, _> = serde_json::from_str(text);

        match msg {
            Ok(ClientMessage::Req(req)) => {
                self.handle_req(ws, req).await;
            }
            Ok(ClientMessage::Event(event_msg)) => {
                self.handle_event(ws, event_msg.1).await;
            }
            Ok(ClientMessage::Close(close)) => {
                self.handle_close(ws, &close.1);
            }
            Err(e) => {
                let notice = RelayMessage::notice(&format!("Parse error: {}", e));
                ws.send_with_str(&serde_json::to_string(&notice).unwrap()).ok();
            }
        }
    }

    async fn handle_req(&mut self, ws: &WebSocket, req: ReqMessage) {
        // Query existing events matching filters
        let events = self.query_events(&req.filters).await;

        // Send matching events
        for event in events {
            let msg = RelayMessage::event(&req.sub_id, &event);
            ws.send_with_str(&serde_json::to_string(&msg).unwrap()).ok();
        }

        // Send EOSE
        let eose = RelayMessage::eose(&req.sub_id);
        ws.send_with_str(&serde_json::to_string(&eose).unwrap()).ok();

        // Store subscription for future events
        // (simplified - need ws_id tracking)
    }

    async fn handle_event(&mut self, ws: &WebSocket, event: NostrEvent) {
        // Verify signature
        if !verify_event(&event) {
            let ok = RelayMessage::ok(&event.id, false, "invalid: signature");
            ws.send_with_str(&serde_json::to_string(&ok).unwrap()).ok();
            return;
        }

        // Store event
        self.store_event(&event).await;

        // Send OK
        let ok = RelayMessage::ok(&event.id, true, "");
        ws.send_with_str(&serde_json::to_string(&ok).unwrap()).ok();

        // Broadcast to subscribers
        self.broadcast_event(&event).await;

        // If NIP-90 job request, process it
        if event.kind >= 5000 && event.kind < 6000 {
            self.process_dvm_job(&event).await;
        }
    }

    async fn store_event(&self, event: &NostrEvent) {
        self.state.storage().sql().exec(
            "INSERT OR IGNORE INTO events (id, pubkey, created_at, kind, tags, content, sig)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            vec![
                event.id.clone().into(),
                event.pubkey.clone().into(),
                (event.created_at as i64).into(),
                (event.kind as i64).into(),
                serde_json::to_string(&event.tags).unwrap().into(),
                event.content.clone().into(),
                event.sig.clone().into(),
            ],
        ).ok();
    }

    async fn query_events(&self, filters: &[Filter]) -> Vec<NostrEvent> {
        // Build SQL query from filters
        // (simplified - full implementation needs proper filter → SQL)
        let rows = self.state.storage().sql()
            .exec("SELECT * FROM events ORDER BY created_at DESC LIMIT 100", vec![])
            .ok();
        // Convert rows to NostrEvent...
        vec![]
    }

    async fn broadcast_event(&self, event: &NostrEvent) {
        for ws in &self.sessions {
            // Check if event matches any subscription
            // Send to matching subscribers
        }
    }

    async fn process_dvm_job(&mut self, event: &NostrEvent) {
        // Parse NIP-90 job request
        // Process job (hello-world for now)
        // Create and sign result event
        // Store and broadcast result
    }
}
```

---

## Phase 4: Entry Point (WebSocket Upgrade)

### `src/lib.rs`
```rust
use worker::*;

mod relay;

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Route all requests to the relay Durable Object
    let relay_do = env.durable_object("RELAY")?;
    let stub = relay_do.id_from_name("main-relay")?.get_stub()?;
    stub.fetch_with_request(req).await
}

// Export Durable Object
pub use relay::durable_object::RelayDurableObject;
```

---

## Phase 5: WASM Nostr Signing

### Modify `crates/nostr/Cargo.toml`
```toml
[features]
default = ["native"]
native = ["rand"]
wasm = ["getrandom/js"]
```

### Event verification (reuse existing)
```rust
// Already in crates/nostr/src/nip01.rs
pub fn verify_event(event: &Event) -> bool {
    // 1. Compute expected ID from serialization
    // 2. Verify Schnorr signature
}
```

---

## Implementation Order

1. **Create crate skeleton** - `crates/cloudflare/` with Cargo.toml, wrangler.toml
2. **Add wasm feature to nostr** - Feature flag for WASM compilation
3. **Implement protocol.rs** - NIP-01 message types
4. **Implement durable_object.rs** - RelayDurableObject with SQLite
5. **Implement WebSocket handling** - Message loop, subscriptions
6. **Implement lib.rs** - Route to DO
7. **Test locally** - `wrangler dev` + WebSocket client
8. **Add DVM processing** - Handle kind 5xxx events

---

## Test Commands

```bash
# Local dev
cd crates/cloudflare && wrangler dev

# Test with websocat (install: cargo install websocat)
websocat ws://localhost:8787

# Then type NIP-01 messages:
["REQ", "test", {"kinds": [1], "limit": 5}]

# Expected response:
["EOSE", "test"]

# Publish an event:
["EVENT", {"id":"...", "pubkey":"...", "created_at":1234567890, "kind":1, "tags":[], "content":"Hello Nostr!", "sig":"..."}]

# Expected response:
["OK", "...", true, ""]
```

### Test Script (TypeScript)
```typescript
// test/relay-handshake.ts
const ws = new WebSocket('ws://localhost:8787');

ws.onopen = () => {
  console.log('Connected to relay');

  // Subscribe to kind 1 events
  ws.send(JSON.stringify(['REQ', 'test-sub', { kinds: [1], limit: 10 }]));
};

ws.onmessage = (e) => {
  const msg = JSON.parse(e.data);
  console.log('Received:', msg);

  if (msg[0] === 'EOSE') {
    console.log('Subscription ready, handshake complete!');
    ws.close();
  }
};
```

---

## Files to Modify

| File | Action |
|------|--------|
| `crates/cloudflare/` | Create new crate |
| `crates/nostr/Cargo.toml` | Add `wasm` feature flag |
| `crates/nostr/src/lib.rs` | Conditional compilation for WASM |
| `Cargo.toml` (workspace) | Add cloudflare to members |

---

## Future Extensions (Not This PR)

- **NIP-11**: Relay information document at `/`
- **NIP-09**: Event deletion
- **NIP-40**: Expiration timestamp
- **NIP-90 DVM**: Full job processing with payments
- **Blossom (BUD-01)**: R2-backed file storage
- **Multiple relays**: Shard by pubkey/kind across DOs
- **External relay sync**: Subscribe to other relays, replicate events
