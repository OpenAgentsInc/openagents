# Tassadar Lab

`Tassadar Lab` is the retained desktop pane for inspecting committed Psionic
Tassadar execution artifacts inside `apps/autopilot-desktop`.

## Current Scope

- This first lane is replay-first only.
- The pane reads curated Psionic replay surfaces and does not start live
  execution itself yet.
- The curated shell currently exposes:
  - the direct article-session replay
  - the compiled article-closure replay

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

- `Left` / `Right`: previous or next replay.
- `Up` / `Down`: previous or next replay update.
- `PageUp` / `PageDown`: move readable-log focus.
- `Home` / `End`: move token-trace chunk focus.
- `[` / `]`: move focused fact line.
- `Tab`: cycle views.
- `1` / `2` / `3` / `4`: jump to `Overview`, `Trace`, `Program`, or `Evidence`.
- `?`: toggle the help overlay.
- `Esc`: dismiss the help overlay if it is open.

## Ownership Boundary

- Psionic remains the source of truth for replay artifacts, proof identity,
  decode posture, runtime capability, and lineage details.
- OpenAgents owns the retained desktop state and WGPUI presentation shell.
