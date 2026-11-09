# Experimental placed objects — #13030 handoff item 1

Baseline `e60054b85e`. This implements the first remaining item in the epic's
November 8 handoff. Closed subissues stay closed. Three supply crates near
spawn are independent world objects; the older baked village props remain
scenery. No catalog completion or full-SRD qualification is claimed.

A compatible aimed bolt with no creature target picks the nearest live crate
on its line and uses the existing `rs_cast_object` executor. Object material
AC, HP, typed damage, ignition, spell admission and action/component payment
remain in the shared SRD engine. A rejected cast rolls back the seat/executor
transaction and restores the temporary press budget and object coordinates.
Burning uses the battle's owned microclock in six-second rounds through
`rs_object_burn`. Class changes preserve object damage; reloading the local
world begins a new scene. Destroyed crates lose their mesh instance and
player/NPC movement collision. Labels show health and burning state.

## Geometry provenance

The mesh is the existing registered
`quaternius/fantasy-props/crate-wooden`, revision
`7b3a0340c09253f5cad302b46a53316affd387844335cc4dbc447120a0d5506b`.
The registered glTF accessors and index pattern were independently inspected
and matched to site 0 prop 8 in the current pack. No asset copy or repack was
made. The 1,426 captured vertices and 4,728 indices are reused as three
translated instances in one draw, with colliders from their measured bounds.
The creator reserves one draw slot before its bounded grass tail; the existing
44-draw capacity stays unchanged.

`verse/tools/assets/experimental_object_mesh.coax` checks the decompressed
pack's exact environment catalog row, captured vertex/index hashes and every
index's captured range. Runtime interaction requires the matching environment
first/count/word tuple. A pack regeneration must rerun the verifier and update
the pinned range if it changes. Runtime does not hash GPU geometry itself.

Vertex SHA-256:
`36c4af43d31ec59a40153419134603fa83700c632acefd9479b738ef473259d9`.
Index SHA-256:
`a44a323e48b0d2c413684960e66989db95c159b913d716c74f329c95ba7ea277`.

## Verification

The initial world regression failed before the adapter existed (the old path
only produced a cosmetic whiff). A subsequent fixture correction selected the
hotbar index rather than a catalog ID. Focused checks cover actual Fire Bolt
rolls; missing/stale scene metadata; direction/range selection; destruction;
compact instanced draws; burning's time boundary; class-change persistence;
and refusal preserving object HP/coordinates, RNG and saved action state.
The native witness uses the actual world adapter, not an independent rules
oracle. The packed-range checker has its own changed-byte rejection check.

Core SRD policy files are unchanged. This world/asset integration uses the
epic's focused-verification policy; no mutation exemption is introduced.

## Limits and next work

Only the new supply crates are interactive. Selection uses Experimental's
existing dungeon visibility facts; general village-wall cover is not newly
qualified. A creature target retains precedence over the empty-bolt object
branch, as specified by the handoff. Objects are local scene state, not room
persistence or multiplayer authority. The remainder of #13030's handoff is
Healer's Kit on a dying ally in Everglade II; Strength-derived controller
jumping; Find Familiar; Wild Shape form swaps at Long Rest; and the actual
Experimental Fireball origin-cover wall regression. Owner choices and the
levels 1–6 scope boundaries in the handoff remain in force.

## Results

- 51 `experimental combat` checks passed, plus seven `Experimental` checks
  including the four placed-object regressions. The creator draw reservation
  and mesh verifier's changed-byte checks also passed.
- `coax/web/tool.sh verse/tools/experimental-objects-native.coax` reported
  `PASS EXPERIMENTAL-OBJECTS`; the mesh verifier reported `PASS OBJECT-MESH`
  on the decompressed tracked `glade.pack.gz`.
- Release Wasm built at **2,641,156 bytes** within the **2,645,000-byte** cap.
- Muted headless Chrome on the Experimental route used hardware Apple Metal
  WebGPU at 1280×900. Starting with three crates, moving briefly toward the
  middle crate and pressing Fire Bolt six times yielded six casts, zero empty
  bolt whiffs, and zero refusals. The center crate disappeared; the other two
  retained their models, labels and health bars.
