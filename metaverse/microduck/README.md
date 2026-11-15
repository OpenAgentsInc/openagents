# Microduck assembly viewer

A standalone research viewer on the OpenAgents Metaverse Engine:
`http://127.0.0.1:9123/demos/microduck/` after the commands below.
It shows the source STAND assembly at metre scale with orbit, zoom, reset,
three camera presets and a normal diagnostic. Drag to orbit, scroll to zoom,
use the arrow keys to turn, **R** to reset, **N** to toggle normals, and
**1 / 2 / 3** for the default, side and overhead views.

This workbench adds visual articulation: tap a part or use the floating menu's
**Parts / Joints** pickers. **Pose** changes a selected joint by five degrees
within its source limits; **Stand** restores the recorded angles. **View →
Explode** separates parts without changing the pose, and **Reset** restores
the original camera, pose and selection. Brackets choose joints, `+` / `-`
change the pose, and `X` toggles explosion. The highlighted line shows the
selected joint's rotation axis. These are visual poses; whole-body dynamics, learning
and hardware control remain separate work in the
[pipeline specification](../../docs/nvidia/microduck-pipeline-2026-11-15.md).

## Assembly film

**Animation → Play assembly + walk film** runs a smooth 18-second sequence:
assembly, eased explosion, reassembly, restored stand, then illustrative
walking. Shared component easing and the engine's gait feed the existing
source-joint pose/palette. The camera follows the visual root, and a foot-bounds
anchor keeps the lowest foot on its stand plane. It is a kinematic animation,
without trained locomotion or contact/balance physics. **Pause animation**
holds it; **Reset** restores the original scene.

## Illustrative servo

The floating menu now includes **Illustrative servo**: Baseline, Delayed
(32 ms), Limited (30 mNm), target/delay/effort controls, Pause, Reset and an
intentional negative-inertia refusal. Selecting another part/joint or a manual
pose leaves the experiment. Live GPU plots show actual shared Psionic target
and observed output (mrad), and requested/delivered effort (mNm). Other joints
and the base stay fixed. These are authored teaching coefficients, not measured
Microduck motors. The [finite protocol and clock](../../docs/protocol/servo-lab-v1.md)
and [observed native/browser evidence](../../docs/verification/microduck-servo-13640/README.md)
state the exact numerical and delivery scope.

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
It retains the source node transforms, stable names, exact mesh ranges and
world bounds alongside the engine's unchanged 48-byte vertices. It retains
metre coordinates, normals and material base colours, and adds a neutral
one-pixel layer for the browser's shared scene layout. It does not simplify the
mesh. The resulting assembly has **861,777 vertices and 796,802 triangles**,
including repeated instances of the source's 38 meshes across 70 nodes.
The viewer supplies its own studio lighting; source metallic/roughness and
clearcoat parameters are not reproduced.

The articulated VPK1 pack is **50,965,060 bytes**, SHA-256
`fc21cb95595d665bb83dc5b90c373dcf197f5b45b34b376c0d110b044bb408b5`.
[`pack-policy.coax`](pack-policy.coax) admits that exact size and digest before
the shared browser host unpacks or uploads any geometry. This uses the existing
[pack gate ABI](../../coax/web/PACK-GATE.md), not a new wire format.

Its catalogue includes one canonical **RBA1** rigid-assembly record, with the
[versioned contract](../../docs/protocol/rigid-assembly-v1.md) and independent
byte fixture in `proto/metaverse-engine/`. The old 1094 pack and feed retain
their identities. The same pinned workshop revision supplies:

- `web/assets/robot/components.json`, 52,622 bytes, SHA-256
  `06bb65fa1db2959215f686919940b8eeb8836254bb13fda0b70a939e3b3d25ef`.
- `lab/runtime/simulation/core/sensors/sources/robot_allcollisions.xml`,
  43,020 bytes, SHA-256
  `ec9d2f731596cfc847a7fc3898ef8b5e67a548928abc23216537f1da8af2729c`.

Verified originals are retained in the shared asset object's store. The
reviewed [source recipe](source.coax) binds all 70 source parts to the MJCF's
14 simulated joints and their parents. The beak is not a simulated joint in
this description. Display labels transliterate the source's middle-dot
separator to `/`; stable identities remain exact. The independent
[`microduck_oracle.py`](../tools/assets/microduck_oracle.py) reads the XML,
glTF and RBA1 separately, verifies every mechanical parent and limit, every
part/range/rest transform, and bounds over all 861,777 converted vertices.

Forward propagation uses the shared pose chain and palette. Source geometry
is already in the world-rest frame, so each rigid delta rotates about its
recorded world-rest pivot, then follows its parent's delta. Explosion offsets
are presentation only. Picking uses the same transformed part bounds; it is
an approximate box hit, rather than a triangle-level surface test. The named
picker also reaches small or hidden parts.

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
cp apps/openagents-web/build/microduck.pack ~/.openagents/verse/private-build/web/microduck-lab.pack
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

## OpenAgents mobile

Home's **Microduck** button opens this same viewer through the app's native
Metal engine host. One finger orbits; pinch zooms. The floating menu offers
View, Pose, Joints and Parts, plus the model license and Close. The menu is
projected into the app's existing accessible grayscale components; the browser
projects the same [Coax recipe](../../docs/protocol/view-menu-v1.md) into
keyboard-accessible controls. The mobile captions retain source attribution.

The app installs the small Coax producer and Metal shaders. The 50,965,060-byte
pinned assembly downloads on first opening and stays in the shared verified
world cache. The signed inventory and installed producer fingerprint prevent
an update from substituting a different native program. The optional full-pack
digest gate rejects altered geometry before parsing/upload, on both hosts.
No GLB source copy or model geometry is bundled into the app. This is an
articulated research viewer, with the model's CC BY-NC-SA terms above.
