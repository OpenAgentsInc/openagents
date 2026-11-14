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
| `-AethyrThingBeside=CM` | A kind an override row replaces is still drawn by the world, the replacement that far to its side |
| `-AethyrGray` | Starts in graybox |
| `-AethyrShot=FILE` `-AethyrShotAfter=N` | Saves a screenshot after N frames and quits. Keys and the mouse are ignored, as with `-AethyrNoInput`, so the desktop's mouse cannot turn an offscreen still |
| `-AethyrNoInput` | Ignores keys and the mouse |
| `-AethyrPlayFile=FILE` | Plays a script from a file on screen, as `-AethyrPlay` plays the bundle's: `key`, `msg` and `frames` lines |
| `-AethyrShots=DIR` `-AethyrShotEvery=N` | Saves `DIR/f00001.png`, ... every N frames (default 2): a film's frames |
| `-AethyrWorld=URL` | The page's address: the world follows its saved edit layer from that site, and its placed pieces are drawn |
| `-AethyrExit` | Quits when a script ends |
| `-AethyrRecord=FILE` | Writes the run's events and things to a file (see "Tune an effect without a rebuild") |
| `-AethyrReplay=FILE` | Plays a recorded file's events and things in place of the world's own |
| `-AethyrFxScale=N` | Every effect N times its size |

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
| `look.py`, `look_source.py` | A starter look in the project's overrides, and the shader source it shares with the importer |
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
| `thing.smuggler` | Every variant of a kind, when the asset has a skeleton | A skeletal mesh (see "A thing with a skeleton") |
| `anim.smuggler.run`, `anim.smuggler_v3` | Nothing: the clips a skeletal `thing.` row plays | An animation sequence |
| `world.w_m00_x127_z127` | One world mesh group | A static mesh |
| `kit.garage_bay` | A kit slot's grey blockout, at every place the slot stands | A static mesh |
| `material.cel`, `material.sky` | The look | A material |
| `event.pickup` | Nothing: adds a sound to the event, played at the event's place | A sound |
| `look.level` | Nothing: adds your lighting and post-processing | A level |
| `look.params` | Nothing: gives your materials the game's numbers | A material parameter collection |
| `look.shadow` | Nothing: gives your materials the key light's shadows | A render target |
| `fx.wall-broke` | Nothing: adds an effect to the event | A Niagara system (see "Effects") |
| `trail.rover` | Nothing: an effect kept on each thing of a kind | A Niagara system |
| `state.digger.drilling` | Nothing: an effect kept on a thing while a flag is set | A Niagara system |
| `look.fx.fill`, `look.fx.shade` | Nothing: the effects' two flat colours over the day | A colour curve |

### A thing with a skeleton

A `thing.KIND` row whose asset is a skeletal mesh draws every one of that
kind with it, at the world's place and yaw, facing +Y in its own space as a
static one does. The world still says only where the thing is; which clip
plays is the bridge's choice from two facts:

- **Its variant.** A world's variants say what a thing is doing. Splice's
  player is 0 on foot, 1 in the air, 2 sneaking, 3 on the hoverboard.
- **Its speed over the ground**, in metres a second, which the bridge
  measures from where the thing was a frame ago.

The clips are rows of the same table, each an animation sequence on the
mesh's skeleton:

| Row | Plays when |
|---|---|
| `anim.KIND.idle`, `.walk`, `.run` | The variant has no rows of its own. Under 0.3 m/s is idle, under 3 is walk, faster is run; a missing one takes the next slower |
| `anim.KIND_vN.idle`, `.walk`, `.run` | The thing is in variant N, by speed in the same way |
| `anim.KIND_vN` | The thing is in variant N, whatever its speed (a jump, a stance) |

A walk or a run keeps step with the rules: one whole turn of the thing's
phase (its eighth number) is one cycle of the clip, so the feet cover the
ground the rules moved it. Other clips loop on the clock. One clip gives way
to the next at once; there is no blend yet, and no animation blueprint.

A skeletal mesh that has vertex colours wears the look's cel material in
place of its own, as the world's things do: a vertex's colour is its flat
fill, the second UV set's u is its cel material (1 skin, 2 hair, 10 cloth)
and the first's u is 1. `character-fit.py --fills` writes a mesh that way. A
mesh with no vertex colours keeps its own materials.

`-AethyrThingBeside=CM` leaves the world drawing a replaced kind and stands
the replacement that many centimetres to its side, to compare the two in one
picture.

`metaverse/splice-bots/tools/character-fit.py --fbx DIR` writes a mesh and
its clips as FBX files for these rows.

### Effects

The rules decide what happens. Unreal shows it. Three kinds of row play a
Niagara system, and none of them changes the run.

| Row | Plays | Where |
|---|---|---|
| `fx.EVENT` | Once, each time the event fires | At the event's place, its +X turned to the event's direction |
| `trail.KIND` | For as long as a thing of that kind is listed | At the thing's place, its +X the way the thing moves (at rest, the way it faces) |
| `state.KIND.FLAG` | While that flag is set on a thing of that kind | At the place the thing's action is (a drill's bit), +X as a trail's |

`EVENT`, `KIND` and `FLAG` are names from the manifest: `events`,
`things.kinds` and `thing_state.flags`. Splice Bots lists the flags
`sprinting`, `board`, `drilling`, `stunned`, `charging`, `dead`, `driving`
and `open`. Two things have a state and are not in `things`: the rover she
drives (`rover`) and the drill mech while it drills (`digger`).

The bridge sets these user parameters on a system every frame. A system
declares the ones it reads:

| Parameter | Type | Value |
|---|---|---|
| `User.Strength` | float | The event's strength. 1 is ordinary. 1 for a trail or a state |
| `User.Speed` | float | The thing's speed in metres a second. 0 for an event |
| `User.Level` | float | 0 to 1: how far the thing is through its action (a drill card, a charge) |
| `User.Direction` | vector | The event's direction, or the way the thing moves, in Unreal's axes |
| `User.Fill`, `User.Shade` | colour | The rows `look.fx.fill` and `look.fx.shade` read at the day's phase (0 dawn, 1 day, 2 dusk, 3 night). Not set when the rows are missing |

A trail's system should spawn nothing at `User.Speed` 0. When a thing goes or
its flag clears, the system is deactivated and ends with its last particle.

An event's record also places its sound: an `event.NAME` row's sound plays at
the event's place, whole within 15 m of the camera and gone by 105 m.

### Tune an effect without a rebuild

`-AethyrRecord=FILE` writes a run's events and things to a text file, one
line each: `f` starts a frame, `v` is the view, `e` is an event's nine
numbers and `t` a thing's sixteen. `-AethyrReplay=FILE`, or the host actor's
**Replay File** in the editor, plays that file in a loop in place of the
world's own events and things, from the recorded view
(`-AethyrReplayFreeView` keeps the game's camera). Change a system, press
Play, and the same hit lands in the same place. `-AethyrFxScale=N` sizes
every effect.

At start the bridge logs each `fx.`, `trail.` and `state.` row that names no
event, kind or flag (`AETHYR_FX row ...`), and each effect as it plays. The
import report lists every row that matches nothing in the manifest
(`overrides.unmatched`).

Kinds, events, the event record's fields and the state flags are listed in
the bundle's `manifest.json` (`things`, `events`, `event_record`,
`thing_state`). Layout is Aethyr's: the import report lists generated actors that
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

## A look of your own

Lighting, shadows, bloom, fog, the colour grade and the sky are Unreal's side of
the picture. Three rows let a project own them. The importer never makes or
changes what they name, so an import leaves your look in place. Run the game with
`-AethyrNoLook` to see it without them.

- **`look.level`** names a level. The bridge loads it beside the generated one
  when the game starts. Put your post-process volume, fog and lights in it. With
  this row, the tone curve and bloom stay on for your volume to set; without it,
  the bridge turns both off. In that level:
  - An actor tagged `AethyrSun` is turned to the game's key light each frame and
    takes its colour. Use a directional light.
  - An actor tagged `AethyrLight0` to `AethyrLight7` is kept at that light of
    the game's, with its colour and reach. Its brightness is its own times the
    game's strength for that light. It is hidden while the game's light is off.
- **`look.params`** names a material parameter collection. Each frame the bridge
  writes the game's uniform block into every vector parameter named `U<n>`
  (floats n to n + 3), as it does for the generated `MPC_Aethyr`. Your materials
  must read your collection, not the generated one, which is made again by each
  import. The bridge also writes these vector parameters when the collection
  has them:

  | Parameter | Holds |
  |---|---|
  | `AeLook` | 1 while the game runs, the clock (0 midnight, 0.5 noon), seconds |
  | `AeSun` | The direction to the key light in Unreal's axes; w is 1 for the moon |
  | `AeShadowU`, `AeShadowV`, `AeShadowD` | A place's u, v and depth (cm) in the shadow picture: `dot(xyz, place) + w` |
  | `AeShadowInfo` | 1 when the shadow picture is drawn, one texel, its width in cm |
  | `AeShadowSet` | Read once, not written: the shadow picture's width and depth in metres |

- **`look.shadow`** names a render target with one float channel. Each frame the
  bridge draws the scene's depth into it as the key light sees it: a square of
  ground ahead of the camera. A material compares a place's depth with it to
  know whether the key light reaches that place.

The cel materials are unlit: they write their own two tones. So Unreal's lights
and shadow maps do not reach them, and a cast shadow on the cel world comes from
`look.shadow`. A light in the look level lights only what has a lit material,
such as a replacement mesh.

`look.py` makes a starter look in `/Game/<Game>/Overrides/Look/` and adds the
three rows, with `material.cel` and `material.sky`. It makes a cel material that
takes the cast shadow into its tones and writes glow brighter than white, a sky
with a sun disc and stars, a post-process pass (inner ink lines, height fog, a
grade for each time of day), and the level. Its dials are the `Ae...` vector
parameters of `MPC_Look`. Run it once, then import again so the importer puts the
two materials on the generated meshes:

```
AETHYR_BUNDLE=/abs/game.aexport UnrealEditor-Cmd Project.uproject \
  -ExecutePythonScript=/abs/metaverse-engine/export/unreal/look.py -unattended -nosplash -RenderOffscreen
```

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
