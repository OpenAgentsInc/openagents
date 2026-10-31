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

