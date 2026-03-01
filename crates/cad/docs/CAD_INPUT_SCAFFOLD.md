# CAD Input Scaffolding

This document captures the MVP CAD reducer/action scaffold in desktop input routing.

## Location

- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/reducers/cad.rs`

## Actions

- `CadDemoPaneAction::Noop`
- `CadDemoPaneAction::CycleVariant`
- `CadDemoPaneAction::ResetSession`

## Behavior

- Actions are lane-neutral and mutate only `RenderState::cad_demo`.
- `Noop` is an explicit no-op for deterministic command-loop tests.
- `CycleVariant` updates active variant + increments revision.
- `ResetSession` restores deterministic defaults.

## Tests

- reducer tests validate:
  - no-op path stability
  - state transition on cycle
  - deterministic reset behavior
- pane-system layout tests validate CAD action button bounds ordering.
