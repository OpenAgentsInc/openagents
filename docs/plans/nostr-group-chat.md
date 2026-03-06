# OpenAgents Nostr Group Chat Plan

Status: initial draft
Date: 2026-03-06

## Goal

Design a Discord-style group chat for OpenAgents Desktop that is:

- Nostr-first, not OpenAgents-proprietary by default
- compatible with our existing Nostr identity keypairs
- able to reuse our existing Bitcoin wallet surface
- deterministic and replay-safe
- good enough for real team chat, buyer/provider coordination, and operator groups

This should not turn the MVP into a generic social client. The first useful version should strengthen the existing Autopilot desktop workflow: group coordination around jobs, providers, buyers, and shared operator rooms.

## Recommendation

Build v1 around a hybrid model:

- `NIP-29` as the authoritative server/group layer
- `NIP-28` as the channel and channel-message layer inside those groups
- `NIP-17` + `NIP-44` + `NIP-59` for DMs and small ad hoc side rooms
- `NIP-42` for relay auth on restricted reads and writes
- `NIP-25` for reactions
- `NIP-51` + `NIP-65` for remembering groups/chats and routing relays
- `NIP-77` for efficient sync and replay-safe gap recovery where available
- `NIP-78` only for app-specific layout/read-state data if we need it

This is an extend-before-rewrite plan. The repo already retains core protocol modules for `NIP-17`, `NIP-28`, `NIP-42`, `NIP-44`, `NIP-59`, `NIP-65`, and `NIP-77` in `crates/nostr/core`, so the first implementation pass should build on those instead of introducing a parallel chat protocol layer.

Do not build v1 on `NIP-EE`. It is explicitly marked unrecommended and is useful here mainly as a warning: large private E2EE groups are hard, and we should keep a seam for a future Marmot/MLS-class transport without making this draft depend on it.

### Transport modes

The implementation should explicitly separate room authority from message transport so we can support multiple room types over time:

- managed public or semi-private server channels: `NIP-29` + `NIP-28`
- private DMs and small side rooms: `NIP-17` + `NIP-44` + `NIP-59`
- future large-group E2EE rooms: Marmot or another successor to `NIP-EE` / MLS

That means the desktop data model should not assume every room is backed by the same event kind family. It should assume:

- one room identity and membership model
- one transport mode per room
- one projection pipeline that can ingest different canonical event envelopes

## Why This Stack

### Why not only NIP-28

`NIP-28` gives us public chat channels and channel messages, but not strong Discord-style group ownership, roles, restricted membership, or relay-enforced moderation.

### Why not only NIP-17

`NIP-17` is the right default for DMs and small side chats, but it is a poor fit for server-like communities:

- no admins
- no join/invite/moderation model
- per-recipient fanout
- no natural "server" or shared channel structure

The NIP itself says large groups should use something more suitable.

### Why NIP-29 in the middle

`NIP-29` is the best fit for a Discord-style "server":

- relay-enforced membership
- admins and roles
- moderation events
- group metadata
- join and leave flows
- optional private or restricted reads and writes

`NIP-29` still lets the payload events be standard Nostr events. That makes it the right authority layer while keeping the content layer interoperable.

## Product Model

### Core objects

- Server: one `NIP-29` group
- Channel: one `NIP-28` channel definition inside that group
- Message: one `NIP-28` kind `42` channel message inside that group
- DM or side room: one `NIP-17` conversation

### Primary use cases

- provider team chat
- buyer/provider job coordination
- operator and support rooms
- private team rooms around Autopilot workflows
- 1:1 and small-group DMs

### Explicit non-goals for the first version

- voice/video
- generic public social feeds
- fully private large-group E2EE
- marketplace-grade paid rooms and role gating
- web/mobile parity

## Proposed Event Model

### 1. Server / workspace

Represent each server as a managed `NIP-29` group hosted on an OpenAgents-controlled relay or Nexus-adjacent relay service.

Use standard `NIP-29` events for:

- group metadata: `39000`
- admins: `39001`
- members when exposed: `39002`
- supported roles: `39003`
- join requests: `9021`
- leave requests: `9022`
- moderation and membership changes: `9000-9020`

This gives us the Discord-like server boundary.

### 2. Channels inside a server

Represent each channel with `NIP-28`:

- create channel: `40`
- update channel metadata: `41`

Every channel event inside a server should also carry:

- `["h", "<group-id>"]`

That lets the relay enforce membership and permissions through `NIP-29`, while clients still understand channel semantics through `NIP-28`.

### 3. Messages inside a channel

Represent channel messages with `NIP-28` kind `42`, plus the group tag:

- `["h", "<group-id>"]`
- `["e", "<kind-40-channel-create-id>", "<relay-url>", "root"]`
- `["e", "<parent-kind-42-id>", "<relay-url>", "reply"]` when threaded
- `["p", "<pubkey>", "<relay-url>"]` for mentions and reply notifications

This gives us a good mapping for:

- top-level channel timeline
- threaded replies
- channel-local message fanout
- server moderation through the group relay

### 4. Channel layout and category data

`NIP-28` does not define enough for Discord-style category ordering and channel presentation. We likely need one thin OpenAgents-specific layer for:

- category grouping
- channel ordering
- hidden or collapsed defaults
- channel type markers such as `text`, `announcement`, `ops`
- optional per-channel ACL hints

Preferred approach:

- start with extra tags on channel metadata events where possible
- if that becomes awkward, use one admin-authored `NIP-78` app-data event keyed per server, for example `d=oa:chat-layout:<group-id>`

Rule: this layout layer may shape presentation, but it must not become the authority for group membership, moderation, or message existence.

### 5. DMs and side rooms

Use `NIP-17` for:

- 1:1 DMs
- small ad hoc private rooms
- encrypted file messages via kind `15`

Use recipient `10050` relay lists for delivery, and keep these chats clearly separate from server channels in the UI.

Practical guardrail:

- if a chat needs roles, server moderation, or starts growing into a real shared room, move it into a `NIP-29` server channel instead of stretching `NIP-17`

### 5.5 Optional E2EE group rooms

We should explicitly preserve a future option for end-to-end encrypted group chat rooms.

Current posture:

- do not ship v1 group chat on `NIP-EE`
- treat `NIP-EE` as useful background material only
- treat Marmot as the current successor path to evaluate for large-group E2EE

Product rule:

- public and moderated server channels should still work without E2EE
- E2EE group rooms should be an optional room type, not a hidden protocol switch under every channel
- the room list and projection model should be able to distinguish:
  - standard server channel
  - DM / side room
  - secure group room

Engineering rule:

- do not tie server/channel UX to `kind 42` only
- keep a clean adapter boundary where a future Marmot/MLS room can provide:
  - membership snapshots
  - room metadata
  - message timeline items
  - reaction and reply references where supported
  - local decrypt / failure states

### 6. Reactions and moderation

Use:

- `NIP-25` reactions on channel messages and DMs
- `NIP-28` hide and mute events for personal client-side moderation where useful
- `NIP-29` relay-side delete, remove-user, and edit-metadata events for authoritative moderation

This lets us keep the split clear:

- personal hide/mute is client state
- actual room policy is server state

## Relevant NIPs To Lean On

| NIP | Use in this design |
| --- | --- |
| `01` | base event model, tags, subscriptions, sorting, relay protocol |
| `10` | root/reply tag markers for channel threads |
| `17` | DMs and small private rooms |
| `25` | reactions |
| `28` | channel creation, metadata, and channel messages |
| `29` | server/group authority, membership, moderation, roles |
| `42` | auth for restricted relay reads and writes |
| `44` | encryption primitive |
| `51` | remember public chats, groups, and DM relay lists |
| `59` | gift-wrap transport for private chats |
| `65` | relay list routing |
| `77` | sync and gap recovery |
| `78` | narrow app-specific layout or read-state sync if needed |

## Primal Patterns Worth Copying

The most useful patterns I found were in Primal's messaging and navigation surfaces, not in its generic feed views.

### Layout and navigation

From `primal-web-app/src/pages/DirectMessages.tsx` and `DirectMessages.module.scss`:

- fixed-width left rail for conversations
- a clean split between list rail and active transcript
- search mounted at the top of the message surface
- clear unread affordances

From `primal-web-app/src/components/NavMenu/NavMenu.tsx` and `NavLink.tsx`:

- strong active-state nav icons
- small unread bubbles that compress cleanly on smaller layouts
- minimal nav chrome with counts only where the count matters

### Conversation rendering

From `primal-web-app/src/components/DirectMessages/DirectMessageContent.tsx`:

- group adjacent messages by sender and recency into visual threads
- show avatar only when a thread segment ends
- show time labels at thread boundaries, not on every line
- support inline payment objects and richer content, not just plain text

### Composer behavior

From `primal-web-app/src/components/DirectMessages/DirectMessagesComposer.tsx`:

- auto-growing composer
- `Enter` sends, `Shift+Enter` inserts newline
- emoji and mention assist without turning the composer into a heavy editor

### Rich content parsing

From `primal-web-app/src/components/DirectMessages/DirectMessageParsedContent.tsx`:

- inline previews for media and links
- mention expansion and embedded note-style objects
- a message body parser that can upgrade plain text into richer message cells

### What not to copy literally

- Do not clone Primal's look.
- Do copy its information density, scroll behavior, unread affordances, and message grouping logic.
- Our version should feel desktop-native and WGPUI-native, closer to a control room than a consumer social app.

## Identity And Wallet Reuse

### Identity

Reuse the existing Nostr identity path and key derivation from `crates/nostr/core`.

That gives us:

- one consistent pubkey for Autopilot, providers, and chat identity
- existing relay-auth compatibility
- no parallel identity system

Future option:

- allow alias inbox keys for DMs where `NIP-17` and `NIP-59` benefit from better metadata protection

### Wallet

Reuse the existing Spark wallet surface from `crates/spark` and the desktop wallet pane.

Initial wallet-enabled chat actions:

- paste and render invoices in chat
- pay invoices from chat explicitly through the wallet flow
- request payment in chat
- tip message authors later

Rule: wallet state remains explicit and truthful. Chat should never imply payment success until the wallet confirms it.

## Sync, Ordering, And Replay Safety

Canonical source of truth:

- signed Nostr events
- relay `OK`, `EOSE`, and auth results

Derived local projection:

- servers
- channels
- messages
- reply trees
- unread counters
- mention counts
- reaction counts
- member roster cache

Rules:

- dedupe by event id across relays
- sort canonically by `created_at`, then event id
- show local echo separately until a relay accepts the event
- never depend on arrival order
- make unread state and local selection rebuildable after restart

Use `NIP-77` where available for:

- gap detection
- backfill
- efficient resync after reconnect

Fallback cleanly to normal `REQ` / `EOSE` when a relay does not support it.

## Spacetime's Role

Spacetime is optional and should be treated as a derived acceleration layer, not authority.

Good uses:

- presence and typing indicators
- synced read cursors
- fast unread counts
- local-first search indexes
- cold-start acceleration and cached projections

Bad uses:

- authoritative membership
- message truth
- moderation decisions
- wallet or payment truth

If we use Spacetime, the invariant must be:

- delete it and rebuild from Nostr events plus local private state without losing canonical chat history

## Ownership And Implementation Shape

This should respect the current repo boundaries.

### `apps/autopilot-desktop`

Own:

- server rail, channel rail, transcript, composer, member list
- unread and selection UX
- wallet-triggered chat actions
- operator and product-specific room flows

### `crates/nostr/core`

Own:

- builders and parsers for the `NIP-29` + `NIP-28` hybrid event model
- `NIP-17`, `NIP-28`, `NIP-42`, `NIP-44`, `NIP-59`, `NIP-65`, and `NIP-77` protocol helpers
- any OpenAgents-specific tag helpers that are still protocol-adjacent

### `crates/nostr/client`

Own:

- relay pooling
- subscriptions
- auth handshake support
- multi-relay dedupe support
- negentropy sync plumbing

### `crates/spark`

Own:

- wallet primitives only

### `crates/wgpui`

Own only reusable presentation primitives if they emerge naturally, for example:

- segmented tabs
- unread badges
- message bubble primitives
- generic virtualized list rails
- composer widgets

Do not move OpenAgents chat product logic into `wgpui`.

## Delivery Phases

### Phase 0: protocol and UX spike

- finalize the hybrid event and tag schema
- decide whether channel order lives in tags or one `NIP-78` layout event
- validate relay support and the Nexus-hosted group-relay story
- define the local projection schema

### Phase 1: read-only server chat

- join a managed group
- load channels
- render transcript
- sync unread counts locally
- support reconnect and replay-safe rebuild

### Phase 2: send, reply, react, DM

- composer
- optimistic local echo
- replies and threads
- reactions
- `NIP-17` DMs and small side rooms

### Phase 3: server operations

- create/edit channels
- join and leave flows
- member list
- moderation actions
- role-aware UI affordances

### Phase 4: wallet-aware chat actions

- invoice previews
- explicit pay/request flows from chat
- tips or payment shortcuts where useful

### Phase 5: optional Spacetime acceleration

- presence
- typing indicators
- synced read cursors
- faster cold start and search

### Phase 6: optional secure-group transport spike

- evaluate Marmot as the successor path to `NIP-EE`
- decide whether secure rooms are:
  - secure channels inside a managed server
  - standalone secure rooms linked from a server
- define device key, membership, welcome, and recovery UX
- prove replay-safe local state storage and rekey handling

## Suggested GitHub Issue Backlog

The list below is the initial issue set needed to build this plan out fully. The titles are phrased so they can be opened directly as GitHub issues.

### 1. Define OpenAgents managed-chat event contract

Description:

- Specify the exact hybrid contract for `NIP-29` groups plus `NIP-28` channels/messages.
- Define required tags, optional tags, and OpenAgents-specific extension tags.
- Lock down how `h`, `e`, `p`, `subject`, ordering, category hints, and channel type hints are represented.

Supporting material:

- `/Users/christopherdavid/code/nips/01.md`
- `/Users/christopherdavid/code/nips/10.md`
- `/Users/christopherdavid/code/nips/28.md`
- `/Users/christopherdavid/code/nips/29.md`
- `/Users/christopherdavid/code/openagents/docs/plans/nostr-group-chat.md`

### 2. Add NIP-29 protocol helpers to crates/nostr/core

Description:

- Implement reusable builders/parsers/validators for group metadata, roles, membership, join/leave, and moderation events.
- Keep app-specific product behavior out of the crate.
- Add fixtures and tests for serialization and tag parsing.

Supporting material:

- `/Users/christopherdavid/code/nips/29.md`
- `/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md`
- `/Users/christopherdavid/code/openagents/crates/nostr/core/src/nip28.rs`
- `/Users/christopherdavid/code/openagents/crates/nostr/core/src/nip17.rs`

### 3. Add hybrid channel helpers for NIP-28-in-NIP-29 rooms

Description:

- Extend the Nostr crate layer with helpers for channel create/update/message events that also carry group context.
- Encode reply threading and mention rules consistently.
- Validate that channel events can be projected deterministically across relays.

Supporting material:

- `/Users/christopherdavid/code/nips/28.md`
- `/Users/christopherdavid/code/nips/29.md`
- `/Users/christopherdavid/code/nips/10.md`
- `/Users/christopherdavid/code/openagents/crates/nostr/core/src/nip28.rs`

### 4. Implement managed group relay support in Nexus-adjacent infrastructure

Description:

- Decide whether Nexus directly owns managed group relay behavior or fronts a dedicated relay role.
- Support group metadata reads, restricted writes, role enforcement, join requests, leave requests, and moderation.
- Ensure relay-enforced truth, not client-enforced pretend moderation.

Supporting material:

- `/Users/christopherdavid/code/nips/29.md`
- `/Users/christopherdavid/code/nips/42.md`
- `/Users/christopherdavid/code/openagents/docs/MVP.md`
- `/Users/christopherdavid/code/openagents/docs/plans/nostr-group-chat.md`

### 5. Implement relay AUTH flow for restricted group chat reads and writes

Description:

- Add end-to-end `NIP-42` auth handling for chat subscriptions and message publishing.
- Surface auth-required and restricted relay failures clearly in the desktop UI.
- Reuse the existing relay client/auth primitives instead of inventing a chat-only auth path.

Supporting material:

- `/Users/christopherdavid/code/nips/42.md`
- `/Users/christopherdavid/code/openagents/crates/nostr/client/src/relay.rs`
- `/Users/christopherdavid/code/openagents/crates/nostr/core/src/nip42.rs`

### 6. Build desktop room list, channel rail, and transcript shell

Description:

- Add the Discord-style shell in `apps/autopilot-desktop`: server/workspace rail, channel rail, transcript pane, header, and composer slot.
- Keep the UI WGPUI-native and aligned with existing pane architecture.
- Match the best interaction patterns from Primal without copying its visual style.

Supporting material:

- `/Users/christopherdavid/code/primal/primal-web-app/src/pages/DirectMessages.tsx`
- `/Users/christopherdavid/code/primal/primal-web-app/src/pages/DirectMessages.module.scss`
- `/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/panes/chat.rs`
- `/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md`

### 7. Implement local chat projection store and replay-safe rebuild

Description:

- Build the canonical local projection for servers, channels, messages, reply trees, reactions, unread counters, and member roster cache.
- Make restart/reconnect rebuild deterministic from relay events plus local private state.
- Do not let arrival order become truth.

Supporting material:

- `/Users/christopherdavid/code/nips/01.md`
- `/Users/christopherdavid/code/nips/77.md`
- `/Users/christopherdavid/code/openagents/docs/MVP.md`
- `/Users/christopherdavid/code/openagents/docs/plans/nostr-group-chat.md`

### 8. Add read-only managed group chat browsing

Description:

- Support joining or opening a managed server, listing channels, backfilling channel history, and rendering read-only transcripts.
- This is the minimum product slice for validating the event model and relay behavior before sending is enabled.

Supporting material:

- `/Users/christopherdavid/code/nips/28.md`
- `/Users/christopherdavid/code/nips/29.md`
- `/Users/christopherdavid/code/primal/primal-web-app/src/components/DirectMessages/DirectMessageConversation.tsx`

### 9. Implement composer, send, local echo, ack, retry, and failure states

Description:

- Add message composition, optimistic local echo, relay acceptance tracking, resend paths, and explicit failed-send states.
- Preserve truthful status instead of silently assuming publish success.

Supporting material:

- `/Users/christopherdavid/code/nips/01.md`
- `/Users/christopherdavid/code/primal/primal-web-app/src/components/DirectMessages/DirectMessagesComposer.tsx`
- `/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/panes/chat.rs`

### 10. Implement threaded replies, mentions, and reactions

Description:

- Support channel-root messages, threaded replies, mention tags, notification hooks, and `NIP-25` reactions.
- Keep message threading deterministic and channel-local.

Supporting material:

- `/Users/christopherdavid/code/nips/10.md`
- `/Users/christopherdavid/code/nips/25.md`
- `/Users/christopherdavid/code/nips/28.md`
- `/Users/christopherdavid/code/primal/primal-web-app/src/components/DirectMessages/DirectMessageContent.tsx`

### 11. Define and implement channel layout metadata

Description:

- Decide how category grouping, channel ordering, room types, and collapsed defaults are stored.
- Prefer standard tags where possible.
- Fall back to one narrow `NIP-78` app-data document only if the standard channel metadata becomes too awkward.

Supporting material:

- `/Users/christopherdavid/code/nips/28.md`
- `/Users/christopherdavid/code/nips/78.md`
- `/Users/christopherdavid/code/openagents/docs/plans/nostr-group-chat.md`

### 12. Implement DMs and small private side rooms with NIP-17

Description:

- Add 1:1 DMs and small encrypted side rooms using `NIP-17`, `NIP-44`, and `NIP-59`.
- Reuse existing Nostr identity and recipient DM relay lists.
- Keep this room type visually distinct from managed server channels.

Supporting material:

- `/Users/christopherdavid/code/nips/17.md`
- `/Users/christopherdavid/code/nips/44.md`
- `/Users/christopherdavid/code/nips/59.md`
- `/Users/christopherdavid/code/nips/51.md`
- `/Users/christopherdavid/code/openagents/crates/nostr/core/src/nip17.rs`

### 13. Add rich message rendering for media, links, invoices, and payment objects

Description:

- Render embedded media, link previews, note references where relevant, and wallet-related message objects such as invoices and payment requests.
- Keep rendering deterministic and avoid side effects in the parser layer.

Supporting material:

- `/Users/christopherdavid/code/primal/primal-web-app/src/components/DirectMessages/DirectMessageParsedContent.tsx`
- `/Users/christopherdavid/code/primal/primal-web-app/src/components/DirectMessages/DirectMessageContent.tsx`
- `/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/panes/wallet.rs`

### 14. Implement unread state, read cursors, and notification counts

Description:

- Add per-room unread counts, last-read tracking, mention counts, and navigation badges.
- Decide which parts remain local-only and which parts sync privately.
- Keep unread rebuildable after reconnect and replay.

Supporting material:

- `/Users/christopherdavid/code/primal/primal-web-app/src/components/NavMenu/NavMenu.tsx`
- `/Users/christopherdavid/code/primal/primal-web-app/src/components/NavLink/NavLink.tsx`
- `/Users/christopherdavid/code/openagents/docs/plans/nostr-group-chat.md`

### 15. Implement member list, roles, and server moderation tools

Description:

- Surface group admins, members, roles, invite/join/leave state, message deletion, mute/remove-user controls, and metadata editing affordances.
- Reflect actual relay policy and server state, not speculative UI state.

Supporting material:

- `/Users/christopherdavid/code/nips/29.md`
- `/Users/christopherdavid/code/nips/28.md`
- `/Users/christopherdavid/code/openagents/docs/MVP.md`

### 16. Add gap recovery and efficient sync with NIP-77 fallback behavior

Description:

- Use `NIP-77` negentropy where supported for backfill and reconciliation.
- Fall back to normal `REQ` / `EOSE` flows when not supported.
- Prove that the projection stays deterministic in both modes.

Supporting material:

- `/Users/christopherdavid/code/nips/77.md`
- `/Users/christopherdavid/code/openagents/crates/nostr/core/src/nip77/mod.rs`
- `/Users/christopherdavid/code/openagents/crates/nostr/client/src/subscription.rs`

### 17. Add chat-driven wallet actions and explicit payment status

Description:

- Let users open wallet actions from messages: pay invoice, request invoice, copy address, inspect payment status.
- Never imply settlement success before Spark confirms it.
- Reuse the current wallet primitives and desktop wallet panes.

Supporting material:

- `/Users/christopherdavid/code/openagents/crates/spark/src/lib.rs`
- `/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/panes/wallet.rs`
- `/Users/christopherdavid/code/openagents/docs/MVP.md`

### 18. Add presence, typing, and search acceleration behind an optional Spacetime adapter

Description:

- Introduce Spacetime only for derived accelerators such as presence, typing indicators, read cursors, and faster search.
- Make sure deleting Spacetime state does not lose canonical chat history.

Supporting material:

- `/Users/christopherdavid/code/openagents/docs/plans/nostr-group-chat.md`
- `/Users/christopherdavid/code/openagents/docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`

### 19. Evaluate Marmot / MLS secure-group rooms as an optional room type

Description:

- Spike end-to-end encrypted large-group chat as a future room type.
- Treat `NIP-EE` as deprecated background material and Marmot as the current successor path to evaluate.
- Answer the product and engineering questions:
  - secure channels inside a managed server vs standalone secure rooms
  - device key handling
  - welcome / invite flow
  - rekeying
  - local encrypted state storage
  - recovery and multi-device behavior

Supporting material:

- `/Users/christopherdavid/code/nips/EE.md`
- `/Users/christopherdavid/code/pylon/docs/nips/EE.md`
- `/Users/christopherdavid/code/openagents/docs/plans/nostr-group-chat.md`

### 20. Build end-to-end regression coverage for managed chat, DMs, and optional secure rooms

Description:

- Add protocol fixtures, relay-behavior tests, projection rebuild tests, UI interaction tests, and wallet-in-chat tests.
- Include reconnect, duplicate relay delivery, out-of-order events, auth failures, moderation events, DM unwrap failures, and future secure-room adapters.

Supporting material:

- `/Users/christopherdavid/code/openagents/crates/nostr/core/tests`
- `/Users/christopherdavid/code/openagents/crates/nostr/client/tests`
- `/Users/christopherdavid/code/openagents/docs/plans/nostr-group-chat.md`

## Main Risks And Open Questions

1. `NIP-29` gives us the right authority model, but real-world relay support is still thinner than baseline `NIP-01` + `NIP-17`. We should assume OpenAgents will need to host the best-supported implementation path.
2. `NIP-28` does not fully define Discord-style categories, ordering, or per-channel permissions. We will almost certainly need one very small OpenAgents-specific layout layer.
3. Private large-group chat remains unresolved if we want strong end-to-end confidentiality. This draft should leave a clean seam for a future Marmot or MLS-class secure group transport.
4. We need to decide whether Nexus itself becomes the managed group relay or whether it fronts a separate chat relay role.
5. Read-state sync is product-critical but not protocol-critical. We should decide early which parts stay local, which parts sync privately, and which parts are purely ephemeral.

## Bottom Line

The right first draft is not "Discord on Nostr" in the abstract.

It is:

- `NIP-29` for managed servers
- `NIP-28` for channels and channel messages
- `NIP-17` for DMs
- Primal-style rails, unread badges, grouped bubbles, and fast composer behavior
- optional Spacetime only as a projection and presence accelerator
- existing OpenAgents Nostr identity and Spark wallet reused directly

That gives us a pragmatic v1 with clear upgrade paths instead of forcing one NIP to do every job badly.
