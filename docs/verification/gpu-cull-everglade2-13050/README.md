# Everglade II: GPU instance culling with a two-phase depth pyramid (#13050)

`report.tsv` is the headless WebGPU run of `/worlds/everglade2` (Apple Metal 3, headless Chrome,
2400 by 1400 at dpr 1, every device pixel) on 2026-11-08, after the trees and grass only fade
(#13023). It passed. One line per view:

    VIEW DIFFERING_PIXELS GPU_OFF_US GPU_ON_US PIECES FRUSTUM DRAWN1 DRAWN2 HIDDEN KEPT PICKED

- **The same image.** At the glade, in the woods (`?at=200`) and at the clearing (`?at=470`),
  with the page held still, the screenshot with the cull on is the screenshot with it off,
  pixel for pixel (0 differing). The check takes on, off and on, and compares once the two
  with the cull on match, because textures and shadow tiles may still be arriving.
- **GPU time.** Each mode's median of six 2.5 s turns, measured by the host's pass timer from
  the first pass's start to the last one's end, compute included: the glade 8.6 to 7.6 ms
  (-11%), the woods 11.0 to 9.2 ms (-17%), the clearing 8.5 to 8.2 ms (-4%). The Mac was busy
  with a release build, so the times move by about 1 ms from run to run. Every run gave the
  cull a gain at all three views.
- **What it leaves out.** The glade: 4229 pieces picked, 465 outside the frustum, 1162 drawn
  by the first phase, 2602 hidden by the pyramid. The second phase draws 0 more while the view
  is still; as you move it draws the pieces last frame's depth hid wrongly.
- **60 FPS.** `realm-perf --only everglade2`: 60.0 FPS, p95 16.7 ms, in all three cases.

To rerun:

```sh
coax/web/wasm.sh apps/openagents-web/everglade2.coax apps/openagents-web/everglade2-face.c \
  apps/openagents-web/build/everglade2.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:PORT
coax/web/tool.sh coax/web/everglade2-cull-check.coax http://127.0.0.1:PORT OUT.tsv
```
