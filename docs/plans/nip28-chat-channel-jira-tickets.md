# Jira Tickets for NIP-28 Chat First Milestone

Use this document to create the Epic and child issues in your Jira project. Create the **Epic first**, then create each child and set **Epic Link** (or Parent) to the Epic. Order of implementation: A → B → C → D → E; F is optional.

**Source:** [NIP-28 Chat Channel PRD](nip28-chat-channel-prd.md)

---

## Epic

| Field | Value |
|-------|--------|
| **Issue type** | Epic |
| **Title** | NIP-28: Basic chat channel with auto-connect |

**Description:**

```
First milestone of Nostr group chat: one default NIP-28 public channel that users auto-connect to (no manual "join" step).

Summary:
- Managed chat projection and Chat pane already exist; they are not fed by a live relay.
- This epic adds: (1) a configured default channel (relay + channel id or group id), (2) a Nostr subscription that requests kinds 40, 41, 42 and ingests events into the projection, (3) auto-selecting that channel when data exists, (4) persistence so history survives restart.

User story:
As a desktop user who has opened the Chat pane, I want to see a default NIP-28 public channel and its messages without clicking "join" or picking a channel, so that I can read and send messages in that channel immediately.

Success criteria:
- Opening Chat pane shows the default channel and transcript when the relay has data.
- New messages from the relay appear in the transcript.
- User can send a message; local echo and ack when relay accepts; message persists after restart.
- Event shapes follow Nostr Managed Chat Contract; implementation uses existing projection and Chat pane.

References:
- docs/plans/nip28-chat-channel-prd.md
- docs/kernel/nostr-managed-chat-contract.md
- docs/plans/nostr-group-chat.md

Delivery process:
- Work on a feature branch (not main).
- Open a PR per logical chunk (aim for one per day); preferred order: config -> subscription -> auto-select -> send -> persistence.
- Each PR must pass: workspace-dependency-drift-check, ownership-boundary-check, touched-clippy-gate, validate_registry.sh.
```

---

## Ticket A — Default channel config (AC1)

| Field | Value |
|-------|--------|
| **Issue type** | Story or Task |
| **Title** | [NIP-28] Define default channel configuration |
| **Epic Link** | Link to Epic above |

**Description:**

```
The app must have a configured default for the NIP-28 channel users auto-connect to.

Acceptance criteria:
- One default is defined (e.g. one relay URL + one channel id, or one group id and its first channel).
- Configuration can be hardcoded for the milestone or read from env/config.
- No UI for changing the default is required in this milestone.

Ref: PRD acceptance criterion 1.
```

---

## Ticket B — Subscription and ingestion (AC2)

| Field | Value |
|-------|--------|
| **Issue type** | Story or Task |
| **Title** | [NIP-28] Relay subscription and ingestion for default channel |
| **Epic Link** | Link to Epic above |

**Description:**

```
On startup (or when the Chat pane becomes active), the app must subscribe to the relay for the default channel's NIP-28 events and ingest them into the managed chat projection.

Acceptance criteria:
- Subscribe for kinds 40, 41, 42 (and optionally NIP-29 kinds if needed) for the default channel.
- For each received EVENT, call managed_chat_projection.record_relay_event(event) (or record_relay_events).
- Subscription is tied to the default channel config (relay URL + channel id or group id).

Ref: PRD acceptance criterion 2. See apps/autopilot-desktop/src/app_state/chat_projection.rs (ManagedChatProjectionState).

Note: No existing code path connects a Nostr relay to managed_chat_projection.record_relay_event.
This ticket includes adding that wiring - e.g. a background worker or runtime lane that opens a
WebSocket connection using crates/nostr/client, subscribes with the NIP-28 filter, and delivers
received events into managed_chat_projection. See apps/autopilot-desktop/src/runtime_lanes.rs and
crates/nostr/client/src/subscription.rs for the existing subscription plumbing to build on.
```

---

## Ticket C — Auto-select default channel (AC3)

| Field | Value |
|-------|--------|
| **Issue type** | Story or Task |
| **Title** | [NIP-28] Auto-select default channel when projection has data |
| **Epic Link** | Link to Epic above |

**Description:**

```
When the projection has at least one group and one channel (e.g. after first EOSE or first batch of events), the UI must show the managed chat workspace and select that default channel with no manual "join" or channel pick.

Acceptance criteria:
- When projection gains first group + channel, selected_workspace and projection's selected_group_id / selected_channel_id are set to the default channel.
- User sees the default channel transcript without clicking anything.

Ref: PRD acceptance criterion 3. See app_state.rs (selected_workspace, active_managed_chat_*).
```

---

## Ticket D — Chat pane transcript and send (AC4)

| Field | Value |
|-------|--------|
| **Issue type** | Story or Task |
| **Title** | [NIP-28] Chat pane shows default channel transcript and send path |
| **Epic Link** | Link to Epic above |

**Description:**

```
The existing Chat pane must show the default channel's transcript and composer; the user can send messages that are published to the relay and appear in the transcript (local echo + ack when relay accepts).

Acceptance criteria:
- Chat pane (apps/autopilot-desktop/src/panes/chat.rs) displays the default channel's messages when it is selected.
- Composer sends kind 42 events to the relay for the default channel.
- Local echo and ack (e.g. via existing outbound message flow) so the message appears and then confirms.

Ref: PRD acceptance criterion 4.
```

---

## Ticket E — Persistence (AC5)

| Field | Value |
|-------|--------|
| **Issue type** | Story or Task |
| **Title** | [NIP-28] Persist ingested events for default channel |
| **Epic Link** | Link to Epic above |

**Description:**

```
Ingested events (and optionally outbound state) must be persisted so that reopening the app or the Chat pane still shows the default channel and history from the projection file.

Acceptance criteria:
- Events added via record_relay_event(s) are included in the projection persistence (existing path: ~/.openagents/autopilot-managed-chat-projection-v1.json).
- After restart, opening the Chat pane shows the default channel and its history from the projection.

Ref: PRD acceptance criterion 5.

Note: Persistence may already work via the existing refresh_projection / persist_current_state
path in ManagedChatProjectionState once events flow through record_relay_event. Verify that this
path is triggered correctly by the new subscription worker; no new persistence code may be needed.
```

---

## Ticket F (optional) — NIP-29 join (AC6)

| Field | Value |
|-------|--------|
| **Issue type** | Story or Task |
| **Title** | [NIP-28] NIP-29 join for restricted default channel (optional) |
| **Epic Link** | Link to Epic above |

**Description:**

```
If the default channel lives in an NIP-29 group that requires membership, the app should send a join request (e.g. on first load) so the relay allows reading/writing.

Acceptance criteria:
- When default channel is in a restricted NIP-29 group, app sends kind 9021 join request as needed.
- Can be implemented as a follow-up if the first milestone uses a public channel.

Ref: PRD acceptance criterion 6 (optional for this milestone).
```

---

## Checklist

- [ ] Create Epic "NIP-28: Basic chat channel with auto-connect" and paste description.
- [ ] Create and link ticket A (default channel config).
- [ ] Create and link ticket B (relay subscription and ingestion).
- [ ] Create and link ticket C (auto-select default channel).
- [ ] Create and link ticket D (Chat pane transcript and send).
- [ ] Create and link ticket E (persist ingested events).
- [ ] Optionally create and link ticket F (NIP-29 join).
- [ ] Set Epic link (or parent) on all child issues.
