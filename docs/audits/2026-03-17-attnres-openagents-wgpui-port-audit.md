# 2026-03-17 AttnRes OpenAgents WGPUI Port Audit

## Intent

This audit answers the current repo-local question:

> what is already implemented across `openagents`, `psionic`, and
> `~/code/attnres`, what is still missing, and what is the ownership-correct
> implementation plan to port the original AttnRes TUI into an
> `openagents` desktop pane using WGPUI?

This document is narrower than the earlier
`2026-03-16-attnres-port-into-psionic-and-wgpui-audit.md`.
That earlier audit was about the broad Burn-to-Psionic migration problem.
This document is about current OpenAgents desktop reality after the repo prune
and after refreshing against the current clean `~/code/psionic` checkout.

This is an architecture and implementation-planning audit. It does not, by
itself, reprioritize `docs/MVP.md`. The desktop AttnRes lab remains a directed
follow-on, not current earn-loop MVP scope, unless explicitly scheduled.

## Implementation Status

- 2026-03-17: phase 1 landed in `openagents` as a replay-first `AttnRes Lab`
  pane with app-owned state, pane registration, renderer wiring, WGPUI view
  mapping, and basic state/registry coverage.
- Remaining follow-ons are still the live inference/parity wiring, the Psionic
  stepwise training runner, and the final interactive control closure.

## Scope

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `Cargo.toml`
- `apps/autopilot-desktop/Cargo.toml`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/app_state_domains.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/research_control.rs`
- `apps/autopilot-desktop/src/panes/mod.rs`
- `apps/autopilot-desktop/src/panes/local_inference.rs`
- `apps/autopilot-desktop/src/panes/psionic_viz.rs`

Original AttnRes sources reviewed:

- `/Users/christopherdavid/code/attnres/README.md`
- `/Users/christopherdavid/code/attnres/examples/demo_tui.rs`
- `/Users/christopherdavid/code/attnres/examples/train_tiny.rs`

Current Psionic sources reviewed:

- `/Users/christopherdavid/code/psionic/docs/ARCHITECTURE.md`
- `/Users/christopherdavid/code/psionic/docs/TRAIN_SYSTEM.md`
- `/Users/christopherdavid/code/psionic/crates/psionic-models/src/attnres.rs`
- `/Users/christopherdavid/code/psionic/crates/psionic-runtime/src/attnres.rs`
- `/Users/christopherdavid/code/psionic/crates/psionic-train/src/attnres.rs`
- `/Users/christopherdavid/code/psionic/crates/psionic-serve/src/attnres.rs`
- `/Users/christopherdavid/code/psionic/crates/psionic-research/src/attnres_residual_comparison.rs`

Verification commands run against current `~/code/psionic`:

- `cargo test -p psionic-models attnres --quiet`
- `cargo test -p psionic-train attnres --quiet`
- `cargo test -p psionic-serve attnres --quiet`
- `cargo test -p psionic-eval attnres --quiet`

## Executive Summary

The current answer is:

- Psionic already implements a real AttnRes CPU-reference model family,
  diagnostics snapshot types, two-phase parity helpers, a bounded tiny-training
  lane, and a bounded served text-generation lane.
- OpenAgents already has the pane system, app-owned state model, renderer
  dispatch, input routing, and WGPUI HUD primitives needed to host an AttnRes
  lab pane.
- OpenAgents does not currently contain any AttnRes pane, AttnRes pane state,
  AttnRes pane actions, AttnRes controller worker, or AttnRes render mapping.
- The biggest missing technical contract for a true tweet-equivalent live lab is
  not WGPUI. It is the lack of a stepwise live training observer or stream in
  Psionic. Current `psionic-train::train_attnres_tiny_next_token(...)` returns a
  completed outcome after the fixed-budget run, with per-step metrics but no
  live UI callback surface.
- The correct port plan is:
  1. add an app-owned AttnRes pane and replay snapshot mapping in
     `apps/autopilot-desktop`
  2. wire live inference and diagnostics using current Psionic APIs
  3. extend Psionic with a renderer-neutral step observer or stream for live
     training
  4. finish the pause/resume/speed/reset loop and only then claim parity with
     the original TUI experience

The most important ownership rule is unchanged:

- AttnRes model math, diagnostics truth, two-phase parity, and training
  semantics belong in `OpenAgentsInc/psionic`
- the pane, view state, event log, input controls, and WGPUI composition belong
  in `apps/autopilot-desktop`
- `crates/wgpui` should only receive generic primitives after the pane exists
  and at least one more pane wants the same abstraction

## MVP And Ownership Constraints

`docs/MVP.md` makes the current product focus explicit: the retained MVP is the
earn-first Autopilot desktop loop, not a research-workbench expansion. The
AttnRes lab does not block the MVP earn loop.

That matters for implementation shape:

- this should land as a bounded app-owned pane, not as a broad repo-expansion
- it should not pull archived backroom code into the pruned repo by default
- it should reuse retained pane and WGPUI patterns instead of inventing a new
  shell

`docs/OWNERSHIP.md` is also clear:

- `apps/autopilot-desktop` owns pane orchestration, app-level event routing, UX
  flows, and app-owned execution snapshots
- `crates/wgpui` owns product-agnostic UI APIs, not product workflows
- `OpenAgentsInc/psionic` owns reusable compute execution substrate, backend
  implementations, diagnostics truth, and later training-class execution

So the port target is not "put the AttnRes app into `crates/wgpui`". The port
target is "build an app-owned `AttnRes Lab` pane that consumes Psionic truth and
uses WGPUI primitives".

## What The Original TUI Actually Provides

The original `demo_tui.rs` is not just a graph. It is a specific information
architecture and control model:

- Controls:
  - `Space` starts, pauses, and resumes
  - `Up` and `Down` change training speed
  - `Left` and `Right` move the selected sublayer
  - `Tab` cycles views
  - `1`, `2`, and `3` jump to `Overview`, `Pipeline`, and `Inference`
  - `?` opens help
  - `r` resets
- View families:
  - `Overview` shows training progress, loss/EMA, architecture/runtime cards,
    routing heatmap, selected sublayer detail, and event feed
  - `Pipeline` shows the algorithm filmstrip, routing logits, softmax weights,
    route story, block schedule, and event feed
  - `Inference` shows two-phase parity, merge split, block cache, selected
    detail, schedule, and event feed

There is also an important semantic fact:

- the original TUI trains on synthetic random token batches each step
- it recomputes diagnostics directly from the live Burn model every tick

That means a "full port" has to decide whether it is preserving:

- the TUI's information architecture and feel, or
- the TUI's exact synthetic training semantics

Those are not the same thing.

## What Is Implemented Today

### 1. Psionic AttnRes core is real and current

Current `~/code/psionic` is clean and synced to `origin/main`, and the AttnRes
lane is already present and tested.

Implemented now:

- `psionic-models` provides the AttnRes CPU-reference model, diagnostics
  snapshots, and two-phase forward path
- `psionic-runtime` provides `AttnResDiagnosticsSnapshot`,
  `AttnResSublayerSnapshot`, and two-phase parity report helpers
- `psionic-train` provides `AttnResTinyTrainingCorpus`,
  `AttnResTinyTrainingConfig`, `AttnResTinyTrainingOutcome`, and
  `AttnResTinyTrainingStepMetrics`
- `psionic-serve` provides `LocalAttnResTextGenerationService`,
  `AttnResTextGenerationStep`, and a pull-driven generation stream with
  diagnostics per generated token
- `psionic-eval` and `psionic-research` already exercise the lane

This is enough to support:

- replayed or one-shot training summaries
- per-sublayer routing inspection
- two-phase parity views
- bounded served inference snapshots

### 2. The OpenAgents desktop shell is already capable of hosting it

The retained app already has the required shell:

- `PaneKind`, `PaneSpec`, pane registry, pane sizing, pane renderer dispatch,
  and pane hit-action routing
- app-owned pane state in `RenderState`
- existing pane patterns for controls and derived visualization
- WGPUI primitives already used in production panes:
  - `Heatmap`
  - `DotsGrid`
  - `Scanlines`
  - `RingGauge`
  - `SignalMeter`

Good local patterns to copy:

- `apps/autopilot-desktop/src/panes/local_inference.rs`
  for controls, state summary, and load/error/action presentation
- `apps/autopilot-desktop/src/panes/psionic_viz.rs`
  for derived telemetry visualization using current WGPUI primitives
- `apps/autopilot-desktop/src/research_control.rs`
  for app-owned background-control and persisted program-state patterns

### 3. The pinned OpenAgents Psionic revision already contains the basic AttnRes APIs

`openagents` is currently pinned to Psionic rev
`43992eceeb5297ed9eb6219e559a44a3de8a0941`.

That pinned rev already contains:

- `psionic-train::train_attnres_tiny_next_token(...)`
- `AttnResTinyTrainingOutcome`
- `AttnResTinyTrainingStepMetrics`
- `AttnResDiagnosticsSnapshot`
- `AttnResTextGenerationStep`
- `LocalAttnResTextGenerationStream`

That means the first pane implementation does not require a Psionic pin bump
just to start. A pin bump should only happen if the pane needs a newer helper or
if we add the missing live observer/stream surface upstream.

## What Is Not Implemented In OpenAgents Today

There is no current AttnRes pane in the retained repo.

Missing now:

- no `PaneKind::AttnResLab`
- no pane registry entry for an AttnRes lab
- no `apps/autopilot-desktop/src/panes/attnres_lab.rs`
- no AttnRes pane state in `RenderState`
- no AttnRes pane input or action model
- no AttnRes controller worker
- no renderer dispatch branch for AttnRes
- no hit testing or keyboard routing for the TUI-equivalent controls
- no snapshot persistence for selected view, selected sublayer, speed, or run
  status
- no OpenAgents docs that describe how to launch or use an AttnRes desktop lab

There is also no current direct `psionic-models` dependency in `openagents`.
That is not automatically a problem, but it matters if the app is expected to
construct `AttnResConfig` and `AttnResNextTokenSample` values directly.

## The Real Gaps

### Gap 1. Live training is not streamable yet

This is the critical blocker for parity with the original tweet/TUI demo.

Current Psionic training gives us:

- whole-run outcome
- per-step metrics after the run finishes
- final trained model
- checkpoints and receipts

Current Psionic training does not yet give us:

- a per-step callback from inside the training loop
- a pull stream of training updates
- pause/resume from the middle of a run
- app-facing "tick" updates carrying both metrics and fresh routing diagnostics

That means the desktop can already render:

- a replayed training session
- a completed run summary
- live inference diagnostics

But it cannot yet honestly reproduce:

- "train live and watch routing evolve in real time"

without extending Psionic.

### Gap 2. The original TUI semantics and current Psionic tiny-training semantics differ

The original TUI trains on synthetic random batches each step.
Current Psionic tiny-training is a bounded corpus-driven training lane with
receipts, held-out loss, and explicit artifacts.

That means the port must make a deliberate choice:

- preserve the original synthetic toy loop exactly, or
- preserve the TUI UX while switching the runtime truth to the Psionic training
  lane

Recommended choice:

- keep the TUI information architecture and controls
- make Psionic the single source of truth for training and diagnostics
- if a synthetic lab mode is still wanted for demo speed, implement that as an
  explicit Psionic-owned lab corpus or lab runner, not as UI-owned math

### Gap 3. App-owned snapshot shaping is still needed

The raw Psionic data structures are useful, but they are not the final UI state
shape.

The app still needs an app-owned snapshot that flattens:

- active view
- selected sublayer
- run status
- pause/resume state
- speed multiplier
- event feed
- training metrics history for charting
- inference parity summary
- currently selected diagnostics slice

That snapshot belongs in `apps/autopilot-desktop`, not in `crates/wgpui`, and
not in Psionic.

### Gap 4. A few convenience APIs may still be worth adding upstream

The app can start without them, but the port gets cleaner if Psionic exposes one
or both of these:

- a reference AttnRes lab corpus/config builder so the app does not need direct
  ownership of `AttnResConfig` and `AttnResNextTokenSample`
- a renderer-neutral training observer or stream type for live updates

Without that helper, `openagents` may need a new `psionic-models` dependency
just to construct corpus/config values for the training lane.

## TUI Surface To Current Port Readiness

| TUI surface | Current data source | Ready now | Notes |
| --- | --- | --- | --- |
| Overview loss chart | `AttnResTinyTrainingOutcome.step_metrics` | Partial | Replay is ready. Live charting needs a training observer or stream. |
| Overview selected sublayer detail | `AttnResDiagnosticsSnapshot.sublayers` | Yes | Current diagnostics already expose source logits, routing weights, block boundaries, and query norm. |
| Overview routing heatmap | `AttnResDiagnosticsSnapshot.sublayers` | Yes | Heatmap mapping is app work, not model work. |
| Pipeline algorithm explainer | Static text from original TUI | Yes | Pure app-owned content. |
| Pipeline logits/softmax bars | `source_logits` and `routing_weights` | Yes | Already present in Psionic diagnostics. |
| Pipeline block schedule | diagnostics boundary fields | Yes | Already present in Psionic diagnostics. |
| Inference two-phase parity | `forward_two_phase*` plus runtime parity helpers | Yes | Current Psionic exposes the needed model and parity APIs. |
| Live inference token-by-token diagnostics | `AttnResTextGenerationStep` stream | Yes | Current serve lane already emits step diagnostics. |
| Event feed | app-owned | No | Needs controller-generated event logging. |
| Pause/resume/speed/reset | app-owned plus train observer support | No | UI control logic is missing, and live training still needs an upstream stream/observer. |

## Recommended Architecture

### App-owned desktop surface

Add a new app-owned pane:

- `PaneKind::AttnResLab`
- title suggestion: `AttnRes Lab`

Recommended new files:

- `apps/autopilot-desktop/src/panes/attnres_lab.rs`
- `apps/autopilot-desktop/src/attnres_lab_control.rs`

Recommended touched files:

- `apps/autopilot-desktop/src/panes/mod.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/app_state_domains.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/render.rs`

The pane should own:

- view selection
- selected sublayer
- event feed
- current lab snapshot
- controls for run, pause, resume, reset, speed, and source mode

The pane should not own:

- model math
- parity calculation
- routing logic
- direct duplicate training loops ported from Burn

### Psionic-owned compute contract

Psionic should remain the source of truth for:

- AttnRes config and model construction
- training semantics
- routing diagnostics
- two-phase parity checks
- token generation diagnostics

Recommended upstream follow-on:

- add a renderer-neutral live-update contract such as
  `AttnResTinyTrainingUpdate`
- add a callback or stream API such as
  `train_attnres_tiny_next_token_with_observer(...)`
  or an iterator-style stream wrapper

The important part is not the exact type name. The important part is that the
desktop gets stepwise updates from Psionic rather than recreating the loop in UI
code.

### WGPUI role

Current WGPUI is already sufficient to start.

No WGPUI blocker was found for:

- heatmaps
- bar-like meters
- scanline overlays
- lattice/grid backdrops
- gauges and small telemetry accents

Potential later extractions:

- a reusable sparkline or loss-chart primitive
- a reusable stacked routing-bars primitive
- a reusable timeline/filmstrip card primitive

Those should not be extracted before the pane exists and proves they are reused.

## Implementation Plan

### Phase 0. Contract and scope lock

Goal:

- decide what "full port" means before writing pane code

Required decisions:

- canonical training semantics:
  - recommended: Psionic tiny-training truth, not UI-owned Burn replay
- initial data source:
  - recommended: current pinned Psionic rev for replay and live inference
- whether to add a small Psionic helper to avoid a direct `psionic-models`
  dependency in `openagents`

Deliverables:

- written decision on training semantics
- app-owned `AttnResLabSnapshot` shape
- Psionic follow-on task for live training observer/stream if needed

### Phase 1. Replay-first pane in OpenAgents

Goal:

- land the pane shell and prove the WGPUI mapping before live execution

Implementation:

- add `PaneKind::AttnResLab`
- add pane spec, sizing, and command-palette entry
- add `AttnResLabPaneState`
- add `attnres_lab.rs` painter
- map a fixed snapshot or a completed Psionic run into:
  - Overview
  - Pipeline
  - Inference
- add sublayer selection, tab selection, and event-feed rendering

Expected outcome:

- the desktop can already display the original information architecture
- no training loop or background worker is required yet

### Phase 2. Live inference and diagnostics worker

Goal:

- replace static/replay state with real Psionic-backed inference diagnostics

Implementation:

- add `attnres_lab_control.rs`
- run seeded AttnRes inference and two-phase parity in a background worker
- consume:
  - `forward_hidden_with_diagnostics(...)`
  - `forward_two_phase_hidden(...)`
  - runtime parity helpers
  - served generation step diagnostics when useful
- wire controls for:
  - selected sublayer
  - selected view
  - reset
  - sample prompt or token-sequence refresh

Expected outcome:

- Overview, Pipeline, and Inference views are driven by real Psionic outputs
- the pane is already useful even before live training lands

### Phase 3. Live training stream

Goal:

- close the main gap with the original tweet/TUI demo

Implementation:

- extend Psionic with a training observer or stream API
- emit per-step updates carrying:
  - `AttnResTinyTrainingStepMetrics`
  - fresh routing diagnostics snapshot
  - step number
  - run status
  - optional checkpoint or receipt references
- wire the pane controls:
  - start
  - pause
  - resume
  - reset
  - speed

Expected outcome:

- the desktop can honestly show routing evolving while the run progresses
- the loss chart, event feed, and selected sublayer all update live

### Phase 4. Product integration and cleanup

Goal:

- make the pane maintainable and predictable inside the retained app shell

Implementation:

- add persistence for last selected view, sublayer, and run mode
- add tests for:
  - snapshot shaping
  - controller state transitions
  - replay-to-paint mapping
  - input actions
- update docs once the pane actually exists
- only extract new generic WGPUI primitives after demonstrated reuse

Expected outcome:

- the pane behaves like the rest of the retained desktop app rather than like a
  one-off lab import

## Concrete Risks And Decisions

### 1. Exact TUI parity vs Psionic-truth parity

Risk:

- the team can accidentally spend time recreating Burn-era demo behavior instead
  of shipping a Psionic-owned desktop lab

Recommendation:

- port the TUI's UX structure, not its duplicate framework ownership

### 2. Dependency creep into the app

Risk:

- the app may take a direct dependency on `psionic-models` just to build
  training corpus/config values

Recommendation:

- prefer a small helper API in Psionic if that keeps the app on higher-level
  types

### 3. Premature WGPUI extraction

Risk:

- pushing pane-specific widgets into `crates/wgpui` before reuse is proven

Recommendation:

- keep first-pass composition app-owned

### 4. Reaching past MVP boundaries

Risk:

- the pane grows into a broad research-workbench expansion unrelated to current
  product goals

Recommendation:

- keep this as a bounded singleton pane with explicit non-MVP status unless
  reprioritized

## Exit Criteria For "Fully Ported And Working"

This port should only be called complete when all of the following are true:

- an `AttnRes Lab` pane opens inside `apps/autopilot-desktop`
- the pane supports the three original view families:
  - Overview
  - Pipeline
  - Inference
- the pane is fed by Psionic truth, not UI-owned duplicate model logic
- selected-sublayer inspection is driven by real diagnostics snapshots
- two-phase parity is computed from real Psionic model/runtime outputs
- training can run live with start, pause, resume, reset, and speed control
- the routing view changes during training from the live update stream
- the event feed reflects real controller events
- the pane state survives normal desktop app lifecycle expectations
- the pane has basic reducer/controller tests and user-facing docs

## Bottom Line

The port is feasible now, but only in stages.

Current OpenAgents and current Psionic are already ready for:

- a replay-first AttnRes pane
- real live inference diagnostics
- real two-phase parity views

They are not yet fully ready for:

- the full original "train live and watch routing evolve" desktop experience

The missing piece is a live training observer or stream in Psionic, not a lack
of WGPUI capability.

So the shortest correct path is:

1. build the app-owned pane now
2. wire replay and inference immediately
3. extend Psionic with the live training contract
4. finish the interactive controls after that contract exists
