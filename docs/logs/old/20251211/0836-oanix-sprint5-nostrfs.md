# OANIX Sprint 5: NostrFs Capability Service

**Date:** 2025-12-11

---

## Summary

Implemented the first capability service for OANIX: NostrFs, which provides Nostr event signing and NIP-90 Data Vending Machine capabilities as a Plan 9-style filesystem interface.

---

## NostrFs Overview

### What It Does

NostrFs exposes Nostr capabilities to agents through a filesystem interface:

1. **Identity Management** - Agent can read its public key and npub
2. **Event Signing** - Agent writes event templates, gets signed events in outbox
3. **NIP-90 DVM Requests** - Agent can request AI inference from DVMs
4. **Outbox/Inbox Pattern** - Clean separation from relay transport

### File Layout

```
/cap/nostr/
├── identity/
│   ├── pubkey       # 64-char hex public key (read-only)
│   └── npub         # bech32 npub1... (read-only)
├── outbox/          # Events waiting to be sent to relays
│   └── {id}.json    # Individual signed events
├── inbox/           # Events received from relays
│   └── {id}.json    # Individual events
├── submit           # Write event template JSON → signed event in outbox
├── request          # Write NIP-90 job request JSON → signed event in outbox
└── status           # Service status JSON
```

### Integration with Existing Nostr Crate

NostrFs builds on top of `crates/nostr` which already implements:
- NIP-01: Basic event structure, signing, verification
- NIP-06: Key derivation from BIP39 mnemonic
- NIP-28: Public chat (channels)
- NIP-90: Data Vending Machine job requests/results/feedback

---

## API Examples

### Programmatic API

```rust
use oanix::NostrFs;
use std::collections::HashMap;

// Create with existing secret key
let nostr = NostrFs::new(secret_key)?;

// Or generate a new identity
let nostr = NostrFs::generate()?;

// Get identity
println!("pubkey: {}", nostr.pubkey());
println!("npub: {}", nostr.npub());

// Add preferred relays
nostr.add_relay("wss://relay.damus.io");
nostr.add_relay("wss://nos.lol");

// Create NIP-90 job request
let mut params = HashMap::new();
params.insert("model".to_string(), "gpt-4".to_string());

let event = nostr.create_job_request(
    5050, // KIND_JOB_TEXT_GENERATION
    "What is the capital of France?",
    params,
)?;

// Event is now in outbox, ready for relay submission
let pending = nostr.outbox_events();
println!("Pending events: {}", pending.len());

// After external relay sends it, remove from outbox
nostr.remove_from_outbox(&event.id);

// When responses arrive, add to inbox
nostr.add_to_inbox(response_event);
```

### File Interface (for agents)

```rust
// Agent reads its identity
let mut handle = nostr.open("/identity/pubkey", OpenFlags::read_only())?;
let mut buf = vec![0u8; 64];
handle.read(&mut buf)?;

// Agent submits an event
let event_json = r#"{"kind": 1, "content": "Hello Nostr!"}"#;
let mut handle = nostr.open("/submit", OpenFlags::write_only())?;
handle.write(event_json.as_bytes())?;
handle.flush()?;  // Event is signed and added to outbox

// Agent submits a NIP-90 job request
let request_json = r#"{
    "kind": 5050,
    "input": "Summarize this article",
    "params": {"max_tokens": "500"},
    "bid": 1000
}"#;
let mut handle = nostr.open("/request", OpenFlags::write_only())?;
handle.write(request_json.as_bytes())?;
handle.flush()?;

// Agent lists outbox
let entries = nostr.readdir("/outbox")?;
for entry in entries {
    println!("Pending: {}", entry.name);
}

// Agent reads status
let status = read_file(&nostr, "/status");
// {"status": "ready", "pubkey": "...", "outbox_count": 2, "inbox_count": 0, "relays": [...]}
```

### In a Complete Agent Namespace

```rust
let task = TaskFs::new(spec, meta);
let logs = LogsFs::new();
let nostr = NostrFs::new(secret_key)?;
nostr.add_relay("wss://relay.damus.io");
let workspace = CowFs::new(project_base);

let ns = Namespace::builder()
    .mount("/task", task)
    .mount("/logs", logs)
    .mount("/cap/nostr", nostr)
    .mount("/workspace", workspace)
    .mount("/tmp", MemFs::new())
    .build();

// Agent workflow:
// 1. Read task from /task/spec.json
// 2. Read workspace files
// 3. Create NIP-90 request via /cap/nostr/request
// 4. External relay connector sends event, receives response
// 5. Agent reads response from /cap/nostr/inbox/{id}.json
// 6. Agent writes result to /task/result.json
```

---

## Design Decisions

### Outbox/Inbox Pattern

NostrFs uses an outbox/inbox pattern rather than direct relay connections:

**Outbox:**
- Events are signed and stored when agent writes to `/submit` or `/request`
- External relay connector reads outbox and sends to relays
- After successful send, event is removed from outbox

**Inbox:**
- External relay connector receives events from subscriptions
- Events are added to inbox via `add_to_inbox()` API
- Agent reads events from `/inbox/{id}.json`

**Why this design:**
1. **Separation of concerns** - NostrFs handles crypto, relay connector handles network
2. **Testability** - Can test NostrFs without network
3. **Flexibility** - Same NostrFs works with different transport layers
4. **Plan 9 philosophy** - Clean file interface, transport is separate

### No Relay Connections in NostrFs

NostrFs intentionally does NOT connect to relays. This would be handled by:
- **WsFs** - Generic WebSocket capability (Sprint 5 remaining)
- **Background relay task** - Connects outbox to relays, relays to inbox
- **External process** - Could be a separate daemon

### Event Signing on Flush

Events are signed when `flush()` is called on the submit/request handle:
- Allows buffered writes of large JSON
- Clear point where signing happens
- Consistent with Plan 9 `wstat` semantics

---

## Test Results

### Unit Tests (13 new, 72 total)

```
test services::nostr_fs::tests::test_nostr_fs_creation ... ok
test services::nostr_fs::tests::test_nostr_fs_generate ... ok
test services::nostr_fs::tests::test_read_identity ... ok
test services::nostr_fs::tests::test_read_status ... ok
test services::nostr_fs::tests::test_sign_event ... ok
test services::nostr_fs::tests::test_create_job_request ... ok
test services::nostr_fs::tests::test_submit_event_via_file ... ok
test services::nostr_fs::tests::test_submit_job_request_via_file ... ok
test services::nostr_fs::tests::test_outbox_operations ... ok
test services::nostr_fs::tests::test_inbox_operations ... ok
test services::nostr_fs::tests::test_readdir_root ... ok
test services::nostr_fs::tests::test_identity_readonly ... ok
test services::nostr_fs::tests::test_relays ... ok
```

### Integration Tests (5 new, 17 total)

```
test nostr_tests::test_nostr_capability_in_namespace ... ok
test nostr_tests::test_nostr_event_submission ... ok
test nostr_tests::test_nostr_job_request_submission ... ok
test nostr_tests::test_nostr_agent_workflow ... ok
test nostr_tests::test_nostr_programmatic_api ... ok
```

### Total: 89 tests passing

---

## Files Created/Modified

### New Files

- `src/services/nostr_fs.rs` - NostrFs implementation (~600 lines)

### Modified Files

- `Cargo.toml` - Added `nostr` and `hex` dependencies
- `src/services/mod.rs` - Export NostrFs with feature gate
- `src/lib.rs` - Re-export NostrFs
- `tests/integration.rs` - Added 5 new integration tests
- `README.md` - Added NostrFs documentation
- `docs/ROADMAP.md` - Updated Sprint 5 progress

---

## Dependencies Added

- `nostr` (workspace crate) - Event signing, NIP-90 types
- `hex` (workspace) - Hex encoding for pubkey conversion

Both are optional via the `nostr` feature flag:
```toml
[features]
nostr = ["dep:nostr"]
```

---

## What's Next

Sprint 5 remaining capabilities:
1. **WsFs** - WebSocket connections for relay transport
2. **HttpFs** - HTTP client for web requests

These would complete the networking capabilities and enable full NIP-90 workflows.

---

## Milestone Update

**M3: Agent with Nostr capability** - ✅ COMPLETE

An agent can now:
- Read its Nostr identity from `/cap/nostr/identity/`
- Submit NIP-90 job requests via `/cap/nostr/request`
- Have events signed and queued in outbox
- (With relay connector) Send/receive from Nostr network
