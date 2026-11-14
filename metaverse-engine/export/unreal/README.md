# The Metaverse Engine to Unreal

The Unreal side of the exporter. The design is
[docs/unreal/2026-11-12-aethyr-to-unreal-exporter-spec.md](../../../docs/unreal/2026-11-12-aethyr-to-unreal-exporter-spec.md).

## Export, check, import

Run from the repository root.

```sh
# 1. Export Splice Bots. The folder must not exist. Same source, same bytes.
coax/web/tool.sh metaverse/splice-bots/splice-export.coax /tmp/splice.aexport

# 2. Read the bundle back with Aethyr's own glTF reader.
coax/web/tool.sh metaverse-engine/tools/unreal-export-check.coax /tmp/splice.aexport

# 3. Import into an Unreal project (Mac). Exit 0 means every check passed
#    and the game itself played the bundle's scripted run.
coax/web/tool.sh metaverse-engine/tools/unreal-import.coax /tmp/splice.aexport \
  ~/UnrealProjects/SpliceExport/AethyrHost.uproject ~/UnrealEngine-5.9/src

# 4. Play it (Mac, from the editor's binaries).
~/UnrealEngine-5.9/src/Engine/Binaries/Mac/UnrealEditor ~/UnrealProjects/SpliceExport/AethyrHost.uproject \
  /Game/Aethyr/Splice/Generated/L_Splice -game -windowed -ResX=1280 -ResY=800

# 5. Package for the web and run a browser fixture.
(cd ~/UnrealEngine-5.9/src && WASM_EMSDK_ROOT=~/emsdk-ue ./Engine/Build/BatchFiles/RunUAT.sh BuildCookRun \
  -project=$HOME/UnrealProjects/SpliceExport/AethyrHost.uproject -platform=Wasm -clientconfig=Development \
  -build -cook -stage -pak -iostore -package -archive \
  -archivedirectory=$HOME/UnrealProjects/SpliceExport/Saved/WasmOut -nocompileeditor -unattended -utf8output)
(cd ~/UnrealEngine-5.9/src/Engine/Platforms/Wasm/Build/Tools/web && ../../../../../Binaries/ThirdParty/Python3/Mac/bin/python3 webrun.py run \
  --stage ~/UnrealProjects/SpliceExport/Saved/WasmOut --fixtures $OLDPWD/metaverse-engine/export/unreal/web-fixtures.json \
  --fixture aethyr-play --headless --query tier=forward --out /tmp/aethyr-web)
grep AETHYR_ /tmp/aethyr-web/console.log
```

The game takes these switches, in the editor's game and (as `--args` or a
fixture's `args`) in the web build:

| Switch | What it does |
|---|---|
| `-AethyrScript` | Runs the bundle's script on its own state and prints the hash lines |
| `-AethyrPlay` `-AethyrPlaySpeed=N` | Plays the script on screen, N frames a tick |
| `-AethyrStage=N` | Starts at a moment of the run (the game's stages) |
| `-AethyrNativeThings` | Unreal draws every kind of thing from its exported model |
| `-AethyrGray` | Starts in graybox |
| `-AethyrShot=FILE` `-AethyrShotAfter=N` | Saves a screenshot after N frames and quits |
| `-AethyrNoInput` | Ignores keys and the mouse |
| `-AethyrWorld=URL` | The page's address: the world follows its saved edit layer from that site, and its placed pieces are drawn |
| `-AethyrExit` | Quits when a script ends |

Step 3 makes the project when it is named `AethyrHost.uproject` and does not
exist. It copies the plugin in, builds the editor modules, and runs
`import.py` in a headless editor. A first run on a new project compiles
engine shaders for several minutes; later runs take about a minute.

## What is here

| Path | What it is |
|---|---|
| `aethyr_host.h` | The host interface with real pointers, for the hosted rules (step 2) |
| `AethyrBridge/` | The Unreal plugin: manifest asset, collision box, marker, and the editor checks Python cannot do |
| `project/` | A bare host project that receives an export |
| `import.py` | The importer and its checks |
| `kits.py` | Which kit slots a `kit.NAME` row replaces, where, and which groups it drops |
| `aethyr_script.c` | Runs a script against a world through the host interface; the bridge compiles the same file |
| `web-fixtures.json` | Browser runs for the engine's web harness |
| `probe/make_overrides.py` | Stands in for a designer: a replacement model and a sound |
| `probe/` | Step 0: the frame-hash probe and the axis fixture |

## What the importer builds

Everything goes under `/Game/Aethyr/<Game>/Generated/`, which is deleted and
rebuilt each time. Nothing else in the project is touched; the importer
compares every other content file before and after.

- `world/StaticMeshes/`: one static mesh per world group, Nanite off.
- `things/StaticMeshes/`: one mesh per kind and variant of moving thing.
- `Materials/`: `MPC_Aethyr` (the world's uniform block, copied in each
  frame), `M_AeCel` and `M_AeSky` (the look), `M_AeInk` (silhouette lines,
  a post-process pass), and `M_AeVertexColour` (plain, for the reference
  shots).
- At run time, the world's instanced draws (`aethyr_instanced`: a mesh written once in its own
  space, and rows that place it) are procedural mesh components, one an instance, moved to their
  rows each frame. A component's mesh is written again only when its store changes.
- `L_<Game>`: the meshes, one `AethyrCollisionBox` per collision box, one
  `AethyrMarker` per placement, the `AethyrGame` host actor, the ink
  volume, and `AethyrGameMode` as the level's game mode.
- `DA_<Game>Manifest`: the bundle's hash and every tuning number.
- `Fonts/`: the two font faces.

## Replacing things

`/Game/<Game>/Overrides/` is yours. The importer makes an empty
`DT_AethyrOverrides` there the first time and never writes it again. Each
row's name says what is replaced and its Asset is the replacement:

| Row name | Replaces | With |
|---|---|---|
| `thing.raider` | Every raider, drawn by Unreal at the world's place and yaw | A static mesh |
| `thing.raider_v1` | One variant (here, a raider in a fight) | A static mesh |
| `world.w_m00_x127_z127` | One world mesh group | A static mesh |
| `kit.garage_bay` | A kit slot's grey blockout, at every place the slot stands | A static mesh |
| `material.cel`, `material.sky` | The look | A material |
| `event.pickup` | Nothing: adds a sound to the event | A sound |

Kinds and events are listed in the bundle's `manifest.json` (`things`,
`events`). Layout is Aethyr's: the import report lists generated actors that
were moved by hand, then rebuilds them.

A kit slot (`kits` in the manifest) lists its places and the world groups
that hold its blockout (named `kTT_...`, the slot's triangles grouped apart).
For a `kit.NAME` row the importer drops those groups and stands the row's
mesh at each place, in the level's `Kits` folder, after the reference shots
(they are of the world as exported). Build the mesh in the slot's own space:
its origin is the place, and its +Y is the slot's +z (into a garage bay, to
its back wall). A place with `open` above 0 is a bay whose back wall has a
way through that wide each side of its middle; leave it open. `kits.py` is
the plan, checked without Unreal: `python3 -I
metaverse-engine/export/unreal/kits.py BUNDLE/manifest.json`. The report's
`kits` lists the slots replaced and, with the reason, those still drawing
the blockout.

## What the importer checks

The report is `Saved/aethyr-import-report.json` in the project.

- The bundle's hash, and mesh, triangle, box, marker, font and tuning counts
  against the manifest.
- Every rendered vertex position and colour, and every mesh's bounds,
  against `world.glb`.
- The material's compile errors, read from Unreal, not from a log.
- Each reference shot against a render of the same camera: at least 98
  percent of pixels agree on covered or empty, and the mean colour
  difference over covered pixels is at most 6 of 255.
- That a second import gives the same report.

## Things to know

- Unreal's glTF import turns Nanite on. A Nanite mesh renders a reduced
  fallback where Nanite is unsupported, so the importer turns it off.
- Unreal stores glTF vertex colours as linear bytes. The material reads them
  as they are.
- Building a project's editor target the first time relinks one engine
  library (NetCore) and re-signs the shared `UnrealEditor.app`. Another
  editor from the same engine keeps running through it.
- Saving the level logs an "Unable to Check Out From Revision Control"
  message in a headless run. The level is saved.
- The reference shots are drawn in software from the same vertex stream the
  page draws (`metaverse-engine/export/raster.coax`). They check position and colour,
  not the anime shading.
