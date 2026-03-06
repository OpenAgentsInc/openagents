# Nostr Managed Chat Contract

Status: draft
Owners: desktop, nostr/core, nexus chat relay
Last updated: 2026-03-06

## Purpose

This document defines the initial managed-chat event contract for OpenAgents Desktop.

It is the normative contract for:

- managed server/workspace chat backed by `NIP-29`
- channel metadata and channel messages backed by `NIP-28`
- the OpenAgents extension tags needed to make those surfaces feel Discord-like

This document does not define DMs in full, wallet settlement, or the future secure-group room transport. It does define how those room types fit into the same room model so the desktop can support multiple chat transports without rewriting its core projection model.

## Scope

This contract covers:

- server identity
- channel identity
- channel metadata
- channel message shape
- reply and mention behavior
- category and ordering hints
- channel type hints
- canonical sort and projection rules

This contract does not yet cover:

- server moderation UX details
- full unread/read-cursor sync semantics
- media-upload contracts
- secure-group MLS/Marmot payload formats

## Design goals

- Nostr-first
- deterministic and replay-safe
- relay-enforced server truth
- minimal OpenAgents-specific extension surface
- room-model compatibility across managed channels, DMs, and future secure-group rooms

## Room model

The desktop room model MUST support these room transport types:

- `managed-channel`
  - authority: `NIP-29`
  - timeline: `NIP-28`
- `dm`
  - authority and timeline: `NIP-17` + `NIP-44` + `NIP-59`
- `secure-group`
  - reserved for a future Marmot / MLS successor path

For this document, only `managed-channel` is normative.

## Managed server identity

A managed server is one `NIP-29` group.

Canonical server id:

- `<host>'<group-id>` when a full relay-qualified id is available
- `<group-id>` as the local authority key inside one known relay domain

Managed-channel events MUST carry:

- `["h", "<group-id>"]`

The `h` tag is the relay-enforced server membership boundary.

## Event families

### Server events

OpenAgents managed servers use standard `NIP-29` event families:

- `39000` group metadata
- `39001` group admins
- `39002` group members when exposed
- `39003` group roles
- `9000-9020` moderation and membership control
- `9021` join request
- `9022` leave request

### Channel events

Managed channels use `NIP-28` event families inside the `NIP-29` server boundary:

- `40` channel create
- `41` channel metadata update
- `42` channel message

All channel events and messages MUST include the `h` tag for the enclosing group.

## Channel identity

The canonical channel id is the `kind:40` channel-create event id.

OpenAgents clients MAY additionally expose a stable UI slug for routing and local references using:

- `["oa-slug", "<slug>"]`

Rules:

- `oa-slug` is a convenience identifier, not the authoritative channel id
- if two channels share the same slug, the `kind:40` event id wins
- channel references in protocol-visible message tags MUST still point to the `kind:40` event id

## Required and optional OpenAgents extension tags

The following non-single-letter tags are reserved for OpenAgents managed chat:

- `oa-slug`
  - stable human-readable slug
- `oa-channel-type`
  - one of:
    - `text`
    - `announcement`
    - `ops`
    - `support`
    - `system`
- `oa-category`
  - stable category id
- `oa-category-label`
  - category display name hint
- `oa-position`
  - decimal sort key for channels inside a category
- `oa-room-mode`
  - one of:
    - `managed-channel`
    - `dm`
    - `secure-group`

Rules:

- unknown `oa-*` tags MUST be ignored by clients that do not understand them
- these tags are UI and projection hints, not authority replacements
- membership, moderation, and write access MUST still come from `NIP-29` relay behavior

## Channel create contract

Channel create is `kind:40`.

Required fields:

- `kind = 40`
- `content = <serialized channel metadata JSON>`
- `tags` MUST include:
  - `["h", "<group-id>"]`
  - `["oa-room-mode", "managed-channel"]`

Recommended tags:

- `["oa-slug", "<slug>"]`
- `["oa-channel-type", "<type>"]`
- `["oa-category", "<category-id>"]`
- `["oa-category-label", "<category-label>"]`
- `["oa-position", "<decimal-sort-key>"]`

The metadata JSON SHOULD include the normal `NIP-28` fields:

- `name`
- `about`
- `picture`
- `relays`

### Example

```json
{
  "kind": 40,
  "content": "{\"name\":\"provider-ops\",\"about\":\"Provider coordination room\",\"picture\":\"\",\"relays\":[\"wss://chat.openagents.example\"]}",
  "tags": [
    ["h", "oa-main"],
    ["oa-room-mode", "managed-channel"],
    ["oa-slug", "provider-ops"],
    ["oa-channel-type", "ops"],
    ["oa-category", "operations"],
    ["oa-category-label", "Operations"],
    ["oa-position", "120"]
  ]
}
```

## Channel metadata update contract

Channel metadata update is `kind:41`.

Required fields:

- `kind = 41`
- `content = <serialized channel metadata JSON>`
- `tags` MUST include:
  - `["h", "<group-id>"]`
  - `["e", "<channel-create-event-id>", "<relay-url>", "root"]`

Recommended tags:

- the same `oa-*` tags used on channel create when those hints change

Authorship rule:

- the authoritative channel metadata stream MUST be emitted by the server's configured channel-authority pubkey for that channel
- clients MUST ignore conflicting `kind:41` metadata updates from other pubkeys even if they are group members

This keeps `NIP-28`'s “same-author” expectation aligned with managed server behavior.

## Channel message contract

Channel messages are `kind:42`.

Required root-message tags:

- `["h", "<group-id>"]`
- `["e", "<channel-create-event-id>", "<relay-url>", "root"]`

Reply-message tags:

- all root-message tags above, plus
- `["e", "<parent-kind-42-id>", "<relay-url>", "reply", "<parent-author-pubkey>"]`

The `content` field is plain text.

### `e` tag rules

Managed-channel messages use `NIP-10` markers:

- the `root` marker always points to the `kind:40` channel-create event id
- the `reply` marker, when present, points to the direct parent `kind:42` message

This means the "thread root" for a channel is the channel itself, not the first top-level message.

### `p` tag rules

`p` tags MUST be used only for direct notification targets:

- the author of the parent message when replying
- explicitly mentioned users

`p` tags MUST NOT be expanded to the whole participant roster.

This keeps notification semantics bounded and avoids unnecessary fanout.

### `subject` tag rules

For managed channels:

- `subject` MUST NOT be used for channel identity, channel naming, or channel ordering
- `subject` SHOULD NOT be emitted on ordinary `kind:42` messages in v1
- if used later, it MUST be treated as a message-local topic hint only

Channel names and descriptions belong in the channel metadata JSON, not in message `subject` tags.

### Examples

Root message:

```json
{
  "kind": 42,
  "content": "Provider runtime deploy starts at 18:00 UTC.",
  "tags": [
    ["h", "oa-main"],
    ["e", "6f3c...channelcreate", "wss://chat.openagents.example", "root"]
  ]
}
```

Reply:

```json
{
  "kind": 42,
  "content": "Acknowledged. I will monitor relay auth failures during rollout.",
  "tags": [
    ["h", "oa-main"],
    ["e", "6f3c...channelcreate", "wss://chat.openagents.example", "root"],
    ["e", "ad91...parentmessage", "wss://chat.openagents.example", "reply", "f0ab...parentauthor"],
    ["p", "f0ab...parentauthor", "wss://chat.openagents.example"]
  ]
}
```

## Category and channel ordering

Managed clients MUST derive channel ordering using:

1. `oa-category` ascending by category id when present
2. `oa-position` ascending as decimal integer when present
3. channel metadata `name` ascending
4. channel create event id ascending

Rules:

- channels without `oa-category` are treated as belonging to the implicit `_uncategorized` bucket
- channels without `oa-position` sort after channels with a numeric position in the same category
- `oa-category-label` is display-only; `oa-category` is the stable grouping key

This is intentionally a hint-based contract. A richer channel-layout authority document may still be added later, but this is enough to make channel order deterministic now.

## Canonical timeline ordering

Within one channel timeline, clients MUST sort messages by:

1. `created_at` ascending
2. event id ascending

Arrival order MUST NOT be treated as truth.

Across relays:

- duplicate events are deduped by event id
- the same event observed from multiple relays is one logical message

## Projection rules

The local managed-chat projection MUST treat these as authoritative:

- `NIP-29` relay-accepted membership and moderation state
- `kind:40` channel-create events
- authoritative `kind:41` channel metadata updates
- `kind:42` messages accepted by at least one configured relay

The projection MUST treat these as derived:

- unread counts
- read cursors
- mention counters
- search indexes
- typing/presence overlays

## Interop and forward compatibility

Clients that only understand plain `NIP-28` can still parse:

- channel metadata JSON
- channel root/reply relationships
- message content

They will ignore:

- `h`
- `oa-*` extension tags

That is acceptable. OpenAgents-specific tags improve managed server UX, but the underlying channel and message shapes remain Nostr-readable.

## Relationship to DMs and future secure groups

This contract intentionally does not reuse `kind:42` for every room type.

Instead:

- managed server channels use this contract
- DMs and small private side rooms use `NIP-17`
- future secure-group rooms use a separate secure-group adapter, likely Marmot / MLS successor based

The desktop room model MUST select rendering and projection behavior by room transport type, not by assuming all rooms are `kind:42` timelines.

## Current implementation notes

This document is intended to drive:

- issue `#3018` protocol helpers in `crates/nostr/core`
- issue `#3019` hybrid channel helpers
- issue `#3020` relay authority behavior
- issue `#3022` desktop shell work
- issue `#3027` channel layout work

If the contract changes, those issue implementations must be updated with it.
