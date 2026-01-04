# Relay Worker Deployment Log

**Date**: 2026-01-03 22:34
**Author**: Claude Opus 4.5

## Summary

Deployed a Nostr relay on Cloudflare Workers at `relay.openagents.com` using Durable Objects with WebSocket hibernation API. The relay implements NIP-01, NIP-11, NIP-28, NIP-32, NIP-42, and NIP-90.

## Live Endpoints

- **WebSocket**: `wss://relay.openagents.com/`
- **NIP-11 Info**: `https://relay.openagents.com/`
- **Workers.dev**: `https://openagents-relay.openagents.workers.dev/`

## Architecture

### Crate Location
`crates/relay-worker/`

### Key Files
- `src/lib.rs` - Worker entry point, HTTP routing
- `src/relay_do.rs` - NostrRelay Durable Object (WebSocket handling)
- `src/nip01.rs` - ClientMessage/RelayMessage types
- `src/nip42.rs` - AUTH challenge generation and validation
- `src/storage.rs` - D1 + DO cache hybrid storage
- `src/subscription.rs` - Filter matching for subscriptions

### Infrastructure
- **D1 Database**: `openagents-relay` (ID: `df30de4e-7dcd-4037-81fe-e67f1a80889c`)
- **Durable Object**: `NostrRelay` class with SQLite storage
- **Custom Domain**: `relay.openagents.com` (configured in Cloudflare dashboard)

## Connecting from Pylon MVP

### WebSocket Connection

```rust
// Connect to the relay
let url = "wss://relay.openagents.com/";
let (ws, _) = tokio_tungstenite::connect_async(url).await?;
```

### Authentication Flow (NIP-42 Required)

1. **On connect**, relay immediately sends AUTH challenge:
   ```json
   ["AUTH", "<32-byte-hex-challenge>"]
   ```

2. **Client must respond** with signed AUTH event (kind 22242):
   ```json
   ["AUTH", {
     "id": "<event-id>",
     "pubkey": "<client-pubkey>",
     "created_at": <unix-timestamp>,
     "kind": 22242,
     "tags": [
       ["relay", "wss://relay.openagents.com/"],
       ["challenge", "<challenge-from-step-1>"]
     ],
     "content": "",
     "sig": "<signature>"
   }]
   ```

3. **Relay responds** with OK:
   ```json
   ["OK", "<event-id>", true, "auth accepted for <pubkey>"]
   ```

4. **After authentication**, client can use REQ/EVENT/CLOSE normally.

### All Operations Require Auth

Any message before authentication returns:
- REQ: `["CLOSED", "<sub-id>", "auth-required: authentication required"]`
- EVENT: `["OK", "<event-id>", false, "auth-required: authentication required"]`

### NIP-11 Relay Info

```bash
curl -H "Accept: application/nostr+json" https://relay.openagents.com/
```

Returns:
```json
{
  "name": "relay.openagents.com",
  "description": "OpenAgents NIP-90 inference relay",
  "supported_nips": [1, 11, 28, 32, 42, 90],
  "limitation": {
    "auth_required": true,
    "max_subscriptions": 20,
    "max_filters": 10,
    "max_limit": 500
  }
}
```

## Implementation Details

### WebSocket Hibernation API

The relay uses Cloudflare's WebSocket hibernation API for efficient persistent connections:

```rust
// In fetch() - accept WebSocket with hibernation
self.state.accept_web_socket(&server);
server.serialize_attachment(&meta)?;

// Return 101 Switching Protocols
Ok(ResponseBuilder::new()
    .with_status(101)
    .with_websocket(pair.client)
    .empty())

// In websocket_message() - retrieve metadata
let meta: ConnectionMeta = ws.deserialize_attachment()?
    .unwrap_or_default();
```

### Storage Layer

Hybrid D1 + DO cache:
- **D1**: Persistent event storage with indexes on pubkey, kind, created_at
- **DO Storage**: Hot cache for recent events, subscription state

### Event Validation

Minimal validation (no signature verification in WASM):
- Check id, pubkey, sig are correct hex lengths (64, 64, 128 chars)
- Store and broadcast to matching subscriptions

## Deployment Commands

```bash
cd crates/relay-worker

# Build WASM
bun run build

# Deploy to Cloudflare
npx wrangler deploy

# Apply D1 schema (if needed)
npx wrangler d1 execute openagents-relay --file=schema.sql
```

## Issues Resolved

### `__wbindgen_start is not a function` Error

**Cause**: Using incorrect WebSocket hibernation API pattern.

**Fix**: Changed from `accept_websocket_with_tags()` + `get_tags()` to `accept_web_socket()` + `serialize_attachment()` + `deserialize_attachment()`.

Reference implementation: `~/code/workers-rs/test/src/counter.rs`

### Edition 2024 Compatibility

The nostr crate uses Rust 2024 edition. Had to inline all workspace dependencies in `crates/nostr/core/Cargo.toml` since relay-worker has its own isolated workspace.

## Dependencies

```toml
[dependencies]
worker = { version = "0.7", features = ["http", "d1"] }
nostr = { path = "../nostr/core", default-features = false, features = ["minimal"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
getrandom = { version = "0.2", features = ["js"] }
hex = "0.4"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
```

## Next Steps

1. Configure `RELAY_PUBKEY` environment variable with operator's pubkey
2. Implement signature verification (requires secp256k1 WASM build)
3. Add rate limiting per authenticated pubkey
4. Implement NIP-90 DVM job routing logic
5. Add metrics/logging to D1 or external service
