# 0003: Aethyr, the GPU-native framework

Status: Accepted (owner, 2026-10-24: "call that Aethyr").

## Context

The owner wants the best hardware-accelerated graphics on every device, and
one framework for mobile, desktop, and browser. Today five renderers and five
UI drawing paths overlap (the
[GPU Native exploration](../aethyr/imported/2026-10-24-gpu-native.md#where-we-are-today)
lists them).

## Decision

**Aethyr** is the OpenAgents framework that runs on every device: the engine
runtime, the renderer, the UI layer, the device profile, and the zone loader.
**Aethyrforge** stays the name of the engine and its authoring toolchain
(world model, compiler, editor, releases), which builds what Aethyr runs.

Aethyr is **GPU native**:

- One retained scene and UI model, and one renderer, draw the interactive
  surface on the GPU on every platform: worlds and the interface that sits
  over them.
- Platform code is a thin host. It supplies a surface (a browser canvas, a
  `CAMetalLayer` on iOS, a `SurfaceView` on Android, a desktop window), input,
  lifecycle, and the services below.
- These stay native or DOM, deliberately: text input and IME, the
  accessibility tree (mirrored from the UI model), sign-in, autofill and
  passkeys, payments, and web pages that must be indexed or shared, such as
  the marketing site.

## Consequences

- Rust Native's view vocabulary (semantic nodes, typed intents, revisions) is
  the starting point for Aethyr's UI tree. Its per-platform widget adapters
  stop growing; Aethyr adds one GPU adapter plus platform bridges.
- GPUI stays the desktop shell for now. No new investment in the fork beyond
  fixes the desktop needs.
- Crates use the `aethyr-` prefix for the framework. Engine crates keep their
  current names until the import.
- A GPU-drawn surface replaces a DOM or native one only after it passes the
  accessibility and text-input bar. The bar itself is an open owner decision.
