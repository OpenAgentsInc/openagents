# Cloudflare Architecture

## Crate Structure

```
crates/
├── nostr/           # Core Nostr types (Event, signing, NIP-01/06/28/90)
├── nostr-client/    # WebSocket CLIENT for connecting to relays
├── nostr-relay/     # Relay SERVER protocol (runtime-agnostic)
└── cloudflare/      # Cloudflare Workers wrapper (Durable Objects)
```

## Dependency Graph

```
nostr-relay ─────▶ nostr (core types)
      │
      ▼
cloudflare ──────▶ nostr-relay (protocol)
      │
      └─────────▶ nostr (types)

nostr-client ────▶ nostr (types)
```

## Why This Split?

### `nostr` (Core)
- Event structure, signing, verification
- NIP implementations (01, 06, 28, 90)
- No runtime dependencies
- Used by ALL other Nostr crates

### `nostr-client` (Client)
- WebSocket client for connecting TO relays
- Uses tokio + async-tungstenite
- Connection pooling, reconnection
- Used by Commander/desktop apps

### `nostr-relay` (Server Protocol)
- Message parsing (CLIENT→RELAY, RELAY→CLIENT)
- Subscription filter matching
- Storage trait (implement for any DB)
- **Runtime-agnostic** - no tokio, no cloudflare
- Feature flags for WASM vs native

### `cloudflare` (Worker)
- Durable Object wrapper around `nostr-relay`
- WebSocket handling via Workers API
- SQLite via DO storage
- HTTP endpoints (NIP-11)

## Durable Object Design

```
┌─────────────────────────────────────────────────────────┐
│                  RelayDurableObject                      │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐    ┌──────────────┐                   │
│  │   SQLite     │    │  WebSocket   │                   │
│  │   Storage    │    │   Sessions   │                   │
│  │              │    │              │                   │
│  │  - events    │    │  - conn_id   │                   │
│  │  - indexes   │    │  - subs      │                   │
│  └──────────────┘    └──────────────┘                   │
│         │                   │                            │
│         └─────────┬─────────┘                            │
│                   │                                      │
│                   ▼                                      │
│  ┌─────────────────────────────────────────────────┐    │
│  │              nostr-relay                         │    │
│  │  - ClientMessage::from_json()                    │    │
│  │  - Filter::matches(event)                        │    │
│  │  - SqlQueryBuilder::build_select()               │    │
│  │  - RelayMessage::to_json()                       │    │
│  └─────────────────────────────────────────────────┘    │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

## Feature Flags

### `nostr-relay`

| Feature | Description |
|---------|-------------|
| `native` (default) | Full signature verification via bitcoin/secp256k1 |
| `wasm` | Skip signature verification (for Cloudflare Workers) |

### Usage

```toml
# Native server
nostr-relay = { path = "../nostr-relay" }

# Cloudflare Workers (WASM)
nostr-relay = { path = "../nostr-relay", default-features = false, features = ["wasm"] }
```

## Message Flow

```
Client                     Worker                     Durable Object
   │                          │                             │
   │ WebSocket Connect        │                             │
   │─────────────────────────▶│                             │
   │                          │ route to DO                 │
   │                          │────────────────────────────▶│
   │                          │                             │
   │ ["REQ", "sub1", filter]  │                             │
   │─────────────────────────▶│ forward                     │
   │                          │────────────────────────────▶│
   │                          │                             │ ClientMessage::from_json()
   │                          │                             │ query SQLite
   │                          │                             │ Filter::matches()
   │                          │◀────────────────────────────│
   │ ["EVENT", "sub1", event] │                             │
   │◀─────────────────────────│                             │
   │ ["EOSE", "sub1"]         │                             │
   │◀─────────────────────────│                             │
   │                          │                             │
```

## Storage Schema

```sql
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    pubkey TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    kind INTEGER NOT NULL,
    tags TEXT NOT NULL,        -- JSON array
    content TEXT NOT NULL,
    sig TEXT NOT NULL
);

CREATE INDEX idx_events_pubkey ON events(pubkey);
CREATE INDEX idx_events_kind ON events(kind);
CREATE INDEX idx_events_created_at ON events(created_at DESC);
```
