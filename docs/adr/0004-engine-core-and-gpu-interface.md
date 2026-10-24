# 0004: Engine core and GPU interface

Status: Accepted. Restates Aethyrforge 0006, which the owner directed on
2026-10-24 ("web is the first target", cross-platform "important").

## Decision

- **One Rust engine core.** Runtime, simulation, render graph, visibility,
  batching, resource lifetimes, and GPU command recording are Rust crates that
  build for `wasm32-unknown-unknown` and native targets.
- **One GPU interface shaped like `webgpu.h`** (`af-gpu` today). Its web
  backend calls the browser's WebGPU. Its native backend links a `webgpu.h`
  implementation: Dawn, with wgpu-native as the fallback per target. No
  backend-specific types cross the interface. There is no WebGL renderer.
- **Web calls are batched.** Each render pass's commands are written to Wasm
  memory and replayed by one JavaScript call, instead of one import call per
  command. Measured single samples on an M5 Max showed per-call imports
  costing 0.22–0.66 ms against 0.10–0.15 ms for direct TypeScript calls for
  about 1,500 draws.
- **Hosts are thin.** On the web, TypeScript keeps canvas and worker
  bootstrap, input, lifecycle, and the WebGPU import table. Native hosts
  supply surface, input, and lifecycle through a C ABI.
- **The TypeScript renderer stays in production** until each Rust pass matches
  its image and is equal or faster on pinned device profiles.

## Revisit when

- After batching, Rust submission is still more than 0.5 ms slower than
  TypeScript at p95 on a pinned web profile.
- Neither Dawn nor wgpu-native qualifies on iOS or Android.
- The Wasm build makes first load materially slower on a mid-range phone.

## Why Rust

Frames are GPU-bound: CPU preparation measured 2–4 ms against a 22–42 ms main
pass, and C made no frame-rate difference over TypeScript. So the language
decides portability and reuse, not speed. Rust runs the existing kernel, text
layout, mobile core, and domain crates on every target today.
