# wgpui

Last verified: 2026-02-26  
Owner lane: `owner:runtime`

`wgpui` is the GPU UI crate used by the desktop MVP (`apps/autopilot-desktop`).
In this pruned repository, the primary implementation lane is desktop-first MVP work,
with optional web/ios support kept behind feature gates.

## Current Scope

- Fast scene-based rendering (`scene`, `renderer`, `text`, `text_system`)
- Input and interaction primitives (`input`, `action`, `keymap`, `interactive`)
- Component library (`components/*`, including `hud` and `live_editor`)
- Layout and styling (`layout`, `layout_helpers`, `styled`, `theme`)
- Platform abstraction (`platform`) and optional framework lane (`app`, `element`, `window`)

## Feature Lanes

- `desktop`: native window/event-loop lane used by the MVP app
- `web`: wasm/websys lane (currently default feature)
- `ios`: iOS-specific platform lane
- `testing`: extra testing harness and helpers
- `audio`: optional audio helpers

## Module Map

- Core rendering: `src/scene.rs`, `src/renderer.rs`, `src/text.rs`, `src/text_system/`
- Platform: `src/platform.rs` (to be decomposed per audit backlog)
- Components: `src/components/`
- Input/action: `src/input.rs`, `src/action/`, `src/keymap/`, `src/interactive.rs`
- Optional framework lane: `src/app/`, `src/element/`, `src/window/`

## Documentation

- Docs index: `docs/README.md`
- Rendering details: `docs/rendering-pipelines.md`
- Layering/z-order: `docs/layer-system.md`
- Action/keymap system: `docs/action-keymap-system.md`
- Theme system: `docs/THEME.md`
- Framework lane internals: `docs/phase1-foundation.md`

## MVP Alignment

For product authority and scope decisions, follow `docs/MVP.md` at repo root.
When docs and code disagree, code behavior in this repo is authoritative.
