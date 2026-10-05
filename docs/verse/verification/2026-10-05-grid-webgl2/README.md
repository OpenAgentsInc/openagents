# The Grid on WebGL2 and GLES 3.0 (#10626)

Recorded on 2026-10-05 on a macOS (Apple silicon) contributor machine with
Google Chrome in headless mode.

## What changed

The engine renderer (`verse_pbr::imported`) that draws the Grid assumed a
WebGPU-class device. Admission (`imported/admission.rs`) now picks a
downlevel layout instead of refusing:

- **Pose block.** The skin pose is a uniform block. On GLES and WebGL2,
  which guarantee only a 16 KiB uniform block, it holds 254 bones
  (16,336 bytes) instead of 256 (16,464 bytes): the scene shader's
  `bones` array is cut and each actor binds only that range of its palette
  buffer. A pack model with more bones than the block holds is refused by
  name. WebGPU, Metal, and Vulkan keep 256 bones.
- **Shadow views.** Local shadows sample one `D2Array` depth texture; the
  cube-array requirement is gone. wgpu's GLES backend turns a square
  texture whose layer count is a multiple of six into a cube map, so on
  GLES the shadow textures get one unused extra layer and stay 2D arrays.
- **Local shadows on GLES.** GLES and WebGL2 cannot copy a depth texture,
  which the static shadow cache needs, so local lights draw without
  shadows there. The instanced pose path already turns off where storage
  buffers are missing.
- `device_profile` records the choice: `shadow_view_dimension`,
  `shadow_texture_layers`, `local_shadows`, `pose_bones`,
  `pose_block_bytes`, and `instanced_poses`. The browser Grid logs the
  profile to the console when it opens.

## Test mechanism

```sh
cargo test -p verse-pbr --lib imported::admission
cargo test -p verse --lib gles_tests::the_engine_scene_shader_translates_to_glsl_es_300
./scripts/build-everglade-web.sh /tmp/grid-web
cp crates/everglade-web/index.html /tmp/grid-web/
python3 -m http.server -d /tmp/grid-web 8765
```

Open <http://localhost:8765/?zone=grid&gl> to force WebGL2, or
<http://localhost:8765/?zone=grid> for WebGPU. The headless runs used:

```sh
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new \
  --enable-logging=stderr --window-size=1280,720 --virtual-time-budget=20000 \
  --screenshot=out.png [--use-angle=swiftshader --enable-unsafe-swiftshader] \
  [--enable-unsafe-webgpu] "http://localhost:8765/?zone=grid&gl"
```

## Results

| Run | Adapter | Result |
| --- | --- | --- |
| Before, WebGL2 | ANGLE SwiftShader | Error card: `Chamber requires cube-array shadow textures with 24 supported layers` ([before-webgl2.png](before-webgl2.png)) |
| WebGL2 | ANGLE (SwiftShader, Vulkan 1.3) | Grid drawn, no validation errors ([webgl2-swiftshader.png](webgl2-swiftshader.png)) |
| WebGL2 | ANGLE Metal, Apple M5 Max | Grid drawn, no validation errors ([webgl2-angle-metal.png](webgl2-angle-metal.png)) |
| WebGPU | Chrome WebGPU (Metal) | Grid drawn, unchanged layout ([webgpu.png](webgpu.png)) |

Console device profiles (excerpt):

- WebGL2: `"backend":"Gl"`, `"quality":"low"`, `"shadow_views":6`,
  `"shadow_view_dimension":"D2Array"`, `"shadow_texture_layers":7`,
  `"local_shadows":false`, `"pose_bones":254`, `"pose_block_bytes":16336`,
  `"instanced_poses":false`.
- WebGPU: `"backend":"BrowserWebGpu"`, `"quality":"medium"`,
  `"shadow_texture_layers":12`, `"local_shadows":true`, `"pose_bones":256`.

The GLSL ES 3.00 translation test covers the scene shader's `vs`, `fs`, and
`shadow_fs` entry points with the cut pose block: no extension, and the
shadows read through `sampler2DArrayShadow`.

## Not covered here

- The Android emulator's SwiftShader GLES (`-gpu swiftshader_indirect`) runs
  the same `Backends::GL` path; it was not rerun for this receipt. The step
  is in `NEEDS_OWNER.md`.
- Walkers seen from the browser need the browser's presence session
  (#10587); its receipt covers both WebGL2 and WebGPU.
