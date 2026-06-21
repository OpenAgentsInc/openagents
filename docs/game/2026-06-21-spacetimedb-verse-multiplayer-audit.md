# SpacetimeDB Verse Multiplayer Audit

Date: 2026-06-21

## Summary

The Verse can become multiplayer without inventing a new backend. The owned
`openagents-world` SpacetimeDB module already has most of the multiplayer
schema and reducers: regions, avatars, avatar positions, pylon focus, local
messages, chat bubbles, emotes, and agent intent. The desktop app also has a
subscription adapter and projection layer that can read those rows.

The missing work is not "add SpacetimeDB." It is to finish the client loop:
connect the live third-person controller pose to the SpacetimeDB position
reducers, render remote avatars as real world characters instead of abstract
training entities, scope subscriptions to what is nearby/on-screen, and make the
new larger Verse/Street map match the region bounds in the module.

The target should be simple and concrete:

- Two users open the Autopilot desktop Verse.
- Each gets one authoritative SpacetimeDB avatar identity for the current
  region.
- Each player's controller pose is published at a bounded rate.
- Other users appear on the map as animated remote avatars.
- Pylon/training data stays owned by the existing OpenAgents projection path;
  SpacetimeDB is the multiplayer presence and interaction layer, not the source
  of business or training truth.

## Sources Reviewed

OpenAgents implementation:

- `apps/openagents-world-spacetimedb/src/lib.rs`
- `apps/openagents-world-spacetimedb/README.md`
- `apps/autopilot-desktop/src/shared/chat-world-spacetimedb.ts`
- `apps/autopilot-desktop/src/shared/chat-world-multiplayer.ts`
- `apps/autopilot-desktop/src/shared/chat-world-visualization.ts`
- `apps/autopilot-desktop/src/ui/chat-world-subscriptions.ts`
- `apps/autopilot-desktop/tests/chat-world-spacetimedb.test.ts`
- `apps/autopilot-desktop/tests/chat-world-subscriptions.test.ts`
- `docs/game/2026-06-17-spacetimedb-openagents-mmo-database-plan.md`
- `docs/game/2026-06-17-spacetimedb-tassadar-integration-next-steps.md`

Local SpacetimeDB references:

- `projects/spacetime/repos/spacetimedb-typescript-sdk/examples/quickstart-chat/src/App.tsx`
- `projects/spacetime/repos/spacetimedb-cookbook/multiple-position-updates/README.md`
- `projects/spacetime/repos/spacetimedb-cookbook/multiple-position-updates/rust-module/src/lib.rs`
- `projects/spacetime/repos/spacetimedb-minecraft/crates/module/src/player.rs`
- `projects/spacetime/repos/spacetimedb-minecraft/crates/module/src/entity.rs`
- `projects/spacetime/repos/spacetimedb-terminal-bench/tasks/multiplayer-counter/instruction.md`
- `projects/spacetime/repos/BitCraftPublic/README.md`

## Current OpenAgents State

### World Module

`apps/openagents-world-spacetimedb/src/lib.rs` already defines the core tables
needed for multiplayer presence:

- `world_region`
- `pylon_station`
- `agent_avatar`
- `avatar_position`
- `pylon_attention`
- `local_chat_message`
- `chat_bubble`
- `local_emote`
- `agent_intent`

The same module also keeps training/projection rows:

- `training_run`
- `run_entity`
- `world_edge`
- `proof_ref`
- `settlement_ref`
- `world_event`
- `projection_cursor`
- `bridge_health`

That split is correct. Training truth, settlement truth, and public projection
truth should continue to come from the current OpenAgents source systems. The
SpacetimeDB module should coordinate shared world presence and local interaction
around that projection.

Browser/user reducers already exist for the multiplayer path:

- `join_region`
- `leave_region`
- `set_avatar_position`
- `focus_pylon`
- `clear_pylon_focus`
- `send_local_message`
- `send_pylon_message`
- `send_emote`
- `set_agent_intent`

The server side is already doing useful safety work:

- A sender may update only its own avatar.
- Avatar positions are bounded by region extents.
- Position writes are throttled by minimum interval.
- Movement is capped by maximum meters per second.
- Stale positions and ephemeral interaction rows have expiry windows.

The main mismatch is scale. The default region extents in the module are still
small relative to the current Verse world:

- X: `-8..8`
- Y: `0..4`
- Z: `-6..6`

That was acceptable for the first Tassadar diagram. It is too small for the
new Street-adjacent scene where the user can walk around, approach pylons, and
see a large road/city into the distance.

### Desktop Client

`apps/autopilot-desktop/src/ui/chat-world-subscriptions.ts` already has a real
SpacetimeDB client path behind the `CHAT_WORLD_MULTIPLAYER` flag. It builds a
generated `DbConnection`, stores the token in local storage, subscribes to the
world queries, joins the region, and wires table insert/update/delete callbacks
into the desktop world store.

`apps/autopilot-desktop/src/shared/chat-world-multiplayer.ts` defines the
current subscription shape. It subscribes to the selected run's world events,
the selected region, station rows, avatar rows, avatar positions, attention,
local messages, chat bubbles, emotes, and agent intents.

`apps/autopilot-desktop/src/shared/chat-world-spacetimedb.ts` has the critical
client-side write planner:

- `planChatWorldAvatarPositionWrite`
- `ChatWorldAvatarPositionWrite`
- bounds validation
- movement speed validation
- minimum write interval validation
- local identity derivation

`apps/autopilot-desktop/src/shared/chat-world-visualization.ts` can already
turn multiplayer rows into renderable world entities. That is enough to prove
the path, but it is not enough for the final Verse experience. Remote users
should become animated avatars using the same character/avatar pipeline as the
local controller, not generic graph entities.

### Current Gap

The live third-person player controller is not yet the canonical writer into
SpacetimeDB. The desktop has pieces of the pipe, but the scene needs one
explicit bridge:

```
local controller pose
  -> local multiplayer pose publisher
  -> planChatWorldAvatarPositionWrite
  -> connection.reducers.setAvatarPosition
  -> SpacetimeDB avatar_position table
  -> subscribed remote clients
  -> remote avatar interpolation/rendering
```

That bridge should be a small, testable service. It should not be embedded as
ad hoc reducer calls inside the renderer frame loop.

### 2026-06-21 Progress: Scene Refresh Must Not Reset The Local Avatar

Before the pose publisher can become trustworthy, the local controller must
survive ordinary desktop projection refreshes. A first follow-up landed in
`three-effect` (`ebe616f`) so the Foldkit training-run element captures the
active local controller pose before a visualization remount and restores that
position into the refreshed scene. OpenAgents now pins that commit.

This does not complete the multiplayer pose publisher, but it removes a blocker
for #5888: a normal Pylon/training/public-activity refresh should no longer
snap the avatar back to the default spawn while the Verse is active.

### 2026-06-21 Progress: #5888 Controller Pose Publishing

The first multiplayer slice now has a desktop-owned `VerseMultiplayerClient`
around the existing SpacetimeDB subscription connection. The client owns
join/leave lifecycle, keeps the last accepted local pose per region, and exposes
`publishLocalPose(pose)` for the Verse controller path.

Local controller pose writes now flow through the existing
`planChatWorldAvatarPositionWrite` guard before any
`set_avatar_position` reducer call. Moving poses can publish at the server's
10 Hz minimum when the region allows it; stationary idle poses are reduced to a
bounded keepalive. Out-of-bounds, too-fast, impossible-jump, missing-region, and
missing-reducer writes are suppressed client-side before they reach
SpacetimeDB. Subscription teardown now attempts `leave_region` before
disconnecting.

The shared `three-effect` training-run element now emits bounded
`local-pose-changed` events from the third-person controller. Autopilot Desktop
maps those renderer events into Foldkit messages and a fire-and-forget command
that publishes through the active multiplayer client only while
`CHAT_WORLD_MULTIPLAYER` is enabled and connected. The Verse remains
single-player-first when SpacetimeDB is unavailable.

## What The SpacetimeDB References Teach

### TypeScript Quickstart

The SpacetimeDB TypeScript quickstart uses the same basic client pattern
OpenAgents already started:

- Build a `DbConnection` with URI, module/database name, and stored token.
- Store the token returned after connect.
- Subscribe using `subscriptionBuilder().subscribe([...])`.
- Register table callbacks for row insert/update/delete.
- Call reducers from UI/client events.
- Remove callbacks and disconnect during cleanup.

OpenAgents is aligned with that pattern. The next work is lifecycle and scene
integration, not a new connection design.

### Multiple Position Updates Cookbook

The SpacetimeDB cookbook's multiple-position example is the most relevant
scaling reference. It separates internal high-frequency position state from
publicly subscribed position rows and publishes two public feeds:

- high-resolution nearby positions
- lower-resolution far positions

The important design lesson is not the exact table names. It is that every
client should not subscribe to every entity at the same fidelity forever. The
Verse can start with one region query, but it needs a path to proximity-based
subscriptions as soon as multiple users and pylons are active.

Recommended OpenAgents adaptation:

- MVP: keep `avatar_position` as the single public table, filtered by region.
- Next: add region/chunk or proximity filtered subscriptions.
- Later: add `avatar_position_hr` and `avatar_position_lr`, or a chunked
  equivalent, if a single table becomes too noisy.

### SpacetimeDB Minecraft Reference

The Minecraft reference keeps server-side tracking rows for which players can
see which entities. `StdbEntityView` and tracked-player logic are useful because
they make visibility explicit instead of forcing each client to receive the
entire world.

Recommended OpenAgents adaptation:

- Do not start with a full entity-view table unless needed.
- Add a bounded "visible avatars near this region/chunk" query first.
- If the Verse grows beyond one loaded region, add server-maintained view rows
  so clients subscribe only to avatars and interactables they can plausibly see.

### BitCraft Public Reference

The BitCraft reference reinforces the broad architecture: SpacetimeDB is strong
when tables and reducers define the shared simulation contract. For OpenAgents,
that means multiplayer presence and local interactions should be expressed as
tables/reducers. It does not mean moving OpenAgents product promises, payouts,
training traces, or settlement authority into the world database.

## Target Multiplayer Experience

The first acceptable multiplayer Verse should feel like this:

- The user opens the Autopilot desktop app and lands in the Verse.
- The local third-person controller remains primary.
- Other connected users appear as avatars in the same region.
- Remote avatars are animated, lit, and interpolated; they do not snap between
  points.
- Remote user names appear only in compact, readable contexts: selected,
  hovered, nearby, or intentionally revealed.
- Tab targeting selects visible/nearby pylons or avatars, not every object in
  the whole simulation.
- Pylon labels remain minimal. Non-pylon training entities are inspectable
  through target selection, not always-on text.
- If SpacetimeDB is down, the Verse still works as a single-player local scene
  with a visible degraded multiplayer state in diagnostics only.

## Proposed Architecture

### Runtime Flow

1. Verse mounts.
2. If `CHAT_WORLD_MULTIPLAYER` is enabled, the desktop opens the SpacetimeDB
   connection.
3. The client subscribes to the active run and active region.
4. After subscription applies, the client calls `join_region`.
5. The player controller publishes a local pose event when the pose changes.
6. A multiplayer pose publisher throttles writes and calls
   `planChatWorldAvatarPositionWrite`.
7. Valid writes call `set_avatar_position`.
8. Remote clients receive `avatar_position` row changes.
9. The renderer interpolates remote avatars into the scene.
10. On teardown, the client calls `leave_region`; stale server rows expire if a
    client crashes or loses network.

### Client Ownership

The desktop should own one `VerseMultiplayerClient`-style service. Its job:

- hold the SpacetimeDB connection
- expose connection state for diagnostics
- expose a `publishLocalPose(pose)` method
- expose current remote avatar snapshots
- own join/leave lifecycle
- keep reducer calls out of visual components

This can wrap the existing `subscribeSpacetimeWorld` logic rather than replace
it. The current code already knows how to connect and subscribe; it needs a
stable publisher interface the player controller can call.

### Pose Contract

Use a small pose shape that does not leak renderer internals:

```ts
type VerseAvatarPose = {
  regionRef: string
  x: number
  y: number
  z: number
  yaw: number
  animation: "idle" | "walk" | "run"
  capturedAtMs: number
}
```

Map `animation` into SpacetimeDB either by extending `avatar_position` or by
using `agent_intent`/emote-style ephemeral rows for a first pass. The cleaner
long-term path is to add a compact motion state to position rows so a remote
avatar can choose the same idle/walk/run clips as the local controller.

### Region And Coordinate Contract

The current default region bounds are too small. The Verse needs a map contract:

- one starter region around the user's pylon/Tassadar site
- a visible Street corridor nearby
- room for the Street to continue in both directions
- future chunking along the Street

Recommended MVP bounds:

- X: at least `-160..160`
- Y: `0..40`
- Z: at least `-120..120`

The region should treat the Tassadar pylon site as just off the road, not
centered on the road. The Street can be visualized as wrapping around a much
larger sphere/arc, but the multiplayer coordinate system should remain local
Cartesian chunks for now. Do not make clients reason about spherical coordinates
until cross-region traversal exists.

### Remote Avatar Rendering

Remote avatars should use the same visual system as the local player wherever
possible:

- same robot/avatar model family
- same lighting/material assumptions
- same animation mixer clips
- interpolation between network updates
- stale fade or disappearance after timeout
- selected/hover ring using the target selection visual language

Do not render remote people as permanent text labels. Name labels should be
stateful and conditional: selected, hovered, close, or explicitly revealed by
HUD.

### Selection

Tab targeting should use camera-visible, nearby candidates only:

- frustum-visible
- not occluded if cheap ray checks are available
- within a maximum distance
- sorted by screen-center distance and world distance
- prefer pylons and avatars over tiny training artifacts

When a remote avatar is selected:

- add a ground ring or small halo
- show a compact HUD panel with name, pylon/ref, status, distance, and last
  update age
- optionally show recent local chat/emote state

## Implementation Plan

### Phase 1: Desktop Multiplayer Client Wiring

Files:

- `apps/autopilot-desktop/src/ui/chat-world-subscriptions.ts`
- `apps/autopilot-desktop/src/shared/chat-world-spacetimedb.ts`
- `apps/autopilot-desktop/src/shared/chat-world-multiplayer.ts`

Tasks:

- Wrap the existing subscription path in a small client/service with an
  explicit pose publisher.
- Store last accepted local write per region/avatar.
- Feed controller position/yaw into `planChatWorldAvatarPositionWrite`.
- Publish at 5-10 Hz while moving; publish less often or send keepalive while
  idle.
- Call `leave_region` on unmount/disconnect when possible.
- Keep all behavior behind `CHAT_WORLD_MULTIPLAYER`.

Acceptance:

- Fake connection test proves controller poses call `set_avatar_position`.
- Out-of-bounds and too-fast writes remain suppressed.
- Disabling the flag makes zero SpacetimeDB calls.

### Phase 2: Remote Avatar Rendering

Files:

- `apps/autopilot-desktop/src/shared/chat-world-visualization.ts`
- `apps/autopilot-desktop/src/shared/chat-world-game-layer.ts`
- `/Users/christopherdavid/work/three-effect` avatar/controller/rendering
  primitives

Tasks:

- Convert remote `agent_avatar` + `avatar_position` rows into remote avatar
  render instances.
- Use the real avatar model/animation path, not geometric placeholders.
- Interpolate between received positions.
- Hide remote labels by default.
- Render selected/hover rings consistently with pylon selection.

Acceptance:

- Two clients see each other as animated avatars.
- The local client does not render a duplicate of itself.
- Remote avatars disappear or fade after stale timeout.

### Phase 3: Region Scale And Street Coordinates

Files:

- `apps/openagents-world-spacetimedb/src/lib.rs`
- `apps/openagents-world-spacetimedb/README.md`
- desktop region selection/projection files

Tasks:

- Increase starter region extents to match the current Verse Street scene.
- Add explicit region metadata for road direction, origin, and starter pylon
  site offset.
- Keep the road visually infinite with repeated/streamed geometry while using
  bounded multiplayer chunks internally.
- Add future region IDs for adjacent Street chunks.

Acceptance:

- User can walk around the pylon site and toward the Street without hitting
  multiplayer bounds immediately.
- Remote positions remain valid throughout the visible starter scene.

### Phase 4: Proximity And Subscription Scope

Files:

- `apps/openagents-world-spacetimedb/src/lib.rs`
- `apps/autopilot-desktop/src/shared/chat-world-multiplayer.ts`

Tasks:

- Filter avatar subscriptions by active region and, if available, active chunk.
- Stop subscribing to global `agent_avatar` rows once the region grows.
- Keep pylon/training detail rows separate from avatar presence.
- Consider adding server-maintained visibility rows if region filtering is not
  enough.

Acceptance:

- Tab targeting and rendering do not cycle through off-screen/off-region users.
- A crowded region can degrade gracefully.

### Phase 5: High/Low Resolution Presence

Only do this once region-level filtering is insufficient.

Possible schema:

- private/internal `avatar_position_internal`
- public `avatar_position_near`
- public `avatar_position_far`

Or keep one table and add chunk/proximity indexes if that is enough.

Reference:

- `spacetimedb-cookbook/multiple-position-updates`

Acceptance:

- Nearby avatars update smoothly.
- Far avatars are cheap and lower frequency.
- Subscription count and row churn stay bounded.

### Phase 6: Ops And Release Gates

Tasks:

- Add a local two-client smoke for the desktop multiplayer path.
- Add a generated-bindings freshness check or document the exact regeneration
  command in the deployment runbook.
- Verify `spacetime.openagents.com` connection failure remains non-fatal.
- Keep pre-push deploy checks green.

Acceptance:

- `bun run check:agent-doc-links`
- relevant desktop tests for SpacetimeDB projection/subscriptions
- relevant world module tests/build
- repo deploy gate passes before push

## Concrete Gaps To Fix

1. The player controller pose is not yet a first-class publisher to
   `set_avatar_position`.
2. The desktop connection/subscription helper does not expose a clean
   multiplayer client interface to the controller loop.
3. Remote avatar rendering currently routes through generic world entity
   projection; it needs the real character/avatar rendering path.
4. The default region bounds are too small for the new Verse/Street scene.
5. The generated TypeScript bindings are imported from the web app path; the
   desktop packaging story should be made explicit or moved into a shared
   generated package.
6. Subscription queries still include some broad/global rows, especially
   avatars and attention. This is tolerable for MVP but wrong for scale.
7. There is no high/low resolution position feed yet.
8. Tab targeting and labels need to use visibility and selection rules, not raw
   object enumeration.
9. Multiplayer identity currently maps from SpacetimeDB identity plus local
   display metadata. It must stay public and never leak private host/device
   details.
10. The failure path needs to remain single-player-first: if SpacetimeDB is
    down, the Verse should still load and move.

## Recommended First Issue

Implement Phase 1 and Phase 2 together as the first real multiplayer slice:

> Wire the Autopilot desktop Verse controller into SpacetimeDB avatar presence
> and render remote users as animated avatars.

Definition of done:

- Start two desktop app instances or one desktop plus one test harness.
- Both join the same `openagents-world` region.
- Movement in one instance appears in the other within one second.
- Remote avatar uses the real model/animation path.
- Local avatar is not duplicated.
- The feature is controlled by `CHAT_WORLD_MULTIPLAYER`.
- Existing SpacetimeDB subscription tests pass.

## Guardrails

- Do not move training, settlement, promise, payout, or proof authority into
  SpacetimeDB.
- Do not let browser clients write pylon/training projection rows.
- Do not leak local paths, hostnames, device IDs, tokens, or private agent
  session details into avatar metadata.
- Do not make labels permanent for every world object.
- Do not block the Verse on SpacetimeDB availability.

## Decision

Use SpacetimeDB for live multiplayer presence and local interaction in the
Verse. Keep OpenAgents' existing projection/authority systems as the source of
truth for training and business state. Finish the desktop client pose publisher
and remote avatar renderer first; then expand the region/chunk model to support
the Street-scale world.
