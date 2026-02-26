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
- [Tool Call Card Progress](tool-call-card-progress.md)
  Targeted component progress notes.

## Scope Note

The desktop MVP app uses `wgpui` through the retained component + scene lanes.
Legacy framework-lane docs were removed with the unused lane retirement.
