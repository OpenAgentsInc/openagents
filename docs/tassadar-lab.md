# Tassadar Lab

`Tassadar Lab` is the retained desktop pane for inspecting Psionic Tassadar
execution truth inside `apps/autopilot-desktop`.

## Current Scope

- The pane now supports both curated replays and live local Psionic-prepared
  article views.
- The current replay shell exposes:
  - the direct article-session replay
  - the compiled article-closure replay
- The current live shells expose:
  - canonical article executor sessions for direct, fallback, and refusal cases
  - canonical planner-owned hybrid workflows for delegated, fallback, and
    refused routing cases

## Open The Pane

- Open the pane from the desktop command palette using `Tassadar Lab`
  (`pane.tassadar_lab`).
- The pane opens as a singleton non-startup surface.

## Views

- `Overview`: replay identity, workload status, metric chips, and event feed.
- `Trace`: readable log excerpt, token-trace chunk, outputs, and replay updates.
- `Program`: program/runtime identity, decode posture, and fact lines.
- `Evidence`: benchmark/proof identity and selected focused lineage detail.

## Controls

- `Left` / `Right`: previous or next case inside the active source mode.
- `Up` / `Down`: previous or next replay update.
- `PageUp` / `PageDown`: move readable-log focus.
- `Home` / `End`: move token-trace chunk focus.
- `[` / `]`: move focused fact line.
- `Tab`: cycle views.
- `1` / `2` / `3` / `4`: jump to `Overview`, `Trace`, `Program`, or `Evidence`.
- `5` / `6` / `7`: switch between `Replay`, `Article Session`, and
  `Hybrid Workflow` source modes.
- `r`: refresh the current source from Psionic.
- `?`: toggle the help overlay.
- `Esc`: dismiss the help overlay if it is open.

## Ownership Boundary

- Psionic remains the source of truth for live executor sessions, live hybrid
  workflows, replay artifacts, proof identity, decode posture, runtime
  capability, and lineage details.
- OpenAgents owns the retained desktop state, source selection, local event
  feed, refresh semantics, and WGPUI presentation shell.
