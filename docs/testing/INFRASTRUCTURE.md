# Test Infrastructure

This document explains the testing infrastructure available in OpenAgents, including test helpers, mocks, and fixtures.

## TestApp Pattern

The `TestApp` pattern provides isolated test environments with mock services. Each crate has its own TestApp implementation tailored to its needs.

### GitAfter TestApp

Location: `crates/gitafter/tests/helpers/test_app.rs`

```rust
use gitafter::TestApp;

#[tokio::test]
async fn test_repository_creation() {
    // Create isolated test environment
    let app = TestApp::new().await.unwrap();

    // Use helper methods
    let repo = app.create_repository(
        "openagents",
        "OpenAgents",
        "Description"
    ).await.unwrap();

    assert_eq!(repo.kind, 30617);

    // Cleanup
    app.shutdown().await;
}
```

**Available Methods:**
- `new()` - Create new test app with mock relay
- `pubkey()` - Get test identity public key
- `create_repository()` - Create repository announcement
- `create_issue()` - Create issue event
- `create_bounty()` - Attach bounty to issue
- `claim_issue()` - Claim issue as agent
- `create_pr()` - Create pull request
- `comment_on_issue()` - Post NIP-22 comment
- `merge_pr()` - Set PR status to merged
- `claim_bounty()` - Claim bounty after merge
- `pay_bounty()` - Release NIP-57 payment
- `get_all_events()` - Get all events from relay
- `get_events_by_kind()` - Filter events by kind
- `shutdown()` - Cleanup test environment

### Autopilot TestApp

Location: `crates/autopilot/tests/helpers/test_app.rs`

```rust
use autopilot::TestApp;

#[tokio::test]
async fn test_trajectory_collection() {
    let app = TestApp::new().await;

    // Run autopilot task
    let result = app.run_task("Create hello.txt file").await.unwrap();

    // Verify trajectory captured
    assert!(result.trajectory.steps.len() > 0);
    assert_eq!(result.success, true);
}
```

## Mock Services

### MockRelay (Nostr)

Location: `crates/testing/src/mock_relay.rs`

In-memory Nostr relay for testing without external dependencies.

```rust
use testing::MockRelay;

#[tokio::test]
async fn test_event_publishing() {
    // Start mock relay on random port
    let relay = MockRelay::start().await;

    // Publish event
    relay.store_event(event).await;

    // Query events
    let events = relay.get_events_by_kind(1621).await;
    assert_eq!(events.len(), 1);

    // Cleanup
    relay.shutdown().await;
}
```

**Features:**
- Event storage and retrieval
- Filter-based queries
- Subscription support
- WebSocket connections
- No external dependencies

### Mock Wallet Service

Location: `crates/testing/src/mock_wallet.rs`

```rust
use testing::MockWalletService;

#[tokio::test]
async fn test_payment_flow() {
    let wallet = MockWalletService::new()
        .with_balance(100000)
        .with_send_success(true);

    let balance = wallet.get_balance().await.unwrap();
    assert_eq!(balance, 100000);

    let tx_id = wallet.send_payment("address", 50000).await.unwrap();
    assert!(tx_id.len() > 0);
}
```

## Test Fixtures

### Event Factories

Location: `crates/testing/src/factories/events.rs`

```rust
use testing::factories::*;

#[test]
fn test_event_validation() {
    // Generate test event with sensible defaults
    let event = EventFactory::new()
        .kind(1621) // Issue
        .content("Test issue")
        .tag("a", "30617:pubkey:repo")
        .build();

    assert_eq!(event.kind, 1621);
}
```

### Identity Factories

```rust
use testing::factories::*;

#[test]
fn test_identity_validation() {
    // Generate test identity
    let identity = IdentityFactory::new()
        .with_npub()
        .build();

    assert!(identity.npub.starts_with("npub1"));
}
```

## Snapshot Testing

We use `insta` for snapshot testing UI components and structured output.

### Basic Usage

```rust
use insta::assert_snapshot;

#[test]
fn test_dashboard_rendering() {
    let dashboard = Dashboard::new()
        .with_balance(50000)
        .render();

    assert_snapshot!(dashboard.into_string());
}
```

### Reviewing Snapshots

```bash
# Run tests and generate snapshots
cargo test

# Review new/changed snapshots
cargo insta review

# Accept all changes
cargo insta accept

# Reject all changes
cargo insta reject
```

### Snapshot Files

Snapshots are stored in `snapshots/` directories next to test files:

```
crates/gitafter/tests/
├── full_workflow_test.rs
└── snapshots/
    └── full_workflow_test__test_dashboard_rendering.snap
```

## Property-Based Testing

We use `quickcheck` for property-based testing of validators and pure functions.

### Basic Example

```rust
use quickcheck::quickcheck;

quickcheck! {
    fn prop_npub_roundtrip(bytes: [u8; 32]) -> bool {
        let npub = encode_npub(&bytes);
        decode_npub(&npub)
            .map(|b| b == bytes)
            .unwrap_or(false)
    }
}
```

### Advanced Properties

```rust
use quickcheck::{quickcheck, Arbitrary};

#[derive(Clone, Debug)]
struct ValidEvent {
    kind: u16,
    content: String,
    // ... fields
}

impl Arbitrary for ValidEvent {
    fn arbitrary(g: &mut Gen) -> Self {
        ValidEvent {
            kind: u16::arbitrary(g) % 40000,
            content: String::arbitrary(g),
        }
    }
}

quickcheck! {
    fn prop_event_serialization(event: ValidEvent) -> bool {
        let json = serde_json::to_string(&event).unwrap();
        let decoded: ValidEvent = serde_json::from_str(&json).unwrap();
        decoded.kind == event.kind
    }
}
```

## Database Testing

### In-Memory Databases

```rust
use autopilot::MetricsDb;

#[test]
fn test_metrics_storage() {
    // Use in-memory database
    let db = MetricsDb::open_in_memory().unwrap();

    // Test database operations
    db.store_session(&session).unwrap();

    let sessions = db.get_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
}
```

### Test Transactions

```rust
#[test]
fn test_rollback_behavior() {
    let db = open_test_db();

    // Use transaction for isolation
    let mut tx = db.transaction().unwrap();

    tx.execute("INSERT INTO ...", params![...]).unwrap();

    // Rollback instead of commit
    // Test data is discarded
}
```

## Async Testing

### Tokio Runtime

```rust
#[tokio::test]
async fn test_async_function() {
    let result = async_operation().await;
    assert_eq!(result, expected);
}
```

### Timeouts

```rust
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_with_timeout() {
    let result = timeout(
        Duration::from_secs(5),
        slow_operation()
    ).await;

    assert!(result.is_ok(), "Operation timed out");
}
```

### WebSocket Testing

```rust
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn test_websocket() {
    let app = TestApp::new().await.unwrap();
    let ws_url = format!("ws://127.0.0.1:{}/ws", app.port);

    let (mut ws, _) = connect_async(&ws_url).await.unwrap();

    // Send message
    ws.send(Message::Text("ping".into())).await.unwrap();

    // Receive response
    let msg = ws.next().await.unwrap().unwrap();
    assert!(matches!(msg, Message::Text(_)));
}
```

## Benchmarking

Location: `crates/<crate>/benches/`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_event_validation(c: &mut Criterion) {
    let event = create_test_event();

    c.bench_function("verify_signature", |b| {
        b.iter(|| verify_signature(black_box(&event)))
    });
}

criterion_group!(benches, bench_event_validation);
criterion_main!(benches);
```

Run benchmarks:

```bash
cargo bench -p nostr-core
```

## Test Organization

### Directory Structure

```
crates/<crate>/
├── src/
│   ├── lib.rs
│   ├── module.rs
│   └── tests/         # Unit tests
│       └── mod.rs
├── tests/             # Integration tests
│   ├── helpers/
│   │   ├── mod.rs
│   │   └── test_app.rs
│   ├── full_workflow_test.rs
│   └── snapshots/
└── benches/           # Benchmarks
    └── performance.rs
```

### Shared Testing Crate

The `crates/testing` crate provides shared utilities:

- `MockRelay` - In-memory Nostr relay
- `MockWalletService` - Mock wallet for payment testing
- `factories::*` - Test data generators
- `fixtures::*` - Common test data
- `assertions::*` - Custom assertions

Import in your tests:

```toml
[dev-dependencies]
testing = { path = "../testing" }
```

## Best Practices

1. **Use TestApp for isolation** - Each test gets its own mock environment
2. **Clean up resources** - Always call `.shutdown()` on test apps
3. **Test one thing per test** - Focused tests are easier to debug
4. **Use descriptive names** - `test_repository_creation_succeeds_with_valid_data`
5. **Avoid test interdependencies** - Tests should run in any order
6. **Mock external services** - Don't depend on network or external APIs
7. **Use fixtures for common data** - DRY principle for test data
8. **Snapshot complex output** - HTML, JSON, large structures
9. **Property test pure functions** - Validators, encoders, parsers
10. **Benchmark critical paths** - Track performance over time
