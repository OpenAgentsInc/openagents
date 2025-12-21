# nostr-client

High-performance Nostr client implementation with relay pool management, intelligent routing, and offline support.

## Features

- **Relay Connection Management**: Connect to multiple relays with automatic reconnection
- **Event Publishing**: Publish events with confirmation and retry logic
- **Subscriptions**: Manage event subscriptions with real-time delivery
- **Relay Pool**: Intelligent multi-relay management with load balancing
- **Outbox Model (NIP-65)**: Route events to optimal relays based on user preferences
- **Local Caching**: Cache events locally for faster access
- **Contact Sync (NIP-02)**: Synchronize contact lists across relays
- **Message Queue**: Offline support with automatic retry and exponential backoff

## Quick Start

```rust
use nostr_client::{RelayConnection, RelayMessage};
use nostr::{EventTemplate, finalize_event, generate_secret_key};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to a relay
    let relay = RelayConnection::new("wss://relay.damus.io")?;
    relay.connect().await?;

    // Subscribe to events
    let filters = vec![json!({
        "kinds": [1],
        "limit": 10
    })];
    relay.subscribe("my-sub", &filters).await?;

    // Publish an event
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "Hello Nostr!".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
    };
    let event = finalize_event(&template, &secret_key)?;
    relay.publish_event(&event, Duration::from_secs(5)).await?;

    // Receive messages
    while let Ok(Some(msg)) = relay.recv().await {
        match msg {
            RelayMessage::Event(sub_id, event) => {
                println!("Event: {}", event.content);
            }
            RelayMessage::Eose(sub_id) => {
                println!("End of stored events");
                break;
            }
            _ => {}
        }
    }

    relay.disconnect().await?;
    Ok(())
}
```

## Architecture

```
RelayPool
├── RelayConnection (relay 1)
│   ├── WebSocket
│   ├── MessageQueue (offline support)
│   └── Subscriptions
├── RelayConnection (relay 2)
└── OutboxModel (NIP-65 routing)
```

## Relay Pool Example

```rust
use nostr_client::{RelayPool, PoolConfig};

let config = PoolConfig::default();
let pool = RelayPool::new(config);

// Add relays
pool.add_relay("wss://relay.damus.io").await?;
pool.add_relay("wss://nos.lol").await?;

// Publish to multiple relays
let confirmations = pool.publish(&event, 2).await?;
println!("Published to {} relays", confirmations.len());

// Subscribe across all relays
pool.subscribe("global-sub", &filters).await?;
```

## Offline Support with Message Queue

```rust
use nostr_client::{RelayConfig, QueueConfig};

let config = RelayConfig {
    enable_queue: true,
    queue_poll_interval: Duration::from_secs(30),
    ..Default::default()
};

let relay = RelayConnection::with_config("wss://relay.damus.io", config)?;

// Events will be queued if relay is offline
// and automatically retried when connection is restored
relay.publish_event(&event, Duration::from_secs(5)).await?;
```

## Contact List Synchronization

```rust
use nostr_client::ContactManager;
use nostr::{Contact, ContactList};

let manager = ContactManager::new();

// Merge contact lists from multiple sources
let local_list = ContactList::new();
let remote_list = ContactList::new(); // From relay

let merged = manager.merge(
    &local_list,
    &remote_list,
    MergeStrategy::Union, // Combine both lists
)?;
```

## Outbox Model (NIP-65)

```rust
use nostr_client::{OutboxModel, OutboxConfig};

let config = OutboxConfig::default();
let outbox = OutboxModel::new(config);

// Load relay lists from NIP-65 events
outbox.update_relay_list(&pubkey, &relay_list_event).await?;

// Get optimal relays for publishing to a user
let write_relays = outbox.get_write_relays(&target_pubkey);

// Publish to optimal relays
for relay_url in write_relays {
    pool.publish_to(&event, &relay_url).await?;
}
```

## Event Caching

```rust
use nostr_client::{EventCache, CacheConfig};

let config = CacheConfig {
    max_events: 10000,
    ttl_seconds: 3600,
};

let cache = EventCache::new(config);

// Store events
cache.insert(&event)?;

// Query cached events
let events = cache.query(&filter)?;
```

## Configuration

### RelayConfig

```rust
pub struct RelayConfig {
    pub auto_reconnect: bool,
    pub reconnect_delay: Duration,
    pub max_reconnect_attempts: usize,
    pub connection_timeout: Duration,
    pub ping_interval: Duration,
    pub enable_queue: bool,
    pub queue_poll_interval: Duration,
}
```

### PoolConfig

```rust
pub struct PoolConfig {
    pub min_confirmations: usize,      // Minimum relay confirmations
    pub publish_timeout: Duration,
    pub max_concurrent_requests: usize,
}
```

## Error Handling

```rust
use nostr_client::{ClientError, Result};

match relay.connect().await {
    Ok(()) => println!("Connected"),
    Err(ClientError::Connection(msg)) => eprintln!("Connection failed: {}", msg),
    Err(ClientError::Timeout(_)) => eprintln!("Connection timeout"),
    Err(e) => eprintln!("Error: {}", e),
}
```

## Performance

- **Connection pooling**: Reuse connections across operations
- **Concurrent publishing**: Publish to multiple relays in parallel
- **Local caching**: Reduce redundant relay queries
- **Message batching**: Combine multiple operations where possible

## Testing

```bash
# Unit tests
cargo test -p nostr-client

# Integration tests (requires local relay)
cargo test -p nostr-integration-tests

# Compatibility tests (requires network)
cargo test -p nostr-client --test compatibility -- --ignored
```

## License

CC-0 (Public Domain)
