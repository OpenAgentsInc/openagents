# UE5 ruins study: Valley of the Ancient as a reference

This page studies Epic's Unreal Engine 5 ruin demos as a reference for Verse.
It says what the demos are, what Verse already has for each technique, the
plan in order of payoff, what an agent inventories from the downloaded
project, and what the license lets us do with it. In short: we study the
project and build our own models in Blender. Nothing from it ships.

Status on October 5, 2026: the project is installed and complete at
`~/Documents/Unreal Projects/ValleyoftheAncient` (19,784 files, 92.1 GB,
engine 5.7), and the editor-level study list below hasn't started, because
Unreal Engine 5.7 isn't installed.

The [content index](valley-of-the-ancient-index.md) inventories the installed
project, and [Female character](female-character.md) specifies an original
player character informed by Echo.

## The download

This section records the download as it stood at 17:50 local; the
[content index](valley-of-the-ancient-index.md#where-it-is) has the
installed location and the vault copy. The Epic Games Launcher installed the
Fab listing **Valley of the Ancient** (listing
`0c19880e-21bd-42ba-8287-1caccc3951b1`, app `AncientGame_5.7`, engine
association 5.7). At 17:50 it wasn't yet under `~/work/UnrealEngine` or
`~/Documents/Unreal Projects`. Here is where it was:

| Path | What's there on October 5, 2026, about 17:50 local |
| --- | --- |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/AncientGame_5.7/stage/` | 23 GB of launcher chunk files (`f/`, about 16,000 chunks), resume data (`m/`), and a backing store (`c/`). Not readable as a project. |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/AncientGame_5.7/data/` | Empty. |
| `/Users/Shared/UnrealEngine/Launcher/VaultCache/FabLibrary/Valley_of_the_Ancient-0c19880e/unreal-engine/manifest` | The 7.1 MB build manifest. |
| `/Users/christopherdavid/Documents/Unreal Projects/ValleyoftheAncient` | The install destination, from `~/Library/Application Support/Epic/EpicGamesLauncher/Data/DownloadManager/DownloadState_*.json`. It doesn't exist yet. |

The manifest lists 19,784 files and 92.1 GB installed. The launcher keeps
the staged chunks while it installs, so peak disk use is about twice that,
which matches the owner's figure of roughly 205 GB. The data volume has
564 GB free. Here's the planned layout, from the manifest:

| Folder | Size | Files |
| --- | --- | --- |
| `Content/AncientContent/Megascans/` (`3DAssets`, `Surfaces`, `3DPlants`, `Decals`) | 38.8 GB | 1,018 |
| `Plugins/GameFeatures/AncientBattle/` (boss, destruction, battle map) | 16.8 GB | 2,478 |
| `DerivedDataCache/` | 16.5 GB | |
| `Content/AncientContent/Geometry/` (`PillarCollection`, `MASS`, `Buttes`, `SpireRockCollection`, `StoneBlock`, `ErosionGround`, `GroundTiles`, and others) | 15.9 GB | 549 |
| `Content/AncientContent/Characters/Echo/` | 1.6 GB | 183 |
| `Content/AncientContent/` other (`Materials`, `Maps`, `Audio`, `Effects`, `Lighting`, `Clouds`) | 2.0 GB | |
| `Content/__ExternalActors__/` (World Partition, one file per actor) | 0.4 GB | 14,244 |
| `Config/`, `Source/`, `Plugins/GameFeatures/HoverDrone/`, `AncientGame.uproject` (renamed `ValleyoftheAncient.uproject` on install) | small | |

The maps are `AncientWorld`, `Startup`, `Megascans_Asset_Zoo`, the battle map
`L_AncientBattleGameplay`, and 169 MegaAssembly maps under
`Content/AncientContent/Maps/MASS/` (for example
`Packed/MASS_Cliff_L_09_Packed.umap` and `Packed/MASS_Butte_XXL_01_Packed.umap`).

`~/work/UnrealEngine` is a 41 GB source checkout of Unreal Engine 5.8.3 with
no macOS binaries built, and the launcher records no installed engine. To
open the project in the editor, the owner needs Unreal Engine 5.7 from the
launcher, or a built 5.8 that upgrades the project in place.

## What the demos are

- **"Lumen in the Land of Nanite"** (May 13, 2020) was the UE5 reveal: a
  real-time PlayStation 5 walkthrough in which the character Echo crosses
  photogrammetry stone ruins, a cave, and a statue hall, lit by Lumen's
  dynamic global illumination. Epic never released its content.
- **"Valley of the Ancient"** (May 26, 2021) shipped with UE5 Early Access as
  a playable sample. Echo returns, but the setting is a desert canyon built
  from Quixel Megascans scanned in Moab, Utah, about 90 percent of its
  environment assets, plus a "Dark World" swapped in through data layers and
  a boss fight against the Ancient One. It uses Nanite, Lumen, World
  Partition with one file per actor, MetaSounds, and Chaos destruction played
  back from physics caches.
  ([Epic's sample documentation](https://dev.epicgames.com/documentation/en-us/unreal-engine/valley-of-the-ancient-sample-game-for-unreal-engine))
- **Nanite** is described in Brian Karis's SIGGRAPH 2021 course "Nanite: A
  Deep Dive" (Advances in Real-Time Rendering).

The stone corridors the owner remembers belong to the 2020 reveal. Valley
has stone pillars, blocks, and spires (`Geometry/PillarCollection`,
`StoneBlock`, `SpireRockCollection`), but most of it is cliffs, buttes, and
ground.

## The external analysis, corrected

The owner pasted an analysis from ChatGPT. It's an external analysis, not
ours. Its claims and our corrections:

| Claim | Correction |
| --- | --- |
| Nanite builds offline cluster hierarchies, selects clusters by screen-space error, and streams cluster groups. | Right in outline. The hierarchy is a DAG of cluster groups, not a tree, so neighboring clusters simplify together without cracks, and selection runs per cluster in parallel. It also leaves out what makes small triangles cheap: a compute software rasterizer and a visibility buffer that shades each pixel once. |
| 433 million input triangles became 882 million Nanite triangles and about 4.6 GB compressed in the 2020 demo, with tens of millions drawn per frame. | We haven't checked these numbers against the talk. If they're right, 882 million counts every level of the hierarchy, so it's the stored total, not a refinement of the source. Epic's 2020 talk described about a billion source triangles per frame reduced to about 20 million drawn. Treat both as approximate. |
| The environments were assembled from Megascans "MegaAssemblies". | Right for Valley: the project carries 169 MegaAssembly maps, which are packed level instances of scans. |
| Lumen's dynamic GI is half the look of stone corridors. | A judgment, and a fair one for enclosed spaces where bounce light sets the mood. |
| World Partition streams cells. | Right. Valley also stores each actor in its own file (`__ExternalActors__`, 14,244 files). |
| Valley includes Chaos-fractured destructible assets. | Right, with a qualifier: the boss destruction plays back recorded caches (for example `Robot_Head_Destruction_Cached.umap`), not a live simulation. |

The analysis recommends, in order:

1. Hero static scanned geometry with offline LODs.
2. A hidden destructible structural representation that swaps in on damage.
3. Separately chunked terrain.
4. World-cell streaming.
5. GPU instancing.
6. Hierarchical frustum culling and Hi-Z occlusion culling.
7. Streamed textures.
8. Better dynamic GI before Nanite.
9. GPU-driven indirect drawing.
10. Cluster-based virtual geometry last.

It also recommends narrow, detailed passages that open into a big reveal.
It didn't know what Verse has. Two of its steps are already done, and one
doesn't fit our budgets. The next sections ground it in the code.

## What Verse has for each technique

All paths are under the repository root.

| Technique | UE5 approach | Verse today | Gap |
| --- | --- | --- | --- |
| Dense static geometry | Nanite clusters, no authored LODs | Static placements merge per pass, material, 8 m cell (`CELL` in `crates/verse-pbr/src/pbr/textured.rs`), and detail level into one indexed draw per cell. Budgets: 1,250,000 placed and 600,000 drawn triangles per frame (`crates/verse-zone-everglade/src/zones/everglade_pack.rs`). | No virtual geometry. Our budgets are about 100 times below Nanite's. |
| Levels of detail | Automatic in Nanite | `Detail` (always, near, far) against up to 8 switch distances, with 2.5 m hysteresis, in `textured.rs` (commit `9725702a71`). `scripts/blender/everglade_lod.py` generates 53 far levels offline; `crates/verse-zone-everglade/src/zones/everglade/detail.rs` switches them at 60 m. | LODs come from a per-zone Blender script, not from the pack compiler. Two levels only. |
| Frustum culling | GPU, per cluster | CPU, per cell: `in_frustum` and `drawn()` in `textured.rs`, with a fog-distance cull and a small-size cull. | No hierarchy above cells. |
| Occlusion culling | Hi-Z, two passes | None. | All of it. Matters most in corridors and crypts. |
| GPU instancing and indirect draw | GPU-driven | Skeletal figures (`imported/instancing.rs`) and repeated static models (`pbr/instanced.rs`, in runs per 8 m cell) in `crates/verse-pbr`. Placements a zone edits stay merged. | No indirect or multi-draw. |
| Dynamic global illumination | Lumen (software and hardware ray tracing, surface cache) | Baked: a CPU ray bake through a BVH stores per-vertex sky visibility and one sun bounce, and an order-one SH probe grid lights characters (`crates/verse-pbr/src/pbr/textured_bake.rs`). The sky is order-two SH with a prefiltered cube (`pbr/environment.rs`). GTAO and contact shadows on the High tier (`pbr/screen.rs`). Up to 32 unshadowed point lights (`Neon::lamps`, commit `2e4f70fc17`). Emissive materials. | No bounce from point lights or emissive surfaces, and no dynamic update when geometry changes. Audit finding V22 says to diagnose readability before adding a general GI system (`docs/audits/2026-10-04-verse-engine-audit.md`). |
| Shadows | Virtual shadow maps | Cascaded sun shadows, 2 or 3 cascades; cascades after the first cache static casters (`crates/verse-pbr/src/pbr/gpu.rs`, `verse_engine::lighting::fit_cascades`). | Point lights cast no shadows. |
| Atmosphere | Sky atmosphere, volumetric fog | Height fog (`verse_engine::lighting::HeightFog`, `crates/verse-pbr/src/fog.rs`), an ephemeris sky (`pbr/sky.rs`), sprite particles for dust and smoke (`crates/verse-pbr/src/fx/`). | No volumetric light shafts. |
| World streaming | World Partition cells, one file per actor | Cooked static chunks stream under CPU and GPU residency budgets (`crates/verse-engine/src/streaming/`, `crates/verse-pbr/src/streaming.rs`, audit V12). | Streamed chunks don't cast cascade shadows or enter the screen-space prepass, and no zone uses streaming for its main content. |
| Textures | Virtual textures, streamed mips | PNGs in the pack, base color only in the textured path, at most 64 images of up to 2048 px; mips cooked on the CPU at upload by role (`verse_engine::mips`, audit V13). | No normal or ORM maps in the textured path, no GPU compression, no streaming. For stone, missing normal maps cost more than missing streaming. |
| Content packaging | `.uasset` cooked per platform | VTP3 packs (`crates/verse-zone-everglade/src/zones/everglade_pack/format.rs`), compiled by `crates/verse-zone-everglade/src/zones/everglade_pack/compile.rs`, with limits `Limits::EVERGLADE`: 12 MiB pack, 480,000 triangles, 20,000 per model, 1024 px textures. | No mesh processing in the compiler. |
| Static-to-destructible swap | Chaos geometry collections, often cached | A building stays in the merged cells until hit, then rises into a `Site` of physics bodies. Its static triangles are hidden by rewriting their indices (`IndexEdits` in `textured.rs`), and the pieces draw posed (`crates/verse-zone-everglade/src/zones/everglade/demolition/town.rs`). Models are carved on a lattice (`carve.rs`), chunked with capped interiors (`chunks.rs`), and held by a support graph that topples unsupported pieces with `crates/physics` (`site.rs`). | Already built. See the plan. |
| Destructible ruin | Chaos fracture | None. The Ruins zone and its voxel ruin (a 32 x 16 x 32 grid of 0.5 m voxels that Fireball carved, `docs/verse/ruins-source-parity.md`) were removed on 2026-10-05. | No destructible ruin; the demolition swap above is the destruction path. |
| Camera in tight spaces | Spring arm with collision | A sphere sweep through the `Sight` trait (`crates/verse-world/src/social/sight.rs`), dispatched per zone in `crates/verse/src/zones/sight.rs`. | Needed for corridors; exists. |
| Authoring | Megascans, Quixel Bridge, MegaAssemblies | Blender scripts under `scripts/blender/` run headless and are the source of every model ([Blender pipeline](blender-pipeline.md), [asset runbook](asset-runbook.md)). The crypt lab and great crypt are Reference-mode models from primitives (`scripts/blender/chamber_lab.py`, `scripts/blender/great_crypt.py`). | No assembly concept: no reusable packed groups of models. |

Platform tiers (`crates/verse-engine/src/quality.rs`) bound all of this.
Low serves WebGL2 with 2 cascades, fixed PCF shadows, and no screen-space
effects. Medium serves phones and browser WebGPU without screen-space
effects. High is desktop with GTAO. Anything in the plan must degrade to
Low.

## The plan

Estimates are agent-hours at the pace defined in
[the smart terminal's estimate basis](../terminal/smart-terminal.md): one
coding agent in one area, including tests and a capture. The order is by
payoff per hour, not by the external analysis's order.

| Order | Work | Hours | Payoff |
| --- | --- | --- | --- |
| 1 | A ruins kit and passage, from the study | 4 to 6 | The look, in our own models |
| 2 | Normal maps in the textured path | 2 to 3 | Stone reads as stone at Low cost |
| 3 | LOD generation in the pack compiler | 3 to 4 | Every zone gets far levels without a script per zone |
| 4 | Per-cell occlusion culling | 3 to 5 | Corridors and crypts draw what's visible |
| 5 | Lamp bounce in the bake, then probe updates | 4 to 8 | Candle and torch light fills stone rooms |
| 6 | Assemblies: packed groups of placements | 2 to 3 | Cliff and ruin sets placed as one unit |
| 7 | Streaming as a zone's main content | 4 to 6 | Zones larger than one pack |
| 8 | Virtual geometry research | 2, first step only | A measured answer on whether it pays |

1. **Ruins kit and passage.** Run the inventory in
   [the study list](#the-study-list), then build a stone passage kit, broken
   columns, arches, debris sets, and boulder assemblies in Blender in
   Reference mode, with our own or CC0 textures. Place them as one passage
   that ends in a reveal ([level design](#level-design-passages-to-a-reveal)).
   Use the great crypt (`crates/verse-world/src/great_crypt.rs`) or
   Everglade's edge as the host; the Ruins zone was removed on 2026-10-05.
   This is the only item that changes what a player sees in a day.
2. **Normal maps.** The textured path reads base color only. Scanned stone
   gets most of its detail from normal maps, and a normal sample costs
   little on every tier. Add the normal image to `TexturedMaterial`, bake
   normals from a high-polygon Blender sculpt onto the low model, and keep
   the 64-image bound by sharing images, as the crypt lab did.
3. **LOD generation in the compiler.** Move what
   `scripts/blender/everglade_lod.py` does into the pack build: decimate
   each admitted model to a recorded ratio, keep its materials, and write
   the far level beside it. Either the compiler calls Blender headless, or
   it simplifies with quadric error in Rust; the second keeps packs
   reproducible without Blender. Keep `Detail` and the hysteresis as they
   are.
4. **Occlusion culling.** Start on the CPU, which works on WebGL2: author a
   few large occluder boxes per zone (walls, cliffs), or compute
   cell-to-cell visibility from the merged cells offline, and skip cells
   behind them. A Hi-Z pass needs compute and depth readback, so it waits
   for the High tier and a measured need.
5. **Lighting.** Audit finding V22 asks to diagnose readability first, so
   begin with captures of the crypt lab at each tier. Then let the CPU bake
   (`textured_bake.rs`) gather static lamps as well as the sun, so candles
   and braziers bounce onto stone. Update probes near a destroyed building
   after the swap. Screen-space GI on the High tier comes after, if
   captures still show flat interiors.
6. **Assemblies.** Valley's MegaAssemblies are packed groups of scans
   placed as one actor. Our equivalent is a named list of placements in the
   zone's layout that the merge treats like any other placements, so it
   costs nothing at draw time.
7. **Streaming.** The chunk stream exists (audit V12). Make a zone's main
   content stream, and close the two gaps: streamed chunks must cast
   cascade shadows and enter the prepass. Do this when a zone outgrows the
   12 MiB pack, not before.
8. **Virtual geometry.** Our budgets are hundreds of thousands of
   triangles, and WebGL2 has no compute, so Nanite's design doesn't fit
   today. The first step is a measurement: build a cluster hierarchy
   (meshlets of 128 triangles, grouped and simplified) offline for one
   dense model, select clusters by screen-space error on the CPU, draw the
   selection through the existing indexed path, and report triangles drawn
   and frame time against the two-level `Detail` path. Go further only if
   that wins.

Two of the external analysis's items are already done:

- **Hidden destructible representation.** This is our demolition system:
  static cells until the first hit, then a raised `Site` with a support
  graph, and index rewrites hide the static triangles
  (`crates/verse-zone-everglade/src/zones/everglade/demolition/town.rs`,
  `docs/verse/destructible-buildings.md`). A ruin uses it as is: carve the
  column or wall on the lattice (`carve.rs`), and the support graph topples
  what loses support. Unlike Valley's caches, ours is a live simulation,
  bounded by `MAX_LIVE` and `MAX_CHUNKS` (smaller on web and phone).
- **Separately chunked terrain.** Terrain is its own mesh in each zone, and
  the 8 m cells chunk everything placed on it.

Indirect drawing stays off the list, since it needs features WebGL2 lacks.
GPU instancing landed on 2026-10-06 for memory rather than draw calls:
merged cells copied every repeated model, and instances upload it once
([Rendering scale](rendering-scale.md)).

## The study list

Run this list in the Unreal Editor once the project opens, or by reading
the project's files. Record numbers and screenshots in a study note under
`bench/verse/<date>/valley-study/`; screenshots stay in that note as
reference and never enter a pack. Each item names the Verse model or
feature it informs.

- [ ] **Levels and World Partition.** Open `AncientWorld`. Record the world
  size, the World Partition grid cell size and loading range, the data
  layers (light world and Dark World), and the actor count. Informs: the
  streaming plan (item 7) and zone size targets.
- [ ] **MegaAssemblies.** List the 169 MegaAssembly maps by type (cliff, butte,
  boulder, ground, spike, fireplace). For five of them, count the member
  meshes and record their scale ranges and how they interlock. Informs:
  assemblies (item 6), and our **cliff assembly** and **boulder assembly**
  sets.
- [ ] **The ruin and cliff areas.** Find the pillar fields
  (`Geometry/PillarCollection`), stone blocks, and spires in the level.
  Record their heights, spacing, and how scans meet the ground. Informs:
  **broken columns**, **stone blocks**, **spire rocks**, and a **ruin debris
  set**.
- [ ] **Unique meshes against instances.** In a region, count unique static
  meshes and their placements. Informs: how many models our ruins kit needs
  for the same variety.
- [ ] **Triangle counts and Nanite settings.** For 20 representative
  meshes, record the source triangle count, Nanite enabled, fallback
  triangle percent, and position precision. Informs: LOD ratios (item 3)
  and the virtual geometry measurement (item 8).
- [ ] **Textures and materials.** Record texture sizes, which maps each
  material uses (albedo, normal, roughness, displacement), tiling, detail
  normals, and how materials blend rock with sand. Informs: normal maps
  (item 2) and a **stone material set** we paint or source from CC0.
- [ ] **Lumen and post settings.** Record the post process volume:
  Lumen GI and reflection methods, final gather quality, exposure, bloom,
  fog, and sky light. Capture one view with Lumen on and off. Informs:
  lighting (item 5) and our color grade (`Neon::grade`).
- [ ] **Destructible assets.** In `Plugins/GameFeatures/AncientBattle/Content/Destruction/`,
  record each geometry collection's fracture pattern, piece count, cluster
  levels, and whether it plays live or from a cache. Informs: carve
  settings for stone in `carve.rs` and a **collapsing pillar** and **arch**.
- [ ] **Echo's animation set.** List the animation sequences and blend
  spaces under `Characters/Echo/`: locomotion, turn-in-place, ledge and
  traversal moves, and hand plants. Record names and lengths only. Informs:
  which moves our character controller lacks. Echo's model and animations
  are not ours to use.
- [ ] **One 50 x 50 m section.** Pick a section with ruins and a cliff
  edge. Inventory every actor in it: mesh, triangles, scale, material, and
  light. Count the triangles drawn from three viewpoints. Informs: the
  first ruins passage (item 1) as a one-to-one composition target, and
  whether our 600,000 drawn-triangle budget can carry that density.

The models this list produces, all ours, built in Blender:

| Model or set | Informed by |
| --- | --- |
| Stone passage kit (wall runs, floor slabs, steps, a doorway, a collapsed section) | The 50 x 50 m section, ruin areas |
| Broken columns (three heights, two broken tops, fallen drums) | `PillarCollection` |
| Arches (intact, cracked, half fallen) | Ruin areas, destructible assets |
| Temple facade (a stepped base, a column screen, a pediment) | The 2020 reveal's style; Valley lacks one |
| Statues (a seated figure, a broken head) | The 2020 statue hall; Valley lacks one |
| Cliff and boulder assemblies | `MASS_Cliff_*`, `MASS_Boulders_*` |
| Ruin debris sets (rubble piles, scattered blocks, sand drifts) | Ground and erosion meshes |
| Stone materials (block, weathered, carved, sand) | Material setups |

## Licensing

**Conclusion: we may study Valley of the Ancient. We may not convert,
import, or ship any of its meshes, textures, Megascans, animations, sounds,
or materials into Verse.**

- Epic's Content License Agreement defines UE-Only Content as content
  "designated as only permitted for use in conjunction with Unreal Engine
  and Unreal Engine-based products," usable only in a product that
  requires the engine code to run, or in rendered video and images
  ([Epic Content EULA](https://www.unrealengine.com/eula/content)).
- Epic labels its own sample content on Fab "UE-Only Content - Licensed for
  Use Only with Unreal Engine-based Products"; the City Sample carries the
  label, and so did the Moab Desert Collections, which hold Valley's
  assemblies. We couldn't load the Valley listing itself (Fab blocks
  automated reads), so the owner should confirm its license panel. Until
  then, treat it as UE-Only.
- Fab's Standard License does allow other engines for listings sold under
  it. Our reading is that it doesn't change the conclusion: Megascans bundled
  inside an Epic sample come under the sample's license, not under a
  Standard License bought for a separate listing.
- Verse doesn't require Unreal Engine code, so UE-Only content can't ship
  in it, and our [asset runbook](asset-runbook.md) only admits content
  whose license allows redistribution and derivative works.

Our mode is the runbook's **Reference only**: nothing from the kit ships,
and the provenance says which kit was studied. This is narrower than the
runbook's Reference mode, which allows a kit's textures; Valley's textures
are excluded too.

An agent may:

- open the project in the Unreal Editor and inspect levels, World
  Partition, assets, counts, materials, and lighting settings;
- read the project's files and the launcher manifest to list names, sizes,
  and structure;
- record numbers, names, and its own written observations in the
  repository;
- take screenshots for the study note, as reference, outside any pack;
- build original models in Blender from primitives and sculpts, with our
  own or CC0 textures, and record in each model's provenance that it was
  made in Reference-only mode with Valley of the Ancient as the study.

An agent may not:

- export any mesh, texture, material, animation, or sound from the project
  (FBX, glTF, OBJ, USD, PNG, or any other format), or copy the project's
  files;
- trace, project, or bake from Valley's geometry or textures onto our
  models, or sample its textures for color;
- commit anything from the project, including screenshots inside a pack,
  to the repository;
- feed the project's assets to an AI model to generate ours;
- import Valley content into Blender for any purpose.

## Level design: passages to a reveal

The 2020 reveal walks the player through a narrow, dark, detailed passage,
then opens onto a wide, bright space with a distant landmark. Detail is
dense where the camera is close; the reveal spends its budget on
silhouette and light. It also suits our renderer: in a passage, occlusion
culling (plan item 4) cuts most of the zone, and at the reveal, far levels
and fog carry the distance.

- **Ruins zone.** The Ruins zone, a 300 m heightfield with one voxel ruin,
  was removed on 2026-10-05, so it no longer hosts a passage. The gully
  composition planned for it (two cliff assemblies forming a 3 m wide gully
  lined with broken columns, turning once so the ruin stays hidden until the
  last bend, then opening onto the ruin in silhouette) can serve another
  host.
- **Great crypt.** The crypt's nave (`crates/verse-world/src/great_crypt.rs`)
  is already a long room. Add a low, narrow entry stair from the
  surface with lamps at wide spacing, so the eye adapts to dark, then open
  into the nave at its full height with the chapel lit at the far end
  (`CHAPEL_BACK`). Keep the cultist fight's lines of sight as they are.
- **Everglade's edges.** Everglade is a 510 m square whose edge is a rising
  tree ring and a forest belt (`crates/verse-world/src/social/everglade.rs`,
  `docs/verse/everglade.md`). Add one ruin path through the belt: a sunken
  stone track between root-split walls, which opens on a clearing with a
  collapsed temple facade. Within the square, it needs no streaming and
  reuses the demolition swap for the facade.

## Open questions for the owner

1. Can you confirm the license shown on the Fab listing for Valley of the
   Ancient? If it says anything other than UE-Only, the conclusion above
   still holds for the bundled Megascans until you say otherwise.
2. Which Unreal Engine do you want for the study: 5.7 from the launcher, or
   a built 5.8.3 from `~/work/UnrealEngine`, which upgrades the project?
   The launcher has no engine installed yet.
3. Which zone hosts the first ruins passage: the great crypt or a new path
   at Everglade's edge? (The Ruins zone was removed on 2026-10-05.)
4. Should the style follow Valley's desert canyon, or the 2020 reveal's
   darker stone halls and statues?
5. Is the 600,000 drawn-triangle budget fixed for desktop, or can the High
   tier take more once occlusion culling lands?
6. Do you want normal maps (plan item 2) before the ruins kit, so the kit's
   first capture shows them?
