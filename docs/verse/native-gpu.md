# Native GPU Verse: Coax everything

Decided by the owner on 2026-10-30: Verse becomes a native desktop app that
draws on the GPU through Vulkan (Metal on Apple), and **Coax writes all of
it**, including the shaders. No browser, no Wasm, no WGSL, and no outside
shader toolchain (no glslang, naga, tint, Dawn or wgpu). The C host does
platform effects only, as AGENTS.md already says.

## Today and the target

Today a realm such as Everglade runs in a browser:

```
realm Coax ─▶ WebAssembly ─▶ coax/web/host.js (1,838 lines) ─▶ WebGPU
   + hand-written WGSL (30 files, ~660 KB; everglade.wgsl alone 3,292 lines)
   ─▶ Chromium (Dawn ─▶ ANGLE ─▶ Vulkan, in a separate GPU process) ─▶ driver
```

On `coderos-4080` that is a kiosk Chromium inside cage on tty8
(`~/everglade-kiosk/`), 60 fps on the RTX 4080. It stays until the native
app can replace it.

The target:

```
realm Coax ─▶ native C ─▶ one executable
   shaders: Coax procedures ─▶ SPIR-V words, written by Coax
   C host: window or screen, input, audio, files, sockets (+ the approved TLS)
   ─▶ Vulkan ─▶ driver ─▶ GPU            (Metal on Mac and iPhone)
```

What that buys: native speed and real threads for the realm, no GPU-process
hop, instant start, local assets, a boot session with no desktop, and the
realm and Psionic (also Coax, already running CUDA on this card) in one
process sharing one GPU.

## Phases

Each phase ends on a check that runs on our own machines. GPU checks on
`coderos-4080` run headless (offscreen images, no display, no VT switch),
so they never disturb a session someone is using.

| Phase | What | Exit check |
| --- | --- | --- |
| **G0** | A SPIR-V writer in Coax (`verse/gpu/spirv.coax`), and a headless Vulkan compute host in C (`verse/gpu/vk.c`) that runs a Coax-built module on the GPU | The GPU's output matches the same Coax function run on the CPU, pixel for pixel within one step of rounding; the module passes the Vulkan validation layers |
| **G1** | `coax spirv FILE`: the checked scalar f32/bool subset that `coax metal` accepts, lowered to SPIR-V functions. The lowering is Coax (`coax/lib/compiler/`); the C++ driver only hands over the tree and writes words | `verse/light.coax` on the GPU agrees with its C build over a grid of inputs |
| **G2** | Graphics: vertex and fragment entry points; the host draws offscreen, then to a window (a Wayland surface: the desktop app) and to the screen directly (`VK_KHR_display`: a boot session) | A frame read back from the GPU matches a golden image; the window runs at the display's rate |
| **G3** | The realm contract on Vulkan: the WebGPU half of `host.js` in C (vertex and uniform buffers, textures, asset packs, frame pacing, input, audio) | Star Catcher runs native with its light (`verse/kms/starcatch-light.coax`) on the GPU |
| **G4** | What shaders need from the language: vector types or a checked vector library, texture sampling, and declared GPU resources (spec section 15 already plans "shared GPU resource declarations") | Spec and checker updated; each new form has `#test` checks and a refusal for what it doesn't accept |
| **G5** | Everglade: `everglade.wgsl` and `everglade-fx.wgsl` rewritten as Coax procedures, pass by pass | Each pass matches headless-Chrome golden frames of the web build within a stated tolerance; Everglade runs native at 60 fps on the 4080 |
| **G6** | One shader source: Coax also emits WGSL (retiring `coax/web/wgsl.py`), so the web build draws from the same Coax; then the other realms move | No hand-written WGSL left in `apps/openagents-web/` |

The Mac desktop app follows the same host contract over Metal; `coax metal`
already emits the shading for the iPhone. Windows comes after, over Vulkan.

## G4 and G5: the shader subset, and Everglade in it

Everglade's shaders are 3,491 lines of WGSL (`everglade.wgsl`,
`everglade-fx.wgsl`). They use vectors and a 4x4 matrix, u32 math and
bitcasts, `for` and `while` loops, structs, storage buffers (u32, f32, a
struct, vec4f), a 2D-array texture, a depth texture with a comparison
sampler, explicit-level sampling, `textureLoad`, `discard` and
`front_facing`. Their entry points are bg, scene, shadow, grass, water
(transmit and emit), bloom, post and fx, plus the particles in
`everglade-fx.wgsl`. The plan keeps the language as it is (no new types in
the checker) and grows the lowering:

- **Vectors are a checked library**, `verse/gpu/shader.coax`: `V2 V3 V4 M4`
  as structs of f32, and procedures (`add3`, `dot3`, `mix3`, `m4_mul`...).
  They run on the CPU like any Coax, so every shader procedure keeps `#test`
  checks. Built-in `v2 v3 v4 m4` with operators (spec section 6.2) can
  replace them later without changing what the shaders compute.
- **`coax spirv` admits a shader subset** the way `coax cuda` admits its
  own: the lowering refuses what it cannot lower (E0704), and the checker
  runs its ordinary rules (no Metal profile). It is f32, bool, s32 and u32,
  structs and fixed arrays of those, views of numbers, if/else, `for` and
  `while` with break and continue, and acyclic calls. `int` is s64 (the
  `Int64` capability) so `.count` and loop counters need no casts, and it
  converts to u32 or s32 where a shader stores it.
- **Resources are parameters.** Coax has no globals, so an entry point's
  view parameters are its bindings, in order (storage buffers; the uniforms
  are a `[]f32`), and a helper takes the views it reads. The lowering binds
  each helper's view parameters to the entry's bindings at each call site,
  making a copy of the helper when two call sites bind it differently.
  Textures and samplers are small structs the library declares. Sampling,
  `textureLoad`, comparison sampling and `discard` are library procedures:
  they have CPU bodies for checks, and the lowering emits the GPU
  instruction instead.
- **Entry points are named on the command line**, as `--fill` is:
  `coax spirv FILE --vertex NAME --fragment NAME`. Builtins are positions,
  not names. A vertex procedure takes the vertex and instance index (u32),
  then optionally a struct of vertex attributes (locations in order), then
  its resources. It returns the clip position (a V4), or a struct whose
  first field is that V4 and whose other fields are the varyings, at
  locations in order. An integer varying is flat. A fragment procedure takes
  that V4 or struct (the first field is then the fragment position),
  optionally a bool (front facing), then its resources. It returns nothing,
  a V4, or a struct of one to eight V4s (its color targets). `--tree` prints
  the checked tree the driver hands the lowering, as decimal words.
- **G5 ports one pass at a time**: bg, then scene and shadow, grass, water,
  bloom and post, fx and particles. Each pass is a Coax file beside
  everglade.coax with `#test` checks. It is compared with a headless-Chrome
  frame of the web build: the same pack, page facts, clock and golden mode,
  and the same frame through `verse/native` (realm-host), so both draw the
  same scene. `coax/web/wgsl-coax.coax` translates the WGSL to Coax
  (`verse/everglade/gpu.coax`, `gpu-fx.coax`; `verse/gpu/wgsl.coax` is
  WGSL's vocabulary), so the live file stays the one source until the web
  build switches to Coax-emitted WGSL (G6). `verse/gpu/everglade-pass.coax
  HOST PASS` checks a pass. The live shaders hash with
  `fract(sin(x) * 43758.5)`, and sin of so large an argument is each GPU's
  own (WGSL bounds its error only on [-pi, pi]). So the check gives both
  sides one integer hash (`verse/gpu/int-hash.coax`, copies only) and
  allows a stated number of values past an f16's rounding: edge pixels
  whose test lands within float rounding. `--live` reports the live hashes
  too.
- **The scene host** (`verse/gpu`) does host.js's loadShaders, bindScene,
  postTargets and draw over Vulkan: the shadow map, the light and aux
  targets, the water copies, the bloom chain and post, then the HUD. The
  pack's texture layers are 27 JPEGs and 9 PNGs, so the decoders come too,
  in Coax.

## Rules for this work

- Shaders are Coax. A realm's light is a Coax procedure with `#test`
  checks, like `verse/light.coax`, and the GPU form is generated.
- The SPIR-V writer and the lowering are Coax. The C++ compiler driver
  only observes the checked tree and writes the words, as it does for
  `coax cuda`.
- The Vulkan loader is the platform's GPU API, like Metal. The Khronos
  validation layers are a test aid only: checks may load them, and nothing
  shipped does.
- Don't take the display on a machine someone is using. Headless first;
  window and screen runs only when the owner asks.

## Tracking

Every phase and native feature is a task in the Verse project
(https://openagents.com/projects/verse), seeded from
`apps/openagents-web/projects/verse/work/native-gpu.json`.

## Status

- 2026-10-30: plan written. G0 started: `verse/gpu/`.
- 2026-10-30: G0 compute passes on `coderos-4080`, headless. `verse/gpu/build.sh`
  then `vk-check 1920x1080` runs the 393-word Coax module on the RTX 4080:
  2,060,582 pixels exact, 13,018 one rounding step off (f32 to u8), 0 wrong,
  and 0 errors and 0 warnings under `VK_LAYER_KHRONOS_validation`.
- 2026-10-31: G1: `coax spirv FILE [--kernel NAME]`. The C++ driver hands
  the checked tree to `coax/lib/compiler/spirv.coax` (tree code v1), which
  writes the SPIR-V and passes the zero-survivor mutation gate. Kernels keep
  IEEE NaN and infinity (`SPV_KHR_float_controls`), and `pow` keeps C's
  meaning for a negative or zero base. `verse/gpu/light-check.sh coderos-4080`
  runs all 18 procedures of `verse/light.coax` on the RTX 4080 against the C
  build over 65,536 rows each, under the validation layers.
- 2026-10-31: G2 offscreen: `coax spirv --fill` (a full-screen pass) and
  `--stroke` (triangles from the realm's vertices, blended one plus one),
  drawn by `vkc_render` into an RGBA f32 image. Star Catcher's sky
  (`verse/kms/starcatch-light.coax`) is now three scalar channel procedures
  that the CPU and the GPU both call. Its star hash was `fract(sin(..)*43758)`,
  which scales sin's last-bit error by about 43,758 and put stars in different
  cells on the GPU (13,748 pixels off by up to 232/255). It is now the mod-289
  permutation over whole numbers, exact in f32 on any hardware. The sky has
  a different star field, equally random. `verse/gpu/frame-check.sh
  coderos-4080` steps the realm 120 frames at 1920x1080, as
  `verse-kms --headless` does, and draws it on the RTX 4080: 2,071,258 pixels
  exact, 2,342 within 1/255, 0 further, and 0 errors and 0 warnings under the
  validation layers. The window and the boot session are the next tasks
  (gpu-window, gpu-display-session).
- 2026-10-31: G2 window: `verse/gpu/vk-window.c`, run by `verse/gpu/window.sh
  HOST [--fullscreen] [--seconds N]`. An xdg-shell window (server-side
  decorations, resize, full screen) with a FIFO swapchain. Each frame it draws
  the offscreen pipeline (`vkc_target`, the passes frame-check uses) and blits
  it to the swapchain image. Keys come from the seat (evdev codes through
  `verse/kms/keys.h`, which kms.c now shares), and the pointer is hidden over
  the window. On `coderos-4080`'s desktop (coder-compositor, DP-1 at 59.951 Hz),
  Star Catcher ran windowed at 1271x1428 for 60 s: 3,599 frames (59.97 a
  second), 0 dropped where presented, 0 of 3,598 compositor frame callbacks
  late (longest interval 21.3 ms), and validation 0/0. Full screen at
  2560x1440 runs at about 59.2-59.7 a second, with a late frame (26-55 ms)
  about once a second. On those frames our own work is 0.2 ms, and the wait is
  for the GPU (the fence, 16-29 ms), which suggests another process's GPU work.
  The one candidate seen is Everglade's Chromium, which uses 24-40% of the GPU
  and is hidden behind a full-screen window. It was left running, so this is
  not confirmed. A high-priority queue (`VK_KHR_global_priority`) is not
  permitted for this user. The full-screen check is the boot session's job
  (gpu-display-session), where nothing else shares the display.
- 2026-10-31: G2 display session: `verse/gpu/vk-display.c`, run by
  `verse/gpu/display.sh HOST [--seconds N] [--demo] [--shot-at S]`. It switches
  to a free VT so the desktop pauses, takes DRM master, and hands the connector
  to Vulkan (`VK_EXT_acquire_drm_display`). It then presents with
  `VK_KHR_display` in FIFO at the mode's rate, through the same offscreen
  pipeline as the window. The window and the display share the swapchain
  presenter (`vkp.h`) and the realm-on-GPU code (`vkr.h`). Keys come from evdev,
  grabbed (`verse/kms/evdev.h`, moved out of kms.c). Escape, `--seconds`,
  SIGINT and SIGTERM end it. A root watchdog kills it and switches back if it
  outlives the limit by 20 s. On `coderos-4080` (RTX 4080, DP-1 2560x1440 at
  59.951 Hz, nobody using the screen), a 20 s demo ran 1,196 frames with
  0 dropped (longest 20.3 ms) and validation 0/0. `--shot` copies the image
  presented at 6 s out of the swapchain, and it shows the realm: the sky,
  moon, stars, basket and score. With a virtual keyboard
  (`verse/gpu/vk-escape.c`, uinput) pressing Escape at 8 s, a 30 s run ended
  "because Escape was pressed" after 420 frames. Both runs returned to VT 1,
  and coder-compositor resumed (it reopened the inputs and set the mode
  again). A grim capture from it afterwards showed the desktop as it was (no
  windows, the cursor). DRM cannot confirm what is scanned out meanwhile: the
  NVIDIA driver flips a leased display through its own modesetting, so the
  planes still name the desktop's last framebuffer. The evidence is the
  captured presented image, plus present timing locked to the display's
  vblank. Booting a machine straight into this (a session entry in the host's
  NixOS config) is not wired: that is a change to the host's config.
- 2026-10-31: G3 begins with Everglade's game natively: `verse/native/`.
  `build.sh everglade OUT` compiles the same Coax and the same browser face
  (`everglade-face.c`) with gcc instead of to wasm32. `realm-host.c` does
  what host.js does without the GPU: an arena, the asset pack read from a
  file and handed over as the browser hands it, page facts, keys, frames and
  actions. The face's pointers are 32-bit offsets, as in WebAssembly.Memory,
  so the build has no PIE, malloc stays on the brk heap, and the arena is
  mapped below 4 GB (`rh_off` refuses anything higher). The network imports
  are refused, as on a page that is offline. `check.sh HOST` runs both builds
  through 240 frames (the clock fixed, golden mode, a walk, a turn and a
  jump). `wasm-run.ts` runs the browser's wasm through the same steps. Every
  frame's 1,230-1,824 vertices, 190 draws, 104,576 grass blades and 126
  particles are the same bytes. Of the uniforms and stores, 206 of 238,394
  words differ, all within libm's rounding (largest 4.8e-7): glibc's
  `sinf`/`cosf` against wasi-libc's.
- 2026-10-31: G4: `coax spirv FILE --vertex V --fragment F`. The C++ driver
  hands the checked tree (code v2) to `coax/lib/compiler/shader.coax`, which
  lowers the subset above to a two-entry module. It passes 29 `#test` checks
  and the zero-survivor mutation gate. Every module the checks pin passes
  `spirv-val --target-env vulkan1.1`. It covers the interfaces, views of
  structs and arrays, conversions, loops, calls with per-binding copies, and
  each GPU procedure. `verse/gpu/shader.coax` is the vector, matrix and
  texture library, with CPU bodies and `#test` checks.
  `verse/gpu/shaders/quad.coax` uses every operator, conversion, GPU procedure
  and binding kind. `verse/gpu/vks.c` draws it: pipelines, textures,
  samplers, targets and depth. `verse/gpu/shader-check.sh coderos-4080` draws
  it on the RTX 4080 against the same procedures run on the CPU: 40,960
  pixels exact, 24,576 within 1e-6, 0 wrong (largest difference 2.98e-8),
  and validation 0/0.
- 2026-10-31: G5, the bg pass. `coax/web/wgsl-coax.coax` (a Coax tool)
  translates `everglade.wgsl` to 161 Coax procedures and `everglade-fx.wgsl`
  to 14, which check. The lowering gained u32 wrapping arithmetic, depth
  texture sizes, and a vector or struct `ifx` as a branch and an `OpPhi`
  (OpSelect takes no scalar condition for those before SPIR-V 1.4). Its
  gates pass: 31 `#test` checks and zero mutation survivors.
  `verse/gpu/everglade-pass.coax coderos-4080 bg --live` uses frame 90's
  2,300 uniforms from the wasm. Headless Chrome draws the WGSL, and the RTX
  4080 draws the Coax under validation, 0 errors and 0 warnings, at 320x180
  with two rgba16float targets. With one hash on both sides, 15 of 460,800
  values are past the tolerance, all on ridge-crown edges (40 allowed). With
  the live sin hashes, 11,749 are, all in the ridge and cloud band. The same
  Coax run on the CPU differs from the 4080 there too, so it is the
  platforms' sin, not the port.
- 2026-10-31: G5, the scene pass. A private variable only an entry writes
  (`paint_leaf_far`) is translated as the entry's own local, handed down by
  value. `verse/gpu/vk-passes.c` draws a spec of passes: buffers, the index
  buffer and the wasm's draw list, a mipmapped layer array, samplers, and a
  depth map one pass draws and the next samples. vks gained mip levels, draw
  lists, depth bias and a `less` comparison sampler. `golden/pass.js` draws
  the same spec. The browser decodes and mipmaps the pack's 36 layers as
  host.js does, and both sides sample those texels, with one linear sampler
  and no anisotropy (host.js's 8x is each GPU's own).
  `everglade-pass.coax coderos-4080 scene` draws the shadow map (2048
  square, bias 2, 2), then the scene's 189 draws, on the RTX 4080:
  validation 0/0, and 313 of 460,800 values past the tolerance, on
  silhouettes and one roof's mip edge (400 allowed). With the live sin
  hashes, 91,456 are past it, since the painterly noise hashes everywhere.
- 2026-10-31: G5, grass and water. Grass (the shadow map, then the blades):
  4 of 460,800 values past the tolerance. Water needed a local array behind
  a view (`wake_foam`, which `water_wake` writes): `coax spirv` keeps it in a
  Private variable that takes the array's value (SPIR-V leaves one
  undefined; WGSL zeroes it, and the first run's foam was garbage there).
  The water check is the frame host.js draws up to the post pass, at
  `#weir` (wasm-run `--hash`): the shadow map; bg (depth always, not
  written), the grass and the scene into light, aux and the depth; the
  light and depth copied; then each water draw's transmit (multiplied) and
  emit (added) halves, aux and alpha kept. vk-passes gained named targets,
  copies, draw ranges and those blends (vkc enables `independentBlend`).
  On the RTX 4080: validation 0/0, 999 values past the tolerance, on
  foliage edges and thin wave lines (2,000 allowed).
- 2026-10-31: G5, bloom and post. `everglade-pass.coax post` draws that
  frame, then host.js's five bloom steps (a quarter, then an eighth of the
  size, each with its own source and step uniform: spec `bind=NAME:RES`,
  `floats`, sized targets) and post_fs, the frame as it goes to the screen,
  its panels included. RTX 4080: 12 passes, validation 0/0, 1,369 of
  230,400 values past the tolerance, on bloomed foliage edges and the
  Ranger's silhouette (2,000 allowed).
- 2026-10-31: G5, fx and particles (G5 complete). `everglade-pass.coax fx`
  draws the water frame and then everglade-fx.wgsl's particles, translated
  as its own module: the smoke over what is behind (premultiplied, alpha
  under), then fire, glows and the shockwave added, soft against the water's
  depth copy. The frame is a Meteor Swarm 260 frames after its cast (srd5
  ruleset, 1 then a click on the road: 429 quads), since the still frames
  hold no particle on screen. WGSL `@interpolate(flat)` became a Coax field
  note, `#flat`, which the lowering decorates Flat (the particles' kind
  compares, interpolated, had striped the road). RTX 4080: validation 0/0,
  9,170 of 460,800 values past the tolerance, median 0.006 (half-float
  steps in stacked fire), 25 pixels off by more than 0.1 at billow edges
  (10,000 allowed); post with the particles, 14 passes, 659 of 230,400.
- 2026-10-31: the pack's images in Coax. `coax/lib/image/png.coax` and
  `deflate.coax` (8f1456e4d9), `jpeg.coax` (baseline, 1 or 3 components, 2x
  chroma, restarts, several scans; libjpeg-turbo's islow IDCT, fancy
  upsampling and fixed-point colour, and its leniency with damaged entropy
  data: 34 good and 65 refused files checked, 1,347 mutants, 0 survived), and
  `image.coax` (either format, 2x2 mips, the browser's upscale of a half-size
  layer and its premultiplied round trip; 0 survivors). All 27 pack JPEGs
  decode to PIL's bytes. `verse/gpu/layers.coax LIST OUT SIDE` writes the
  layers file in about 2.5 s; `everglade-pass --native-layers` draws the
  scene pass with them: 343 of 460,800 values past the tolerance (313 with
  the browser's decode, 400 allowed).
- 2026-10-31: gpu-realm-host, Everglade natively at 60 fps. `vk-spec.c`
  keeps a spec on the GPU: every buffer, texture, target and pipeline made
  once, the whole spec recorded into one command buffer, with named counts
  (`let NAME V`, `$NAME`) and `each draws:A:B` ... `end` for the water's
  per-draw halves. Targets and depths may follow the frame's size (`target
  NAME /4`, `depth NAME frame`). vk-passes and the browser's runner read the
  same format, and the post and water checks are unchanged (659 and 999
  past the tolerance). `everglade-pass live --emit DIR` writes the live
  WGSL's spec and modules with the layers decoded in Coax. `vk-realm.c` runs
  the realm's C (realm-host) each frame, writes its uniforms, stores (on a
  version change), particles and draw list into the spec's buffers in place,
  and presents the post target on the screen (`vkd.c`, the display
  takeover now shared with vk-display; `vkp_present`). `verse/gpu/realm.sh
  HOST check` draws 90 golden frames offscreen: the 90th is vk-passes'
  frame to the bit, validation 0/0. On the RTX 4080 the realm's step is
  about 2 ms and the GPU's frame 4.1 ms at 2560x1440 (6.9 ms at 3840x2160).
  `realm.sh HOST screen --seconds 25 --demo` ran on DP-1 at 2560x1440:
  1,433 frames in 23.9 s (59.94 a second), longest gap 21 ms, then back on
  VT 1; the presented image is the village, the grass, the shadows and the
  Ranger walking.
- 2026-10-31: the HUD and the pointer natively. The spec's last pass is
  host.js's HUD: the frame's own vertices through fx_vs/fx_fs (a pass
  option `vertices=NAME:FLOATS`, a vec4f attribute per four floats, and
  blend `max`, which vks and the browser runner both take); wasm-run dumps
  them as hud.bin. post with the HUD against the browser: 15 passes, 639 of
  230,400 values past the tolerance, validation 0/0. `verse/kms/mice.h`
  reads the mice (grabbed, relative motion kept inside the screen) as
  host.js's pointer and wheel, and vk-realm draws the pointer as a dot after
  the HUD. On the RTX 4080 at 2560x1440: 1,434 frames in 23.9 s (59.94 a
  second), 0 dropped, longest gap 20.3 ms; the presented image has the
  title, the reply and map buttons, the HP bar, the place labels and the
  "E: swing the hammer" prompt. Still to come: sound, and the realm's
  actions (navigation, sockets), which a bare screen ignores for now.
