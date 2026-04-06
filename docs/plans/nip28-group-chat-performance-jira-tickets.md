# Jira Tickets — NIP-28 Group Chat Performance

Use this as the current issue breakdown for managed NIP-28 group chat performance work in `apps/autopilot-desktop`.

This backlog is based on the `2026-03-27` code review of the current reducer, lane worker, projection rebuild, and chat pane render path. It is scoped to the retained MVP codebase and follows the current crate ownership split:

- keep product behavior in `apps/autopilot-desktop`
- do not move app-specific chat workflow into `crates/wgpui` or `crates/nostr/core`

This doc is intentionally narrower than the older default-channel performance notes. It covers the issues that still exist in the current tree.

---

## Epic

- **Title:** `NIP-28: remove managed group chat steady-state lag and relay churn`
- **Goal:** Make NIP-28 managed group chat scale with recent change rather than total retained history.
- **Success measure:** Busy channels stay responsive during normal use, live traffic does not trigger repeated relay resubscribe churn, and large retained histories do not force full projection or render work on every loop.

### Out of scope

- redesigning NIP-28 protocol semantics
- moving managed chat into a separate crate
- rebuilding the entire chat UI
- introducing full infinite-scroll history pagination in this pass

---

## Progress

| Ticket | Status | Notes |
|-------|--------|-------|
| P1 | ✅ Done | Reducer-side pre-send dedupe landed in `Nip28ChatLaneWorker`. `sync_managed_chat_subscriptions(&mut self, ...)` now normalizes inputs, compares against cached `ManagedChatSubscriptionSyncRequest`, and suppresses identical dispatches. Worker-side dedupe in `handle_command()` kept as second line of defense. 7 unit tests pass: `cargo test -p autopilot-desktop --lib -- nip28_chat_lane::tests::sync_managed_chat_subscriptions`. |
| P2 | ✅ Done | `handle_command()` no longer sets `subscriptions_dirty` on cursor-only changes. `since_created_at` still updates in worker state for future reconnect backfill, but only relay or channel set changes trigger `replace_subscription()`. Reconnect-on-error and missing-subscription paths unchanged. 4 unit tests pass: `cargo test -p autopilot-desktop --lib -- nip28_chat_lane::tests::handle_command`. |
| P3 | ✅ Done | `record_relay_events()` no longer triggers full rebuild+persist inline. Sets `projection_dirty` flag instead; `flush_if_dirty()` runs once per frame in the reducer after all events are recorded. Persistence decoupled from `refresh_projection()` via `persist_dirty` flag with 2s throttle in `persist_if_dirty()`. Redundant normalize removed from `persist_managed_chat_projection_document()`. Shutdown path calls `flush_persist()`. 19 tests pass: `cargo test -p autopilot-desktop --lib -- chat_projection::tests managed_chat_projection chat_regression_tests chat_state_browses`. |

---

## Ticket P1 — Stop reducer-side duplicate subscription sync dispatch

**Type:** Performance / Bug
**Priority:** P0 — Critical
**Estimate:** S
**Status:** Done

### Implementation

- Added `ManagedChatSubscriptionSyncRequest` as a normalized internal request type with `PartialEq`/`Eq`.
- Added `last_sync_request: Option<ManagedChatSubscriptionSyncRequest>` to `Nip28ChatLaneWorker`.
- `sync_managed_chat_subscriptions(&mut self, ...)` normalizes inputs via existing `normalize_relay_urls()` and `normalize_channel_ids()`, compares against the cached request, and skips dispatch when identical.
- Request is only cached after a successful `command_tx.send()`, so send failures are retried on the next call.
- Worker-side dedupe in `handle_command()` is preserved as a second line of defense.
- Reducer call site in `drain_runtime_lane_updates()` required no changes — `state` is already `&mut`.

### Verification

- 7 unit tests added and passing:
  - `sync_managed_chat_subscriptions_first_send_dispatches`
  - `sync_managed_chat_subscriptions_identical_request_suppressed`
  - `sync_managed_chat_subscriptions_equivalent_input_suppressed_after_normalization`
  - `sync_managed_chat_subscriptions_resend_when_relays_change`
  - `sync_managed_chat_subscriptions_resend_when_channels_change`
  - `sync_managed_chat_subscriptions_resend_when_since_changes`
  - `sync_managed_chat_subscriptions_retry_after_send_failure`
- `cargo check -p autopilot-desktop` — clean
- `cargo test -p autopilot-desktop --lib -- nip28_chat_lane::tests::sync_managed_chat_subscriptions` — 7/7 pass

### Summary

The reducer sends `SyncManagedChatSubscriptions` every loop even when the desired relay set, channel set, and effective subscription window have not changed.

### Problem

Worker-side dedupe preserves correctness, but the app still pays for avoidable command allocation, queue traffic, normalization, and equality checks on every reducer pass. Under load this adds avoidable pressure to the NIP-28 lane and makes debugging command flow noisier than it should be.

### Current behavior

`drain_runtime_lane_updates` always calls:

- `configured_relay_urls()`
- `managed_chat_projection.discovered_channel_ids()`
- `managed_chat_projection.subscription_since_created_at(...)`
- `nip28_chat_lane_worker.sync_managed_chat_subscriptions(...)`

even when the derived tuple is unchanged.

### Required change

- Add pre-send dedupe before sending `SyncManagedChatSubscriptions`.
- Store the last sent normalized subscription request on `Nip28ChatLaneWorker`.
- Only send a new command when one of these inputs changes materially:
  - normalized relay URLs
  - normalized channel IDs
  - effective backfill cursor
- Keep worker-side dedupe in place as a second line of defense.

### Acceptance criteria

- No `SyncManagedChatSubscriptions` command is sent during idle steady-state when relays, channels, and cursor are unchanged.
- Reducer-side behavior remains correct when relays or discovered channel IDs change.
- Existing subscription sync behavior still works on cold start, reconnect, and channel discovery.

### Files

- `apps/autopilot-desktop/src/input/reducers/mod.rs`
- `apps/autopilot-desktop/src/nip28_chat_lane.rs`
- `apps/autopilot-desktop/src/app_state/chat_projection.rs`

### Notes

- This ticket is about reducer-to-lane command traffic only.
- Do not fold the larger resubscribe-cursor fix into this ticket. That is tracked separately in P2.

---

## Ticket P2 — Stabilize the managed chat backfill cursor and stop live resubscribe churn

**Type:** Performance / Bug
**Priority:** P0 — Critical
**Estimate:** M
**Status:** Done

### Implementation

- Changed `handle_command()` in `nip28_chat_lane.rs` so that `subscriptions_dirty` is
  only set when `relay_changed || channel_changed`, not on `since_changed` alone.
- `state.since_created_at` still updates on cursor-only changes so future reconnects
  use the latest backfill cursor.
- Reconnect-on-error path (`poll_events()` recv error handler) and
  `missing_subscription` cold-start path remain unchanged — both correctly trigger
  resubscription.
- 4 unit tests added: `handle_command_since_only_change_does_not_set_dirty`,
  `handle_command_relay_change_sets_dirty`, `handle_command_channel_change_sets_dirty`,
  `handle_command_since_only_change_still_updates_cursor`.

### Verification

- `cargo check -p autopilot-desktop` — clean
- `cargo test -p autopilot-desktop --lib -- nip28_chat_lane::tests` — 14/14 pass
  (3 existing + 7 P1 + 4 P2)

### Summary

The current `since_created_at` value tracks the newest seen event timestamp minus overlap. On an active channel that means the subscription cursor changes as traffic arrives, and the worker treats that as a reason to replace relay subscriptions.

### Problem

This is more expensive than duplicate no-op command traffic. Once the cursor moves, the worker marks subscriptions dirty and calls `replace_subscription`, which unsubscribes and resubscribes per relay. On a busy channel this can create repeated subscription churn, backfill overlap replay, and relay-side work during ordinary steady-state chat traffic.

### Current behavior

- `ManagedChatProjectionState::subscription_since_created_at()` derives the cursor from the newest event in retained history.
- The reducer sends that value to the worker.
- `handle_command()` in `nip28_chat_lane.rs` treats any `since_changed` value as a subscription change.
- `reconcile_connections()` then replaces subscriptions on connected relays.

### Required change

- Separate the steady-state live subscription from the reconnect/backfill cursor.
- Define explicit rules for when the backfill cursor is allowed to change:
  - cold start
  - relay reconnect after disconnect
  - discovered channel set changes
  - explicit manual refresh or catch-up action, if one exists later
- Do not resubscribe just because newer kind-42 events arrived on an already-connected subscription.
- Preserve overlap-based catch-up on reconnect so short disconnects do not create message gaps.
- Add instrumentation or trace points that make it obvious when a subscription replacement actually happens and why.

### Acceptance criteria

- After initial connect completes, ongoing inbound message traffic does not trigger unsubscribe/resubscribe cycles by itself.
- `replace_subscription()` is only reached when relays change, channel IDs change, or the lane is reconnecting and needs catch-up.
- Reconnect still backfills with overlap and does not drop recent messages across short disconnects.
- Subscription IDs remain stable during ordinary steady-state traffic.

### Files

- `apps/autopilot-desktop/src/app_state/chat_projection.rs`
- `apps/autopilot-desktop/src/input/reducers/mod.rs`
- `apps/autopilot-desktop/src/nip28_chat_lane.rs`

### Notes

- This ticket is the main fix for relay churn.
- Keep the overlap model. The bug is not that overlap exists. The bug is that it is being recomputed into the active subscription every time traffic advances the high-water mark.

---

## Ticket P3 — Batch relay ingestion and remove full projection rebuild plus persist from the hot path

**Type:** Performance
**Priority:** P1 — High
**Estimate:** L
**Status:** Done

### Summary

`record_relay_events()` currently triggers a full normalize, full projection rebuild, outbound reconciliation, and synchronous JSON persist whenever a relay batch is recorded.

### Problem

This is one of the most expensive current paths in managed chat. Normal inbound traffic can force:

- full sort and dedupe of retained relay events
- full projection rebuild
- full outbound reconciliation against retained relay IDs
- full JSON encode and file rewrite

That work runs in the same reducer-driven path that is supposed to keep the app responsive.

### Current behavior

- The reducer accumulates `nip28_relay_events`.
- `record_relay_events()` extends `relay_events` and immediately calls `refresh_projection()`.
- `refresh_projection()` normalizes the full retained history, rebuilds the full snapshot, and persists the document synchronously.

### Required change

- Split relay ingestion into two phases:
  - append new relay events to a pending buffer
  - flush pending events into the projection on a bounded trigger
- Introduce a flush policy suitable for MVP:
  - flush on EOSE
  - flush on a short debounce under sustained inbound traffic
  - flush immediately for user-visible local state changes that need instant feedback, if required
- Decouple persistence from every flush:
  - maintain a dirty flag
  - throttle or schedule disk writes instead of writing on every rebuild
  - ensure dirty state is flushed on clean shutdown
- Preserve restart safety. Do not trade responsiveness for silent data loss.

### Acceptance criteria

- Initial multi-batch relay sync does not perform a full normalize + rebuild + persist for every relay batch.
- Under sustained inbound traffic, projection rebuild frequency is bounded and intentionally controlled.
- Disk writes are throttled and no longer occur synchronously for every relay batch.
- The visible managed chat snapshot remains correct after EOSE and after local send ack/error transitions.
- Restart still restores retained managed chat state correctly.

### Files

- `apps/autopilot-desktop/src/app_state/chat_projection.rs`
- `apps/autopilot-desktop/src/input/reducers/mod.rs`

### Notes

- Keep the persisted document format unless a schema change is clearly necessary.
- If this needs to split into two implementation PRs, do it as:
  - batching and flush policy
  - persistence throttling and shutdown flush

---

## Ticket P4 — Rebuild managed chat channel and message indexes in one pass

**Type:** Performance
**Priority:** P1 — High
**Estimate:** M

### Summary

`rebuild_managed_chat_projection()` repeatedly rescans the full `messages` map per channel to rebuild `message_ids`, `root_message_ids`, unread counts, and mention counts.

### Problem

As retained history grows, rebuild cost scales poorly. The current path effectively compounds total work by channel count:

- build the full `messages` map
- then, for each live channel, walk all messages again to find that channel's messages
- then rescan channel-local message IDs for unread and mention counts

This is avoidable. The rebuild can derive channel-local message lists in the same pass that creates messages.

### Required change

- Refactor rebuild logic so message-to-channel grouping happens once while ingesting relay events and outbound local echoes.
- Maintain channel-local collections during the rebuild instead of re-filtering `messages.values()` per channel.
- Preserve current behavior for:
  - reply threading
  - reaction summaries
  - unread counts
  - mention counts
  - latest message selection
- Keep output ordering stable.

### Acceptance criteria

- `rebuild_managed_chat_projection()` no longer filters the full `messages` map once per channel to construct `message_ids`.
- Snapshot contents for groups, channels, messages, unread counts, mention counts, replies, and reactions remain behaviorally equivalent.
- Rebuild cost scales roughly with total messages plus total channels, not total channels multiplied by total messages.

### Files

- `apps/autopilot-desktop/src/app_state/chat_projection.rs`

### Notes

- Do not convert this ticket into a broad projection rewrite.
- Favor a focused internal refactor that keeps public state shapes stable.

---

## Ticket P5 — Make kind-0 metadata fetch incremental and add a bounded missing-author bootstrap

**Type:** Performance / Usability
**Priority:** P2 — Medium
**Estimate:** S

### Summary

The reducer currently scans all projected messages after each relay batch to collect author pubkeys for kind-0 fetch. That is unnecessary per-batch work. At the same time, the app still needs a safe way to backfill missing author metadata for already-retained history.

### Problem

The current fetch trigger does this after each relay batch:

- walk `snapshot.messages.values()`
- clone every `author_pubkey`
- send the full list to the lane

Worker-side dedupe via `fetched_kind0_pubkeys` preserves correctness, but CPU work still scales with total message count rather than the current delta.

### Required change

- Change the normal ingestion path to collect author pubkeys from the current relay batch only.
- Deduplicate within the batch before sending to the worker.
- Add one bounded bootstrap path for retained history authors that are still missing metadata:
  - run on initial managed chat load, post-EOSE, or another explicit one-shot point
  - do not run every reducer loop
- Keep worker-side session dedupe via `fetched_kind0_pubkeys`.

### Acceptance criteria

- Per-batch kind-0 trigger cost scales with batch size, not total projected message count.
- Author metadata still fills in for newly seen authors.
- Missing author metadata from retained history can still be backfilled through a bounded one-shot bootstrap path.
- No duplicate kind-0 fetch requests are sent for pubkeys already fetched in the session.

### Files

- `apps/autopilot-desktop/src/input/reducers/mod.rs`
- `apps/autopilot-desktop/src/nip28_chat_lane.rs`
- `apps/autopilot-desktop/src/app_state/chat_projection.rs`

### Notes

- This ticket should preserve current user-visible behavior for author labels.
- Do not introduce an always-on background scan of the full history.

---

## Ticket P6 — Add a managed chat transcript layout cache and stop reparsing unchanged markdown every paint

**Type:** Performance / UI
**Priority:** P1 — High
**Estimate:** M

### Summary

Managed chat currently reparses and remeasures markdown-heavy message content across paint and height-calculation paths for the active channel. The managed-system transcript already has a cache. Managed chat does not.

### Problem

For the active managed channel, the current UI path does expensive repeated work:

- `transcript_content_height()` parses and measures markdown for every visible managed message
- the paint path parses and renders markdown again for those same messages
- large active channels therefore get slower to render even when no message content changed

This is a likely contributor to perceived lag after projection work is fixed.

### Required change

- Add a managed-chat row layout cache similar in spirit to the existing managed-system cache.
- Cache at least:
  - parsed markdown document
  - measured markdown height
  - attachment extraction and attachment height
  - total row height
- Use a cache key that changes when layout-relevant inputs change, such as:
  - message event ID
  - width
  - any delivery or reply state that changes row height
- Reuse cached layout in both height calculation and paint.
- Keep the rendered output identical unless a deliberate UI change is part of the implementation.

### Acceptance criteria

- Unchanged managed chat rows do not reparse markdown on every frame.
- Transcript height calculation and paint share cached row layout data.
- Large managed channels remain responsive while scrolling and while idle.
- Managed-system transcript behavior is unchanged.

### Files

- `apps/autopilot-desktop/src/panes/chat.rs`
- `apps/autopilot-desktop/src/app_state.rs`

### Notes

- If a simple row cache is not enough, a follow-on virtualization ticket can be added later.
- Do not expand this into a full chat-pane redesign.

---

## Suggested shipping order


| Order | Ticket                                                         | Why                                                      |
| ----- | -------------------------------------------------------------- | -------------------------------------------------------- |
| 1     | P2 — Stabilize backfill cursor and stop live resubscribe churn | Removes the highest-cost steady-state relay behavior     |
| 2     | P1 — Stop duplicate subscription sync dispatch                 | Cleans up reducer-to-lane pressure and complements P2    |
| 3     | P3 — Batch relay ingestion and remove hot-path rebuild/persist | Removes the biggest projection and disk hot path         |
| 4     | P4 — Rebuild channel/message indexes in one pass               | Lowers full rebuild cost once P3 flushes are bounded     |
| 5     | P6 — Add managed chat transcript layout cache                  | Removes large-channel UI lag after data-path fixes       |
| 6     | P5 — Make kind-0 fetch incremental plus bootstrap              | Important cleanup, but smaller than the main lag drivers |


---

## Dependency notes

- P2 should land before or alongside P1 so the team does not mistake command dedupe for the full churn fix.
- P3 and P4 are complementary:
  - P3 reduces how often full rebuild work happens
  - P4 reduces how expensive each rebuild is
- P6 should be validated after P3/P4 because renderer lag is easier to measure once reducer and projection churn are under control.
- P5 is independent enough to ship earlier if a small cleanup PR is needed.
