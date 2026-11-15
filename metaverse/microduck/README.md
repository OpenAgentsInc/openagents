# Microduck assembly viewer

A standalone research viewer on the OpenAgents Metaverse Engine:
`http://127.0.0.1:9123/demos/microduck/` after the commands below.
It shows the source STAND assembly at metre scale with orbit, zoom, reset,
three camera presets and a normal diagnostic. Drag to orbit, scroll to zoom,
use the arrow keys to turn, **R** to reset, **N** to toggle normals, and
**1 / 2 / 3** for the default, side and overhead views.

This is static geometry inspection. Joint animation, dynamics, learning and
hardware control remain the work in the [pipeline specification](../../docs/nvidia/microduck-pipeline-2026-11-15.md).

## Source and rights

Original mechanical geometry: **Pollen Robotics**,
[microduck_rl](https://github.com/pollen-robotics/microduck_rl), revision
`2b581c641406a48346e696212930ea881c222c52` as recorded by the workshop.
The assembled GLB comes from the official
[NVIDIA DLI Microduck Workshop](https://github.com/NV-DLI/Microduck-Workshop),
revision `b283d134f52db75f747581f9ed47f64f3f94df7b`, file
`web/assets/robot/microduck.glb`. Its SHA-256 is
`eea0931fefde8b2d352eaaa73b5e380f2790f63fb716a8c1a2ce219ffe58c1ac`
(11,893,404 bytes).

Pollen's official [Hugging Face simulator](https://huggingface.co/spaces/pollen-robotics/microduck-simulator)
was also inspected at `efedf47f4d4ae52d17ad2e108a5b8795d41537a3`.
Its smaller GLB is a library of loose STL parts; the simulator assembles them
using its robot description. The workshop export already contains that assembly
and presentation base colours, so the viewer imports that complete scene.
No downloaded scripts are executed.

The geometry, converted engine pack and robot screenshots are
**[CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/)**,
with credit to Pollen Robotics and NVIDIA DLI. The
[model terms](https://github.com/pollen-robotics/microduck_rl/blob/develop/LICENSE-MODELS.md)
and [workshop notice](https://github.com/NV-DLI/Microduck-Workshop/blob/b283d134f52db75f747581f9ed47f64f3f94df7b/web/assets/robot/NOTICE.md)
apply independently of repository software licences. This geometry is for
noncommercial research; the proposed saleable platform needs its own cleared
geometry. The model and pack are kept outside the public website's asset tree.

## Import and ownership

The shared registry contains the immutable asset
`pollen-robotics/microduck-workshop/microduck`, revision
`901495a8b4bf10bc2342b3ce6af4a9c26fbb1d9d68be09ad7fb9a46d99e1caf5`.
The package snapshot is
`c75411f7ee53972ed822a8ae802eb6bab35028999a38e4885d1459d38929407d`.
Its portable [inventory](../../assets/registry/packages/microduck-workshop.files.json)
pins the original GLB, model terms, workshop notice and source receipt. Originals
and content-addressed objects live under `~/.openagents/assets/`; no app-private
source model is committed. The registry's general runtime profile remains
`unqualified`: this one viewer does not qualify animation, physics or every host.

[`microduck-pack.coax`](../tools/assets/microduck-pack.coax) checks the source
digest before calling the existing engine `pi_glb` importer and `pack_write`.
It flattens the source node transforms into the engine's 48-byte static vertices,
retains metre coordinates, normals and material base colours, and adds a neutral
one-pixel layer for the browser's shared scene layout. It does not simplify the
mesh. The resulting assembly has **861,777 vertices and 796,802 triangles**,
including repeated instances of the source's 38 meshes across 70 nodes.
The viewer supplies its own studio lighting; source metallic/roughness and
clearcoat parameters are not reproduced.

The VPK1 pack is 50,927,404 bytes, SHA-256
`44a2b852208c8a29d17851e2a99401d257c2f711bc59cf92fe8b4a4c20bd681e`.
[`pack-policy.coax`](pack-policy.coax) admits that exact size and digest before
the shared browser host unpacks or uploads any geometry. This uses the existing
[pack gate ABI](../../coax/web/PACK-GATE.md), not a new wire format.

## Build and view

Use an installed, qualified Coax compiler. Fetch main's built pages first; if
the current head has no passing page build, fetch the nearest stored ancestor
reported by `coax/remote/pages.sh nearest origin/main`. Build only this viewer:

```sh
coax/remote/pages.sh origin/main
coax/remote/remote.sh run -- coax/web/tool.sh metaverse/tools/assets/microduck-pack.coax --fetch apps/openagents-web/build/microduck.pack
coax/remote/remote.sh run -- coax/web/wasm.sh apps/openagents-web/microduck.coax apps/openagents-web/microduck-face.c apps/openagents-web/build/microduck.wasm
coax/remote/remote.sh run -- coax/web/tool.sh coax/web/wgsl-module.coax apps/openagents-web/gpu/microduck.coax apps/openagents-web/microduck.wgsl apps/openagents-web/gpu/microduck.module
mkdir -p ~/.openagents/verse/private-build/web
cp apps/openagents-web/build/microduck.pack ~/.openagents/verse/private-build/web/microduck.pack
OA_SERVE_NO_API=1 OA_SERVE_NO_CODER=1 BROWSER=/usr/bin/true coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9123 --build /usr/bin/true
```

`--build /usr/bin/true` keeps this inspection server from starting the whole
site build when a file changes. Rebuild the viewer remotely with the commands
above. The `/private/` route is local development only; a static deployment
does not serve the licensed pack. `--fetch` can reconstruct the exact pack
on a machine without the imported source cache; it checks the same source pin.

Capture without opening a browser window or playing audio:

```sh
coax/web/tool.sh shot http://127.0.0.1:9123/demos/microduck/ /tmp/microduck.png 1440x1000 3000 --keep
```

The first shot in a fresh checkout can omit `--keep`; every following shot
reuses the shared headless browser. Verification and the rendered screenshot
are recorded in [the viewer receipt](../../docs/verification/microduck-viewer-2026-11-15/README.md).
