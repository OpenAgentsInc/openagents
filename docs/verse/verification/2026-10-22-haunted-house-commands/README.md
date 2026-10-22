# Haunted House commands and Jev verification

- **Scope:** the scene-command proof in #12502, following the owner's request to replace the slow coding loop with constrained scene actions. Raw WGSL draft/undo integration remains on that broader issue.
- **Implementation and creator flow:** [`apps/haunt-preview`](../../../../apps/haunt-preview/README.md).
- **Setup:** Apple M5 Max, 128 GiB RAM, macOS 26.4, Chrome 155.0.8059.85, Apple `metal-3` adapter, 834 × 665 render pixels at device scale 1.
- **Ready view:** [a real Jev edit through the UI button](ready.png).
- **Evidence:** [`receipt.json`](receipt.json), [12 real decision examples](decision-cases.json), and [the house still running after the deliberate timeout](preview.png). Browser keys, API credentials, and preview connection tokens are omitted.

## Measured results

| Path | Samples | Result |
| --- | --- | --- |
| Direct scene edit to submitted frame | 30 | 13.6 ms median; 14.0 ms p95 |
| Jev: add a big pumpkin to my left | 1 | 311.5 ms through frame application and Kiln save; 300.5 ms SDK call |
| Jev: move the selected pumpkin to (1, 2) | 1 | 279.4 ms total; 256.9 ms SDK call |
| Jev: make the lantern blue | 1 | 226.8 ms total; 215.6 ms SDK call |
| Additional UI-button smoke after rebasing | 1 | 753 ms total, rounded UI display; no separate SDK timing retained |
| Deliberately stalled browser request | 1 fixture | Stopped after 2,703 ms; the scene kept rendering |
| Shader regression: save to submitted frame | 30 | 13.0 ms median; 21.0 ms p95 |
| Shader regression: save to screenshot-observed pixels | 30 | 201 ms median; 234 ms p95 |

The live model reported `jev-1.14.0`. Each text request contains four Choice
questions in one SDK exchange. These end-to-end calls are individual
observations, not a model latency percentile. Frame timing ends after
`queue.submit`; it is not a hardware presentation timestamp.

The earlier [201.7-second Pylon source-patch exchange](../2026-10-22-haunted-house-webgpu/README.md)
remains historical evidence. It involved several coding-agent turns. Normal scene
edits now apply commands to data or uniforms; the browser did not compile a shader
or recreate the GPU device for any of the 30 scene edits or the real Jev actions.

## Acceptance

- Manual add, move, and resize work through the UI and the shared transaction path.
- Invalid positions fail without changing the accepted scene or history.
- A real Kiln WASI file write to `haunt/scene.json` enters the same command path and can be undone. Manual and model edits preserve stable IDs, pose, view, and device.
- Undo/redo preserve object identity and increase revision numbers. Unit tests check stale/replayed/foreign transactions, schema refusal, atomic grouped edits, history bounds, and preserving custom shaders during the v1 upgrade.
- Real Jev decisions add and move a pumpkin and change the light. Exact coordinates come from text spans; code validates the bounds.
- The 12-example service probe accepted eight explicit supported requests, rejected two unsupported/compound requests, and held two uncertain requests. One negation had the wrong top action but stayed below the confidence floor and could not execute. “Put a big pumpkin” was ambiguous between add and move and was held; “Add a big pumpkin” passed. This small probe is not broad calibration.
- A manual edit cancels a delayed decision. Its eventual response cannot overwrite the newer scene. Controls stay enabled during inference.
- A timeout leaves the scene unchanged and rendering continues. There is no automatic fallback to the coding agent or retry loop.
- Kiln OPFS save/reopen round-trips the project without GPU handles, model data, or player state in the saved representation.
- Foreign origins receive HTTP 403 from the credentialed decision route.
- Bounded GPU readback produces a nonempty PNG; its temporary buffer is released. Teardown leaves zero owned buffers or active pipelines. No uncaught browser errors remain.
- Shader syntax, binding, and empty-draft failures still retain the working shader; movement, repair, rapid saves, and unsupported-WebGPU handling pass.

## Checks

- `cargo test -p kiln`: 44 passed, including a real SDK transport fixture with fixed questions, unused uncertainty, and unknown-action refusal.
- `cargo build -p kiln --bin haunt-director`; Kiln Wasm/WASI build.
- `npm run check`; `npm test`: 9 passed; `npm run build`.
- `HAUNT_URL=... HAUNT_EVIDENCE=... openagents browser run -- npm run verify:browser`: both browser scripts passed, including real Jev requests.
- Both workspace dependency generators, dependency-target normalization, and `scripts/dev/check-workspace-hack.sh` passed after adding the SDK dependency. Stale generated dependency lines were regenerated.

Public hosting, collaboration, device reconstruction, arbitrary behavior edits,
and general free-form authoring remain separate phases. The local native service
holds the TypeSafe credential; the renderer never receives it.

## Multiple pumpkins regression

The exact UI request `add 5 huge pumpkins to my left` now adds five size-2.5
pumpkins in one transaction. The browser check verifies left placement, a real
Kiln save, one-step undo/redo, and unchanged player position, device, and shader.
Counts come from the request text; the action definition accepts a group as one
action. See [the receipt](pumpkin-group-regression.json) and
[the resulting editor](five-huge-pumpkins.png). The full editor check also passed.

## Expanded asset catalog

Six original procedural props join pumpkins: ghosts, gravestones, coffins, candles,
crates, and cauldrons. The shared catalog drives the picker, validation, rendering,
and the Rust service’s fifth Choice question (asset kind).

[The browser receipt](assets-receipt.json) records real UI edits and real Jev
requests for every new kind, a three-ghost group and one-step undo, unsupported
asset rejection, unchanged player/device/shader, and Kiln OPFS save/reopen.
Those six text edits took 151.6–202.2 ms in this run; the group took 152 ms.
The [gallery](assets-gallery.png) comes from a separate isolated warm-light check
that also passed manual editing and save/reopen.

Validation: 44 Kiln library tests, 12 project/protocol tests, TypeScript check,
production build, and all three browser scripts. Existing pumpkin scenes and
legacy command names remain accepted. Startup controls now wait for scene restore.

## Ghost float and chase behaviors

`make the ghosts float around` changes every ghost in one reversible transaction.
`make that ghost chase me` changes the selected ghost. `make that ghost stop moving`
freezes and saves its current X/Z position. Direct controls use the same commands.
Literal target references are resolved in code; six independent Jev questions
choose action and applicable arguments in one request. No inference runs per frame.

The [behavior receipt](behaviors-receipt.json) records the exact UI requests at
149.7, 154.9, and 136.4 ms, including Kiln save. It verifies walking, single/group
undo, batched stop positions, 30 behavior edits without geometry rebuilds, real
OPFS reopen, and continued simulation with the last working shader after a bad
WGSL edit. Unsupported behaviors and non-ghost targets leave the scene unchanged.

The captures [before drift](behaviors-float-a.png) and [after drift](behaviors-float-b.png)
use fixed lighting and an unchanged camera: 33,656 pixels changed from mesh movement.
The worker updates reusable vertex ranges for ghosts, keeps two owned GPU buffers
and one pipeline, and releases them at teardown. Authored scene data does not change
per frame; reopening resumes rules at authored positions.

Checks: 45 Kiln library tests, 16 project/protocol/simulation tests, TypeScript check,
production build, the full four-script browser suite, and the final focused behavior
check including batched stopping. Unit simulation checks include chasing through
the connecting corridor, stop distance, wall bounds, and capped elapsed time.

## Room, wing, and wall editing

The [layout receipt](layout-receipt.json) records real UI requests: `knock down this
wall` (180.7 ms), `add a room` (137.6 ms), and `add a wing to my right` (139.0 ms),
including real Kiln saves. A wing adds three connected rooms as one transaction.
The [expanded house](expanded-house.png) and [floor plan](expanded-floorplan.png)
show the resulting seven-room test scene after one additional direct room edit.

The browser check verifies player traversal through the new doorway, rejection
of undo beneath the player, asset placement and ghost behavior in the extension,
save/reopen, wall targeting, one-step undo/redo, shader failure recovery, unchanged
player/device during edits, and bounded resource teardown. The same saved plan
drives geometry, collision, and doorway routes for chasing ghosts.

Checks: 45 Kiln library tests, 22 project/protocol/simulation/layout tests, TypeScript
check, production build, and all five browser scripts. Unit cases cover shared-wall
ownership, room bounds/overlap/connectivity, doorway picking, wing atomicity, ghost
routes into new rooms, and wall removal remaining open after an extension.

## Everglade medieval kit

This earlier asset-only stage has moved into **Medieval Town Lab**; see the
[walkable lab results](#medieval-town-lab) below. Haunted House no longer offers
these assets.

The [medieval receipt](medieval-receipt.json) records eight textured assets from
Everglade's licensed **Modular Medieval Town**: cottage, townhouse, barrel, bench,
chair, market stall, cart, and fountain. Our house recipes assemble the actual
kit pieces. A 32 m courtyard adds outdoor floor and a doorway to the existing
house, with the same save, undo, and redo behavior.

All eight kinds pass real UI add/move/resize/remove/undo/redo and Jev requests.
The final kit requests take 135–220 ms including Kiln saves; the house-alias
request takes 134.7 ms. Exact catalog aliases resolve in code, with the action
still checked by Jev. The owner-visible cottage request takes 217 ms.

The bundle contains 19,599 triangles, 17 base-color texture layers, 2,822,256 B
of vertices, and a 2,159,869 B atlas PNG. File identities and dimensions are
committed; licensed geometry, textures, and captures stay outside Git under
`~/.openagents/verse/private/medieval-town/` and the task's scratch directory.
The [preview README](../../../../apps/haunt-preview/README.md#placeable-assets)
describes preparation, licensing boundaries, and current prefab limitations.

The browser check walks into the courtyard, draws the full-size cottage and
props, makes 20 edits without geometry rebuilds or extra GPU resources, and
reopens the scene through real Kiln OPFS. A missing bundle disables new kit
placements while preserving saved kit entities. Editable-material counters remain
separate from the fixed kit material: two house buffers, two kit buffers, one
editable pipeline, one kit pipeline, one depth texture, and one kit texture array.
Teardown releases both renderers' resources. Shader failure recovery, ghost
behaviors, room/wing/wall changes, and the original editor checks also pass.

Checks: 46 Kiln library tests, 23 browser project/protocol/simulation/layout tests,
TypeScript check, production build, and all six browser scripts through isolated
`openagents browser run` profiles. The final asset and medieval checks run again
after exact-alias resolution and the missing-bundle save preservation change.


## Medieval Town Lab

The [town lab receipt](town-lab-receipt.json) verifies the separate `/medieval/`
page, fullscreen canvas, floating hotbar, and small asset/building popovers.
Haunted House keeps its seven spooky asset types; Medieval Town has eight kit
assets, daylight, enterable cottages, and townhouses with stairs and two floors.
Both use the same typed commands, with separate project IDs, material/scene paths,
and real Kiln OPFS snapshots. Old Haunted House saves remove medieval prefabs and
outdoor additions, preserve authored indoor rooms and props, and save the cleanup.

Real browser movement enters doors, stops at walls, traverses a removed interior
wall, enters a new room, climbs the townhouse stairs, and enters an upstairs room.
Undo refuses to remove the occupied upper floor. New cottages immediately get
rooms and collision; wings add three connected rooms in one undoable edit.
Upstairs furniture, layouts, and wall changes survive save/reopen. Invalid WGSL
keeps the working shader, scene, player, and device. The named worker and these
same storage/editing paths also pass against the production multi-page build.

Six real Jev requests, including saving, measure 163–284 ms: wall removal,
downstairs/upstairs rooms, a three-room wing, an upstairs chair, and a new cottage.
The existing five Haunted House browser checks pass, including 30 shader edits,
30 scene edits, ghost behaviors, room/wall edits, and OPFS reopen. Scoped Rust
checks pass 48 Kiln library tests; TypeScript and production builds pass; 30 unit
tests include doorway collision, stair ascent/descent, overlap rejection, floor
selection, undo, old-save cleanup, and refusing scene files over 64 KiB.

The kit converter now retains roof part ranges, admits three raw wall/door/window
pieces, and respects source material alpha modes. Textures and licensed captures
stay outside Git. The task scratch directory contains `town-lab-final/town/`
captures of the exterior, cottage interior, upstairs, and a newly added house.
The [creator README](../../../../apps/haunt-preview/README.md#medieval-town-lab)
describes the controls, private preparation, shared runtime, and limits.

## Automatic building choices

The [building-choice receipt](building-choices-receipt.json) records the exact
request `add 2 buildings your choice anywhere u want` and three paraphrases.
The exact request added a cottage and a townhouse on clear plots in 145 ms through
frame submission and Kiln save. Each request preserved player position and the
working shader, undid and redid as one edit, and reopened from Kiln. Negated and
mixed requests stayed refused. Run `npm run verify:building-choices` under
`openagents browser run` with `HAUNT_URL` set to the local preview.
