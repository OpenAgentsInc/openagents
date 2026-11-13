# The Metaverse Engine to Unreal exporter: specification

Specification, 2026-11-12. Status: proposed. Nothing here is built.

We build games in the OpenAgents Metaverse Engine and export them to Unreal Engine (UE) 5.9. The first
game is Splice, as it stands on `main` (`metaverse/splice-bots/splice-game.coax`).
This document defines the smallest exporter that turns that game into an
Unreal project that plays the same run.

Import from Unreal into the Metaverse Engine comes later. This spec keeps that door open
and does not build it.

Background: [the two-session audit](2026-11-12-splice-unreal-vs-aethyr-session-audit.md)
shows why the game is authored in the Metaverse Engine.

## 1. Decision

**The minimal exporter moves content, and hosts the rules. It does not
translate the rules.**

| Part of the game | How it reaches Unreal |
|---|---|
| The static world | Exported once as glTF. Unreal imports it as native static meshes in a level. |
| Collision boxes, placements, tuning, strings | Exported as one JSON manifest. The importer turns them into volumes, markers and a data asset. |
| The look (cel shading, glow, sky, ink) | One hand-written Unreal material set in the bridge plugin, driven each frame by the game's own uniform block. |
| The rules | The game's Coax module compiled to C and built into the Unreal project. A bridge plugin calls it each tick through the same host interface the web page uses. |
| Moving things (player, Digger, raiders, boss) | Drawn from the rules' per-frame vertex stream into one dynamic mesh. |
| The interface (HUD, edit bay, banners) | Drawn from the rules' per-frame overlay stream. |
| Input | Unreal keys and mouse forwarded to the rules. |

Why this is the minimum:

- **The boundary already exists.** A world on the web is a Coax module behind
  a host interface: `coaxial_frame`, `coaxial_store`, `coaxial_key`,
  `coaxial_pointer`, `coaxial_look_delta` (`metaverse/splice-bots/splice-face.c`).
  The browser host implements that interface. A second host inside Unreal
  implements the same one. No rule is rewritten, so the Unreal build cannot
  drift from the Metaverse Engine build.
- **The two other routes cost more and give less.**

| Route | Cost | Problem |
|---|---|---|
| Port by hand | The Unreal session wrote 3,812 lines of Python and 15 Blueprints for one game | Two copies of every rule. They drifted within a day. |
| Translate Coax rules to Blueprints | A compiler back end | Epic removes Blueprints and Actors in UE6 ([ue6.md](ue6.md)). The work has a two-year life. |
| Host the compiled rules (this spec) | One plugin, written once, for every Metaverse Engine game | Rules are not editable inside Unreal. See section 9. |

- **Content still becomes real Unreal assets.** The world is static meshes
  in a level, not a blob. A designer can replace an exported mesh, material
  or sound in Unreal, and the replacement survives re-export (section 7).
  That is the part of Unreal the project needs.

## 2. What Splice is made of

Everything the exporter must carry, read from the source on `main`:

| Piece | Where | Size and shape |
|---|---|---|
| Static world | `g_world` | Triangles written once into a store of up to 150,000 words. No index buffer. |
| Vertex | `metaverse-engine/render/cel-mesh.coax` | Six 32-bit words: x, y, z; normal as three bytes plus an object number; sRGB colour as three bytes plus a material byte; one float parameter. |
| Materials | `MAT_*` in `cel-mesh.coax` | Eleven classes: world, skin, hair, glow, flat, floor, made, ink, smoke, window, cloth. |
| Kits | `metaverse-engine/kit/land.coax`, `metaverse-engine/kit/machines.coax` | Functions that write triangles: mesas, canyon walls, rover, Digger, figure, mech. |
| Moving things | `g_live` | Rebuilt every frame from the game state and the clock. |
| Collision | `g_layout`, `g_collide`, `ground_y` | Up to 44 axis-aligned boxes (x0, x1, z0, z1, height), a raised deck with two ramps, circle pushes. |
| Placements | `pick_x`, `foe_ax`, constants | 10 pickups, 9 foes with two patrol points each, rover, post, crate, boss, three weak points, the rail curve, two hidden stages. |
| Lights | `g_lights`, `g_key_light` | Eight point lights, some moving on the beat. One key light direction per time of day. |
| Look presets | `g_variant`, `g_clock`, `animelab_style_block` | Four moods as a 128-float style block. |
| Camera | `g_camera` | Follow camera with wall pull-in. Fixed shots for title, bay, ride and memory. |
| Rules | `g_step` and helpers | About 450 lines. State is one struct, `SpGame`. |
| Tuning | `T_*` constants | About 50 numbers. |
| Interface | `g_hud`, `g_bay`, `g_overlay` | Quads and glyph quads in two fonts, Michroma and IBM Plex Mono. |
| Input | `g_key`, `g_pointer`, `g_look`, `g_lock` | Character codes and small negative codes for arrows, Enter, Tab. |
| Modes | `?scene=N`, `?gray` | 17 still scenes and the graybox mode. |

Splice has no sound, no skinned meshes, no textures on world geometry and no
network play. The exporter does not handle those in this version.

## 3. The bundle

The exporter writes one folder. It is the only thing the Unreal side reads.

```
splice.aexport/
  manifest.json        what the bundle is and what it expects
  world.glb            the static world
  rules/               the game as C, plus one header
  fonts/               Michroma and IBM Plex Mono (OFL)
  shots/               reference pictures and their cameras
```

### 3.1 `manifest.json`

Versioned (`"aexport": 1`). All positions are in Metaverse Engine coordinates: metres,
y up. It contains:

- `game`: id, name, source commit, bundle hash.
- `axes`: the mapping to Unreal units and axes, stated as a matrix. Measured
  in step 0 with Unreal's default glTF import: Unreal X = x, Unreal Y = z,
  Unreal Z = y, and 1 m = 100 units. The Metaverse Engine is right-handed with y up, which
  is glTF's own convention, so the file carries the Metaverse Engine's numbers unchanged
  and Unreal's importer does the flip. The Unreal session's hand mapping
  (X = −z, Y = x) is a different, also valid, choice and is not used.
- `collision`: the boxes from `g_layout`, the deck and its ramps.
- `placements`: every named spot with its kind and position.
- `tuning`: every `T_*` constant by name.
- `strings`: every text the interface shows.
- `keys`: the key codes the rules expect, by name.
- `materials`: the eleven classes, each with how it shades.
- `uniforms`: the offsets in the uniform block that the look reads (key
  light, eight lights, style block, graybox flag).
- `stores`: vertex layout and capacities for the live and overlay streams.
- `shots`: for each reference picture, the game state used and the camera.

### 3.2 `world.glb`

glTF 2.0 binary, because both sides already speak it. Unreal 5.9 imports it
through Interchange (`Engine/Plugins/Interchange`), and the Metaverse Engine already reads
it (`metaverse-engine/anim/gltf-access.coax`, `metaverse-engine/anim/import-container.coax`).

- One node per group. A group is one material class inside one 64 m grid
  cell, so Unreal can cull by cell.
- Attributes: `POSITION`, `NORMAL`, `COLOR_0` (the vertex colour, linear),
  `TEXCOORD_0` carrying the vertex parameter and the object number.
- Materials: one per class. Glow uses `KHR_materials_emissive_strength`. The
  rest use `KHR_materials_unlit`. Unreal's importer reads both extensions
  and vertex colour. The importer then swaps each for the plugin's own
  material by class name, so the glTF materials are only a readable
  fallback.
- No lights or cameras in the file. They move, so the rules own them.

The exporter welds identical vertices and writes an index buffer. The Metaverse Engine
draws the same triangles without one.

### 3.3 `rules/`

- The Coax compiler already emits C for the web build: `coax/web/wasm.sh`
  runs `coax c SOURCE --library --header` and compiles the result with
  clang for `wasm32-wasip1`. The exporter emits the same C for the game
  module and its loads.
- `aethyr_host.h` (`metaverse-engine/export/unreal/aethyr_host.h`) declares the host
  interface with real pointers. The web face passes pointers as 32-bit
  integers, which is wrong for a 64-bit target, so the bundle carries its
  own face, `metaverse/splice-bots/splice-ue-face.c`.
- Unreal's build tool compiles these files inside the bridge plugin for each
  platform. There is no prebuilt library to keep in step.

## 4. The exporter

A native Coax tool, per the tools rule (`coax/tools-in-coax.md`).

| File | Role |
|---|---|
| `metaverse-engine/export/glb-write.coax` | Writes glTF binary from cel vertices. Generic. |
| `metaverse-engine/export/bundle.coax` | Writes the manifest and the folder. Generic. |
| `metaverse/splice-bots/splice-export.coax` | The command. Loads the game, calls `g_reset` and `g_world`, lists placements and tuning. Per game. |
| `metaverse-engine/export/raster.coax` | Draws the reference shots in software. Generic. |
| `metaverse-engine/tools/unreal-export-check.coax` | Reads a bundle back with the Metaverse Engine's own glTF reader. |
| `metaverse-engine/tools/unreal-import.coax` | Stages, builds and runs the Unreal import, and returns its verdict. |

The per-game file sits beside the game because the engine may not load from
`apps/` (`metaverse-engine/layer-allowlist.txt`).

```
coax/web/tool.sh metaverse/splice-bots/splice-export.coax OUT_DIR
```

Rules for the tool:

- It runs the game's own world code. It never re-describes the world.
- Same source commit, same bytes out. The bundle hash is in the manifest.
- It refuses to write into an existing folder, like the animation exporter
  (`metaverse-engine/anim/export-policy.coax`).
- It reads no Unreal file and needs no Unreal install.

One change to the game is needed first. Tuning, placements and strings are
constants and inline tables today. They move into tables the exporter can
list. The web build's vertex stream must be byte-identical before and after
that change.

## 5. The Unreal side

Two pieces, both kept in this repo under `metaverse-engine/export/unreal/` and copied
into the target project by the importer.

### 5.1 Importer

A Python script for the Unreal editor. Firm reason 6: the editor's asset API
exists only in Python and C++.

It runs through the editor's Model Context Protocol (MCP) server
([mcp-tools.md](mcp-tools.md)) or headless with `UnrealEditor-Cmd`. It:

1. Checks the manifest version and bundle hash.
2. Imports `world.glb` with Interchange's scripted import into
   `/Game/Aethyr/Splice/Generated/`.
3. Assigns the plugin's materials by class name.
4. Builds the level `L_Splice`: the meshes, one blocking volume per
   collision box, one named marker per placement, one bridge actor.
5. Writes `DA_SpliceTuning` and the string table from the manifest.
6. Imports the two fonts.
7. Copies `rules/` into the plugin and triggers a build.

The importer follows the rules the audit paid for:

- **Safe to rerun.** It deletes `Generated/` and rebuilds it. It never
  touches anything else.
- **Loud.** After import it reads back what exists and compares counts with
  the manifest: meshes, triangles, volumes, markers. Any difference is a
  failure with a non-zero exit.
- **Materials are checked.** It reads each material's compile result and
  fails on an error. A fallback material is a failure.
- **It takes its own pictures.** It renders each reference camera from the
  editor and writes the frames next to the bundle's `shots/`.

### 5.2 Bridge plugin

A C++ plugin, `AethyrBridge`. Firm reason 6. It is written once and is the
same for every Metaverse Engine game.

It holds one actor, `AAethyrGame`, that is the host:

- **State.** Allocates the game state at the size the rules report and zeros
  it, as the web host does.
- **Tick.** Calls `coaxial_frame` with Unreal's delta time, clamped as the
  web host clamps it.
- **Live mesh.** Reads the live store and updates one dynamic mesh
  component when the store version changes. Same vertex decoding as the
  exporter.
- **Overlay.** Reads the overlay stream and paints it over the viewport:
  solid quads, and glyph quads drawn with the two imported fonts.
- **Look.** Copies the uniform offsets named in the manifest into a material
  parameter collection: key light, eight lights, style block, graybox flag.
- **Camera.** Places the player camera at the eye, target and field of view
  the rules report.
- **Input.** Forwards keys, pointer position, and mouse deltas. Captures the
  mouse when the rules ask for it.

The plugin's materials are the one hand translation in the design:
`M_AeCel`, `M_AeGlow`, `M_AeSky` and a depth-only ink pass. They are unlit
and compute their own tone, so they work on Unreal's forward web tier,
which has no normal buffer. Their target is the Metaverse Engine shader's output, and
section 8 defines how close they must be.

The static world in the level is the exported meshes, not the store, so the
plugin skips store 0.

## 6. What the first version does not do

- Translate rules to Blueprints, Verse or C++ gameplay classes.
- Translate WGSL to Unreal materials automatically.
- Export moving things as separate Unreal assets. That is step 3 below.
- Sound. Splice has none. Step 3 adds an event list for it.
- Saves, network play, skinned meshes, textures.
- The 17 still scenes. The graybox mode works because it is a uniform flag
  and a vertex colour rule.
- Import from Unreal.

## 7. Replacing exported content in Unreal

Re-export must not destroy a designer's work.

- `/Game/Aethyr/Splice/Generated/` belongs to the importer and is rebuilt
  each time.
- `/Game/Splice/Overrides/` belongs to people. The importer never writes
  there.
- A data table, `DT_AethyrOverrides`, maps an exported name to a replacement
  asset. The importer applies it after each import.

In the first version the names are world groups and material classes. A
designer can swap a canyon wall mesh or the glow material. Layout stays in
the Metaverse Engine: moving a wall in Unreal does not move the rules' wall. The importer
reports any generated actor that was moved by hand.

## 8. Acceptance

Each step ends on a check a script runs. No step is done on a claim.

Issues: step 0 is #13399, step 1 is #13400, step 2 is #13401, step 3 is
#13402.

**Step 0. Prove the two risks. One day.**

1. Compile the game's C inside an Unreal module for Mac and for the Wasm
   target. Unreal's web build is 64-bit, and the Coax C has only been built
   for 32-bit Wasm. Call `coaxial_frame` once and compare the first frame's
   vertex bytes with the web build's.
2. Export three marker cubes at known Metaverse Engine positions, import them, and
   read their Unreal positions and face winding. This fixes the axes in the
   manifest.

If the Wasm compile fails, stop and fix the compiler's C output before any
other work.

**Step 0 result, 2026-11-12: both risks are cleared.** The probe is in
`metaverse-engine/export/unreal/probe/` with the commands to rerun it.

The probe runs a fixed 34-frame script (leave the title, walk forward) and
hashes every chunk a host reads at frames 0, 1 and 33: the overlay stream,
the uniform block, the world store and the live store.

| Build of Splice's C | Pointer size | Result |
|---|---|---|
| Web build today (`wasm32-wasip1`, run in Node) | 4 bytes | Reference |
| Native Mac, clang, arm64 | 8 bytes | Every chunk byte-identical |
| 64-bit Wasm, Unreal's Emscripten 6.0.10 with `-sMEMORY64=1`, run in Node | 8 bytes | Byte-identical |
| Inside an Unreal 5.9 editor module on Mac, run headless | 8 bytes | Same hashes |
| Inside a packaged Unreal 5.9 Wasm build (`wasm64-unknown-emscripten`), in headless Chrome | 8 bytes | Same hashes |

Facts from the run:

- The Coax C needed no change. The only new code is a face with real
  pointers (`metaverse/splice-bots/splice-ue-face.c`) behind
  `metaverse-engine/export/unreal/aethyr_host.h`.
- Unreal's build settings turn `-Wunreachable-code-aggressive` into an
  error, and the generated C trips it. The module that compiles the rules
  turns that one warning off.
- Splice's static world is 68,394 vertices (1.64 MB). The live store is 372
  vertices on the title screen. The game state is 1,984 bytes.
- The Unreal Wasm package of the bare probe project took 442 seconds to
  build and is 289 MB on disk as a Development build.
- Axes: three marker cubes at the Metaverse Engine (10, 0, 0), (0, 5, 0) and (0, 0, 20)
  import at Unreal (1000, 0, 0), (0, 0, 500) and (0, 2000, 0). Their faces
  wind the same way as Unreal's own cube.
- Not confirmed: vertex colour after import. Unreal's Python mesh API reads
  positions and UVs only, so step 1 checks colour in a render.

**Step 1. Static world.**

- The bundle is byte-identical on two runs.
- Our own glTF reader loads `world.glb` and gets the same triangle count
  and bounds the game wrote.
- The importer's counts match the manifest.
- A second import changes nothing.
- With stand-in lighting, six reference cameras render in the editor, and
  each frame is compared with the Metaverse Engine frame from the same camera. This
  step checks geometry and colour only: silhouette overlap of at least 98
  percent.

**Step 1 result, 2026-11-12: done.** Commands are in
`metaverse-engine/export/unreal/README.md`.

| Check | Result |
|---|---|
| Two exports | Byte-identical |
| Read back with `metaverse-engine/anim`'s glTF reader | 58 meshes, 22,798 triangles, 18,618 vertices (welded from 68,394), hash and bounds match |
| Importer counts against the manifest | 58 meshes, 22,798 triangles, 30 collision boxes, 50 markers, 2 font faces, 49 tuning numbers |
| Every vertex position and colour against `world.glb` | All match |
| Material compile errors | None |
| Six reference shots | 100 percent of pixels agree on five, 99.98 on the sixth; mean colour difference at most 0.24 of 255 |
| Second import | Same report, byte for byte; a file outside `Generated/` is untouched |
| A manifest that disagrees with the world | The import exits 1 and names the mesh |

What changed from the plan:

- **The reference shots are drawn in software**, from the same vertex stream
  the page draws, not by the page. That removes the anime shading, the sky
  and the post pass from the comparison, which is what step 1 measures.
- **No string table.** The hosted interface draws its own text (step 2), so
  nothing in Unreal reads the strings yet.
- **The tuning and placement lists live in `splice-export.coax`**, not in new
  tables inside the game file. A constant added to the game is not exported
  until it is listed there.
- **The command is `metaverse/splice-bots/splice-export.coax`**, beside the
  game, because the engine may not load from `apps/`.
- **Collision is `AethyrCollisionBox` actors**, a box component set to block
  everything, not brush volumes.
- **Nanite is turned off on import.** Unreal's importer turns it on, and the
  rendered mesh was then a 13,542-triangle fallback. The importer's own
  count check caught this.
- **Unreal stores the vertex colours as linear bytes**, which confirms the
  item step 0 left open.

**Step 2. Hosted rules. This is the minimal exporter.**

- The scripted run that passes in the Metaverse Engine (title to SCENE RESTORED, with the
  same key presses) passes in the Unreal editor, with the same state hash at
  each beat.
- The same run passes in the packaged web build on both renderer tiers, in
  a real browser, with real key presses.
- The six reference frames match the Metaverse Engine frames within a set colour
  difference. The first run sets the number and it only tightens.
- The interface frames (HUD, edit bay, both fail banners) match within the
  same measure.
- Web build frame time is recorded with the live mesh updating every frame.

**Step 2 result, 2026-11-12: done.** One command exports, and one imports,
builds and proves the run (`metaverse-engine/export/unreal/README.md`).

| Check | Result |
|---|---|
| The scripted run, title to SCENE RESTORED, in the Unreal editor's game | The same ten hashes as the native reference, and every expected fact holds |
| The same in the packaged web build, forward and deferred tiers, headless Chrome | The same eleven lines as the bundle's `rules/run.expected` |
| The run played on screen by the game itself (`-AethyrPlay`) | Reaches SCENE RESTORED on both web tiers |
| Real key presses in the browser | Enter starts the run and holding W walks out through the scanner gate onto the rail ride, on both tiers. G turns the graybox on and off. |
| Web frame time, the mine, live mesh rebuilt every frame | 16.67 ms (the display's 60 Hz) on both tiers; the host's own work is about 1.2 ms a frame for 16,500 to 20,300 live vertices |
| Graybox | Toggles in the web build; the interface, world and sky follow the engine's rule |

One caveat on frame time: a later re-measure read 33.33 ms at every stage,
including one with 2,340 live vertices and 0.27 ms of host work. The Mac was
then on battery at 17 percent, which halves the browser's frame rate, so
that reading says nothing about the build. Measure again on mains power.

Frames against the Aethyr page at the same stage (`unreal-frame-compare`,
1280 by 800, mean difference of 255). These are the first run's numbers, so
they are the ceiling from here:

| Stage | Mean difference | Pixels within 24 |
|---|---|---|
| 1 village at dawn | 10.78 | 63.3% |
| 3 rail ride | 8.87 | 80.7% |
| 5 mine | 5.40 | 92.5% |
| 8 party at night | 34.35 | 47.2% |
| 10 boss | 9.39 | 84.1% |
| 12 SCENE RESTORED | 6.90 | 98.8% |
| 6 edit bay | 3.75 | 97.4% |
| 13 Imperial Sweep banner | 2.92 | 99.4% |
| 14 CUT banner | 6.00 | 99.3% |

The party's number is mostly time: the floor tiles change colour on the beat
and the two captures are at different beats. The rest is what the look does
not translate yet.

What changed from the plan, and what is not done:

- **The scripted run places the smuggler between beats** (messages 611 to
  613), as the game's own test does. It is not a walk through the level by
  keys. Real keys are checked separately and only for the opening.
- **The hash at each beat is of what a host reads** (overlay, uniforms, both
  stores), not of the state's bytes, so it does not depend on pointer size.
- **The look is translated, not matched.** `M_AeCel` and `M_AeSky` carry the
  cel bands, point lights, rim, aerial fade, tone, saturation and lift, and
  the sky's gradient, sun band and moon. Not translated: the key light's
  cast shadows, bloom, the vignette, clouds, stars, the far ridge and the
  lake, and the rounded corners of the overlay's panels.
- **The ink pass draws silhouettes only.** It reads depth alone, which is all
  the forward web tier has. Aethyr also inks the line between two things
  that touch and creases, which need object numbers and normals.
- **Glow is inside `M_AeCel`**, not a separate material.
- **Unreal's tone curve, exposure and lens effects are off** in the game
  view: the materials write final colour.
- **The import step names Splice's level** when it plays the run. A second
  game needs that line generalised.

**Step 3 result, 2026-11-12: done.**

| Check | Result |
|---|---|
| The things list leaves the page unchanged | The run's ten hashes are the same with and without it |
| Every kind drawn by Unreal (`-AethyrNativeThings`) | The run plays to SCENE RESTORED; the party frame differs from the world-drawn one by 0.81 of 255 |
| A replaced kind survives a re-export | A stand-in designer replaces the raider's model and gives the pickup event a sound. After a new export and import the three override files have the same hashes, and the game uses both |
| A sound in the web build | `PlaySound2D` is called with the loaded sound six times over the run, on the forward tier. Nothing listens in a headless browser, so audible output is not confirmed |
| Web frame time, the party | Host work 1.03 ms a frame with the world drawing things, 0.29 ms with Unreal drawing all of them (live vertices 13,500 to 2,340); the frame stays at 16.67 ms |

What changed from the plan:

- **A kind is drawn by Unreal only when it has a model**: an override row, or
  all kinds with `-AethyrNativeThings`. By default the world still draws
  everything, because an exported model is the thing at rest: it does not
  dance, walk or spin its drill.
- **The models are lifted from the game's own drawing**, not re-described:
  the exporter runs `g_live` at a moment of the run and moves each thing's
  vertices into its own space.
- **Events are the game's own messages** plus two more, so no rule was
  rewritten to make sounds possible.
- **Not exported as things:** the rubble, the boss wall, the boss's arm and
  weak points, the dance floor and beams, the Battery beside the rail, and
  the rover parked at the mine (it is part of the static world).
- **World overrides are by mesh group**, which is a material class in a
  ground cell, not one wall or one rock.

**Step 3. Native things and sound.**

- The game lists its moving things each frame: kind, variant, position,
  yaw, pose values. The exporter bakes one mesh per kind and variant. The
  bridge places them and stops drawing those kinds from the live stream.
- An override can replace a kind with a designer's mesh.
- The rules emit named events (pickup, hit, drill, splice, success). A data
  table maps events to sounds.

## 9. Risks and open questions

| # | Item | Answer today |
|---|---|---|
| R1 | Coax C on 64-bit Wasm inside Unreal's toolchain | Cleared in step 0: same bytes as the web build, in a packaged Unreal Wasm build. |
| R2 | Updating a dynamic mesh every frame on the web tier | Measured: about 1.2 ms a frame for 20,000 vertices; 60 frames a second holds on both tiers. |
| R3 | Look parity with a hand-written material | Translated from Aethyr's shader source and measured (step 2's table). Shadows, bloom and full ink are still missing. |
| R4 | Overlay text drawn with Unreal fonts will not match glyph for glyph | Each glyph is drawn at its own place in its own face; the interface frames differ by 3 to 6 of 255. |
| R5 | Rules are not editable in Unreal | By design. Rules are edited in the Metaverse Engine and re-exported in seconds. |

Decided, 2026-11-12: the design brief's "Blueprints only" rule does not
apply to this work. The bridge is a C++ plugin.

Owner decisions still open:

1. **Does the jam accept a game whose rules are a compiled C library inside
   an Unreal project?** I have not read the jam rules. If they require
   gameplay written in Unreal, hosted rules do not qualify and the entry
   stays the hand-built Unreal project.
2. **Which project receives the export:** the existing `Rovers` project, or
   a new clean one.

## 10. Licence

The rule in [the learning note](2026-11-11-learning-from-unreal-5-9.md)
stands: ideas only, never Epic's code.

- The exporter is Coax and reads no Unreal source at run time. Its output is
  glTF, a Khronos format, and our own JSON.
- The importer and the bridge plugin call Unreal's public API. They are
  Unreal-based code written by us. They contain no copied engine source, no
  engine shader text and no engine template files.
- The Unreal source on this Mac was read for this spec only to confirm what
  the importer accepts: the glTF extensions in
  `Engine/Plugins/Interchange/Runtime/Source/Parsers/GLTFCore`, the scripted
  import calls in `InterchangeManager.h`, and that the MCP plugin, the glTF
  exporter plugin and the Wasm platform exist.
- Both fonts are under the SIL Open Font License.

## 11. Toward two-way

Choices here that the import direction will use:

- **glTF both ways.** Unreal 5.9 ships a glTF exporter plugin
  (`Engine/Plugins/Enterprise/GLTFExporter`), and the Metaverse Engine reads glTF.
- **One manifest schema.** An import bundle is the same folder with the
  arrows reversed: meshes, collision, placements, tuning.
- **Stable names.** Every exported thing has a name that survives a round
  trip, which is what the override table already needs.
- **Existing work.** The Metaverse Engine already reads animation from an Unreal project
  (`metaverse-engine/anim/unreal-source.coax`, `metaverse-engine/tools/anim-unreal-export.coax`).
  Level import should extend that tool and its file policy, not start
  again.

What import cannot carry is logic. Blueprints and Verse do not become Coax.
An imported Unreal project arrives as a world to write rules for.
