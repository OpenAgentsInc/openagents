# 84. One realm on every host

> **Status: unimplemented — design draft.** Today a realm runs in the
> browser through [`coax/web/host.js`](../../coax/web/host.js) and WebGPU.
> The native Vulkan port has recorded all-realm evidence
> ([docs/verse/native-gpu.md](../../docs/verse/native-gpu.md#status)):
> `verse/native/realm-host.c` runs a realm natively and `verse/native/check.sh`
> compares it with the browser build frame by frame; `verse/gpu/` writes
> SPIR-V from Coax and draws offscreen and to a window on Vulkan. Each host
> handles resize, focus and backgrounding its own way. This chapter plans one
> host contract and recovery behavior for Aethyr. That common contract is
> not implemented; the native port is an existing baseline, not future work.

A realm should not know which host it runs on. The host owns the window or
canvas, input devices, audio, files and sockets; the realm owns its state and
its frames. This chapter writes that boundary down once, lists the events
every host must deliver, and checks that the same inputs give the same
frames on every host.

## What this chapter teaches

- The host contract: what the host does and what the realm does.
- The lifecycle events a realm receives: resize, focus lost, hidden,
  shown and leave.
- How a realm recovers when the GPU device is lost or audio is interrupted.
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

## Recovery: device loss and audio interruption

> **Design draft.** Not implemented.

A host can lose its GPU device or its audio output while a realm runs. The
browser reports a lost WebGPU device through `GPUDevice.lost` (after a driver
reset, a tab moved to another GPU, or memory pressure). An `AudioContext` can
move to `suspended` or `interrupted` (a phone call, another app taking the
audio session, or Bluetooth headphones disconnecting), and native hosts get
the same events from Metal, Vulkan, Core Audio and the iOS audio session. The
host contract adds two events, `gpu_lost` and `audio_interrupted`, with these
rules.

### GPU device loss

1. **The simulation doesn't depend on the GPU.** State lives in Coax memory,
   not in GPU buffers, so a lost device loses no state. Steps keep running at
   the fixed rate, and the realm keeps accepting input.
2. **The host recreates the device.** It requests a new adapter and device,
   then rebuilds every GPU resource from its source: pipelines from the
   emitted WGSL or SPIR-V, buffers and textures from the assets and the
   current state. A resource that can't be rebuilt from source is a design
   error that the check reports.
3. **Frames resume from current state.** The first frame after recovery
   draws the current step. No frame from before the loss is replayed.
4. **Recovery has a bound.** The host tries to recreate the device up to 3
   times, 1 second apart. If every try fails, it shows a message that the
   graphics device stopped and offers a reload. The realm's save is written
   first.
5. **Every loss is reported.** The frame report of
   [chapter 83](ch083-inspect-and-profile-a-running-realm.md) counts each
   loss, its reason and the recovery time.

### Audio interruption

1. **The audio clock pauses with the device.** While audio is interrupted, no
   blocks are rendered, and the audio clock of
   [chapter 68](ch068-sound-in-the-world.md) doesn't advance.
2. **The simulation continues.** Steps run, and sound events from those steps
   are not queued for later. A sound whose scheduled sample falls inside the
   interruption is dropped, so the realm doesn't play a burst of old sounds
   when audio returns.
3. **Music resumes on the beat grid.** When audio returns, the music restarts
   at the next bar line of the current section, computed from the current
   step.
4. **Resumption needs a gesture where the platform requires one.** If the
   browser requires a user gesture to resume an `AudioContext`, the realm
   shows a control to turn sound back on, and the control is reachable from
   the keyboard ([chapter 15](ch015-ui-and-the-hud.md)).
5. **Replay is unaffected.** The rendered stream of chapter 68 is defined by
   the session, so replay renders the dropped sounds. The live output and the
   rendered stream differ only during the reported interruption.

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
- A forced device loss (`GPUDevice.destroy()` in the browser, a device reset
  natively) during play is recovered within 3 seconds. The state digest is
  the same as in a run without the loss, and the first frame after recovery
  draws the current step.
- A device loss that can't be recovered saves the realm and shows the reload
  message, and the save restores the same digest.
- Audio suspended for 5 seconds during a session and then resumed plays no
  sound scheduled inside the interruption, restarts the music at the next bar
  line, and leaves the rendered-stream hash unchanged.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (the clock clamp),
[4](ch004-input-becomes-typed-actions.md) (input from every device),
[5](ch005-replay-a-session-exactly.md) (digests),
[11](ch011-the-wgsl-renderer.md) (one renderer) and
[15](ch015-ui-and-the-hud.md) (the safe area). Coax:
[chapter 21](../../coax/book/ch21-targets.md) for targets and
[chapter 17](../../coax/book/ch17-gpu.md) for GPU programs.
