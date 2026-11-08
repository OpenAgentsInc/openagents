# GPU cull: pages streamed into GPU slots, shadow requests and texture demand (#13051)

`report.json` is the headless WebGPU run of `/worlds/valley-lab/cull` (Apple Metal 3,
headless Chrome) on 2026-11-08. It passed. It extends the #13008 run
([gpu-cull-13008](../gpu-cull-13008/README.md)), and every check from that run still passes.

What's new:

- **Pages come from VPK3 range fetches.** `aethyr/tools/cull-fixture.coax` writes each scene's
  geometry as a VPK3 file (`build/cull-0.vpk3`, 30 pages, 149,531 bytes; `build/cull-1.vpk3`,
  38 pages), one VPK3 page per DAG page. The page reads its manifest and then each page
  with HTTP Range. `apps/openagents-web/cullstream.coax` inflates each page in Coax, admits it
  by its SHA-256 (`ppk_admit`) and checks its layout before the residency hears it arrived.
  The run made 72 range fetches for the all-resident scenes, and refused 0 pages.
- **Installs copy bytes into GPU slots.** On install, Coax places the page for its slot. Its
  triangles' vertex indices move by the slot's first vertex, and each cluster's first triangle
  moves by the slot's first triangle. The host copies the triangles and vertices into the slot
  and the new first triangles into the cluster records. The shaders don't change. An evicted
  page's slot is reused, and its clusters are not drawn because their page is not installed.
- **Residency from nothing, all slots.** 30 slots for 30 pages: the views reach the CPU's full
  choice at frames 17, 0 and 0, every frame covered from inside.
- **Eviction.** 26 slots for views 5 and 7, which need 26 pages each and 28 together. Over five
  view changes, 32 installs and 6 evictions. Each view still reaches its full choice. Every
  frame from inside (view 7) is covered while slots refill. At the end of each view, the
  streamed slots draw the same image as the same pages all resident, pixel for pixel.
- **Shadow requests.** `cl_cull_shadow` runs an orthographic light view's draw rule and frustum
  over the installed pages, and writes page requests weighted by 0.25. It draws nothing and
  doesn't change what the camera draws. With both views, each page's request is the larger of
  the camera's and the light's. The light alone wants one page that the camera only keeps.
- **Texture demand.** Each cluster's box carries its texture's UV density (rounded to quarter
  octaves) and its texture id. Each drawn cluster writes `MIPS - mip` for its texture after
  the page entries, with atomicMax keeping the finest wanted. On all 10 views, the GPU's
  wanted mip for every texture matches `cl_mip` recomputed in f64 from the drawn set.

To rerun:

```sh
coax/web/tool.sh coax/web/wgsl-module.coax --site
coax/web/tool.sh aethyr/tools/cull-fixture.coax apps/openagents-web/build
coax/web/wasm.sh apps/openagents-web/cullstream.coax apps/openagents-web/cullstream-face.c apps/openagents-web/build/cullstream.wasm
coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:PORT
coax/web/tool.sh coax/web/cull-check.coax http://127.0.0.1:PORT/worlds/valley-lab/cull OUT.json
```
