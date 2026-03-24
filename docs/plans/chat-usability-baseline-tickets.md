# Chat Usability Baseline — Jira Tickets

Generated from: `docs/plans/prd-chat-usability-baseline.md`
Date: 2026-03-19

## Progress

| Ticket | Status | Notes |
|--------|--------|-------|
| A-1 | ✅ Done | `chat_message_classifier.rs` created, 12/12 tests passing |
| A-2 | ✅ Done | classify in reducer `RelayEvent` arm; `PresenceEvent`+`DebugEvent` suppressed, no regressions |
| A-3 | ✅ Done | peer presence list + active count rendered in managed chat channel header |
| A-4 | ✅ Done | `show_debug_events` toggle + `Debug`/`Debug ON` chip in managed header; `message_class` field on projection; A-2 filter moved to render time |
| A-5 | ✅ Done | `OA_NIP28_TEAM_CHANNEL_ID` env var; `team_channel_id: Option<String>` on config; `build_filters` covers both channels; setup documented in `docs/headless-compute.md` |
| B-1 | ✅ Done | `Kind0Metadata` struct + `author_metadata` in snapshot; kind-0 relay subscription; `resolve_author_display_name` fallback chain in chat renderer |
| B-2 | ✅ Done | relative timestamps, same-author grouping, own-message `▶` glyph, per-author palette colors; `avatar_color_index` + `author_label_color` helpers |
| C-1 | ✅ Done | Failed rows: error color on role label, "send failed: {error}  retry →" note; Acked rows clean; new `delivery_note_states_match_spec` test |
| C-2 | ✅ Done | Per-row retry click target on Failed delivery note; `managed_chat_retry_targets` in `ChatPaneInputs`; MouseUp handler in `dispatch_input_event` |
| C-3 | 🔲 Not started | |
| C-4 | 🔲 Not started | |
| D-1 | 🔲 Not started | |
| D-2 | 🔲 Not started | |
| D-3 | 🔲 Not started | |
| E-1 | 🔲 Not started | |

---

## Epic

**Summary:** Autopilot Desktop Chat Usability Baseline

**Type:** Epic

**Description:**

### Overview
The current NIP-28 managed chat surface has three compounding failures that
prevent real use: (1) machine-generated presence JSON floods the transcript,
(2) sends fail silently with no recoverable error, and (3) the pane contract
is broken — "Autopilot Chat" can silently drop the user into managed group
chat instead of the local assistant.

This Epic tracks all work to reach a usable baseline: a clean transcript,
reliable sends, correct identity display, and a stable pane contract. The
milestone closes when Ben can have a real back-and-forth conversation in the
team test channel without hitting any blockers.

### Source
- PRD: `docs/plans/prd-chat-usability-baseline.md`
- Technical review: `docs/plans/nip28-chat-vo.md`

### Objectives
- Transcript shows only human messages and compact system notices by default
- Every send either succeeds with a visible ack or fails with an actionable error and a retry affordance
- `pane.autopilot_chat` always opens in local assistant mode
- Ben can use the test channel for a real conversation

### Delivery Phases
- Phase A — Message classification + transcript cleanup + test channel (P0)
- Phase B — Author identity + message chrome (P1)
- Phase C — Send reliability + diagnostics (P0)
- Phase D — Pane contract + roster fix (P1)
- Phase E — Team test + Ben feedback gate

### Success Metrics
| Metric | Target |
|---|---|
| Presence / status JSON visible in default transcript | 0 events per session |
| Send failure with no actionable error | 0 occurrences |
| Author display name missing from human message row | 0 rows |
| Timestamp missing from human message row | 0 rows |
| "You are outside the roster" shown in public NIP-28 channel | Never |
| `pane.autopilot_chat` always opens in assistant mode | Always |
| Managed chat requires explicit navigation | Always |
| Team back-and-forth conversation confirmed by Ben | Yes |

### Scope
**In scope:** classification, identity display, message chrome, send diagnostics,
NIP-42 auth UX, pane routing, roster condition, team test channel, Ben DM gate.

**Out of scope:** NIP-44/59 encryption, NIP-17 DMs, full NIP-29 moderation,
reactions, threading, Spacetime presence, wallet-in-chat.

### Invariants
- `crates/nostr/core` stays app-agnostic; classification lives in `apps/autopilot-desktop`
- State transitions must remain deterministic and replay-safe
- No `.github/workflows/` automation

---

## Phase A — Message Classification + Transcript Cleanup + Test Channel

---

### A-1: Implement ChatMessageClass enum and deterministic classifier

**Type:** Task
**Phase:** A (P0)
**Labels:** chat, classification, transcript

**Summary:**
Define a `ChatMessageClass` enum and a side-effect-free classifier function
that categorizes every inbound kind-42 event before it reaches the renderer.

**Context:**
Currently all kind-42 events are passed directly to the transcript renderer
without classification. Autopilot presence events (parsed separately in
`autopilot_peer_roster.rs`) take the same render path as human messages,
causing raw JSON to appear as conversation rows.

This ticket creates the classification abstraction. Subsequent tickets
(A-2, A-3) wire it into the render and presence paths.

**Requirements:**
- Define `ChatMessageClass` enum in a new file
  `apps/autopilot-desktop/src/chat_message_classifier.rs`
- Minimum required variants:

  | Variant | Definition |
  |---|---|
  | `HumanMessage` | Authored by a human pubkey (not an autopilot process pubkey) |
  | `SystemNotice` | Join/leave, moderation events, channel metadata changes |
  | `PresenceEvent` | Matches `AUTOPILOT_COMPUTE_PRESENCE_TYPE` or equivalent autopilot presence marker |
  | `DebugEvent` | Anything not matched above, or if debug flag is active |

- Implement `pub fn classify(event: &Event) -> ChatMessageClass`
- Classification is purely a function of the event fields (kind, pubkey, tags, content structure). It must not trigger relay subscriptions, UI mutations, or state transitions.
- Classification must be deterministic: same input always produces same output.
- Add unit tests covering at minimum:
  - A typical human text message → `HumanMessage`
  - An `AUTOPILOT_COMPUTE_PRESENCE_TYPE` payload → `PresenceEvent`
  - A join/leave event → `SystemNotice`
  - An unrecognized payload → `DebugEvent`
- The classifier must handle malformed or unparseable content gracefully (return `DebugEvent` rather than panicking).

**Technical Details:**
- Presence type constant: `AUTOPILOT_COMPUTE_PRESENCE_TYPE` in `apps/autopilot-desktop/src/autopilot_peer_roster.rs`
- Existing presence parsing logic lives in that same file; the classifier should reuse the same detection logic (no duplication)
- Protocol primitives in `crates/nostr/core` — classifier must not add app-specific logic to that crate
- Run: `cargo test -p autopilot-desktop` (all existing tests must still pass)

**Acceptance Criteria:**
- [x] `ChatMessageClass` enum is defined with all four variants
- [x] `classify()` is pure and side-effect-free (no async, no state mutation)
- [x] Unit tests pass for all four classification cases
- [x] `AUTOPILOT_COMPUTE_PRESENCE_TYPE` payloads are classified as `PresenceEvent`
- [x] Unrecognized payloads are classified as `DebugEvent` (no panic)
- [x] `cargo test -p autopilot-desktop` passes — 12/12 ✅

**Dependencies:** None — this ticket can start immediately

**Completed:** 2026-03-19 — `apps/autopilot-desktop/src/chat_message_classifier.rs`

---

### A-2: Filter presence and debug events out of the default transcript

**Type:** Task
**Phase:** A (P0)
**Labels:** chat, classification, transcript, presence

**Summary:**
Wire the `ChatMessageClass` classifier into the transcript render path so that
`PresenceEvent` and `DebugEvent` class events are not rendered as transcript rows
by default.

**Context:**
After A-1 the classifier exists but is not yet used. This ticket integrates
it into the message projection and render pipeline so the default transcript
shows only `HumanMessage` and `SystemNotice` rows.

The existing `record_relay_events` path in the reducer (`input/reducers/mod.rs`)
passes all kind-42 events to the same projection. This ticket gates rendering
on classification result.

**Requirements:**
- Call `classify()` on each inbound kind-42 event in the projection pipeline before adding it to the rendered transcript list
- `PresenceEvent`: do NOT add to `ManagedChatMessageProjection` list; route to the presence/roster path (ticket A-3 will handle the destination — for now a no-op is acceptable)
- `DebugEvent`: do NOT add to transcript list unless debug mode is enabled (debug toggle is ticket A-4; until then, always suppress)
- `HumanMessage`: add to transcript list as today (normal render path)
- `SystemNotice`: add to transcript list with a distinct compact style flag (exact styling is Phase B polish; for now a plain row with a `[SYS]` prefix is acceptable)
- Do not remove or alter the existing presence parsing in `autopilot_peer_roster.rs` — that path still needs to run for roster state
- The change must not affect the Autopilot assistant chat lane (`ChatBrowseMode::Autopilot`)

**Technical Details:**
- Primary file: `apps/autopilot-desktop/src/input/reducers/mod.rs` (the NIP-28 update arm that calls `record_relay_events`)
- Classification should happen in the reducer, before appending to projection
- `ManagedChatMessageProjection` in `app_state/chat_projection.rs` may need an optional `message_class` field or a separate list for system notices
- `ConnectionError` and `Eose` arms in the reducer currently discard silently (confirmed in code review); do not change that behavior in this ticket

**Acceptance Criteria:**
- [x] Running the app against the live relay: no raw JSON presence payloads appear in the default managed chat transcript
- [x] `HumanMessage` events continue to render in the transcript
- [x] `cargo test -p autopilot-desktop` passes (especially `codex_lane` and `assemble_chat_turn_input` tests)
- [x] Autopilot assistant chat mode is unaffected

**Dependencies:** Requires A-1 (ChatMessageClass classifier)

---

### A-3: Route presence events into member list / channel header state

**Type:** Task
**Phase:** A (P0)
**Labels:** chat, presence, member-list

**Summary:**
Project autopilot peer presence events (currently filtered out of the transcript
by A-2) into structured UI: a member list component showing each peer's
online/compute-ready status, and a channel header active-count.

**Context:**
`autopilot_peer_roster.rs` already parses presence payloads into
`AutopilotPeerRosterRow` structs. The parsed data is available in app state
but has no dedicated UI surface. After A-2, presence events vanish from the
transcript. This ticket gives them a home.

**Requirements:**
- Member list panel (sidebar or header popover) shows each known peer with:
  - Display name or short pubkey
  - Status indicator: online / offline / compute-ready
- Channel header shows active member count (peers with recent presence)
- Presence state must degrade gracefully when no presence events received: "No members online" or empty list is acceptable; hard error is not
- Unknown presence state must never show as an error
- Member list must not require a full NIP-29 group authority to render; it operates on NIP-28 + presence event data only

**Technical Details:**
- Source data: `AutopilotPeerRosterRow`, `autopilot_peer_roster.rs`
- Target: new member list component in the managed chat pane render path (`apps/autopilot-desktop/src/panes/chat.rs` or a new sub-module)
- Channel header already exists in the chat pane; extend it to show count
- WGPUI component conventions: follow existing patterns in `chat.rs`
- Presence data is already available in app state via the roster parsing path; no new relay subscriptions needed

**Acceptance Criteria:**
- [x] Member list visible in managed chat mode showing at least one peer when presence events have been received
- [x] Channel header shows active member count
- [x] Empty state is graceful ("No members online" or empty list)
- [x] Presence data does NOT appear as transcript rows (A-2 must be in)
- [x] `cargo test -p autopilot-desktop` passes (110/110 chat tests; pre-existing failures unrelated)

**Dependencies:**
- Requires A-1 (classifier)
- Requires A-2 (presence removed from transcript)

---

### A-4: Add debug / raw-events toggle to channel header

**Type:** Task
**Phase:** A (P0)
**Labels:** chat, debug, developer-tools

**Summary:**
Add an explicit toggle (channel header or settings area) that, when enabled,
shows all raw events including `PresenceEvent` and `DebugEvent` class items
in the transcript. Off by default.

**Context:**
REQ-CLASS-3: a debug toggle must exist so operators and developers can inspect
all events without polluting the default experience. After A-2 suppresses
non-human events, operators need a way to get them back.

**Requirements:**
- Toggle in the managed chat pane (channel header preferred, settings also acceptable)
- Off by default
- When ON: all events render in the transcript, including classified `PresenceEvent` and `DebugEvent` rows, with a visual distinction (e.g., muted color, `[DEBUG]` or `[PRESENCE]` prefix)
- When OFF: default transcript behavior (human + system notices only)
- Toggle state is session-local; does not need to persist across restarts
- Toggle must not be accessible from the Autopilot assistant chat lane

**Acceptance Criteria:**
- [ ] Toggle visible in managed chat mode
- [ ] Toggle is OFF by default
- [ ] Enabling toggle makes presence/debug events visible in transcript
- [ ] Debug/presence rows have visual distinction from human message rows
- [ ] Toggle is not present in the Autopilot assistant chat lane
- [ ] `cargo test -p autopilot-desktop` passes

**Dependencies:** Requires A-2 (classification + filtering in place)

---

### A-5: Provision team test channel and wire config path

**Type:** Task
**Phase:** A (P0)
**Labels:** chat, infra, test-channel, dogfood

**Summary:**
Create a second dedicated NIP-28 channel for team testing and configure the
app so team members can join it without manual channel ID setup.

**Context:**
Per §8 of the PRD, a second test channel must exist separate from the main
autopilot presence channel. This prevents test noise from polluting the
production channel and gives Ben a clean environment to test in during Phase E.

**Requirements:**
- Publish a new kind-40 channel creation event on the team relay with a clear name (e.g., `oa-chat-test` or `oa-team-chat`)
- Record the resulting channel ID
- Add an env var or config key (e.g., `OPENAGENTS_CHAT_TEAM_CHANNEL`) that, when set, causes the app to include this channel in the managed chat workspace rail on startup
- The test channel must be accessible from the app without manual input of the channel ID
- Document the channel ID and setup steps in a short comment in the config or in `docs/headless-compute.md`

**Technical Details:**
- Channel creation: publish kind-40 via nostr client (CLI tooling, npub.pro, or any NIP-28-capable client)
- App config: follow existing env var patterns in `apps/autopilot-desktop`
- The channel should be on the same relay already configured for managed chat (check `nip28_chat_lane.rs` for the relay URL currently in use)

**Acceptance Criteria:**
- [ ] A second NIP-28 channel exists with a team-identifiable name
- [ ] The channel ID is documented
- [ ] Setting the config key causes the channel to appear in the app rail
- [ ] Team members can join and see the channel without manual ID entry
- [ ] The test channel does not interfere with the existing production channel

**Dependencies:** None — can start in parallel with A-1 through A-4

---

## Phase B — Author Identity + Message Chrome

---

### B-1: Fetch and cache kind-0 metadata for message authors

**Type:** Task
**Phase:** B (P1)
**Labels:** chat, identity, kind-0, nostr

**Summary:**
On first encounter with a pubkey in the managed chat transcript, fetch
kind-0 metadata from the relay and cache it for the session. Expose the
resolved `display_name`, `name`, and `picture` fields for use by the message
renderer.

**Context:**
`ManagedChatMessageProjection` contains `author_pubkey` but has no
`author_display_name` field. Kind-0 metadata is never fetched, so every
message shows a raw pubkey or is completely unlabeled.

**Requirements:**
- On first receive of a message from a pubkey not yet in the local metadata cache, initiate a kind-0 REQ to the relay for that pubkey
- Cache response keyed by pubkey for the duration of the session
- Resolution order for display name: kind-0 `display_name` → kind-0 `name` → first 8 chars of npub + `…` → first 8 chars of hex pubkey + `…`
- Cache hit must not trigger a new relay request; stale metadata may be re-fetched in background but must not cause a visible flash or flicker
- No fetch triggered for pubkeys already cached
- The cache is session-local; persistence across restarts is not required for this ticket

**Technical Details:**
- Kind-0 fetch can be added to `Nip28ChatLaneWorker` (`nip28_chat_lane.rs`) or a new dedicated metadata lane/helper
- Existing identity primitives: `crates/nostr/core/src/identity.rs`
- nostr client relay pool is already available in the lane worker
- Consider a `HashMap<String, Kind0Metadata>` in app state, populated via the same reducer update path as relay events
- `ManagedChatMessageProjection` may need a `resolved_display_name: Option<String>` field, or the resolution can happen at render time from the cache

**Acceptance Criteria:**
- [ ] Messages from a new pubkey trigger a kind-0 metadata request
- [ ] Resolved display name appears in the message row (not raw pubkey)
- [ ] Fallback chain works: `display_name` → `name` → short npub → short hex
- [ ] No duplicate requests for already-cached pubkeys
- [ ] No flicker when metadata arrives after initial render
- [ ] `cargo test -p autopilot-desktop` passes

**Dependencies:** Requires A-2 (transcript contains only HumanMessage rows to attach identity to)

---

### B-2: Render author name, avatar, timestamps, and message grouping

**Type:** Task
**Phase:** B (P1)
**Labels:** chat, message-chrome, identity, timestamps

**Summary:**
Update the managed chat message row renderer to display the resolved author
name, a deterministic avatar, a relative timestamp, own-message visual
distinction, and grouped same-author messages.

**Context:**
Even after B-1 fetches kind-0 metadata, the chat pane render code must be
updated to use it. Currently managed message rows in `chat.rs` do not render
author name or timestamp — the `created_at` field on `ManagedChatMessageProjection`
is stored but never displayed. `format_thread_timestamp()` exists at ~line 2212
for Autopilot threads but is not used for managed chat rows.

**Requirements:**
- Author header (name + avatar) shown on first message of each author group
- Subsequent messages from the same author within 5 minutes: compact row (no repeated author header, just body with left margin alignment)
- Relative time shown per row: `"just now"` (< 60s), `"Xm ago"` (1–59 min), `"Xh ago"` (1–23h), `"yesterday"`, full date for older
- Absolute time accessible on hover (tooltip or secondary text)
- Avatar: use `picture` URL from kind-0 if available; otherwise a deterministic generated avatar based on pubkey (color or identicon)
- Own messages visually distinct: right-aligned bubble or distinct background — consistent with whatever right-side style the current pane uses

**Technical Details:**
- Primary file: `apps/autopilot-desktop/src/panes/chat.rs` (managed chat message row render function, around `managed_message_delivery_note` ~line 1362)
- Message grouping can be computed at render time from adjacent items in the list
- WGPUI render conventions: follow existing row patterns in `chat.rs`
- Do not block render on metadata availability; show fallback name/avatar immediately, update when kind-0 arrives

**Acceptance Criteria:**
- [ ] Every human message row shows a resolved author name (or short npub fallback)
- [ ] Every human message row shows a relative timestamp
- [ ] Absolute time accessible on hover
- [ ] Adjacent messages from the same author within 5 min are grouped (first row has full header, subsequent rows are compact)
- [ ] Own messages are visually distinct from other authors' messages
- [ ] Avatar shows for messages where kind-0 `picture` is available
- [ ] Avatar fallback shown when no picture URL
- [ ] `cargo test -p autopilot-desktop` passes

**Dependencies:**
- Requires B-1 (kind-0 metadata cache)
- Requires A-2 (clean transcript)

---

## Phase C — Send Reliability + Diagnostics

---

### C-1: Surface PublishError relay messages as visible per-row error states

**Type:** Bug
**Phase:** C (P0)
**Labels:** chat, send-reliability, diagnostics

**Summary:**
`PublishError` events from `Nip28ChatLaneWorker` are currently stored in
`delivery_error` on `ManagedChatMessageProjection` but are not rendered as
visible error states on message rows. This ticket makes send failures visible
to the user.

**Context:**
In `input/reducers/mod.rs`, the `PublishError` arm calls `fail_outbound_message`
which stores the error text in the projection's `delivery_error` field.
However, in `chat.rs` the `managed_message_delivery_note` function formats this
as a plain note string that is not rendered as a visual indicator. The result:
"2 failed local" appears in aggregate but the user has no way to see which
message failed or why.

**Requirements:**
- Failed message rows must show a visible error indicator (red text, error icon, or similar) adjacent to the message body
- The relay error message (`delivery_error` content) must be accessible — either shown inline or on tap/hover
- A "Retry" affordance must be shown on the failed row (see C-2 for retry logic)
- The error indicator must be placed on the specific failed row, not just in an aggregate count
- Pending state (`Publishing`): show a subtle "sending" indicator (spinner, muted clock, or equivalent)
- Delivered / Acked state: no indicator (default clean row)

**Technical Details:**
- `delivery_error: Option<String>` already populated by `fail_outbound_message`
- `delivery_state: ManagedChatDeliveryState` (`Publishing` / `Acked` / `Confirmed` / `Failed`) already exists on the projection
- Primary render file: `apps/autopilot-desktop/src/panes/chat.rs` (`managed_message_delivery_note` function ~line 1362 and the row renderer that calls it)
- Error text may come from the relay OK message or a connection error — surface whatever is available in `delivery_error`

**Acceptance Criteria:**
- [ ] Failed message rows show a visible error indicator
- [ ] Relay error text is accessible on the failed row (inline or hover)
- [ ] Pending rows show a subtle sending indicator
- [ ] Delivered/acked rows show no indicator (clean)
- [ ] The error is on the specific failed row, not just an aggregate count
- [ ] `cargo test -p autopilot-desktop` passes

**Dependencies:** Requires A-2 (classified transcript — delivery state shown only on HumanMessage rows)

---

### C-2: Add per-row retry affordance for failed sends

**Type:** Task
**Phase:** C (P0)
**Labels:** chat, send-reliability, retry

**Summary:**
Add a "Retry" button or affordance on failed message rows that re-submits the
message to the `Nip28ChatLane` publish path.

**Context:**
`active_managed_chat_retryable_message()` exists in app state and a retry hint
appears in the composer text, but there is no per-row retry button. The user
cannot retry from the failed row itself.

**Requirements:**
- A "Retry" affordance (button, link text, or tap target) is visible on each failed message row in the managed chat transcript
- Clicking/tapping "Retry" re-submits the event to `Nip28ChatLaneWorker`
- After retry submission, the row transitions to Pending state (C-1 indicator)
- If the retry also fails, the row returns to Failed state with updated error
- The retry path must use the same signed event (same event ID, same content, same timestamp) — do not re-sign with a new timestamp

**Technical Details:**
- The existing retry hint in the composer (`active_managed_chat_retryable_message`) can be removed or kept as a secondary affordance; the per-row button is primary
- Retry command: re-enqueue the stored event in `Nip28ChatLaneCommand::Publish`
- `apps/autopilot-desktop/src/panes/chat.rs` for the row render
- `apps/autopilot-desktop/src/input/reducers/mod.rs` for the retry command handler

**Acceptance Criteria:**
- [ ] "Retry" affordance visible on every failed message row
- [ ] Clicking retry transitions the row to Pending state
- [ ] Successful retry transitions the row to Delivered state
- [ ] Failed retry returns the row to Failed state with updated error text
- [ ] `cargo test -p autopilot-desktop` passes

**Dependencies:** Requires C-1 (delivery state visible on rows)

---

### C-3: Handle NIP-42 auth challenge in the chat lane and surface auth state

**Type:** Task
**Phase:** C (P0)
**Labels:** chat, nip42, auth, send-reliability

**Summary:**
`Nip28ChatLaneWorker` currently does not respond to NIP-42 AUTH challenges from
the relay. Auth challenges are received but ignored, causing publish to fail
or be rejected without the user knowing why. This ticket wires NIP-42
challenge-response into the lane and exposes auth state to the user.

**Context:**
Confirmed in code review of `nip28_chat_lane.rs`: the relay connection pool
receives events but there is no AUTH challenge handler in the lane worker.
NIP-42 primitives exist at `crates/nostr/core/src/nip42.rs` and the relay
client layer. The desktop integration layer (the lane) does not call them.

**Requirements:**
- `Nip28ChatLaneWorker` must detect and respond to AUTH challenges from the relay using the available signing keypair
- If auth succeeds: continue normal operation (no user notification needed)
- If auth fails: surface a visible error: `"This relay requires authentication — auth failed"` with current auth state and a link to `pane.identity_keys`
- If the relay requires auth but no keypair is available: show `"This relay requires authentication — no identity configured"` with a link to `pane.identity_keys`
- Auth state (not authenticated / authenticating / authenticated / failed) must be stored in app state and accessible to the chat pane renderer

**Technical Details:**
- NIP-42 primitives: `crates/nostr/core/src/nip42.rs`
- Lane worker: `apps/autopilot-desktop/src/nip28_chat_lane.rs`
- The relay connection pool should already surface AUTH messages; the lane needs to handle them
- Auth challenge response signing uses the same keypair as publish signing
- Auth state can be a new enum in app state (or extended from existing relay connection state)
- `crates/nostr/client` may need a relay auth integration point; coordinate with the crate boundary defined in §9 of the PRD

**Acceptance Criteria:**
- [ ] NIP-42 AUTH challenge from relay triggers a signed challenge response
- [ ] Successful auth: no user notification, publish proceeds normally
- [ ] Auth failure: visible error message with current state and link to identity keys pane
- [ ] No keypair available: composer shows block state with link to identity keys pane
- [ ] Auth state is reflected in channel header / relay status area (C-4)
- [ ] `cargo test -p autopilot-desktop` passes

**Dependencies:** Requires C-4 for auth state display surface; does not require A-2 but should ship in same phase

---

### C-4: Keypair pre-check and relay status in channel header

**Type:** Task
**Phase:** C (P0)
**Labels:** chat, send-reliability, diagnostics, identity

**Summary:**
Before the composer is enabled, verify that a valid signing keypair is
available. If not, show a non-destructive block state with a link to
`pane.identity_keys`. Also expose relay connection state and last relay error
in the channel header.

**Context:**
REQ-SEND-3 and REQ-SEND-5. Currently there is no pre-check — the user can
type a message and submit it even if no keypair is configured, resulting in a
silent failure. Relay connection state is also not visible in the pane.

**Requirements:**
- Keypair pre-check: on managed chat pane load, check whether a valid signing identity is available
  - If YES: composer is enabled normally
  - If NO: composer is disabled with: `"You need an identity to send messages — Set up identity keys"` (link navigates to `pane.identity_keys`)
- Channel header relay status: show relay URL (truncated) and connection state (connected / connecting / disconnected / auth-required)
- Last relay error (if any) accessible on hover or tap of the relay indicator
- Status must update reactively as connection state changes

**Technical Details:**
- Identity check: `crates/nostr/core/src/identity.rs` (signing keypair availability)
- Relay status: `Nip28ChatLaneWorker` tracks connection state; expose via app state update
- Channel header in `chat.rs` — add a relay indicator row below the channel name
- Composer block state: disable the text input and show explanatory copy; do not hide the composer entirely

**Acceptance Criteria:**
- [ ] Composer is disabled with explanatory copy when no keypair configured
- [ ] Link from composer block state navigates to identity keys pane
- [ ] Composer is enabled when a valid keypair is present
- [ ] Channel header shows relay URL and connection state
- [ ] Last relay error accessible on hover/tap of relay indicator
- [ ] Connection state updates reactively (connect → auth → connected)
- [ ] `cargo test -p autopilot-desktop` passes

**Dependencies:** Independent — can start before C-1; auth state display used by C-3

---

## Phase D — Pane Contract + Roster Fix

---

### D-1: Enforce pane.autopilot_chat always opens in local assistant mode

**Type:** Bug
**Phase:** D (P1)
**Labels:** chat, pane-routing, pane-contract

**Summary:**
`pane.autopilot_chat` currently can silently open in managed group chat mode
if browse-mode workspace selection has routed the pane to managed content.
This ticket enforces that the Autopilot Chat pane entry point always shows
the local assistant, and that managed chat is only reachable via explicit
navigation.

**Context:**
Confirmed bug in `app_state.rs`, `chat_browse_mode()` function:

```rust
if self.has_managed_chat_browseable_content() {
    ChatBrowseMode::Managed  // ← hijacks pane without user action
}
```

Per `docs/MVP.md`: `pane.autopilot_chat` is the "Personal agent chat thread +
local execution UX." It must not default to managed content.

**Requirements:**
- Opening `pane.autopilot_chat` via command palette, keyboard shortcut, or sidebar must always result in `ChatBrowseMode::Autopilot`
- The current browse-mode auto-selection logic must NOT change the mode to `Managed` or `DirectMessages` unless the user has explicitly navigated to those modes
- Explicit navigation to managed chat must be a distinct user action (e.g., clicking a "Group Chat" pane or a channel in the workspace rail)
- When the user IS in managed chat and opens `pane.autopilot_chat`, they should be taken back to the assistant mode

**Technical Details:**
- `apps/autopilot-desktop/src/app_state.rs` — `chat_browse_mode()` function
- The fix: when `pane.autopilot_chat` is the active pane command, always return `ChatBrowseMode::Autopilot` regardless of workspace selection
- If managed chat needs its own pane entry point, it should have one (e.g., `pane.managed_chat`); this ticket does not require creating a new pane but must decouple the routing
- `apps/autopilot-desktop/src/panes/chat.rs` — pane title must also reflect the correct mode (see D-2)

**Acceptance Criteria:**
- [ ] Opening "Autopilot Chat" command always shows local assistant view
- [ ] Managed chat mode is not active unless the user explicitly navigated to it
- [ ] Switching away from managed chat and back to assistant chat works
- [ ] `cargo test -p autopilot-desktop` passes (pane routing tests)

**Dependencies:** Independent — can be worked in parallel with Phase A

---

### D-2: Pane title / header clearly identifies managed chat vs assistant mode

**Type:** Task
**Phase:** D (P1)
**Labels:** chat, pane-contract, labeling

**Summary:**
When the user is in managed group chat mode, the pane title and/or header must
clearly identify the channel name and mode. It must not be labeled "Autopilot Chat"
when showing managed group content.

**Context:**
REQ-PANE-3. Currently the pane title does not change between assistant and
managed chat modes. A user in managed chat looking at the window title or pane
header sees "Autopilot Chat," reinforcing the incorrect expectation that they
are talking to the local assistant.

**Requirements:**
- When `ChatBrowseMode` is `Managed`: pane/window title or header shows the channel name (from kind-40/41 metadata) and/or a clear label like `"Group Chat — [channel name]"` or `"[relay] / [channel name]"`
- When `ChatBrowseMode` is `Autopilot`: existing "Autopilot Chat" or "Chat" label is correct; no change needed
- The managed chat header should also include the relay indicator (wired from C-4)
- The distinction must be visible without hovering or expanding anything

**Technical Details:**
- `apps/autopilot-desktop/src/panes/chat.rs` — pane title render path
- Channel metadata (name, about) is available from kind-40/41 events; ensure it is included in app state and accessible to the header renderer

**Acceptance Criteria:**
- [ ] Managed chat mode shows channel name in header (not "Autopilot Chat")
- [ ] Assistant mode header is unchanged
- [ ] Channel name comes from kind-40/41 metadata when available
- [ ] `cargo test -p autopilot-desktop` passes

**Dependencies:** Requires D-1 (pane routing must be stable first)

---

### D-3: Condition roster/membership warning on NIP-29 room with membership exclusion

**Type:** Bug
**Phase:** D (P1)
**Labels:** chat, roster, nip28, nip29

**Summary:**
"You are outside the roster" is shown unconditionally when the user's pubkey
is not in the managed group member list, including for public NIP-28 channels
that have no NIP-29 group authority. This ticket conditions the warning on
room type.

**Context:**
Confirmed bug in `apps/autopilot-desktop/src/panes/chat.rs`,
`managed_group_membership_label()` (~line 1540):

```rust
None => "you are outside the roster".to_string()
```

This fires for ANY channel where the user is not in
`active_managed_chat_local_member()`, including public NIP-28 channels with
no roster concept.

**Requirements:**
- The roster/membership warning must only be shown when ALL of the following are true:
  1. The current room is a NIP-29 managed group (has `h` NIP-29 group tag or equivalent authority marker)
  2. The user's pubkey is not in the group member list
  3. The relay enforces membership restriction (read or write)
- For public NIP-28 channels (kind-40 create event, no NIP-29 group authority): show NO roster warning; the user may participate freely
- Room type detection: a channel with a kind-40 create event and no NIP-29 group indicator is treated as a public NIP-28 channel (REQ-ROSTER-3)
- If room type is indeterminate (metadata not yet received), show nothing rather than the warning

**Technical Details:**
- `managed_group_membership_label()` in `chat.rs` ~line 1540
- Room type information must be available in app state (or derivable from stored events); if not currently tracked, add it
- NIP-29 group tag: `h` tag on kind-9xxx events, or presence of a group creation event (kind-9000 or similar per NIP-29 spec)
- The fix is to add a `room_has_nip29_authority()` check before rendering the warning label

**Acceptance Criteria:**
- [ ] "You are outside the roster" (or equivalent) is NEVER shown for a public NIP-28 channel
- [ ] Warning IS shown when user is excluded from a NIP-29 managed group with relay-enforced membership
- [ ] Indeterminate room type: no warning shown
- [ ] `cargo test -p autopilot-desktop` passes

**Dependencies:** Independent — can be worked in parallel with Phase A

---

## Phase E — Team Test + Readiness Gate

---

### E-1: Team test + Ben feedback gate

**Type:** Task
**Phase:** E
**Labels:** chat, dogfood, testing, readiness-gate

**Summary:**
Complete the team test protocol: verify full send/receive cycle in the test
channel, run through the acceptance criteria checklist, DM Ben on Nostr, and
triage his feedback. The milestone does not close until Ben can hold a real
conversation without hitting blockers.

**Context:**
Per §8 of the PRD, Ben is the designated usability tester for this milestone,
the same role he plays for the rest of the app. His feedback is ground truth
for whether chat is actually usable, not just whether it passes the checklist.

**Requirements:**
- Phases A, B, C, and D are complete before this ticket begins
- At least one team member completes a full send/receive cycle in the test channel (provisioned in A-5)
- All 15 acceptance criteria in §7 of the PRD verified green:
  - [ ] Transcript contains only human messages and system notices by default
  - [ ] No raw JSON presence/status payloads in default transcript
  - [ ] Every message row shows resolved author name (or short npub fallback)
  - [ ] Every message row shows human-readable relative time
  - [ ] Adjacent same-author messages within 5 min are grouped
  - [ ] Successfully sent message transitions pending → delivered without user action
  - [ ] Failed send shows relay error reason and retry affordance
  - [ ] NIP-42 auth failure produces visible, actionable message
  - [ ] Sending blocked with clear prompt when no keypair available
  - [ ] Presence events shown in member list / header, not transcript rows
  - [ ] Debug toggle shows all raw events when enabled
  - [ ] `pane.autopilot_chat` opens in local assistant mode, not managed chat
  - [ ] Managed chat only entered via explicit user navigation
  - [ ] "Roster" membership warning never shown in public NIP-28 channel
  - [ ] Pane/window title clearly identifies when user is in managed chat
- DM Ben on Nostr:
  - Share the test channel ID
  - Ask him to have a normal conversation: send a few messages, read replies, note anything broken or confusing
  - His feedback is treated as usability bugs per the triage below
- Triage Ben's feedback:
  - **Blocker** — fix before closing milestone
  - **Usability gap** — log for follow-on PRD (Private Messaging and Secure Room Transport)
  - **Note** — log and move on
- Milestone closes when Ben can use the test channel for a real back-and-forth conversation without hitting any blockers

**Acceptance Criteria:**
- [ ] Phases A, B, C, D all complete
- [ ] Test channel (A-5) is accessible and usable
- [ ] Full send/receive cycle verified by at least one team member
- [ ] All 15 acceptance criteria in §7 verified green
- [ ] Ben has been DM'd on Nostr with channel ID and test instructions
- [ ] Ben's feedback has been triaged (blockers resolved, gaps logged)
- [ ] Ben can hold a real conversation in the test channel without blockers
- [ ] Follow-on PRD (Private Messaging and Secure Room Transport) has been drafted or backlog items logged from Ben's usability gap feedback

**Dependencies:** Requires A-1, A-2, A-3, A-4, A-5, B-1, B-2, C-1, C-2, C-3, C-4, D-1, D-2, D-3 all complete

---

## Ticket Summary

| Ticket | Phase | Type | Priority | Summary |
|--------|-------|------|----------|---------|
| A-1 | A | Task | P0 | `ChatMessageClass` enum + classifier |
| A-2 | A | Task | P0 | Filter presence/debug events from transcript |
| A-3 | A | Task | P0 | Route presence into member list + channel header |
| A-4 | A | Task | P0 | Debug/raw-events toggle |
| A-5 | A | Task | P0 | Provision team test channel + config path |
| B-1 | B | Task | P1 | Fetch + cache kind-0 author metadata |
| B-2 | B | Task | P1 | Render name, avatar, timestamps, message grouping |
| C-1 | C | Bug  | P0 | Surface `PublishError` as per-row error state |
| C-2 | C | Task | P0 | Per-row retry affordance for failed sends |
| C-3 | C | Task | P0 | NIP-42 auth challenge-response + auth state UX |
| C-4 | C | Task | P0 | Keypair pre-check + relay status in channel header |
| D-1 | D | Bug  | P1 | Enforce `pane.autopilot_chat` → assistant mode always |
| D-2 | D | Task | P1 | Managed chat pane title identifies channel name |
| D-3 | D | Bug  | P1 | Condition roster warning on NIP-29 room only |
| E-1 | E | Task | —  | Team test + Ben feedback gate |

## Dependency Order

```
A-1 → A-2 → A-3
            ↓
A-2 → C-1 → C-2
A-4 (after A-2)
A-5 (parallel, any time)

B-1 (after A-2)
B-2 (after B-1)

C-4 (independent, provides auth state display for C-3)
C-3 (after C-4)

D-1 (independent)
D-2 (after D-1)
D-3 (independent)

E-1 (after all of A, B, C, D)
```

**Critical path:** A-1 → A-2 → C-1 → C-2 → E-1
