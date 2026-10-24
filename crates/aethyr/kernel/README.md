# Local preview kernel

This implemented slice owns player/camera state, a 60 Hz accumulator, walking
gravity/jump, conservative AABB collision, and local typed scene edits. It builds
as native Rust for tests and as browser Wasm. The JSON prototype bridge is
separate from the proposed [binary ABI](../../../docs/aethyr/engine/runtime-abi.md).

Run `tools/build-kernel.sh --test` from the repository root, then
`node tools/build-kernel.smoke.mjs`. The generated `public/kernel.wasm` stays
outside Git. Build intermediates use an external compiler/workspace-keyed target.

Exports are `memory`, `af_alloc(length)`, `af_request(pointer, length)`,
`af_response_len()`, and `af_free(pointer, length)`. Both request and response
are UTF-8 JSON. Reacquire `memory.buffer` after a request, then release both
slabs using their exact lengths. A zero pointer means the request could not run.

Requests use `type`: `init`, `step`, `set_camera`, `snapshot`, or `command`.
Step reports only bounded player state; explicit snapshots and edits return
the scene. Camera coordinates are right-handed, Y up, foot-relative, with yaw
zero facing -Z and eye height 1.62 meters. Fly mode supports vertical input;
walk mode applies collision and gravity. Camera changes preserve the scene/tick.

Walking uses Everglade's 6.4008 m/s run, 4.1148 m/s backpedal, 1.6× forward
sprint, 6.2 m/s jump, and 15.696 m/s² gravity. The town keeps its 0.3-meter
collision radius and 1.8-meter height. `step.input.turn` optionally turns at π
radians per second; the host can keep WASD strafing and mouse look. Jump input
is held state, with one jump per rising edge. Player summaries include measured
`velocity`, horizontal `speed`, `ahead`, `side`, and `verticalSpeed` for animation.

`set_static_collision` accepts `triangles`, an array of three XYZ vertices per
triangle. The default replaces the static mesh; `append: true` adds a validated
chunk. Send chunks below the 4 MiB message limit. The complete mesh is bounded
to 2,000,000 triangles, eight million spatial-index references, and 4,096 surfaces
that span more than 64 two-meter cells. The index is shared across staged edits,
and survives `init` and `set_camera`; a scene change must explicitly replace it.
The imported town's compiler supplies these local collision inputs outside Git.

The walker tests a vertical capsule against triangle faces and edges in bounded
0.08-meter movement substeps, slides along blocked axes, follows walkable floors,
and climbs steps up to 0.25 meters. `query_camera` takes `from`, `to`, and an
optional sphere `radius` (default 0.12 meters), and returns the clear `fraction`
of a segment up to 80 meters long. It checks static triangle faces, edges, and
vertices, authored boxes, and transformed building solids. The independent
browser character camera uses this query for immediate pull-in and eased return,
with a 0.25-second glide between first person and a third-person orbit.

`gameplay_init` installs up to 65,536 source cells and their support links;
`set_static_collision` optionally includes one `owners` value per triangle.
`gameplay_aim`, `gameplay_prepare_cast`, and `gameplay_cast` target and apply the
three Druid abilities. The renderer checks its complete prospective geometry
before the cast commits. Simulation removes destroyed cells from triangle
collision, collapses unsupported cells, and steps up to 256 moving fragments;
resting fragments can become solid after the player is clear. `gameplay_reset`
restores the source cells without changing authored scene history. The
[implementation record](../../../docs/aethyr/engine/implementation.md#local-druid-destruction)
describes the local scope, checks, and current limits.

Commands contain `commandId`, `baseRevision`, and one to eight typed operations.
Edits validate on a staged world; retries are idempotent. Undo/redo use inverse
operations, and rebuilding a wall through a walking player fails without losing
history. Rectangular rooms have a default south entrance and shared doorway
segments. `wallColliders` supplies the exact derived wall pieces for rendering.

An entity's optional `buildingPlan` owns local rooms, openings, support patches,
and stair ramps. Position, yaw, and uniform scale transform its collision and
floor heights together. Openings include their actual horizontal offset and
lintel height; upper-floor support omits the stair hole. Wall IDs include the
instance ID, so editing one cottage does not change another. Saved `removedWalls`
restores through `init`. Derived walls include `localMin`, `localMax`, and a
transform for drawing, plus conservative world bounds for picking. Optional
`init.buildings` supplies the same plan for standalone instances.

This is a local prototype boundary, not an authenticated publishing or
collaboration service. Its walker has a flat fallback ground, bounded static
triangle contacts, capsule-footprint/AABB/OBB contacts, and ramp-guided floor
support. Dynamic triangle collision, full semantic building conversion, navigation,
general rigid bodies, and the safe behavior VM
remain proposed work. Decorative imported house meshes do not become editable
interiors merely by enabling this kernel.
