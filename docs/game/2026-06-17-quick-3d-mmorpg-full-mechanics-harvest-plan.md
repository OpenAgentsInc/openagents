# Quick 3D MMORPG Full Mechanics Harvest Plan

Date: 2026-06-17
Status: Deep audit and phased implementation plan.
Reference repo: `/Users/christopherdavid/work/projects/repos/Quick_3D_MMORPG`

## Goal

Harvest the complete mechanics vocabulary from `Quick_3D_MMORPG` into the
OpenAgents game/world stack.

"100%" here means every mechanic in the reference app is inventoried, mapped to
an OpenAgents destination, and either implemented, deliberately deferred, or
rejected with a clear reason. It does not mean vendoring the reference app as a
whole. The reference repo under `projects/repos/` remains read-only source
material. Large code or asset copies should not happen by default.

The immediate destination remains:

- `three-effect`: shared Three.js, controller, animation, model, terrain,
  particle, picking, interpolation, and world-render primitives.
- `openagents/apps/openagents.com`: `/tassadar` and later MMO-world surfaces
  that compose shared primitives against real OpenAgents state.
- deleted legacy world module: live world interaction state,
  proximity, avatar position, local chat, and public-safe world rows.
- Existing OpenAgents Worker/D1 authority: run truth, proof, receipt,
  settlement, payout, product-promise, pylon, and training authority.

## Source Coverage

Audited source files:

- Client runtime: `client/src/main.js`, `entity.js`, `entity-manager.js`,
  `finite-state-machine.js`.
- Rendering and scene: `threejs_component.js`, `gltf-component.js`,
  `render-component.js`, `load-controller.js`, `loading-manager.js`.
- Player and camera: `player-input.js`, `player-entity.js`,
  `player-state.js`, `third-person-camera.js`.
- Network and remotes: `network-controller.js`,
  `network-player-controller.js`, `network-entity-controller.js`,
  `spawners.js`, `npc-entity.js`.
- Gameplay components: `health-component.js`, `attacker-controller.js`,
  `inventory-controller.js`, `equip-weapon-component.js`,
  `quest-component.js`, `spatial-grid-controller.js`.
- Labels and effects: `floating-name.js`, `health-bar.js`,
  `particle-system.js`, `blood-effect.js`, `sorceror-effect.js`,
  `level-up-component.js`.
- Terrain and world dressing: `terrain.js`, `terrain-chunk.js`,
  `terrain-builder.js`, `terrain-builder-threaded.js`,
  `terrain-builder-threaded-worker.js`, `terrain-shader.js`,
  `texture-splatter.js`, `textures.js`, `quadtree.js`,
  `scenery-controller.js`.
- Shared data/math: `client/shared/defs.mjs`,
  `client/shared/spatial-hash-grid.mjs`, `terrain-height.mjs`,
  `terrain-constants.mjs`, `noise.mjs`, `simplex-noise.mjs`,
  `math.mjs`, `spline.mjs`.
- Server: `server/index.mjs`, `world-server.mjs`, `login-queue.mjs`,
  `world-manager.mjs`, `world-client.mjs`, `world-entity.mjs`.

Approximate audited code volume is 8.5k lines across client, shared, and server
source. The reference app is small enough to account for every subsystem, but
it is a demo MMO, not a production authority model.

## Existing Harvest Already Started

The first OpenAgents harvest from this repo has already landed in
`three-effect` and `/tassadar`:

- `three-effect/packages/core/src/playerControllerPrimitives.ts`
  - WASD plus pointer-lock mouselook.
  - Third-person follow camera.
  - MMORPG-style local character movement state.
- `three-effect/packages/core/src/assetPrimitives.ts`
  - GLTF loading and skinned clone support.
  - model render options, material texture assignment, shadows,
    frustum-culling policy, animation mixer handle, and disposal helpers.
- `three-effect/packages/core/src/trainingRun.ts`
  - `cameraMode: "perspective_walk"`.
  - `controller: "wasd_mouselook"`.
  - selection-first clicks before pointer lock.
  - center-reticle raycast selection while locked.
  - camera-facing text-label handles for the run title and node labels.

That is phase one, not the full mechanics harvest.

## Implementation Progress

As of 2026-06-17:

- `three-effect` issue #3 landed typed spatial hash grid, hit-target registry,
  and minimum-distance layout helpers in
  `packages/core/src/spatialPrimitives.ts`.
- `three-effect` issue #4 landed row interpolation plus stale/despawn helpers
  in `packages/core/src/mmoEntityPrimitives.ts`.
- `three-effect` issue #5 landed animation FSM and locomotion transition
  helpers in `packages/core/src/animationPrimitives.ts`.
- `three-effect` issue #6 landed bone attachment, equipment attachment, and
  disposal helpers in `packages/core/src/attachmentPrimitives.ts`.
- `three-effect` issue #7 landed billboard name plate, status bar, speech
  bubble, and combined entity overlay primitives in
  `packages/core/src/billboardPrimitives.ts`.
- `three-effect` issue #8 landed spline particle emitters and evidence-backed
  event burst gating in `packages/core/src/eventBurstPrimitives.ts`.
- OpenAgents issue #5272 landed a first-class SpacetimeDB `world_region`
  contract, service-owned region upsert, region-bounded avatar/station/chat
  writes, region-specific avatar cadence/stale expiry, updated bridge
  projection, and regenerated web bindings.
- `three-effect` support commit `d87a79f` moved the training-run renderer's
  node/entity picking path onto the shared `HitTargetRegistry`, so unlocked
  clicks and pointer-locked center-reticle clicks use the same shared raycast
  primitive.
- OpenAgents issue #5273 bound `/tassadar` station/avatar rendering to the new
  spatial primitives. Browser snapshots now subscribe to `world_region`,
  require a matching `avatar_position` row before drawing an avatar, apply the
  shared spatial hash/minimum-distance layout helper to row-backed stations and
  avatars, carry layout-adjusted chat bubbles with their anchor rows, and use
  the region row for movement/controller bounds.
- OpenAgents issue #5274 enforces the real-entity-only canvas rule in
  `/tassadar`: lifecycle/status categories stay in a compact text HUD, the
  spatial graph keeps only the real run node plus authority-backed pylon,
  station, avatar, proof, receipt, corpus, and chat entities, loss/chart chrome
  remains hidden, and no beams/bursts or transfer dots are emitted without
  evidence-backed rows.
- `three-effect` issue #9 landed the first terrain substrate in
  `packages/core/src/terrainPrimitives.ts`: deterministic seeded height
  sampling, stable chunk keys, quadtree LOD planning, synchronous and
  module-worker-backed chunk geometry building, skirt vertices, splat
  texture-index/weight output, and a width-keyed chunk pool. This is isolated
  substrate only; no OpenAgents page consumes terrain by default.
- OpenAgents issue #5275 added the owned world asset catalog and provenance
  policy in `docs/game/2026-06-17-openagents-world-asset-catalog.md`. It records
  current `/tassadar` procedural assets as approved-owned, defines target
  avatar/station/prop/material/adornment IDs, and gates Quick reference assets
  by source/license/production eligibility.

The original issue sequence is now implemented through the owned asset catalog.
Next work should move from substrate/catalog into actual owned avatar and prop
asset production, a machine-readable asset manifest, and only then runtime model
consumption.

## Mechanics Inventory

### 1. Runtime Shell And ECS

Quick uses a simple entity/component system:

- `Entity` owns position, quaternion, components, handlers, and a dead flag.
- Components register handlers and broadcast events by topic.
- `EntityManager` keeps active/inactive entities and destroys dead entities.
- `finite-state-machine.js` supplies generic state transitions.
- `main.js` wires controllers, spawners, login, grid, terrain, loader, and the
  `requestAnimationFrame` update loop.
- The frame delta is clamped to `1 / 30` seconds before update.

OpenAgents harvest:

- Do not port the ECS wholesale into `openagents.com`.
- Add small shared primitive contracts in `three-effect` only where the pattern
  helps render loops:
  - `FrameClock` / clamped delta helper.
  - disposable entity handles.
  - eventless `update(delta)` handle convention.
  - optional typed lightweight scene entity registry for render-only objects.
- Keep authoritative world state in SpacetimeDB/Worker projections rather than
  in a browser ECS.

### 2. Scene, Camera, Lighting, Fog, Sky

Quick scene setup:

- `WebGLRenderer` with shadow maps.
- `PerspectiveCamera` with 60 degree field of view.
- custom fog shader chunk overrides.
- directional light follows the player.
- hemisphere light.
- cube skybox with star texture.
- dat.GUI debugging controls.

OpenAgents harvest:

- `three-effect` should provide named scene presets, not app-local setup:
  - `createMmoRendererPreset`.
  - `createMmoLightingRig`.
  - `createWorldFogMaterialPatch` or a safer shader chunk utility.
  - `createSkyboxEnvironment`.
- `/tassadar` should keep the current full-bleed canvas and add only minimal HUD
  overlays. No large boxed dashboard chrome in the primary scene.
- Scene presets must not create fake activity. Lighting/fog/sky are ambience;
  pulses, beams, or moving data must remain evidence-backed.

### 3. Asset Loading And Model Instances

Quick asset mechanics:

- `LoadController` caches textures and FBX/GLTF assets.
- Queued callbacks wait on the first in-flight load.
- Skinned GLTF models are cloned with `SkeletonUtils.clone`.
- render components apply shadows, material options, texture maps by material
  name, visibility, bounding boxes, and object cleanup.
- animated model components create `AnimationMixer` instances and broadcast
  loaded model/bone events.

OpenAgents harvest:

- `assetPrimitives.ts` already covers the first GLTF path.
- Next additions:
  - cache/in-flight dedupe layer for textures and GLTFs.
  - FBX/OBJ loading only if a current asset requires it.
  - bone map extraction helper.
  - named animation clip resolver by fuzzy label.
  - render handle that binds a model to a live world entity row.
  - `onLoaded` and `onDisposed` lifecycle hooks.
- Prefer GLB/GLTF as the canonical OpenAgents asset format. FBX should be a
  source import path, not a long-term runtime dependency unless unavoidable.

### 4. Player Input, Movement, And Camera

Quick player mechanics:

- keyboard state for W/A/S/D, Shift, Space, Backspace.
- mouseup raycast selection with `Box3` against pickable meshes.
- local player movement with acceleration/deceleration.
- `A`/`D` yaw the character, `W`/`S` move along local forward.
- `Shift` selects run speed.
- movement is blocked during attack/death/dance states.
- collision checks nearby spatial-grid entities.
- Y position is pinned to terrain height.
- third-person camera computes ideal offset/look-at, clamps above terrain, and
  lerps using `1 - 0.01^delta`.

OpenAgents harvest:

- The first-person controller is already live.
- Add:
  - action map abstraction so pointer lock, third-person, and future avatar
    modes share keyboard state.
  - collision adapter against world entities and station bounds.
  - selectable hit-target adapter for lock-first and select-first surfaces.
  - third-person avatar mode for actual visible player/avatar bodies.
  - optional ghost/fly mode for operators.
  - camera presets: overview, walk, third-person chase, inspect-selected.
- `/tassadar` should let an unlocked click select a node if hit, otherwise enter
  pointer lock. While locked, center-reticle click should inspect a hit node.

### 5. Character FSM And Animation

Quick animation states:

- `idle`, `walk`, `run`, `attack`, `death`, `dance`.
- idle/walk/run crossfade.
- walk/run preserve animation phase when switching between each other.
- attack and dance loop once and return to idle.
- death loops once and clamps.
- `player.action` broadcasts the current state.

OpenAgents harvest:

- Add `three-effect` animation FSM primitives:
  - named state definitions.
  - clip lookup by label list.
  - crossfade transition policy.
  - phase-preserving transitions for locomotion.
  - one-shot action state with finished callback.
  - disabled movement states.
- OpenAgents mappings:
  - `idle`, `walk`, `run` for viewer/avatar movement.
  - `inspect`, `talk`, `work`, `verify`, `settle`, `blocked` as OpenAgents
    specific animation states once real avatar models exist.
  - Avoid literal combat states on `/tassadar` unless an actual game surface
    calls for combat. Training proof work is not combat.

### 6. Networked Entities And Interpolation

Quick network mechanics:

- Socket.IO to `ws://localhost:3000`.
- login FSM waits for `login.commit`.
- server sends local `world.player` packet.
- client sends transform every 0.1 seconds.
- server sends `world.update` snapshots.
- remote entity controller queues transforms and interpolates over 0.1 seconds.
- remote entities die if stale for 10 seconds.
- nearby entity descriptions are only sent when not already cached.

OpenAgents harvest:

- Do not port Socket.IO.
- Port the mechanics into SpacetimeDB-compatible primitives:
  - transform row adapter.
  - latest-row interpolation.
  - stale row TTL.
  - entity description cache keyed by stable id.
  - proximity subscription group.
  - dead/despawn animation policy.
- The MMO database owns avatar/world interaction state; Worker projections own
  proof/run/settlement authority.
- For `/tassadar`, SpacetimeDB rows should drive avatar positions, pylon
  station positions, local chat, attention, and selected-entity state.

### 7. Server Authority And Tick Model

Quick server mechanics:

- HTTP server plus Socket.IO.
- login queue state machine.
- `WorldManager` owns `WorldEntity` rows, spatial grid, terrain height, monster
  spawners, and 0.1 second client snapshots.
- `WorldNetworkClient` sends proximity-filtered nearby updates.
- `WorldAIClient` never times out, follows players, attacks, then idles.
- dead AI entities respawn through spawners.

OpenAgents harvest:

- Server authority must become SpacetimeDB reducers plus existing Worker/D1
  authority, not a new Node game server.
- Useful ideas to port:
  - 10 Hz position update target.
  - proximity-filtered snapshot/subscription boundary.
  - scheduled expiry of stale presence.
  - service-only reducers for bridge facts.
  - client reducers for movement/chat/selection only.
- Reject:
  - client-authored run truth.
  - client-authored proof/settlement status.
  - demo random spawns as live product state.

### 8. Combat, Damage, Stats, XP, Leveling

Quick gameplay mechanics:

- character definitions include health, max health, strength, wisdomness,
  level, experience, and attack stats.
- attack has timing, cooldown, range, and melee/magic type.
- attack event fires at an animation timing point.
- server filters targets by range and forward dot product.
- melee damage uses strength and equipped weapon damage.
- magic damage uses wisdomness.
- health reaches zero, entity enters death.
- XP requirement is roughly `2 ** (level - 1) * 100`.
- level-up increments stats and spawns effects.

OpenAgents harvest:

- Do not map training progress to health or damage.
- Harvest as generic game mechanics for future MMO surfaces:
  - timed action windows.
  - cooldowns.
  - range checks.
  - facing checks.
  - one-shot action events.
  - stat progression.
  - level-up VFX.
- Product mappings must be explicit:
  - reputation can be verified-work count.
  - capabilities can be equipment.
  - XP can be earned receipts or accepted work count only if backed by records.
  - "damage" should not appear on `/tassadar`; use "blocked", "stale",
    "verified", "accepted", "settled", or "attention" states.

### 9. Inventory And Equipment

Quick mechanics:

- 24 inventory slots and 8 equipment slots.
- item database seeded from weapon definitions.
- drag/drop DOM UI.
- equipped weapon broadcasts `inventory.equip`.
- weapon model loads from FBX and attaches to a named hand bone.
- material conversion makes weapons darker/metallic.
- inventory changes replicate to nearby clients.

OpenAgents harvest:

- Add `three-effect` bone attachment helpers:
  - find bone by name.
  - attach/detach object to bone.
  - transform offsets and scale policy.
  - dispose attached model.
- Add data model direction:
  - equipment = public capabilities, tools, badges, proof instruments, or role
    markers.
  - inventory = owned/available capabilities only when backed by real product
    data.
- UI should be OpenAgents-native, not Quick DOM drag/drop copied as-is.
- For `/tassadar`, capability/equipment indicators should be small overlays or
  avatar adornments, not a game inventory panel yet.

### 10. UI, HUD, Chat, Quest Journal

Quick UI mechanics:

- login panel fade.
- icon bar toggles inventory, stats, and quests.
- chat input sends `chat.msg` on Enter.
- local server messages appear in a chat area.
- quest component opens a hardcoded quest journal when a pickable NPC is
  selected.
- action messages are written to combat chat.

OpenAgents harvest:

- Keep `/tassadar` primary view sparse: nodes, stations, avatars, selected
  inspector, and minimal status text.
- Harvest:
  - local chat bubbles and nearby transcript.
  - selected entity inspector.
  - compact icon HUD.
  - entity-attached quest/task prompt.
  - system event feed.
- OpenAgents mappings:
  - quest = real task/promise/challenge with source ref.
  - chat = public-safe local chatter row.
  - system event = proof-backed world event.
  - stats = selected entity metadata, not global fake gauges.

### 11. Terrain, Quadtree, Workers, Biomes

Quick terrain mechanics:

- `HeightGenerator` uses seeded simplex noise.
- `CubeQuadTree` currently uses only one side, effectively a flat terrain plane.
- visible chunks are selected near the player.
- chunk builder uses four Web Workers.
- each worker builds positions, colors, normals, UVs, terrain coords, and two
  weight vectors.
- chunks include skirts to hide cracks.
- chunks are pooled, hidden, retired, and recycled.
- terrain shader uses texture arrays, triplanar sampling, noise-based UV
  offsets, and four-way texture blending by splat weights.
- texture splatter picks dirt, grass, gravel, rock, snow, cobble, and sandy rock
  by height, biome, and normal.
- biome and color noise influence base colors.

OpenAgents harvest:

- Add `three-effect` terrain primitives in phases:
  - deterministic height sampler.
  - quadtree LOD planner.
  - worker-backed chunk geometry builder.
  - chunk pool/recycle manager.
  - triplanar texture array material.
  - terrain splat weight generator.
  - biome sampling interface.
- For `/tassadar`, do not add mountainous fantasy terrain yet. Start with a
  clean 2.5D ground plane and station layout. Terrain matters once the world
  expands beyond one run map.
- For future world views, terrain can represent zones, regions, or topology,
  but it must not imply training metrics unless backed by data.

### 12. Scenery And World Dressing

Quick scenery mechanics:

- cloud GLBs spawn around the world.
- trees, rocks, plants, grass, flowers select by biome.
- deterministic noise jitters position, scale, and rotation.
- some props join the spatial grid for collision.
- 21x21 cells around the player are considered for spawning.
- spawned scenery uses stable keys from cell coordinates.

OpenAgents harvest:

- Add:
  - deterministic prop scatter.
  - biome/category catalog.
  - scenery collision registration.
  - stable-cell spawn keys.
  - prop LOD and pooling.
- For OpenAgents, scenery should be domain-specific:
  - pylon stations.
  - agent workbenches.
  - proof gates.
  - settlement terminals.
  - registry obelisks.
  - region markers.
- Avoid fantasy tree/rock clutter on `/tassadar` until the run semantics are
  clearer.

### 13. Particles, Billboards, Health Bars, Names

Quick VFX mechanics:

- general point-sprite particle system.
- alpha, size, and color splines.
- emitter rate accumulator.
- emitter life and drag.
- per-particle rotation.
- distance sorting for transparent particles.
- blood/fire hit effects tied to attack events and target bones.
- sorceror hand fire effects tied to attack action.
- level-up burst.
- health bar is a billboard shader plane above the entity.
- floating name is a canvas texture sprite attached to a loaded model.

OpenAgents harvest:

- Add `three-effect` primitives:
  - spline-driven particle emitter.
  - point-sprite particle system with texture, alpha, color, size, and drag.
  - bone-attached emitter.
  - one-shot burst.
  - billboard status bar.
  - floating name plate / speech bubble.
  - distance-sorted transparent particle render path.
- Map VFX to real OpenAgents events:
  - proof verified = verification burst.
  - receipt recorded = receipt glint.
  - settlement recorded = settlement burst only when backed by receipt.
  - local chat = speech bubble.
  - stale/blocked = static or slow warning state, not fake data movement.
- Keep the already-established rule: every pulse or animation that implies an
  event needs a source row/ref.

### 14. Spatial Hash Grid, Collision, And Picking

Quick mechanics:

- 2D spatial hash grid over X/Z.
- grid clients hold bounds and linked-list cells.
- `FindNear` deduplicates query results using incrementing query ids.
- player checks nearby colliders with a radius.
- server uses the same grid for proximity snapshots and combat range.
- picking raycasts Box3 bounds against pickable meshes.

OpenAgents harvest:

- Add shared spatial primitives:
  - typed 2D spatial hash grid.
  - entity bounds registration.
  - `findNear` for proximity UI.
  - collision query adapter.
  - raycast target registry that supports mesh, sphere, and box hit areas.
  - minimum-distance layout pass for station/node placement.
- Current `/tassadar` should use the minimum-distance pass so nodes do not stack
  visually. Future SpacetimeDB rows should preserve stable coordinates but allow
  display-layout relaxation when rows collide.

### 15. Data Definitions And Asset Catalog

Quick definitions:

- character catalog: paladin, sorceror, warrok, zombie.
- each has base model, path, hand anchors, name offsets, attack params, scale,
  initial inventory, and stats.
- weapon catalog has type, damage, render model, scale, and icon.

OpenAgents harvest:

- Add an OpenAgents world catalog:
  - avatar model type.
  - station model type.
  - role/status materials.
  - anchor names.
  - display label offsets.
  - allowed interaction states.
  - data authority boundary for each visible attribute.
- Store catalog data in owned source, not in the reference repo path.

## Asset And License Notes

The reference code is MIT licensed:

- repo license: MIT, 2021 simondevyoutube.
- client license: MIT, 2020 simondevyoutube.

Reference assets are mixed and require a separate asset ingestion decision:

- Trees, nature, nature2, and weapons include Quaternius CC0 license files.
- Character GLBs are from Mixamo according to `client/resources/characters/readme.txt`.
- UI/weapon icons reference `game-icons.net`.
- Terrain textures come from freepbr.com or OpenGameArt according to the
  terrain README.

Plan:

- Code mechanics can be studied and reimplemented.
- CC0 Quaternius assets may be candidates for prototype import after attribution
  and asset provenance are recorded.
- Mixamo/game-icons/terrain textures require a separate compatibility check
  before any production use.
- Prefer owned/generated OpenAgents-specific GLBs and textures for brand-critical
  surfaces.
- The owned catalog is now
  `docs/game/2026-06-17-openagents-world-asset-catalog.md`; it is the production
  gate until a machine-readable manifest replaces it.

## Target Architecture

### three-effect

Own shared, data-agnostic Three primitives:

- controllers: first-person, third-person, action maps, camera presets.
- animation: FSM, crossfade policy, one-shot action windows.
- models: load/cache/clone/render/bone attachment/equipment.
- world: spatial hash grid, hit target registry, entity interpolation.
- terrain: height sampling, chunk workers, LOD planner, triplanar materials.
- VFX: particles, billboards, name plates, speech bubbles, event bursts.
- scene: lighting/fog/sky presets, renderer lifecycle.

### openagents-world-spacetimedb

Own live interaction rows:

- regions.
- avatar identity and positions.
- pylon stations.
- local chat.
- bubbles.
- local emotes.
- attention/focus.
- user selection.
- public-safe event references bridged from authority.

It must not own:

- settlement truth.
- payout truth.
- proof validity.
- training-run truth.
- product-promise state.
- wallet secrets or private prompts.

### openagents.com

Own product composition:

- `/tassadar` live run world.
- selected entity inspector.
- public-safe HUD overlays.
- bridge from Worker projection plus SpacetimeDB rows into
  `three-effect` visualization options.
- authentication and safe capability gating where required.

### Worker/D1 authority

Continue owning:

- training run records.
- pylon/public projection records.
- verification challenges and verdicts.
- trace contribution records.
- settlement and receipt refs.
- product promises.
- public API projections.

## Harvest Roadmap

### P0: Contract And Tests For Existing Harvest

Status: started.

- Freeze the current controller, label, selection, and GLTF primitive contracts.
- Add regression tests around:
  - pointer-lock click decision.
  - unlocked click selects first, locks only on empty.
  - center-reticle selection while locked.
  - labels face camera in perspective mode.
  - animation handles dispose cleanly.
- Update `/tassadar` smoke coverage for:
  - canvas renders.
  - WASD movement.
  - mouselook.
  - selectable nodes before and during pointer lock.
  - no buttons/chrome regressing into the main view.

### P1: World Entity And Spatial Primitives

- Port the spatial hash grid as typed `three-effect` code.
- Add hit target registry.
- Add minimum-distance layout relaxation for overlapping nodes/stations.
- Add transform interpolation helper for remote rows.
- Add stale/despawn state policy.
- Bind `/tassadar` pylon stations and avatar rows through those helpers.

### P2: Avatar Model And Animation Stack

- Add animation FSM primitives.
- Add clip resolver and locomotion phase preservation.
- Add bone map and anchor helpers.
- Add visible avatar model handle.
- Add third-person avatar camera mode.
- Map SpacetimeDB avatar rows to visible local/remote avatar bodies.

### P3: Local Chat, Bubbles, Name Plates, Status Bars

- Add billboard text/speech bubble primitive if the current text-label handle
  is not enough.
- Add nearby chat bubble rendering from SpacetimeDB `chat_bubble` rows.
- Add selected entity name plate policy.
- Add status bar primitive for non-training semantic state, such as online,
  stale, verifying, or blocked.

### P4: Equipment, Capabilities, And Attachments

- Add bone-attached model helper.
- Add capability/equipment catalog.
- Render small role/capability adornments for agents and pylons.
- Keep inventory UI out of `/tassadar` until there is real capability data to
  inspect.

### P5: Particle And Event VFX

- Add spline-driven particle emitter.
- Add event burst primitive.
- Add bone-attached emitter primitive.
- Bind bursts only to real world events:
  - proof verified.
  - replay rejected.
  - receipt recorded.
  - settlement recorded.
  - local chat/emote.
- No looping data beams unless a row says the event is live and not expired.

### P6: Server/SpacetimeDB Authority Loop

- Use SpacetimeDB reducers instead of Quick's Socket.IO server.
- Implement 4-10 Hz avatar position update path.
- Add reducer-side bounds checks and jump rejection.
- Add expiry of stale avatars and chat bubbles.
- Add proximity subscriptions by region or selected run.
- Add bridge-only reducers for authority-backed facts.

### P7: Terrain And Scenery

- Status: first shared terrain substrate landed in `three-effect` issue #9.
- Add deterministic flat-world station terrain first.
- Then add terrain primitives from Quick:
  - height sampler.
  - quadtree LOD.
  - worker chunk builder.
  - chunk pool.
  - triplanar material.
  - biome/prop scatter.
- Do not ship fantasy terrain on `/tassadar` until it improves legibility.
  The run page should stay about real nodes and proof-backed events.

### P8: Game Mechanics Layer

- Add generic timed action/cooldown primitive.
- Add reputation/progression model only from receipt-backed facts.
- Add task/quest prompt primitive backed by OpenAgents tasks, promises, or
  challenges.
- Add party/guild/team surfaces later, using verified work and membership rows.
- Avoid combat metaphors on live training pages unless a separate game mode is
  clearly labeled as game/simulation.

### P9: Asset Pipeline

- Status: initial owned asset catalog landed in OpenAgents issue #5275.
- Create owned asset catalog.
- Decide whether to use any CC0 Quaternius prototype props.
- Avoid production reliance on Mixamo/game-icons/terrain assets until license
  review is complete.
- Prefer OpenAgents-specific generated or commissioned assets for public launch.

## `/tassadar` Specific Plan

Near-term `/tassadar` should remain focused:

- Only real nodes/stations/avatars on the main canvas.
- Minimal overlay text, no large bordered status box.
- No loss curve until a real public loss curve exists.
- No top lifecycle row as spatial nodes unless each item is a real entity.
  Lifecycle counts belong in a legend or compact HUD, not mixed into the world
  as fake places.
- Pylon station glyphs are the real "registered pylons seen" objects.
- Blue/animated transfer dots should stay absent unless they correspond to an
  actual row-backed event with source refs and expiry.
- User movement/camera motion is allowed because it is interaction, not data
  motion.
- Selection opens a compact inspector with proof refs, settlement refs, world
  events, or row provenance.

World entities that can appear now:

- canonical run center.
- pylon station per bridge-backed pylon ref.
- pylon agent/avatar per bridge-backed avatar row.
- proof/settlement reference objects only when their source refs exist.
- local viewer/avatar if the user is connected to the world database.

World entities that should not appear yet:

- fake lifecycle checkpoints as physical nodes.
- fake loss chart.
- fake packet beams.
- fake moving workers without an avatar row.
- fake combat, XP, or damage states.

## OpenAgents Mapping Table

| Quick mechanic | OpenAgents mechanic | Destination |
| --- | --- | --- |
| Login queue | Join world/region | SpacetimeDB reducer plus app auth |
| Local player | Viewer/operator avatar | SpacetimeDB avatar row plus `three-effect` controller |
| Remote player | Remote human/agent avatar | SpacetimeDB row interpolation |
| NPC monster | Agent/pylon actor | SpacetimeDB agent avatar, not combat by default |
| Quest NPC | Real task/challenge/promise station | Worker projection plus inspector |
| Chat | Local spatial chatter | SpacetimeDB `local_chat_message` |
| Combat action | Timed real event or explicit game action | `three-effect` action primitive |
| Health bar | Status/freshness/readiness bar | billboard primitive |
| XP/level | Receipt-backed reputation/progression | product data model |
| Inventory | Capabilities/tools/badges | OpenAgents catalog |
| Weapon attachment | Tool/capability visual attachment | bone attachment primitive |
| Terrain | World region topology | `three-effect` terrain primitives |
| Scenery | stations, gates, terminals | owned OpenAgents props |
| Particle hit effect | proof/receipt/settlement event burst | evidence-backed VFX |
| Spatial grid | proximity/collision/layout | `three-effect` spatial primitive |
| World update packet | row subscription/update | SpacetimeDB adapter |

## Verification Gates

Before any mechanic is considered harvested:

- It has a named owner package.
- It has a typed public contract.
- It has tests where the behavior is deterministic.
- It has a browser smoke if it affects `/tassadar`.
- It has a disposal path for Three resources.
- It does not introduce fake data motion.
- If it renders product state, each visible claim resolves to an authority row
  or source ref.
- If it imports assets, license/provenance is recorded.

For `/tassadar`, each release should smoke:

- page loads public prod route.
- canvas is nonblank.
- WASD moves camera/avatar.
- mouselook rotates camera while locked.
- unlocked click selects nodes before pointer lock.
- locked click raycasts center reticle.
- labels face camera.
- nodes keep minimum spacing.
- no unsupported charts or fake data beams appear.

## Issue Breakdown To Create Next

1. `three-effect`: typed spatial hash grid, hit target registry, and
   minimum-distance layout helper.
2. `three-effect`: row interpolation and stale/despawn helpers for MMO entity
   updates.
3. `three-effect`: animation FSM and locomotion transition helpers.
4. `three-effect`: bone attachment and capability/equipment render helpers.
5. `three-effect`: billboard speech bubble/status bar/name plate primitives.
6. `three-effect`: spline particle emitter and evidence-backed event burst
   primitive.
7. `openagents-world-spacetimedb`: avatar position update reducer, bounds
   checks, stale expiry, and proximity subscription shape.
8. `openagents.com`: bind `/tassadar` station/avatar rows to spatial
   primitives with selection inspector.
9. `openagents.com`: replace any remaining fake/lifecycle spatial nodes with
   a compact legend/HUD and real station entities only.
10. `three-effect`: terrain height/quadtree/worker chunk prototype.
11. `openagents.com`: owned world asset catalog and license/provenance record.

Status as of 2026-06-17: all eleven issues in this initial breakdown have been
implemented or documented. Follow-on issues should be smaller and asset-specific:
owned GLB production, machine-readable manifest, runtime manifest validation,
and only then `/tassadar` model consumption.

## Bottom Line

Quick's highest-value lessons are not the fantasy skin. They are the practical
mechanics stack: controller, camera, animation FSM, model loading, bone
attachments, proximity grid, interpolation, terrain LOD, particles, billboards,
local chat, and server-filtered world state.

OpenAgents should harvest those mechanics into shared primitives, then bind them
to real run, pylon, avatar, proof, and settlement rows. The public `/tassadar`
page should become a sparse, navigable, inhabited run space first. Richer MMO
systems can follow once the row-backed world is stable.
