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

- `Overview`: run telemetry, routing heatmap, selected sublayer detail, event
  feed.
- `Pipeline`: algorithm filmstrip, block schedule, routing logits and weights,
  route story.
- `Inference`: decode summary, two-phase parity, merge split, block cache, and
  selected-sublayer inference detail.

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
