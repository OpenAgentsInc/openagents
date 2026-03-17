# Tassadar Lab

`Tassadar Lab` is the retained desktop pane for inspecting Psionic Tassadar
execution truth inside `apps/autopilot-desktop`.

Important scope note:

- OpenAgents owns the desktop shell, local playback controls, pane persistence,
  and CLI or desktop-control surface.
- Psionic remains the source of truth for replay bundles, live article-session
  snapshots, hybrid workflow snapshots, proof identity, route posture, and run
  lineage.
- Local playback is desktop-owned navigation over Psionic's ordered update
  surface; it does not invent synthetic executor state that Psionic did not
  provide.

## Current Scope

- The pane now supports both a canonical replay-family explorer and live local
  Psionic-prepared article views.
- The replay explorer now exposes the full canonical Psionic replay catalog,
  grouped into explicit families:
  - article-session artifacts
  - article-hybrid workflow artifacts
  - compiled article closure
  - acceptance report
  - learned promotion
  - learned 9x9 fit
  - learned horizon policy
  - architecture comparison
- The current live shells expose:
  - canonical article executor sessions for direct, fallback, and refusal cases
  - canonical planner-owned hybrid workflows for delegated, fallback, and
    refused routing cases

## Open The Pane

- Open the pane from the desktop command palette using `Tassadar Lab`
  (`pane.tassadar_lab`).
- The pane opens as a singleton non-startup surface.
- The pane restores the last source mode, case selection, view, focused update,
  readable-log cursor, token window, fact-line focus, help visibility, and
  local playback speed.
- If the app closed while playback was running, the pane restores in a paused
  inspection state rather than silently resuming.

## Views

- `Overview`: replay identity, workload status, metric chips, and event feed.
- `Trace`: readable log excerpt, token-trace chunk, outputs, and replay updates.
- `Program`: program/runtime identity, decode posture, and fact lines.
- `Evidence`: benchmark/proof identity and selected focused lineage detail.

## Controls

- `Space`: play, pause, resume, or replay from the start if the selected trace
  is already complete.
- `Left` / `Right`: previous or next case inside the active source mode.
- `Up` / `Down`: previous or next replay update.
- `PageUp` / `PageDown`: move readable-log focus.
- `Home` / `End`: move token-trace chunk focus.
- `[` / `]`: move focused fact line.
- `Tab`: cycle views.
- `1` / `2` / `3` / `4`: jump to `Overview`, `Trace`, `Program`, or `Evidence`.
- `5` / `6` / `7`: switch between `Artifact Explorer`, `Article Session`, and
  `Hybrid Workflow` source modes.
- `8` / `9`: move to the previous or next replay family inside explorer mode.
- `r`: reset local playback focus to the start of the current source.
- `f`: refresh the current source from Psionic.
- `-` / `=`: decrease or increase local playback speed.
- `,` / `.`: decrease or increase token-trace window size (`16`, `32`, `64`).
- `?`: toggle the help overlay.
- `Esc`: dismiss the help overlay if it is open.

The top-row WGPUI controls expose the same actions for pointer-driven use:

- previous / next replay family
- previous / next case
- artifact-explorer / article-session / hybrid-workflow source switching
- refresh
- help
- playback
- reset
- slower / faster

## CLI Control

The running desktop app exposes the same Tassadar controller through
`autopilotctl`, so you can drive the pane without opening it:

```bash
autopilotctl tassadar status
autopilotctl tassadar play
autopilotctl tassadar pause
autopilotctl tassadar reset
autopilotctl tassadar refresh
autopilotctl tassadar view trace
autopilotctl tassadar source article-session
autopilotctl tassadar family next
autopilotctl tassadar family set learned-9x9-fit
autopilotctl tassadar case next
autopilotctl tassadar update prev
autopilotctl tassadar readable-log next
autopilotctl tassadar token next
autopilotctl tassadar fact next
autopilotctl tassadar speed set 5
autopilotctl tassadar speed increase
autopilotctl tassadar window decrease
```

These commands target the existing desktop-control runtime and mutate the same
pane-owned state that the WGPUI surface reads.

## Persistence

The pane persists desktop-owned state in the Autopilot log directory:

- default path: `~/.openagents/logs/autopilot/tassadar-lab.json`
- override root: `OPENAGENTS_AUTOPILOT_LOG_DIR`

Persisted state is intentionally pane-owned only:

- playback mode and running or paused posture
- source mode, selected replay family, and selected case
- selected view
- focused replay update
- readable-log, token-trace, and fact-line cursors
- playback speed and token-trace window size
- help visibility
- local event feed

## Ownership Boundary

- Psionic remains the source of truth for live executor sessions, live hybrid
  workflows, replay artifacts, proof identity, decode posture, runtime
  capability, and lineage details.
- OpenAgents owns the retained desktop state, source selection, local event
  feed, refresh semantics, and WGPUI presentation shell.
