# PRD: Autopilot Desktop Chat Usability Baseline

**Status:** Draft
**Date:** 2026-03-19
**Owner:** `apps/autopilot-desktop`
**Spec authority:** `docs/MVP.md`, `docs/plans/nostr-group-chat.md`
**Dogfood target:** Internal team chat channel — Ben is the primary usability tester

---

## 1. Goal

Make group chat in Autopilot Desktop usable enough for the internal team to actually use it.

The concrete bar is: **the team can have real back-and-forth conversations reliably in a dedicated test channel, and Ben can provide usability feedback from that experience the same way he does for the rest of the app.**

The current chat surface has three compounding failures preventing this:

1. **The transcript is unreadable.** Machine-generated presence and status events (kind 42) render as first-class conversation messages. Raw JSON floods what should be a human conversation view.
2. **Sending is unreliable and silent.** Messages fail with no actionable explanation. Auth requirements, relay policy, and identity state are invisible to the user.
3. **The pane contract is broken.** "Autopilot Chat" is supposed to be the local assistant experience, but browse-mode routing can silently swap the user into managed/group chat without their knowledge.

This PRD scopes the work to fix all three problems. The milestone is done when a team member can open the app, navigate to the team test channel, read the conversation, send a message reliably, and know who said what and when — without needing to know anything about Nostr internals.

---

## 2. Non-goals for this phase

The following are explicitly out of scope:

- Full NIP-44 / NIP-59 encrypted DM rollout
- Large-group E2EE room architecture (Marmot / MLS evaluation)
- Voice / video
- Marketplace-grade room permissions and paid channel gating
- Full social network feature set
- Web / mobile parity
- Advanced threading UI (deep nested thread views)

These belong in the follow-on PRD, **Private Messaging and Secure Room Transport**, outlined in §10.

---

## 3. Success metrics

| Metric | Target |
|---|---|
| Team can hold a back-and-forth conversation in the test channel | Yes — confirmed by Ben |
| Presence / status JSON visible in default transcript | 0 events per session |
| Send failure with no actionable error shown | 0 occurrences |
| User can identify message author by display name | 100% of human messages |
| User can identify message time | 100% of human messages |
| "You are outside the roster" shown in public NIP-28 channel | Never |
| `pane.autopilot_chat` always opens in local assistant mode | Always |
| Managed chat requires explicit user navigation to enter | Always |

---

## 4. User-facing problems (ordered by urgency)

### P0 — Presence JSON floods the transcript

**What the user sees:** The message list contains walls of raw JSON objects, status payloads, and system state events. These are valid kind-42 events but are not human-authored messages.

**Root cause:** `nip28_chat_lane.rs` fetches all kind-42 events matching the channel filter (`{"kinds": [41, 42], "#e": [channel_id]}`). The chat pane renders every inbound `RelayEvent(Event)` as a message row without classifying what it contains. Machine presence events authored by autopilot peers (parsed by `autopilot_peer_roster.rs`) follow the same render path as user messages.

**Impact:** The transcript is unusable as a conversation surface.

---

### P0 — Message sends fail silently

**What the user sees:** `2 failed local` in the send state, or messages that appear locally but never appear on other clients. No explanation of why. No recovery action offered.

**Root cause candidates** (require lane diagnostics to confirm):
- `PublishError` events from `Nip28ChatLaneWorker` are not surfaced to the user
- NIP-42 auth challenge from the relay is not responded to, or the response fails and no error is shown
- No usable keypair is bound to the chat session
- Relay write policy rejects the event for reasons other than auth (kind policy, rate limit, relay-specific rules)
- Identity state (`crates/nostr/core/src/identity.rs`) is not confirmed valid before publish

**Impact:** Users cannot participate in managed group chat. Broken send kills chat credibility immediately.

---

### P1 — No author display names

**What the user sees:** Messages show raw pubkeys or are completely unlabeled. Authors are indistinguishable.

**Root cause:** Kind-0 metadata is not being fetched and applied to message rows. No display name resolution, avatar, or fallback to short npub is implemented.

**Impact:** Even if the JSON noise is removed, a list of anonymous messages is not a readable conversation.

---

### P1 — Pane contract ambiguity: assistant vs managed chat

**What the user sees:** Opening "Autopilot Chat" (`pane.autopilot_chat`) can result in the managed NIP-28 channel view rather than the local assistant experience, depending on workspace selection state and browse mode behavior.

**Root cause:** The pane selection logic does not enforce a stable contract between "local assistant" and "managed group chat." Both modes can be served from the same pane entry point.

**Impact:** This is a product trust failure. Per `docs/MVP.md`, "Autopilot Chat" is defined as the personal agent chat thread and local execution UX. Hijacking it with managed group content breaks the app's primary promise.

---

### P1 — "You are outside the roster" shown incorrectly

**What the user sees:** A roster membership warning is shown for a channel that is a public NIP-28 channel with no NIP-29 group authority.

**Root cause:** The UI is applying NIP-29-style roster membership logic unconditionally, without checking whether the current room has a managed group authority behind it. NIP-28 public channels have no roster concept.

**Impact:** Confusing and incorrect. Users reasonably conclude they cannot participate when they actually can.

---

### P2 — No timestamps on messages

**What the user sees:** Messages have no time attribution. Relative recency and conversation flow are impossible to read.

**Root cause:** The `created_at` field present on every Nostr event is not rendered in message rows.

**Impact:** Secondary, but part of the overall message chrome and scannability problem.

---

## 5. Requirements

### 5.1 Message classification and rendering policy

**REQ-CLASS-1:** Define an explicit in-process message taxonomy. Every inbound kind-42 event must be classified before rendering. The minimum required categories are:

| Class | Definition | Default render |
|---|---|---|
| `HumanMessage` | Content authored by a human pubkey, not an autopilot process pubkey | Full message row with author, time, body |
| `SystemNotice` | Join/leave, moderation events, channel metadata changes | Compact notice row, distinct visual style |
| `PresenceEvent` | Autopilot peer presence/status updates (currently parsed by `autopilot_peer_roster.rs`) | Absorbed into member list / header state; not a transcript row |
| `DebugEvent` | Any event not matching the above, or if a debug flag is set | Hidden by default; accessible via raw event inspector |

**REQ-CLASS-2:** Classification must be deterministic and side-effect-free. Classifying an event must not trigger relay subscriptions, UI mutations, or state transitions.

**REQ-CLASS-3:** A debug/raw-events toggle must exist (e.g., in the channel header or settings pane) that allows any class to be made visible. This preserves operator and developer visibility without polluting the default transcript.

---

### 5.2 Author identity display

**REQ-IDENT-1:** Every `HumanMessage` row must show a resolved display name. Resolution order: kind-0 `display_name`, kind-0 `name`, short npub (first 8 chars + `…`), short hex.

**REQ-IDENT-2:** Kind-0 metadata must be fetched on first encounter with a pubkey and cached locally for the session. Stale metadata may be re-fetched in background but must not cause visual flicker.

**REQ-IDENT-3:** Avatar display, if implemented, must fall back to a deterministic generated avatar (e.g., based on pubkey) when no picture URL is available.

**REQ-IDENT-4:** The app's own messages must be visually distinguishable from other authors (right-aligned or distinct styling is acceptable).

---

### 5.3 Message chrome

**REQ-CHROME-1:** Every `HumanMessage` row must show a human-readable relative time (e.g., "just now", "3 min ago", "yesterday") derived from the event `created_at`. Absolute time must be accessible on hover or tap.

**REQ-CHROME-2:** Adjacent messages from the same author within a short time window (≤ 5 minutes) must be grouped visually. Only the first row in a group shows the full author header (name + avatar). Subsequent rows in the group show only the message body with a compact left margin.

**REQ-CHROME-3:** Message delivery state must be shown per-row for locally composed messages:
- Pending (sent to lane, not yet acked): subtle indicator
- Delivered (relay `OK` ack received): default/no indicator
- Failed (relay error or timeout): error indicator with failure reason accessible

---

### 5.4 Send reliability and diagnostics

**REQ-SEND-1:** All `PublishError` events from `Nip28ChatLaneWorker` must be surfaced to the user with the relay error message. A generic "message failed to send" is not acceptable.

**REQ-SEND-2:** NIP-42 auth challenge/response must be handled in the chat lane. If a relay requires auth and auth fails, the user must see:
- "This relay requires authentication"
- Current auth state (signed in / not signed in / auth failed)
- A clear next action (e.g., "Check identity keys" linking to `pane.identity_keys`)

**REQ-SEND-3:** Before attempting to publish, the chat lane must verify that a valid signing keypair is available. If no keypair is available, the composer must show a non-destructive block state with a link to `pane.identity_keys`.

**REQ-SEND-4:** Failed sends must be retryable from the message row. A "Retry" affordance must be visible on failed rows.

**REQ-SEND-5:** The relay URL, connection state, and last relay error must be visible somewhere accessible (channel header detail, `pane.relay_connections`, or a status tooltip). Users should be able to see "connected to relay X" without opening a separate diagnostics pane.

---

### 5.5 Presence in structured UI

**REQ-PRESENCE-1:** Autopilot peer presence data (currently routed through `autopilot_peer_roster.rs`) must be projected into dedicated structured UI:
- Member list (sidebar or header popover): online/offline/compute-ready status per peer
- Channel header: active member count, channel name, relay indicator
- Optional badges on peer names

**REQ-PRESENCE-2:** Presence state must not appear as transcript rows in the default view. The raw events remain accessible via the debug toggle (REQ-CLASS-3).

**REQ-PRESENCE-3:** Presence UI must degrade gracefully when no presence events have been received. "No members online" or an empty member list is acceptable. Unknown state must never show as a hard error.

---

### 5.6 Assistant vs managed chat separation

**REQ-PANE-1:** `pane.autopilot_chat` (command `Autopilot Chat`) must always open in local assistant mode. It must not default to managed group chat, regardless of workspace selection or browse-mode state.

**REQ-PANE-2:** Managed NIP-28 group chat must be accessible through a distinct pane or clearly labeled tab/mode switch. The user must make an explicit navigation choice to enter managed chat.

**REQ-PANE-3:** If the user is in managed chat mode, the pane title or header must clearly indicate this (e.g., channel name, "Group Chat", server name). It must not be labeled "Autopilot Chat."

**REQ-PANE-4:** Browse-mode workspace selection must not silently change the active pane mode from assistant to managed. Mode changes must be explicit user actions.

---

### 5.7 Membership / roster copy

**REQ-ROSTER-1:** "You are outside the roster" (or equivalent membership warning) must only be shown when:
- The current room is a NIP-29 managed group, AND
- The user's pubkey is not in the group's member list, AND
- The relay enforces membership restriction on reads or writes

**REQ-ROSTER-2:** For public NIP-28 channels with no NIP-29 group authority, no roster warning must be shown. The user may participate freely.

**REQ-ROSTER-3:** Room type must be determinable from available event data: a channel with a kind-40 create event and no `h` NIP-29 group tag is treated as a public NIP-28 channel.

---

## 6. Out-of-scope clarifications

The following are explicitly not required in this milestone and must not be added as part of implementation:

- NIP-44 / NIP-59 encryption for group messages
- NIP-17 DM send/receive UI
- Full NIP-29 join/leave/moderation flows (read is OK, write actions are next phase)
- Reaction (NIP-25) support
- Threading / reply tree rendering beyond flat grouping
- Spacetime-backed presence sync
- Wallet-in-chat actions (invoice rendering, tipping)

---

## 7. Acceptance criteria

A chat session is usable when all of the following are true:

- [ ] The transcript contains only human messages and compact system notices by default
- [ ] No raw JSON presence/status payloads are visible in the default transcript
- [ ] Every message row shows a resolved author name (or short npub fallback)
- [ ] Every message row shows a human-readable relative time
- [ ] Adjacent messages from the same author within 5 minutes are grouped
- [ ] A successfully sent message transitions from pending → delivered without user action
- [ ] A failed send shows the relay error reason and a retry affordance
- [ ] NIP-42 auth failure produces a visible, actionable message
- [ ] Sending is blocked with a clear prompt when no keypair is available
- [ ] Presence events are shown in member list / header, not transcript rows
- [ ] A debug toggle shows all raw events when enabled
- [ ] `pane.autopilot_chat` opens in local assistant mode, not managed chat
- [ ] Managed chat is only entered via explicit user navigation
- [ ] "Roster" membership warning is never shown in a public NIP-28 channel
- [ ] The pane/window title or header clearly identifies when the user is in managed chat

---

## 8. Team test protocol

### Purpose

Before this milestone is considered done, the internal team must validate it through actual use. Ben is the designated usability tester for this — the same role he plays for the rest of the app. His feedback is the ground truth for whether chat is actually usable, not just whether it passes the acceptance criteria checklist.

### Test channel setup

A **second dedicated NIP-28 channel** must be provisioned for team testing, separate from the main autopilot presence channel. This prevents test noise (failed sends, debug toggles, test messages) from polluting the production channel.

The test channel should be:
- Created with a clear name (e.g., `oa-chat-test` or `team-chat-beta`)
- Accessible from the desktop app via the managed chat workspace rail
- Pre-configured via env var or settings so team members can join without manual channel ID setup

### Readiness gate — DM Ben

When the following are all true:
- Phases A, B, C, and D are complete
- At least one internal team member has verified a full send/receive cycle in the test channel
- The acceptance criteria checklist in §7 is fully green

**DM Ben on Nostr to invite him to test.** Share the test channel ID and ask him to try having a normal conversation — send a few messages, read replies, see if anything feels broken or confusing. His feedback should be treated as usability bugs, not polish requests.

### Feedback loop

Ben's feedback comes back as one of:
- **Blocker** — something prevents basic use; fix before calling the milestone done
- **Usability gap** — something is confusing or rough; triage against the follow-on PRD scope
- **Note** — cosmetic or future work; log and move on

The milestone closes when Ben can use the test channel for a real conversation without hitting any blockers.

---

## 9. Crate and file ownership

| Area | Primary file(s) | Owner crate |
|---|---|---|
| Message classification logic | New: `chat_message_classifier.rs` | `apps/autopilot-desktop` |
| Transcript render | Existing chat pane render path | `apps/autopilot-desktop` |
| Kind-0 metadata fetch + cache | New or existing identity fetch helper | `apps/autopilot-desktop` / `crates/nostr/client` |
| Presence projection (member list) | `autopilot_peer_roster.rs` + new member list component | `apps/autopilot-desktop` |
| Send diagnostics / PublishError surface | `nip28_chat_lane.rs` + chat pane state | `apps/autopilot-desktop` |
| NIP-42 auth handling | `crates/nostr/client` relay auth + chat lane integration | `crates/nostr/client`, `apps/autopilot-desktop` |
| Pane routing / assistant vs managed separation | Pane selection logic | `apps/autopilot-desktop` |
| Roster condition check | Chat pane membership display logic | `apps/autopilot-desktop` |

Protocol primitives for NIP-42, NIP-44, NIP-59 already exist in `crates/nostr/core`. This milestone does not require new crate-level protocol work — the gap is in application-level classification, routing, and diagnostic UX.

---

## 10. Delivery phases

### Phase A — Message classification + transcript cleanup + test channel (P0)

**Scope:**
- Implement `ChatMessageClass` enum and classifier
- Route presence events to member list / header, remove from transcript default
- Route system notices to compact notice rows
- Route debug events to hidden-by-default debug pane
- Add debug toggle affordance
- Provision the team test channel (see §8) and wire up the env var / config path so team members can join it

**Done when:** Zero raw JSON events appear in the default transcript in a live session, and the test channel is joinable by the team.

---

### Phase B — Author identity + message chrome (P1)

**Scope:**
- Kind-0 metadata fetch on pubkey first-encounter
- Display name resolution with fallback chain
- Avatar display with deterministic fallback
- Relative timestamp per message row
- Message grouping by same author / short time window
- Own-message visual distinction

**Done when:** Every human message row shows a name and time, grouped messages are visually coherent.

---

### Phase C — Send reliability + diagnostics (P0)

**Scope:**
- Surface all `PublishError` payloads with relay message text
- Per-row delivery state: pending / delivered / failed
- Retry affordance on failed rows
- NIP-42 auth challenge response in chat lane
- Auth failure UX with link to identity keys pane
- Keypair availability pre-check before compose is enabled
- Relay status visible in channel header or tooltip

**Done when:** No send failure is silent. Every failure has a visible reason and a recovery action.

---

### Phase D — Pane contract + roster fix (P1)

**Scope:**
- Enforce `pane.autopilot_chat` always opens in assistant mode
- Managed chat requires explicit navigation (distinct pane entry or labeled mode switch)
- Pane title / header identifies managed chat mode
- Browse-mode selection does not silently switch mode
- Roster warning conditioned on NIP-29 room + membership exclusion only

**Done when:** All acceptance criteria in §7 for pane and roster are met.

---

### Phase E — Team test + Ben feedback (readiness gate)

**Scope:**
- At least one team member completes a full send/receive cycle in the test channel
- All §7 acceptance criteria are green
- DM Ben on Nostr to invite him to test (see §8)
- Triage his feedback: blockers fixed before milestone closes, gaps logged for follow-on

**Done when:** Ben can have a real back-and-forth conversation in the test channel without hitting any blockers.

---

## 11. Follow-on PRD

After this baseline ships, the next PRD should cover:

**Private Messaging and Secure Room Transport**

Scope:
- NIP-17 DM send/receive with full composer and transcript
- NIP-44 encryption integration (build on existing `crates/nostr/core/src/nip44.rs`)
- NIP-59 / kind `1059` gift-wrap handling for DM delivery
- Undecryptable-message UX (missing key, wrong key, malformed wrap)
- Recipient relay compatibility (`NIP-65` relay list routing)
- Private-room identity trust and metadata display
- DM surface in the channel/room rail (separate from server channels)

That phase unlocks 1:1 DMs and small private rooms as described in `docs/plans/nostr-group-chat.md` Phase 2.

---

## 12. Dependency map

```
Phase A (classification)
  └── required before: Phase B (identity, chrome) — transcript must be clean before investing in row polish
  └── required before: Phase C (send) — delivery state belongs on human message rows only

Phase B (identity + chrome)
  └── can run parallel to: Phase C

Phase C (send)
  └── depends on: Phase A (classified message rows to attach delivery state to)
  └── depends on: NIP-42 auth plumbing in crates/nostr/client (already present, needs desktop integration)

Phase D (pane contract)
  └── independent — can start any time, but should ship with or before Phase A
      because the pane contract confusion compounds the transcript readability problem

Phase E (team test + Ben)
  └── depends on: Phases A, B, C, D all complete
  └── test channel must be provisioned (part of Phase A)
```

---

## 13. Invariants this work must not break

Per `docs/MVP.md` and `docs/OWNERSHIP.md`:

- Sync and state continuity must remain deterministic and replay-safe.
- Wallet and payout state must be explicit and truthful; chat must never imply payment unless the wallet confirms it.
- `crates/nostr/core` protocol primitives must remain app-agnostic. Classification and routing logic lives in `apps/autopilot-desktop`, not in the protocol crate.
- No new `.github/workflows/` automation.
- Do not pull in archived code from the backroom without explicit direction.
