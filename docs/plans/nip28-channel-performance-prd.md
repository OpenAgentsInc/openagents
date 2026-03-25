# PRD: NIP-28 Default Channel Performance

**Status:** Draft
**Date:** 2026-03-24
**Owner:** Engineering

---

## Problem

The default NIP-28 channel (`ebf2e35092632ecb81b0f7da7d3b25b4c1b0e8e7eb98d7d766ef584e9edd68c8` on `wss://relay.damus.io`) serves three simultaneous functions:

1. User-facing group chat ("General group")
2. Compute presence heartbeat bus — every provider node publishes `oa.autopilot.presence.v1` kind-42 messages every 30 seconds
3. Peer roster source — the app scans all messages in this channel to determine who is online and eligible for compute

Because public relays retain all messages indefinitely and the subscription has no time-window filter, the channel accumulates thousands of stale presence heartbeats. Observed state: **4746 cached events**, growing at ~2880 events/day per active provider.

On startup the app fetches all historical events and performs a full O(N) sort + rebuild + disk write on every 64-event batch. With 4746 events this results in ~74 sequential full rebuilds before EOSE, each one larger than the last. The render thread is blocked the entire time, causing a visible hang.

The problem worsens with channel age. It is not self-correcting.

---

## Goals

- Eliminate the startup hang on the default channel regardless of channel history length.
- Stop unbounded memory and disk growth caused by stale presence events.
- Keep presence accuracy intact (providers correctly show online/offline).
- Keep chat history display intact (recent messages still visible).
- Degrade gracefully as the channel grows older.

## Non-Goals

- Migrating to a different channel or relay.
- Changing the presence heartbeat interval.
- Paginated chat history / infinite scroll (out of scope for this fix).

---

## User Impact

| Symptom | Root Cause |
|---------|-----------|
| App hangs on startup for several seconds | 74 full projection rebuilds during initial event burst |
| Chat pane slow to become interactive | Render thread blocked by rebuild + disk I/O loop |
| Memory grows over time without restart | Stale presence events accumulate in `relay_events` |
| Disk write file grows with channel age | All events serialized to JSON on every refresh |
| Worsens for all users as channel ages | No time-window filter; relay delivers full history |

---

## Solution Overview

Six targeted fixes, ordered by impact. Each is independent and can ship separately.

### Fix 1 — Add `since` filter to NIP-28 subscription
**File:** `apps/autopilot-desktop/src/nip28_chat_lane.rs`

Split the subscription into two filters with a time window:
- Channel create/metadata (kinds 40, 41): no time limit, tiny volume
- Chat messages (kind 42): `since = now - 86400` (24h)

Eliminates 90%+ of initial relay load. Turns a 4746-event burst into dozens. Channel age no longer affects startup time.

### Fix 2 — Defer `refresh_projection` until EOSE
**Files:** `apps/autopilot-desktop/src/app_state/chat_projection.rs`, `apps/autopilot-desktop/src/input/reducers/mod.rs`

`record_relay_events` currently calls `refresh_projection` (sort + rebuild + disk write) on every incoming batch. Change it to accumulate events, then flush once at EOSE.

Turns 74 rebuilds per connect into 1.

### Fix 3 — Prune stale presence events before persisting
**File:** `apps/autopilot-desktop/src/app_state/chat_projection.rs`

After normalize, remove kind-42 events whose `oa.autopilot.presence.v1` TTL has expired (`expires_at < now`, or `created_at + 90s < now` as fallback). Normal chat messages (kind-42 without presence content) are kept.

Stops unbounded memory and disk growth. Idempotent — safe to run on existing large caches.

### Fix 4 — Throttle disk writes
**File:** `apps/autopilot-desktop/src/app_state/chat_projection.rs`

Add a dirty flag and minimum write interval (1 second). Write to disk on EOSE, on clean shutdown, or on a timer tick rather than on every incoming event batch.

Removes blocking file I/O from the hot event path.

### Fix 5 — Fix kind-0 pubkey collection scope
**File:** `apps/autopilot-desktop/src/input/reducers/mod.rs`

After each event drain the reducer collects author pubkeys from ALL snapshot messages to pass to `fetch_kind0_if_needed`. Change to collect only from the new events just received. The lane worker deduplicates with `fetched_kind0_pubkeys` so correctness is unchanged.

Drops per-batch work from O(total messages) to O(batch size).

### Fix 6 — Wire incremental peer presence index
**File:** `apps/autopilot-desktop/src/app_state.rs`

`build_autopilot_peer_presence_index` has an incremental path (`previous_index` parameter) that skips already-processed messages. The call site in `app_state.rs` passes `None`, forcing a full replay every time. Wire the cached index through.

Drops presence scanning from O(all messages) to O(new messages) per cycle.

---

## Impact Summary

| Fix | Scope | Impact |
|-----|-------|--------|
| 1. `since` filter | Relay subscription | Eliminates 90%+ of initial load |
| 2. Defer rebuild to EOSE | Projection rebuild | 74 rebuilds → 1 per connect |
| 3. Prune stale presence | Event retention | Stops unbounded memory/disk growth |
| 4. Throttle disk writes | Persistence | Removes blocking I/O from hot path |
| 5. Fix kind-0 collection | Reducer loop | O(N) per batch → O(batch size) |
| 6. Wire incremental index | Peer roster | Presence scan O(new) not O(all) |

---

## Acceptance Criteria

- App becomes interactive within 2 seconds of chat pane open on a channel with 5000+ historical events.
- Memory footprint of `relay_events` does not grow after EOSE when no new messages arrive.
- Presence accuracy unchanged: providers show online within one heartbeat interval (30s) of going online.
- Chat history shows messages from the last 24 hours correctly.
- Disk write file size does not grow unboundedly across restarts.
