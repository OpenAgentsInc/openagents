# Valley Lab: first hierarchy and simplification fixtures

Story: the unlisted geometry inspector opens `#hierarchy`, colors the real
ownership frontier of the original rock, and exposes small source/coarse
patches with real triangle counts and an exact error for their supported family.
`H` cycles levels; `F` cycles rock, planar fan and bent fan. Phone buttons
expose VIEW, LEVEL and NEXT SOURCE. The original asset/reference and leaf
views remain available, with the same source hashes.

`aethyr/render/geometry/hierarchy.coax` is the reusable Coax authority. Its
binary tree has explicit parent/child links, contiguous leaf ranges, summed
triangles, heights and union bounds. Odd tails are promoted without duplicate
ownership. A height-limited frontier partitions every leaf exactly once;
ascending each leaf a fixed number of parents would overlap uneven branches,
so the inspector uses the tested height rule. Node capacity is at most
1,023 for the existing 512-leaf ceiling. Malformed metadata or insufficient
capacity refuses before publishing any candidate.

| Rock source divisions | Leaves | Nodes | Root height | Triangles at every level |
| --- | --- | --- | --- | --- |
| 8 | 9 | 17 | 4 | 768 |
| 16 | 36 | 71 | 6 | 3,072 |
| 32 | 204 | 407 | 8 | 12,288 |

These rock parents are identity groups. They retain all source triangles,
with zero geometric error and no triangle savings. The vertex path still
decodes the checked leaves; its colors now identify their actual ancestor.
Yellow bars show the first frontier group's bounds. This establishes
ownership and bounds rather than representing a simplified parent mesh.

`aethyr/render/geometry/fan.coax` deliberately supports one exact family:
a 4 m square at y=0 with an interior apex at (0,h,0). The source has four
triangles; removing the apex yields two triangles without moving any boundary
edge or changing its segmentation. Both surfaces are graphs over the same
square. Matching points vertically bounds their symmetric surface distance
by |h|, and the removed apex attains that distance to the simplified plane.
Thus the geometric error is exactly |h| for the coarse patch and zero for
the unchanged source. The displayed bent fixture has h=0.75 m. This is an
analytic family-specific bound, not a general mesh error estimator. Tests
sample the piecewise affine graph as additional checks, not as its proof.
The inspector lifts patches by 1.2 m for visibility; translation preserves
the error and shared edges. Yellow bars retain the original source bounds.

Evidence:

- `realm-checks.log`: 451/451 checks pass. New integration checks exercise
  every hierarchy level at all three rock resolutions, nonoverlapping
  coverage and conservative bounds, actual index counts, arena/vertex/draw
  limits, fixture errors, and phone source/level controls. Existing walker,
  asset identity and decoded-reference checks remain in the same graph.
- `hierarchy-mutation.log`: all 119 authority root mutants killed. The
  hand-worked 1/3/5-leaf cases check exact ownership, odd tails, each bound
  axis, input refusal and preserved prior output. The 512-leaf case reaches
  the physical 1,023-node ceiling and refuses another leaf. Capacity itself
  bounds leaf count: required nodes are 2*n-1.
- `fan-mutation.log`: all 10 simplifier root mutants killed. Exact topology,
  positive winding/area, positive/negative/zero heights, graph-distance
  checks and mixed-resolution adjacent-edge samples cover the fixture.
  Immutable source/coarse topology tables are separate from the constructor.
  Neither mutation run uses equivalent annotations.
- `native-hierarchy.log`: native Coax build/report with every frontier's
  groups, triangles and covered leaves, plus exact fixture errors and all
  three unchanged original source SHA-256 values.
- `browser.log`: actual muted headless hardware WebGPU on desktop and phone
  viewports; distinct parent-level GPU readbacks, real 4->2 fixture changes,
  0/750 mm errors, H/F keyboard and actual phone touch, original/decoded lit
  reference-region equality, source hashes, controller regression, unlisted
  metadata and no console errors. `browser-first.log` retains a failed old
  six-view harness expectation; it was updated to traverse the new seventh
  hierarchy view before comparing the lit baseline.
- PNGs show actual hardware-rendered parent groups/bounds, the bent source
  and its coarse phone view. They are not edited images or mockups.
- `release-build.log`: 88,309-byte release Wasm under the unchanged
  160,000-byte cap. `artifacts.sha256` identifies source and artifact bytes.
  Realm generation, visual-source checks, whitespace and the repository
  tools gate pass. The initial scene/HUD shader is unchanged from the
  preceding cluster slice `fe5b2fc220`; its passing emission/fixture receipt
  is reused rather than rerun.

The existing 6 MB CPU arena declaration includes the tree, retained source,
bounded admission scratch and render buffers. Frame checks cover the high
rock profile and both fixture resolutions. Browser timings describe only
30 static coarse-fixture frames from CPU submission through GPU queue
completion; they are not GPU timestamps, route FPS or a 60-second valley
performance qualification. Phone viewports on this Mac do not qualify a
physical phone. The initial renderer adoption remains unchanged.

Toolchain: the same qualified Coax 0.50.0 binary as the prior slice, source
fingerprint `c82201e2cfff7c937fc0a29bf7010ae1ec75e7ab746ad29548bac7c2db61409e`.
The compiler's previous release bootstrap used `COAX_MUTATION=0`; this
slice's hierarchy and simplifier mutation gates ran separately as recorded.

Reproduce:

```shell
coax/bin/coax test apps/openagents-web/valleylab.coax --library
coax/bin/coax mutate aethyr/render/geometry/hierarchy.coax --library --root-only
coax/bin/coax mutate aethyr/render/geometry/fan.coax --library --root-only
coax/web/tool.sh aethyr/tools/valley-hierarchy.coax
coax/web/tool.sh valley-lab-check http://127.0.0.1:PORT
```

The [roadmap](../../verse/2026-11-05-valley-lab.md) advances to boundary-locked
parent simplification with conservative errors on a broader supported patch
class. Arbitrary rock simplification, screen-error selection, page residency,
streaming, saved walkthroughs, source import and shared/native renderer
qualification remain open. No book draft is marked complete.
