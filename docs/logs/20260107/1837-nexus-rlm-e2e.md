# Nexus RLM E2E Test Log

**Date:** 2026-01-07 18:37
**Status:** in-progress

## Summary

Testing RLM (Recursive Language Model) queries via Pylon → Nexus → Pylon provider.

## Provider Setup

Updated config to prefer nexus relay and apple_fm backend:

```toml
relays = [
    "wss://nexus.openagents.com",
    "wss://relay.damus.io",
    "wss://nos.lol",
]
min_price_msats = 0
require_payment = false
backend_preference = [
    "apple_fm",
    "ollama",
    "llamacpp",
]
```

## Fixed Issues

### 1. Subscription filter missing kind:5940

**File:** `crates/compute/src/services/relay_service.rs`

The job subscription filter didn't include kind 5940 for RLM sub-queries:

```rust
// Before
"kinds": [5000, 5001, 5002, 5003, 5004, 5005, 5050, 5100, 5250]

// After
"kinds": [5000, 5001, 5002, 5003, 5004, 5005, 5050, 5100, 5250, 5940]
```

## Test Results

### Regular NIP-90 Jobs (kind 5050)
✅ Working - Provider receives and completes jobs, results published

### RLM Jobs (kind 5940)

#### Provider Side
✅ Receives kind:5940 job requests
✅ Processes jobs via Apple FM backend
✅ Publishes results (kind:6940) to 3/3 relays

Example:
```
Received job request event: a5bd250c683090c27d87b1fd412f7b40096a128e656c90d9b893076f71d9c8cf (kind: 5940)
Processing job job_a5bd250c683090c2 with prompt: Name one color
Job job_a5bd250c683090c2 completed, 13 tokens
Published event 0289d6b7284612cc6b3711ae145e1cd58c55cb06ae2f6e7d2d38ebdfec27ed0f to 3/3 relays
```

#### Client Side
❌ Client times out waiting for result
- Subscribes to `{"kinds": [6940], "#e": [job_id]}`
- Result IS stored in Nexus (stats show 5+ RLM results)
- But subscription doesn't receive the event

### Stats API
```json
{
  "rlm": {
    "subqueries_total": 5,
    "subqueries_24h": 5,
    "results_total": 5,
    "results_24h": 5,
    "providers_active": 1
  }
}
```

## Current Issue

**Problem:** RLM results are stored in Nexus D1 database but not delivered to subscribed clients.

**Hypothesis:** The Durable Object (relay_do.rs) may not be forwarding stored events to matching subscriptions. When a result is published after the subscription is created, the relay needs to either:
1. Match it against active subscriptions and push to WebSocket clients
2. Return stored events that match the filter on EOSE

**Next Steps:**
1. Check `crates/nexus/worker/src/relay_do.rs` subscription handling
2. Verify events are being matched against subscriptions
3. Test with wscat/websocat to manually query for results

## Commits

- Added kind:5940 to relay service subscription filter
