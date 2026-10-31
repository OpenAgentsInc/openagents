# 85. One realm on every host

> **Status: unimplemented — design draft.** Today a realm runs in the
> browser through [`coax/web/host.js`](../../coax/web/host.js) and WebGPU.
> A native path is in progress ([docs/verse/native-gpu.md](../../docs/verse/native-gpu.md)):
> `verse/native/realm-host.c` runs a realm natively and `verse/native/check.sh`
> compares it with the browser build frame by frame; `verse/gpu/` writes
> SPIR-V from Coax and draws offscreen and to a window on Vulkan. Each host
> handles resize, focus and backgrounding its own way. This chapter plans one
> host contract for Aethyr. Nothing in `aethyr/` runs yet.

A realm should not know which host it runs on. The host owns the window or
canvas, input devices, audio, files and sockets; the realm owns its state and
its frames. This chapter writes that boundary down once, lists the events
every host must deliver, and checks that the same inputs give the same
frames on every host.

## What this chapter teaches

- The host contract: what the host does and what the realm does.
- The lifecycle events a realm receives: resize, focus lost, hidden,
  shown and leave.
- How to compare two hosts byte for byte.
- Which targets exist, and in what order they arrive.

## Outline

1. **The contract.** A realm is its world entry, its module, its assets
   and its save slot ([docs/verse/realm-host.md](../../docs/verse/realm-host.md)).
   The host does platform effects only: no graph, animation, layout or
   interaction policy, as `verse/platform/host.h` states.
2. **Lifecycle.** The browser host sends hidden and shown on
   `visibilitychange`, leave on `pagehide`, and `coaxial_blur` when the page
   loses the keyboard, so nothing stays held. A hidden realm stops stepping;
   the clock of [chapter 1](ch001-a-frame-has-a-fixed-step.md) clamps the
   frame when it returns.
3. **Size and density.** The host reports `vw`, `vh`, `dpr` and `coarse`
   (a touch pointer). A realm lays out in the safe area of
   [chapter 15](ch015-ui-and-the-hud.md) and never reads the window
   directly.
4. **Native parity.** `check.sh` builds the same Coax and face both ways and
   runs the same steps (Everglade: a walk, a turn and a jump; Hold the
   Stair: a wave). Vertices, draw lists, counts and particles must be the
   same bytes; uniforms within libm's rounding.
5. **GPU phases.** G0 a SPIR-V writer and a compute host; G1 `coax spirv`;
   G2 graphics offscreen, in a window and on a boot display; G3 the realm
   contract on Vulkan; G4 the shader subset; G5 Everglade's shaders in
   Coax; G6 Coax also emits WGSL so the web build uses the same source.
6. **Targets.** Browser (WebGPU), Linux desktop and boot session (Vulkan),
   Mac and iPhone (Metal), then Windows over Vulkan. A signed iOS app
   never loads new code.

## What exists today

- [coax/web/host.js](../../coax/web/host.js): the browser host and its
  lifecycle events.
- [verse/native/realm-host.c](../../verse/native/realm-host.c) and
  [realm-run.c](../../verse/native/realm-run.c): a realm natively.
- [verse/native/check.sh](../../verse/native/check.sh): native against the
  browser's wasm.
- [verse/platform/host.h](../../verse/platform/host.h): resident platform
  storage with generation-checked handles.
- [docs/verse/native-gpu.md](../../docs/verse/native-gpu.md): phases G0–G6
  and their status.

## What it builds

- `aethyr/host.h`: the versioned host contract in C, shared by every host.
- `aethyr/lifecycle.coax`: the events a realm receives and what each one
  does to the loop and to input.
- A parity check per realm, run for every pair of hosts it ships on.

## Acceptance checks and oracles

The chapter is done when:

- Every realm builds against `aethyr/host.h` and no realm reads a platform
  API directly.
- `check.sh` passes for every realm on the native host, not only
  Everglade, Hold the Stair, Belt and the water lab.
- Hiding a realm for 10 minutes and showing it again runs at most the
  clamped number of steps, and no key stays held after focus is lost.
- A resize from 1440x900 to 390x844 keeps the HUD inside the safe area.
- A recorded session replays to the same digest in the browser and
  natively ([chapter 5](ch005-replay-a-session-exactly.md)).

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (the clock clamp),
[4](ch004-input-becomes-typed-actions.md) (input from every device),
[5](ch005-replay-a-session-exactly.md) (digests),
[11](ch011-the-wgsl-renderer.md) (one renderer) and
[15](ch015-ui-and-the-hud.md) (the safe area). Coax:
[chapter 21](../../coax/book/ch21-targets.md) for targets and
[chapter 17](../../coax/book/ch17-gpu.md) for GPU programs.
