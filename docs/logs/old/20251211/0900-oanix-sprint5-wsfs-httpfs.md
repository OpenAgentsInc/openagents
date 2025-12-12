# OANIX Sprint 5: WsFs and HttpFs Capability Services

**Date:** 2025-12-11

---

## Summary

Completed Sprint 5 of OANIX by implementing the remaining capability services: WsFs (WebSocket connections) and HttpFs (HTTP client). All three capability services (NostrFs, WsFs, HttpFs) are now complete.

---

## WsFs Overview

### What It Does

WsFs provides WebSocket connection management as a filesystem interface:

1. **Connection Management** - Open, close, and monitor WebSocket connections
2. **Message Queuing** - Outbox/inbox pattern for message exchange
3. **State Tracking** - Connection states: Connecting, Open, Closing, Closed, Error
4. **External Transport** - Actual WebSocket I/O handled by external connector

### File Layout

```
/cap/ws/
├── control           # Write: {"url": "wss://..."} or {"id": "conn-0"}
├── status            # Overall service status JSON
└── conns/
    └── {id}/
        ├── in        # Read incoming messages (FIFO queue)
        ├── out       # Write outgoing messages (queued for send)
        ├── status    # Connection state JSON
        └── url       # The WebSocket URL (read-only)
```

### Key API Methods

```rust
// Programmatic API
let ws = WsFs::new();

// Open connection
let conn_id = ws.open_connection("wss://relay.example.com")?;

// Connection lifecycle
ws.set_connected(&conn_id)?;  // Mark as open (transport calls this)
ws.close_connection(&conn_id)?;  // Request close
ws.set_closed(&conn_id)?;  // Mark as closed
ws.set_error(&conn_id, "Connection refused")?;  // Mark error

// Send message (queues to outbox)
ws.send_message(&conn_id, b"Hello".to_vec())?;

// External transport drains outbox
let messages = ws.drain_outbox(&conn_id)?;

// Transport adds received messages to inbox
ws.receive_message(&conn_id, b"Response".to_vec())?;

// Read from inbox (FIFO)
let msg = ws.read_message(&conn_id)?;
```

### File Interface Example

```rust
// Open connection via control file
write_file(&ws, "/control", r#"{"url": "wss://relay.example.com"}"#);

// List connections
let conns = ws.readdir("/conns")?;

// Write to outbox
write_file(&ws, "/conns/conn-0/out", "Hello WebSocket!");

// Read from inbox
let msg = read_file(&ws, "/conns/conn-0/in");

// Close connection
write_file(&ws, "/control", r#"{"id": "conn-0"}"#);
```

---

## HttpFs Overview

### What It Does

HttpFs provides HTTP request/response capabilities as a filesystem interface:

1. **Request Submission** - Write request JSON, get queued for execution
2. **Request Queue** - Pending requests tracked in `/pending/`
3. **Response Collection** - Completed responses in `/responses/`
4. **State Tracking** - Pending, InProgress, Completed, Failed

### File Layout

```
/cap/http/
├── request           # Write request JSON → queued for execution
├── pending/          # Pending requests
│   └── {id}.json     # Request details
├── responses/        # Completed responses
│   └── {id}.json     # Response with status, headers, body
└── status            # Service status
```

### Key API Methods

```rust
// Programmatic API
let http = HttpFs::new();

// Submit request
let request = HttpRequest {
    method: HttpMethod::Get,
    url: "https://api.example.com/data".to_string(),
    headers: HashMap::from([("Authorization".to_string(), "Bearer token".to_string())]),
    body: None,
    timeout_secs: Some(30),
    ..Default::default()
};
let req_id = http.submit_request(request);

// External executor takes pending request
let pending = http.take_pending(&req_id)?;

// Executor completes request
http.complete_request(HttpResponse {
    request_id: req_id.clone(),
    status: 200,
    status_text: "OK".to_string(),
    headers: HashMap::new(),
    body: r#"{"result": "success"}"#.to_string(),
    duration_ms: 150,
    completed_at: now(),
});

// Or executor fails request
http.fail_request(&req_id, "Connection timeout");

// Check state
let state = http.get_state(&req_id);  // Some(RequestState::Completed)

// Get response
let response = http.get_response(&req_id)?;
```

### File Interface Example

```rust
// Submit request via file
let request_json = r#"{
    "method": "POST",
    "url": "https://api.example.com/data",
    "headers": {"Content-Type": "application/json"},
    "body": "{\"key\": \"value\"}"
}"#;
write_file(&http, "/request", request_json);

// Check pending
let pending = http.readdir("/pending")?;

// Read response (after executor completes)
let response = read_file(&http, "/responses/req-0.json");

// Check status
let status = read_file(&http, "/status");
// {"pending_count": 0, "completed_count": 1, "failed_count": 0, ...}
```

---

## Complete Agent Namespace

With all capability services complete, a full agent namespace looks like:

```rust
let task = TaskFs::new(spec, meta);
let logs = LogsFs::new();
let workspace = CowFs::new(project_base);
let ws = WsFs::new();
let http = HttpFs::new();

#[cfg(feature = "nostr")]
let nostr = NostrFs::new(secret_key)?;

let ns = Namespace::builder()
    .mount("/task", task)
    .mount("/logs", logs)
    .mount("/workspace", workspace)
    .mount("/cap/ws", ws)
    .mount("/cap/http", http)
    #[cfg(feature = "nostr")]
    .mount("/cap/nostr", nostr)
    .mount("/tmp", MemFs::new())
    .build();

// Agent workflow:
// 1. Read task from /task/spec.json
// 2. Read/write workspace files
// 3. Submit HTTP requests via /cap/http/request
// 4. Open WebSocket connections via /cap/ws/control
// 5. (With nostr feature) Sign events via /cap/nostr/submit
// 6. Log progress to /logs/
// 7. Write result to /task/result.json
```

---

## Test Results

### Unit Tests (38 new, 92 total)

**WsFs Tests (20 new):**
```
test services::ws_fs::tests::test_ws_fs_creation ... ok
test services::ws_fs::tests::test_open_connection_programmatic ... ok
test services::ws_fs::tests::test_open_connection_via_file ... ok
test services::ws_fs::tests::test_close_connection_via_file ... ok
test services::ws_fs::tests::test_connection_lifecycle ... ok
test services::ws_fs::tests::test_connection_error ... ok
test services::ws_fs::tests::test_send_receive_messages ... ok
test services::ws_fs::tests::test_send_via_file ... ok
test services::ws_fs::tests::test_receive_via_file ... ok
test services::ws_fs::tests::test_read_connection_status ... ok
test services::ws_fs::tests::test_read_connection_url ... ok
test services::ws_fs::tests::test_readdir_root ... ok
test services::ws_fs::tests::test_readdir_conns ... ok
test services::ws_fs::tests::test_readdir_connection ... ok
test services::ws_fs::tests::test_read_overall_status ... ok
test services::ws_fs::tests::test_max_connections ... ok
test services::ws_fs::tests::test_cannot_send_on_closed ... ok
test services::ws_fs::tests::test_fifo_message_order ... ok
```

**HttpFs Tests (18 new):**
```
test services::http_fs::tests::test_http_fs_creation ... ok
test services::http_fs::tests::test_submit_request_programmatic ... ok
test services::http_fs::tests::test_submit_request_via_file ... ok
test services::http_fs::tests::test_complete_request ... ok
test services::http_fs::tests::test_fail_request ... ok
test services::http_fs::tests::test_read_response_via_file ... ok
test services::http_fs::tests::test_read_failure_via_file ... ok
test services::http_fs::tests::test_readdir_root ... ok
test services::http_fs::tests::test_readdir_pending ... ok
test services::http_fs::tests::test_read_status ... ok
test services::http_fs::tests::test_request_state_transitions ... ok
test services::http_fs::tests::test_take_pending ... ok
test services::http_fs::tests::test_default_timeout ... ok
test services::http_fs::tests::test_clear_response ... ok
test services::http_fs::tests::test_http_methods ... ok
```

### Integration Tests (11 new, 23 total)

**WsFs Integration (4 new):**
```
test ws_tests::test_ws_capability_in_namespace ... ok
test ws_tests::test_ws_connection_lifecycle ... ok
test ws_tests::test_ws_message_exchange ... ok
test ws_tests::test_ws_agent_workflow ... ok
```

**HttpFs Integration (5 new):**
```
test http_tests::test_http_capability_in_namespace ... ok
test http_tests::test_http_request_response_lifecycle ... ok
test http_tests::test_http_error_handling ... ok
test http_tests::test_http_agent_workflow ... ok
test http_tests::test_http_multiple_requests ... ok
```

**Combined Capability Tests (2 new):**
```
test combined_capabilities_tests::test_full_capability_namespace ... ok
test combined_capabilities_tests::test_multi_capability_workflow ... ok
```

### Total: 115 tests passing (92 unit + 23 integration)

---

## Files Created/Modified

### New Files

- `src/services/ws_fs.rs` - WsFs implementation (~700 lines)
- `src/services/http_fs.rs` - HttpFs implementation (~600 lines)

### Modified Files

- `src/services/mod.rs` - Export WsFs, HttpFs and their types
- `src/lib.rs` - Re-export new types
- `Cargo.toml` - Added `[[example]]` with required-features
- `tests/integration.rs` - Added 11 new integration tests

---

## Design Decisions

### Queue-Based Pattern

Both WsFs and HttpFs use a queue-based pattern where:
- Agent submits requests/messages through the filesystem
- External executor/connector handles actual I/O
- Responses arrive through the filesystem

**Benefits:**
1. **Testability** - Can test without network
2. **Portability** - Works in WASM environments
3. **Separation of concerns** - Capability handles queueing, transport handles I/O
4. **Plan 9 philosophy** - Clean file interface, network is separate

### Connection State Machine (WsFs)

```
Connecting → Open → Closing → Closed
         ↘ Error
```

- Agent initiates state changes (open, close)
- Transport confirms state changes (connected, closed, error)

### Request State Machine (HttpFs)

```
Pending → InProgress → Completed
                   ↘ Failed
```

- Agent submits requests (pending)
- Executor takes and processes (in_progress)
- Executor completes or fails (completed/failed)

---

## Sprint 5 Complete

All three capability services are now implemented:

| Service | Purpose | Status |
|---------|---------|--------|
| NostrFs | Nostr event signing, NIP-90 DVM | ✅ Complete |
| WsFs | WebSocket connection management | ✅ Complete |
| HttpFs | HTTP request/response client | ✅ Complete |

---

## Next Steps (Future Sprints)

1. **External Connectors** - Implement actual WebSocket and HTTP executors
2. **WASI Integration** - Connect capabilities to WASI runtime
3. **Relay Connector** - Bridge WsFs to Nostr relays
4. **End-to-End Testing** - Full agent workflows with real network
