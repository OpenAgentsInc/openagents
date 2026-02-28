# WGPUI MVP Boundaries

This crate now separates retained MVP runtime surfaces from optional non-MVP surfaces.

## Default (MVP) path

- Enabled by default features (`desktop`).
- Includes reusable UI primitives and runtime components consumed by `apps/autopilot-desktop`.
- Excludes demo/storybook/testing integration surfaces from the default compile path.

## Non-MVP surfaces

- Grouped under `src/non_mvp/`.
- Gated behind `feature = "non-mvp-surfaces"`.
- Current modules:
  - `integration` (chat-application integration surface used for demos/experiments).

## Feature mapping

- `testing` implies `non-mvp-surfaces`.
- `storybook` implies `non-mvp-surfaces`.

This keeps MVP runtime wiring lean while preserving optional developer/demo surfaces behind explicit feature flags.
