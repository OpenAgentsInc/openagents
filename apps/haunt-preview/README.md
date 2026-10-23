# Browser creator labs

A local browser editor with direct WebGPU, Effect, the real Kiln WASI/OPFS
workspace, and the Rust Jev SDK. Scene controls apply typed commands immediately.
Natural language takes one bounded decision request, then uses those same commands.
You can combine up to four changes in one request; they save and undo together.

- `/` opens **Haunted House**, with its spooky props and existing room editor.
- `/medieval/` opens **Medieval Town Lab**, with a fullscreen world and floating
  hotbar. Asset, building, and shader tools open as small popovers.

The labs share the renderer and command runtime, with separate project IDs,
scene/material paths, catalogs, and Kiln OPFS snapshots. Older Haunted House saves
lose their medieval prefabs and outdoor additions while keeping spooky props and
authored indoor rooms; the cleaned scene is saved back to Kiln.

## Creator flow

| State | What you see | Next action |
| --- | --- | --- |
| Open | A running house and placement controls | Walk, or place a prop |
| Select and change | An asset picker and selector, X/Z/size fields, and lantern colors | Add, move, resize, remove, or change light |
| Describe | A short request such as “Add a big pumpkin to my left” | See the result, keep walking, or cancel |
| Invalid or unclear | A short explanation; the working house continues | Correct the coordinates, use a control, or describe one clearer change |
| Changed | The same walk, with Undo and Redo | Keep the change or undo it |
| Saved | “Saved in Kiln” after the OPFS write | Reopen the scene or continue editing |
| Publish later | Local editing only | Public publishing is phase 4; no inactive publish control is shown |

Shader source editing is under **Advanced**. Ordinary scene edits do not invoke
Kiln's general coding loop, rewrite source, or compile a shader. Tint changes
update a uniform; procedural object changes update geometry; kit edits update instances. New behaviors or shader programs
belong in an explicit authoring workflow, rather than the placement interaction.

## Natural requests and defaults

Both labs treat omitted preferences and phrases such as `your choice`, `anywhere`,
`surprise me`, `wherever it fits`, and `up to you` as choices the editor can make.

- A generic prop or decoration becomes a pumpkin in Haunted House or a barrel in
  Medieval Town. Generic town furniture becomes a chair; generic buildings use
  cottages and townhouses. Explicit asset names stay explicit.
- Automatic placement finds clear floor space near the player, then searches the
  rest of the world on the same floor. It avoids walls, the player, and other props.
  Buildings use clear outdoor plots. Explicit coordinates and directions take priority.
- A singular request adds one object. `A couple` means two; `a few`, `some`,
  `several`, and an unnumbered plural mean three. Literal counts stay exact, with
  a limit of eight per request. Unsupported counts fail before anything changes.
- Delegated sizes use normal size. Delegated lighting uses warm light; delegated
  ghost movement uses floating. A room or wing with no location preference uses
  a nearby wall where the extension fits.
- Moving a selected prop `somewhere else` uses the same clear-space search.
  Each change saves in Kiln and uses one undo step. Unsupported objects or actions and negated
  requests leave the scene unchanged.

Run `npm run verify:natural-defaults` through `openagents browser run` with
`HAUNT_URL` set to the local preview to test natural phrases in both labs.

## Combined requests

- Haunted House: `add 3 ghosts and make them chase me`, `add 2 pumpkins and 3 candles`,
  or `add 2 ghosts, turn the light blue, and make them float`.
- Medieval Town: `add 2 houses and put benches outside`, or
  `add 2 cottages and put 2 benches outside each house`.
- `Them` and `it` can refer to the newest group created in the request. Existing
  ghosts keep their behavior unless you explicitly address all ghosts.
- Outside props go beside the new houses, with clear entrances. An unnumbered
  plural adds one per house; explicit counts are totals unless you say `each`,
  `every`, or `per house`.
- All parts use one decision call, scene revision, Kiln save, and undo step.
  If any part fails, none of the changes apply. Up to four parts and 32 underlying
  edits fit in one request. Use undo or redo on its own.

Run `npm run verify:compound` under `openagents browser run` with `HAUNT_URL` set
for browser checks of combined edits, references, saves, and failures.

## Run

From the repository root:

```sh
export CARGO_TARGET_DIR=~/work/openagents-target-1
. scripts/dev/cargo-build-env.sh
cargo build -p kiln --bin haunt-director
export KILN_OUT="$(openagents scratch)/haunt-kiln-site"
KILN_TRANSCRIPT=0 KILN_DEV=0 scripts/build-kiln.sh
# Load the existing server-only TypeSafe credential, or set TYPESAFE_API_KEY.
set -a; . ~/work/.secrets/typesafe.env; set +a
cd apps/haunt-preview
npm ci
npm run dev
```

Open the `127.0.0.1` URL Vite prints in current Chrome or Edge with WebGPU and
OffscreenCanvas. The Vite adapter starts `haunt-director` on a free loopback port
and stops it with the dev server. `HAUNT_DIRECTOR_BIN` can name another build.
Keep `KILN_OUT` set for `npm run dev` and `npm run build`.

The API key stays in server process environments; only the Rust SDK sends it. Do not put it in a `VITE_*`
variable. Direct controls and shader editing work without a decision credential;
text requests then report that the service is unavailable. Static `dist/` serves
the local controls; natural language requires this dev adapter or an authenticated
host integration. This change does not deploy a public decision endpoint.

## Placeable assets

Choose **Asset to add**, set X/Z/size, and use **Add asset**. The catalog includes
pumpkins, ghosts, gravestones (also tombstones or headstones), coffins, candles,
wooden crates, and green-potion cauldrons. These are original procedural meshes;
candles and potion have glowing surfaces, without adding dynamic light sources.
Every kind supports move, resize, remove, groups, undo/redo, and Kiln save/reopen.
Try `Add three ghosts to my right` or `Add a cauldron at (1, 3)`.

The renderer, picker, project validation, and Rust Jev questions use
[`src/assets.json`](src/assets.json). Existing pumpkin scenes still load, and the
old pumpkin command names remain accepted. Controls enable after the saved scene
opens, so an early edit cannot race restoration.

## Medieval Town Lab

Walk through either house's front doorway. The cottage has two ground-floor
rooms; the townhouse has two floors, with stairs along its right wall. Floors,
stairs, walls, aiming, and collision read the same saved plan. Your location names
the building, room, and floor. Try `knock down this wall`, `add a room to my left`,
or `add a small wing to my left`. Rooms attach to the aimed outside wall on your
current floor; a wing adds three connected rooms in one change. Wall removal
changes both visible kit pieces and collision. Added rooms have floors, ceilings,
kit walls, and connecting doorways. Undo refuses to remove an occupied upper-floor
extension. Upstairs furniture retains its height after save/reopen.

The eight placeable kit assets are cottage (`house`), townhouse (`manor`), barrel,
bench, chair, market stall, cart (`wagon`), and fountain. New houses immediately
get rooms and collision. `Add 2 buildings your choice anywhere u want` chooses
a cottage and a townhouse, finds clear plots, and saves both in one undoable edit.
`Anywhere`, `wherever there is room`, and `you choose where` let the editor find
space across town; explicit styles and counts still apply. Relative requests find clear outdoor plots; a single
explicit coordinate stays exact. Exact catalog aliases resolve in code, while Jev
checks the action, placement, and size. The town supports twelve houses, sixteen
rooms per floor, a 96 m boundary, and a 64 KiB scene file. Moving/resizing a house keeps its local plan.

To prepare Everglade's licensed **Modular Medieval Town** export, run:

```sh
npm run prepare:medieval
```

The Pillow conversion reads `~/.openagents/verse/private/medieval-town/kit-build`
and writes `haunt-webgpu` beside it. Override input with `HAUNT_MEDIEVAL_SOURCE`
or output with `HAUNT_MEDIEVAL_OUT`. Dev/build copies prepared files into ignored
`public/medieval-kit/`; licensed geometry, images, and captures stay outside Git.
`src/medieval-kit.json` pins ranges, dimensions, part roles, lengths, and hashes.
Review and update the pins together when changing recipes. Opaque source materials
ignore packed texture alpha; cutout materials retain it. Textures are 256 px.

Geometry and textures load once. Wall edits update kit instances and the small
floor/stair mesh without shader builds. Advanced WGSL edits affect ground, floors,
stairs, and extension roofs; the kit has its own fixed material. Missing bundles
draw wall proxies, preserve saved objects, and disable new kit placements. Both
renderers release their GPU resources on teardown.

Rust's `start_lab("/kiln/", "medieval")` selects `medieval/scene.json`,
`medieval/material.wgsl`, `kiln-medieval-workspace-v1.bin`, and
`kiln-medieval-base-v1.bin`. Existing Haunted House paths and `start_headless`
stay compatible. Both pages retain `hauntPreview` for test clients; Medieval Town
also exposes `townPreview`.

## House construction

- `knock down this wall` removes the aimed or selected wall. The target appears
  beside the room name in the viewport and in **House layout**. Use the wall
  selector or select a wall on the floor plan for an explicit target.
- `add a room` attaches a 6-meter room to that wall or opening. Directional
  requests such as `add a room to my left` choose a wall relative to your view.
- `add a wing to my right` attaches three connected rooms in one undoable edit.
  Small, normal, large, and huge sizes select 4, 6, 8, and 10 meters. The direct
  **Add room**, **Add wing**, and **Knock down wall** buttons use the same commands.
- Shared walls gain doorways. Floors, ceilings, visible walls, player collision,
  asset placement, and ghost routes read the same saved plan. Removing a wall
  opens the wall; adding a room supplies new floor beyond the house.
- Layout changes save to Kiln and undo/redo together. A change that would remove
  floor beneath the player or rebuild a wall through them waits until they move
  clear. Edits preserve player position, ghost behaviors, and the working shader.
- The bounded plan supports up to 16 connected, non-overlapping rectangular rooms
  inside ±64 meters. Existing saved scenes load the original three-room plan
  until their first layout edit. No build or shader compilation is needed.

## Ghost behaviors

- `make the ghosts float around` gives every existing ghost a local drifting and
  bobbing motion. `make that ghost chase me` changes the selected ghost only.
- Use **Float selected**, **Chase me**, **Stop selected**, or **Float all ghosts**
  for direct changes. Select a ghost in **Selected asset** first.
- **Stop selected** freezes its current position and saves that position. Behavior
  changes use the same Kiln transaction and undo/redo path, including a whole group
  as one change. Other asset kinds cannot receive ghost behavior.
- The browser simulates movement locally. Ghosts stay within the room floor plan;
  chasing ghosts slide along walls and stop short of the player. Simulation steps
  cap elapsed time to avoid jumps after a background pause.
- Saved scenes contain behavior rules and authored positions, not each animation
  frame. Reopening starts the rules from those positions. Ghost transforms update
  only their vertex-buffer ranges; changing float/chase rules does not rebuild
  geometry, compile WGSL, or create a GPU buffer per frame.
- Code resolves literal plural, selected, and explicit-ID references. Limited
  subsets such as “two ghosts” need selection rather than silently affecting all.
  Jev chooses the action and behavior in one bounded call.

## Decision path

- One Jev SDK call asks six independent Choice questions together: action,
  asset kind, placement, size, light color, and ghost behavior. Every question includes an unsupported option.
- Only the arguments used by the selected action contribute to its confidence
  floor (0.75 for this prototype). Uncertain requests make no change. This policy
  is exercised on a small retained example set, not broadly calibrated.
- Adding a group is one action: `add 5 huge pumpkins to my left` adds five
  size-2.5 pumpkins on the player’s left. Counts from 1 to 8 (digits or words)
  come from the request text. The group uses one transaction and one undo step.
  Placement spreads objects apart, avoids existing props and room boundaries,
  and reports insufficient space without partially adding a group.
- Code reads exact X/Z coordinates and `scale=` from source spans. Jev never
  invents numerical values. Relative positions use the current player pose when
  the result is applied. Existing objects use the selected or explicitly named ID.
- A result applies only to the project revision and selection it was requested
  against. A newer manual edit cancels a pending decision. No delayed decision
  may overwrite it.
- SDK budget: 2.2 seconds, zero automatic retries. UI budget: 2.7 seconds. The
  renderer and direct controls keep working while the request is pending.
- The local adapter and native endpoint check the editor origin, bound request
  sizes, and expose fixed questions rather than arbitrary prompts, tools, or models.

This follows TypeSafe's [function-calling pattern](https://docs.typesafe.ai/cookbooks/function_calling):
code owns the functions, the decision chooses closed-set values, and software
handles exact arguments and execution.

## Project and command contract

- Each lab scene is a schema-1 project with a stable project ID, monotonically
  increasing revision, material asset path, tint enum, and up to 64 stable asset
  IDs with X/Z/scale. Town furniture can also have a floor height; houses keep
  local room plans. `readProject` copies only saved fields and refuses unsupported
  versions, duplicate IDs, invalid assets, out-of-room coordinates, and invalid sizes.
- Manual shader saves, Original, and Kiln WGSL writes share this history. Undo and
  redo restore both the source file and preview. Empty or invalid drafts stay saved
  while the last working shader continues. Shader source and scene data each have
  a separate 64 KiB limit; saved projects also allow room for JSON escapes.
- GPU handles, Effect values, player pose, model responses, and undo history are
  outside saved project data. Inspection returns copies.
- A transaction carries its ID, project ID, base revision, and an allowed operation.
  Add, move, resize, remove, tint, shader source, replace, undo, and redo share `SceneHistory`.
  Batches contain at most eight project edits and validate atomically.
- The worker validates the targeted preview/token and command, prepares the change,
  and commits at a frame boundary. Failed commands preserve the project, history,
  player, shader, and device. IDs survive undo/redo; revision numbers never rewind.
- Undo/redo retain 32 recent changes; replay IDs retain 128 entries. Older replays
  also fail their base revision. The preview holds one in-flight scene change.
- Manual controls, Jev decisions, and real Kiln `write` edits share this command
  path. A whole-scene Kiln write must use the next revision. Accepted scenes save
  through the actual WASI `write` tool into OPFS. Workspace saves serialize with
  shader saves. Local scene reopen is supported; cross-tab collaboration is not.
- `hauntPreview.snapshot()`, `.command(operation)`, `.capture()`, and `.save(wgsl)`
  provide bounded inspection, transactions, PNG capture, and asset-draft validation.
  Capture runs in the worker with one encoding in flight. No evaluate-JavaScript
  command or account key is sent to that worker.
- A future session adapter must use existing Loro rooms on `coder-sync` and their
  grants. It carries project/transaction/base-revision identity, admits changes
  through the host, and forwards accepted data to this preview. Local undo is not
  a second collaboration authority, identity service, or storage backend.

## Worker asset imports

`AssetImportSession.importAsset` retains source bytes before converting them in
a worker. `updateSource` edits dependencies such as palettes and binary buffers
and rebuilds their accepted users. A failed conversion keeps the last accepted
GPU resources. Reopening recompiles the exact accepted source versions, including
when a newer draft is invalid. `acquireAssetImportSession` uses an Effect scope to
close the worker and GPU resources together.

The `OPENAGENTS_haunt` glTF 2 extension describes owned geometry and image cards.
Declare it in both `extensionsUsed` and `extensionsRequired`. Its version 1
geometry recipe contains a Haunted House project and an optional inline or
project-local palette. It reuses `houseGeometry`, preserving positions, normals,
palette colors, and glow. A card recipe names a local PNG/JPEG and dimensions;
its compiled quad includes UVs and an image material. The exact recipe and palette
types are in [`asset-owned-source-adapter.ts`](src/asset-owned-source-adapter.ts).
Recipes contain data, never executable code.

- Sources: 8 MiB per file, 64 IDs, and 32 MiB across retained original, draft, and
  accepted versions. Paths stay inside the project; imports never fetch URLs.
- Outputs: 16 MiB including mesh attributes and decoded images. Meshes have at
  most 65,536 vertices and 196,608 triangle indices; images use RGBA8 with at most
  4 MiB per image. Owned recipes have an 80 KiB limit and preflight geometry size.
- Worker: two conversions at once and a 16 MiB compiled cache. Bulk output buffers
  transfer ownership; cache entries keep separate copies. Diagnostics name the
  failed source, and cancelled or stale work cannot replace an accepted output.

Run the isolated Worker/OPFS/WebGPU adapter check without a model or account:

```sh
HAUNT_EVIDENCE="$(openagents scratch)/haunt-import-check" \
  openagents browser run -- npm run verify:asset-import
```

The check records source hashes, compile and total import time, transfer bytes,
cache counters, GPU readback, dependency edits, cancellation, reopen, and cleanup.
It uses the minimal runtime adapter; the reusable renderer remains in #12503.

## Check

Run `npm run verify:source-history` through `openagents browser run` with
`HAUNT_URL` set to the local preview to check shader undo and reopen in both labs.


```sh
npm run check
npm test
npm run build
HAUNT_URL=http://127.0.0.1:PORT/ \
  HAUNT_EVIDENCE="$(openagents scratch)/haunt-commands-check" \
  openagents browser run -- npm run verify:browser
```

The browser checks cover shader failures and recovery, 30 shader edits, manual
scene edits, 30 scene edits, real Jev decisions, interleaved Kiln/manual edits,
undo/redo, stale/canceled requests, timeout, OPFS reopen, origin checks, capture,
and teardown. `HAUNT_DECISIONS=0` skips only live inference; mocked delay checks
still run and are recorded separately. That option does not establish live acceptance.

See the [retained scene/decision results](../../docs/verse/verification/2026-10-22-haunted-house-commands/README.md).
Shader timing ends after first frame submission; screenshot timing is a conservative
observation bound. Neither is a GPU presentation timestamp. Resource counters cover
the demo's objects, not total driver allocations. The earlier Pylon source-patch
record remains historical evidence, rather than the current interaction.

Scene Undo covers objects, light color, behaviors, the house layout, and raw WGSL
source. Manual saves, **Original**, and Kiln writes use the same history.

Device reconstruction (#12503), full game parity, shared projects, and public
publishing remain follow-ups. The scoped browser-language exception is in
[the agent contract](../../AGENTS.md).
