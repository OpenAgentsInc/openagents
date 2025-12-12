# OANIX Sprint 5: Complete - All Capability Services

**Date:** 2025-12-11 09:15 CST

---

## Summary

Sprint 5 is **100% complete**. All three capability services are implemented:

| Service | Lines | Unit Tests | Integration Tests | Purpose |
|---------|-------|------------|-------------------|---------|
| NostrFs | ~600 | 13 | 5 | Nostr event signing, NIP-90 DVM |
| WsFs | ~700 | 20 | 4 | WebSocket connection management |
| HttpFs | ~600 | 18 | 5 | HTTP request/response client |

**Total Tests:** 115 passing (92 unit + 23 integration)

---

## Architecture: Outbox/Inbox Pattern

All three capability services follow the same design pattern:

```
┌─────────────────────────────────────────────────────────────┐
│                    Agent (WASM/Native)                       │
├─────────────────────────────────────────────────────────────┤
│  /cap/nostr/submit  │  /cap/ws/control  │  /cap/http/request │
│         ↓           │         ↓          │         ↓          │
│      OUTBOX         │      OUTBOX        │       QUEUE        │
├─────────────────────────────────────────────────────────────┤
│              External Executor/Transport Layer               │
│    (Relay Connector)  │  (WS Transport)  │  (HTTP Client)    │
├─────────────────────────────────────────────────────────────┤
│       INBOX          │       INBOX        │     RESPONSES     │
│         ↓           │         ↓          │         ↓          │
│  /cap/nostr/inbox/  │  /cap/ws/.../in   │  /cap/http/responses/│
└─────────────────────────────────────────────────────────────┘
```

**Why this pattern:**
1. **Testable** - No network required for testing
2. **Portable** - Works in WASM/browser environments
3. **Plan 9 philosophy** - Clean file interface, transport is separate
4. **Composable** - Same pattern, different capabilities

---

## NostrFs Details

### File Layout
```
/cap/nostr/
├── identity/
│   ├── pubkey       # 64-char hex public key
│   └── npub         # bech32 npub1...
├── outbox/          # Events waiting to send
│   └── {id}.json
├── inbox/           # Events received
│   └── {id}.json
├── submit           # Write event template → signed in outbox
├── request          # Write NIP-90 request → signed in outbox
└── status           # Service status
```

### Key Features
- Event signing with secp256k1
- NIP-90 Data Vending Machine job requests
- Relay list management
- Outbox/inbox event storage

---

## WsFs Details

### File Layout
```
/cap/ws/
├── control          # {"url": "wss://..."} or {"id": "conn-0"}
├── status           # Overall service status
└── conns/
    └── {id}/
        ├── in       # Read incoming messages (FIFO)
        ├── out      # Write outgoing messages
        ├── status   # Connection state JSON
        └── url      # WebSocket URL
```

### Connection States
```
Connecting → Open → Closing → Closed
         ↘ Error
```

### Key Features
- Multiple concurrent connections (max configurable)
- FIFO message queues (inbox/outbox)
- Connection state machine
- External transport integration

---

## HttpFs Details

### File Layout
```
/cap/http/
├── request          # Write request JSON → queued
├── pending/         # Pending requests
│   └── {id}.json
├── responses/       # Completed responses
│   └── {id}.json
└── status           # Service status
```

### Request States
```
Pending → InProgress → Completed
                   ↘ Failed
```

### Key Features
- All HTTP methods (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS)
- Custom headers and body
- Configurable timeout
- Request/response queue management

---

## Complete Agent Namespace

```rust
use oanix::*;

// All capability services
let task = TaskFs::new(spec, meta);
let logs = LogsFs::new();
let workspace = CowFs::new(WorkspaceFs::new("./project")?);
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
```

---

## Files Created/Modified

### New Files (Sprint 5)
- `src/services/nostr_fs.rs` (~600 lines)
- `src/services/ws_fs.rs` (~700 lines)
- `src/services/http_fs.rs` (~600 lines)
- `docs/logs/20251211/0836-oanix-sprint5-nostrfs.md`
- `docs/logs/20251211/0900-oanix-sprint5-wsfs-httpfs.md`

### Modified Files
- `src/services/mod.rs` - All exports
- `src/lib.rs` - Public re-exports
- `Cargo.toml` - Dependencies and example config
- `tests/integration.rs` - 11 new tests (23 total)
- `docs/ROADMAP.md` - Sprint 5 complete
- `README.md` - Full capability documentation

---

## Test Summary

### Unit Tests by Module
| Module | Tests |
|--------|-------|
| namespace | 1 |
| cow_fs | 10 |
| func_fs | 6 |
| http_fs | 18 |
| logs_fs | 8 |
| map_fs | 5 |
| mem_fs | 8 |
| task_fs | 9 |
| workspace_fs | 9 |
| ws_fs | 20 |
| **Total** | **92** |

### Integration Tests
| Pattern | Tests |
|---------|-------|
| TaskFs pattern | 1 |
| Workspace snapshots | 1 |
| Control files | 1 |
| Full namespace | 1 |
| Layered snapshots | 1 |
| Read-only enforcement | 1 |
| Dynamic recomputation | 1 |
| Namespace routing | 1 |
| TaskFs service | 1 |
| LogsFs service | 1 |
| Complete environment | 1 |
| Task failure | 1 |
| WsFs tests | 4 |
| HttpFs tests | 5 |
| Combined capabilities | 2 |
| **Total** | **23** |

---

## Milestones Achieved

| Milestone | Description | Status |
|-----------|-------------|--------|
| M1 | Run "Hello World" WASI in namespace | ✅ Sprint 2 |
| M2 | Terminal-Bench task in OANIX env | ✅ Sprint 4 |
| M3 | Agent with Nostr capability | ✅ Sprint 5 |
| M4 | Full capability suite (WsFs, HttpFs) | ✅ Sprint 5 |

---

## Next: Sprint 6 - OanixEnv & Scheduler

Sprint 6 will implement:

1. **OanixEnv** - Complete environment abstraction
   - Wraps Namespace + WasiRuntime
   - Status tracking
   - Resource management

2. **Job Scheduler** - Multi-job execution
   - Job queue with priorities
   - Concurrent execution
   - Resource limits

See ROADMAP.md for full Sprint 6 spec.
