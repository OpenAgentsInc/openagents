# Nexus WebSocket Protocol

Nexus uses WebSocket connections for real-time Nostr relay communication. This document covers the WebSocket implementation details specific to the Nexus Cloudflare Worker deployment.

## Connection Flow

```
Client                          Nexus Worker                    Durable Object
   |                                 |                                |
   |------ WebSocket Upgrade ------->|                                |
   |                                 |------ Route to DO ------------>|
   |                                 |                                |
   |<----- AUTH Challenge -----------|<-------------------------------|
   |                                 |                                |
   |------ AUTH Event -------------->|------------------------------->|
   |                                 |                                |
   |<----- OK (auth accepted) ------|<-------------------------------|
   |                                 |                                |
   |------ REQ/EVENT/CLOSE -------->|------------------------------->|
   |                                 |                                |
   |<----- EVENT/EOSE/CLOSED -------|<-------------------------------|
```

## NIP-42 Authentication

Nexus is a private relay that requires authentication for all operations.

### Challenge Generation

When a WebSocket connection is established, the Durable Object immediately sends an AUTH challenge:

```rust
// Generate unique challenge
let challenge = nip42::generate_challenge();

// Send AUTH message
let auth_msg = RelayMessage::Auth { challenge };
server.send_with_str(&serde_json::to_string(&auth_msg)?)?;
```

### Auth Validation

The client must respond with a signed AUTH event:

```json
["AUTH", {
  "kind": 22242,
  "created_at": 1704067200,
  "tags": [
    ["relay", "wss://nexus.openagents.com"],
    ["challenge", "<challenge-string>"]
  ],
  "content": "",
  "pubkey": "<client-pubkey>",
  "sig": "<signature>"
}]
```

The relay validates:
1. Event kind is 22242
2. Relay tag matches configured RELAY_URL
3. Challenge tag matches the sent challenge
4. Event signature is valid
5. Event is not expired (created_at within tolerance)

### Rejection on Unauthenticated Messages

Before authentication, all non-AUTH messages are rejected:

| Message | Response |
|---------|----------|
| REQ | `["CLOSED", "<sub-id>", "auth-required: authentication required"]` |
| EVENT | `["OK", "<event-id>", false, "auth-required: authentication required"]` |
| CLOSE | `["NOTICE", "auth-required: authentication required"]` |

## Keep-Alive Mechanism

Cloudflare terminates idle WebSocket connections after ~100 seconds. Nexus implements automatic ping/pong keep-alive:

### Server-Side Auto-Response

```rust
fn setup_websocket_auto_response(state: &State) {
    // Configure edge-level auto-response
    if let Ok(pair) = WebSocketRequestResponsePair::new("ping", "pong") {
        state.set_websocket_auto_response(&pair);
    }
}
```

This runs at the Cloudflare edge:
- **No Durable Object wake** - Extremely efficient
- **Immediate response** - Sub-millisecond latency
- **Prevents idle timeout** - Connection stays alive

### Client Requirements

Clients should send text "ping" messages periodically:

```
Client: "ping"
Nexus (edge): "pong"
```

Recommended interval: **30 seconds**

## WebSocket Hibernation

Nexus uses Cloudflare's WebSocket Hibernation API for efficient connection handling:

### Benefits

1. **Memory efficient** - Durable Object can hibernate between messages
2. **Connection metadata** - Stored as attachment, survives hibernation
3. **Auto-response** - Ping/pong handled at edge during hibernation

### Connection Metadata

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMeta {
    pub conn_id: String,        // Unique connection ID
    pub pubkey: Option<String>, // Authenticated pubkey
    pub challenge: String,      // Auth challenge
    pub authenticated: bool,    // Auth status
}
```

Stored via `ws.serialize_attachment(&meta)` and retrieved via `ws.deserialize_attachment()`.

## Error Handling

All errors in message handling are caught to prevent connection closure:

```rust
async fn websocket_message(&self, ws: WebSocket, message: WebSocketIncomingMessage) -> Result<()> {
    if let Err(e) = self.handle_websocket_message_inner(&ws, message).await {
        console_log!("websocket_message error (non-fatal): {:?}", e);
        // Send notice but don't propagate error
        let _ = ws.send_with_str(&serde_json::to_string(&RelayMessage::Notice {
            message: "internal error".to_string(),
        })?);
    }
    Ok(()) // Always return Ok to keep connection alive
}
```

## Subscription Management

Subscriptions are stored in Durable Object storage with a key format:

```
sub:{conn_id}:{subscription_id}
```

This ensures:
- Subscriptions are tied to specific connections
- Subscriptions survive DO hibernation
- Easy cleanup on connection close

### Broadcast

When an event is stored, it's broadcast to all matching subscriptions:

```rust
async fn broadcast_event(&self, event: &Event) -> Result<()> {
    let websockets = self.state.get_websockets();

    for ws in websockets {
        let matching_subs = subs.get_matching_subscriptions(&ws, event).await?;
        for sub_id in matching_subs {
            let msg = RelayMessage::Event {
                subscription_id: sub_id,
                event: event.clone(),
            };
            ws.send_with_str(&serde_json::to_string(&msg)?)?;
        }
    }
}
```

## Configuration

Environment variables in `wrangler.toml`:

| Variable | Description | Default |
|----------|-------------|---------|
| `RELAY_NAME` | Relay name for NIP-11 | "Nexus" |
| `RELAY_URL` | Relay WebSocket URL | "wss://nexus.openagents.com" |
| `RELAY_DESCRIPTION` | Relay description | - |
| `RELAY_PUBKEY` | Relay operator pubkey | - |
| `RELAY_CONTACT` | Contact email | - |
| `SUPPORTED_NIPS` | Supported NIPs list | "1,11,42,89,90" |

## Troubleshooting

### Connection Closes with Code 1006

**Cause**: Abnormal closure, usually idle timeout

**Solution**: Ensure client sends "ping" every 30 seconds

### AUTH Rejected

**Cause**: Invalid auth event

**Check**:
1. Relay tag matches exactly (including trailing slash)
2. Challenge tag matches sent challenge
3. Event signature is valid
4. Event not expired

### Events Not Received

**Cause**: Subscription filter mismatch or not authenticated

**Check**:
1. Connection is authenticated (received OK for AUTH)
2. Filter matches expected events
3. Subscription is active (not closed)

### Broadcast Not Working

**Cause**: Filter mismatch in subscription

**Check**:
1. e-tag filter for job results
2. kinds filter includes expected kind
3. Connection is still active
