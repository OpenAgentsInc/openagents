# NIP-28 Chat Channel (First Milestone) — PRD

**Status:** draft  
**Date:** 2026-03-09  
**Implements first milestone of:** [Nostr Group Chat Plan](nostr-group-chat.md)  
**Contract:** [Nostr Managed Chat Contract](../kernel/nostr-managed-chat-contract.md)

---

## Summary

This milestone ships a **basic NIP-28 public chat channel** that desktop users **auto-connect to**—no manual “join channel” step for the default case. Today the managed chat projection and panes UI exist and can display groups, channels, and messages, but no live relay subscription feeds events into the projection; data only comes from the persisted projection file or tests. This deliverable adds a relay subscription for the default channel, ingests NIP-28 events (kinds 40, 41, 42) into the existing projection, and ensures the Chat pane shows that channel and its transcript as soon as data is available.

---

## Goals

- User sees a **default NIP-28 channel** and its messages without clicking “join” or picking a channel.
- **Messages flow from the relay** into the existing managed chat projection and appear in the Chat pane transcript.
- **Auto-select** the default channel when the projection has at least one group and one channel (e.g. after first EOSE or first batch).
- **Send path** works: user can compose and send messages; they are published to the relay and appear in the transcript (local echo + ack when relay accepts).
- Event shapes and tags follow the [Nostr Managed Chat Contract](../kernel/nostr-managed-chat-contract.md); implementation reuses the existing projection and panes UI.

---

## Current state (context)

From the Phase 0 audit:

- **In place:** NIP-28 and NIP-29 protocol modules in `crates/nostr/core`; managed chat helpers (NIP-28-in-NIP-29) in `managed_chat.rs`; `ManagedChatProjectionState` and `record_relay_events` in `apps/autopilot-desktop/src/app_state/chat_projection.rs`; Chat pane in `apps/autopilot-desktop/src/panes/chat.rs` that already renders managed groups, channels, and messages when the projection has data; selection and persistence (projection file at `~/.openagents/autopilot-managed-chat-projection-v1.json`).
- **Missing for this milestone:** A live Nostr relay subscription that requests kinds 40, 41, 42 (and optionally NIP-29 kinds) and calls `managed_chat_projection.record_relay_event(event)` on each received event; a defined default channel (relay URL + channel id or group id) and configuration for it; logic to auto-select the default channel when the projection first gains content.

---

## Scope

**In scope**

- One default NIP-28 channel (relay URL + channel id, or group id and its first channel).
- Auto-connect behavior: subscribe to the relay for that channel’s events on startup (or when Chat pane becomes active) and ingest into the existing projection.
- Auto-select that channel in the UI when the projection has at least one group and one channel.
- Integration with the existing Chat pane: transcript, composer, send; local echo and ack when relay accepts.
- Persistence of ingested events (and outbound state) so reopening the app still shows the default channel and history.
- No new pane type is required; the existing Chat pane (`apps/autopilot-desktop/src/panes/chat.rs`) is the integration surface.

**Out of scope (this milestone)**

- NIP-29 join flows (sending 9021 join request; can be a follow-up if the default channel lives in a restricted group).
- Multiple channels / server picker UX beyond the single default.
- DMs, moderation UX, or other transports.
- Changes to the kernel contract or to NIP-28/NIP-29 protocol crates beyond what is needed to wire the subscription.

---

## User story (primary)

**As a** desktop user who has opened the Chat pane,  
**I want** to see a default NIP-28 public channel and its messages without clicking “join” or picking a channel,  
**So that** I can read and send messages in that channel immediately.

### Acceptance criteria

1. **Default channel is defined.** The app has a configured default (e.g. one relay URL + one channel id, or one group id and its first channel). Configuration can be hardcoded for the milestone or read from env/config.

2. **Subscription and ingestion.** On startup (or when the Chat pane becomes active), the app subscribes to the relay for the default channel’s NIP-28 events (kinds 40, 41, 42 as needed) and ingests received events into the existing managed chat projection (`record_relay_event` / `record_relay_events`).

3. **Auto-select when data exists.** When the projection has at least one group and one channel (e.g. after first EOSE or first batch of events), the UI shows the managed chat workspace and selects that default channel (no manual “join” or channel pick).

4. **Panes UI.** The existing Chat pane ([`apps/autopilot-desktop/src/panes/chat.rs`](../../apps/autopilot-desktop/src/panes/chat.rs)) shows the channel’s transcript and composer; the user can send messages that are published to the relay and appear in the transcript (local echo + ack when relay accepts).

5. **Persistence.** Ingested events (and optionally outbound state) are persisted so that reopening the app or the Chat pane still shows the default channel and history from the projection file.

6. **NIP-29 join (optional for this milestone).** If the default channel lives in an NIP-29 group that requires membership, the app sends a join request (e.g. on first load) so the relay allows reading/writing; otherwise this can be a follow-up.

---

## Success criteria

- Opening the Chat pane shows the default channel and its transcript when the relay has data for that channel.
- New messages from the relay for the default channel appear in the transcript without a manual refresh.
- The user can send a message; it appears as local echo and is confirmed when the relay accepts it; it remains visible after restart (persisted).
- Event shapes and tags comply with the [Nostr Managed Chat Contract](../kernel/nostr-managed-chat-contract.md); the implementation uses the existing projection types and the existing Chat pane.

---

## References

- [Nostr Managed Chat Contract](../kernel/nostr-managed-chat-contract.md) — normative event contract for managed servers and NIP-28 channels.
- [Nostr Group Chat Plan](nostr-group-chat.md) — product and event model; this PRD implements the first milestone (basic NIP-28 channel + auto-connect).
- [MVP Spec](../MVP.md) — overall Autopilot product; NIP-28 chat supports the desktop workflow and is not the core “earn sats” loop.

---

## Delivery process

- Work on a feature branch, not `main` (e.g. `feat/nip28-chat-channel`).
- Open a PR per logical chunk; aim for at least one per day.
- Preferred PR order mirrors ticket order: config → subscription → auto-select → panes send → persistence.
- Each PR must pass the repo validation gates before merge:
  - `scripts/lint/workspace-dependency-drift-check.sh`
  - `scripts/lint/ownership-boundary-check.sh`
  - `scripts/lint/touched-clippy-gate.sh`
  - `scripts/skills/validate_registry.sh`
