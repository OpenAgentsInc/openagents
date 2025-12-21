# nostr-relay

Production-ready Nostr relay implementation with SQLite storage, WebSocket server, and comprehensive NIP-01 validation.

## Features

- **WebSocket Server**: High-performance async WebSocket handling with tokio-tungstenite
- **SQLite Storage**: Efficient event storage with connection pooling (r2d2)
- **Event Validation**: Comprehensive NIP-01 message validation
- **Subscription Management**: Efficient event filtering and real-time delivery
- **Broadcast System**: Real-time event distribution to active subscriptions
- **Rate Limiting**: IP-based rate limiting and spam protection
- **NIP-11 Support**: Relay information document endpoint
- **Metrics & Admin**: Built-in metrics and admin API

## Quick Start

```rust
use nostr_relay::{RelayServer, RelayConfig, Database, DatabaseConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure database
    let db_config = DatabaseConfig {
        path: PathBuf::from("./relay.db"),
        max_conn: 10,
        ..Default::default()
    };
    let db = Database::new(db_config)?;

    // Configure relay
    let config = RelayConfig {
        bind_addr: "127.0.0.1:7000".parse()?,
        max_message_size: 512 * 1024, // 512 KB
        ..Default::default()
    };

    // Start relay
    let server = RelayServer::new(config, db);
    server.start().await?;

    Ok(())
}
```

## Architecture

```
┌─────────────────────────────────────────────┐
│         WebSocket Server (warp)             │
└────────────────┬────────────────────────────┘
                 │
┌────────────────▼────────────────────────────┐
│         Connection Manager                  │
│    Per-client state, subscription tracking  │
└────────────────┬────────────────────────────┘
                 │
   ┌─────────────┼─────────────┐
   ▼             ▼             ▼
┌────────┐  ┌────────┐  ┌────────┐
│ Writer │  │ Reader │  │  Meta  │
│  Pool  │  │  Pool  │  │  Pool  │
└────────┘  └────────┘  └────────┘
     │           │           │
     └───────────┼───────────┘
                 ▼
       ┌─────────────────┐
       │  Broadcast Bus  │
       └─────────────────┘
```

## Configuration

### RelayConfig

```rust
pub struct RelayConfig {
    pub bind_addr: SocketAddr,      // WebSocket bind address
    pub max_message_size: usize,    // Max message size (bytes)
    pub rate_limit: RateLimitConfig,
    pub relay_info: RelayInformation, // NIP-11 info
}
```

### DatabaseConfig

```rust
pub struct DatabaseConfig {
    pub path: PathBuf,              // SQLite database path
    pub max_conn: u32,              // Connection pool size
    pub cache_size_kb: usize,       // SQLite cache size
}
```

### RateLimitConfig

```rust
pub struct RateLimitConfig {
    pub events_per_minute: u32,     // Event publish rate limit
    pub max_subscriptions: usize,   // Max subscriptions per connection
    pub max_connections_per_ip: usize,
    pub ban_duration_secs: u64,     // Ban duration for violations
}
```

## Event Validation

The relay performs comprehensive NIP-01 validation:

```rust
// Validation checks:
// - Event ID is 64-char lowercase hex
// - Pubkey is 64-char lowercase hex
// - Signature is 128-char lowercase hex
// - Timestamps are reasonable (not too far future/past)
// - Tags array is properly formatted
// - Content length is within limits
// - Cryptographic signature verification (with full feature)
```

## Subscription Filtering

```rust
// Supported filter attributes:
let filter = json!({
    "ids": ["event_id_prefix"],      // Event ID prefix matching
    "authors": ["pubkey_prefix"],    // Author pubkey prefix matching
    "kinds": [1, 3, 10002],          // Event kinds
    "since": 1234567890,             // Unix timestamp
    "until": 1234567900,             // Unix timestamp
    "limit": 100,                    // Max events to return
    "#e": ["referenced_event_id"],   // Tag filters
    "#p": ["referenced_pubkey"],
});
```

## NIP-11 Relay Information

```rust
use nostr_relay::RelayInformation;

let relay_info = RelayInformation {
    name: Some("My Nostr Relay".to_string()),
    description: Some("A fast Nostr relay".to_string()),
    pubkey: Some("relay_operator_pubkey".to_string()),
    contact: Some("admin@relay.example.com".to_string()),
    supported_nips: vec![1, 9, 11, 12, 15, 16, 20, 22, 33, 40],
    software: Some("nostr-relay".to_string()),
    version: Some("0.1.0".to_string()),
    limitation: Some(Limitation {
        max_message_length: Some(512 * 1024),
        max_subscriptions: Some(20),
        max_filters: Some(10),
        max_limit: Some(5000),
        max_subid_length: Some(64),
        max_event_tags: Some(2000),
        max_content_length: Some(64 * 1024),
        ..Default::default()
    }),
    ..Default::default()
};

// Serve NIP-11 endpoint
server.start_info_server("0.0.0.0:8080".parse()?).await?;

// Query with:
// curl -H "Accept: application/nostr+json" http://localhost:8080
```

## Metrics

```rust
let metrics = server.metrics();

println!("Connections: {}", metrics.active_connections());
println!("Events stored: {}", metrics.total_events_stored());
println!("Events/sec: {}", metrics.events_per_second());
println!("DB queries/sec: {}", metrics.db_queries_per_second());
```

## Rate Limiting

```rust
use nostr_relay::RateLimitConfig;

let rate_limit = RateLimitConfig {
    events_per_minute: 60,          // 1 event/sec average
    max_subscriptions: 20,          // 20 concurrent subs
    max_connections_per_ip: 10,     // 10 connections per IP
    ban_duration_secs: 3600,        // 1 hour ban
};
```

## Storage

### Event Storage

Events are stored in SQLite with indexes on:
- Event ID (primary key)
- Pubkey + created_at (for author queries)
- Kind + created_at (for kind queries)
- Tag filters (#e, #p, etc.)

### Replaceable Events

The relay automatically handles:
- Regular events (stored permanently)
- Replaceable events (keep latest only)
- Ephemeral events (not stored)
- Addressable events (replaceable by kind+pubkey+d-tag)

## Security

### Event Validation
- All events cryptographically verified (with full feature)
- Invalid events rejected with detailed error messages
- Malformed messages logged and client notified

### Rate Limiting
- Per-IP connection limits
- Event publish rate limits
- Subscription count limits
- Automatic IP banning on violations

### Resource Protection
- Maximum message size limits
- Maximum event content length
- Maximum tag count per event
- Connection timeout handling

## Performance

- **Throughput**: 10,000+ events/sec
- **Concurrent connections**: 10,000+
- **Subscription filtering**: < 1ms per event
- **Database**: Connection pooling with r2d2
- **Broadcasting**: Efficient fan-out to active subscriptions

## Testing

```bash
# Unit tests
cargo test -p nostr-relay

# Integration tests with client
cargo test -p nostr-integration-tests

# Run relay for testing
cargo run -p nostr-relay --example simple_relay
```

## Production Deployment

### Systemd Service

```ini
[Unit]
Description=Nostr Relay
After=network.target

[Service]
Type=simple
User=nostr
ExecStart=/usr/local/bin/nostr-relay --config /etc/nostr/relay.toml
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### Nginx Reverse Proxy

```nginx
upstream nostr_relay {
    server 127.0.0.1:7000;
}

server {
    listen 443 ssl;
    server_name relay.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://nostr_relay;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## Monitoring

```bash
# Check relay health
curl http://localhost:8080/health

# Get metrics
curl http://localhost:8080/metrics

# Get stats
curl http://localhost:8080/stats
```

## License

CC-0 (Public Domain)
