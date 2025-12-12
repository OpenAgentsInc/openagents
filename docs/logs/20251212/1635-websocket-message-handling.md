# WebSocket Message Handling for Cloudflare Relay

**Date:** 2025-12-12
**Time:** 16:22 - 16:35

## Summary

Implemented WebSocket message handling using Cloudflare's Durable Object hibernation API. The relay now fully supports NIP-01 protocol over WebSocket.

## Changes

### `relay_do.rs` - WebSocket Hibernation API

Added implementation of Cloudflare's WebSocket hibernation pattern:

```rust
impl DurableObject for RelayDurableObject {
    // ... new, fetch ...

    async fn websocket_message(
        &self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        // Process message through process_message()
        // Send responses back via ws.send_with_str()
    }

    async fn websocket_close(
        &self,
        ws: WebSocket,
        _code: usize,  // Note: usize, not u16
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        // Cleanup subscriptions and connection tracking
    }
}
```

Key implementation details:
- Uses `self.state.accept_web_socket(&server)` for hibernation support
- Tracks WebSockets in `RefCell<Vec<WebSocket>>` for broadcasting
- Stores subscriptions per connection ID in `RefCell<HashMap<String, SubscriptionManager>>`

### Type Fixes

- `websocket_close` `code` parameter: `u16` → `usize` (worker v0.7 API)
- Removed `websocket_error` method (WebSocketError type doesn't exist in worker v0.7)
- Fixed `broadcast_event` to use `manager.matching(event)` returning `&Subscription`

## Test Results

All NIP-01 commands working:

```bash
# REQ command - Subscribe
$ echo '["REQ", "test", {"kinds": [1]}]' | websocat ws://localhost:8788
["EOSE","test"]

# EVENT command - Publish
$ cat event.json | websocat ws://localhost:8788
["OK","5cfc93f5ce62fc5b1da1189cb47cba8f20670df0256b77e021a59485f43fa426",true,""]

# REQ command - Query stored events
$ echo '["REQ", "q", {"kinds": [1]}]' | websocat ws://localhost:8788
["EVENT","q",{...event...}]
["EOSE","q"]

# CLOSE command - Unsubscribe
$ echo '["CLOSE", "test"]' | websocat ws://localhost:8788
["CLOSED","test",""]
```

Event ID validation working correctly:
```bash
$ echo '["EVENT", {...invalid id...}]' | websocat ws://localhost:8788
["OK","...",false,"invalid event id: computed X, got Y"]
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Cloudflare Worker Durable Object                           │
├─────────────────────────────────────────────────────────────┤
│  WebSocket Hibernation Flow:                                 │
│                                                              │
│  1. Client connects → handle_websocket_upgrade()             │
│     └─ accept_web_socket() enables hibernation               │
│                                                              │
│  2. Message arrives → websocket_message()                    │
│     └─ process_message() → RelayMessage responses            │
│                                                              │
│  3. Client closes → websocket_close()                        │
│     └─ Cleanup subscriptions and tracking                    │
│                                                              │
│  Storage (in-memory, per DO instance):                       │
│  ├─ events: RefCell<Vec<Event>>                              │
│  ├─ subscriptions: RefCell<HashMap<ConnID, SubManager>>      │
│  └─ websockets: RefCell<Vec<WebSocket>>                      │
└─────────────────────────────────────────────────────────────┘
```

## Files Changed

| File | Change |
|------|--------|
| `crates/cloudflare/src/relay_do.rs` | Added WebSocket hibernation API |

## Next Steps

1. **Persistent Storage** - Replace in-memory Vec with SQLite via Durable Object storage
2. **Proper WebSocket Tracking** - Map connection IDs to WebSockets for broadcast
3. **NIP-90 DVM** - Detect kind 5xxx events and process jobs
4. **Deployment** - Deploy to Cloudflare with `wrangler deploy`
