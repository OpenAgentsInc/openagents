# 2026-03-16 attnres Port Into Psionic And WGPUI Audit

## Intent

This audit answers a concrete repo-local question:

> after reading the current Psionic roadmaps, `crates/psionic/README.md`, and
> `crates/psionic/docs/TRAIN_SYSTEM.md`, what is the ownership-correct way to
> port the full `~/code/attnres` codebase into OpenAgents, replace Burn with
> Psionic where appropriate, and migrate the TUI visualization experience into
> WGPUI?

The useful answer is not:

- "copy the crate into `crates/psionic/` and rename types"
- "keep Burn forever inside Psionic because the model already works"
- "recreate the terminal demo in app code with synthetic numbers"
- "treat the TUI, web demo, tests, and fixtures as optional because the model
  math is the only thing that matters"

The useful answer is:

- inventory every `attnres` surface
- map each surface onto current OpenAgents ownership boundaries
- identify the real Burn to Psionic semantic gaps
- recommend a dependency-ordered port sequence
- define how the terminal dashboards become app-owned WGPUI panes without
  duplicating the model logic again

This is a planning and architecture audit only. It does not widen active MVP
product scope in `docs/MVP.md`.

## Scope

Psionic/OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ROADMAP.md`
- `crates/psionic/docs/ROADMAP_CLUSTER.md`
- `crates/psionic/docs/ROADMAP_FM.md`
- `crates/psionic/docs/ROADMAP_METAL.md`
- `crates/psionic/docs/ROADMAP_MLX.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/psionic-core/src/lib.rs`
- `crates/psionic/psionic-ir/src/lib.rs`
- `crates/psionic/psionic-ir/src/autodiff.rs`
- `crates/psionic/psionic-runtime/src/lib.rs`
- `crates/psionic/psionic-backend-cpu/src/lib.rs`
- `crates/psionic/psionic-models/src/lib.rs`
- `crates/psionic/psionic-models/src/tassadar_executor_transformer.rs`
- `crates/psionic/psionic-train/src/lib.rs`
- `crates/psionic/psionic-train/src/core_loop.rs`
- `crates/psionic/psionic-train/src/optimizer.rs`
- `crates/wgpui/src/components/hud/heatmap.rs`
- `apps/autopilot-desktop/src/panes/psionic_viz.rs`
- `apps/autopilot-desktop/src/panes/frame_debugger.rs`
- `apps/autopilot-desktop/src/panes/key_ledger.rs`

Additional Burn sources reviewed for prerequisite planning:

- `/Users/christopherdavid/code/burn/README.md`
- `/Users/christopherdavid/code/burn/Cargo.toml`
- `/Users/christopherdavid/code/burn/burn-book/src/building-blocks/autodiff.md`
- `/Users/christopherdavid/code/burn/burn-book/src/building-blocks/module.md`
- `/Users/christopherdavid/code/burn/burn-book/src/building-blocks/learner.md`
- `/Users/christopherdavid/code/burn/burn-book/src/custom-training-loop.md`
- `/Users/christopherdavid/code/burn/contributor-book/src/guides/adding-a-new-operation-to-burn.md`
- `/Users/christopherdavid/code/burn/contributor-book/src/project-architecture/module.md`
- `/Users/christopherdavid/code/burn/contributor-book/src/project-architecture/serialization.md`
- `/Users/christopherdavid/code/burn/crates/burn-core/src/module/param/base.rs`
- `/Users/christopherdavid/code/burn/crates/burn-derive/src/module/base.rs`
- `/Users/christopherdavid/code/burn/crates/burn-derive/src/module/record_struct.rs`
- `/Users/christopherdavid/code/burn/crates/burn-dispatch/src/ops/tensor.rs`
- `/Users/christopherdavid/code/burn/crates/burn-store/src/lib.rs`
- `/Users/christopherdavid/code/burn/crates/burn-store/src/traits.rs`
- `/Users/christopherdavid/code/burn/crates/burn-store/src/pytorch/mod.rs`
- `/Users/christopherdavid/code/burn/crates/burn-train/src/metric/store/base.rs`
- `/Users/christopherdavid/code/burn/crates/burn-train/src/renderer/base.rs`
- `/Users/christopherdavid/code/burn/crates/burn-train/src/renderer/tui/metric_numeric.rs`
- `/Users/christopherdavid/code/burn/crates/burn-train/src/renderer/tui/renderer.rs`
- `/Users/christopherdavid/code/burn/examples/custom-renderer/src/lib.rs`
- `/Users/christopherdavid/code/burn/examples/text-generation/src/training.rs`

`attnres` sources reviewed from `~/code/attnres`:

- `README.md`
- `ROADMAP.md`
- `ARCHITECTURE.md`
- `Cargo.toml`
- `src/*.rs`
- `tests/*.rs`
- `examples/compare_residuals.rs`
- `examples/visualize_weights.rs`
- `examples/train_tiny.rs`
- `examples/demo_tui.rs`
- `benches/attn_res_benchmark.rs`
- `fixtures/*.json`
- `web-demo/crate/src/*.rs`
- `web-demo/src/*.ts`

## Executive Summary

Porting `attnres` into this repo is reasonable, but only if it is treated as a
bounded Psionic model-family and visualization program, not as a Burn-shaped
crate transplant.

The correct owner split is:

- `crates/psionic/psionic-models` for the AttnRes model family, config,
  descriptors, weight bundles, and reusable diagnostics types
- `crates/psionic/psionic-runtime` for the CPU-reference forward path and the
  two-phase inference runtime
- `crates/psionic/psionic-train` for the tiny training reference loop,
  checkpoints, optimizer wiring, and train receipts
- `crates/psionic/psionic-eval` for parity, property, differential, and
  benchmark coverage
- `apps/autopilot-desktop` for the WGPUI "AttnRes Lab" pane and control flow
- `crates/wgpui` only for generic visualization primitives that become
  demonstrably reusable after the app port exists

The biggest architectural fact is this:

`attnres` depends on far more than matmul and RMSNorm. It relies on a full
small-framework surface from Burn:

- parameter trees and module composition
- embedding lookup and linear layers
- tensor view ops
- softmax, exp, max, mean, reshape, swap, repeat, masking, and activation ops
- autodiff through the whole forward path
- optimizer integration
- record serialization

Current Psionic already has useful pieces:

- compact tensor and graph substrate
- CPU reference backend
- backend extensions for `rms_norm`, `layer_norm`, `rotary_embedding`, and
  scaled dot-product attention
- a typed optimizer and training-core substrate

But current Psionic is still missing enough public framework semantics that a
direct Burn to Psionic rewrite would become an ad hoc pile of one-off
shortcuts unless the port is sequenced carefully.

The most important technical constraint is that backend-extension autodiff is
currently refused in `psionic-ir`. That means existing `rms_norm` and
`scaled_dot_product_attention` extensions are not by themselves sufficient to
recreate `train_tiny` honestly. The port must either:

- expand generic differentiable framework semantics first, or
- add explicit backward support for the required extension ops before claiming
  train parity

The TUI port should not reproduce the current mistake from `attnres/web-demo`,
which ships a second independent algorithm implementation just to drive
visuals. OpenAgents should keep one AttnRes runtime truth and feed both the
WGPUI pane and any later web surface from shared diagnostics snapshots.

The Burn-specific prerequisite details now live in the companion audit:

- [2026-03-16-burn-prerequisites-for-attnres-psionic-port-audit.md](./2026-03-16-burn-prerequisites-for-attnres-psionic-port-audit.md)

## What attnres Actually Contains

`attnres` is not just one model file. It is five separate surfaces.

### 1. Core model library

Files:

- `src/config.rs`
- `src/block_state.rs`
- `src/rms_norm.rs`
- `src/attention.rs`
- `src/feed_forward.rs`
- `src/attn_res_op.rs`
- `src/layer.rs`
- `src/model.rs`
- `src/two_phase.rs`
- `src/serialization.rs`
- `src/utils.rs`

What it does:

- defines `AttnResConfig`
- defines the block-state invariant
- implements the learned depth-routing residual op
- builds a small decoder-style transformer
- adds the paper's two-phase inference path
- adds basic config and weight serialization

### 2. Verification surface

Files:

- `tests/unit_tests.rs`
- `tests/integration_tests.rs`
- `tests/property_tests.rs`
- `tests/differential_tests.rs`
- `fixtures/attn_res_forward.json`
- `fixtures/block_state_tracking.json`

What it defends:

- zero-init pseudo-query means uniform averaging
- weights sum to one over depth
- block-boundary behavior is exact
- full AttnRes and block AttnRes both behave as intended
- two-phase forward matches standard forward within bounded error
- gradients propagate through the model
- serialization round-trips preserve outputs

### 3. Training and benchmark harnesses

Files:

- `examples/train_tiny.rs`
- `benches/attn_res_benchmark.rs`

What they do:

- tiny next-token training with Adam and cross-entropy
- forward-path microbenchmarks for op, model, masked model, and sequence
  scaling

### 4. Research and visualization examples

Files:

- `examples/compare_residuals.rs`
- `examples/visualize_weights.rs`
- `examples/demo_tui.rs`

What they do:

- compare uniform AttnRes against mean residuals
- inspect learned routing weights
- provide a fully interactive dashboard with:
  - overview
  - pipeline filmstrip
  - inference/two-phase view
  - live training telemetry
  - event feed
  - selected-sublayer inspector

### 5. Web demo stack

Files:

- `web-demo/crate/src/lib.rs`
- `web-demo/crate/src/tensor.rs`
- `web-demo/src/*.ts`

What it does:

- duplicates the AttnRes algorithm in a separate pure-Rust WASM engine
- drives heatmap, bar chart, loss curve, norms chart, and static architecture
  diagrams in a browser

This is useful as reference material, but it is exactly the kind of duplicate
runtime truth OpenAgents should avoid.

## Ownership-Correct Landing Zones

| `attnres` surface | OpenAgents target | Why |
| --- | --- | --- |
| `config.rs`, `block_state.rs`, model descriptors, weight metadata | `crates/psionic/psionic-models/src/attnres.rs` | Reusable model-family ownership belongs in `psionic-models`, not the app |
| `attn_res_op.rs`, `layer.rs`, `model.rs`, `two_phase.rs` CPU reference execution | `crates/psionic/psionic-runtime/src/attnres.rs` plus `psionic-models` family wrappers | Runtime execution truth belongs in Psionic runtime; model family identity belongs in `psionic-models` |
| tiny training loop and checkpointed reference run | `crates/psionic/psionic-train/src/attnres.rs` | Training control and receipts belong in `psionic-train` |
| parity, property, differential, and benchmark harnesses | `crates/psionic/psionic-eval/src/attnres.rs` and crate tests | Eval and benchmark truth belong in `psionic-eval` |
| fixtures JSON | `crates/psionic/fixtures/attnres/` | Keeps parity fixtures next to other Psionic-owned fixtures |
| Burn record serialization | replace with Psionic-native `safetensors` plus JSON manifest in `psionic-models`; optional one-shot Burn import tool in `psionic-research` or gated bin | Burn should not become a long-lived runtime dependency of Psionic |
| `compare_residuals.rs` and `visualize_weights.rs` narrative programs | `crates/psionic/psionic-research` or `psionic-eval` examples | These are experiment drivers and reports, not app UX and not core runtime |
| `demo_tui.rs` | `apps/autopilot-desktop/src/panes/attnres_lab.rs` plus app state/reducers | The interactive visual experience is app-owned product UX |
| `web-demo` visual language | use as visual reference only for WGPUI pane; do not port the duplicate WASM engine | One model runtime should feed all views |

## Burn To Psionic Gap Analysis

### What Burn is doing for attnres today

`attnres` uses Burn for all of the following:

- `#[derive(Module)]` parameter trees
- `Param<Tensor<_>>`
- embedding and linear layers
- tensor shape transforms such as:
  - `unsqueeze_dim`
  - `squeeze_dim`
  - `reshape`
  - `swap_dims`
  - `repeat_dim`
- tensor math such as:
  - `sum_dim`
  - `mean_dim`
  - `max_dim`
  - `max_pair`
  - `exp`
  - `sqrt`
  - `powf_scalar`
  - `softmax`
  - `matmul`
  - `clamp_min`
- masking helpers such as `triu`
- autodiff end to end through the forward graph
- optimizer wiring through Adam
- record-based save/load

### What current Psionic already has that helps

- compact tensor metadata and graph substrate in `psionic-core` and
  `psionic-ir`
- CPU reference execution in `psionic-backend-cpu`
- reusable optimizer math in `psionic-train`
- reusable model weight metadata and `safetensors`-backed loading surfaces in
  `psionic-models`
- backend extensions for:
  - `rms_norm`
  - `layer_norm`
  - `scaled_dot_product_attention`
  - RoPE

### What is still missing for an honest port

Current Psionic does not yet expose enough general framework semantics to
express AttnRes cleanly as a trainable model family.

The missing or incomplete categories are:

- differentiable elementwise ops beyond add and multiply:
  - subtraction
  - division
  - max-pair
  - exp
  - sqrt as graph op
  - power
- differentiable reductions beyond sum:
  - mean
  - max
  - log-sum-exp or equivalent softmax support
- common view helpers:
  - squeeze
  - unsqueeze
  - transpose or swap-dims convenience
  - repeat or tile
- indexing and lookup:
  - embedding or gather semantics
- activations and losses:
  - GELU
  - softmax
  - cross entropy
- train-time semantics:
  - dropout posture
  - module or parameter tree surface
  - state-tree load and save discipline
- autodiff for backend extensions

That last point matters immediately:

- `psionic-ir::gradient_support_for_op(...)` currently marks
  `BackendExtension` as unsupported for reverse-mode gradients
- so a model graph built directly from existing Psionic backend extensions will
  not reproduce `train_tiny` honestly

### Practical implication

The AttnRes port should be treated as a bounded semantics pilot for
`ROADMAP.md` and `ROADMAP_MLX.md`, not as a special-case reason to bypass the
framework-core work.

The right rule is:

- add missing generic semantics where they are generally useful
- keep fused or backend-specific AttnRes fast paths as a later optimization
- do not solve the first port by hiding the missing framework in one opaque
  AttnRes-only backend extension

## Burn-Derived Prerequisites

The deeper Burn review changes the roadmap in one useful way: some prerequisites
that were previously implied should be made explicit before the AttnRes port
starts claiming training or checkpoint parity.

The relevant Burn lessons are not "copy Burn APIs." They are:

- stable parameter identity and parameter-tree traversal are prerequisites for
  honest optimizer state, checkpoint restore, and selective import
- adding an op is not done when the forward path works; it is done when reverse
  mode, tests, and backend admission are also defined
- model IO needs a native Psionic artifact contract plus an optional import
  bridge for foreign weights
- visualization should sit on top of event or snapshot feeds, not by letting UI
  code inspect live model internals ad hoc

That translates into four explicit prerequisites:

1. A small Psionic parameter-tree contract

- enough to enumerate trainable tensors, assign stable IDs, and apply updates
  or imported values by path or ID
- this should live across `psionic-models` and `psionic-train`, not as an
  AttnRes-only shim

2. An AttnRes admission rule for new Psionic ops

- no AttnRes-enabling op should be considered "ported" until the CPU reference
  implementation, autodiff posture, and test coverage are all in place
- this is the Burn op-extension lesson, translated into Psionic terms

3. A Psionic-native checkpoint and import boundary

- native saved form remains `safetensors` plus manifest
- Burn is allowed only as a one-shot importer for legacy artifacts when needed

4. A diagnostics event or snapshot contract for the future WGPUI pane

- the pane should subscribe to runtime or training snapshots
- it should not become a second place where AttnRes math is recomputed

The companion prerequisite audit expands these points in detail and maps them
onto the current Psionic crates.

## Recommended Port Sequence

### Phase 1: AttnRes-Enabling Psionic Semantics

Goal:

- make Psionic capable of expressing AttnRes forward and backward honestly on
  the CPU reference lane

Required work:

- add a minimum reusable parameter-tree surface for trainable model families:
  - stable parameter IDs
  - trainable versus frozen parameter classification
  - parameter enumeration or visit support for optimizer application and import
- add the minimum generic tensor semantics required by AttnRes:
  - subtract and divide
  - mean reduction
  - max reduction or equivalent softmax support
  - squeeze and unsqueeze
  - repeat or broadcast helpers
  - embedding or gather
  - GELU
  - softmax
- either:
  - implement autodiff for the needed backend extensions, or
  - express RMSNorm and attention through differentiable primitive ops for the
    reference lane
- adopt a Burn-style completeness rule for each newly added AttnRes-enabling
  op:
  - graph admission
  - CPU reference execution
  - reverse-mode support or explicit refusal
  - parity tests

Exit criteria:

- reusable parameter traversal exists for AttnRes-class models
- a CPU-reference graph can represent the AttnRes forward path
- autodiff can propagate through the reference path
- no Burn dependency is needed for forward or backward

### Phase 2: Reusable AttnRes Model Family In Psionic

Goal:

- land the reusable model-family types and CPU-reference model execution

Recommended files:

- `crates/psionic/psionic-models/src/attnres.rs`
- `crates/psionic/psionic-runtime/src/attnres.rs`

Recommended contents:

- `AttnResConfig`
- `AttnResBlockState`
- `AttnResSublayerKind`
- `AttnResSublayerSnapshot`
- `AttnResDiagnosticsSnapshot`
- `AttnResModelDescriptor`
- `AttnResWeightBundle`
- CPU-reference forward
- CPU-reference `forward_hidden`
- CPU-reference two-phase forward

Port notes by source file:

- `config.rs`: mostly direct port
- `block_state.rs`: direct port
- `rms_norm.rs`: port as reusable AttnRes-facing helper backed by Psionic
  semantics
- `attention.rs`: reuse generic Psionic attention building blocks; do not leave
  it as a Burn-only module wrapper
- `feed_forward.rs`: re-express on top of Psionic linear plus GELU semantics
- `attn_res_op.rs`: keep as explicit depth-routing op in model code first; add
  runtime fusion later only if needed
- `layer.rs` and `model.rs`: port closely because their block-boundary logic is
  the real algorithmic invariant
- `two_phase.rs`: port into runtime-owned code because it is an execution-mode
  distinction, not just model metadata

Exit criteria:

- standard forward and two-phase forward both run through Psionic CPU
- model diagnostics expose routing weights, logits, query norms, block
  boundaries, and partial-block state without app-specific logic

### Phase 3: Tests, Fixtures, And Benchmark Truth

Goal:

- make the Psionic AttnRes port at least as defensible as the original crate

Recommended landing zones:

- `crates/psionic/psionic-models/tests/attnres_*.rs`
- `crates/psionic/psionic-eval/src/attnres.rs`
- `crates/psionic/fixtures/attnres/*`

Tests to port first:

- zero-init uniform averaging
- single-source identity
- weight-sum-to-one over depth
- block-boundary matrix
- odd block-size boundary before MLP
- full AttnRes sublayer splitting
- large-magnitude and near-zero stability
- two-phase equivalence
- differential fixtures
- property tests for boundedness and finiteness

Benchmark port recommendation:

- keep developer-local Criterion benches if useful
- also add Psionic-owned benchmark receipts in `psionic-eval`, because the rest
  of Psionic uses machine-readable benchmark truth rather than raw ad hoc
  output

Exit criteria:

- the original `attnres` invariants are defended in Psionic-owned tests
- two-phase parity thresholds are documented and enforced
- benchmark results are captured as receipts, not only console output

### Phase 4: Training And Checkpoint Port

Goal:

- replace `examples/train_tiny.rs` and Burn recorder save/load with Psionic
  train and model IO surfaces

Recommended files:

- `crates/psionic/psionic-train/src/attnres.rs`
- `crates/psionic/psionic-eval/src/attnres_training.rs`

Recommended behavior:

- tiny fixed-budget next-token training reference run
- explicit train manifest and checkpoint lineage
- typed optimizer config using `TrainingOptimizerConfig::adam(...)` or
  `adamw(...)`
- optimizer state keyed by stable Psionic parameter identity rather than
  positional assumptions
- held-out eval that confirms training actually changes routing and loss
- optional one-shot Burn import path for legacy `.mpk` or Burn-managed weights
  only if migration pressure is real

Serialization rule:

- canonical saved format should be Psionic-native:
  - `safetensors` weights
  - JSON manifest or descriptor
  - explicit config digest
- Burn `.mpk` should not become the canonical persisted format inside Psionic

If legacy Burn checkpoints matter:

- add a one-shot importer binary behind a dedicated feature or in a research
  tool crate
- use Burn only long enough to convert old artifacts into Psionic-native weight
  bundles

Exit criteria:

- tiny training runs end to end without Burn
- checkpoints restore correctly
- loss and routing diagnostics are persisted as Psionic-owned artifacts

### Phase 5: WGPUI AttnRes Lab

Goal:

- port the `demo_tui` experience into the desktop app using real Psionic
  diagnostics

Recommended app-owned files:

- `apps/autopilot-desktop/src/panes/attnres_lab.rs`
- `apps/autopilot-desktop/src/attnres_lab_control.rs`
- `apps/autopilot-desktop/src/input/reducers/attnres_lab.rs`
- state additions in app-owned pane state modules and registry wiring

Recommended UI rule:

- do not move AttnRes business logic into `crates/wgpui`
- only extract a new WGPUI primitive when at least two panes want the same
  thing

How the TUI maps into WGPUI:

| TUI surface | WGPUI port |
| --- | --- |
| overview metrics cards | app-owned cards using existing pane shell patterns |
| depth routing heatmap | reuse `wgpui::components::hud::Heatmap` |
| loss chart | app-owned line chart painter, similar to current custom pane painters |
| norms chart | app-owned multi-series line chart painter |
| routing mass / logits bars | app-owned bar chart painter |
| selected sublayer inspector | standard text and quad composition |
| event feed | standard list rendering |
| algorithm filmstrip | app-owned timeline/step chips |
| two-phase merge gauges | reuse existing gauge primitives where possible |
| scanline HUD feel | reuse `Scanlines`, `DotsGrid`, and existing pane-shell motifs |

Recommended data flow:

- Psionic emits `AttnResDiagnosticsSnapshot`
- `psionic-train` or the app-owned control worker also emits metric and
  lifecycle snapshots in a renderer-neutral format
- app control worker runs train step / reset / inspect actions
- pane consumes immutable snapshots
- WGPUI paints from snapshot only

That avoids both of these bad outcomes:

- UI code recomputing model math
- a second visualization-only AttnRes implementation

Exit criteria:

- the pane can run live training, reset, pause, select sublayers, and switch
  views
- all displayed values come from Psionic diagnostics snapshots
- no synthetic placeholder routing field is needed after initial boot

### Phase 6: Optional Follow-Ons

These are reasonable later, but they should not block the main port.

Possible follow-ons:

- `psionic-research` experiment runners for residual-vs-AttnRes comparisons
- `psionic-serve` integration if AttnRes becomes a serious served text model
- later web demo generated from shared Psionic diagnostics instead of the
  duplicate WASM engine
- backend acceleration on Metal or CUDA once the CPU reference lane is honest

Non-goal for the first port:

- cluster execution
- Apple FM integration
- compute-market provider exposure

Those roadmaps are orthogonal. AttnRes is a bounded model and framework pilot,
not a reason to widen product scope.

## TUI To WGPUI Port Details

The `demo_tui` is worth porting almost one-for-one because it encodes the
right questions:

- what source blocks does each sublayer route to?
- how selective is that routing?
- when does a block boundary occur?
- how does two-phase execution differ from standard execution?
- how does training change `||w_l||`, entropy, and dominant routes?

The most useful port is not a generic dashboard. It is a focused AttnRes lab
with the same three view families the TUI already proved out:

### Overview

Keep:

- training progress
- loss and EMA
- average selectivity
- average and max query norm
- depth-routing heatmap
- selected sublayer card
- event feed

### Pipeline

Keep:

- algorithm filmstrip
- per-source logits bars
- per-source routing-mass bars
- route story
- block schedule

### Inference

Keep:

- standard vs two-phase parity metric
- phase-1 versus phase-2 schedule view
- online merge gauges
- block-cache health

WGPUI implementation notes:

- `Heatmap` already exists and should handle the routing matrix immediately
- sparkline, loss line, and norms line charts should start as app-owned custom
  painters, similar to current hand-painted chart work in
  `frame_debugger.rs`, `key_ledger.rs`, and `psionic_viz.rs`
- if those chart painters become reused elsewhere, then extract generic
  `wgpui` primitives after the pane proves the API

## What Should Not Be Ported Literally

Several `attnres` surfaces should be translated, not copied.

### 1. Burn module derivations

Do not recreate Burn's module system inside Psionic just for AttnRes.

Instead:

- use this port to advance Psionic's own model-family and parameter-tree story

### 2. Burn record formats as canonical artifacts

Do not make `.mpk` a permanent Psionic storage contract.

Instead:

- adopt Psionic-native weight bundles and descriptors
- quarantine Burn import as an optional migration bridge only

### 3. The duplicate web-demo engine

Do not carry both:

- one Psionic AttnRes implementation
- one visualization-only duplicate runtime

Instead:

- keep one AttnRes runtime truth
- derive browser or desktop views from shared diagnostics data

### 4. Ratatui-specific layout code

Do not attempt a line-by-line port of the terminal widgets.

Instead:

- port the information architecture and interaction model
- redesign the actual scene composition for WGPUI's retained GPU UI

## Acceptance Gates For An Honest Port

The port is only done when all of the following are true.

- CPU reference forward works in Psionic without Burn
- two-phase forward matches standard forward within documented parity budgets
- property, integration, unit, and differential tests are ported
- gradient flow exists through the trainable path used by the tiny trainer
- checkpoints restore and preserve outputs
- the WGPUI pane reads real diagnostics from the Psionic runtime
- the pane can show:
  - routing heatmap
  - selected-sublayer details
  - training loss history
  - query-norm history
  - two-phase parity status
- no product-specific business logic leaks into `crates/wgpui`
- no Burn runtime dependency remains on the main execution path

## Recommended First Implementation Slice

If this port starts immediately, the best first slice is not the full UI.

The highest-signal first landing is:

1. add the missing differentiable Psionic semantics required for CPU-reference
   AttnRes
2. land the minimum reusable parameter-tree contract needed for optimizer state
   and import
3. land `psionic-models::attnres` plus fixtures and core parity tests
4. land `psionic-runtime::attnres` two-phase execution and diagnostics
5. land a minimal desktop AttnRes pane that can replay fixture-backed
   diagnostics

That slice proves the owner split and removes the biggest risk:

- getting stuck with a WGPUI port that has no stable runtime truth behind it

After that:

6. land training and checkpoint support in `psionic-train`
7. switch the pane from fixture replay to live Psionic train-step snapshots
8. add benchmark receipts and optional research or serve follow-ons

## Bottom Line

`attnres` should come into this repo as:

- a Psionic research model family with CPU-reference execution truth
- a Psionic training and eval reference lane
- an app-owned WGPUI lab pane fed by shared diagnostics snapshots

It should not come in as:

- a Burn island inside `crates/psionic/*`
- a second duplicate visualization runtime
- a WGPUI-owned bundle of model-specific logic
- a premature served-product or marketplace lane

The port is worth doing because it is a bounded, useful forcing function for
the exact Psionic gaps the roadmaps already identify:

- broader framework semantics
- honest autodiff and trainability
- reusable model-family ownership
- truthful runtime diagnostics
- richer app-owned visualization on top of shared machine-facing truth
