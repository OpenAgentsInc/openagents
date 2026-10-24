> Deprecated (owner, 2026-10-24): history only. Architecture decisions live in [`docs/adr/`](../../../adr/README.md).

# 0001: Browser-first hybrid engine

Status: selected, 2026-10-22. Graphics placement revised by
[decision 0006](0006-cross-platform-engine-core.md) on 2026-10-24: the renderer
moves into the Rust core over the standard WebGPU API, and TypeScript keeps the
editor and web host glue.

## Context

Creators need fast interface iteration and direct browser integration. Large
simulation/compiler workloads need explicit memory, data parallelism, and useful
native tooling. GPU performance depends on submitted work and hardware. No legacy
language restriction or existing renderer determines this clean-sheet choice.

## Decision

Use React DOM/TypeScript components, Effect Atom UI state, and Effect 4 services
for the trusted editor, an owned
Rust/Wasm CPU kernel/compiler/behavior VM, and a thin direct WebGPU host with WGSL.
Exchange coarse typed buffers, cache immutable resources, and keep frame/tick
inner loops free of per-entity effects or cross-language calls.
The frontend boundary and its independent frame loop are specified in
[decision 0005](0005-react-effect-frontend.md).

## Alternatives and consequences

- **Entire engine in TypeScript:** excellent browser iteration; retained as a
  reference baseline for performance experiments. The selected CPU core favors
  explicit memory and reuse across compiler/service hosts. This is not proof of
  a universal Rust speedup.
- **Entire browser editor/renderer in Rust/wgpu:** viable, especially for native
  parity. Adds Wasm/UI integration and compiled engine edits without a browser
  GPU bypass; not selected for the editor. Decision 0006 later moves the
  renderer, but not the editor, into Rust.
- **Three.js/Babylon/PlayCanvas as engine:** useful reference implementations;
  making their scene/editor contracts authoritative conflicts with the requested
  ownership of the engine and compiler. Their code and algorithms can be studied
  under appropriate licenses without adopting their object model.
- **Custom language/browser/runtime:** high ownership but little demonstrated
  advantage for authoring. Standards-based web distribution remains the target.

The split adds ABI, toolchain, and qualification obligations. [Runtime ABI](../runtime-abi.md)
must make those boundaries explicit. Decision 0006 replaces the separate native
adapter with one Rust renderer that runs on browser WebGPU and on native Dawn.

## Revisit

Change work placement if equivalent workloads show interop, download, compilation,
or maintenance costs outweigh the selected benefit. Record quality, profile,
latency, and failure behavior in the experiment. See [performance](../performance.md).
