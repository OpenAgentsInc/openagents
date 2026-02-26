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
- Platform abstraction (`platform`)

## Feature Lanes

- `desktop`: native window/event-loop lane used by the MVP app (default)
- `web`: wasm/websys lane
- `ios`: iOS-specific platform lane
- `testing`: testing harness/DSL lane (feature-gated)
- `storybook`: visual storybook demo lane (feature-gated)
- `network`: URL fetch/image loading lane (feature-gated)
- `audio`: optional audio helpers

Dependency gating notes:

- `network` enables `reqwest` for desktop URL fetch helpers.
- `audio` enables `rodio` and also enables `network` for URL-based audio sources.
- `storybook` only affects the storybook example target.
- `testing` only enables the `testing` module and testing viewer lane.

## Module Map

- Core rendering: `src/scene.rs`, `src/renderer.rs`, `src/text.rs`, `src/text_system/`
- Platform: `src/platform/`
- Components: `src/components/`
- Input/action: `src/input.rs`, `src/action/`, `src/keymap/`, `src/interactive.rs`

## API Surface and Preludes

`wgpui` now exposes explicit prelude lanes:

- `wgpui::prelude::core`
- `wgpui::prelude::desktop` (`desktop` feature only)

Crate-root exports are intentionally narrower than before. For advanced or niche
surfaces, use explicit module paths (for example `wgpui::components::...`,
`wgpui::layout::...`, `wgpui::text_system::...`) instead of assuming broad
root re-exports.

Migration guidance:

1. Prefer importing shared app primitives from `wgpui::prelude::core`.
2. Use `wgpui::prelude::desktop` in desktop apps where `Renderer` and
   `DesktopPlatform` are primary.
3. For anything not in the prelude, import directly from its module path.

## Documentation

- Docs index: `docs/README.md`
- Rendering details: `docs/rendering-pipelines.md`
- Layering/z-order: `docs/layer-system.md`
- Action/keymap system: `docs/action-keymap-system.md`
- Theme system: `docs/THEME.md`

## MVP Alignment

For product authority and scope decisions, follow `docs/MVP.md` at repo root.
When docs and code disagree, code behavior in this repo is authoritative.
