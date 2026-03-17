# AttnRes Lab

`AttnRes Lab` is the retained desktop pane that ports the original Burn/TUI
demo into `apps/autopilot-desktop` using WGPUI for presentation and Psionic for
all model, routing, parity, and training truth.

## Open The Pane

- Open the pane from the desktop command palette using `AttnRes Lab`
  (`pane.attnres_lab`).
- The pane restores the last selected view, sublayer, speed, help visibility,
  and stepped training position.
- If the app closed while a run was active, the pane restores in a paused state
  rather than silently resuming.

## What The Pane Runs

- Training uses the repo-owned Psionic tiny-training reference corpus and the
  Psionic stepwise runner.
- Inference uses the current stepped Psionic model through the local AttnRes
  generation service.
- Routing diagnostics and two-phase parity are computed from real Psionic model
  and runtime outputs.

## Views

- `Overview`: training telemetry, loss stream, block topology, runtime,
  routing heatmap, selected sublayer detail, and event feed.
- `Pipeline`: algorithm filmstrip, inspector, pre-softmax depth scores,
  softmax routing mass, route story, block schedule, and event feed.
- `Inference`: two-phase parity, two-phase schedule, online merge, block cache
  health, selected detail, and event feed.

## Visual Language

- The pane now uses the retained `Psionic Mesh` pane as its visual reference
  rather than the flatter first-pass cards.
- The top hero now exposes run state, selected-sublayer routing focus, and a
  live ribbon of the active route field.
- Overview now restores the original TUI panel inventory explicitly: loss
  history, topology, and runtime all have dedicated cards instead of being
  collapsed into one summary block.
- Pipeline now splits the original inspector, pre-softmax score view, and
  routing-mass view into dedicated WGPUI panels rather than one combined chart.
- Inference now restores the original schedule, merge, and cache-health panel
  split and keeps the TUI's `Merge Split` / `Block Cache` vocabulary visible in
  parity detail.
- Training telemetry uses ring gauges, signal triplets, and ribbon-history
  rails for loss, EMA, and selectivity instead of text-only summaries, so the
  desktop port is strictly denser and more legible than the terminal original.

## Controls

- `Space`: start, pause, resume, or restart after completion.
- `Up` / `Down`: increase or decrease training speed.
- `Left` / `Right`: inspect the previous or next sublayer.
- `Tab`: cycle views.
- `1` / `2` / `3`: jump to `Overview`, `Pipeline`, or `Inference`.
- `?`: toggle the help overlay.
- `Esc`: dismiss the help overlay if it is open.
- `r`: reset to the seeded reference checkpoint.

The top-row WGPUI controls expose the same actions for pointer-driven use:

- playback
- reset
- refresh live snapshot
- slower / faster
- help
- previous / next sublayer

## CLI Control

The running desktop app exposes the same AttnRes controller through
`autopilotctl`, so you can manage the lab without opening the pane:

```bash
autopilotctl attnres status
autopilotctl attnres start
autopilotctl attnres pause
autopilotctl attnres reset
autopilotctl attnres refresh
autopilotctl attnres view overview
autopilotctl attnres sublayer next
autopilotctl attnres sublayer set 4
autopilotctl attnres speed increase
autopilotctl attnres speed set 5
autopilotctl wait attnres-running
autopilotctl wait attnres-paused
autopilotctl wait attnres-completed
```

These commands target the existing desktop-control runtime and mutate the same
persisted pane-owned AttnRes state the WGPUI surface reads.

## Persistence

The pane persists desktop-owned state in the Autopilot log directory:

- default path: `~/.openagents/logs/autopilot/attnres-lab.json`
- override root: `OPENAGENTS_AUTOPILOT_LOG_DIR`

Persisted state is intentionally pane-owned only:

- playback mode
- current stepped position
- selected view
- selected sublayer
- speed
- help visibility
- event feed

Psionic remains the source of truth for all AttnRes training and inference
semantics. The desktop rebuilds the live snapshot from persisted pane state
rather than serializing model internals itself.
