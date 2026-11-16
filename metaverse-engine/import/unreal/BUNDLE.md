# The import bundle, version 1

The contract between the two halves of the Unreal importer
([the spec](../../../docs/unreal/2026-11-16-unreal-to-metaverse-engine-importer-spec.md)).

Two producers write it:

- **The editor route**: `export_level.py` running inside a UE 5.9 editor
  on a source project.
- **The cooked route**: the cooked-package reader, from packages that
  `UnrealPak IoStore -FindAndExtract` unpacked from a shipped build.
- **The data route**: a game's own producer over the loose data a build
  ships beside its packages (CSV and JSON tables, WAVs, icons). It carries
  only what that data states, so it may have tables (section 3.7) and no
  Unreal packages at all.

One consumer reads it: the Coax importer (`metaverse-engine/import/unreal/`),
which writes the pack. The consumer does not know or care which route made
a bundle, and a bundle from each route for the same build must agree
(`bundle-diff`, section 6).

## 1. Rules

- **Raw values only.** Every number is what Unreal reports, in Unreal's
  frame: centimetres, X forward, Y right, Z up, left-handed. Rotations are
  quaternions `[x, y, z, w]` in that frame. Producers convert nothing. The
  consumer converts once (`axes.coax`).
- **Mesh files are glTF 2.0 binary.** The editor route writes them with
  Unreal's glTF exporter, so their vertices are in glTF's frame (metres,
  y up, right-handed). The cooked route writes them in the same frame. The
  manifest's `bounds_cm` are in Unreal's mesh-local frame, so the consumer
  can check the two against each other.
- **Ids are Unreal object paths**, for example
  `/Game/SpliceBots/Things/SM_Garage.SM_Garage`. An id is unique within its
  list and is how every list refers to every other. A data-route id is the
  file's path under the build's data folder, without its extension, for
  example `/Data/audio/wav/ui-move`; a project configuration file's is its
  path under the project, for example `/Config/DefaultSpliceTuning`.
- **Every file is listed** in `files` with its SHA-256 and size. A file on
  disk that is not listed, or listed and missing, fails the bundle.
- **Sorted.** Every list is sorted by id (placements by map, then name), so
  the same content gives the same manifest bytes.
- **Nothing is dropped silently.** Anything a producer reads and does not
  carry goes into `skipped.json` with a reason.

## 2. Folder

```
NAME.aimport/
  manifest.json
  meshes/       NNNN.glb          static meshes, LOD 0
  skeletal/     NNNN.glb          skeletal meshes with their skin
  anims/        NNNN.glb          one clip each, on its skeleton
  textures/     NNNN.png          8-bit PNG; 16-bit for height or HDR sources
  sounds/       NNNN.wav          PCM
  landscape/    MAP-NNNN.r16      heights, little-endian u16, plus .json beside
  tables/       NNNN.csv|json|ini a data route's tables, byte for byte
  skipped.json
```

File names are numbers, not asset names, so a long or odd asset name never
reaches the file system. The manifest maps ids to files.

## 3. `manifest.json`

```json
{
  "aimport": 1,
  "source": {
    "route": "editor",
    "project": "SPLICEBOTS",
    "revision": "70c5badcd169ed2c2eb18774dc91da78e4e7129f",
    "engine": "5.9.0",
    "maps": ["/Game/SpliceBots/World/Maps/L_SolarFarm"],
    "plan_sha256": "..."
  },
  "files": [ { "path": "meshes/0001.glb", "sha256": "...", "bytes": 12345 } ],
  "meshes": [],
  "skeletal_meshes": [],
  "skeletons": [],
  "animations": [],
  "materials": [],
  "textures": [],
  "sounds": [],
  "maps": []
}
```

`source.route` is `editor`, `cooked` or `data`. `tables` (section 3.7) is
optional; a bundle without it has none. `revision` is the project's version
control revision, or `null` when unknown (a cooked build).

### 3.1 `meshes[]`

```json
{
  "id": "/Game/.../SM_Garage.SM_Garage",
  "file": "meshes/0001.glb",
  "triangles": 5120,
  "vertices": 3311,
  "bounds_cm": [-500, -400, 0, 500, 400, 620],
  "nanite": false,
  "materials": ["/Game/.../MI_Garage.MI_Garage"],
  "collision": {
    "complex_as_simple": false,
    "boxes":    [ { "center_cm": [0, 0, 310], "rotation": [0, 0, 0, 1], "extent_cm": [500, 400, 310] } ],
    "spheres":  [ { "center_cm": [0, 0, 0], "radius_cm": 50 } ],
    "capsules": [ { "center_cm": [0, 0, 0], "rotation": [0, 0, 0, 1], "radius_cm": 30, "length_cm": 120 } ],
    "convex":   [ { "vertices_cm": [[0, 0, 0], [10, 0, 0], [0, 10, 0], [0, 0, 10]] } ]
  },
  "origin": "project"
}
```

- `triangles` and `vertices` are LOD 0 as Unreal counts them. For a Nanite
  mesh they are the source triangles, and the glTF must hold those, not
  the fallback.
- `materials` is one id per material slot, in slot order. The glTF's
  primitives use the same order.
- Collision lengths are full lengths; `extent_cm` is half size, as Unreal
  stores it. `length_cm` is the capsule's cylinder length, without its
  caps.
- `origin` is `project` (made for this game) or `engine` (under `/Engine/`
  or a template folder: `Characters/Mannequins`, `LevelPrototyping`,
  `ThirdPerson`, `StarterContent`). The consumer replaces `engine` assets
  with the engine's own (spec, section 9.2).

### 3.2 `skeletal_meshes[]`, `skeletons[]`, `animations[]`

```json
{ "id": "/Game/.../SK_Raider_1.SK_Raider_1", "file": "skeletal/0001.glb",
  "skeleton": "/Game/.../SKEL_Raider.SKEL_Raider", "triangles": 9000, "vertices": 6000,
  "bounds_cm": [...], "materials": ["..."], "origin": "project" }

{ "id": "/Game/.../SKEL_Raider.SKEL_Raider", "origin": "project",
  "bones": [ { "name": "root", "parent": -1, "rest_translation_cm": [0, 0, 0], "rest_rotation": [0, 0, 0, 1], "rest_scale": [1, 1, 1] } ] }

{ "id": "/Game/.../A_Raider_Walk.A_Raider_Walk", "file": "anims/0001.glb",
  "skeleton": "/Game/.../SKEL_Raider.SKEL_Raider", "frames": 33, "rate": 30, "seconds": 1.1, "origin": "project" }
```

A skeleton whose bones are the UE Mannequin's has `origin: engine`, and the
consumer retargets its meshes and clips onto the engine's skeleton.
That consumer is `ur_rig_bundle` in [rig-io.coax](rig-io.coax) (#13651; CLI
`coax/web/tool.sh metaverse-engine/tools/unreal-rig.coax bundle BUNDLE MESH_ID NAME OUT_DIR`).
It reads the mesh's `origin: project` clips as the character's own and never reads an
`origin: engine` clip: the six base actions (idle, walk, jog, jump start, loop and land)
come from the engine's own clips instead.

### 3.3 `materials[]`

```json
{
  "id": "/Game/.../MI_Garage.MI_Garage",
  "parent": "/Game/.../M_Toon.M_Toon",
  "root": "/Game/.../M_Toon.M_Toon",
  "blend": "opaque",
  "two_sided": false,
  "shading": "default_lit",
  "textures": [ { "name": "BaseColor", "texture": "/Game/.../T_Garage_D.T_Garage_D" } ],
  "scalars":  [ { "name": "Roughness", "value": 0.7 } ],
  "vectors":  [ { "name": "Tint", "value": [1, 0.5, 0.2, 1] } ],
  "baked":    { "base_color": "/Game/.../T_Garage_D.T_Garage_D" },
  "origin": "project"
}
```

- `blend`: `opaque`, `masked`, `translucent`, `additive`.
- `shading`: `default_lit`, `unlit`, `subsurface`, `other`.
- Parameters are the instance's values after inheritance, so a reader
  needs no parent chain. A texture parameter with no texture set is left
  out.
- A root material names itself as `parent` and `root`. An instance whose
  parent is a plugin's content (outside `/Game/` and `/Engine/`, for
  example `/InterchangeAssets/`) is carried flattened: it names itself as
  `parent` and `root`, its parameters as above, and its own bake. The
  plugin's material gets a `skipped.json` row (`unsupported`).
- `baked` maps `base_color`, `normal`, `orm`, `emissive` to texture ids the
  producer baked, when it baked any. Baked textures appear in `textures[]`
  like any other, with `"baked_from"` naming the material.

### 3.4 `textures[]`

```json
{ "id": "/Game/.../T_Garage_D.T_Garage_D", "file": "textures/0001.png",
  "width": 1024, "height": 1024, "srgb": true, "compression": "default", "origin": "project" }
```

`compression` is Unreal's setting in lower case without its prefix:
`default`, `normalmap`, `masks`, `grayscale`, `hdr`, `alpha`, `displacementmap`,
`vectordisplacementmap`, `bc7`, `ui` (`userinterface2d`), `other`. The consumer
picks colour or data layer from it and from how materials use the texture.

### 3.5 `sounds[]`

```json
{ "id": "/Game/Audio/SFX_Hover.SFX_Hover", "file": "sounds/0001.wav",
  "seconds": 1.5, "channels": 1, "rate": 48000, "origin": "project" }
```

### 3.6 `maps[]`

```json
{
  "id": "/Game/SpliceBots/World/Maps/L_SolarFarm",
  "placements": [
    { "name": "Garage_2/StaticMeshComponent0", "actor": "Garage_2", "actor_class": "/Script/Engine.StaticMeshActor",
      "component": "StaticMeshComponent0", "instance": -1, "kind": "static",
      "asset": "/Game/.../SM_Garage.SM_Garage",
      "translation_cm": [100, 200, 0], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1],
      "materials": ["/Game/.../MI_Garage.MI_Garage"],
      "collision": "block", "visible": true, "cast_shadow": true, "tags": [] }
  ],
  "lights": [
    { "name": "Sun", "kind": "directional", "translation_cm": [0, 0, 0], "rotation": [0, 0, 0, 1],
      "color": [1, 0.95, 0.9], "intensity": 10, "units": "lux", "radius_cm": 0,
      "inner_deg": 0, "outer_deg": 0, "cast_shadows": true, "movable": "stationary" }
  ],
  "spots": [
    { "name": "PlayerStart", "kind": "player_start", "translation_cm": [0, 0, 92], "rotation": [0, 0, 0, 1], "tags": [] }
  ],
  "volumes": [
    { "name": "BlockingVolume_1", "class": "/Script/Engine.BlockingVolume",
      "translation_cm": [0, 0, 0], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1],
      "boxes": [ { "center_cm": [0, 0, 0], "rotation": [0, 0, 0, 1], "extent_cm": [100, 100, 100] } ] }
  ],
  "landscapes": [
    { "name": "Landscape", "translation_cm": [0, 0, 0], "rotation": [0, 0, 0, 1], "scale": [100, 100, 100],
      "components": [ { "file": "landscape/L_SolarFarm-0001.r16", "x": 0, "y": 0, "size": 127 } ],
      "layers": [ { "name": "Sand", "file": "landscape/L_SolarFarm-0001-Sand.png" } ],
      "material": "/Game/.../M_Landscape.M_Landscape" }
  ],
  "atmosphere": { "fog": {}, "sky": {}, "post": {} },
  "streamed": ["/Game/.../LI_Bay"]
}
```

- **Placements.** One row per static or skeletal mesh component, and one
  per instance of an instanced or foliage component (`instance` is its
  index; `-1` for a plain component). `name` is `actor/component`, plus
  `#instance` for an instance, and is unique in its map. Transforms are
  world transforms.
- `kind`: `static` or `skeletal`. `asset` is a mesh or skeletal mesh id.
- `materials`: the component's materials after overrides, one per slot.
- `collision`: `block`, `overlap`, `none`, from the component's collision
  setting as it behaves for a walking player.
- Actors from Blueprint classes are carried through their components. What
  the Blueprint does is not, and the actor gets one `skipped.json` row with
  reason `logic`.
- `lights.kind`: `directional`, `point`, `spot`, `rect`, `sky`. `units` is
  Unreal's: `lux` for directional, else `candelas`, `lumens` or `unitless`.
- `spots.kind`: `player_start`, `tag` (any actor tagged `aethyr.NAME`, named
  `NAME`), `target_point`.
- `volumes`: blocking and trigger volumes with their brush as boxes, so the
  consumer can apply the invisible-blocker rule.
- `landscapes`: heights per component, in Unreal's raw u16 (0 to 65535,
  32768 is zero; height in cm = (raw − 32768) × scale.z / 128). Layers are
  8-bit weight maps the size of the component.
- `atmosphere`: the numbers of the sky light, sky atmosphere, height fog and
  the unbound post process volume, by Unreal's property name. Recorded,
  not applied. A producer may add keys beside `fog`, `sky` and `post`; the
  editor route adds `all` (one `{kind, actor, values}` row per such actor,
  when a map has more than one, as in a map with day, dusk and night data
  layers) and `light_data_layers` (light name to its data layer paths).
- Data layers: a placement's or spot's actor in a data layer carries the
  tag `datalayer:PATH` for each, after its own tags.
- Editor-only components (sprites, arrows, billboards) are not listed. A
  hidden component that collides is listed with `visible: false`, so the
  invisible-blocker rule can see it; one that doesn't collide gets a
  `hidden` row in `skipped.json`.
- `streamed`: sublevels and level instances the map loads. Their content is
  in the same map's lists, already at world transforms; this list only
  names where it came from.

### 3.7 `tables[]`

```json
{ "id": "/Data/input_bindings", "file": "tables/0018.csv", "format": "csv",
  "rows": 157, "origin": "project" }
```

A table the game reads at run time, carried byte for byte. `format` is `csv`,
`json` or `ini`; `rows` is a CSV's records after its header (quoted fields may
hold line breaks), 1 for a JSON file, or an ini file's `key=value` lines (not
its `[section]`, `;` or `#` comment, or blank lines). An ini table's id is its
path under the project, for example `/Config/DefaultSpliceTuning`. The consumer carries each one line
for line in the pack's catalog.

## 4. `skipped.json`

```json
[ { "map": "/Game/.../L_SolarFarm", "actor": "BP_Rover_2", "class": "/Game/.../BP_Rover.BP_Rover_C",
    "component": "", "reason": "logic" } ]
```

Reasons: `logic` (Blueprint or C++ behaviour), `niagara`, `sequencer`,
`audio_graph` (cues, MetaSounds), `widget`, `decal`, `hidden`, `unsupported`
(anything else, with `detail` saying what).

A row about an asset rather than a placed actor (a MetaSound, a Niagara
system or a Blueprint class the project holds, a material that did not
bake) has `map`, `actor` and `component` empty, and ends its `detail` with
the asset's id.

## 5. Producers

| Route | Code | Runs |
|---|---|---|
| Editor | `metaverse-engine/import/unreal/export_level.py`, driven by `metaverse-engine/tools/unreal-level-export.coax PLAN.json PROJECT.uproject ENGINE_DIR NEW_DIR.aimport` | Headless `UnrealEditor-Cmd` (with a renderer, for material baking) on the source project, from a clean worktree at the plan's revision; the driver admits the bundle with `bundle.coax` before it reports success |
| Cooked | `metaverse-engine/import/unreal/cooked/` | Over the folder `UnrealPak IoStore -FindAndExtract=BUILD.utoc -PackageFilter=* -OutPath=DIR` writes |
| Data | a game's own, for example `metaverse/splice-bots/tools/jam-data-bundle.coax` | Over a build's loose data folder, with the plan; where the build's copy of a file differs from the revision's, the build's wins and the plan pins its SHA-256 |

## 6. Checks

- `bundle.coax` admits a bundle: version, every file's hash and size, every
  id reference resolves, lists sorted, counts in range. It refuses whole
  and names the first fault.
- `bundle-diff` compares two bundles of the same build (editor and cooked)
  asset by asset: counts, bounds within 0.01 cm, transforms within
  0.01 cm and 1e-5 in each quaternion component, texture sizes, and the
  same `skipped.json` reasons. It is the cooked reader's answer key.
