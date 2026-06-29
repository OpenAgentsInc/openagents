# MMO-style multiple characters per account — Verse presence layer

Date: 2026-06-21
Branch: `verse/mmo-characters-per-account`
Scope: **presence layer only** (avatar identity + rendering). No inventory,
stats, save-game, or per-character progression — just "who is visible, and as
whom."

## Problem

Two Autopilot Desktop instances cannot see each other's avatars in the Verse.

Root cause: the world module derives the avatar key **only** from the
SpacetimeDB identity. In the deleted legacy world module:

```rust
fn avatar_ref_for_sender(ctx: &ReducerContext) -> String {
    format!("avatar.identity.{}", ctx.sender())
}
```

`join_region` / `set_avatar_position` / `leave_region` (and the chat/emote/
intent/attention reducers) all call this with no notion of a character. Since
`avatar_ref` is the primary key of `agent_avatar` and `avatar_position`, one
account = exactly one avatar row. Two instances sharing one account/token
collapse onto the **same** row and overwrite each other's position.

The desktop client compounded the confusion: it self-filtered remote avatars
against a hardcoded constant `CHAT_WORLD_DESKTOP_AVATAR_REF =
"avatar.desktop.local"` (`apps/autopilot-desktop/src/shared/chat-world-multiplayer.ts`),
a value that never matched any real avatar key the module wrote.

## The model

Two orthogonal axes:

- **Account** = the SpacetimeDB identity (`ctx.sender()`) / Pylon agent. A token
  shared across instances is fine and expected. Ownership: a client can only
  ever write avatars under its own identity.
- **Character** = a client-supplied `character_id`. One account can field MANY
  characters simultaneously, each a distinct, separately-visible avatar.

The avatar key becomes:

```
avatar.identity.<sender>.char.<character_id>
```

Embedding the sender keeps ownership automatic (the identity segment is server-
authoritative — the module always uses `ctx.sender()`, never a client value),
while the `<character_id>` segment lets one account split into many avatars.

### Character selection

A character is chosen **per app launch** via the `OA_CHARACTER` env var,
defaulting to a stable per-install value `"main"`. Resolved through the same env
path as every other Verse setting (`chatWorldCharacterId()` in
`apps/autopilot-desktop/src/shared/chat-world-flags.ts`), so there is no new RPC
surface.

```
OA_CHARACTER=main  ./AutopilotDesktop   # instance A
OA_CHARACTER=alt   ./AutopilotDesktop   # instance B, SAME account
```

→ two characters, both visible to each other and to everyone else.

### The separate "distinct account" axis

Multiple *accounts* (rather than multiple characters of one account) remain a
separate axis: give each instance its own `PYLON_HOME` / profile and webview
`partition` so they obtain distinct SpacetimeDB identities. That path is out of
scope for this PR, which focuses on **multiple characters per one account**.
Because the identity segment partitions the key space, the two axes compose
cleanly: account A's `main`/`alt` and account B's `main` are three distinct
avatars.

## Changes

### 1. World module (the deleted legacy world module)

- Added a `character_id: String` parameter to every reducer that derives an
  avatar key: `join_region`, `set_avatar_position`, `leave_region`,
  `focus_pylon`, `clear_pylon_focus`, `send_local_message`, `send_pylon_message`,
  `send_emote`, `set_agent_intent`.
- Changed the helper to `avatar_ref_for_sender(ctx, character_id) ->
  "avatar.identity.<sender>.char.<sanitized id>"`, factored through a pure
  `avatar_ref_for_identity(identity, character_id)` so it is unit-testable.
- Added `sanitize_character_id`: trims, keeps only `[A-Za-z0-9._-]`, bounds to
  64 chars, and falls back to `"main"` when empty. It never errors, so a junk id
  degrades gracefully (worst case: two junk ids collapse onto `main` for that
  one account) rather than breaking presence.
- All other behavior is identical (validation, rate limits, region bounds,
  stale expiry, attention/chat/emote/intent semantics).

### 2. Historical generated TS bindings

This document predates the Cloudflare/Effect world cutover. The generated web
binding directory described here was deleted during the hard decommission pass;
the live contract now flows through `packages/world-contract` and
`packages/world-client`.

### 3. Desktop client (`apps/autopilot-desktop`)

- `chatWorldCharacterId()` resolves `OA_CHARACTER` (default `"main"`).
- `subscribeCloudflareWorld` resolves the character id and plumbs it into the
  multiplayer client. The `joinRegion` / `setAvatarPosition` / `leaveRegion`
  reducer calls now carry `characterId`.
- The connection's `onConnect(ctx, identity, token)` callback now yields the
  live SpacetimeDB identity (hex) up through `onConnected`. Once known, the
  client computes `localAvatarRef = "avatar.identity." + identity + ".char." +
  characterId` and re-paints so the scene picks it up.
- Self-filter fix: the projection carries `localAvatarRef`; `view.ts` feeds
  `multiplayer.localAvatarRef` (falling back to the legacy constant only pre-
  connect, when there are no remote rows yet) into the scene layer. Each
  instance now hides **only its own character** and renders all others —
  including other characters of the same account.

The hex normalization (`identityToHex`) and character-id sanitization
(`sanitizeChatWorldCharacterId`) mirror the Rust helpers so the client-computed
self-filter key matches the `avatar_ref` the module actually writes.

## How two instances now render each other

1. Both launch with the same account token but `OA_CHARACTER=main` and
   `OA_CHARACTER=alt`.
2. Each `join_region("...", display, character_id)` → the module writes a
   distinct `agent_avatar` + `avatar_position` row keyed
   `avatar.identity.<sender>.char.main` and `...char.alt`.
3. Each instance subscribes to the region's avatars/positions and self-filters
   on its **own** per-character key, so instance A (main) renders alt, and
   instance B (alt) renders main. Both avatars move independently.

## Scene stability

This change is additive to the presence projection and reducer args. Avatar/
pose/stream updates still flow through the same dispatch path; nothing here
remounts the Three scene or resets pose. The self-filter switch from the
pre-connect fallback to the real per-character key happens via a normal snapshot
re-dispatch, not a scene rebuild.

## Test / verify plan

Deterministic checks (no manual screenshots):

- **World module** (`cargo test` in the deleted legacy world module):
  distinct `character_id`s under one sender → distinct avatar keys (→ distinct
  rows); same id is stable (so `leave_region(character_id)` targets exactly one
  character); same id on different identities never collides; sanitization is
  bounded and safe.
- **Desktop**
  (`apps/autopilot-desktop/src/shared/chat-world-multiplayer.test.ts`,
  `tests/chat-world-subscriptions.test.ts`,
  `tests/chat-world-cloudflare.test.ts`): avatar-key construction +
  sanitization; the projection carries `localAvatarRef`; reducer calls carry
  `characterId`; and an integration-style test that, once the identity is known,
  the scene self-filters **only** the local character while rendering the same
  account's other character and unrelated remotes.
- **Gate**: `bun run check:deploy` (includes the desktop typecheck + Verse test
  bundle) green.

Manual confirmation (optional): launch two instances with `OA_CHARACTER=a` and
`OA_CHARACTER=b` on the same account → two avatars visible to each other.
