# Local battle crowd

Battle Lab is an opt-in Open Ground creator experiment. It adds two armies of
Paladins, Sorcerors, Zombies, and Warroks alongside the existing player, grass,
sky, scenery, and editable haunted props. These are locally simulated fighters,
not additional network clients. The normal Rust MMORPG server still owns the
connected player's movement, inventory, health, and shared NPCs.

## Implementation

- The [Town spell swarm](research/town-spell-swarm-2026-10-23.md) supplied the
  starting pattern: shared geometry, GPU simulation, bounded buffers, and
  batched rendering. The battle adds animated character detail levels,
  spatial combat queries, damage, and visibility compaction.
- Typed `battle` actions start, reset, stop, change the viewing camera, enter
  either army at a chosen map position, control the local fighter, and respawn.
  They use the existing editor service and preview bridge. Public/player builds
  refuse these actions. Battle state never changes the authored project.
- The direct WebGPU renderer owns fixed-capacity actor, spatial-grid, damage,
  visibility, and indirect-draw buffers. There is no JavaScript object, React
  component, CPU skeleton evaluation, or draw call per fighter.
- Physical body radii follow measured source torso, head, and foot bounds:
  0.63 / 0.63 / 0.64 / 1.33 metres for Paladin / Sorceror / Zombie / Warrok.
  Stable nearest-contact selection, reciprocal velocity limits, and damped
  overlap correction keep facing independent of collision pushes. Turning is
  bounded to four radians per second; live targets require a substantially
  closer replacement. Respawn waits for an unoccupied formation slot.
- A fixed 60 Hz compute simulation advances movement, target selection, attack
  windup, facing checks, delayed damage, health, death, and replenishment. At
  most three ticks run per frame; dropped simulation time is reported. Defeated
  fighters remain for eight seconds before their slot is replenished.
- A fixed ID-based subset of rear formation slots uses ranged attacks,
  independently of character class. Ranged spacing, projectile prediction,
  blocked-melee sidesteps, and siege target scoring use bounded GPU rules,
  described below. A bounded pool of 16,384 ballistic projectiles
  and eight animated trebuchets adds flight, impacts, and area damage. Counters
  report shots, hits, active projectiles, and pool overflows separately.
  Player firebolts follow the aim ray and sweep for the first opposing body or
  terrain hit; NPC shots and siege stones keep their ballistic trajectories.
  Siege and projectiles add two draws; first person adds one weapon/focus draw.
- A separate Rust/Wasm controller moves the local battle participant over the
  exact rendered terrain. The GPU owns this participant's combat health,
  damage, death, and attack acceptance. Entry and respawn wait for a GPU reset
  acknowledgement and grant two seconds of protection. Repeated attack inputs
  cannot restart a strike, and inhibited attack edges cannot fire later.
  A bounded asynchronous GPU query supplies up to 64 nearby living body circles
  to Rust. Swept movement stops at those bodies and slides along them; entry
  finds a clear nearby position. Old-life, stale, or overflowing contact samples
  stop translation until a fresh sample arrives, while looking and attacking
  continue. The query uses 8,320 GPU bytes and never waits inside the frame loop.
- First person follows the participant's eyes; third person follows the
  full-detail source character with mouse-controlled vertical aim. A short
  overhead dolly stays about 9–14 metres above the terrain. The map supports
  entry anywhere inside the battlefield, with team and character selection.
  W/S move, A/D turn, Shift runs, mouse movement aims, Space or a held left
  click attacks, 1/2 select sword/firebolt, and V switches first/third person.
  Third person labels the controlled fighter “You”. Death releases the mouse
  for the Respawn control. Armies above 500,000 start in dolly view; the wide
  cameras remain available for stress testing.
- The experiment admits up to one million fighters after checking the device's
  actual buffer, storage-binding, and dispatch limits. This is an allocation
  ceiling, not a measured performance claim. The spatial grid has 512² cells,
  each four metres wide with 64 combat-query entries. Ranks use a two-metre
  lattice, with staggered Warroks and extra room around their larger bodies.
  One million fighters occupy an arena with a 1,000-metre radius. Configured
  arenas cannot compress these formations. Grid and projectile overflows remain explicit
  fidelity limits; a high frame rate does not excuse dropped combat queries.
- Battle participation is local. It sends neutral controls to the connected
  MMORPG player; that server continues running, so the native player can
  coast or be attacked. Battle entry never teleports that player or copies
  battle health into the shared world. Leaving returns to its live state.
- Source class health, damage, ranges, and attack timing follow the
  [Quick MMORPG adaptation](quick-mmorpg-port.md). Crowd steering uses bounded
  spatial queries and avoidance. This experiment does not implement the shared
  server's inventory, chat, XP, cover checks, or multiplayer authority for
  these extra fighters.
- The offline cooker uses pinned Meshoptimizer 1.3.0 to select approximately
  600/150-triangle near/far meshes. It retains source vertex attributes and
  evaluates the owned character animator at 60 Hz into half-float position and
  normal textures. The vertex shader interpolates animation frames. Four
  classes and two mesh detail levels require eight indirect draws. A new optional
  distant silhouette tier adds four draws, described below.
- Source base colors use 256-pixel texture-array layers. The crowd has simpler
  lighting than the ordinary high-detail player: it omits normal/emissive maps
  and individual cast shadows. This is an explicit crowd rendering profile.
  All four source character materials are opaque. The cooker ignores image
  alpha before resizing, and the renderer uses opaque depth-writing draws
  without alpha testing or alpha-to-coverage. Distant silhouettes use a separate
  pipeline with coverage edges; source mesh materials stay opaque.
- Terrain sampling uses the visible terrain's actual vertices and triangles,
  including its coarser outer area. Starting a battle pins that terrain patch
  around the battle. Overview and orbit change only the rendering camera;
  they do not teleport the connected player. Their explicit spectator profile
  scales fog visibility with battlefield radius so close-camera height fog does not
  obscure the armies from above. Ordinary player views keep the source fog.
  Movement input is neutral while
  spectating.
- Small asynchronous GPU counter readbacks report actual living, defeated,
  moving, attacking, visible, and culled populations. Explicit probes sample
  up to 32 stable actor IDs, positions, health values, states, and a state hash.
  Allocated pool size is never substituted for the living or visible count.

The 60 Hz private four-class animation/LOD asset set is 46,159,644 bytes, compared
with 17,274,652 bytes for the earlier 20 Hz set. Geometry, materials, clip timing,
and per-frame texture reads stay unchanged; the additional samples improve
curved motion between baked poses. The loader also accepts the older 20 Hz set.
Performance evidence must identify the asset set it used. Original
models, textures, cooked assets, and video evidence remain outside Git. Prepare
the set from an existing private MMORPG world with:

```sh
npx tsx tools/assets/compile-battle-crowd.ts --base PRIVATE_ROOT --output NEW_PRIVATE_ROOT
```

Open `/?battle=10000` on the private Open Ground development app to join a
local guest session and start Battle Lab directly in dolly view. Counts from
100 through 1,000,000 are supported; malformed values use the small 10,000
default. Public/player builds and other profiles ignore this shortcut.
Stopping a battle does not automatically restart it.

## Million-fighter rendering profile

The optional distant tier keeps each fighter's actual position, heading,
health state, and animation clock. It draws a two-triangle silhouette only
when the character projects below eight pixels in height. Larger characters
keep the existing 600/150-triangle meshes, including nearby fighting crowds.
No simulation tick, actor, contact, or attack is removed for this rendering tier.

In Aethyr, the device profile's `crowd_tier_distance` knob
([ADR 0007](../../adr/0007-capability-and-detail.md),
[device profile](../device-profile.md)) drives both the tier distances and
this threshold. At `Detail` 1 the threshold is eight pixels, as in
Aethyrforge; it rises linearly to 16 pixels at the knob's floor of 0.4. The
16-pixel readability floor is half the 32-pixel atlas tile: a fighter taller
than that always keeps its mesh, and `BattleCrowd.setSilhouettePixels`
refuses a larger value. The crowd's near and draw distances follow the same
knob and never drop below its floor.

`tools/assets/compile-battle-impostors.mjs` bakes the existing private source
geometry, animation, and albedo into per-class color and normal atlases. Each
has eight yaw views, 32-pixel tiles, and approximately 12 animation samples per
second with the source clip durations and loop flags. The baker includes every
pose in the bounds, adds two-pixel tile margins, and checks every used frame
and view for coverage. Unused tiles and borders must be clear; color and normal
coverage must match. Four-sample resolves preserve silhouette edges.

This is a distant-image approximation, with discrete viewing directions and
animation frames. It does not reproduce full geometry at every camera elevation.
The renderer selects it by actual viewport height and projection, rather than
population size. Camera-facing quads use source-sized bounds, local normals,
and the existing sun, ambient light, faction tint, hit flash, and fog. A bounded
four-tap filter stays inside each tile. The four-sample path uses
alpha-to-coverage; the single-sample API path uses a half-coverage cutoff.

The optional atlases add 64 MiB to the private assets, for a total battle closure
of 113,268,508 bytes, below the existing 128 MiB admission limit. The old private
asset sets remain supported. Twelve indirect bins hold the eight mesh batches
and four silhouette batches. GPU probes expose each actual batch's instance
and index counts; far/class totals include silhouettes, and triangle counters
use their real two-triangle geometry. In the million-fighter wide profile,
all one million instances remain submitted while triangles fall from
146,850,000 to 2,000,000.

Simulation work also falls without changing its decisions. Each grid cell now
records a team bitmask instead of two unused centroid sums. Enemy-only searches
and damage queries skip friendly-only cells; physical contacts still include
both teams. Conservative squared-distance rejection avoids unnecessary square
roots, while surviving candidates retain the original distance comparisons
and ID tie breaks. A randomized equivalence test covers target ordering,
rounded-distance ties, both teams, and body-contact boundaries. The nine AI,
nine squad, and 17 kernel regression groups pass on the production GPU shader.

To bake a new private asset set without modifying its source folder:

```sh
AF_BATTLE_ASSET_ROOT=SOURCE_PRIVATE_ROOT \
AF_BATTLE_IMPOSTOR_OUTPUT=EMPTY_PRIVATE_OUTPUT \
openagents browser run -- node tools/assets/compile-battle-impostors.mjs
```

The output reuses source files through hard links, creates new content-addressed
atlases and a battle manifest, and replaces its own scene manifest. Licensed
source assets, atlases, and captures stay outside Git.

### Million-fighter measurements and stop point

The completed before/after profiles used an Apple M5 Max, Chrome 155 with the
Metal WebGPU adapter, 1280 × 720, DPR one, seed one, five seconds of warmup,
and one ten-second diagnostic window per camera. Only one GPU scene ran at a
time. GPU p95 covers the complete submitted render and compute frame.

| Camera | Previous completions/s | New completions/s | Previous GPU p95 | New GPU p95 |
| --- | --- | --- | --- | --- |
| Overview | 16.76 | 59.56 | 63.13 ms | 8.78 ms |
| Orbit | 17.16 | 59.32 | 57.57 ms | 9.18 ms |
| Dolly | 59.75 | 59.46 | 10.62 ms | 6.49 ms |
| First person | Not measured in this baseline | 59.95 | Not measured | 5.64 ms |
| Third person | Not measured in this baseline | 59.57 | Not measured | 6.10 ms |

Every new diagnostic window has zero measured additional dropped battle time
and zero grid/projectile overflow, with advancing GPU combat and fresh status.
Wide probes submit exactly one million instances and two million triangles.
Nearby fighters remain mesh draws. First person includes real movement,
accepted attacks, 1,280 outgoing damage, incoming hits, death, and respawn.

The initial third-person route accepted five attacks but dealt no damage:
its default downward pitch put firebolts into the ground about four metres
away. That run remains a failed participation check. The tool now sends a
public look adjustment after entry and every respawn, retaining the same damage
gate. Its separate ten-second rerun reached 59.89 completions/s, 4.95 metres of
sampled movement, five accepted attacks, 2,721 outgoing damage, 600 incoming
damage, three observed deaths/respawns, and zero sampled added local lost time.
No player pose or health was injected.

A longer production-path matrix started with GPU timestamp instrumentation
disabled, two 30-second windows per camera, and active first/third-person
participation. The user then requested immediate wrap-up and no more runs.
Only these windows finished before the test stopped:

| Camera | Repeat | Measured duration | Completed submissions/s | Lowest full one-second completion bin |
| --- | --- | --- | --- | --- |
| Overview | 1 | 30.182 s | 59.77 | 55 |
| Overview | 2 | 30.243 s | 60.01 | 59 |
| Orbit | 1 | 30.264 s | 59.97 | 59 |

All three pass the explicit experimental mean target: at least 58 accepted and
completed frames/s, no measured additional simulation loss, no overflow,
fresh advancing combat, and complete wide population visibility. This target
is separate from the existing stricter pacing gate; short stalls, queue-fence
notification timing, and physical display presentation are not erased by the
mean. The all-five-camera sustained matrix is incomplete. The second Orbit
window was interrupted; sustained Dolly and fighting first/third-person
windows were not performed in that matrix. No 1920 × 1080 qualification or new
video recording was performed. The previous 750,000-fighter videos do not show
this build and must not be presented as new million-fighter footage.

Private evidence directories are `battle-million-baseline-wide`,
`battle-million-impostor-profile-1`, `battle-million-third-aim-check`, and
`battle-million-all-views-final`. The last contains completed window files,
the interrupted raw window, and `stopped-at-user-request.json`. Its source SHA
and the corrected third-person rerun are
`2be1689bd478a9a498efa1e8a5caef9f5898fd85ba17b52ce3ee7c07789f1d77`.
The earlier all-view diagnostic source SHA is
`12bd89cfe2d2ee7b4ef69b5eacb4e74f47f5b05b245182ea92a4806debfd182b`;
later changes add the single-sample coverage fallback and corrected test aim.
Both completed reports verify unchanged sources and assets before/after.
The shared new battle-asset SHA is
`13c53ae005b31e87b527dab717e00581f5dcb0267167a712f4d87a1aedebdf55`,
with manifest SHA
`9de6b180693b4c661105f8885e710a902fe545db6dea7ea1724ccb779b754a97`.

Verification completed before the stop request:

- Full suite: 1,386 passed, two skipped; production build and TypeScript passed.
- Final narrow changes: 36 crowd tests, five window/camera tests, TypeScript,
  and shader validation passed. The single-sample fallback was not GPU-tested;
  the application uses four samples.
- Production GPU: nine AI cases, nine squad cases, and 17 kernel groups passed,
  including convoy motion, body contacts, player combat, and Meteor Shower.
- The final baker passed all 4,416 used frame/view coverage checks, matching
  color/normal alpha, clear unused tiles, and clear tile borders. A repeated
  bake produced the same content-addressed manifest. Production loading admits
  and hash-checks the complete 113,268,508-byte asset closure.

No further tests, recordings, or performance runs followed the stop request.
The saved commands support a future run when requested; a mean near 60 in the
completed diagnostics is not a completed sustained all-view qualification.

### RTX 4080 measurements (measured on CoderOS), 2026-10-25

These runs are measured on an NVIDIA GeForce RTX 4080 (driver 595.71.05,
16 GiB) through the CoderOS GPU browser: ungoogled-chromium 152 inside a
headless cage compositor, WebGPU on the NVIDIA Vulkan adapter (`nvidia` /
`lovelace`), with DevTools on `127.0.0.1:9340`. They used openagents commit
`6c6b5122f0` (the port merged in `f2e981d2e55`, clean tree) and
`verify-battle-crowd.mjs`. The source pin is
`86941a2af1721ce98a75e1365c7b6652bfa0597b7c1fabd3aeaff9463f45c2e8`; it differs
from the M5 Max pin because the port adds the `Detail` hooks, which run at
`Detail` 1 here. The silhouette asset set is the same as on the M5 Max
(`13c53ae005b31e87b527dab717e00581f5dcb0267167a712f4d87a1aedebdf55`). Settings
also match: 1280 × 720, DPR one, seed one, five seconds of warmup, and active
first/third-person participation. The cage output paces frames at about
62 Hz, so completions top out near 62/s instead of 60/s. Local model servers
held about 10.5 GiB of the card's memory during all runs.

**Diagnostic profile** (one ten-second instrumented window per camera; M5 Max
columns copied from the table above):

| Camera | RTX 4080 completions/s | M5 Max completions/s | RTX 4080 GPU p95 | M5 Max GPU p95 | RTX 4080 GPU p50 |
| --- | --- | --- | --- | --- | --- |
| Overview | 62.18 | 59.56 | 6.21 ms | 8.78 ms | 5.46 ms |
| Orbit | 62.18 | 59.32 | 6.41 ms | 9.18 ms | 5.64 ms |
| Dolly | 62.22 | 59.46 | 6.49 ms | 6.49 ms | 5.48 ms |
| First person | 62.32 | 59.95 | 6.33 ms | 5.64 ms | 5.55 ms |
| Third person | 62.43 | 59.57 | 6.34 ms | 6.10 ms | 5.49 ms |

Every window had zero measured dropped battle time, zero grid and projectile
overflow, and fresh, advancing GPU combat. Overview and orbit submitted exactly
one million instances and two million triangles, all of them silhouettes.
Strict pacing failed in every window: completion-interval p95 was 21–30 ms
against the 17.5 ms limit, with no stalls over 50 ms. The cause is uneven
cage/compositor cadence, not GPU time. The first-person window passed the
participation check (3.5 m of movement, 1,784 outgoing damage). The
third-person window moved 4.1 m and dealt 3,367 damage, but failed that check
because it sampled 0.017 s (one tick) of local dropped time.

**Sustained production-path matrix** (GPU timestamps off, two 30-second
qualification windows per camera, all ten windows completed):

| Camera | Repeat 1 completions/s | Repeat 2 completions/s | Lowest full one-second bin |
| --- | --- | --- | --- |
| Overview | 62.21 | 62.21 | 61 / 61 |
| Orbit | 62.17 | 62.28 | 61 / 61 |
| Dolly | 62.15 | 62.25 | 61 / 61 |
| First person | 62.33 | 62.38 | 61 / 62 |
| Third person | 62.35 | 62.35 | 62 / 62 |

All ten windows met the 58/s experimental mean target, with no measured
dropped simulation time and no overflow. Overview, orbit and dolly pass the
all-view mean check with two windows each. Repeat 2 of first person (10.4 m of
movement, 5,743 damage) and of third person (7.3 m, 8,603 damage) also pass
participation. Repeat 1 of each failed participation because it sampled
0.033 s of local dropped time, so the run reports `qualified: false`. Strict
pacing was not met for the same cadence reason as above. There are no GPU
timings, because timestamps were off.

**Old asset set without silhouette atlases** (same diagnostic settings, asset
pin `c38bd51f76492f345b44fdedfe03b3b74dd3e1fcda01a780c4ee088f8aea5d9b`): the
renderer falls back to the mesh tiers, with no silhouette draws and no errors.
Every window passed its functional checks, and first/third-person
participation passed.

| Camera | Completions/s | GPU p95 | Triangles |
| --- | --- | --- | --- |
| Overview | 45.98 | 23.31 ms | 146,850,000 |
| Orbit | 44.05 | 24.11 ms | 146,850,000 |
| Dolly | 61.51 | 6.52 ms | 405,179 |
| First person | 62.05 | 9.51 ms | 37,687,975 |
| Third person | 61.46 | 9.11 ms | 37,797,896 |

On the 4080, silhouettes cut overview GPU p95 from 23.31 ms to 6.21 ms and
triangles from 146.85 million to 2 million. First-person GPU p95 falls from
9.51 ms to 6.33 ms. On the M5 Max, the pre-silhouette overview p95 was 63.13 ms.

The first attempt, before these runs, lost the WebGPU device with
`VK_ERROR_OUT_OF_DEVICE_MEMORY` while allocating the battle textures. At that
point about 1.4 GiB of the card's memory was free. Chromium restarted its GPU
process, which freed memory, and every later run succeeded. On a 16 GiB card,
the million-fighter profile needs the other GPU processes to leave a few GiB
free.

**Image comparison with the Aethyrforge `f42c02c` build.** Both builds served
the same silhouette asset set. Each was captured on the same GPU browser with
seed one, one million fighters, 1280 × 720, and the overview, orbit and dolly
cameras three seconds after the start. The overview frames from Aethyrforge
and one openagents capture differ in 9 of 921,600 pixels, none by more than
8/255 (PSNR 94.6 dB). Two captures of the openagents build itself differ in
7.5% of pixels, because the live battle is advanced by frame-paced GPU ticks
and is not frozen at a fixed tick. Orbit and dolly move the camera over time:
the difference between the builds (PSNR 16–30 dB) is the same as between two
captures of one build. A bit-exact image-for-image gate needs a capture that
stops at a fixed tick; [fixed-tick capture](#fixed-tick-capture) adds it.

Private evidence directories on CoderOS are `battle-million-4080-diagnostic`,
`battle-million-4080-sustained`, `battle-million-4080-stable-fallback` and
`kiki-compare`. Captures and assets stay out of Git.

## NPC decisions

Battle fighters use rule-based GPU state machines, not model inference. Each
fixed tick updates idle, walking, attacking, or dead fighters from positions,
nearby bodies, targets, health, and timers. They do not send prompts, learn from
play, or plan a route through the world. Class health, damage, attack clips, and
windups remain those of the source adaptation. The ranged assignment still
depends on rear formation position and actor ID; it does not make every
Sorceror ranged or every Warrok melee.

The tactical update adds these behaviors:

- **Ranged spacing.** Each ranged fighter has a stable preferred distance of
  48–56 metres in arenas with a radius of at least 100 metres. It approaches
  beyond its preferred distance plus four metres and retreats below its
  preferred distance minus eight metres, or under close enemy pressure. It
  holds position between those thresholds. Smaller arenas scale the distances
  down: preferred distance is capped at the greater of 4.5 metres and 60% of
  the arena radius; retreat, approach, and pressure offsets shrink with it.
  Fighters face their target while backing away. They finish an attack already
  in progress, including its windup and facing checks, before moving again.
  They start another ranged attack only when not retreating and inside their
  approach threshold. At the arena boundary, if clamping a proposed half-metre
  retreat leaves less than 0.2 metres of travel, they hold and can fire under
  the normal cooldown and facing rules. The existing 72-metre maximum range
  remains unchanged.
- **Target retention and prediction.** Fighters keep a living target until a
  replacement is less than 75% as far away. Staggered local searches run every
  12 ticks outside attacks; invalid targets trigger an immediate search.
  Ranged fighters without a local target inspect at most nine nearby enemy
  frontline formation slots, skipping dead or friendly actors. Shots lead a
  crowd target's measured velocity, with a flight estimate of 0.3–1.8 seconds
  and at most six metres of prediction. The local participant has no velocity
  in this GPU input and is aimed at its current position. A projectile's
  destination stays fixed after launch, so a target can change direction and
  avoid it.
- **Blocked-melee sidesteps.** A melee fighter outside striking distance can
  move sideways around a close, stationary friendly blocker. Its side is
  stable for its ID and seed, and its lateral speed is 55% of its normal
  speed. The existing contact solver still limits movement. This does not
  replace ordinary forward movement, contact handling, or moving convoys, and
  does not add a new animation clip.
- **Siege target selection.** Each trebuchet examines 24 candidates distributed
  across the army instead of choosing one random frontline row. It ignores
  dead and friendly candidates, scores nearby living enemy clusters, gives
  attacking enemies more weight, and subtracts nearby allies. Each candidate
  query covers at most 25 grid cells with 64 entries per cell. The chosen shot
  leads motion by at most six metres. A successful launch keeps the six-second
  reload; missing targets or unavailable projectile slots retry after half a
  second. Blast damage still excludes allies; the scoring penalty does not
  introduce friendly fire.

These individual tactics add no per-fighter CPU work, model calls, actor fields,
or new GPU buffers. The squad layer described below adds shared orders and GPU
storage. Cover, full navigation, and obstacle-aware line of sight remain absent.
Existing spacing, turn-rate, animation-clock,
convoy, player-contact, damage, and overflow checks remain necessary. The
capacity measurements and films below predate this tactical update and do not
qualify its frame rate.

### Individual-tactics GPU checks

`battle-ai-kernel-final/battle-kernel-browser.json` passes all 17 regression groups:
129-fighter ranged and siege combat, 256-fighter living-body separation and
consecutive motion/animation checks, the 10,000-fighter rear convoy, real
Rust/Wasm participant contacts, damage, death and respawn, and Meteor Shower.

`tools/verify-battle-ai.mjs` adds nine compute-only fixtures with explicitly
constructed initial actors. Subsequent movement, shots, damage, and siege
choices come from the production shader. The final report,
`battle-ai-focused-edge-final/battle-ai-browser.json`, observes:

- 359 retreat ticks without a premature attack, 5.954 metres of approach,
  zero drift while holding range during cooldown, and firing when cornered.
- A shot that predicts a moving target's position and damages it from 1,000
  to 774 health, without damaging its ally. Another shot clamps the predicted
  target from x = 100.538 to the reachable arena boundary at x = 100.
- A blocked melee fighter moving 1.229 metres sideways, then 3.967 metres
  forward, while keeping at least 0.102 metres between living body bounds.
- All eight siege choices matching independently calculated candidate scores
  and predicted destinations, including the six-metre lead cap. With no living
  target, none fires and all retry after half a second.

The first focused fixture incorrectly expected a continuously pursued fighter
to stop retreating and fire within six seconds. Its failed report remains in
`battle-ai-focused-1`; cornered firing and normal attack timing have separate
passing cases. These fixtures do not measure rendering or frame rate.

`battle-ai-scale-final/battle-crowd-browser.json` records one instrumented
15-second diagnostic per view in headless Chrome at 1280 × 720. Both simulate
500,000 fighters, with at least 492,889 living and 465,456 moving fighters:

| View | Fighters submitted per frame | Completed frames per second | GPU envelope, 95th percentile |
| --- | --- | --- | --- |
| Close overhead dolly | 988–1,098 | 59.974 | 4.958 ms |
| Third person | 123,980–132,235 | 59.819 | 8.389 ms |

Both functional windows pass and drop no battle simulation time or global
kernel/host time. Cumulative grid and projectile overflow counters stay zero
across the direct probes bracketing each window and drain. Sustained pacing
qualification remains false: these are single 15-second windows, and the
third-person 95th-percentile completion interval is 18.4 ms, above the 17.5 ms
limit. This does not establish universal 60 FPS or physical display presentation.
The final tactical, kernel, and scale reports identify source fingerprint
`f5b08d041fce`; the asset-backed checks use asset fingerprint `c38bd51f7649`.
The scale run verifies that source and assets stay unchanged during measurement.

## Squad coordination and free camera

Formation tiles contain four rows by eight columns, split by team. Each has a
shared GPU commander and at most 32 assigned fighters. The commander runs after
the spatial grid is built, on tick 1 and every 12 ticks afterward. It observes
its bounded membership, existing member targets, and at most 25 nearby cells
with 64 entries each. Nearby living opponents include the local player; distant
squads gain no global knowledge of that player.

Orders combine a shared destination with distinct member offsets:

- **Advance:** keep moving toward the opposing army when no intervention is
  needed. Far rear ranks keep their existing forward motion.
- **Flank:** healthy selected squads take a stable side route around a nearby
  target. Members share the route instead of choosing independent random sides.
- **Withdraw:** weakened, pressured squads fall back together. A two-second
  hold and recovery conditions prevent rapid order switching. Fresh Zombie
  strength uses its actual 20-health spawn value.
- **Screen:** melee guards move between a threatened ranged anchor and its
  opponent, while ranged fighters keep their distance. Dead, friendly, and
  inactive participants do not trigger protection orders.

Fighters finish authored attack locks and can defend themselves at close range.
The existing body solver, turn limit, source damage, and delayed hits remain in
use. This is local coordination, not a navigation mesh or an army-wide planner.
The existing combat binding gains 65,536 records of 64 bytes: four MiB, with
62,500 possible team/tile slots at one million actors. Empty slots are not active
squads. Actor size stays 64 bytes, and the simulation still uses eight storage
bindings. No squad state crosses into React or requires per-actor CPU objects.

In third person, captured mouse movement or an unlocked right drag orbits the
camera independently of body heading. The wheel zooms from 2.5 to 18 metres.
W/S still move along the fighter's heading; A/D turn the fighter. Clicking to
attack turns the body toward the camera heading at at most four radians per
second before starting the strike. A brief click can wait up to 1.2 seconds;
blur, Escape, camera/weapon changes, death, and a new life clear pending intent.
First person retains direct mouse aim. The shoulder offset causes normal
third-person ray parallax; switching views preserves the orbit heading.

Overview and Orbit use a fog scale of `max(10, radius * 0.08)`. The old fixed
scale of ten saturated the source height fog at large camera distances,
turning the army and ground into a flat lavender surface. First/third-person
fog remains unchanged. The controls also follow externally started battle
counts, so the selected population agrees with the running battle.

### Coordination and camera evidence

`battle-squad-human-final/battle-squads-browser.json` passes nine production-GPU
cases. All 32 weakened members withdraw together by a mean 6.025 metres, while
the matching far-pressure control advances 6.092 metres. All 28 tested guards
follow the shared flank by a mean 2.819 metres. Against a nearby opposing player,
23 of 28 guards approach while all four ranged fighters back away. Inactive,
dead, and friendly player controls produce no screening order. The fixtures
check actual member movement, shared records, update cadence, body clearance,
and overflow counters. Existing nine individual-AI cases also pass.

`battle-squad-kernel-final/battle-kernel-browser.json` passes the natural crowd,
convoy, player-contact, combat, and Meteor Shower regression groups.
`battle-squad-cameras-final-3/battle-cameras-browser.json` passes 18 application
checks, including every camera preset at 300,000 and one million fighters,
zero player movement/turning during free look, bounded zoom, and heading
continuity. Native UI checks also exercise wheel zoom and the view button.
The camera harness preserves its failed preliminary runs: they exposed stale
counter sampling, unrelated body pushes, and an incorrect zero-parallax test
expectation. A browser-only WGSL expression error is retained separately in
`battle-squad-focused-compile-failure`. Offline validation alone did not catch it.

These correctness checks do not establish frame rate. Squad performance and
live footage use separate measured windows; private evidence remains outside Git.

### Squad performance and footage, 2026-10-24

On the Apple M5 Max, Chrome 155/Metal, 1280 × 720 at device scale one,
separate first/third-person diagnostic windows used five seconds of warmup
and 15 seconds of measurement. GPU timestamps cover the complete submitted
frame. Every fighter remains simulated; camera culling affects drawing only.

| Simulated pool | Camera | Completed submissions/s | Full-frame GPU p95 | Dropped battle time |
| --- | --- | --- | --- | --- |
| 500,000 | First | 59.98 | 9.70 ms | 0 s |
| 500,000 | Third | 59.96 | 8.39 ms | 0 s |
| 750,000 | First | 59.15 | 15.86 ms | 0 s |
| 750,000 | Third | 59.87 | 13.11 ms | 0 s |
| 1,000,000 | First | 59.70 | 14.92 ms | 0.0167 s |
| 1,000,000 | Third | 59.24 | 18.25 ms | 0.0167 s |

These are short headless measurements, not sustained 60 Hz qualification or
physical display measurements. One million exceeds the 16.67 ms GPU budget
in third person and loses a simulation tick in both windows. Wide cameras
that draw the whole pool remain more expensive. Reports are retained in
`battle-squad-scale-final` and `battle-squad-scale-750k`.

The new 750,000-fighter clips use real participation controls, ranged attacks,
and a slow mouse-look sweep. Each has its own reset, five-second warmup,
20-second unrecorded comparison, and 20-second live recording. They use the
production rendering path without GPU timestamp instrumentation. Native
MediaRecorder timestamps are preserved; no frames are synthesized or retimed.

| Clip | Unrecorded completions/s | Recorded completions/s | Decoded video frames/s | Native duration |
| --- | --- | --- | --- | --- |
| First person | 59.99 | 59.97 | 59.86 | 20.015 s |
| Third person, free orbit | 59.79 | 59.78 | 59.71 | 19.998 s |

Timed status samples show nine player attacks in each clip, dealing 4,197 /
6,786 damage and defeating 29 / 48 opponents respectively. Actual sampled
travel is 1.69 / 2.33 metres through the crowded ranks; the camera sweep does
not move the body. Sampled moving populations stay above 695,681 / 698,184,
with 189,000–196,689 / 184,969–198,939 submitted instances. Grid and projectile
overflow remain zero. No character pose or health is injected.

Sampled battle loss is unchanged during each shot; cumulative loss since reset
is 0.1008 / 0.0871 seconds, including setup. Participant cumulative loss is
0.033333 seconds and does not increase in the samples. Measured kernel and
host-clamp window deltas are zero. The reports do not freeze a battle-loss
counter at recording start, so sampled deltas do not prove a loss-free whole
window. Queue-fence p95 intervals exceed the stricter 17.5 ms gate; neither
clip claims strict pacing qualification.

The private artifacts are `battle-squad-first-750k` and
`battle-squad-third-750k` under the session scratch directory. Each includes
the MP4, raw window evidence, decoded frame cadence, source/asset fingerprints,
and a recording report. Both pin source SHA
`67469dce7a99ab8ead343f4fcbb4f0750b6395cf565a01454c300641408376b2`
and battle-asset SHA
`c38bd51f76492f345b44fdedfe03b3b74dd3e1fcda01a780c4ee088f8aea5d9b`;
before/after checks match. Media SHA-256 values are
`7576f0a18928389974244db86a4761530b59147ec19d0a066cf3b746be439ba0`
and `8e3779a1d691ffad8f367a03668e1f418e75936d931c9af3da5cf1a06c966801`.
The local gallery plays one clip at a time and starts paused.

## Meteor Shower

While participating, select **3 Meteor Shower**, aim at the ground circle, and
click or press **Space** to cast. The hotbar's **Cast** button uses the same
input path. **1** and **2** still select Sword and Firebolt. First and third
person both pick the actual displayed terrain through Town's camera and hand-ray
convention. Targets beyond 80 metres, the sky, and invalid surfaces do not cast.
A click and release between frames remains available for the next simulation
tick; invalid and old-life inputs cannot start a later shower.

The ability adapts the existing Town/Everglade spell: a 2.5-second windup,
six meteors staggered by approximately 0.13 seconds, and a one-second fall.
Destinations scatter within six metres of the selected point and remain fixed
when the player changes aim. Each impact applies the cast's shared 40d6 damage
roll to opposing living fighters inside four metres. The falling fire trails,
impact rings, health loss, and local kill credit use the existing bounded GPU
projectile and delayed-damage buffers. Friendly fighters are excluded. A separate additive draw renders soft fire
trails, impact flares, rings, and embers; opaque projectiles keep their existing
draw. The shared indirect buffer grows by 20 bytes. With this draw recorded,
battle counters report 11 draws, or 12 in first person, including zero-instance
draws when no meteor is visible.
The selected circle remains visible through grass; ordinary Town markers retain
their existing depth test.

Battle Lab adds a 4.2-second cooldown and reserves all six projectile slots before
accepting a cast. A full pool reports overflow and accepts no partial shower.
Death, leaving, and a new life cancel meteors still waiting to launch. Meteors
already airborne finish their fall. Counters separately report casts, impacts,
hits, cooldown ticks, and occupied meteor visual slots, including pending falls
and brief impact pulses. Combat remains local to Battle Lab; authored props and
the connected MMORPG player's state use their existing authorities.

The actual GPU checks in `battle-meteor-kernel-safe` and the final
`battle-meteor-kernel-final` observed all six falls and
impacts, a shared roll of 148, 2,072 damage, 14 opposing hits, and 13 credited
kills. Readbacks verified delayed damage against subsequent actor health,
no early or friendly damage, frozen destinations, cooldown and invalid-target
rejection, lifecycle cancellation, and completion of airborne meteors. The first
fixture in `battle-meteor-kernel` placed the caster on the enemy side; its death
during windup correctly canceled the shower. That failed evidence remains
unchanged. The intermediate soft-fire verifier also retained an old draw-count
expectation; the final verifier checks the additional effect draw. Targeting
tests cover terrain creases, the fine/coarse cell boundary,
range limits, first/third-person cameras, and pointer aiming. These checks do
not establish a new frame-rate qualification or a networked combat ability.

## Measurement and live video

`tools/verify-battle-crowd.mjs` runs one isolated Chrome scene at 1280 × 720.
It retains frame submissions, queue completions, interval distributions, full
render/compute timing where supported, combat probes, and population samples.
Use five seconds of warmup and two 30-second windows per view for qualification.
Failed windows remain part of the evidence. Do not run other GPU scenes or
builds during these measurements.

`tools/record-battle.mjs` records the running transferred canvas with
`captureStream` and MediaRecorder. It measures unrecorded and recorded
cohorts separately, with five seconds of warmup for each, and checks the encoded
video's timestamps, duration, and decoded frame count with `ffprobe`. It does
not stop the live loop, manufacture frames, or replace timestamps with a
nominal 60 FPS timeline.

`AF_BATTLE_CAPTURE_HZ=60|120` selects the native capture request rate. It does
not change the display-driven render loop. The
[canvas capture draft](https://www.w3.org/TR/mediacapture-fromelement/#html-canvas-element-media-capture-extensions)
requires new painted content before adding a frame. Requesting 120 times per
second is an experiment to reduce capture/render timing misses; the actual
encoded cadence is measured separately.

Recorded canvas frames do not include the React controls. Frame submission and
completion evidence also cannot establish physical display presentation.
Results apply to the tested browser, machine, viewport, crowd profile, camera,
and active population; they are not a multiplayer capacity claim.

## Fixed-tick capture

The live battle steps whenever frames arrive, so two captures of one build
never show the same moment. A Battle Lab **hold** freezes one exact moment:

- `window.aethyrforge.battle({ op: "hold", tick, cameraSeconds?, time? })`
  restarts the running battle from its seed and runs exactly `tick` fixed
  1/60 s steps, three per frame at most, then stops. Frame timing never changes
  the step count, and held frames add no dropped time. `tick` runs from 0 to
  36,000 (`BATTLE_HOLD_LIMITS.maxTick`, ten simulated minutes).
- The spectator camera is placed `cameraSeconds` into its path (default
  `tick / 60`), and the world clock that drives grass, sky and water is fixed
  at `time` (default `tick / 60`).
- MMO actors (your avatar, other players and their effects) are hidden while
  held, so only the battle and the world are in the frame.
- Status reports `battle.hold = { tick, steps, reached, cameraSeconds, time }`.
  `reached` turns true when `steps === tick`. The GPU tick counter in a
  `readBattle` probe then also equals `tick`.
- Only `overview`, `orbit` and `dolly` can hold. A participant's pose feeds the
  simulation every frame, so joining, first and third person are refused while
  held. Any other battle action, including `{ op: "release" }`, releases the
  hold; the live battle then continues from the held tick.
- Screenshots taken while held (`capturePixels`, `captureBlob`) use the held
  battle camera unless they pass their own.

The live path is unchanged: without a hold, `BattleCrowd.prepare` uses the
same accumulator as before.

**Why it is deterministic.** The simulation reads only the seed-built actor
buffers, the delayed-damage and combat buffers, and the simulation words of
the frame uniform (battle center, radius, counts, seed, terrain, respawn time
and participant). `BATTLE_VIEW_FRAME_WORDS` lists the view-only words (camera,
clock, LOD distances, pixel scale, lighting and frustum planes).
`tests/battle-crowd.test.ts` checks that the simulation WGSL reads none of
them. It also runs two holds to the same tick on the mock device, one with
steady frames and a fixed camera and one with jittery frame times (0 to 10 s)
and a moving camera. Both produce the same seeded buffers, the same ordered
dispatches and ping-pong bind groups, and the same simulation words at every
one of the steps, and stop at exactly that tick. Damage accumulates with
integer atomics, and targets break ties by id, so the GPU's thread order does
not change the outcome. One exception: the order in which visible fighters
are compacted into draw lists is not fixed. Opaque, depth-tested pixels do not
depend on it, but exact depth ties could, and the probe's `sampleSha256`
includes the projectile visible list. The tool therefore compares its own
camera-independent state digest instead.

**Capture tool.** `tools/capture-battle-tick.mjs` opens Battle Lab, starts the
battle, and for each view holds the tick and waits for `reached`. It then
checks that the probe's GPU tick equals the requested tick and captures the
held frame twice. The two captures must be byte-identical (`repeatable`). It
writes `<stem>.png`, `<stem>.rgba` (raw RGBA8) and one JSON report with the
state digest, RGBA and PNG SHA-256, source and asset pins, and browser mode.
Settings: `AF_BATTLE_COUNT` (default 1,000,000), `AF_BATTLE_SEED` (1),
`AF_BATTLE_TICK` (180), `AF_BATTLE_VIEWS` (`overview,orbit,dolly`),
`AF_BATTLE_CAMERA_SECONDS` and `AF_BATTLE_TIME` (both `tick / 60`),
`AF_BATTLE_WIDTH`/`AF_BATTLE_HEIGHT` (1280 × 720), and `AF_BATTLE_LABEL` (file
prefix, default the short revision). It needs the benchmark build
(`VITE_C_TOWN_BENCHMARK=1`) for the `readBattle` probe.

```sh
cd apps/aethyr
VITE_C_TOWN_BENCHMARK=1 AETHYRFORGE_PRIVATE_ASSETS="$BATTLE_ASSETS" npm run dev   # separate shell
OPENAGENTS_CHROME_PORT=9340 AF_EDITOR_URL=http://127.0.0.1:55316/ \
AF_BATTLE_ASSET_ROOT="$BATTLE_ASSETS" AF_CAPTURE_DIR="$EVIDENCE/battle-tick-a" \
AF_BATTLE_TICK=180 AF_BATTLE_LABEL=a node tools/capture-battle-tick.mjs
# ...the same again into battle-tick-b (another run, or another build)...
node tools/capture-battle-tick.mjs --compare \
  "$EVIDENCE/battle-tick-a/battle-tick-a-s1-t180.json" \
  "$EVIDENCE/battle-tick-b/battle-tick-b-s1-t180.json"
```

`--compare` exits non-zero unless both reports use the same count, seed, tick,
camera time and clock, and every view matches in both state digest and RGBA
bytes. For a view whose images differ, it reports the number of differing
pixels and the largest channel difference.

A build that lacks the hold, such as Aethyrforge `f42c02c`, cannot take part
directly. Apply this change to that checkout first, so both builds freeze the
same moment. The paths map per [provenance](../provenance.md).

## Spacing and motion correction, 2026-10-24

Visual review rejected the earlier clips despite their average frame rates.
The old formation placed centers about 0.82 metres apart and used one
0.72-metre spacing rule for every class. Collision pushes could turn an actor
180 degrees in one tick. Selecting the first contacts from atomic scatter
order and frequently changing targets amplified the instability.

The current formation uses measured class bounds and stable contact ordering.
In two consecutive-tick GPU checks of a naturally fighting 256-member crowd,
all living body bounds remained disjoint. The largest turn per tick was
0.066667 radians, matching the four-radian-per-second limit, and the largest
horizontal step was 0.056703 metres. The checks included 11,103 naturally tight
contact-pair samples in the later window, with movement, attacks, deaths, and
retained targets. A separate 10,000-member GPU check reads the entire army at
one-second checkpoints and requires continued far-rear convoy progress. It
rejected the earlier frozen rear ranks before the heading fix. Moving allies
now permit following motion, while head-on contacts keep their stopping limits.
All 137 eligible far-rear fighters advanced in the corrected five-second check;
the smallest net advance was 16.358 metres, including 3.400 metres in the final
second.
Large-target melee reach and firebolt capsules also follow the measured body
bounds. The participant Warrok hit check begins at a disjoint 2.1-metre
separation. Clock, teleport, overlap, and target-churn negative controls
ensure the metrics reject those defects. This bounded check is not a proof
that every silhouette or corpse stays separate in every large battle.

All seeded neighboring pairs were also checked at 129, 10,000, and 1,000,000
members against source-measured body disks. The current 60 Hz atlas preserves
geometry, materials, and source clip timing, while reducing interpolation
error in fast attacks. Private evidence is retained in `battle-kernel-motion-axis-fixed`,
`battle-kernel-motion-convoy` (the rejected rear-rank run), and
`battle-stable-asset-proof.json`. Earlier assets and rejected clips remain
unchanged outside Git.

A separate real GPU-to-Wasm participant check approaches a naturally fighting
Warrok, holds forward input against its body, slides tangentially, attacks,
takes damage, dies, and respawns. The smallest living-body clearance was
0.128 metres. The player stopped on 47 approach frames, moved 1.351 metres
while sliding, and dealt 300 damage. Missing and prior-life samples blocked
translation while looking continued. Fourteen focused Rust tests cover swept
movement, stale samples, moving-body uncertainty, recovery coverage, and death.
The successful browser report is `battle-kernel-player-contact-final`.
The earlier `battle-kernel-player-contact` failure retained host-only fields in
the verifier's Rust request; the corrected verifier uses the production field
boundary.

Contact age includes GPU queue and readback time from the query's host timestamp.
Rust uses the larger of wall age and simulation-tick age, rejects samples older
than 0.12 seconds, and expands sampled body circles by the maximum possible
motion during that age. Recovery candidates stay inside the sampled four-metre
neighborhood. These checks cover the tested local participant and bounded
nearby bodies; they do not qualify a networked physics simulation.

### Replacement live recordings

Three new 30-second live recordings use runtime revision `6973485` and the
60 Hz source animation set. All source and asset hashes matched before and
after each recording. First and third person enter the local battle through
public typed actions; the overhead camera follows the fight from above.

| Camera | Simulated pool | Recorded completions/s | Decoded video frames/s | Min. sampled moving | Min. sampled submitted |
| --- | ---: | ---: | ---: | ---: | ---: |
| First person | 500,000 | 59.985 | 59.905 | 468,498 | 122,165 |
| Third person | 500,000 | 59.653 | 58.807 | 468,405 | 122,625 |
| Overhead dolly | 1,000,000 | 60.015 | 59.969 | 960,581 | 993 |

Matched unrecorded windows completed 59.993 / 59.959 / 60.005 submissions per
second. These are headless Chrome 155 measurements on an Apple M5 Max with
128 GiB RAM, WebGPU/Metal, 1280 × 720, DPR 1, and five seconds of warmup.
The strict pacing and GPU-budget qualification flags remain false. Pool counts
include off-camera actors; the dolly submits only its nearby view. These mean
rates do not establish physical display cadence or performance on other devices.

The first-person participant moved 2.358 metres, fired seven times, dealt
6,256 damage, and defeated 52 opponents during the recorded window. The
third-person participant moved 2.451 metres and swung 12 times; allied bodies
blocked its route, and those swings dealt no damage. The independent GPU/Wasm
check above verifies successful melee, incoming damage, death, and respawn.
All three recorded windows had zero sampled grid/projectile overflow deltas.
Sampled battle dropped-time deltas were zero, while reset-to-stop gauges were
0.0599 / 0.1343 / 0 seconds; these different clock scopes must not be combined.

Private evidence is retained in `battle-player-contact-first-500k`,
`battle-player-contact-third-500k`, and `battle-player-contact-dolly-1m`.
The source-file SHA-256 is
`c3c6b8a2cbd4fd9482df5a26fcd0eb867a7db2db92d822e7ea63fc49189ad118`;
the battle-asset closure SHA-256 is
`c38bd51f76492f345b44fdedfe03b3b74dd3e1fcda01a780c4ee088f8aea5d9b`.
The gallery copies the original native-timestamp videos without retiming.
Actual playback and native consecutive frames 121–132 were inspected for each
view; PTS and unchanged-file hashes are in `battle-player-contact-visuals`.
Those short visual checks complement the numerical motion/contact checks; they
are not an exhaustive proof of every frame or silhouette.

Earlier `battle-stable-scale` timing windows are excluded from current capacity
claims because most rear fighters were stationary. Intermediate
`battle-corrected-*` recordings contain corrected crowd steering but predate
player-body collision and have been replaced in the local gallery. All rejected
recordings remain outside Git alongside their original reports.

## Previous crowded-build results, 2026-10-24

These results predate the spacing, steering, and 60 Hz animation fixes. The
clips were rejected during visual review for interpenetrating bodies and
rapid facing reversals. Their frame rates remain timing evidence for that old
build; they do not establish acceptable battle quality or current capacity.

The earlier 100,000-fighter tables and video are historical results from the
melee-only implementation. The following measurements include ranged attacks,
projectiles, siege weapons, and a separately controlled participant.

These runs used an Apple M5 Max, headless Chrome 155, WebGPU/Metal,
1280 × 720, DPR 1, seed 1, and five seconds of warmup. Rates below count
**successful completed GPU submissions per actual observation second**. They
are neither accepted-frame rates nor physical display measurements. Every
strict qualification flag remains false.

### Short instrumented capacity sweep

`battle-million-scale` contains one requested eight-second window per cell;
actual observations lasted 8.04–8.25 seconds. GPU profiling was enabled.

| Simulated pool | Wide orbit completed/s | First-person completed/s | Dolly completed/s |
| --- | ---: | ---: | ---: |
| 500,000 | 58.14 | 59.38 | 59.83 |
| 750,000 | 30.52 | 52.97 | 60.09 |
| 1,000,000 | 16.27 | 27.23 | 59.40 |

Wide views submitted the entire pool, including corpses. First-person
minimum submissions were 128,357 / 191,599 / 256,897; dolly minima were
5,129 / 5,088 / 5,003. Submission counts include occluded geometry, not
pixel-visible fighters. A million simulated fighters with a narrow dolly
therefore does not demonstrate a million rendered fighters near 60/s.

The million-wide view submitted 146.85 million triangles per frame. Median
full GPU time was 59.44 ms, including 37.36 ms in the main HDR pass; the
million-dolly medians were 8.13 / 1.11 ms. This shows geometry
rendering cost alongside simulation cost. Million-wide dropped
**1.533333 simulation seconds between benchmark start and stop**; its
cumulative loss since reset reached 2.754 seconds, including warmup. Grid
**total** overflow and projectile overflow stayed zero in all nine probes.

The distant silhouette profile below replaces this earlier next step.
Improve neighbor-data locality separately, measuring combat-query cost and
accuracy rather than inferring it from camera culling.

### Live 30-second clips

Each clip has matched unrecorded/recorded windows with profiling disabled.
Video is the live canvas with native timestamps, without trimming or retiming;
React controls are excluded. Minima below belong to the recorded window.

| Clip | Pool | Engine unrecorded/s | Engine recorded/s | Decoded video/s | Min. living | Min. submitted |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| First | 500,000 | 59.49 | 59.82 | 59.74 | 492,284 | 109,215 |
| Third | 500,000 | 59.66 | 59.80 | 59.70 | 492,381 | 113,176 |
| Dolly | 1,000,000 | 59.99 | 60.02 | 59.97 | 984,490 | 4,402 |
| Wide | 450,000 | 49.03 | 49.88 | 49.86 | 442,472 | 450,000 |

The slower wide result is retained. Earlier `battle-final-wide-500k` completed
59.66/s unrecorded but 53.55/s recorded; its video decoded at 53.57/s.

Recorded timed samples show first-person **7 attacks / 26,712 damage dealt /
214 kills / 43.47 m travel / 2 respawns**, and third-person **13 / 8,100 / 23 /
41.16 m / 2**. Travel excludes respawn displacement. These timed counters are
separate from later bracketing GPU probes; dolly/wide have no participant.

Sampled battle-loss deltas and global benchmark kernel/clamp deltas were zero.
Recorded cumulative battle loss **since reset** was first 0.073 s, third
0.0717 s, dolly/wide 0; participant maxima were 0.033333 s, with zero sampled
delta. Recording reports lack a frozen battle-start counter, so these are not
whole-window battle-loss proofs. All overflow gauges/deltas were zero.

Artifacts: `battle-final-first-500k`, `battle-final-third-labelled-500k`,
`battle-final-dolly-1m`, `battle-final-wide-450k`. First/dolly and the sweep pin
`bd755ce` / source SHA `6d8362a115f961fde1b8aad8ce1aaee540a9cde431001c6af3028d9fa95a8aa1`;
third/wide pin `2088e2e` /
`4850810f6478ef11045908145a4d6a49bb349309e0573a65e2953897ab929947`.
Asset SHA
`757e7b5b3c1e8db86d20bc7b6080836e344e7b0433add7261e0ba82c06948bf3`;
source and asset checks matched before/after each recording. Reports retain
full revisions, media fingerprints, freshness checks, and failed gates.

## Historical melee-only capacity, 2026-10-24

These measurements precede ranged combat, siege, the local participant, and
the larger spatial grid. See the expanded results above for the current build.

**100,000 fighters** was the largest tested pool that averaged at least 58
completed GPU submissions per second in every sustained orbit and player-view
check. This is an around-60 mean-rate result. Occasional stalls and dropped
simulation time remain; it does not pass the stricter frame-pacing gate.

| View | Repeat | Completed submissions/s | Min. sampled living | Min. sampled submitted | Dropped battle time |
| --- | --- | --- | --- | --- | --- |
| Orbit | 1 | 59.42 | 96,547 | 100,000 | 0 s |
| Orbit | 2 | 59.38 | 96,540 | 100,000 | 0 s |
| Player | 1 | 58.71 | 96,564 | 60,073 | 0 s |
| Player | 2 | 58.21 | 96,604 | 61,256 | 0.191 s |

Each window used five seconds of warmup, 30 seconds of measurement, seed 1,
1280 × 720, the same grass and sky, and the same crowd profile. At least 95,798
fighters were moving in the samples. All four windows had advancing combat,
fresh status counters, and zero sampled grid overflow or invalid actors.
Living fighters and held corpses are reported separately. The crowd allocation
was 51,903,968 GPU bytes, approximately 49.5 MiB, including its animation and
material textures.

The machine was an Apple M5 Max with 128 GiB RAM. Chrome 155 used WebGPU's
Apple Metal adapter. These are headless Chrome measurements; the installed
launcher blocked a headed run under its screen-lease rule. Detailed profiling
was disabled for these windows, so they provide no GPU-duration measurement or
physical display cadence evidence. The runtime revision was `f6383d9`.
The corrected player-view verifier was `afe66a1`; its only source difference
was moving a report write until after the GPU probe.

Evidence is retained outside Git in `battle-final-100k` and
`battle-final-100k-player` under the session scratch directory. The first
combined report retains a rejected player window: saving the large report
before probing allowed the simulation to advance after the measured window,
causing a false freshness rejection. The corrected two player windows passed
all freshness checks. The rejected window also contained real dropped time;
it is not used to claim a stall-free result.

Wide orbit views submitted the entire pool at the 145–148-triangle far detail
level. Player views include both detail levels and frustum culling. Submitted
instances include occluded bodies and held corpses; these counts do not measure
visible pixels. The ordinary player retains its full source model. These
extra fighters are local GPU simulations, not 100,000 network connections or
authoritative multiplayer characters.

### Higher counts and remaining limits

| Pool | Sustained wide result | Player-view result |
| --- | --- | --- |
| 125,000 | 58.77–59.64/s, four 30-second windows | 56.22 / 57.09/s, two 30-second windows |
| 150,000 | Earlier wide-only results retained separately | 57.61/s, 15-second check |
| 175,000 | Not measured in the final sweep | 52.37/s, 15-second check |
| 200,000 | 57.97/s, 15-second check | 52.92/s, 15-second check |
| 300,000 | 43.67/s, 15-second check | 39.08/s, 15-second check |

The 125,000-fighter short player checks initially reached 59.7/s. Longer checks
exposed growing close-range load as combat concentrated more bodies near the
camera. Visible instances, near-detail instances, and neighbor queries rose
over time. Wide views remained near 59/s with the same simulation running.
These uninstrumented counters do not isolate compute time from rendering time.
The higher-count failures and the earlier unsuccessful recording remain in
the private evidence.

Source review also found an incorrect cutout path for opaque character
materials. The cooker and renderer now preserve the source opaque semantics,
and opaque actors draw before grass and blended effects. Short player checks
improved substantially, but this is not a controlled opacity-only speedup claim.

The strict 60 Hz gate still reports failure. Accepted frame intervals include
occasional stalls, including a 116.7 ms maximum in the second 100,000-fighter
player window. No result here establishes stall-free gameplay, performance on
other hardware, or production multiplayer capacity.

A long-run check also found that the UI rejected the shared character's
animation clock after 120 seconds, freezing the displayed FPS and population.
Quick MMORPG status now accepts its bounded session clock. Measurement and
recording tools reject frozen frame, sample, and tick counters and compare
status against the frozen simulation-step count at the measured stop boundary.
Later direct GPU probes separately establish combat progress. They cannot date
the final HUD sample: the world keeps running while the tool drains replies
and saves its report. The 120-tick freshness limit remains unchanged.

A final UI check against the same server after an hour found another clock
boundary: Rust emits a capped actor time of 3600.000244 seconds. The client now
accepts that exact float value and caps presentation extrapolation to it.
Regression checks cover joining the old world, continued valid UI updates,
and preserving the actual reason when a join fails.

To reproduce a complete sustained check against a prepared benchmark server:

```sh
AF_EDITOR_URL=http://127.0.0.1:55326/ \
AF_BATTLE_ASSET_ROOT="$BATTLE_ASSETS" \
AF_CAPTURE_DIR="$BATTLE_EVIDENCE" \
AF_BATTLE_COUNTS=100000 AF_BATTLE_INSTRUMENTED=0 \
AF_BATTLE_VIEWS=overview,orbit,player AF_BATTLE_REPEATS=2 \
AF_BATTLE_MODE=qualification AF_BATTLE_WINDOW_MS=30000 \
AF_BATTLE_WARMUP_MS=5000 \
openagents browser run -- node tools/verify-battle-crowd.mjs
```

The server needs `VITE_C_TOWN_BENCHMARK=1`. Keep private output outside the
checkout. The stricter gate's failure is retained in the report.

### Historical melee-only recording

The final 100,000-fighter recording runs for 30.033 seconds, beginning with an
orbit and switching to the unchanged player's view after about 18 seconds.
Its recorded cohort completed **58.10 GPU submissions/s**; the matched
unrecorded cohort completed **59.31/s**. Both had at least 96,569 sampled living
fighters and passed freshness and combat-progress checks. The recorded cohort
dropped 0.0813 seconds of battle time. This remains an around-60 mean-rate
result with occasional stalls, not strict frame-pacing qualification.

The original H.264 file contains **1,734 decoded frames at 57.74 frames/s**,
1280 × 720, with native timestamps. The capture request rate was 120 Hz; the
render loop remained display-driven. No frames were generated offline, and the
video was not trimmed or retimed. The canvas recording excludes the React HUD.
Passthrough decoding produced 1,734 distinct picture hashes, with no consecutive
bit-identical pictures. This does not establish render-to-video frame-ID alignment.

The source revision was `5503171`. Source and private battle-asset hashes
matched before and after recording, and no browser/GPU errors were reported.
The video and complete reports remain outside Git in
`battle-live-final-100k-120` under the session scratch directory. Earlier failed
recordings remain alongside it. The actual media SHA-256 is
`3c5310360ae76231fcb0a95a30ecdf93afc29e58b30692d79a3e6ecfca48b664`.
