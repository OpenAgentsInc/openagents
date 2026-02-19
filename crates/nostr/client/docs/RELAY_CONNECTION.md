# Relay Connection

The `RelayConnection` manages WebSocket connections to Nostr relays with automatic reconnection, keep-alive pings, and NIP-42 authentication.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    RelayConnection                           │
├─────────────────────────────────────────────────────────────┤
│  ┌───────────────┐  ┌─────────────┐  ┌─────────────────┐   │
│  │ WebSocket     │  │ Message     │  │ Subscriptions   │   │
│  │ Stream        │  │ Loop        │  │ HashMap         │   │
│  │               │  │             │  │                 │   │
│  │ tokio-tungstenite│ Parses msgs │  │ SubID → Sender  │   │
│  └───────────────┘  │ Routes events│ └─────────────────┘   │
│                     └─────────────┘                         │
│  ┌───────────────┐  ┌─────────────┐  ┌─────────────────┐   │
│  │ Ping Task     │  │ Auth Key    │  │ Connection      │   │
│  │               │  │             │  │ State           │   │
│  │ Keep-alive    │  │ NIP-42      │  │                 │   │
│  │ every 30s     │  │ signing     │  │ Connected/      │   │
│  └───────────────┘  └─────────────┘  │ Disconnected    │   │
│                                       └─────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Keep-Alive Mechanism

WebSocket connections can be terminated by intermediaries (load balancers, CDNs, Cloudflare) if idle. The `RelayConnection` implements a ping/pong keep-alive mechanism:

### Client-Side Ping

```rust
// Started automatically when connecting
// Sends "ping" text message every 30 seconds (configurable)
let config = RelayConfig {
    ping_interval: Duration::from_secs(30),
    ..Default::default()
};
```

### Server-Side Auto-Response (Cloudflare Workers)

For relays running on Cloudflare Workers (like Nexus), the server configures automatic ping/pong response at the edge:

```rust
// In relay_do.rs
fn setup_websocket_auto_response(state: &State) {
    if let Ok(pair) = WebSocketRequestResponsePair::new("ping", "pong") {
        state.set_websocket_auto_response(&pair);
    }
}
```

This runs at the Cloudflare edge without waking the Durable Object, making it extremely efficient.

### Why Text Ping Instead of WebSocket Ping Frames?

Cloudflare's `set_websocket_auto_response` API works with text messages, not WebSocket ping/pong frames. Using text "ping"/"pong" allows:

1. Edge-level response (no DO wake)
2. Configurable response text
3. Works with WebSocket hibernation API

## NIP-42 Authentication

When a relay sends an AUTH challenge, the connection automatically signs and sends an AUTH event:

```
Relay → Client: ["AUTH", <challenge>]
Client → Relay: ["AUTH", <signed-event>]
Relay → Client: ["OK", <event-id>, true, "auth accepted for <pubkey>"]
```

### Setting Auth Key

```rust
// For single relay
relay.set_auth_key(private_key).await;

// For pool (sets on all relays)
pool.set_auth_key(private_key).await;
```

## Connection States

```rust
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}
```

## Configuration Options

```rust
pub struct RelayConfig {
    /// Connection timeout
    pub connect_timeout: Duration,  // Default: 10s

    /// Delay before reconnection attempt
    pub reconnect_delay: Duration,  // Default: 5s

    /// Ping interval for keep-alive
    pub ping_interval: Duration,    // Default: 30s

    /// Enable SQLite event queue
    pub enable_queue: bool,         // Default: true
}
```

## Message Flow

### Incoming Messages

1. WebSocket message received
2. Parse as JSON array
3. Match message type:
   - `EVENT` → Route to subscription channel
   - `OK` → Update pending confirmation
   - `EOSE` → Mark subscription as end-of-stored-events
   - `AUTH` → Sign and send auth response
   - `NOTICE` → Log warning
   - `CLOSED` → Handle subscription closure

### Outgoing Messages

1. Serialize to JSON
2. Send via WebSocket
3. For EVENT messages, track pending confirmation

## Subscription Management

```rust
// Create subscription with channel
let (subscription, rx) = Subscription::with_channel(
    "my-sub".to_string(),
    vec![filter]
);

// Store subscription
relay.subscribe("my-sub", &[filter]).await?;

// Events are routed to the channel
while let Some(event) = rx.recv().await {
    println!("Event: {}", event.id);
}

// Unsubscribe
relay.unsubscribe("my-sub").await?;
```

## Error Handling

```rust
use nostr_client::error::ClientError;

// Connection errors
ClientError::Connection(msg)  // WebSocket connection failed
ClientError::Protocol(msg)    // Protocol violation
ClientError::Timeout(msg)     // Operation timed out
ClientError::Internal(msg)    // Internal error

// Automatic reconnection on disconnect
// Configure with reconnect_delay in RelayConfig
```

## Usage Example

```rust
use nostr_client::relay::{RelayConnection, RelayConfig};
use std::time::Duration;

// Custom config
let config = RelayConfig {
    connect_timeout: Duration::from_secs(15),
    reconnect_delay: Duration::from_secs(3),
    ping_interval: Duration::from_secs(25),
    enable_queue: false,
};

// Create connection
let relay = RelayConnection::with_config("wss://nexus.openagents.com", config);

// Set auth key for NIP-42
relay.set_auth_key(private_key).await;

// Connect
relay.connect().await?;

// Subscribe
let mut rx = relay.subscribe_with_channel("events", &[filter]).await?;

// Process events
while let Some(event) = rx.recv().await {
    println!("Received: {}", event.content);
}

// Disconnect
relay.disconnect().await?;
```

## Cloudflare-Specific Considerations

When connecting to relays on Cloudflare Workers (like Nexus):

1. **Idle Timeout**: Cloudflare terminates idle WebSocket connections. The ping/pong mechanism prevents this.

2. **Durable Object Hibernation**: Nexus uses WebSocket hibernation API. Connections can survive DO hibernation.

3. **Code 1006**: If you see WebSocket close code 1006, it indicates an abnormal closure (often idle timeout). Ensure ping is working.

4. **Authentication Required**: Nexus requires NIP-42 authentication for all operations. Set auth key before connecting.
