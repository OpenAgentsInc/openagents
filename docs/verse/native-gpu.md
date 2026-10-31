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
