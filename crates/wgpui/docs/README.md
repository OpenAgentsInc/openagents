# wgpui Docs

Last verified: 2026-02-26  
Owner lane: `owner:runtime`

This folder contains subsystem documentation for the current `wgpui` crate in
this MVP-pruned repository.

## Index

- [Rendering Pipelines](rendering-pipelines.md)
  GPU pipeline architecture for quads, lines, text, and images.
- [Layer System](layer-system.md)
  Scene layer ordering and overlay behavior.
- [Text Rendering Coordinate System](text-rendering-coordinate-system.md)
  Logical/physical pixel flow and scaling invariants.
- [Action and Keymap System](action-keymap-system.md)
  Action dispatch, keybindings, and context resolution.
- [Theme System](THEME.md)
  Theme token usage and extension guidance.
- [Phase 1 Foundation](phase1-foundation.md)
  `app` / `element` / `window` framework lane internals.
- [Tool Call Card Progress](tool-call-card-progress.md)
  Targeted component progress notes.

## Scope Note

The desktop MVP app currently uses `wgpui` primarily through component + scene
surfaces. The framework lane (`app`, `element`, `window`) remains available but
is not yet the canonical default for `apps/autopilot-desktop`.
