# What to harvest from Zed collab before retiring it

Date: 2026-07-25. Status: recorded before removal.

## Why this exists

Owner direction, 2026-07-25: retire Zed collab in Omega, *"unless there is
anything worth harvesting for our buzz-parity plan."*

There is. Not the backend — the parity ledger already says the backend is
Nostr — but the **GPUI panel shapes**. Zed's collab panel is a working,
shipped implementation of several interaction patterns the Buzz parity ledger
requires Omega to render natively. Deleting 6,200 lines without writing down
what they solved would be the real loss, because the next agent to build the
workroom pane would rediscover the same problems by hand.

This is a map, not a preservation order. The code goes. Git keeps it.

## What is being retired

| Crate or file | Lines | Fate |
| --- | --- | --- |
| `crates/collab_ui/src/collab_panel.rs` | 4307 | removed |
| `crates/collab_ui/src/channel_view.rs` | 719 | removed |
| `crates/collab_ui/src/collab_panel/channel_modal.rs` | 686 | removed |
| `crates/collab_ui/src/call_stats_modal.rs` | 270 | removed |
| `crates/collab_ui/src/collab_panel/contact_finder.rs` | 174 | removed |
| registration in `crates/zed/` | — | removed |

## What maps onto the parity ledger

The ledger rows come from
`docs/buzz/2026-07-24-omega-buzz-full-parity-recommendation.md`.

### Workrooms — "rooms, channels, nested threads, replies, reactions, pins, bookmarks, read state"

`collab_panel.rs` is the largest single reference. Worth reading before
building the Omega workroom pane:

- **Nested channel tree rendering** with expand/collapse state, depth-indented
  rows, and stable identity across refreshes. The ledger's "rooms, channels,
  nested threads" is this shape.
- **Drag and drop reparenting** of channels, including the rejection cases.
- **Context menus per row kind**, where the available actions differ by whether
  the row is a channel, a contact, or a call participant.
- **Unread and mention indicators** on tree rows — the ledger's "read state"
  and the Home/attention row's "unread work".

`channel_view.rs` is a collaborative document view bound to a channel. The
useful part is the binding pattern between a channel identity and an editor
buffer, which the "Code work" row needs in a Nostr-backed form.

### People and agents — "roster, roles, presence, status, profile, mention, team membership"

- `contact_finder.rs` is a small, complete fuzzy roster picker. The Omega
  equivalent needs to resolve Nostr identities instead of Zed contacts, but the
  picker shape transfers directly.
- `channel_modal.rs` handles membership management and invitation UI — the
  ledger's "team membership", and adjacent to the Governance row's "membership,
  role changes, revocation".

### Voice — "dictation, huddles, transcripts, recording controls"

`call_stats_modal.rs` renders live call quality. Marked a later milestone in
the ledger, so this is the least urgent, but it is the only worked example in
the tree of surfacing realtime media health to a user.

## What is deliberately not harvested

- **The collab server protocol and its data model.** The ledger is explicit:
  Workrooms are "native GPUI panes over the Nostr workroom log", and People and
  agents are "signed Nostr identity, profile, membership, and presence events".
  Carrying Zed's channel/contact model forward would fight that.
- **Zed's contact and channel identity.** Omega identity is Nostr keys.
- **The confirmation prompts** in `collab_panel.rs:2504,2535,2570`. These are on
  the omega#54 removal list and die with the feature rather than separately.

## Recovering the code

Everything is in git. The retirement commit is the parent to read from:

```sh
git show <retirement-commit>^:crates/collab_ui/src/collab_panel.rs
```

Nothing here should be restored wholesale. The value is in reading how a
shipped implementation handled tree state, drag targets, and per-row action
sets, then building the Nostr-backed equivalent.
