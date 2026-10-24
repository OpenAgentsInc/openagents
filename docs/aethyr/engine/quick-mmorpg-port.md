# Quick_3D_MMORPG port

Status: implemented local multiplayer prototype. Browser checks cover login,
both player classes, equipment, chat, keyboard controls, and independent players.
Source geometry and animation comparisons pass. Final combat and moving-scene
checks are recorded below; matching geometry does not establish pixel parity.

The reference is SimonDev's [Quick_3D_MMORPG at revision
`547884332ca650abe96264f7230702d36481b9bc`](https://github.com/simondevyoutube/Quick_3D_MMORPG/tree/547884332ca650abe96264f7230702d36481b9bc).
The live entry is `client/index.html` → `client/src/main.js`, with
`server/index.mjs`. The separate `client/main.js` is an older, unreferenced demo.
The requested exceptions are Aethyrforge's grass and sky. Source terrain,
characters, scenery, controls, combat, and game UI are the comparison target.
All source distances use `worldScale = 1/6`; angles and animation times retain
their source values.

## Source inventory and implementation

Client filenames without a directory below are under upstream `client/src`;
`base.css` is under `client`, and `shared/` means `client/shared/`. Other source
paths are relative to the pinned checkout. Implementation paths are relative to
this repository. “Implemented” describes code, not completed browser acceptance.

| Live source system | Source files | Owned implementation and acceptance |
| --- | --- | --- |
| Startup and composition | `client/index.html`, `base.css`, `src/main.js`, `spawners.js`, `entity*.js` | `apps/editor/src/MmorpgUI.tsx`, `apps/preview/preview.worker.ts`: one world/frame loop; source art, login, loading, and model-ready transition. Compare login/HUD captures and repeated entry/exit. |
| Keyboard, actions, follow camera | `player-input.js`, `player-entity.js`, `player-state.js`, `third-person-camera.js` | `crates/mmorpg-core/src/motor.rs`, `packages/runtime/quick-mmorpg-controller.ts`: W/S, A/D turn, Shift, idle Space attack, idle Backspace dance; source acceleration/drag, action locks, camera offset/damping/FOV. Test UI focus, release, backward movement, transitions, and death. |
| Classes and authored animation | `shared/defs.mjs`, `player-entity.js`, `npc-entity.js` | `tools/assets/compile_quick_mmorpg_world.py`, `packages/renderer/mmorpg-characters.ts`: Paladin, Sorceror, ZombieGuy, Warrok; all 20 source clips, skeletons, maps, material factors, and scales. Compare idle/movement/action/death views for each class. |
| Health and stats | `health-component.js`, `shared/defs.mjs`, `server/src/world-entity.mjs` | `crates/mmorpg-core/src/data.rs`, `world.rs`: authoritative health and source Strength, Wisdomness, Benchpress, Curl, XP, and level fields; five-row stats panel and health frame. Verify class defaults and received damage. |
| Inventory and hand equipment | `inventory-controller.js`, `equip-weapon-component.js`, `network-player-controller.js` | Typed 24-slot bag plus eight equipment slots; versioned atomic swaps; source Sword, Axe, and Hammer_Small geometry/icons; animated hand attachment and unequip. Exercise occupied/empty swaps, retries, remote visibility, and armed/unarmed damage. |
| Combat | `attacker-controller.js`, `shared/defs.mjs`, `server/src/world-entity.mjs` | Server owns facing-cone/range checks, windup, cooldown, equipment damage, PVP, NPC damage, and one damage application per strike. Test misses, multiple in-cone targets, cover, death locks, and independent clients. |
| AI and population | `server/src/world-manager.mjs`, `world-client.mjs` | Source 81 × 81 candidate grid, 10% occupied, 90% Zombie/10% Warrok; five-second acquisition, chase/give-up ranges, separation, collision, 30-second corpse removal, and replacement actor IDs. Verify acquisition, pursuit, attacks, cleanup, and respawn. |
| Multiplayer and lifecycle | `network-controller.js`, `network-player-controller.js`, `network-entity-controller.js`, `server/src/world-server.mjs` | `crates/server`, `packages/runtime/mmorpg-session.ts`: typed WebSocket room, prediction/replay, 100 ms remote interpolation, explicit despawns, disconnect cleanup, and visible connection errors. Check two independent browsers and reconnect. |
| Chat, entry, and combat messages | `ui-controller.js`, `server/src/world-client.mjs`, `world-manager.mjs` | Named nearby chat, green entry/leave messages, red local combat messages; text rendering, 64-character limit, bounded history. Verify one message per retry and no gameplay keys while typing. |
| Names and source UI panels | `floating-name.js`, `ui-controller.js`, `base.css` | Current-frame GPU name billboards at class-specific heights, tested against world depth; source portrait, icons, stats with all five hover tooltips, inventory, empty journal, and mutually exclusive panels. Compare framing, fonts, layout, occlusion, and death removal. |
| Terrain and scenery | `terrain.js`, `terrain-builder-threaded-worker.js`, `texture-splatter.js`, `shared/terrain-height.mjs`, `scenery-controller.js` | Shared seeded height/noise in Rust, TypeScript, and WGSL; moving terrain patch; all 36 prop variants and source biome/jitter/selection rules; streamed 21 × 21 cells; source tree-root collision. Verify hills, materials, stream boundaries, and tree blocking while retaining our grass. |
| Hit and casting effects | `blood-effect.js`, `sorceror-effect.js`, `particle-system.js` | `packages/renderer/mmorpg-effects.ts`: blood at target Head/Hips, magic fire/smoke on damage, and Sorceror's two-hand casting emitters; source textures/curves and event deduplication. Observe actual hits, misses, moving bones, blend order, and cleanup. No invented projectile. |
| Lighting and world appearance | `threejs_component.js`, `render-component.js`, `texture-splatter.js` | Direct WebGPU retains source sunlight/hemisphere parameters, character/prop materials, and shadows through owned rendering. Image comparison remains pending; see explicit differences below. |

Supporting live files also include `finite-state-machine.js`,
`spatial-grid-controller.js`, `load-controller.js`, `terrain-chunk.js`,
`terrain-builder-threaded.js`, `terrain-shader.js`, `textures.js`, `quadtree.js`,
and `utils.js`; shared `math.mjs`, `noise.mjs`, `simplex-noise.mjs`,
`spatial-hash-grid.mjs`, `spline.mjs`, and `terrain-constants.mjs`; and server
`src/login-queue.mjs`. Their state, loading, spatial queries, interpolation,
noise, curves, terrain preparation, and connection admission belong to the
owned modules above, rather than a second JavaScript server or Three runtime.

The class data currently exercised by native checks is:

| Class | Initial/max HP | Damage | Hit delay | Range in meters |
| --- | --- | --- | --- | --- |
| Paladin | 200/200 | 300 armed; 10 unarmed | 0.35 s | 1.667 |
| Sorceror | 100/100 | 20 | 1 s | 6.667 |
| Zombie | 20/50 | 5 | 1 s | 1.667 |
| Warrok | 1,000/1,000 | 40 | 1.5 s | 2.5 |

## Authority and intentional adaptations

- `af-mmorpg-core` is shared by native authority and Wasm prediction. The Rust
  server advances at 60 Hz and publishes snapshots at 10 Hz. Clients send
  sequenced controls and typed commands, never accepted poses, health, or a
  replacement inventory. This replaces upstream's trusted client transforms,
  separate attack packets, and whole-inventory uploads.
- The protocol is `aethyrforge.mmorpg.v1`, at
  `/v1/worlds/open-ground/ws`. Welcome binds the source revision, release, seed,
  scale, spawn, and yaw. Prediction initializes from authority and reconciles by
  replaying unacknowledged controls. Commands use bounded request-ID caches.
- Source `Math.random` monster population becomes a recorded seeded population.
  Join uses the configured source-area center and a safe nearby free position;
  source joins scatter randomly. Default Paladin and optional Sorceror selection
  replace random player-class selection.
- Weapon slot 24 consistently controls appearance and damage. Upstream lets any
  changed equipment slot drive the hand while only its first slot controls
  damage. Backpack swaps are also authoritative here.
- NPC attack locks cover the full authored clip as well as the source cooldown;
  this avoids cutting off the longer Zombie attack. Terrain/static cover and
  capsule separation are owned improvements. Tree blocking retains the source
  horizontal root-distance rule, not leaf-canopy bounds.
- Facing cones use horizontal direction while reach remains three-dimensional.
  Upstream normalizes facing by feet-to-feet distance in three dimensions, which
  prevents visibly aligned opponents from hitting on steep slopes. Chest-height
  cover checks still prevent attacks through terrain and solid scenery.
- Personal respawn restores the dead player without resetting peers, inventory,
  or the world. Reconnect starts a new anonymous visit; accounts, durable player
  persistence, and shared-world deployment are not implemented. NPC health bars
  remain an earlier requested extension; upstream imports their component but
  never attaches it.
- The server caps 32 players, 1,024 monster spawners, 64 connections, 8 KiB
  incoming messages, and 240 messages/second/connection. Interest snapshots cap
  64 remote actors; rendering selects the nearest 63 plus the local player.
  Prediction replay caps 120 ticks. Particles cap 4,096 and emitters 128.
  Name labels share a cached 10.67 MiB full-mip atlas and one draw, capped at
  64 labels/128 triangles. Repeated NPC names share texture layers.
  Scenery uses 441 candidate cells; terrain uses a moving 3,328 m, 257 × 257 patch
  instead of upstream's quadtree. Its central 256 m uses 2 m vertex spacing;
  outer vertices use 24 m spacing. These are explicit resource limits.

## Dormant source code and remaining differences

- No live source path attaches `QuestComponent`/`PickableComponent`, emits
  `health.add-experience`, or calls the level-up spawner. The journal is empty;
  normal play earns no XP. The owned core retains the source XP arithmetic as an
  unwired hook. No quests, rewards, turn-ins, loot, or progression are fabricated.
  Upstream's unused level-up effect also calls obsolete particle APIs.
- Our grass and sky replace the source grass/space-sky look. The source's 20
  static cloud meshes are not imported. Source biome shape/scenery still apply.
- The source height/squared-distance fog equation applies to ground, props,
  characters, and grass. Source sunlight, hemisphere light, linear output, and
  character shadows replace the town's geometry lighting; the retained cloud
  sky has its own exposure and tone mapping. Exact shadow filtering and particle
  compositing still depend on the owned renderer rather than Three r124.
- Login retains the one-second model-ready fade, with reduced-motion support.
  Character selection, readable errors, a send button, keyboard-accessible
  inventory swaps, and respawn are deliberate UI additions.
- Upstream's active dat.GUI wireframe switch and five biome-rebuild controls are
  not ported. They remain an explicit developer-tool gap. The empty General
  folder is not a game feature.
- The older `client/main.js`, unused guards/alternate zombie resources, and
  unreachable loader/terrain prototypes are not the live parity target.

## Verification record

Private reports remain outside Git; their filenames identify the current local
evidence. Frame rates describe the measured local viewport and machine, not a
guarantee for every browser or device.

| Check | Observed result |
| --- | --- |
| Native kernel/core/server | 100 kernel tests passed, one preexisting ignored; 16 core and three server tests passed. Server tests use two real independent WebSocket clients and reject forged authority, check receipts/chat/equipment, and verify cleanup/despawn. Exact-terrain regressions cover melee in both directions on a steep hill. |
| Native versus Wasm | `mmorpg-wasm-proof.json`: Paladin and Sorceror, 480 ticks each across walk/run/idle/attack/dance; exact numeric match, 120-tick replay match, excessive replay rejected atomically. Kernel SHA-256 `ce7489dce0ed942a72f0a914395fee6da2205ac47f26418fd906c963f78affbb`, 854,027 bytes. |
| Source noise/scenery oracle | Nine terrain coordinates; 625 original-JavaScript cells match 240 placements and 11 trees, plus position/scale/yaw goldens and tree-root blocking. |
| Character/equipment import | `mmorpg-world-character-source-proof.json`: 686,286 vertices across four classes/20 clips compared with original Three GLTF/FBX loaders, AnimationMixer, and skeleton evaluation; maximum error 0.000001012 m. Thirteen importer/cooker tests and 48 scoped TypeScript tests passed; TypeScript check passed. |
| Static prop import | `mmorpg-scenery-source-proof.json`: all 36 original FBX variants, 33,488 triangles/100,464 corners; full model transforms, normals, and UV conversion compared through Three FBXLoader. Maximum position error 0.000000582 m, normal error 0.000000062, UV error 0.000000120. |
| Source animation playback | `mmorpg-animation-oracle.json`: 252 frames across seven ordinary transitions. `mmorpg-animation-interruption-oracle.json`: 360 frames/five rapid-transition recipes, including reused clips and all six simultaneous contributions, compared with unmodified Three r124. Action times/weights match exactly; maximum synthetic joint translation/rotation/scale error 0.000000000000000444. |
| Browser session and UI | `mmorpg-ui-browser.json`: 12 checks pass with two independent browser contexts. Paladin/Sorceror presence, chat, 32 inventory slots, Axe/Hammer selection, Sword drag, source stats, journal, actual keyboard movement/actions, focus release, and disconnect cleanup pass without JavaScript errors or failed requests. Walk/run measured 3.303/6.587 m/s. |
| Browser startup | `mmorpg-world-browser.json`: 60 FPS at 1,280 × 720, 17 nearby actors from 692 live NPCs/697 spawners. First control in 1.92 s on the measured warm local run. Blue cloud sky and the corrected source model facing were reviewed in subsequent UI captures. |
| Browser combat and moving scene | `mmorpg-combat-browser-v3/mmorpg-world-browser.json`: Paladin kills Zombie and Warrok, Warrok reduces player health from 200 to 160, Sorceror kills Zombie with hand fire and hit particles. The 15.57-second run-and-turn sample submitted 946 frames (60.76 FPS), crossed terrain windows, and completed with no JavaScript errors or failed requests. |
| GPU names and source tooltips | `mmorpg-ui-gpu-final/mmorpg-ui-browser.json`: all 12 browser checks pass again, including five exact source tooltips and current-frame GPU names. Two-player checkpoint submitted 11 labels/22 triangles in one draw with three shared texture layers; no duplicate DOM names or JavaScript errors. |
| Repository checks | 1,204 TypeScript tests: 1,202 passed, two skipped; production kernel/typecheck/Vite build passed. The optimized Wasm identity above is unchanged after the native slope-combat fix. |
| CPU sample | `mmorpg-native-proof.json`: debug build, one player/692 live NPCs/697 spawners, 1,800 ticks in 0.754 s; mean tick 0.417 ms, p95 0.550 ms, max 0.742 ms; one snapshot 7,869 bytes. Wasm JSON prediction mean 0.0334 ms/p95 0.0728 ms over 960 calls. These exclude browser rendering/network latency. |
| Original bootstrap | Scratch copy matches all 1,058 source-file hashes; HTML, modules, and Socket.IO handshake pass. No reference browser or side-by-side visual result is claimed yet. |

Run the checked-in browser tools through a disposable Chrome profile:

```sh
AF_CAPTURE_DIR="$CAPTURES/ui" openagents browser run -- node tools/verify-mmorpg-ui.mjs
AF_CAPTURE_DIR="$CAPTURES/combat" AF_MMO_FULL=1 \
  openagents browser run -- node tools/verify-mmorpg-world.mjs
```

Keep captures outside Git. `AF_EDITOR_URL` selects an isolated local world;
`AETHYRFORGE_EDITOR_PORT`, `AETHYRFORGE_PREVIEW_PORT`, and
`AETHYRFORGE_MMORPG_PORT` select its three ports. Do not benchmark simultaneous
GPU scenes. Compare original/owned fixed poses separately from moving-camera
frame rates; the retained grass/sky are intentional visual differences.

## Launch the owned world and reference

Use the [private asset guide](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/assets/README.md) prerequisites and a
prepared Open Ground directory containing our grass and sky. Choose a fresh
private output directory; source and derived art never enter Git.

```sh
python3 tools/assets/compile_quick_mmorpg_world.py \
  --reference "$HOME/work/Quick_3D_MMORPG" \
  --base "$AETHYRFORGE_GROUND_BASE" --out "$AETHYRFORGE_MMORPG_OUTPUT"
npx tsx tools/assets/compile-mmorpg-terrain.ts "$AETHYRFORGE_MMORPG_OUTPUT"
npm run build:kernel
AETHYRFORGE_PRIVATE_ASSETS="$AETHYRFORGE_MMORPG_OUTPUT" npm run dev:ground
```

Open `http://127.0.0.1:55316/`; preview uses 55317 and the local world server
55319. The launcher builds the Rust server and binds the two exact browser
origins. Enter a guest name, choose a class, and enter the world. Click the world
for keyboard focus; use W/S, A/D, Shift, Space, and Backspace as listed above.

For the original, copy the pinned checkout to `$REFERENCE_RUN/source`, excluding
`.git` and `node_modules`. In that scratch copy only, add a private
`server/package.json` declaring `socket.io: "3.1.0"` and
`gl-matrix: "3.3.0"`, taken from its retained lockfile. Run `npm ci
--ignore-scripts --no-audit --no-fund` there. In separate shells, with ports free:

```sh
node --random-seed=1 "$REFERENCE_RUN/source/server/index.mjs"
```

Serve the scratch client's directory on loopback port 55320, adding
`Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: credentialless` to responses. An ordinary
`python3 -m http.server` omits these headers and the source terrain workers fail
when creating SharedArrayBuffer. Keep the original files unchanged.

Open `http://127.0.0.1:55320/`. The original client connects to localhost:3000
and fetches Three r124/Socket.IO from its original CDNs. Its random seed is a
repeatable comparison aid, not the same generator as the owned server. Keep
downloaded art, reference captures, runtime logs, and model/UV proofs private.
Code retains the upstream MIT notices in
[`third_party/quick-mmorpg`](../../../apps/aethyr/third_party/quick-mmorpg); character, icon, nature,
tree, and weapon art retain their separate source notices.
