# Aethyrforge

**Aethyrforge is a browser-first creation engine for building, playing, and
publishing interactive worlds while they keep running.**

Creators work directly in the world. Place a house, walk inside, change a room,
remove a wall, write a behavior, or describe a change. The editor, runtime, and
agents share one understandable, reversible editing model. A creator can publish
a chosen version without rebuilding the engine or interrupting an existing game.

## Repository status

The repository contains the **design specification, version 0.1, dated 2026-10-22**,
and an initial runnable Medieval Town prototype. React DOM, Effect Atom, and
Effect 4 power the editor; an independent worker renders the private source town
through direct WebGPU, with a Rust/Wasm camera and editing kernel. The complete
engine specification is not implemented or qualified. Performance figures in
the design remain targets. The unlimited-engineering premise removes legacy migration and staffing
constraints. Browser limits, physical resources, security, and verification still
shape the design.

The design makes concrete choices and records how to falsify them. Changes to a
contract require corresponding examples, decision records, and acceptance updates.

The [C/Wasm town experiment](../../docs/aethyr/engine/research/c-medieval-town-results-2026-10-23.md)
records measured CPU/build comparisons, browser checks, and their limits. Its
current `c-frame` option keeps Rust character movement while C prepares rendering
data. The [experiment guide](../../docs/aethyr/engine/research/c-town/README.md) also reproduces the
separate, frozen C movement comparison.

For an asset-free playable game, see the [courtyard starter](../../docs/aethyr/engine/starter-game.md).

For an asset-free 100,000-fish comparison, run `npm run dev:boids` and open
`http://127.0.0.1:55336/boids.html`. The [Boids Lab guide](../../docs/aethyr/engine/research/boids-lab-2026-10-23.md)
records the CPU/GPU implementation, production browser measurements, and limits.

## Run Medieval Town locally

Use Node 24+, Binaryen 132 (`wasm-opt`), the pinned Rust toolchain with `wasm32-unknown-unknown`, and the
locally purchased/exported kit. No vendor assets are included in Git.
The kernel build runs Binaryen's `-O3` optimization and removes debug and producer
sections before publishing the Wasm file. Set `AETHYRFORGE_WASM_OPT` when the
optimizer is outside `PATH`.

```sh
npm ci
npm run build:kernel
export AETHYRFORGE_MEDIEVAL_SOURCE="/absolute/path/to/private/repaired-export"
export AETHYRFORGE_LOOK_METADATA="/absolute/path/to/private/recovery/metadata"
export AETHYRFORGE_PRIVATE_ASSETS="$HOME/.openagents/verse/private/aethyrforge"
export AETHYRFORGE_LIGHTING_OUTPUT="$HOME/.openagents/verse/private/aethyrforge-lighting"
npm run prepare:assets
# Optional: use the prepared Everglade/Grove character pack from OpenAgents.
export AETHYRFORGE_EVERGLADE_PACK="/absolute/path/to/private/everglade.vtp"
npm run prepare:character
npm run dev
```

The source export and recovered metadata are local inputs, outside Git. The asset
tools require NumPy, Pillow, and OpenEXR; install
[`tools/assets/requirements.txt`](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/assets/requirements.txt) in your Python
environment. The [private import guide](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/assets/README.md) covers recovering
source cameras and lights, repairing old mesh references in a separate export
copy, and the bounce compiler's coverage and limits.

To try C render preparation with the current town, build its small Wasm module
with LLVM Clang and LLD, then select the backend when starting the existing app:

```sh
node tools/build-c-town.mjs --targets frame
VITE_C_TOWN_BACKEND=c-frame npm run dev
```

The default backend remains `baseline`. Both choices use the same current Rust
character controller, authored edits, WGSL, and private assets.

`prepare:assets` runs four stages in order: `prepare:scene` compiles geometry,
materials, decals, the visible HDRI sky and its IBL, and water; `prepare:lighting`
creates separate atmosphere lighting and IBL maps, validates and copies their
files into the private assets, then links them; `prepare:bounce` prepares the
occluded sky and one-bounce field from that selected lighting. Finally,
`prepare:destruction` creates supported breakable cells and their matching
walking surfaces. The lighting
stage preserves the visible sky and existing probe links. The bounce stage
publishes its own probe link only after its outputs pass validation.

For a lighting-only refresh, run the last two stages sequentially:

```sh
npm run prepare:lighting
npm run prepare:bounce
```

To inspect a new lighting map before linking it, run these commands instead:

```sh
npx tsx tools/assets/compile-atmosphere.ts \
  --metadata "$AETHYRFORGE_LOOK_METADATA" \
  --look "$AETHYRFORGE_PRIVATE_ASSETS/scene-look.json" \
  --out "$AETHYRFORGE_LIGHTING_OUTPUT"
npx tsx tools/assets/compile-ibl.ts \
  --manifest "$AETHYRFORGE_LIGHTING_OUTPUT/lighting-environments.json" \
  --look "$AETHYRFORGE_PRIVATE_ASSETS/scene-look.json"
```

Those commands only write the separate lighting directory. To select the result,
run `npx tsx tools/assets/link-lighting.ts`, then `npm run prepare:bounce`.
The linker defaults to the two exported output directories; it also accepts
`--manifest PATH --out DIR`. It verifies image hashes, IBL source identities,
and current light metadata, copies content-addressed files first, then atomically
replaces each scene's `lightingEnvironmentURL` and updates `source-index.json`.
The visible `environmentURL` and existing probe/bounce fields remain unchanged.
Run preparation commands sequentially, with one writer per private output.
An interrupted publication can be retried; remove `.lighting-publication.lock`
only after confirming its preparation process has stopped.

The atmosphere map is an owned single-scattering lighting approximation, separate
from the visible source HDRI. It excludes the direct sun disk, clouds, fog,
multiple scattering, and ground bounce. Source sunlight and sky color are baked;
Skylight intensity remains a runtime factor. This profile does not establish
Unreal or reference-gallery parity.

For recovered dust and chimney smoke, follow the [effects recovery steps](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tools/assets/README.md#ambient-dust-and-smoke).
Set `AETHYRFORGE_PARTICLE_METADATA` to that recovery's `metadata` directory and
`AETHYRFORGE_SOURCE_TEXTURES` to its `textures` directory, then run
`npm run prepare:effects`. This optional stage publishes the effects link into
the prepared scenes; the licensed atlas stays in the private asset directory.

Open `http://127.0.0.1:55304/`. The isolated preview uses port 55305. Click the
world to look, use WASD to move, Space to jump, Shift to run, and V to switch
first and third person. Scroll to adjust the follow camera. Fly remains available
in the hotbar, with Space/C to rise/fall. Escape releases the pointer.
Choose Meteor Shower (1), Lightning Bolt (2), or Thunderbolt (3), aim at the town,
then click or press Enter to cast. Right click or Escape cancels targeting.
**Destroy the Castle (4)** faces and targets the distant castle automatically.
Cast to start a sustained meteor and lightning barrage; **Stop barrage** or Escape
stops future waves. The castle remains rubble until **Restore town**.
**Restore town** clears local damage and spell effects; step out of a broken
wall before restoring it. Spells affect this play session, not the saved project.
The hotbar opens assets, rooms, commands,
lighting, and WGSL editing. Try “add a cottage in front of me”, “add five barrels to my left”,
“add a room”, “knock down this wall”, and “undo”. Invalid shader edits keep the
working image and player position.

This development server binds loopback and serves the selected private output
directory locally. Static builds do not bundle the purchased scene. See
[implementation status](../../docs/aethyr/engine/implementation.md) for the current scope and gaps.

## Run Open Ground locally

The [Quick MMORPG world](../../docs/aethyr/engine/quick-mmorpg-port.md) adds the source game's
Paladin and Sorceror players, Zombie and Warrok enemies, shared multiplayer,
equipment, inventory, chat, stats, scenery, terrain, and effects to this same
scene. It keeps our grass and sky. Follow that guide to prepare the four-class
private asset set and start the Rust world server with `npm run dev:ground`.
Guest sessions and world state last for the server's current run.

The [haunted starter](../../docs/aethyr/engine/fantasy-assets.md#open-ground-haunted-starter) adds
eight Quaternius props near spawn and compact in-world Edit tools. Prop edits
save on this device; the house shell and shared building edits are later steps.

The lighter local encounter remains available with its original asset set:

Open Ground is a standalone walking zone with continuous green grass, Medieval
Town's sky and lighting, and the Paladin from
[Quick_3D_MMORPG](https://github.com/simondevyoutube/Quick_3D_MMORPG). The character
retains its body, helmet, equipped sword, original base and normal maps, and six
animations: idle, walk, run, attack, dance, and death. Its movement, animation
transitions, and fixed following camera adapt the source controller to the owned
Rust/WebGPU runtime. See the [source notice](third_party/quick-mmorpg/NOTICE).

Its direct WebGPU grass uses
clumped growth, curved blades, wind, and player bending, guided by the
[Ghost of Tsushima grass talk](https://www.youtube.com/watch?v=Ibe1JBF5i5Y) and
adapted from [Quick Grass](https://github.com/simondevyoutube/Quick_Grass).
GPU preparation feeds two bounded instanced draws; blades fade near the camera.
Their base height is 0.75 meters. The
[reference study](../../docs/aethyr/engine/research/grass-reference-study-2026-10-23.md) records
the techniques, controlled comparisons, measurements, and remaining work.
The zone has a live FPS counter, nearby zombies, and sword combat. A compact
overlay shows your health and the remaining and defeated zombies; **Restart**
starts the fight again. Attack and dance use the character's source animations.
Buildings, props, quests, and world-editing tools are absent. Its assets and
walking surfaces are prepared separately from the town.

Open **Effects** to adjust the shared wind, blade height and width, and grass
response while walking. **Preview impact** plays a cosmetic flash, sparks,
smoke, and debris; smoke follows the same wind field as the grass. Parameter
edits keep the player, world time, and active effects running. These preview
settings reset on reload. See [shared wind and live effects](../../docs/aethyr/engine/shared-wind.md)
for the interface, limits, and measured checks.

Prepare the ground first, then compose the Paladin and zombies into new private
outputs. Clone both reference repositories if you do not already have them, and
choose output directories that do not exist:

```sh
git clone https://github.com/simondevyoutube/Quick_Grass "$HOME/work/Quick_Grass"
git clone https://github.com/simondevyoutube/Quick_3D_MMORPG "$HOME/work/Quick_3D_MMORPG"
python3 tools/assets/compile_ground_zone.py \
  --source "$AETHYRFORGE_MEDIEVAL_RUNTIME" \
  --grass-style quick-grass \
  --grass-source "$HOME/work/Quick_Grass/public/textures" \
  --out "$HOME/.openagents/verse/private/aethyrforge-ground-base"
python3 tools/assets/compile_quick_mmorpg_character.py \
  --reference "$HOME/work/Quick_3D_MMORPG" \
  --base "$HOME/.openagents/verse/private/aethyrforge-ground-base" \
  --out "$HOME/.openagents/verse/private/aethyrforge-ground-paladin"
python3 tools/assets/compile_quick_mmorpg_character.py --zombie \
  --reference "$HOME/work/Quick_3D_MMORPG" \
  --base "$HOME/.openagents/verse/private/aethyrforge-ground-paladin" \
  --out "$HOME/.openagents/verse/private/aethyrforge-ground"
AETHYRFORGE_PRIVATE_ASSETS="$HOME/.openagents/verse/private/aethyrforge-ground" npm run dev:ground
```

Set `AETHYRFORGE_MEDIEVAL_RUNTIME` to the existing prepared Medieval Town asset
directory. All preparation stages keep source and derived art outside Git.
Open `http://127.0.0.1:55316/`; its preview runs on port 55317. Click the world
to focus the controls. Use W/S to move forward and backward, A/D to turn, and
Shift to run. From idle, press Space to attack or Backspace to dance. The camera
follows the character at a fixed offset; press Esc to release the mouse.
Set `AETHYRFORGE_EDITOR_PORT` and `AETHYRFORGE_PREVIEW_PORT` to unused ports
when another worktree already has a development server running.

## The engine we are specifying

| Part | Choice |
| --- | --- |
| Creator experience | Fullscreen world, compact floating hotbar, contextual tools, accessible alternatives |
| Editor | React DOM and TypeScript, Effect Atom for UI state, Effect 4 for application services |
| Simulation | Owned Rust kernel compiled to WebAssembly, fixed-step simulation, coarse memory interfaces |
| Graphics | Owned direct WebGPU renderer and WGSL materials |
| World | Semantic entities, buildings, rooms, walls, openings, terrain, behaviors, and assets |
| Compilation | Incremental Rust compiler deriving graphics, physics, navigation, and selection together |
| Safe creator scripting | A documented TypeScript subset compiled to metered behavior bytecode |
| Authoring | One typed transaction path for direct tools, semantic commands, and agents |
| Collaboration | Validated branch revisions, explicit conflicts, offline branches, separate play sessions |
| Publishing | Immutable content-addressed releases and signed channel updates |
| Integration | Replaceable adapters for Kiln, OpenAgents, decision models, asset stores, and native hosts |

## Start reading

- [Product and scope](../../docs/aethyr/engine/product.md): who creates, what they create, and what success means.
- [Creator experience](../../docs/aethyr/engine/creator-experience.md): the complete open-to-publish journey.
- [Frontend UI](../../docs/aethyr/engine/frontend.md): React, Effect Atom, Effect 4, and the independent engine frame loop.
- [Architecture](../../docs/aethyr/engine/architecture.md): ownership, language choices, and component boundaries.
- [World model](../../docs/aethyr/engine/world-model.md): why a house is editable structure, not only a mesh.
- [Contracts](../../docs/aethyr/engine/contracts.md): transactions, generations, errors, and portable wire examples.
- [Delivery and acceptance](../../docs/aethyr/engine/delivery.md): dependency milestones and exit gates.
- [Full specification index](../../docs/aethyr/engine/README.md): every subsystem and its source of truth.
- [Medieval Town visual target](../../docs/aethyr/engine/references/medieval-town.md): inspected gallery, every shot, sky/lighting/material requirements, and parity gates.

## What makes iteration fast

1. Most edits change data, not engine code.
2. The compiler rebuilds only affected outputs and reuses cached work.
3. The runtime activates complete compatible generations and preserves live state.
4. Expensive CPU work uses Rust/Wasm; repeated objects share GPU resources.
5. Publishing transfers changed artifacts and updates a release reference.

No language choice eliminates shader compilation, network latency, or hardware
limits. The [performance contract](../../docs/aethyr/engine/performance.md) defines exactly what is
timed and on which workloads.

## Design checks

```sh
python3 tools/check_spec.py
```

The checker validates local documentation links, requirement ownership, JSON
contracts and examples, and bounded abstract activation/publishing models. It is
documentation evidence, not an engine test. See [verification](../../docs/aethyr/engine/verification.md).

No vendor art, private sibling implementation, secrets, production accounts, or
runtime dependencies are copied into this initial specification. Licensing and
third-party distribution gates are in [integration and ownership](../../docs/aethyr/engine/integration.md).
