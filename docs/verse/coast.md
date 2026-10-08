# The coast

Status: C1 implementation in progress, revised 2026-10-08. W1 through W11
are closed. The coast remains split into C1 through C6; C1 reuses the
shipped water renderer, spectrum, clipmap, shared clock, and spell code.

The coast is a loaded zone where Verse's ocean is the main attraction: a
temperate bay with a sand beach and surf, sea cliffs, a harbor with
rowboats, tide pools, a lighthouse, a reef and a wreck to dive, and islands
offshore. Every water capability and rule it uses is specified in
[Water](water.md); this document says what the zone adds and how it uses
each water phase.

## Contents

- [What exists today](#what-exists-today)
- [Identity and entry](#identity-and-entry)
- [Layout](#layout)
- [Terrain and bathymetry](#terrain-and-bathymetry)
- [Water](#water)
- [How the coast uses each water phase](#how-the-coast-uses-each-water-phase)
- [Life and light](#life-and-light)
- [Streaming and budgets per tier](#streaming-and-budgets-per-tier)
- [Multiplayer](#multiplayer)
- [Assets](#assets)
- [Tests and captures](#tests-and-captures)
- [Phases and estimates](#phases-and-estimates)
- [Open questions](#open-questions)

## What exists today

Checked against main `17617d4ab2` and the C1 branch:

- **Water Lab.** `verse-zone-water` retains its public paths while
  `verse-water-spells` owns the shared hotbar, Water Orb, Thunderbolt,
  spell rules, and water simulation. Its cove remains available.
- **Shared water renderer.** `verse_pbr::water` provides the shared shader,
  scene-copy refraction, planar reflection, High-tier SSR, ripples, wakes,
  underwater optics, and measured adaptive budgets. WebGL2 keeps the Low
  path without GPU timestamps or compute requirements.
- **Spectrum and ocean.** `physics::water::WaveSet::with_spectrum` supplies
  gameplay waves. `verse_pbr::water::Water::ocean` and `WaterSurface::ocean`
  drive W10's camera-centered clipmap. `verse_zone_water::coast` is the
  retained W10 fixture; C1 extends its bay rather than replacing the water
  system.
- **Fields.** `verse_pbr::water::field::Field` packs depth, shore distance,
  and two current channels into RGBA16F pages, 64 texels or 128 m per side
  at 2 m resolution. Ten pages cover 1,280 m, including a 40 m border around
  the playable square. C1 adds shelter as a separate R8 plane; renderer
  admission and upload must count it before C1 closes.
- **Clock and physics.** `physics::water::tick_at` derives 120 Hz ticks from
  Unix milliseconds. `physics::water::Water` is the query boundary for
  buoyancy and currents. The tide folds the integer tick before converting
  to floating point. The gameplay sampler uses the same shelter function
  that generates the render mask.
- **Qualification.** W11 records measured tier budgets and calibrated
  overrun behavior in [Water](water.md#budgets-per-tier). Those limits are
  admission budgets, not a guarantee that the coast keeps every effect.
  C6 still measures the complete coast and qualifies owner-only devices.
- **Remaining C1 integration.** Register the zone, entry and return paths,
  tide-aware rendering and shelter, and per-tier bay captures. C2 through
  C6 still add the pack, boats, diving content, transitions, and multiplayer
  behavior described below.

The coast doesn't replace the Water Lab. The Lab stays a small, fast-loading
demo; the coast reuses its modules (see
[Reuse from the Water Lab](#reuse-from-the-water-lab)).

## Identity and entry

- **Zone.** `ZoneId::Coast`, label `Coast`, serialized `coast`, world ID
  `verse-coast` (the `verse-` convention that Everglade, the Grove, the
  crypt, and the Water Lab use), arch sign `COAST`, and `half_extent`
  600 m, following
  [Build and register a new zone](zones.md#build-and-register-a-new-zone).
  `verse --coast` opens it directly. "The coast" is a working name.
- **From Coder's plaza.** The west arch is now the Water Lab's, so the coast
  gets a new tap-and-button arch, `COAST_ARCH`, at `(0, 0, 24)`: straight
  ahead of the spawn, between the Lagrange 1 arch at `(12, 0, 12)` and the
  Water Lab's at `(-12, 0, 12)`, and opposite the Physics Lab's at
  `(0, 0, -22)`. C1 confirms the spot with
  `plaza_portals_stand_clear_of_structures` and moves it if a structure
  stands there. The HUD control is **Enter Coast**, and the map gets a
  **Coast portal** landmark.
- **From the Grid.** The OpenAgents app's bare world gets a third walk-in
  gate ([`zones/gate.rs`](../../crates/verse/src/zones/gate.rs)),
  `GRID_COAST_AT` with `GRID_COAST_OPEN`, beside the Everglade gate at
  `[9, 11]` and the hidden Lagrange 1 gate at `[-9, 11]`. A proposed spot is
  `[18, 20]` (side, forward, m); C5 places it with the same clearance tests
  the Everglade gate has (6 m from the ball, 5 m from every block, 6 m from
  the reset pillar, 12 m from other gates, and 14 m from the Gym's walls). It
  needs zone storage, as the Everglade gate does, and loads the coast pack
  the same way: the player stays on the Grid with **Loading Coast**, its
  percentage, and **Cancel**, then **Retry** or **Dismiss** on failure.
- **From Everglade.** A trail gate where Glade Run leaves Walden Woods, at
  the stream's last point (`layout::STREAM`, `[-9, -128]`), loads the
  coast at the estuary, where the same stream reaches the sea. Today
  `Intent::Return` always restores the saved plaza or Grid pose. C5 adds a
  zone-to-zone transition that keeps the pose saved when the player first
  left the plaza or the Grid, swaps packs (Everglade's character stays
  loaded, because the coast walks Everglade's character), and re-keys
  presence from `verse-everglade` to `verse-coast`. Until C5, the arches
  are the only way in.
- **Arrival.** Both arches load the zone at the arrival terrace above the
  beach, facing the bay. The estuary gate arrives at the estuary.
- **Return.** A return portal on the arrival terrace (`coast::RETURN_PORTAL`)
  goes back to the plaza or, entered from the Grid, through the coast's own
  walk-in arch to 3.5 m in front of the Grid's coast gate. After C5, the
  estuary gate goes back to Everglade's trail gate. The zone panel's
  **Plaza** or **Leave** control works anywhere, as in Everglade.
- **Character and hotbar.** Entry loads Everglade's pack for the character,
  as the Water Lab and the crypt do. The hotbar is the Water Lab's: Water
  Walk, Control Water, Create or Destroy Water, Sleet Storm, Water
  Breathing, the Water Orb, and the Thunderbolt, plus Levitate and Feather
  Fall from Everglade's bar. On phones it sits above the sticks, as
  Everglade's does.

## Layout

The zone frame puts mean sea level at y = 0, with +x east and +z south, as
on the plaza, where the Physics Lab's north arch stands at z = −22. The
playable area is the square |x|, |z| ≤ 600 m, with land to the northeast and
open sea to the south and west. Beyond 600 m the ocean draws to the horizon
on the clipmap, but isn't playable.

```text
             north (-z)
   +-------------------------------------------+
   | headland  lighthouse      dunes, pines    |
   |  cliffs ##   harbor ==             gate>  |  <- estuary gate (C5)
   |  cave  ## breakwater=\    terrace  /      |
   |  pools  ~~             \ beach  estuary   |
   |  reef ~~      wreck     \     \  /        |
   |  ~~~~~~~~~~~~~~~~~~~~~~~~\  beach         |
   | ~~~~~~~~ bay ~~~~~~~~~~~~~\               |
   | ~~~ Gull Island ~~~~~~~~~~~\ sandbar      |
   | ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ \ Tern Islet|
   +-------------------------------------------+
   west (-x)            south (+z)        east (+x)
```

| Place | Center (x, z), m | What it is |
| --- | --- | --- |
| Arrival terrace | (120, −120), 14 m up | A paved overlook on the dunes above the beach, with the return portal and a view of the whole bay. |
| Driftwood Beach | From (−80, −180) to (240, 180) | A crescent of sand about 480 m long and 40 to 80 m wide at mean tide, facing southwest. Its bed slopes about 1 in 30 below the water, so the swell shoals and breaks (V7) into surf with shore foam (S9). Driftwood, dune fencing, and beach grass. |
| The estuary | Mouth at (100, −10) | Where the stream from Everglade meets the beach: a river body whose flow runs into the surf, a reed marsh (B7) inland of the mouth, and Everglade's trail gate 250 m up the stream at about (300, −280), after C5. |
| The harbor | Basin at (−130, −230) | At the beach's north end, behind a stone breakwater about 150 m long that runs east-southeast from the headland: piers on pilings, a boathouse, moorings and buoys, six boardable rowboats, and two larger moored boats as props. A baked shelter mask cuts the swell inside the breakwater to a tenth. |
| The headland and the cliffs | From (−450, −400) to (−200, −100) | Sea cliffs 35 to 45 m high along the west, with a sea cave, a rock arch, sea stacks, and footpaths to the top. |
| The lighthouse | (−330, −260), on the headland at 40 m | A 24 m tower with a gallery to walk around, a lamp room, a keeper's cottage, and a fog bell. |
| Tide pools | (−230, −120) | A rock shelf below the headland that the tide covers and uncovers, with pools 0.2 to 0.8 m deep. |
| The reef | (−380, −50) | Rocky reef with kelp off the cliffs, 6 to 15 m deep. |
| The wreck | (−100, −120) | A sunken sloop 12 m down at the harbor's approach. |
| The open bay | (0, 150) | Deeper than 7.5 m and wider than 15 m everywhere past 150 m from the shore, so Control Water's whirlpool works there. |
| Gull Island | (−100, 230) | About 250 m offshore and 120 m across: a rocky shore, a cove beach on its east side, and a grassy top 18 m up with nesting gulls. Reached by rowboat or by a long swim. |
| The sandbar and Tern Islet | From (240, 180) to (330, 330) | A sandbar 170 m long from the beach's south end to a small islet 40 m across. It is dry at low tide and under water at high tide, so a player who lingers swims back. |
| Far isles | Beyond 600 m to the southwest | Low island silhouettes on the horizon, drawn as distant proxies and never playable. |

## Terrain and bathymetry

The land and the sea bed are one generated heightfield, `coast::terrain::ground(x, z)`,
a pure Rust function like the Water Lab's
[`terrain::ground`](../../crates/verse-zone-water/src/terrain.rs) and
Everglade's height function. Everything reads it: the character's feet,
buoyant bodies, the water's depth, the drawn terrain, and the baked textures
below. Nothing is downloaded for the ground.

**Land.** Dunes rise from the beach's back edge (about 2.5 m) to 10 to 16 m
at the arrival terrace, then roll inland to 25 m at the northeast corner.
The headland is a plateau at 35 to 45 m that drops to the sea as cliffs.
The heightfield can't overhang, so it carries the cliffs' steep slope and
the coast rocks kit covers it with cliff modules; the sea cave, the arch,
and the sea stacks are meshes with their own colliders. The estuary carves a
channel 6 m wide and 1 m deep from the gate to the mouth.

**Bathymetry**, in meters below mean sea level:

| Area | Depth | Why |
| --- | --- | --- |
| Beach face, shoreline to 150 m out | 0 to 5, about 1 in 30 | Shoaling and surf (V7) need a gentle, even slope. |
| Open bay | 5 to 20 by 400 m out | Whirlpool room, swimming, and boats on real swell. |
| Harbor basin and channel | 4, channel 5 | Moorings that never ground at low tide (1.2 m below mean). |
| Reef shelf | 6 to 15 | One-breath dives for most characters reach the top, not the base. |
| Wreck site | 12 | Deep enough to need Water Breathing or a strong breath. |
| Channel to Gull Island | 18 | A real swim, with no wading route. |
| Playable edge | 40 | The FFT ocean reads as deep water past the bay. |
| Sandbar crest | 0.6 | Dry when the tide is more than 0.6 m below mean. |
| Tide-pool shelf rim | 0.3 | The pools separate from the sea when the tide is more than 0.3 m below mean. |

**Baked fields.** On install, the zone generates these from `ground` at 2 m per texel.
W10 groups the field into ten 64 × 64 pages per axis (640 × 640 texels),
covering the playable square and its 40 m border. Pages stream near the
camera under the water field budget:

- **Depth** below mean sea level (one channel of RGBA16F), for absorption (S6),
  shoaling (V7), caustics (S10), and the shore foam's distance.
- **Shore distance** (one channel of RGBA16F), signed, for shore foam (S9) and surf placement.
- **Shelter mask** (R8), 0.1 inside the breakwater and 1 in the open, that
  scales the swell's amplitude in both the gameplay surface and the shader.
- **Flow** (two channels of RGBA16F), the estuary's current and the offshore bounds current
  (P3, P4).

The bake is deterministic, so every client builds the same textures from the
zone seed without a download.

## Water

- **Bodies.** One ocean body (B4) with sea water's density of 1,025
  kg/m³: Gerstner gameplay swell (V1), spectral detail from W4 (V2), the
  geometry clipmap from W10 (M2), and shoaling surf (V7). The estuary is a
  river body (B2) that flows into the ocean; the marsh is a marsh body
  (B7); each tide pool is a pool body (B5) joined to the ocean while the
  tide covers it.
- **Clock.** A presence-only zone world has no host to hand out a tick, so
  the coast's world tick is the Unix time divided by the 120 Hz physics
  step and floored. Every client derives the same tick from its clock; a
  second of clock skew moves the tide by at most 5 mm and the swell by one
  second of phase, which the shared-body corrections in
  [Multiplayer](#multiplayer) absorb.
- **Tide (ours).** Sea level is the mean level plus 1.2 m × sin(2π t / T),
  with a game-scale period T of 24 minutes: a real tide's 12 hours and 25
  minutes is too slow to see in a session. It is a pure function of the
  world tick and the zone seed, like the swell, so it needs no replicated
  state. At high tide the beach is about 36 m narrower. The sandbar and
  the tide pools follow the thresholds in
  [Terrain and bathymetry](#terrain-and-bathymetry).
- **Presets** (M4). A temperate coastal look from a coastal Jerlov water
  type for the bay, a murkier harbor, and a silty, brackish estuary. Storm
  weather raises the sea state through the wave controls (V3).
- **Swimming and diving.** The rules in
  [Swimming and wading](water.md#swimming-and-wading) and
  [Breath and suffocation](water.md#breath-and-suffocation). The reef and
  the wreck sit deeper than one breath carries most characters there and
  back, so Water Breathing matters. The water is not frigid.
- **Bounds (ours).** Past 560 m from the center on either axis, an offshore
  current (P3) pushes swimmers and boats back inside, rising from 0 to
  2 m/s at the 600 m edge, so there is no invisible wall at sea.
- **Spells.** Every rule in [Spells and water](water.md#spells-and-water)
  applies; [W8](#w8-spells) lists what the coast adds.
- **Boats.** The rowboat from [W6](#w6-interaction-and-rowboats).
- **Weather.** The coast's climate in [W9](#w9-weather).

### Reuse from the Water Lab

C1 starts from the Water Lab rather than a blank crate:

- `terrain.rs`'s height-function shape, its `Reach` river, and its drawn
  terrain, extended to the coast's 1.2 km and its baked fields.
- `sea.rs`'s patches and light, replaced by the clipmap ocean once W10 lands.
- `floats.rs`'s floating bodies, for the beach's barrels, crates, and
  driftwood.
- `spells.rs`, `orb.rs`, `bolt.rs`, and `hotbar.rs`, shared by moving them
  into a crate both zones depend on (for example, `verse-water-spells`)
  rather than copying them, with the Water Lab re-exporting under its old
  paths.
- `targets.rs`'s dummies stay in the Water Lab; the coast has none.

## How the coast uses each water phase

Each subsection says what the coast takes from a phase, what it adds, and
what the coast does on each tier before and after the phase lands. A phase
that hasn't landed leaves the coast correct but plainer, as
[Quality tiers](water.md#world-scale-and-multiplayer) (M5) requires.

### W4 spectral waves

From [#10776](https://github.com/OpenAgentsInc/openagents/issues/10776):

- **Sea state.** A JONSWAP spectrum with directional spreading from the
  southwest, fetch-limited inside the bay. Clear weather is a sea state of
  2 to 3 (significant wave height 0.5 to 1.2 m); Storm is 5 (2.5 to 4 m).
  The gameplay surface keeps at most eight Gerstner terms plus W4's lowest
  CPU cascade, so a rowboat bobs on the wave the player sees.
- **Cascades.** The baked looping tile on Low, two 64² cascades on Medium,
  and three 128² cascades on High, by
  [Budgets per tier](#streaming-and-budgets-per-tier).
- **Surf.** Shallow-water dispersion over the depth field makes the swell
  slow, steepen, and break along Driftwood Beach and the sandbar, with
  whitecaps from the Jacobian offshore and persistent foam on Medium and
  High. The shelter mask keeps the harbor nearly flat.
- **Before W4.** No coast: C1 is blocked by W4.

### W5 reflections and refraction

From [#10777](https://github.com/OpenAgentsInc/openagents/issues/10777):

- **Planar reflection (S3)** for the harbor basin and the tide pools, the
  coast's flat water: the boats, piers, and lighthouse mirror there. One
  plane at a time, so the plane is the harbor's when the camera is within
  200 m of it and a tide pool's when the camera is over the shelf.
- **Screen-space reflection (S4)** on High for the open sea: the cliffs,
  the island, and the lighthouse beam on the swell.
- **Refraction (S5)** in the shallows over the sand, the tide pools, and the
  reef top, on Medium and High.
- **Contact foam** around pilings, breakwater blocks, sea stacks, and
  floating bodies, and **soft particles** for sea mist and surf spray.
- **Low tier.** Sky reflection (S2) only, with absorption over the baked
  depth.

### W6 interaction and rowboats

From [#10778](https://github.com/OpenAgentsInc/openagents/issues/10778):

- **Rowboats.** Six of the [Rowboats](water.md#rowboats) specification's
  boats, moored at the harbor's piers, reusing Everglade's
  `town/rowboat.glb` (from `town_props.py`) with the oars the boats kit
  adds. On the coast they meet real swell: in surf over about 1.5 m,
  broadside to a breaking wave, they capsize by the 60° roll rule. A
  rowboat can reach Gull Island and Tern Islet.
- **Wakes and trails.** Kelvin wakes (P8) behind rowboats and swimmers, and
  foam trails (P9) that drift with the estuary's flow and the bounds
  current.
- **Splashes and spray.** Surf crest spray against the sea stacks, the
  breakwater, and the cliffs (P10), and entry splashes for divers off the
  piers and the rock arch.
- **Debris.** The piers, the boathouse, and the rowboats break under the
  existing destruction; their planks float (P6) and drift on the swell.

### W7 underwater

From [#10779](https://github.com/OpenAgentsInc/openagents/issues/10779):

- **Fog and color.** Underwater fog continuous with each preset's
  absorption: blue-green in the bay, green-brown in the harbor, brown in
  the estuary.
- **Waterline and window.** The split waterline when the camera straddles
  the swell, Snell's window from below, and distortion on Medium and High.
- **Light.** Sun shafts over the reef and the wreck on Medium and High, and
  caustics (S10) on the sand, the reef, the wreck, the pilings, and
  swimmers on every tier (a baked flipbook on Low).
- **Sound.** The underwater low-pass, plus surf heard muffled from below.
- **Before W7.** C4's diving is blocked by W7; the Water Lab's analytic bed
  caustics are the only underwater light.

### W8 spells

From [#10780](https://github.com/OpenAgentsInc/openagents/issues/10780),
with the Water Lab's Water Orb and Thunderbolt:

- **Control Water.** The open bay meets the whirlpool's minimum (15 m by
  7.5 m) past the 7.5 m contour. A flood cast on the open sea makes the
  SRD's 20-foot wave, which carries rowboats and may capsize them (the 25
  percent roll). Part Water opens a trench to the wreck. Redirect Flow
  turns the estuary's current or the surf's backwash.
- **Freezing.** The ocean counts as still water for the freezing table, but
  ice that forms on swell breaks into floes that ride the gameplay surface;
  the harbor freezes into a sheet. Ice thaws in stages as the table says.
- **Lightning.** The whole ocean is one body, so the
  [6 m conduction rule](water.md#lightning-in-water) keeps a strike at sea
  local; the tide pools are separate bodies while the tide is out. In Storm,
  Call Lightning deals its extra 1d10.
- **Water Orb.** It grows at the open-water rate everywhere near the shore,
  and an orb thrown into the surf pours into the ocean
  ([Water Orb](water.md#water-orb)).
- **Water Walk** follows the gameplay swell, so walkers ride the surf.

### W9 weather

From [#10781](https://github.com/OpenAgentsInc/openagents/issues/10781):

- **Climate.** `Climate::coast()`: Clear 0.35, Overcast 0.25, Fog 0.15,
  Rain 0.15, and Storm 0.10, with wind of 3 to 18 m/s from the southwest
  quadrant. Everglade's climate is mostly clear.
- **Sea state.** The schedule's wind drives W4's wind speed and direction,
  so the swell builds over a block's 60 s blend into Storm.
- **Rain and wetness.** Rain ripples on every body, puddles on the terrace,
  the piers, and the lighthouse gallery, and wet surfaces and characters.
- **Rising water** (R4) raises the estuary and the marsh only; the ocean has
  the tide instead.
- **Fog.** Fog weather pulls the zone's fog in to 150 m and turns on the
  lighthouse lamp and the fog bell.

### W10 clipmap ocean and streaming

From [#10782](https://github.com/OpenAgentsInc/openagents/issues/10782):

- **Clipmap.** The ocean draws on a camera-centered clipmap (M2) of 3, 4,
  or 5 rings by tier, out to the tier's draw distance and the horizon, and
  replaces the Water Lab's fixed warped grid. The estuary, the marsh, and
  the tide pools are ordinary patches cut on 8 m cells.
- **Streaming.** The baked depth, shore, shelter, and flow textures stream
  with the 8 m cells they overlap under `verse_engine::streaming`, and count
  toward the zone's resident bytes.
- **Coastal test scene.** W10's capture example is shaped like the coast's
  beach and harbor, so C1 starts from it.
- **Multiplayer split.** W10's two-client test is the base for the coast's
  [Multiplayer](#multiplayer) checks.
- **Shipped (W10).** `verse_zone_water::coast` holds the scene: `ground`,
  `ocean` and `water_set` (the gameplay sea), `field` (2 m texels, 128 m
  pages), `surface` (`WaterSurface::ocean`), `frame_water`, and `Buoys`.
  The shared clock is `physics::water::tick_at`, and host events are
  `physics::water::Event`. The bed must reach the horizon (the capture adds
  a flat far bed), because Medium and High refract what lies under the
  water.

### W11 measurement

[#10783](https://github.com/OpenAgentsInc/openagents/issues/10783) measures
the water system per tier. When it has landed, C6 replaces the coast's
target budgets with measured ones.

## Life and light

- **Wildlife.** Gulls over the beach and the island, crabs in the tide
  pools, fish schools on the reef, and seals on Gull Island's rocks. All of
  it is visual and seeded on each client, like Everglade's ducks. Gulls and
  seals avoid players within 6 m.
- **The lighthouse lamp.** A sweeping beam that turns on at golden hour and in
  fog, reflected on the water by screen-space reflection on High and the harbor's
  planar mirror on Medium and High, with a fog bell when the weather is Fog.
- **Hour.** The zone opens at golden hour and keeps the Water Lab's **T**
  toggle between golden hour and noon.
- **No enemies.** The coast has no combat encounters in this
  specification. Sea creatures to fight would follow the
  [combat model](combat-model.md) in a later issue.

## Streaming and budgets per tier

The terrain, including the sea bed's bathymetry, is generated in Rust, as
Everglade's ground is. Static models come from a pinned coast pack and
stream by 8 m cells near the camera, with proxies for distant cells (the
cliffs, the island, and the far isles), as
[Rendering scale](rendering-scale.md#6-distance-residency-and-hlod) plans for
Everglade. The ocean draws on its clipmap and doesn't stream by cell; its
textures count toward the zone's resident bytes.

These are targets for C6 to measure, not results, and C6 revises them from
W11's measurements. Over budget, a tier draws less and never fails.

| Budget | Low (WebGL2, GLES 3.0) | Medium (phones, WebGPU) | High (desktop) |
| --- | --- | --- | --- |
| Resident GPU bytes for the zone | ≤ 160 MiB, Everglade's `RESIDENT_BYTES_BUDGET` | ≤ 160 MiB | ≤ 224 MiB, the renderer's `textured::MAX_BYTES` |
| Of which water (from [Water](water.md#budgets-per-tier)) | ≤ 8 MiB | ≤ 32 MiB | ≤ 64 MiB |
| GPU time for water, 1080p equivalent | ≤ 3.5 ms | ≤ 3.5 ms | ≤ 4 ms |
| Fog end and draw distance | 600 m | 1.2 km | 2 km, the atmosphere's validated maximum |
| Clipmap rings | 3 | 4 | 5 |
| Spectral cascades | A baked looping tile | 2 × 64² | 3 × 128² |
| Reflection | Sky only | Sky and a half-resolution plane | Sky, a plane, and SSR |
| Triangles drawn a frame | ≤ 250,000 | ≤ 400,000 | ≤ 600,000, Everglade's `DRAWN_TRIANGLE_BUDGET` |
| Radius of cells at their near level | 64 m | 128 m | 256 m |
| Rowboats, floes, and debris simulated | 32, water's buoyant-body budget | 64 | 128 |
| Wildlife instances | 32 | 128 | 384 |
| Other players drawn | 16 | 32 | 64 |
| Compressed pack download | ≤ 48 MiB | ≤ 48 MiB | ≤ 48 MiB |

Phones run Medium; a browser runs Medium on WebGPU and Low on WebGL2.

## Multiplayer

The coast follows the water system's split between simulated and visual
state ([The surface model and determinism](water.md#the-surface-model-and-determinism))
and joins its own NIP-MV world, `verse-coast`, as Everglade joins
`verse-everglade`. Verse has no authoritative host for a zone world today
([Networking](networking.md)), so the coast uses NIP-MV's serverless
profiles and leaves an authority to a later hosted social profile.

- **No new stream for the sea.** The swell, the tide, the weather schedule,
  and rising water are functions of the shared clock and the zone seed, so
  every client computes them.
- **Rowboats** are NIP-MV [shared bodies](../../nips/openagents/NIP-MV.md#shared-bodies)
  in a body set `verse-coast.bodies.v1`: six boats with their home moorings.
  Boarding as the rower claims the boat (last toucher), the rower's client
  simulates it on the shared swell and reports it in its pose frames, and
  each client snapshots rest poses so a late joiner sees where boats were
  left. Passengers ride their seats and publish no pose of their own while
  seated. Debris and floes stay local.
- **Swimmers** publish poses as players do today. Other clients need each
  swimmer's medium (wading, swimming, or diving) to pick its animation.
  Carrying it in entity state is an additive change to
  [NIP-MV](../../nips/openagents/NIP-MV.md), reviewed under the Nostr skill
  in C6.
- **Spell events** (ice, a flood, a trench, a whirlpool, a redirected
  current, a Water Orb) need a cast event with a start tick, an area, and a
  duration, so other clients and late joiners see them. NIP-MV has none
  today; C6 proposes one as a gesture profile under the same review, and
  until it lands, spells on the coast are local to the caster.
- **Interest.** Pose frames and entity states carry NIP-MV's cell tags, 64 m
  cells by default (not the renderer's 8 m cells), and a client subscribes
  to the cells within its draw distance, so a swimmer at Gull Island
  doesn't receive the harbor's traffic at full rate.
- **Local only.** Spectral detail, foam, spray, ripples, cosmetic floes,
  and wildlife stay on each client, seeded where they must match.
- **Simulated company.** `openagents verse walkers 5 --world coast` puts
  walkers on the beach for multiplayer checks.

## Assets

Every model comes from a script under `scripts/blender/`, following the
[Blender pipeline](blender-pipeline.md): staged in
`assets/verse/generated/`, recorded in its
[`PROVENANCE.md`](../../assets/verse/generated/PROVENANCE.md), and admitted into
a pinned coast pack with levels of detail (`kit_lod.py`) by a
`coast_admit.py` that follows `everglade_admit.py`. The pack follows
Everglade's pinned-pack pattern
([`everglade_pack.rs`](../../crates/verse-zone-everglade/src/zones/everglade_pack.rs)):
a compiled SHA-256 and byte length, a content-addressed file name, and a
bounded decoder. The headland's grass and pines reuse the admitted foliage
(`foliage.py`), and the cottages reuse the town kit's textures. All
generated work is ours under the repository's license; no third-party mesh
or texture enters the pack without its license in `PROVENANCE.md`.

| Kit | Script | Pieces | Triangle budget |
| --- | --- | --- | --- |
| Coast rocks | `coast_rocks.py` (new) | Cliff modules (straight, corner, and inlet), sea stacks, the arch, the sea cave's mouth, boulders, the tide-pool shelf with pool hollows, and reef rocks, from displaced primitives on one rock atlas | 1,500 to 4,000 a module at the near level |
| Beach | `beach_props.py` (new) | Driftwood, dune fencing, beach grass cards, shells, seaweed wrack, rope, nets, crates, and barrels; floating props carry densities for buoyancy (P1) | Under 600 a prop |
| Harbor | `harbor.py` (new) | Piers and pilings, breakwater blocks, the boathouse, moorings and buoys, bollards, a hand crane, and two moored boats | Under 3,000 a piece |
| Lighthouse | `lighthouse.py` (new) | The tower, gallery, lamp room with an emissive lens, keeper's cottage, and fog bell | Under 12,000 in all |
| Boats | `boats.py` (new) | Oars for the existing `town/rowboat.glb`, the moored sloop, and the wreck's hull pieces | Under 4,000 a boat |
| Underwater | `reef.py` (new) | Kelp strands as vertex-animated cards, anemones, and the reef's small rocks | Under 200 a strand |
| Wildlife | `wildlife.py` (extended) | A gull (`idle`, `flap`, and `glide`), a crab (`idle` and `walk`), a seal (`idle` and `swim`), and a fish for schools, beside Everglade's duck and songbird | Under 800 an animal |
| Effects | `scripts/blender/fx/` (extended) | Surf crest spray and sea mist sheets, beside W6's splash and spray | Sprite sheets only |

## Tests and captures

- **Zone tests** (C1): entry from the plaza arch and return to the saved
  pose; intents scoped to the zone; the tide's level at fixed ticks; the
  sandbar dry at low tide and wet at high tide; the tide pools joined and
  separate; the bounds current turning a swimmer at the edge.
- **Bathymetry tests** (C1): `ground` matches the depth table within 0.5 m
  at sample points, and the bake is identical on two runs.
- **Boats** (C3): a rowboat crosses to Gull Island and back; one capsizes
  broadside in Storm surf; two clients converge on one boat's pose through
  shared-body reports.
- **Diving** (C4): breath runs out at the wreck without Water Breathing and
  not with it.
- **Transitions** (C5): Everglade's gate to the estuary and back keeps the
  saved plaza pose; the Grid gate's clearance tests.
- **Captures.** A `coast_capture` example renders fixed views (the bay from
  the terrace at noon and dusk, the surf, the harbor's mirror, the reef from
  below, the lighthouse in fog, and a storm) for every tier through both
  renderers, under `bench/verse/<date>/coast/`, as the `water_capture`
  example does.
- **Devices.** Phone and browser runs go in `NEEDS_OWNER.md` (C6).

## Phases and estimates

Estimates are agent-hours at the water system's pace. Each phase is one
issue, merges on its own, and leaves every tier correct.

| Phase | Issue | Work | Blocked by | Estimate |
| --- | --- | --- | --- | --- |
| C1 | [#10885](https://github.com/OpenAgentsInc/openagents/issues/10885) | Revise this specification against what W4 and W10 shipped; the zone shell: identity, the plaza arch, generated terrain and bathymetry with its baked fields, the ocean body with the tide and the shared clock, the harbor's shelter mask, the estuary, the spawn and return portal, the shared spell crate, and zone tests | W4, W10, W3 | 10 h |
| C2 | [#10886](https://github.com/OpenAgentsInc/openagents/issues/10886) | The Blender kits and the pinned coast pack, with admission, levels of detail, and captures | C1 | 12 h |
| C3 | [#10887](https://github.com/OpenAgentsInc/openagents/issues/10887) | The harbor's rowboats on swell as shared bodies, Gull Island, the sandbar and Tern Islet, and the offshore bounds current | C2, W6 | 8 h |
| C4 | [#10888](https://github.com/OpenAgentsInc/openagents/issues/10888) | Diving and tide pools: the reef, kelp, the wreck, fish, crabs, caustics, and the tide-pool bodies | C2, W7 | 8 h |
| C5 | [#10889](https://github.com/OpenAgentsInc/openagents/issues/10889) | Zone-to-zone transitions in the runtime, Everglade's estuary gate, and the Grid's coast gate | C1 | 6 h |
| C6 | [#10890](https://github.com/OpenAgentsInc/openagents/issues/10890) | The coast's climate, spells on the open sea, the NIP-MV medium field and cast events, the multiplayer checks, and per-tier measurement against W11; device runs go in `NEEDS_OWNER.md` | C3, C4, W8, W9 | 6 h |

The total is about 50 agent-hours. C4 and C5 can run beside C3.

Each phase's acceptance:

- **C1.** This document matches W4's and W10's shipped interfaces;
  `verse --coast` and the plaza arch enter and return; the zone and
  bathymetry tests above pass; a capture of the bay per tier is committed.
- **C2.** The coast pack is pinned with its digest file kept, within the
  48 MiB download and the resident budget; `PROVENANCE.md` lists every
  model; captures show each kit in place.
- **C3.** The boat tests pass; Gull Island and Tern Islet are walkable; the
  bounds current turns swimmers and boats at the edge.
- **C4.** The diving test passes; captures from below show caustics and sun
  shafts on the reef and the wreck per tier.
- **C5.** The transition tests pass on the desktop and in the OpenAgents
  app's Grid.
- **C6.** The budget table holds measured numbers; two clients agree on the
  sea, the boats, and the cast events; the NIP-MV changes are reviewed and
  merged; device runs are recorded in `NEEDS_OWNER.md`.

## Open questions

1. **Name.** "The coast" is a working name for the zone and its arch.
2. **Combat.** Should the coast have sea creatures to fight, or stay a
   place to explore?
3. **The plaza arch's spot.** `(0, 0, 24)` is unverified against the
   plaza's structures; C1 decides, and the alternative is a ring of arches
   farther out.
4. **The Water Lab's future.** Once the coast ships, the Lab can stay a
   separate demo, or become a cove inside the coast reached on foot, which
   frees an arch.
5. **Authority.** Shared bodies trust every participant. If boats or spells
   ever decide anything with stakes (races, combat), the coast needs a
   hosted social profile ([Networking](networking.md)) instead.
6. **Tide period.** 24 minutes shows a full tide in a long session; a
   shorter period would make the sandbar a puzzle, a longer one a rarity.
