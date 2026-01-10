# Nexus MVP (v0.1)

Minimum viable relay to support Pylon v0.1 end-to-end job flow.

## Scope

**In scope:**
- NIP-90 Data Vending Machine events
- NIP-89 Handler announcements
- NIP-42 Authentication (agent identity)
- Basic Nostr relay functionality (NIP-01)
- BOLT-11 Lightning payment flow

**Out of scope for v0.1:**
- NIP-60/61 Cashu/Nutzaps
- NIP-34 Git events
- NIP-32 Labels/Reputation
- Horizontal scaling

---

## Required Event Kinds

### NIP-90: Job Marketplace

| Kind | Direction | Purpose |
|------|-----------|---------|
| `5000-5999` | Buyer → Relay | Job request |
| `6000-6999` | Provider → Relay | Job result (kind = request + 1000) |
| `7000` | Provider → Relay | Job feedback |

**Pylon uses `kind:5050`** for text inference jobs.

### NIP-89: Handler Discovery

| Kind | Direction | Purpose |
|------|-----------|---------|
| `31990` | Provider → Relay | Handler announcement (addressable) |

### NIP-42: Authentication

| Kind | Direction | Purpose |
|------|-----------|---------|
| `22242` | Client → Relay | Auth event (ephemeral, not stored) |

**AUTH message flow:**
1. Relay sends: `["AUTH", "<challenge>"]`
2. Client signs kind:22242 event with `relay` and `challenge` tags
3. Client sends: `["AUTH", <signed-event>]`
4. Relay validates and responds: `["OK", "<event-id>", true, ""]`

---

## Event Flow

```
┌────────┐                    ┌────────┐                    ┌────────┐
│ Buyer  │                    │ NEXUS  │                    │Provider│
└───┬────┘                    └───┬────┘                    └───┬────┘
    │                             │                             │
    │ [WebSocket connect]         │       [WebSocket connect]   │
    │────────────────────────────▶│◀────────────────────────────│
    │                             │                             │
    │ AUTH challenge              │         AUTH challenge      │
    │◀────────────────────────────│────────────────────────────▶│
    │                             │                             │
    │ AUTH kind:22242             │         AUTH kind:22242     │
    │────────────────────────────▶│◀────────────────────────────│
    │                             │                             │
    │ OK (authenticated)          │         OK (authenticated)  │
    │◀────────────────────────────│────────────────────────────▶│
    │                             │                             │
    │ 1. REQ kind:31990           │                             │
    │────────────────────────────▶│                             │
    │                             │                             │
    │ 2. EVENT (handler list)     │                             │
    │◀────────────────────────────│                             │
    │                             │                             │
    │ 3. EVENT kind:5050 (job)    │                             │
    │────────────────────────────▶│                             │
    │                             │ 4. EVENT kind:5050          │
    │                             │────────────────────────────▶│
    │                             │                             │
    │                             │ 5. EVENT kind:7000          │
    │                             │   (payment-required)        │
    │                             │◀────────────────────────────│
    │ 6. EVENT kind:7000          │                             │
    │◀────────────────────────────│                             │
    │                             │                             │
    │ [Buyer pays BOLT-11 invoice outside Nostr]                │
    │                             │                             │
    │                             │ 7. EVENT kind:6050          │
    │                             │   (result)                  │
    │                             │◀────────────────────────────│
    │ 8. EVENT kind:6050          │                             │
    │◀────────────────────────────│                             │
    │                             │                             │
```

---

## Subscription Filters

Nexus must efficiently handle these subscription patterns:

### Provider subscribes to job requests
```json
["REQ", "jobs", {"kinds": [5050], "since": <now>}]
```

Or with specific provider tag:
```json
["REQ", "jobs", {"kinds": [5050], "#p": ["<provider-pubkey>"]}]
```

### Buyer subscribes to feedback/results
```json
["REQ", "results", {
  "kinds": [6050, 7000],
  "#e": ["<job-request-id>"],
  "#p": ["<buyer-pubkey>"]
}]
```

### Discover handlers
```json
["REQ", "handlers", {"kinds": [31990], "#k": ["5050"]}]
```

### Optional semantic intent filter (extension)

Nexus supports an optional `intent` field on filters for semantic routing when
DSPy is configured:

```json
["REQ", "jobs", {"intent": "job_request", "since": 1720000000}]
```

If DSPy is unavailable, intent filtering falls back to kind-based matching.

---

## Event Validation

### Job Request (kind:5000-5999)
- Must have valid signature
- Should have `i` tag (input data)
- May have `bid` tag (max payment in msats)
- May have `relays` tag (where to publish results)
- May have `p` tag (specific provider)

### Job Feedback (kind:7000)
- Must have valid signature
- Must have `e` tag referencing job request
- Must have `p` tag with customer pubkey
- Must have `status` tag: `payment-required`, `processing`, `error`, `success`, `partial`
- If `payment-required`, must have `amount` tag with BOLT-11 invoice

### Job Result (kind:6000-6999)
- Must have valid signature
- Must have `e` tag referencing job request
- Must have `p` tag with customer pubkey
- Should have `request` tag with original request JSON
- Content is the result payload

### Semantic Classification (DSPy)
- **Event intent**: classify intent from kind/content/tags
- **Job kind**: classify job type + complexity from request payloads

### Handler Announcement (kind:31990)
- Addressable/replaceable event
- Must have `d` tag (unique identifier)
- Must have one or more `k` tags (supported job kinds)
- Content is optional metadata JSON

### Auth Event (kind:22242)
- Must have valid signature
- Must have `relay` tag matching relay URL
- Must have `challenge` tag matching sent challenge
- `created_at` must be within 10 minutes of current time
- **Not stored** — ephemeral, used only for session auth

---

## Storage Requirements

### Hot Storage (in-memory or fast cache)
- Active job requests (kind:5xxx) from last 24h
- Active feedback (kind:7000) from last 24h
- All handler announcements (kind:31990)

### Persistent Storage (SQLite)
- Job results (kind:6xxx) for 7 days
- Handler announcements (kind:31990) forever
- Index: `kind`, `pubkey`, `created_at`, `#e`, `#p`, `#k`

### Retention Policy
```
kind:5000-5999  → 24 hours (auto-expire)
kind:6000-6999  → 7 days
kind:7000       → 24 hours (auto-expire)
kind:31990      → forever (addressable, replaced not duplicated)
```

---

## Implementation Checklist

### Phase 0: Project Setup (Cloudflare)
- [x] Create `crates/nexus/worker/` directory structure
- [x] Set up wrangler.toml with D1 + Durable Objects
- [x] Create D1 schema (events table + indexes)
- [x] Reuse protocol code from `crates/relay-worker/`:
  - [x] `nip01.rs` (ClientMessage, RelayMessage)
  - [x] `subscription.rs` (Filter matching)
  - [x] `storage.rs` (adapt for D1)
- [x] Basic worker entry point

### Phase 1: Basic Relay
- [x] Durable Object for WebSocket handling
- [x] NIP-01: Basic protocol (EVENT, REQ, CLOSE, OK, EOSE)
- [x] Event signature validation (stub for WASM, real for native)
- [x] D1 storage backend
- [x] Subscription management with DO state

### Phase 2: NIP-90 Support
- [x] Accept kind:5000-5999 (job requests)
- [x] Accept kind:6000-6999 (job results)
- [x] Accept kind:7000 (job feedback)
- [x] Filter by `#e` tag (job reference)
- [x] Filter by `#p` tag (pubkey targeting)
- [ ] 24h auto-expiry for jobs/feedback

### Phase 3: NIP-89 Support
- [x] Accept kind:31990 (handler announcements)
- [ ] Handle addressable event replacement (same `d` tag)
- [ ] Filter by `#k` tag (supported job kinds)

### Phase 4: NIP-42 Authentication
- [x] Generate random challenge on connect
- [x] Send `["AUTH", "<challenge>"]` to new connections
- [x] Parse incoming `["AUTH", <event>]` messages
- [ ] Validate kind:22242 auth events:
  - [ ] Verify signature
  - [x] Check `relay` tag matches our URL
  - [x] Check `challenge` tag matches sent challenge
  - [x] Check `created_at` within 10 minutes
- [x] Track authenticated pubkey per connection
- [x] Return `["OK", "<event-id>", true, ""]` on success
- [x] Return `["OK", "<event-id>", false, "auth-required: <reason>"]` on failure

### Phase 5: Production Ready
- [x] NIP-11 relay information document
- [ ] Rate limiting (higher limits for authenticated connections)
- [ ] Metrics endpoint
- [ ] TLS termination
- [x] Health check endpoint

---

## Test Scenarios

### Scenario 1: Provider Registration
1. Provider publishes `kind:31990` handler announcement
2. Verify it's stored and queryable by `#k` tag

### Scenario 2: Job Submission
1. Buyer queries for handlers (`kind:31990`, `#k:5050`)
2. Buyer publishes `kind:5050` job request
3. Provider receives job via subscription
4. Provider publishes `kind:7000` with `payment-required`
5. Buyer receives feedback
6. Buyer pays invoice (external)
7. Provider publishes `kind:6050` result
8. Buyer receives result

### Scenario 3: Broadcast Job
1. Buyer publishes `kind:5050` without `#p` tag
2. Multiple providers receive via `kind:5050` subscription
3. First provider to respond wins

### Scenario 4: Authentication
1. Client connects via WebSocket
2. Relay sends `["AUTH", "<challenge>"]`
3. Client signs kind:22242 with relay URL and challenge tags
4. Client sends `["AUTH", <event>]`
5. Relay validates signature, relay tag, challenge tag, timestamp
6. Relay responds `["OK", "<event-id>", true, ""]`
7. Client is now authenticated with their pubkey

### Scenario 5: Auth Failure
1. Client connects and receives challenge
2. Client sends auth event with wrong challenge
3. Relay responds `["OK", "<event-id>", false, "auth-required: challenge mismatch"]`
4. Client remains unauthenticated

---

## Configuration

### Cloudflare Workers (v0.1 Target)

**wrangler.toml:**
```toml
name = "nexus"
main = "build/index.js"
compatibility_date = "2024-01-01"

[vars]
RELAY_URL = "wss://nexus.openagents.com"
AUTH_REQUIRED = "true"

[[d1_databases]]
binding = "DB"
database_name = "nexus"
database_id = "<your-d1-id>"

[[durable_objects.bindings]]
name = "NEXUS_RELAY"
class_name = "NexusRelay"

[[migrations]]
tag = "v1"
new_classes = ["NexusRelay"]
```

### Native Rust (Future)

```toml
[nexus]
bind = "0.0.0.0:443"
domain = "nexus.openagents.com"

[storage]
path = "./data/nexus.db"

[limits]
max_subscriptions = 100
max_message_size = 65536
job_retention_hours = 24
result_retention_days = 7

[tls]
cert = "/etc/letsencrypt/live/nexus.openagents.com/fullchain.pem"
key = "/etc/letsencrypt/live/nexus.openagents.com/privkey.pem"
```

**See:** `BACKENDS.md` for full deployment architecture

---

## Success Criteria

Pylon v0.1 is unblocked when:

1. ✅ `pylon start -m provider` can connect, authenticate, and subscribe to jobs
2. ✅ `pylon rlm "what is 2+2"` publishes kind:5940 and receives kind:6940 result
3. ✅ Provider's kind:6940 reaches buyer within 5 seconds
4. ✅ WebSocket connections stay alive with ping/pong keep-alive
5. ✅ Both buyer and provider authenticate successfully (NIP-42)
6. Handler discovery works: `#k:5050` returns provider's `kind:31990`

---

## Recent Fixes (2026-01-08)

### WebSocket Keep-Alive

Cloudflare terminates idle WebSocket connections after ~100 seconds. Fixed with:

**Server-side (relay_do.rs):**
```rust
fn setup_websocket_auto_response(state: &State) {
    if let Ok(pair) = WebSocketRequestResponsePair::new("ping", "pong") {
        state.set_websocket_auto_response(&pair);
    }
}
```

**Client-side (nostr-client):**
- Added `ping_task` field to RelayConnection
- Sends "ping" text message every 30 seconds
- Works with Cloudflare's edge auto-response

### DvmClient Race Condition

Fixed race condition where job results arrived before `await_result` was called:

**Problem:** Result channel was created in `await_result`, but events could arrive before it was called.

**Solution:** Pre-create result channel in `subscribe_to_job_events` (called from `submit_job`):
1. Create `(tx, rx)` channel
2. Insert `tx` into `pending_results` **before** subscribing
3. Store `rx` in `result_receivers`
4. `await_result` retrieves pre-created receiver

### RLM Support

Full end-to-end support for RLM queries:
- Kind 5940: RLM sub-query request
- Kind 6940: RLM result
- `pylon rlm` command documented in CLI.md
