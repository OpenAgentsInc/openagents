# Nostr Client Documentation

The `nostr-client` crate provides a high-level Nostr client for connecting to relays, subscribing to events, and interacting with NIP-90 Data Vending Machines.

## Components

### Core

- **RelayConnection** - Single relay WebSocket connection with automatic reconnection
- **RelayPool** - Multi-relay connection manager with unified subscription interface
- **Subscription** - Event filtering and channel-based event delivery

### NIP-90 DVM

- **DvmClient** - High-level interface for submitting NIP-90 jobs

## Documentation

| Document | Description |
|----------|-------------|
| [DVM_CLIENT.md](./DVM_CLIENT.md) | NIP-90 Data Vending Machine client usage |
| [RELAY_CONNECTION.md](./RELAY_CONNECTION.md) | Relay connection and keep-alive |

## Quick Start

### Basic Relay Connection

```rust
use nostr_client::relay::RelayConnection;
use nostr_client::pool::RelayPool;

// Single relay
let relay = RelayConnection::new("wss://relay.damus.io").await?;
relay.connect().await?;

// Multi-relay pool
let pool = RelayPool::default();
pool.add_relay("wss://relay.damus.io").await?;
pool.add_relay("wss://nos.lol").await?;
pool.connect_all().await?;
```

### Subscribing to Events

```rust
use serde_json::json;

let filter = json!({
    "kinds": [1],
    "limit": 10
});

let mut rx = pool.subscribe("my-sub", &[filter]).await?;

while let Some(event) = rx.recv().await {
    println!("Received: {} - {}", event.id, event.content);
}
```

### NIP-90 Job Submission

```rust
use nostr_client::dvm::DvmClient;
use nostr::{JobRequest, JobInput, KIND_JOB_TEXT_GENERATION};

let client = DvmClient::new(private_key)?;

let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
    .add_input(JobInput::text("What is 2+2?"));

let submission = client.submit_job(request, &["wss://nexus.openagents.com"]).await?;
let result = client.await_result(&submission.event_id, Duration::from_secs(60)).await?;
```

## Features

- **Automatic Reconnection** - Connections automatically reconnect on failure
- **Keep-Alive Ping** - Configurable ping interval prevents idle timeouts
- **NIP-42 Authentication** - Automatic AUTH handling for authenticated relays
- **Event Queueing** - Optional SQLite-backed event queue for offline operation
- **Backpressure** - Bounded channels prevent unbounded memory growth

## Configuration

```rust
use nostr_client::pool::PoolConfig;
use nostr_client::relay::RelayConfig;
use std::time::Duration;

let mut config = PoolConfig::default();
config.relay_config.connect_timeout = Duration::from_secs(10);
config.relay_config.reconnect_delay = Duration::from_secs(5);
config.relay_config.ping_interval = Duration::from_secs(30);
config.relay_config.enable_queue = false;  // Disable SQLite queue

let pool = RelayPool::new(config);
```

## Error Handling

```rust
use nostr_client::error::ClientError;

match pool.connect_all().await {
    Ok(()) => println!("Connected!"),
    Err(ClientError::Connection(e)) => println!("Connection error: {}", e),
    Err(ClientError::Protocol(e)) => println!("Protocol error: {}", e),
    Err(ClientError::Timeout(e)) => println!("Timeout: {}", e),
    Err(e) => println!("Other error: {:?}", e),
}
```
