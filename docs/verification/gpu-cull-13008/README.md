# GPU cull, request array and two-phase Hi-Z (#13008)

`report.json` is the headless WebGPU run of `/worlds/valley-lab/cull`
(Apple Metal 3, headless Chrome) on 2026-11-08. It passed.

What it checks:

- **The cut.** For 10 views of two DAGs built by `aethyr/render/geometry/dag.coax`,
  the GPU's per-cluster choice (`cl_lod` in `aethyr/render/geometry/cull.coax`,
  emitted as `apps/openagents-web/cull.wgsl`) is the CPU's `dag_draw` choice for
  every cluster. 0 clusters differ, and 0 lie within 0.1% of the tolerance.
  Views 4, 5 and 7 of the shell and view 1 of the occlusion scene mix levels.
- **Coverage from inside.** From five eyes inside the closed shell, over three
  frames each (Hi-Z on from the second), no pixel of the 960 x 540 target keeps
  the clear colour. Views 6 and 7 draw a cut that mixes levels 0 and 1.
- **Residency.** From nothing, the GPU's `atomicMax` requests drive
  `residency.coax` (through `apps/openagents-web/cullstream.coax`) with 30
  slots. The cut draws only installed clusters and stays covered at every frame
  with any page in. It reaches the full choice at frame 17. The residency keeps
  its rules (`res_valid`) on every frame.
- **Occlusion.** Behind a 16 x 12 m wall, phase 1 hides 40 of 71 clusters (view
  0) and 40 of 63 (view 1). Phase 2 retests them against the new pyramid, and
  all 40 stay hidden. The image is the image without occlusion, pixel for pixel.

Timing is wall time per frame with readback, 0.52 ms with occlusion and 0.48 ms
without. The scene is too small for occlusion to pay off, so these numbers show
only that the passes run. The cost they save is measured where the geometry is
heavy (see the follow-up for Everglade II).

To rerun:

```sh
coax/web/tool.sh coax/web/wgsl-module.coax --site
coax/web/tool.sh aethyr/tools/cull-fixture.coax apps/openagents-web/build/cull-fixture.bin
coax/web/wasm.sh apps/openagents-web/cullstream.coax apps/openagents-web/cullstream-face.c apps/openagents-web/build/cullstream.wasm
coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:PORT
coax/web/tool.sh coax/web/cull-check.coax http://127.0.0.1:PORT/worlds/valley-lab/cull OUT.json
```
