# 2026-03-17 AttnRes Full-Run, Paper, and Reference Gap Audit

## Intent

This audit answers the current question more precisely than the earlier
desktop-port audit:

> what do we actually have today in `openagents`, what does the official
> `Attention-Residuals` repo and the local Burn reference repo provide, why do
> the current Psionic/WGPUI runs finish in a couple seconds, and what is still
> missing before we can honestly say the AttnRes experience is fully ported?

The key clarification is that there are three different targets:

1. the official Moonshot paper/research target in
   `/Users/christopherdavid/code/Attention-Residuals`
2. the local Burn reference implementation and demos in
   `/Users/christopherdavid/code/attnres`
3. the current Psionic/WGPUI desktop lane in `openagents`

Those are not equivalent today.

## Requested Product Bar

The requested bar for this work is not:

- a tiny training demo
- a short bounded inspection-only loop
- a paper-only visualization pass

The requested bar is:

- the full local interactive run the Burn reference repo already has
- except implemented truthfully on Psionic for model/training/inference and
  rendered in WGPUI inside `openagents`

That means the immediate parity target is the Burn local demo semantics, not
just "some AttnRes-shaped run" and not the much larger paper-scale research
regime.

## Sources Reviewed

OpenAgents:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/attnres-lab.md`
- `docs/audits/2026-03-17-attnres-openagents-wgpui-port-audit.md`
- `apps/autopilot-desktop/src/panes/attnres_lab.rs`
- `apps/autopilot-desktop/src/attnres_lab_control.rs`
- `apps/autopilot-desktop/src/app_state_domains.rs`

Official Moonshot AttnRes repo:

- `/Users/christopherdavid/code/Attention-Residuals/README.md`
- `/Users/christopherdavid/code/Attention-Residuals/Attention_Residuals.pdf`

Local Burn reference repo:

- `/Users/christopherdavid/code/attnres/README.md`
- `/Users/christopherdavid/code/attnres/ARCHITECTURE.md`
- `/Users/christopherdavid/code/attnres/examples/demo_tui.rs`
- `/Users/christopherdavid/code/attnres/examples/train_tiny.rs`
- `/Users/christopherdavid/code/attnres/examples/visualize_weights.rs`
- `/Users/christopherdavid/code/attnres/examples/compare_residuals.rs`
- `/Users/christopherdavid/code/attnres/web-demo/src/main.ts`

Current Psionic:

- `/Users/christopherdavid/code/psionic/crates/psionic-train/src/attnres.rs`

## Executive Summary

The full local AttnRes reference run is now implemented in Psionic/OpenAgents.

What is true now:

- `openagents` has a real WGPUI `AttnRes Lab` pane.
- `psionic` has real AttnRes model, diagnostics, training, inference, and
  two-phase parity machinery.
- the pane now runs a Psionic-owned local-reference config targeting the Burn
  `demo_tui` full-run bar at `320` steps instead of the earlier bounded
  `6`-step lane.
- `autopilotctl` can create, manage, and inspect that full local run
  headlessly.
- the controller now builds live snapshots from the active Psionic runner/model
  state instead of replaying the full training loop on every tick.

What is still not true:

- the current run is not the Moonshot paper’s large-scale training regime
- the desktop lab is still a local reference/demo surface, not a paper-scale
  distributed research/operator surface

The original 2-second-ish runtime was expected from the old code as written,
not a timing bug:

- Burn `demo_tui` uses `max_steps = 320` and paced tick delays from `260 ms`
  down to `20 ms` depending on speed.
- the old Psionic/OpenAgents lane called
  `AttnResTinyTrainingConfig::reference()`, which was hard-coded to:
  - `budget: TrainingLoopBudget::new(6, 1, 1)?`
  - `step_duration_ms: 25`

That local-reference gap is now closed. The paper-scale target remains
separate.

## What The Official Moonshot Repo Actually Gives Us

`/Users/christopherdavid/code/Attention-Residuals` is primarily the paper plus
figures. It is not the runnable local reference implementation.

Relevant truths from the paper and repo:

- It defines three conceptual targets:
  - standard residuals
  - Full AttnRes
  - Block AttnRes
- It argues that Block AttnRes is the scalable practical variant.
- It explicitly calls out:
  - cache-based pipeline communication
  - two-phase inference with online softmax merge
  - bounded inference overhead
  - bounded training overhead even with pipeline parallelism

The paper-level claims that matter for parity planning are:

- Block AttnRes is the practical drop-in target, not only Full AttnRes.
- two-phase inference is part of the real algorithm story, not optional UI
  decoration.
- paper-scale AttnRes includes systems concerns that the current desktop pane
  does not surface at all:
  - cross-stage caching
  - pipeline-communication savings
  - scaling-law and training-dynamics evidence

Important constraint:

- the official repo does not give us runnable code for the paper-scale system
  in this checkout
- so the paper is the architectural truth source, not the direct desktop demo
  source

## What The Burn Reference Repo Actually Gives Us

`/Users/christopherdavid/code/attnres` is the real local reference
implementation for desktop-demo parity.

The relevant surfaces are:

- `examples/demo_tui.rs`
  - the strongest reference for the interactive local demo
- `examples/train_tiny.rs`
  - a simpler CLI training example
- `examples/visualize_weights.rs`
  - explicit weight/routing inspection
- `examples/compare_residuals.rs`
  - standard-residual vs AttnRes equivalence and comparison narrative
- `web-demo/`
  - a browser-side explainer/demo with richer comparison visuals

The most important parity facts from `demo_tui.rs`:

- it runs a materially longer local demo:
  - `max_steps = 320`
- it exposes real pacing:
  - speed-dependent tick delays from `260 ms` to `20 ms`
- it records runtime telemetry the current desktop pane does not expose:
  - `last_train_ms`
  - `last_diag_ms`
  - `avg_loop_ms`
  - `steps_per_second`
- it has explicit small-screen and compact-layout behavior:
  - `MIN_WIDTH = 80`
  - `MIN_HEIGHT = 24`
  - `COMFORT_WIDTH = 100`
  - `COMFORT_HEIGHT = 32`
  - dedicated `draw_small_ui(...)`
  - compact vs full inference/overview/pipeline layouts
- it includes several demo-oriented views that are only partially represented
  in OpenAgents today:
  - training card with ETA and speed semantics
  - parity card with loop timings
  - comparison/weight-visualization ideas echoed in other examples and the
    web demo

This Burn repo is the right "full local run" reference target for the desktop
port. The paper-scale target is separate and much larger.

For the requested product bar, `demo_tui.rs` is the binding parity reference.
`train_tiny.rs` is not the bar.

## What OpenAgents/Psionic Actually Has Today

### Implemented

- real desktop pane shell in `apps/autopilot-desktop`
- real Psionic-backed AttnRes controller and persisted pane state
- real two-phase parity checks
- real per-sublayer routing diagnostics
- real WGPUI cards for:
  - Overview
  - Pipeline
  - Inference
- programmatic control via `autopilotctl`
- stronger visual language borrowed from `Psionic Mesh`

### Implemented Local-Reference Closure

- live training is Psionic-backed through a new local-reference config/runner
  rather than the old bounded 6-step lane
- the desktop contract now consumes a non-`tiny` public Psionic surface for the
  full local run
- runtime pacing and throughput are surfaced through loop timing,
  steps-per-second, and ETA in both the pane and `autopilotctl`
- the desktop controller no longer performs quadratic replay to render the
  current step

### Still Outside This Closure

If the target is "the full run they have, except in Psionic/WGPUI", the main
closure is now done. The remaining optional follow-ons are:

- more explicit whole-pane compact/small-layout branches matching the Burn TUI
  structure even more literally
- the comparison-oriented companion demos from the reference repo:
  - standard residual vs AttnRes comparison
  - explicit weight-visualization flow
  - richer browser-demo style explainer surfaces

## Why The Old Run Finished So Fast

This is the direct cause chain:

- `openagents` calls `lab_training_config()`
- `lab_training_config()` returns
  `AttnResTinyTrainingConfig::reference()`
- that reference config is hard-coded in Psionic to:
  - `max_steps = 6`
  - `step_duration_ms = 25`

So a fast completion was the expected behavior of the old codebase.

That is no longer the active local-reference path.

## UI/Layout Findings

The screenshot issue was real:

- several AttnRes cards painted long mono strings into fixed-width regions
  without width budgeting
- `Selected Detail` in inference view had too little vertical budget relative
  to the number of rows it always tried to render
- the pane did not degrade gracefully when cards got shorter or narrower

Best-practice patterns already present elsewhere in the app:

- pre-wrapping or chunking text before painting
- truncating summary lines instead of letting them bleed across cards
- explicit compact/small-layout branches for constrained viewports

Remediation landed in the pane:

- width-aware truncation for long hero and summary lines
- height-aware multiline fitting for event/detail/note text
- a more compact `Selected Detail` rendering path for shorter cards
- a smaller `Block Cache Health` share so `Selected Detail` gets more height

This makes the current pane more honest and less brittle, and it now sits on
top of the actual full local reference run instead of the old bounded lane.

## What "Full Run" Should Mean Going Forward

There are two defensible meanings, and they should not be conflated.

### Meaning A: Full Local Reference Run

This is the immediate target and the one the current desktop port should meet.

Definition:

- match the Burn reference demo’s local interactive run semantics closely
- at minimum:
  - comparable run length
  - comparable pacing
  - comparable live telemetry
  - comparable small/compact UI behavior

This milestone is now implemented.

The desktop persistence path is also hardened for this transition:

- incompatible persisted `Completed` state from the retired `6`-step lane is
  migrated/reset on load so the pane cannot remain falsely `Completed` at
  `6/320`

### Meaning B: Full Paper/Research Run

This is a larger systems/research target.

Definition:

- support the paper’s Block AttnRes system story in Psionic:
  - cross-stage caching
  - two-phase inference
  - larger-scale experimental configs
  - training-dynamics and scaling-law evidence

This should be treated as a separate research/operator milestone, not as the
same thing as the desktop demo port.

## Recommended Implementation Plan

### Phase 1: Full Local Run Closure

Completed:

- added a Psionic-owned local-reference AttnRes config/runner targeting the
  Burn `320`-step full-run bar
- stopped exposing the old bounded lane as the desktop-owned default contract
- exposed real loop timing and throughput in the desktop pane and
  `autopilotctl`
- removed the quadratic snapshot replay path from the live desktop lane

### Phase 2: Bring In The Relevant Reference Side Demos

At minimum, port the useful reference-demo ideas that still have no desktop
equivalent:

- standard residual vs AttnRes comparison
- explicit depth-weight visualization
- browser-demo style explainer/comparison surfaces where they improve the
  desktop lab

This may be one richer pane or a small AttnRes pane family.

### Phase 3: Separate Paper-Scale Research Surfaces

Do not overload the desktop demo pane with all paper-scale concerns.

If needed later, add separate research/operator surfaces for:

- Block vs Full sweeps
- scaling-law runs
- training-dynamics charts
- pipeline/caching metrics

## Bottom Line

Current status is:

- the WGPUI pane exists
- the Psionic substrate exists
- the programmatic control exists
- the desktop now runs the full local `320`-step reference lane instead of the
  old bounded `6`-step path
- the pane and CLI now surface long-run timing and throughput truth

So the correct claim now is:

- "the full local AttnRes reference run is ported into Psionic/WGPUI"

The remaining non-closure is:

- "paper-scale Moonshot research training is a separate target"
