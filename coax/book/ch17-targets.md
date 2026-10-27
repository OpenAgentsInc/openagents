# 17. Targets: Apple first, then the browser

> **Design.** Spec §17.2 and §20. Today `coax0`'s C output is tested on
> the host it's built on (macOS arm64). Every other target needs its own
> evidence before Coax calls it supported.

## 17.1 A target is a promise with evidence

Being able to emit code for a CPU doesn't make it a supported target.
A target is supported when it has passing tests for:

- the runtime and startup;
- the linker and C ABI, in both directions;
- debugging: symbols, stack traces, source locations;
- an end-to-end program on the real platform.

Coax publishes a target matrix, and each row needs those proofs. The Jai
report makes the same point about Jai: its strongest platform evidence
comes from particular projects, not from one official support matrix.

## 17.2 Apple native, first

The native OpenAgents app is the first target, because it's where the
speed is:

```
Swift view controller → C API (home_app.h) → engine → metal-cpp → Metal → GPU
```

There's no JavaScript, WebGL or ANGLE in that path. A Coax subsystem
joins it as a C object file built by Xcode, called through the same C
boundary. Development reload happens on the Mac host (Chapter 15).

## 17.3 The browser, proved separately

The web build runs the engine through WebAssembly. A Coax browser target
has its own proof, a vertical slice through a real browser:

| Boundary | What to prove |
| --- | --- |
| Memory layout | Pointer width (wasm32) and every struct shared with JavaScript |
| Memory growth | JavaScript typed-array views go stale when Wasm memory grows; refresh them |
| Scheduling | Async loading, the main loop, worker communication |
| Threads | Which browser features and headers (COOP/COEP) the design needs |
| Graphics | WebGPU or WebGL resource lifetimes, layouts, device loss |
| Debugging | Source maps, stack traces, release symbols |
| Iteration | Incremental assets, state kept across reloads, deployment timing |

The model to study is Polygon, a procedural engine rewritten in Jai.
Its author first moved geometry generation into Jai compiled to
WebAssembly, sharing pointers and sizes with JavaScript typed arrays,
then later built a renderer on wgpu natively and WebGPU in the browser.
The staged approach is the lesson: move expensive CPU work behind a
small boundary first, and decide about the renderer later.

## 17.4 Later

Android, Linux and Windows follow the same rule. Each needs its own
runtime, linker, ABI and debugging evidence. Until then, the honest
statement is: **C output that builds and runs on the tested host.**
