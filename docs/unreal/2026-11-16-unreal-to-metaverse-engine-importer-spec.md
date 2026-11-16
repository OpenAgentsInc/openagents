# The Unreal to Metaverse Engine importer: specification

Specification, 2026-11-16. Status: proposed. Nothing here is built. No
issues are filed yet. The first target is the Splice Bots game jam entry
(section 7).

[The exporter](2026-11-12-aethyr-to-unreal-exporter-spec.md) turns a
Metaverse Engine game into an Unreal Engine (UE) 5.9 project. This document
specifies the reverse: the smallest importer that turns content made in
Unreal into Metaverse Engine content: packs, terrain, placements and
collision that a realm loads and a player can walk through.

The exporter's last section left this door open. It made three choices for
the import direction: glTF both ways, one manifest schema, and names that
survive a round trip. It also said what import cannot carry: logic. This
spec keeps all four.

Background:

- [The exporter's specification](2026-11-12-aethyr-to-unreal-exporter-spec.md)
- [What belongs in the prototype and what belongs in Unreal](2026-11-14-what-belongs-in-unreal.md)
- [Why the two builds differed](2026-11-12-splice-unreal-vs-aethyr-session-audit.md)
- [Learning from UE 5.9](2026-11-11-learning-from-unreal-5-9.md), section 5 (licensing)
- [Web-built Unreal games in the Metaverse](web-games-in-the-verse.md), section 7 (legal)
- [MCP tools](mcp-tools.md) and [UE6](ue6.md)
- The exporter's own guide: `metaverse-engine/export/unreal/README.md`

## 1. Decision

**The importer moves content. It never moves logic.**

| Part of an Unreal project | What it becomes in the Metaverse Engine |
|---|---|
| Static meshes | Pack meshes in the pack's 48-byte vertex (`metaverse-engine/editor/vertices.coax`), one per source asset, drawn as instances |
| Placements (actors, instanced and foliage components) | One row per instance: mesh, position, rotation, scale, stable name, in the pack's cell content (`metaverse-engine/host/pack-cells.coax`) |
| Textures | Pack texture layers (colour) and data layers (normal and ORM, linear) |
| Materials | A material record: the textures it uses and its scalar and vector parameters. The material graph is not translated (section 6) |
| Simple collision | Boxes, spheres, capsules and convex hulls, checked against the visible mesh |
| Landscape | Terrain tiles in the `aethyr.terrain.tile.v1` profile (`metaverse-engine/world/terrain-tile.coax`) |
| Named spots (player start, tagged actors, our markers) | Named placements, like the exporter's `placements` |
| Lights | A key light, point and spot lights, as numbers. The look is set by the realm, not copied |
| Sound waves | Pack sounds |
| Skeletal meshes and animation | The existing animation path (`metaverse-engine/tools/anim-unreal-export.coax`), extended, not rebuilt |
| Blueprints, Verse, C++ gameplay, StateTree, GAS, widgets | Nothing. Each is listed by name in the bundle's `skipped.json` |
| Niagara, Sequencer, MetaSounds, sound cues | Nothing in the first version. Listed in `skipped.json` |

Why this is the minimum:

- **Logic does not translate, and should not.** The exporter hosts the
  rules in Unreal so there is one copy of them. A Blueprint or Verse
  translator would make a second copy, in the other direction, against a
  framework Epic removes in UE6 ([ue6.md](ue6.md)). An imported Unreal
  project arrives as a world to write Coax rules for.
- **The boundary already exists in the repo, three times.** The animation
  importer reads skeletal meshes and clips from an isolated project
  (`metaverse-engine/anim/unreal-source.coax`, `anim-unreal-export.coax`).
  Valley Lab read a World Partition map's actors, instances, materials and
  textures into a private pack (`metaverse/tools/valley-world-export.py`,
  `valley-world-assets.py`, `valley-world-pack.coax`). The browser editor
  admits dropped glTF, PNG and sound files into a pack under a licence
  (`metaverse-engine/editor/pack-import.coax`). This spec generalizes those
  three into one engine tool. It does not start again.
- **The Unreal side stays thin.** Python in the editor only reads and
  writes files (firm reason 6, `coax/tools-in-coax.md`). Coax owns the
  coordinate change, the welding, the budgets, the licence check and the
  pack bytes. That is the rule Valley's adapters already follow: "The Coax
  packer owns coordinate conversion, material selection and budgets."

### 1.1 Two uses

| Use | Source | What comes back | First target |
|---|---|---|---|
| **Level import** | Any UE 5.9 project and one map in it | A walkable realm: meshes, placements, terrain, collision, named spots, lights, sounds | **The Splice Bots game jam entry** (section 7) |
| **Round trip** | A project the exporter made (`AethyrHost.uproject`), after a designer has worked in it | The designer's content from `/Game/<Game>/Overrides/`: meshes for `kit.`, `thing.` and `world.` rows, their textures, and `event.` sounds. Also a report of generated actors moved by hand | Splice, from the exporter's project (section 7.4) |

The jam entry comes first because it is the game the founders want back
in the engine. It was built by hand in Unreal, so it has no exporter
names, but most of its content already sits outside Unreal's formats
(section 7). The round trip comes second. It has an exact answer: the
world the importer reads back must equal the world the exporter wrote.

### 1.2 What the round trip does not change

[What belongs in Unreal](2026-11-14-what-belongs-in-unreal.md) sets one
direction of data: the rules tell Unreal what happened, and Unreal tells
the rules nothing but input. The importer keeps that line.

- **Content comes back. Layout does not.** A designer's mesh for
  `kit.garage_bay` can come back so the prototype page draws it. A wall
  moved in Unreal does not move the rules' wall. The importer writes the
  move to `layout-delta.json`, by the exported name, and someone edits the
  game's Coax if the move should stand. Nothing writes Coax source
  automatically.
- **Presentation that is Unreal's stays Unreal's.** Niagara systems
  (`fx.`, `trail.`, `state.` rows), `look.level`, `look.params`,
  `look.shadow` and `material.` rows are reported as skipped. The
  prototype keeps its own stand-in for each effect, as that audit
  recommends.
- **The prototype stays small.** Imported content is a separate pack that
  a page opts into (`?pack=NAME`), with its own byte budget. The
  prototype's default page does not grow. Splice's page is 443 KB
  ([download size](2026-11-12-splice-download-size.md)).

## 2. What an Unreal level is made of

What the importer reads, and how, for UE 5.9. "In repo" means a script on
`main` already calls it. "Unverified" means step 0 must confirm it.

| Piece | Where in Unreal | Read with | Status |
|---|---|---|---|
| Map and its actors | `UWorld`, World Partition actor descriptors | `EditorLoadingAndSavingUtils.load_map`, `WorldPartitionBlueprintLibrary.get_actor_descs`, `load_package` | In repo (Valley) |
| Static mesh placements | `StaticMeshComponent`, `InstancedStaticMeshComponent` | `get_components_by_class`, `get_world_transform`, `get_instance_transform(i, world_space=True)` | In repo (Valley) |
| Foliage | `InstancedFoliageActor`'s instanced components | The same instanced component calls | Unverified for foliage actors |
| Mesh geometry | `StaticMesh` source LOD 0 | `AssetExportTask` to glTF, one file per mesh | In repo (Valley) |
| Textures | `Texture2D` | `AssetExportTask` to PNG | In repo (Valley) |
| Material parameters | `MaterialInstance` | `MaterialEditingLibrary` texture, scalar and vector parameter getters | In repo (Valley) |
| Material baked to textures | Any material | The glTF exporter's material baking | Unverified, and baking may need a renderer, which `-nullrhi` lacks |
| Simple collision | `BodySetup` aggregate geometry | Counts in `EditorStaticMeshLibrary`; shapes may need C++ | Unverified |
| Landscape heights and layers | `ALandscapeProxy` | Heightmap and weightmap export | Unverified headless |
| Lights | Directional, point, spot, rect light components | Component properties | Unverified |
| Sound waves | `SoundWave` | `AssetExportTask` to WAV | Unverified |
| Skeletal meshes, clips | `SkeletalMesh`, `AnimSequence` | `EchoSourceSDK` commandlet (`metaverse-engine/tools/unreal-echo-sdk/`) | In repo |

## 3. The bundle

The Unreal side writes one folder. It is the only thing the Coax side
reads, and the Coax side never opens an Unreal file.

```
NAME.aimport/
  manifest.json        what the bundle is, where it came from, what it holds
  meshes/              one glTF binary per source static mesh
  textures/            one PNG per source texture, as Unreal exported it
  sounds/              one WAV per source sound wave
  landscape/           heights and layer weights per landscape component
  skipped.json         every actor and component not carried, with its class
  layout-delta.json    round trip only: generated actors moved by hand
```

### 3.1 `manifest.json`

Versioned (`"aimport": 1`). It reuses the exporter manifest's field names
wherever the meaning is the same (`game`, `axes`, `collision`,
`placements`, `materials`). Every position in it is in Unreal's own
units and axes, exactly as the editor reported it. **The Unreal side
converts nothing.** The Coax side converts once, in one place.

It contains:

- `source`: project path, map path, engine version string, the plan's
  hash (section 4.1), and the version-control revision of the project when
  the plan names one.
- `axes`: the conversion the Coax side will apply, stated so a reader can
  check it (section 3.2).
- `meshes`: per source mesh: its `/Game/` path, file, SHA-256, triangle and
  vertex counts as Unreal reports them, material slots, and simple
  collision.
- `placements`: per instance: mesh path, stable name (section 3.3),
  translation in centimetres, rotation as a quaternion, scale, actor label,
  component name, and tags.
- `materials`: per source material: path, parent path, texture parameters
  with their texture paths, scalar and vector parameters.
- `textures`, `sounds`: path, file, SHA-256, size.
- `landscape`: components, their origins, sample spacing, layer names.
- `lights`: kind, position, direction, colour, intensity and units,
  radius.
- `spots`: player starts, actors with an `aethyr.` tag, and, on a round
  trip, our own `AethyrMarker` actors by name.
- `provenance`: per asset, its licence class (section 9.2).

### 3.2 Axes and units

The exporter measured the mapping in its step 0 with three marker cubes:
Unreal X = x, Unreal Y = z, Unreal Z = y, and 100 units = 1 m. The
importer applies the inverse:

- Position: x = X / 100, y = Z / 100, z = Y / 100.
- Rotation: swapping two axes is a reflection, so it also turns Unreal's
  left-handed frame into the engine's right-handed one. A rotation
  quaternion (qx, qy, qz, qw) in Unreal becomes (−qx, −qz, −qy, qw).
- Scale: (sx, sy, sz) becomes (sx, sz, sy).

Splice Bots' own files state their frame as "X east, Y south, Z up"
(`graybox/farm.json`). The terrain tile profile states "+x east, +y up, +z
north". Under the mapping above, Unreal's south becomes the engine's +z.
Step 0 settles which way north points in an imported realm and writes it
into `axes.coax`, so a compass, a sun direction or a map reads correctly.

The quaternion rule is derived, not measured. Step 0 measures it with
rotated, non-uniformly scaled cubes, and the round trip in step 3 checks it
against every exported marker. If a measurement disagrees, the measurement
wins and this section is corrected.

glTF files from Unreal's exporter are already in glTF's frame (right-handed,
y up, metres). The Coax side reads them with the engine's own glTF reader
(`metaverse-engine/anim/gltf-access.coax`) and applies no second flip.
Step 0 confirms that a mesh and its placement agree.

### 3.3 Stable names

Every carried thing has a name that survives a round trip, as the
exporter's override table already requires.

- On a round trip, the exporter's names are used: `world.w_m00_x127_z127`,
  `kit.garage_bay`, `thing.raider`, marker names. The importer reads them
  from actor labels and the override table's row names.
- On a level import, a placement's name is its actor label and component
  name, plus its instance index for an instanced component. A mesh's name
  is its `/Game/` path. Names are fixed by the source, so a rerun gives the
  same names.
- Pack names are 1 to 24 bytes (`pip_name`, `pack-import-policy.coax`).
  The Coax side shortens a long name to a fixed form and keeps the full
  source path in the manifest beside it.

## 4. The tools

### 4.1 The plan

An import starts from a plan, a small JSON file in the profile
`world.unreal.source.v1`, modelled on the animation importer's
`anim.unreal.source.v1`. The Coax side admits it before any editor runs.

It names:

- the project file and the map;
- what to take: whole map, or a list of World Partition cells or bounds;
- the `/Game/` folders to read assets from. Any reference outside them is
  refused, not followed;
- the licence class of each folder (section 9.2);
- the profile: `round-trip` (names and checks of the exporter's project) or
  `level`.

### 4.2 Unreal side

A Python script for the editor, `metaverse-engine/import/unreal/export_level.py`.
Firm reason 6: the editor's asset API exists only in Python and C++.

- It runs headless with `UnrealEditor-Cmd` in an **isolated data-only
  project** that mounts the source project's `Content/`. It never loads the
  source project's own modules or plugins, as the animation importer and
  Valley do. The one exception is our own `AethyrBridge` plugin on a round
  trip, because the generated level's collision boxes and markers are its
  classes, and it is our code.
- It never saves a source package. The Coax side checks every source file's
  hash before and after the run, as the exporter's importer checks the
  files outside `Generated/`.
- It writes the bundle and nothing else, and it writes counts to the log:
  actors read, carried and skipped; meshes; instances; textures.
- What Python cannot read (likely simple collision shapes, possibly
  landscape weights) goes into a commandlet in `AethyrBridge`, which already
  holds "the editor checks Python cannot do". Step 0 decides which.

### 4.3 Coax side

| File | Role |
|---|---|
| `metaverse-engine/import/unreal/plan.coax` | Admits a `world.unreal.source.v1` plan. `#authority`, with refusal fixtures |
| `metaverse-engine/import/unreal/bundle.coax` | Reads `manifest.json` and checks every file's hash and count |
| `metaverse-engine/import/unreal/axes.coax` | The one conversion of section 3.2. `#authority`, with `#forall` checks that it inverts the exporter's mapping |
| `metaverse-engine/import/unreal/convert.coax` | Meshes into pack vertices, placements into cell rows, collision, terrain tiles, spots, lights |
| `metaverse-engine/tools/unreal-level-import.coax` | The command: admits the plan, runs the editor, reads the bundle, writes the pack |
| `metaverse/splice-bots/splice-import.coax` | Splice's round trip: maps override rows to Splice's kit slots and kinds |

The pack bytes are `metaverse-engine/editor/pack.coax`'s `pack_write`, and
the writer reads its own output back with `pack_read` before it reports
success, as the browser editor's import does. The editor run is the
animation importer's `au_sdk_run`: no stdout pipe, an owned process id, a
time limit.

```
coax/web/tool.sh metaverse-engine/tools/unreal-level-import.coax PLAN.unreal.json NEW_DIR
```

Rules for the tool:

- **Same source, same bytes.** The pack is built from parsed geometry
  sorted by stable name, never from Unreal's raw glTF bytes, because
  Unreal's exporter is not known to be deterministic. Two runs on the same
  content and engine version give the same pack hash.
- **Whole or refused.** A mesh over its limit, a texture over its side, a
  pack over its budget, a reference outside the plan's folders, a licence
  that does not fit the output: the import writes nothing and names the
  first refusal. This is the engine's rule everywhere (`budget.coax`,
  `assets.coax`), and the owner's: no hidden limits.
- **It refuses to write into an existing folder**, like both existing
  exporters.
- **Private by default.** Output goes under the owner's private asset root
  (`~/.openagents`, the animation importer's `anim_export_path` rule) unless
  every asset is in a public licence class (section 9.2).
- **Loud.** It ends with a table of counts: read, carried, skipped,
  refused. A count that does not match the manifest is a failure with a
  non-zero exit.

## 5. Conversion rules

### 5.1 Meshes

- Read source LOD 0. A Nanite mesh exports its source triangles, not its
  fallback: the exporter's step 1 found Unreal's 13,542-triangle fallback in
  place of the real mesh, and only the count check caught it. The importer
  compares the glTF's triangle count with the count Unreal reports and fails
  on a difference.
- Weld identical vertices and write an index buffer, into the 48-byte
  pack vertex: position, normal, UV, colour.
- A mesh stands where its source pivot is. Unlike the browser editor's
  import, it is not moved to the ground, because the placements depend on
  the pivot.
- A mesh over the policy's triangle limit is refused in the first version.
  Large meshes need the cluster path (`metaverse-engine/render/geometry/`,
  as Valley's clusters use), which steps 1 to 5 do not need.

### 5.2 Placements

- One row per instance, in the cell its position falls in (64 m cells,
  `metaverse-engine/world/cells.coax`), with the cell's content digest in
  the catalog's `Z` line.
- Instanced and foliage components keep their instancing: one mesh, many
  rows. A Blueprint actor's static mesh components are carried as plain
  placements. Its behaviour is listed in `skipped.json`.
- Hidden-in-game components are skipped and listed.

### 5.3 Collision

The owner's rule is that what a player can see they can walk to, and what
they walk into matches what they see. Unreal does not enforce that. The
importer does.

- Simple collision shapes come across as boxes, spheres, capsules and
  convex hulls, in the mesh's space.
- A mesh with no simple collision and "use complex as simple" gets a mesh
  collider through the existing mesh BVH (`metaverse/tools/mesh-bvh.coax`).
- A mesh with no collision at all is drawn and not collided. It is listed,
  so nobody mistakes a wall that lets players through for a working wall.
- A collider that reaches more than a set distance outside its visible
  mesh's bounds is reported. The first run sets the distance and it only
  tightens.
- `BlockingVolume` and other invisible blockers are refused by default and
  listed. A plan may admit one by name when a visible mesh stands inside it.

### 5.4 Materials and textures

- Textures are copied as PNG, then admitted as pack texture layers (colour,
  sRGB) or data layers (normal, ORM, linear, `#13012`), by how the material
  uses them. A texture whose use is unknown is refused, not guessed.
- A material instance becomes a material record: its textures and its
  parameters by name. A realm's shader decides what the names mean. The
  importer ships a mapping for the parameter names of the materials it is
  tested on, and lists every parameter it did not map.
- A material that is not an instance, or whose parent the mapping does not
  know, falls back to one of two things, chosen in step 0: a texture baked
  by Unreal's glTF exporter, or flat colour from its base colour parameter.
  Either way it is listed. A fallback is reported, never silent, as the
  exporter's importer fails on a fallback material.
- No Unreal shader or material graph is translated. Section 9.1 says why.

### 5.5 Landscape

- Heights become `aethyr.terrain.tile.v1` tiles. Unreal's landscape is in
  centimetres with its own height scale. The converter records the
  rescaling, as that profile requires, and refuses a tile it cannot express.
- Neighbouring tiles share their edge samples bit for bit
  (`terrain_tile_edges_agree`).
- Paint layers become material influences: at most four a sample, as the
  profile allows. A sample with more than four non-zero layers is refused,
  never trimmed. The plan may merge named layers first.
- The landscape's visibility layer becomes the profile's holes.

### 5.6 Spots and lights

- A player start becomes the realm's spawn spot. A level with none is
  refused: the exporter's audit lists a missing player start as one of the
  faults that made a player fall through the world.
- Actors tagged `aethyr.NAME` become named spots, so rules can be written
  against them. On a round trip the `AethyrMarker` actors come back under
  their own names.
- The strongest directional light becomes the key light direction. Point
  and spot lights come across as numbers, within the realm's light budget;
  lights over the budget are listed by brightness. Sky, fog and post
  process settings are recorded in the manifest and not applied.

### 5.7 Sound

- Sound waves are copied as WAV and admitted as pack sounds. Cues,
  MetaSounds and attenuation are listed, not carried.
- On a round trip, an `event.NAME` sound comes back tied to the same event
  name, so the prototype can play it when the rule fires.

## 6. What the first version does not do

- Translate Blueprints, Verse, C++ gameplay, StateTree, GAS or widgets into
  Coax.
- Translate material graphs or shaders.
- Carry Niagara, Sequencer, Chaos destruction, cloth, MetaSounds or
  animation blueprints.
- Carry large meshes through the cluster path.
- Import a game's interface or text.
- Import MetaHuman content, ever (section 9.2).
- Apply layout moves to a game's Coax source.
- Watch a project and import on save. An import is one command.

## 7. First target: the Splice Bots jam entry

Looked at on 2026-11-16: the Windows Shipping build submitted to the game
jam on 2026-11-15 (`OLD-SPLICEBOTS-Win64-Shipping-20261115-152753.zip`,
334 MB packed, 464 MB unpacked, 178 files), and the source project it was
built from, `~/games/SPLICEBOTS` (UE 5.9, a git repository).

### 7.1 The packaged build alone is not enough

The build's meshes, textures, materials and maps are cooked packages
inside IoStore containers: `SPLICEBOTS-Windows.ucas` (196 MB),
`.utoc`, `.pak` (11 MB) and `global.ucas`. The editor cannot open cooked
content, and reading those containers ourselves means parsing Epic's
package formats, which section 9.1 rules out. **Nothing is imported from
the cooked containers.**

### 7.2 What the build does give us

The build also ships its game data as loose files under
`SPLICEBOTS/Data/`. These are ours, in plain formats, and need no Unreal
to read:

| Files | What they hold | Becomes |
|---|---|---|
| `level_layout.csv` (727 rows) and `graybox/{farm,lab,plain,ring}.json` | Every box of four levels: id, kind (304 walls, 193 platforms, 162 markers, 24 bays, 23 ramps, 6 floors, 3 lights, plus others), centre and size in cm, yaw. Frame stated in the file: X east, Y south, Z up | Collision, named spots and graybox placements |
| `blockouts/*.obj` (64) and `blockouts.csv` | One 1 m box per asset id, origin at the feet | Stand-in meshes, keyed by the same asset ids as the real meshes |
| `tuning.csv`, `robots.csv`, `items.csv`, `cards.csv`, `card_triggers.csv`, `pickups.csv`, `effects.csv`, `horizon_giants.csv`, `sniper_lines.csv` | Tuning numbers with units and the system they belong to; robot, item and card tables | Data for the Coax rules. Many `tuning.csv` rows name the prototype's own Coax constant and line already |
| `strings.csv`, `messages.csv`, `ui_event_panels.csv` | Interface text | Interface strings |
| `input_bindings.csv`, `input/*.json` (7 contexts) | Key bindings | The realm's input table |
| `audio/wav/*.wav` (9), `sounds.csv`, `audio/*.csv` | Music beds, interface and jetpack sounds | Pack sounds |
| `icons/*.png`, `icons.csv` | Hotbar icons | Pack texture layers |
| `fonts/` | Michroma and IBM Plex Mono (OFL) | Already in the engine |

That is enough for a walkable graybox of all four levels, with the
game's numbers, text, keys and sounds, without starting Unreal.

### 7.3 What the source project adds

The real models and the final level layout are in the source project:

- `Assets/Source/` has the source files the cooked meshes were made from:
  110 glTF binaries and 23 FBX files (Digger and his legs, the Director and
  her clips, six raiders, and the Tripo and Blender things). The glTF files
  are read with the engine's glTF reader directly, with no Unreal at all.
  The FBX files are the Director's body and clips and the raiders' FBX
  twins; they go through the animation importer (step 6).
- The maps: `L_SolarFarm` (the game's default map),
  `L_SolarFarm_GreekQuarter`, and the graybox maps `L_Farm`, `L_Lab`,
  `L_Plain`, `L_Ring`, `L_Cabin`. Placements of the real meshes come from
  these, through the editor script of section 4.2.
- The source project matches the build's data closely but not exactly. 92
  of the build's 98 data files are byte-identical (ignoring line endings)
  to commit `70c5bad` ("Jam submission docs", 2026-11-15 11:16 CST). Six
  differ from every commit: `messages.csv`, `input_bindings.csv`,
  `input/Board.json`, `input/Play.json`, `audio/sfx.csv` and
  `icons/icons.csv`, and the build's nine WAVs are not committed. The build
  was made on the Windows machine with uncommitted edits. **The import pins
  `70c5bad` plus those files from the build**, and the plan records both
  hashes. Committing them to the project would remove the special case.
- The project has moved on since the jam (Tripo raiders, crafting, the
  Microduck). The import reads the pinned commit from a clean worktree of
  the project, never the working copy.

### 7.4 Licence classes in this project

From the project's own `Docs/jam/04-third-party.md`, applied to section 9.2:

| Content | Class | Import |
|---|---|---|
| Blender and script-made models, the Greek quarter, icons, music, chirps, our sounds, all `Data/` tables | Ours | Yes |
| Michroma, IBM Plex Mono | Open (OFL) | Yes |
| Tripo models (Director, Digger, raiders, mechs, parts) | Tripo's terms; the doc says to confirm the plan used | Private until confirmed |
| Her animation clips | Source not confirmed: Tripo presets or Epic samples | Private until confirmed |
| Mannequin skeleton, third-person animation set, Basic Shapes, `LevelPrototyping`, `ThirdPerson` | Epic Content | Refused for any public pack |

Until the founders decide, the whole Splice Bots import goes to the
private root (section 4.3), as Valley Lab's packs do, whatever its classes.

### 7.5 The round trip for Splice

What comes back from `~/UnrealProjects/SpliceExport/AethyrHost.uproject`
(or the project the founders choose):

| Override row | Comes back as | Used by the prototype |
|---|---|---|
| `kit.NAME` | A pack mesh, with its textures | The kit slot draws it in place of the blockout, at every place the slot stands (`kits.py` already lists them) |
| `thing.NAME`, `thing.NAME_vN` | A pack mesh | The thing draws it at the rules' position and yaw |
| `world.GROUP` | A pack mesh | Replaces that world group's triangles |
| `event.NAME` | A pack sound | Plays when the event fires |
| `thing.NAME` with a skeleton, `anim.` | The animation path (step 6) | Later |
| `fx.`, `trail.`, `state.`, `look.`, `material.` | Nothing | The prototype's stand-in stays |

And from `Generated/`:

- The level's static meshes, read back, must equal the exported world
  (step 3). This is the importer's test oracle, and it costs nothing to
  keep running.
- Any generated actor whose transform differs from the export goes to
  `layout-delta.json`: its exported name, the old and new positions in
  engine metres, and the collision box it maps to if any.

The prototype loads the round-trip pack only with `?pack=splice-unreal`.
The pack has its own budget, shown with the page's size, so the cost of
the designer's content is visible before anyone ships it.

## 8. Acceptance

Each step ends on a check a script runs. No step is done on a claim.

**Step 0. Prove the risks. One day.**

1. **Axes.** Export a fixture of cubes, rotated on three axes and scaled
   unevenly, from a new Unreal level. Import them and compare the
   positions, rotations and winding with the cubes' known values. Extend
   `metaverse-engine/tools/unreal-axes-fixture.coax` rather than writing a
   second fixture.
2. **Headless glTF export.** Export one static mesh with vertex colour, UVs
   and two material slots from `UnrealEditor-Cmd` 5.9 in an isolated
   project. Read it with the engine's glTF reader and compare counts and
   bounds with what Unreal reports. Try a Nanite mesh and confirm the source
   triangles come out.
3. **Material baking.** Try the glTF exporter's material baking headless. If
   it needs a renderer, decide between running without `-nullrhi` and the
   flat-colour fallback.
4. **Collision and landscape.** Read one mesh's simple collision shapes and
   one landscape component's heights and weights headless. Whatever Python
   cannot read goes into the `AethyrBridge` commandlet.

The animation importer's default editor path is
`/Users/Shared/Epic Games/UE_5.8/Engine/Binaries/Mac/UnrealEditor-Cmd`.
5.8 was uninstalled on 2026-11-11 ([build-from-source.md](build-from-source.md)).
Step 0 points both importers at the 5.9 source build, through
`OPENAGENTS_UNREAL_EDITOR`, and changes the default.

**Step 1. The Splice Bots jam entry, from its data. No Unreal.**

- A plan names the build's `Data/` folder and `70c5bad` of the source
  project. A Coax reader admits `level_layout.csv`, the four graybox
  files, the blockouts and the tables, and refuses a row it cannot read,
  naming the file and line.
- The four levels come out as one private pack and realm each: every
  `wall`, `platform`, `ramp`, `floor` and `bay` box as collision with its
  blockout drawn on it, every `marker` as a named spot. Counts match the
  CSV per level and per kind.
- Every box's centre and size come back within 0.1 mm of the CSV after the
  axis change, and a yawed box's corners match.
- The route walk test walks each level and finds no invisible blocker.
- A headless shot of each level from its player start is taken and kept as
  the step's reference.
- The nine WAVs, the icons, strings and key bindings are in the pack, and
  their counts match the CSVs.

**Step 2. The Splice Bots jam entry, with its real models and maps.**

- The glTF files in `Assets/Source/` at `70c5bad` come in as pack meshes
  without Unreal, by asset id.
- The editor script reads `L_SolarFarm`, `L_SolarFarm_GreekQuarter` and the
  graybox maps at that commit from a clean worktree, and their placements
  land on those meshes.
- Every placement whose mesh is Epic Content is listed and refused;
  nothing is drawn in its place silently.
- Headless shots from three cameras agree with the editor's shots from
  the same cameras in silhouette, at least 98 percent.
- The game is walkable from her start to the far edge of the solar farm.

**Step 3. Round trip of the generated world.**

- Export Splice, import it into Unreal with the existing importer, then
  import it back with this one.
- The world read back has the exporter's counts: 58 meshes, 22,798
  triangles, 30 collision boxes, 50 markers.
- Every vertex position is within 0.1 mm of `world.glb`, and every vertex
  colour is equal.
- Every marker comes back at its exported place within 0.1 mm, under its
  own name.
- Two imports give the same pack hash.
- Move one generated wall by hand in Unreal. The next import lists it in
  `layout-delta.json` with its name and the move, and changes nothing else.
- No source package's hash changes.

**Step 4. Round trip of a designer's content.**

- Use the stand-in designer (`metaverse-engine/export/unreal/probe/make_overrides.py`)
  and add one textured `kit.` mesh and one `world.` mesh.
- The import carries every `kit.`, `thing.`, `world.` and `event.` row, and
  lists every other row by name in `skipped.json`.
- `?pack=splice-unreal` draws the replaced kit slot and thing in the
  prototype. A headless shot (`coax/web/tool.sh shot`) and the Unreal
  editor's shot from the same camera agree within a colour difference the
  first run sets. The number only tightens.
- The event's sound plays in the prototype when the event fires. A
  headless browser has no audio output, so the check is that the page
  starts the sound, as the exporter's step 3 checks `PlaySound2D`.
- The pack's size is reported beside the page's 443 KB.

**Step 5. A level built in Unreal for the test.**

- Build a small level in Unreal for the purpose, from content we made: a
  landscape with two paint layers and a hole, a few meshes with simple
  collision, one foliage type, a Blueprint actor with a mesh, a blocking
  volume, three lights and a player start.
- The import's counts match the manifest. The Blueprint actor's mesh is
  placed and its behaviour is in `skipped.json`. The blocking volume is
  refused by default.
- Terrain heights sampled at a grid of points equal Unreal's landscape
  heights within 1 mm.
- A route walk test walks every path in the level with the shared walker
  and finds no invisible blocker and no gap where a visible wall should
  stop the player.
- Headless shots from three cameras agree with the Unreal editor's shots in
  silhouette, at least 98 percent, as the exporter's step 1 required. Colour
  is not compared in this step: the look is the realm's.
- A one-byte-over-budget pack, a reference outside the plan's folders and
  an asset in a refused licence class each refuse the whole import.

**Step 6. Characters.**

- A skeletal `thing.` row on the round trip, and a skeletal mesh on a level
  import, go through the animation importer's plan
  (`anim.unreal.source.v1`) and come back as a rig and clips.
- The plan for the level writes the animation plan, so it is one command.

## 9. Licensing

### 9.1 Code

The rule in [the learning note](2026-11-11-learning-from-unreal-5-9.md),
section 5, stands: ideas only, never Epic's code.

- The importer reads data that the editor writes through its public API:
  glTF (a Khronos format), PNG, WAV and our JSON. It never parses Unreal's
  package formats itself, and never reads engine source at run time.
- The Python script and any `AethyrBridge` commandlet call Unreal's public
  API from inside Unreal. That is ordinary licensee use, like the
  animation SDK and the exporter's importer.
- No material graph, shader, constant table or file format of Epic's is
  translated. A material comes across as the textures and numbers the
  creator set, not the code that shades them.

### 9.2 Content

The engine is not Unreal, and the EULA limits Epic Content to Unreal-based
products ([web-games-in-the-verse.md](web-games-in-the-verse.md),
section 7). So the content rules are stricter than the code rules. Every
asset in a bundle has one licence class, set by the plan per folder and
checked by path:

| Class | Examples | What the importer does |
|---|---|---|
| Ours | Splice's generated content, our designers' overrides, a level we build | Public pack, under a licence from the policy list |
| Open | CC0, CC BY, CC BY-SA, the licences `pack-import-policy.coax` admits | Public pack, with the licence recorded per asset |
| Fab Standard License | Assets bought on Fab | **Private only** until the owner rules (section 10). Fab's licence allows any engine, but the policy list does not hold it |
| Epic Content | Anything under `/Engine/`, starter content, the Mannequin, `LevelPrototyping`, Epic samples such as Valley of the Ancient, Megascans claimed under the old Unreal-only licence | **Refused for a public pack.** Admitted only to the private root as reference, as Valley Lab's packs are, and never shipped |
| MetaHuman | `/Game/MetaHumans/` and any MetaHuman asset | **Refused always**, private or not ([metahuman-support.md](../metaverse/metahuman-support.md)) |

And for every class:

- Imported Unreal content stays out of every training, Gym or dataset
  pipeline. The EULA forbids using Licensed Technology as AI training
  input, and the owner's MetaHuman rule forbids it for MetaHumans.
- An asset whose class the plan does not set is refused. Nothing is
  assumed to be ours.
- A public pack must still pass the licence check in
  `metaverse-engine/editor/pack.coax`.

## 10. Owner decisions

1. **Does content come back to the prototype at all?** The round trip in
   section 7 lets the prototype page show a designer's meshes and sounds
   behind `?pack=`. [What belongs in Unreal](2026-11-14-what-belongs-in-unreal.md)
   expects the two builds to stop looking alike. This spec recommends yes,
   opt-in, so playtests of the prototype can show final art when someone
   wants them to.
2. **Fab assets in public packs.** Fab's Standard License allows use in any
   engine. Adding it to the policy list makes bought assets shippable in
   realms. This spec recommends counsel reads the licence first, as the web
   games doc already asks for the EULA (decision O7 there).
3. **Who may run a level import against a creator's project?** Only the
   owner's Mac today. A maker-facing import, from a creator's uploaded
   project, needs the guest terms of [web-games-in-the-verse.md](web-games-in-the-verse.md)
   first.

4. **Splice Bots' unconfirmed licences.** The Tripo plan and the source of
   her animation clips decide whether those models and clips can ever leave
   the private root (section 7.4). Until then the whole import stays private.
5. **The six uncommitted jam files.** Commit them to the Splice Bots project
   so the import can pin one commit instead of a commit plus files from the
   build (section 7.3).

## 11. Risks and open questions

| # | Item | Answer today |
|---|---|---|
| R1 | The quaternion rule in section 3.2 is derived, not measured | Step 0 measures it |
| R2 | Headless material baking may need a renderer | Step 0 decides; flat colour is the fallback, and it is always listed |
| R3 | Simple collision shapes may not be readable from Python | A commandlet in `AethyrBridge`, which already exists for this kind of gap |
| R4 | Unreal's glTF export may not be deterministic | The pack is built from parsed, sorted geometry, so the pack hash is stable either way |
| R5 | Large levels exceed the pack limits | Refused whole in the first version. The cluster path and paged cells, which Valley already uses, are the next step |
| R6 | UE6 changes the editor's asset API | The Unreal side is one script. Epic says MCP and Python are UE6's content path ([ue6.md](ue6.md)), so the script should survive with edits |
| R7 | A designer expects layout changes in Unreal to change the game | `layout-delta.json` reports them every import; the rules' layout is still edited in Coax |
