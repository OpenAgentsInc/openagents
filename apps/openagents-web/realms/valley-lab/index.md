# Valley Lab

An unlisted Aethyr workbench at `/worlds/valley-lab`, written in Coax.
Three stations build toward the full valley from independently checked pieces.

- **1 WALK** (`#walk`): the sandstone basin, arch, steps and block traveller,
  with Aethyr's fixed-step walker, capsule collision, camera and touch sticks.
- **2 ASSETS** (`#assets`): one original procedural rock, its provenance,
  licence, measured bounds, content hash, byte size and geometry counts.
- **3 MESH** (`#geometry`): the same indexed asset, triangle edges and bounds.
  It is the full-detail baseline for future virtualized geometry work.

WASD moves; Shift runs; Space jumps. Arrows turn the camera and the wheel
zooms. Left/right touch drags move/look, and a right tap jumps. R or RESET
returns to spawn; Escape releases input. The rock's walkable collision proxy
is its conservative measured bounding box.

Use the station tabs or keys 1–3. In inspection views drag or use arrows to
orbit, wheel to zoom, **V** to cycle lit/albedo/normals/edges/view-depth, and
**L** to change tessellation (768 / 3,072 / 12,288 triangles). On phones tap
VIEW or DETAIL. The same canonical native and Wasm mesh hashes are checked.

All art here is generated from original Coax source under Apache-2.0. No
Unreal assets are used. No cluster LOD, geometry paging, scene streaming or
encounter is implemented yet. The roadmap is
`docs/verse/2026-11-05-valley-lab.md` in the repository.
