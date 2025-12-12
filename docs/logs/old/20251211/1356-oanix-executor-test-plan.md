# OANIX E2E Executor Testing Plan

## Overview

Create comprehensive E2E tests for Sprint 7 executors with a **hybrid approach**:
1. **Local mock servers** - deterministic, fast, for CI
2. **Live endpoints** - real-world validation smoke tests

## Test Module Structure

```
crates/oanix/tests/
├── integration.rs                 # Existing (1900+ lines)
└── executor/                      # NEW
    ├── mod.rs                     # Test harness exports
    ├── fixtures/
    │   ├── mod.rs
    │   ├── http_mock.rs           # wiremock wrapper
    │   ├── ws_echo.rs             # tokio-tungstenite echo server
    │   ├── nostr_relay.rs         # NIP-01 mock relay
    │   └── helpers.rs             # Shared test utilities
    ├── http_tests.rs              # HttpExecutor E2E tests
    ├── ws_tests.rs                # WsConnector E2E tests
    ├── nostr_tests.rs             # NostrRelayConnector E2E tests
    └── smoke/
        ├── mod.rs
        ├── http_live.rs           # httpbin.org tests
        ├── ws_live.rs             # Public echo server tests
        └── nostr_live.rs          # Real relay tests
```

---

## Phase 1: Test Fixtures

### 1.1 HTTP Mock Server (`fixtures/http_mock.rs`)
Use wiremock (already in dev-deps):
- `HttpMockServer::start()` - spawn mock server
- `mount_get(path, status, body)` - GET endpoint
- `mount_post_echo(path)` - echo POST body
- `mount_slow_response(path, delay_ms)` - timeout testing
- `mount_error(path, times)` - retry testing

### 1.2 WebSocket Echo Server (`fixtures/ws_echo.rs`)
Build with tokio-tungstenite:
- `WsEchoServer::start()` - bind to random port
- Echoes text/binary messages back
- Handles ping/pong, close frames
- `shutdown()` for cleanup

### 1.3 Nostr Mock Relay (`fixtures/nostr_relay.rs`)
NIP-01 implementation:
- Handle `["EVENT", ...]` → respond `["OK", ...]`
- Handle `["REQ", sub_id, ...]` → respond `["EOSE", ...]`
- Handle `["CLOSE", sub_id]`
- `inject_event()` - simulate incoming events
- `received_events()` - verify sent events

### 1.4 Test Helpers (`fixtures/helpers.rs`)
```rust
pub struct ExecutorTestFixture {
    pub http_fs: Arc<HttpFs>,
    pub ws_fs: Arc<WsFs>,
    pub nostr_fs: Arc<NostrFs>,
    pub executor: ExecutorManager,
}

pub fn fast_test_config() -> ExecutorConfig;
pub async fn wait_for_response(http_fs, req_id, timeout) -> bool;
pub async fn wait_for_ws_state(ws_fs, conn_id, state, timeout) -> bool;
```

---

## Phase 2: HTTP Executor Tests (`http_tests.rs`)

| Test | Description |
|------|-------------|
| `test_http_get_full_flow` | GET request → mock server → response in HttpFs |
| `test_http_post_with_body` | POST with JSON body, verify echo |
| `test_http_timeout` | 100ms timeout vs 10s delay → failure |
| `test_http_retry_on_error` | 2 failures then success → retry works |
| `test_http_concurrent_requests` | 10 parallel requests all succeed |
| `test_http_headers` | Custom headers sent correctly |

---

## Phase 3: WebSocket Connector Tests (`ws_tests.rs`)

| Test | Description |
|------|-------------|
| `test_ws_connect_and_echo` | Connect → send → receive echo → close |
| `test_ws_connection_timeout` | Connect to bad port → Error state |
| `test_ws_multiple_connections` | 5 concurrent connections |
| `test_ws_message_fifo_order` | 10 messages arrive in order |
| `test_ws_reconnect_on_close` | Server closes → reconnect |
| `test_ws_ping_keepalive` | Connection stays alive with pings |

---

## Phase 4: Nostr Relay Tests (`nostr_tests.rs`)

| Test | Description |
|------|-------------|
| `test_nostr_send_event` | Sign event → sent to mock relay |
| `test_nostr_subscription` | REQ sent → EOSE received |
| `test_nostr_receive_event` | Relay pushes event → NostrFs inbox |
| `test_nostr_multiple_relays` | Event broadcast to 2+ relays |
| `test_nostr_nip90_job_request` | Kind 5050 job request flow |
| `test_nostr_relay_reconnect` | Relay disconnects → reconnect |

---

## Phase 5: Live Smoke Tests (`smoke/`)

### HTTP Live (`smoke/http_live.rs`)
```rust
#[tokio::test]
#[ignore] // cargo test -- --ignored
async fn test_http_live_httpbin_get() {
    // GET https://httpbin.org/get
}

async fn test_http_live_httpbin_post() {
    // POST https://httpbin.org/post
}
```

### WebSocket Live (`smoke/ws_live.rs`)
```rust
#[ignore]
async fn test_ws_live_echo() {
    // Try wss://ws.ifelse.io or wss://echo.websocket.events
}
```

### Nostr Live (`smoke/nostr_live.rs`)
```rust
#[ignore]
async fn test_nostr_live_relay() {
    // Connect to relay.damus.io, nos.lol
    // Send ephemeral kind 20001 event (won't persist)
}
```

---

## Critical Files to Create

| File | Lines | Description |
|------|-------|-------------|
| `tests/executor/mod.rs` | ~30 | Module declarations |
| `tests/executor/fixtures/mod.rs` | ~20 | Fixture exports |
| `tests/executor/fixtures/http_mock.rs` | ~80 | wiremock wrapper |
| `tests/executor/fixtures/ws_echo.rs` | ~100 | Echo server |
| `tests/executor/fixtures/nostr_relay.rs` | ~150 | NIP-01 mock |
| `tests/executor/fixtures/helpers.rs` | ~80 | Test utilities |
| `tests/executor/http_tests.rs` | ~200 | HTTP E2E tests |
| `tests/executor/ws_tests.rs` | ~200 | WS E2E tests |
| `tests/executor/nostr_tests.rs` | ~200 | Nostr E2E tests |
| `tests/executor/smoke/mod.rs` | ~10 | Smoke test module |
| `tests/executor/smoke/http_live.rs` | ~60 | Live HTTP tests |
| `tests/executor/smoke/ws_live.rs` | ~50 | Live WS tests |
| `tests/executor/smoke/nostr_live.rs` | ~60 | Live Nostr tests |

---

## Cargo.toml Changes

```toml
# Already have:
# wiremock = "0.6"
# tokio-test = "0.4"

# No new dependencies needed - wiremock + tokio-tungstenite already available
```

---

## Test Commands

```bash
# Fast CI tests (mock servers only)
cargo test --features "net-executor,nostr" -p oanix executor

# Include live smoke tests
cargo test --features "net-executor,nostr" -p oanix -- --ignored

# Just smoke tests
cargo test --features "net-executor,nostr" -p oanix smoke -- --ignored
```

---

## Implementation Order

1. **Phase 1**: Create fixtures (http_mock, ws_echo, helpers)
2. **Phase 2**: HTTP executor tests (6 tests)
3. **Phase 3**: WebSocket connector tests (6 tests)
4. **Phase 4**: Nostr mock relay + tests (6 tests)
5. **Phase 5**: Live smoke tests (3 test files)
6. **Phase 6**: Run all tests, fix issues

---

## Key Reference Files

- `crates/oanix/src/executor/http.rs` - HttpExecutor implementation
- `crates/oanix/src/executor/ws.rs` - WsConnector implementation
- `crates/oanix/src/executor/nostr.rs` - NostrRelayConnector implementation
- `crates/oanix/src/services/http_fs.rs` - HttpFs API
- `crates/oanix/src/services/ws_fs.rs` - WsFs API, WsState enum
- `crates/oanix/src/services/nostr_fs.rs` - NostrFs API, Filter type


