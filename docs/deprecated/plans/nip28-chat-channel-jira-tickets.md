# Jira Tickets for NIP-28 Chat First Milestone

Use this as the issue breakdown for the first NIP-28 chat milestone.

## Epic

- Title: `NIP-28: Basic chat channel with auto-connect`
- Goal: one default public NIP-28 channel that auto-connects, renders in the existing chat pane, and supports restart-safe transcript + send flow.

## Ticket A — Default channel config

- Add one default relay URL + channel ID.
- Allow overrides with env vars.
- No user-facing config UI required in this milestone.

## Ticket B — Relay subscription and ingestion

- Subscribe to kinds `40`, `41`, `42` for the default channel.
- Feed relay events into `ManagedChatProjectionState::record_relay_event`.
- Keep the implementation in desktop app wiring rather than reusable protocol crates.

## Ticket C — Auto-select default channel

- When the managed chat projection first has content, auto-select the managed chat workspace and first channel.
- Do this once, without clobbering an explicit user selection.

## Ticket D — Transcript and send path

- Reuse the existing chat pane transcript/composer.
- Publish kind-42 outbound events through the new lane.
- Preserve local echo plus ack/error transitions in the existing outbound message model.

## Ticket E — Persistence

- Confirm relay-ingested events persist through the managed chat projection document.
- Confirm the default channel transcript is visible again after restart.
