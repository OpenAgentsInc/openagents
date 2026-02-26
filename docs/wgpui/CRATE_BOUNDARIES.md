# WGPUI Crate Boundaries

This document defines the current split of WGPUI responsibilities after the
Phase 1 decomposition.

## Current crates

- `crates/wgpui-core`
  - Owns foundational primitives: `color`, `geometry`, `input`, `curve`, `scene`.
  - Must remain product-agnostic and renderer-agnostic.
- `crates/wgpui-render`
  - Owns `renderer` and `svg` modules.
  - Depends on `wgpui-core` primitives for draw input.
- `crates/wgpui`
  - Compatibility facade for existing callers.
  - Re-exports moved modules and continues to host remaining modules
    (`components`, `platform`, `text`, `layout`, etc.).

## Phase 1 migration outcome

- Core and renderer code moved out of the `wgpui` monolith into dedicated crates.
- `apps/autopilot-desktop` continues to build without behavior changes by using
  the `wgpui` facade.

## Next phase (follow-up decomposition)

- Split component modules into `wgpui-widgets`.
- Split platform adapters into dedicated crates (`wgpui-platform-*`).
- Isolate non-MVP testing/demo lanes into a dedicated testing crate.
