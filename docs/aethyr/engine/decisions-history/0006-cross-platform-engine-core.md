> Deprecated (owner, 2026-10-24): history only. Architecture decisions live in [`docs/adr/`](../../../adr/README.md).

# 0006: One Rust engine core over the standard WebGPU API, web first

Status: selected, 2026-10-24. Not implemented. Revises the graphics placement in
[decision 0001](0001-hybrid-engine.md); the editor frontend in
[decision 0005](0005-react-effect-frontend.md) is unchanged.

## Context

Aethyrforge must reach mobile and, later, desktop with the same worlds and
releases. The web stays the first shipping target. Decision 0001 placed the
renderer in a thin TypeScript WebGPU host and left native graphics as a later,
separately qualified adapter. Keeping that split would mean a second renderer
for each native platform, with its own bugs and its own image drift.

The October 23 [C/Wasm Town experiment](../research/c-medieval-town-results-2026-10-23.md)
shows that the CPU language is not the frame-rate lever for current scenes.
Submitted CPU preparation was 1.9–4.0 ms median while the main HDR GPU pass
was 30–42 ms. C saved about 0.5 ms, and pooled TypeScript recovered most of
that saving. The language choice is therefore a portability and ownership
decision, not a speed decision. GPU work is addressed by
[decision 0007](0007-gpu-driven-rendering.md).

## Decision

**Targets, in order.** Web first, then mobile (iOS and Android), then desktop.
Each native target is its own qualified profile in
[platform support](../platform-support.md). The first mobile delivery is the
web player in mobile browsers; native embedding follows the same core.

**One engine core in Rust.** The runtime, simulation kernel, render graph,
visibility, batching, resource lifetimes, and GPU command recording live in
Rust crates that build for `wasm32-unknown-unknown` and for native targets.
The same crates produce the same frames on every platform.

**Program against the standard WebGPU API.** The engine owns a thin GPU crate
whose interface follows `webgpu.h`, the shared C header that browser and native
WebGPU implementations expose. It has two backends:

- **Web:** calls the browser's WebGPU through a small generated JavaScript
  import table. The browser keeps full validation, and existing browser tools
  such as WebGPU Inspector still see every call.
- **Native:** links [Dawn](https://dawn.googlesource.com/dawn), the WebGPU
  implementation in Chromium, through `webgpu.h`. Dawn reaches Metal, Vulkan,
  and Direct3D 12 and validates the same way Chrome does, so a native build
  accepts the same shaders and resource use as the web build.

wgpu is the fallback native backend if Dawn integration fails qualification on
a target. The GPU crate's interface does not expose backend-specific types.

**Shared WGSL artifacts.** Every backend consumes the same compiled WGSL.
[Decision 0008](0008-shader-toolchain.md) owns shader authoring.

**TypeScript keeps the editor and web host glue.** React DOM, Effect Atom, and
Effect 4 keep the editor ([decision 0005](0005-react-effect-frontend.md)).
On the web, TypeScript also owns canvas and worker bootstrap, input and
lifecycle events, and the WebGPU import table. It no longer owns the render
graph or command encoding. Native host applications supply the surface, input,
and lifecycle through a C ABI and never reach into engine state.

**Threads.** The runtime uses one job system. On native it runs on OS threads.
On the web, the cross-origin-isolated shared-memory profile becomes the primary
performance profile for the player and preview origins, with threaded Wasm,
SIMD, and a shared job pool. The transferable-buffer profile stays a qualified
fallback with identical results, never a separate rule set. Creators never weaken
security in their own browser to open a project.

**Migration.** The current TypeScript renderer stays the production path until
a Rust pass reaches exact image parity and equal or better measurements on the
pinned Town and battle profiles. The port moves one pass at a time behind the
existing renderer profile switch, with the image comparison retained.

## Alternatives and consequences

- **Keep the TypeScript renderer and add a native renderer later:** fastest on
  the web today, but two renderers drift and double every graphics fix.
- **C or C++ core with Dawn:** the industry-standard route, and native Dawn is
  the same choice here. Measurements show no frame benefit from C, and it would
  abandon the existing Rust kernel, simulation, and server crates.
- **wgpu on every platform:** one Rust API, but its native backends do not share
  browser validation, so a native build can accept work a browser rejects.
  Retained as the fallback.
- **Wrap the web build in a native WebView:** a cheap early mobile channel and
  allowed as an interim distribution. It does not give native control of
  thermal state, background behavior, or store requirements, so it is not the
  native profile.
- **A third-party engine:** rejected for the ownership reasons in decision 0001.

Consequences:

- Each WebGPU call from Wasm crosses into JavaScript. That cost is acceptable
  only because decision 0007 reduces submissions to tens of commands per frame.
  The qualification records submitted CPU time for the Rust and TypeScript
  paths on the same profile.
- The GPU crate, the C ABI for hosts, and the JavaScript import table add
  [runtime ABI](../runtime-abi.md) and conformance obligations.
- Native builds bring Dawn's build, size, and update cost into the release
  process, separately for each platform.
- Mobile requires measured memory, thermal, and suspend/resume budgets before a
  profile is qualified.

## Revisit

Revisit the Rust renderer if, after decision 0007's GPU-driven path lands, its
submitted CPU time on a pinned web profile exceeds the TypeScript path's by more
than 0.5 ms at p95, or if the Wasm build materially increases startup time.
Revisit Dawn if it cannot be qualified on iOS or Android within the mobile
milestone; switch that target to wgpu rather than adding a third renderer.
[Performance qualification](../performance-qualification.md) owns the method.
