# NIP-28 Default Channel Performance — Jira Tasks

**Epic:** NIP-28 Default Channel Performance Fix
**PRD:** `docs/plans/nip28-channel-performance-prd.md`
**Priority order:** Tasks should be shipped in order 1 → 6. Each is independent but later tasks assume Fix 1 is in place.

---

## PERF-1 — Add `since` time filter to NIP-28 subscription

**Type:** Bug / Performance
**Priority:** P0 — Critical
**Estimate:** S (half day)

### Context

The subscription in `nip28_chat_lane.rs` fetches all historical kind-42 events with no time window. On a channel with 4746+ events the relay delivers the full history on every connect, causing the startup burst that blocks the render thread.

### File

`apps/autopilot-desktop/src/nip28_chat_lane.rs` — `build_filters` (line 84)

### Current behavior

```rust
fn build_filters(channel_ids: &[&str]) -> Vec<serde_json::Value> {
    vec![
        json!({"kinds": [40], "ids": channel_ids}),
        json!({"kinds": [41, 42], "#e": channel_ids, "limit": 512}),
    ]
}
```

No `since` field. Relay delivers entire channel history on every connect.

### Required change

- Accept a `since_epoch_secs: u64` parameter.
- Split kind-40/41 (metadata, no time limit) from kind-42 (messages, time-limited).
- Use `since = now - 86400` (24h) for kind-42.
- Pass current epoch seconds into `build_filters` from `run_nip28_chat_lane_loop`.

### Acceptance criteria

- On a cold connect, relay delivers only events from the last 24 hours for kind-42.
- Kind-40 and kind-41 events are still fetched without a time filter.
- No regression in existing chat message display for recent history.

---

## PERF-2 — Defer `refresh_projection` until EOSE

**Type:** Performance
**Priority:** P0 — Critical
**Estimate:** M (1 day)

### Context

`record_relay_events` calls `refresh_projection` (O(N) sort + rebuild + disk write) immediately on every incoming batch. During initial sync with 64 events/batch this triggers ~74 sequential full rebuilds, each larger than the last. The render thread is blocked the entire time.

### Files

- `apps/autopilot-desktop/src/app_state/chat_projection.rs` — `record_relay_events` (line 340), `refresh_projection` (line 639)
- `apps/autopilot-desktop/src/input/reducers/mod.rs` — `drain_runtime_lane_updates` (line 204)

### Current behavior

`record_relay_events` extends the event list then immediately calls `refresh_projection`, triggering a full rebuild + disk write on every 64-event batch.

### Required change

- Split `record_relay_events` into accumulate-only (no rebuild) and a separate `flush_pending_events` that calls `refresh_projection`.
- In `drain_runtime_lane_updates` in the reducer, call flush only when an `Eose` update arrives, not on every batch.
- Add a dirty flag so a flush is also triggered on shutdown and on explicit user actions (e.g. send message).

### Acceptance criteria

- `refresh_projection` is called once per connect (at EOSE) not once per 64-event batch.
- Total rebuild count during a 4746-event initial sync is 1, not 74.
- Chat and presence data are fully visible after EOSE.
- No data loss if the process exits before EOSE (dirty state handled on shutdown).

---

## PERF-3 — Prune stale presence events before persisting

**Type:** Performance / Correctness
**Priority:** P1 — High
**Estimate:** M (1 day)

### Context

`relay_events` accumulates every presence heartbeat ever received. Presence events expire after 90 seconds (`AUTOPILOT_MAIN_CHANNEL_PRESENCE_STALE_AFTER_SECONDS`). Keeping expired events wastes memory and inflates the disk cache file indefinitely.

### File

`apps/autopilot-desktop/src/app_state/chat_projection.rs` — `refresh_projection` (line 639)

### Required change

- After `normalize_managed_chat_relay_events`, add a pruning pass.
- For each kind-42 event: attempt to parse as `oa.autopilot.presence.v1`. If it parses as presence and its effective `expires_at` (from payload, or `created_at + AUTOPILOT_MAIN_CHANNEL_PRESENCE_STALE_AFTER_SECONDS`) is in the past, remove it.
- Normal kind-42 chat messages (those that do not parse as presence) are always retained.
- Pass current epoch seconds into `refresh_projection` or read it inside.

### Acceptance criteria

- After EOSE on a channel with 4746 events, `relay_events.len()` reflects only non-expired content.
- Presence accuracy is unchanged: the most recent heartbeat per provider is retained until it expires.
- The disk cache file size does not grow across restarts when no new chat messages arrive.
- Existing large local caches are pruned correctly on next startup.

---

## PERF-4 — Throttle disk writes in `refresh_projection`

**Type:** Performance
**Priority:** P1 — High
**Estimate:** S (half day)

### Context

`persist_managed_chat_projection_document` serializes the full event list to JSON and writes to disk on every call to `refresh_projection`. With Fix 2 in place this is reduced to once per EOSE, but any code path that calls `refresh_projection` outside of EOSE (e.g. send message, ack) still writes synchronously on the render thread.

### File

`apps/autopilot-desktop/src/app_state/chat_projection.rs` — `refresh_projection` (line 660)

### Required change

- Add a `needs_persist: bool` dirty flag and `last_persist_at: Instant` to `ManagedChatProjectionState`.
- In `refresh_projection`, set `needs_persist = true` instead of writing immediately.
- Write to disk only when: `needs_persist && last_persist_at.elapsed() > Duration::from_secs(1)`.
- Flush unconditionally on clean shutdown.
- Expose a `tick_persist` method for the render loop to call periodically.

### Acceptance criteria

- Disk writes happen at most once per second under sustained event load.
- No data loss: dirty state is flushed on shutdown.
- No regression in projection load on restart.

---

## PERF-5 — Fix kind-0 pubkey collection scope in reducer

**Type:** Performance
**Priority:** P2 — Medium
**Estimate:** XS (1–2 hours)

### Context

After each event drain the reducer collects author pubkeys by walking all `snapshot.messages.values()` — this is O(total messages) per batch. Only new pubkeys need to be checked since the lane worker deduplicates via `fetched_kind0_pubkeys`.

### File

`apps/autopilot-desktop/src/input/reducers/mod.rs` — lines 251–259

### Current behavior

```rust
let author_pubkeys: Vec<String> = state
    .autopilot_chat
    .managed_chat_projection
    .snapshot
    .messages
    .values()
    .map(|m| m.author_pubkey.clone())
    .collect();
state.nip28_chat_lane_worker.fetch_kind0_if_needed(author_pubkeys);
```

Walks all N messages every drain cycle.

### Required change

Collect pubkeys only from the events in the current batch:

```rust
let new_author_pubkeys: Vec<String> = nip28_relay_events
    .iter()
    .map(|e| e.pubkey.clone())
    .collect();
state.nip28_chat_lane_worker.fetch_kind0_if_needed(new_author_pubkeys);
```

The lane worker's `fetched_kind0_pubkeys` set ensures no duplicate kind-0 subscriptions are issued.

### Acceptance criteria

- Kind-0 metadata is fetched for all unique authors (existing behavior preserved).
- No duplicate kind-0 subscriptions for authors already fetched.
- Per-batch reducer work for this step is O(batch size) not O(total messages).

---

## PERF-6 — Wire incremental peer presence index through call site

**Type:** Performance
**Priority:** P2 — Medium
**Estimate:** S (half day)

### Context

`build_autopilot_peer_presence_index` has a `previous_index` parameter that enables incremental processing — it skips messages already seen in a prior index build. The call site in `app_state.rs` (line 10118) passes `None`, forcing a full replay of all messages every time presence is queried.

### Files

- `apps/autopilot-desktop/src/app_state.rs` — call to `build_autopilot_peer_presence_index` (line 10118)
- `apps/autopilot-desktop/src/app_state/chat_projection.rs` or `AutopilotChatState` — wherever the cached index is stored

### Required change

- Store the last built `AutopilotPeerPresenceIndex` on `AutopilotChatState` (or the appropriate state struct).
- Pass `Some(&cached_index)` to `build_autopilot_peer_presence_index` on subsequent calls.
- Invalidate the cached index when `replace_relay_events` is called (full rebuild cases).

### Acceptance criteria

- After initial EOSE, subsequent presence queries process only new messages (O(new) not O(all)).
- Presence results are identical to the current full-rebuild path.
- Cache is correctly invalidated when relay events are fully replaced (e.g. on reconnect with `replace_relay_events`).

---

## Dependency graph

```
PERF-1 (since filter)
    └─► PERF-2 (defer rebuild)
            └─► PERF-3 (prune stale presence)
                    └─► PERF-4 (throttle disk writes)

PERF-5 (kind-0 scope)   — independent, ship any time
PERF-6 (incremental index) — independent, ship after PERF-2
```

PERF-1 and PERF-2 together eliminate the startup hang. PERF-3 and PERF-4 address the long-term growth problem. PERF-5 and PERF-6 clean up per-cycle overhead.
