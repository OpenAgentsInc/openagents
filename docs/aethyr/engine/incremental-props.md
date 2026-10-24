# Incremental authored props

Status: implemented browser slice for [issue #3](https://github.com/OpenAgentsInc/aethyrforge/issues/3).

Medieval Town and the owned courtyard use the same prop update path. Added catalog
props now block walking. Their collider uses the mesh bounds and local center,
including an off-center pivot, scale, and yaw. Old saves without an explicit
`collisionOffset` retain their previous floor-pivot collision convention.

## Authority and activation

The Rust authored document, commands, and undo history remain authoritative.
`prepare` validates a candidate without changing the live world. The renderer
prepares derived records and checks WebGPU error scopes. The worker then commits
the same Rust transaction and activates the prepared rendering synchronously.
A rejected transaction or discarded candidate leaves the previous world active.

`RuntimePropTable` assigns each authored ID a slot and generation. Deletion makes
the old handle unresolvable; slot reuse advances the generation. Superseded,
discarded, and activated plans reject access to their staging views. A caller may
borrow a matrix view only while `plan.ready()` is true; it must not retain the raw
typed view beyond that lifetime. Inspection returns copies.

The table stores matrices, collision bounds, and selection poses separately from
the authored document. Normal transforms derive one record and copy its changed
fields from reserved scratch storage. The GPU instance buffer reserves capacity
and receives a 64-byte write for one changed matrix. Membership changes update
draw references; draw ordering follows authored entity ordering. Capacity growth
copies the table and replaces the GPU buffer explicitly.

Rust shares unchanged room, building, and fixed collision outputs. Simple prop
edits replace only affected prop colliders. Building, room, wall, and material
changes use the complete compiler and renderer preparation path. Those fallbacks
preserve generation-checked prop slots.

## Current cost boundaries

This slice does not make an entire command constant-time. Rust still validates
all authored entities, clones authored history and map metadata, checks response
size, and serializes complete edit snapshots. The renderer still compares the
structural description and traverses draw references. Frame movement sends a
small player result rather than the complete authored world; React receives
bounded status summaries, not an atom per entity.

`status.runtimeTables` contains cumulative table counters, actual authored-matrix
GPU upload bytes, complete rebuilds, and incremental updates. `bytesAllocated`
and `bytesCopied` cover typed backing buffers only, including explicit inspection
copies. They exclude JavaScript objects, Rust allocations, browser internals, and
GPU driver memory. `status.kernelWork` describes the last accepted Rust update:
validated entities, rebuilt prop and collision records, and structural rebuilds.
`runtimeEditMetrics` separates kernel preparation, renderer preparation, kernel
commit, and activation. It excludes persistence, UI response, and presentation.

## Reproduce

Run `npm test`, `npm run check`, and `sh tools/build-kernel.sh --test`.
Run `tools/verify-props.mjs` through `openagents browser run` with an external
`AF_CAPTURE_DIR`. `AF_VERIFY_WORLD` selects `starter` or `medieval-town`; the
private town requires `AETHYRFORGE_PRIVATE_ASSETS`. Run again with
`AF_PROP_UPDATE_MODE=full` to force complete renderer preparation for the same
edits. This diagnostic mode keeps the same authoritative kernel and rules.

The browser check proves free walking lanes before adding a blocker, then checks
add, move, remove, occupied-edit rejection, undo, redo, and save/reopen against
collision, selection, and fixed images. A transform among 32 added props checks
bounded derived work and measures uploads. Raw captures remain outside Git.
The town probe uses an owned courtyard beside the source town; camera setup is
explicit and does not claim continuous traversal from the town entrance.

## Recorded result

The [scrubbed result](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/kaigen/results/issue-3.json) records 15 checks
per mode in the courtyard and 16 per mode in Medieval Town. Every recorded fixed
image matches byte for byte between incremental and forced-full preparation.
The occupied edit preserves the previous image, revision, and selection.

For one moved prop among 33 authored props, both worlds upload 64 matrix bytes
instead of 4,096. The incremental table allocates no typed backing buffer and
copies 108 bytes, compared with 4,096 allocated and 4,268 copied by the complete
matrix path. Rust validates 33 entities and rebuilds one prop collider in both
modes. The single Town edit takes 5.3 ms through activation versus 6.5 ms; the
courtyard takes 0.8 versus 0.9 ms. These are descriptive samples from one shared
Mac, not a latency distribution or frame-rate claim. Tests pass: 575 TypeScript
tests, 55 Rust kernel tests, type checking, the production build, and spec checks.
