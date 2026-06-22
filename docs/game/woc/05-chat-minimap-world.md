# WoC Chat, Minimap, Nameplates, and Procedural World

Date: 2026-06-22
Scope: chat UI, minimap/compass/coords/subzone, nameplate projection, player context menu/card, procedural render.

## Chat

WoC's chat is split into pure models plus a DOM renderer in `hud.ts`:

- **Channels** (`src/ui/chat_channels.ts`, pure): a tab set
  (`say/yell/party/general/world/lfg/guild/officer`) with send prefixes (`/s /y /p /general
  /world /lfg /gu /o`) and auto-join on opening opt-in tabs. The model is DOM-free and
  unit-tested.
- **Timestamps** (`chat_timestamp.ts`, pure): optional `[HH:MM]` via `Intl.DateTimeFormat`,
  locale-aware.
- **Profanity** (`profanity.ts`, pure): cosmetic client-side masking with confusable
  normalization (leet folding) to match the server's soft-word list; toggleable.
- **Rendering** (`hud.ts`): `Enter` focuses `#chat-input`; each message is a `<div>` with
  `data-chan` for tab filtering, an optional timestamp, a clickable player-name span
  (right-click -> context menu), profanity masking, and a localized template split on
  `__WOC_CHAT_NAME__` / `__WOC_CHAT_MESSAGE__` placeholders so punctuation localizes. Caps
  at 200 messages, auto-scrolls when near the bottom.

### Relevance to us

**Adopt the pure models (`chat_channels`, `chat_timestamp`, `profanity`).** The Verse
already has local chat bubbles and a forum-reflection lane (#5904-#5907). A channel model
maps onto Verse contexts (local/proximity, party-of-Pylons, run-room, global) and the
localized-template + clickable-name pattern is the right base for Verse chat. The
client-side profanity mask pairs with the server-side two-tier filter from doc 04. Render
side is Foldkit for us, not raw DOM, but the models port directly.

## Minimap, compass, coords, subzone

Each is a tiny pure core plus canvas rendering in `hud.ts`:

- **Compass** (`compass.ts`, ~80 lines pure): 8-point rose, `bearingDegrees(facing)`,
  `headingLabel(bearing)`, and `compassView(facing, halfWindow)` returning marks with a
  `[-1,1]` offset fraction the HUD slides via CSS.
- **Minimap** (`hud.ts`): a circular canvas with a pre-rendered terrain background
  (sampled from the shared `terrainHeight`) and real-time blips iterating `sim.entities`:
  friends/guild dots, NPC quest markers (`!`/`?`), objects, mobs (alive/targeting/lootable),
  party members (class-colored, with off-map regroup arrows). Discrete zoom presets via
  `minimap_zoom.ts` (pure), persisted.
- **Coords** (`coords.ts`, ~6 lines pure): floor to yards, format without grouping.
- **Subzone** (`subzone.ts`, pure): `nearestSubzone(x, z, pois, current)` with enter radius
  32 yd and an 8 yd deadband (hysteresis to stop flicker).
- **World map** (`M` key): full-zone terrain canvas with viewport zoom/pan and overlays
  (trees/rocks, quest givers, dungeon portals, friends/guild, a player facing-triangle).

### Relevance to us

**Adopt the pure cores (`compass`, `minimap_zoom`, `coords`, `subzone`) directly**; they
are trivial, tested, and dependency-free. A Verse minimap is high-value: it would render
Pylon bases, the central Tassadar run core, assignment markers, and remote avatars as
blips, with the same hysteresis on subzone/region labels. The blip-iteration approach maps
straight onto our SpacetimeDB avatar rows + Worker run projection. Note the discipline:
**the minimap reads the same world the 3D scene reads**, so it cannot drift. Build the
canvas/Foldkit rendering ourselves; reuse the math.

## Nameplates (world-space labels)

Pure projection helpers the renderer calls per frame:

- `nameplate_projection.ts`: `isProjectedNameplateAnchorVisible(camera, worldPos, camSpace)`
  (in front of camera) and `nameplateScreenTransform(x, y)` -> a
  `translate3d(...) translate(-50%,-100%)` center-top anchor.
- `nameplate_threat.ts`: `isMobThreateningViewer(e, viewerId)` (alive hostile mob aggroed on
  the viewer) to tint a plate.
- `nameplate_combo.ts`: `comboPipsFor(player, e)` -> 0..5 pips.

The renderer pools a DOM nameplate per active entity, projects world -> screen each frame,
and localizes names through `entity_i18n.ts`.

### Relevance to us

**Adopt the pure projection math.** Verse entities (Pylon bases, agent avatars, run cores,
assignment markers) all want world-anchored labels with name + state + a small status bar
(for example a run-step progress, or a Pylon's online/assigned/verified/settled state).
`nameplate_projection.ts` is the exact world->screen anchor logic we need, and it is
renderer-agnostic enough to drive from a `three-effect` label primitive (we already track a
"port drei/troika text into three-effect" note). The threat/combo specifics are game
mechanics; the projection + pooled-label pattern is the reusable core.

## Player context menu and player card

- **Context menu** (`player_context_menu.ts`, pure): `chatPlayerContextActions(state)`
  returns whisper/invite/friend/ginvite/ignore/report/close based on online + relationship
  state. DOM-free and testable.
- **Player card** (`player_card.ts`, pure Canvas 2D): composes a 1200x630 shareable card
  (character pose, class color, level, stats grid, gear grid, percentile rank, optional
  holder badge, referral footer). Three selectable poses, async font load.

### Relevance to us

**Medium/low.** The context-menu action model is the right pattern for right-clicking a
Pylon or agent avatar in the Verse (inspect, message, tip, view-run, report). The player
card is a nice polish reference for a shareable Pylon/agent or run card (OG image), and it
aligns with our existing forum/share surfaces; build only if we want shareable Verse
cards. Both are pure and easy to adapt later.

## Procedural world (terrain, sky, water, foliage, weather, characters)

WoC's world is overwhelmingly generated from code (the asset-free philosophy that matches
our owned-asset policy):

- **Terrain** (`terrain.ts`): deterministic heightfield shared with the sim, chunked LOD
  (~60u chunks, dense near hubs, coarse in wilderness, skirts hiding cracks), PBR splat
  shading (per-vertex weights from slope/height/road distance) on the high tier, vertex-color
  Lambert on the low tier.
- **Sky** (`sky.ts`): equirect HDRIs per biome cross-faded by zone with PMREM IBL on high
  tier; canvas gradient dome on low tier. One canonical sun direction shared across sky,
  water glints, and shadows.
- **Water** (`water.ts`): dual-scroll normal-map ripples + broad swell, Fresnel sky tint,
  HDR sun glints, shoreline foam from precomputed shore depth.
- **Foliage** (`foliage.ts`): instanced GLB trees/rocks placed deterministically from the
  sim's `generateDecorations(seed)`, per-bucket variant hashing for variety without draw-call
  blowup, streamed grass chunks with a per-frame build budget, wind sway via
  `onBeforeCompile`.
- **Weather** (`weather.ts`): one pooled `THREE.Points` cloud in a camera-relative box,
  biome-driven type (snow/rain/clear) with cross-fades, procedural canvas flake/streak
  textures, deterministic mulberry32 RNG.
- **Characters** (`render/characters/`): GLB rigs cloned per entity with their own mixer, a
  `ClipMap` per skeleton, an animation state machine (`anim_state.ts`) deriving idle/walk/
  run/cast/swim/sit/jump from velocity/state, and LOD (baked-pose far, shadow proxy mid,
  full rig close). Offscreen WebGL captures class/skin headshots to PNG for the HUD.
- **Quality tiers + material dedup** (`gfx.ts`, `render_budget.ts`): programs reused by
  signature; shadow resolution scales with tier.

The renderer reads `IWorld` and never mutates it; characters are the only real 3D model
files, everything else is procedural.

### Relevance to us

**Adapt architecture and principles, not code 1:1.** We build Verse visuals in
`three-effect` (Three.js) per workspace policy, and the WoC patterns are an excellent
proven reference: deterministic-heightfield-shared-with-logic, chunked LOD with skirts,
splat shading from slope/height, HDRI biome cross-fade with one canonical sun, instanced
deterministic foliage with per-bucket variant hashing, streamed grass with a frame budget,
pooled camera-relative weather, and the velocity-driven character animation state machine.
The single most transferable idea is **one canonical sun/seed shared by every system so
sky, water, shadows, and minimap agree**, and **deterministic placement from a seed shared
with the logic layer**. We do not adopt the GLB rig dispatch (different assets) or the
WoW biomes; we port the patterns into `three-effect` primitives (terrain, sky, water,
label, particle, avatar) as our game-direction docs already plan.

## Net for the adaptation plan

High priority pure cores: **chat channels/timestamp/profanity**,
**compass/minimap-zoom/coords/subzone**, **nameplate projection/threat/combo**. Medium:
**player context-menu action model**, and the **procedural-world architecture** ported into
`three-effect` (terrain/sky/water/foliage/weather/animation patterns, especially the shared
canonical sun + shared seed). Low: **player card** (shareable Verse cards if wanted). Drop:
GLB rig dispatch and WoW biome/zone content.
