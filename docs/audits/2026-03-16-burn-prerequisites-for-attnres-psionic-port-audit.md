# 2026-03-16 Burn Prerequisites For AttnRes Psionic Port Audit

## Intent

This audit answers the narrower follow-up question behind the main AttnRes port
roadmap:

> after looking more deeply through `~/code/burn`, which Burn ideas are actual
> prerequisites for an honest `attnres -> Psionic` port, and which ones should
> stay behind?

This is not a recommendation to pull Burn into Psionic as a permanent runtime
dependency.

It is a prerequisite audit for
[2026-03-16-attnres-port-into-psionic-and-wgpui-audit.md](./2026-03-16-attnres-port-into-psionic-and-wgpui-audit.md).

## Sources Reviewed

Burn sources reviewed:

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

OpenAgents/Psionic sources re-checked while writing this prerequisite audit:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/psionic-ir/src/autodiff.rs`
- `crates/psionic/psionic-models/src/lib.rs`
- `crates/psionic/psionic-train/src/lib.rs`
- `crates/psionic/psionic-train/src/optimizer.rs`

## Executive Summary

Burn contributes five real ideas that matter to the AttnRes port:

1. parameter identity is first-class
2. module traversal is the bridge between autodiff, optimizer state, and
   serialization
3. a new op is only "real" once forward, backward, and tests all exist
4. storage is native-first but adapters are allowed at the boundary
5. the dashboard is renderer-neutral event consumption, not model-specific UI

Psionic does not need Burn's syntax, derives, or recorder formats.

Psionic does need the equivalent contracts where AttnRes depends on them:

- stable trainable-parameter identity
- reusable parameter traversal
- an op-admission rule that includes autodiff posture
- native checkpoint layout plus an optional Burn import bridge
- runtime and training diagnostics snapshots that the WGPUI pane can consume

The key practical conclusion is:

- the AttnRes port should not start with "rewrite the Burn model in Psionic"
- it should start with a small prerequisite layer that makes optimizer state,
  checkpoint restore, and diagnostics ownership coherent

## What Burn Is Actually Providing To attnres

Burn is doing more than tensor math for `attnres`.

### 1. Stable parameter identity

`Param<Tensor<_>>` gives Burn three things at once:

- a stable parameter ID
- trainable versus frozen posture
- inclusion in save/load and optimizer traversal

That matters directly for `attnres` because:

- `pseudo_query` and `gamma` are trainable parameters with semantic identity
- tiny training needs gradients mapped back onto those exact parameters
- checkpoint restore cannot rely on positional tensor ordering if the model
  grows or changes

Psionic implication:

- AttnRes should not introduce its own ad hoc `Vec<f32>` parameter registry
- Psionic needs a small reusable parameter-tree contract for model families

### 2. Module traversal as infrastructure

Burn's `Module::map` and `Module::visit` are the hidden infrastructure behind:

- optimizer updates
- state export
- state import
- parameter counting and inspection

This is one of the most important prerequisite ideas from Burn because Psionic
already has reusable optimizer math in `psionic-train`, but that optimizer
surface currently works on naked vectors. What it does not yet own is the
bridge from "model family with named parameters" to "vector or tensor updates
applied to the correct parameter".

Psionic implication:

- add parameter traversal before claiming checkpoint or training parity for
  AttnRes
- keep it generic enough that later model families can reuse it

### 3. Op completeness discipline

Burn's contributor guidance for adding a new op is clear:

- extend the tensor API
- implement backend execution
- implement autodiff
- add tests

That is the exact discipline Psionic needs while closing the AttnRes gap.
Right now, `psionic-ir` still marks `BackendExtension` ops as unsupported for
reverse-mode gradients, which means "forward works" is not enough for training.

Psionic implication:

- every AttnRes-enabling primitive or extension needs an admission checklist:
  - graph admission
  - CPU reference execution
  - reverse-mode support or explicit refusal
  - tests
- do not let AttnRes bypass missing generic framework semantics by hiding them
  inside opaque backend extensions

### 4. Storage and adapter boundary

Burn's storage stack is useful mostly as a boundary lesson:

- keep a native storage contract
- allow adapters and remappers at import time
- do not let foreign formats define the core runtime contract

The useful part for Psionic is not Burn's `.mpk` format. The useful part is the
idea that a foreign artifact loader can exist without becoming the canonical
internal model format.

Psionic implication:

- canonical AttnRes storage should remain `safetensors` plus Psionic manifest
- a one-shot Burn import tool is acceptable if legacy artifacts matter
- Burn should not sit on the main runtime or training path once import is done

### 5. Renderer-neutral metric and progress events

Burn's TUI renderer is not a model-specific training dashboard. It is a consumer
of metric and progress events delivered through a renderer interface. The TUI
thread, ratatui widgets, and alternate-screen handling are implementation
details. The important architecture is:

- training emits progress and metric updates
- a renderer consumes those updates
- the renderer can be swapped without changing model logic

That is exactly the lesson the WGPUI AttnRes port needs.

Psionic implication:

- the future `AttnRes Lab` pane should consume snapshots or renderer-neutral
  events
- it should not read mutable model internals directly or recompute AttnRes math
  in UI code

## Burn Ideas That Should Become Psionic Prerequisites

| Burn idea | Why `attnres` depends on it | Psionic prerequisite | Owner |
| --- | --- | --- | --- |
| `Param<Tensor<_>>` with stable `ParamId` | training, checkpoint restore, selective import | stable parameter IDs plus trainable/frozen posture | `psionic-models` plus `psionic-train` |
| `Module::visit` / `Module::map` | optimizer application, save/load, inspection | reusable parameter traversal or apply/visit surface | `psionic-models` plus `psionic-train` |
| autodiff backend plus op-extension checklist | AttnRes training needs gradients through softmax/RMSNorm/attention path | op admission rule including reverse mode | `psionic-ir` plus backend/runtime crates |
| recorder/store plus adapters | AttnRes has save/load today and may need migration from Burn weights | Psionic-native bundles plus optional Burn importer | `psionic-models` and optional research/import bin |
| renderer-neutral metric events | TUI and future WGPUI need live metrics without duplicating model code | diagnostics snapshot or renderer-event contract | `psionic-train`, `psionic-runtime`, app-owned pane |

## Recommended Prerequisite Program

### P0: Parameter Identity And Traversal

Goal:

- create the minimum reusable bridge between model families and training or IO
  surfaces

Required capabilities:

- stable parameter ID
- stable parameter path or manifest path
- trainable versus frozen classification
- enumerate parameters for optimizer updates
- enumerate parameters for weight import and export

What is enough for AttnRes:

- a lightweight parameter-tree surface is enough
- Psionic does not need to recreate Burn's derive macros first
- lazy parameter initialization is optional and can wait

Why this comes first:

- without it, optimizer state, checkpoint restore, and import are all forced
  into positional or AttnRes-only hacks

### P1: AttnRes Op Admission Rule

Goal:

- close the semantic gap with a repeatable standard instead of one-off fixes

Required rule for each AttnRes-enabling op:

- graph node shape and dtype rules are explicit
- CPU reference execution exists
- reverse-mode support exists, or refusal is explicit and the op is kept off the
  trainable path
- unit and parity tests exist

This should cover at least:

- subtract
- divide
- mean reduction
- max or log-sum-exp support
- exp
- sqrt
- power
- squeeze and unsqueeze
- broadcast or expand
- gather or embedding lookup
- softmax
- GELU
- cross-entropy or equivalent loss support

Optional optimization later:

- once the primitive path is honest, fused backend-extension versions can return

### P2: Native Checkpoint Layout Plus Burn Import Bridge

Goal:

- keep Psionic-native artifacts canonical while still allowing migration

Canonical saved form:

- `safetensors` weights
- Psionic JSON manifest
- config digest or descriptor digest
- optional optimizer-state artifact keyed by stable parameter identity

Optional migration bridge:

- a dedicated Burn importer binary or research tool
- path remapping and partial-load support modeled after Burn's store adapters
- convert once, then run natively in Psionic

What should not happen:

- `.mpk` becoming the Psionic checkpoint contract
- Burn participating in normal inference or training after import

### P3: Optimizer Application Bound To Model Identity

Goal:

- connect `psionic-train` optimizer math to real model families

Current useful Psionic fact:

- `psionic-train` already has reusable optimizer update math

Current missing bridge:

- optimizer state is not yet tied to model-owned parameter identity in the way
  Burn's `GradientsParams` and optimizer adaptor expect

Required prerequisite:

- a model-to-optimizer adapter that:
  - collects parameter buffers in a stable order
  - maps gradients back by parameter identity
  - stores optimizer state per parameter identity
  - reapplies updates without leaking training-only logic into the app

### P4: Renderer-Neutral Diagnostics Contract

Goal:

- make the future WGPUI port honest and ownership-correct

Required output from runtime or training surfaces:

- progress snapshots
- loss history
- query-norm history
- routing snapshots
- event feed or lifecycle feed
- two-phase parity status

Required rule:

- the app consumes immutable snapshots or events
- `crates/wgpui` remains product-agnostic
- AttnRes math remains in Psionic, not in the pane

This is the Burn dashboard lesson translated correctly into OpenAgents
ownership boundaries.

## Burn Ideas That Are Not Prerequisites

These should stay behind unless a later general Psionic need appears.

- `#[derive(Module)]` syntax itself
- full Burn recorder format compatibility as a first-class storage contract
- ratatui-specific widget layout and alternate-screen handling
- Burn's exact `Learner` API shape
- backend decorators like fusion or router as a prerequisite for AttnRes

Useful rule:

- port semantics, not Burn ergonomics

## How This Changes The Main AttnRes Roadmap

The companion AttnRes roadmap should now be read with these explicit additions:

### Phase 1 changes

- add parameter identity and traversal to the prerequisite list
- use an op-admission checklist that includes autodiff, not just forward
  execution

### Phase 4 changes

- make optimizer state per-parameter, not per-position
- keep Burn import optional and quarantine it behind a migration boundary

### Phase 5 changes

- drive the WGPUI pane from snapshots or metric events
- avoid a model-aware UI implementation that bypasses Psionic runtime truth

## Bottom Line

The most useful Burn port ideas are architectural contracts:

- parameter identity
- parameter traversal
- forward-plus-backward op completeness
- native-first storage with import adapters
- renderer-neutral diagnostics

Those are the real prerequisites for bringing `attnres` into Psionic honestly.

What should not be ported is the Burn-shaped surface syntax around them.
