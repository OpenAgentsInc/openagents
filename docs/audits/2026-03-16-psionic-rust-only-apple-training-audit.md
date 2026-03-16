# 2026-03-16 Psionic Rust-Only Apple Training Audit

## Scope

This audit answers four specific questions:

1. Why the Apple adapter lane originally still invoked Python.
2. What changed once the Rust-native training and export issues landed.
3. Why the operator experience originally went silent for long stretches.
4. What happened once telemetry, the Rust-only gate, and the final cleanup
   issue landed.

This is a codebase-grounded audit of the current tree, not a wish list.

## Executive Summary

Update after `#3768` through `#3775` on 2026-03-16:

- the shipped Apple operator path no longer uses Python for the train phase or
  the export phase
- `apps/autopilot-desktop/src/apple_adapter_training_control.rs` now launches
  the repo-owned `AppleAdapterTrainingExecutionBackend` through
  `run_apple_adapter_sft_export(...)`
- a real operator run,
  `rust-native-3768-validation-1773641773075`, completed the Rust train step,
  wrote repo-owned checkpoints, and staged a `.fmadapter`
- the follow-on live operator run,
  `rust-native-3769-validation-1773643126445`, completed held-out eval and
  runtime smoke with `runtime_smoke_passed=true`, `runtime_asset_size_bytes`
  `133312320`, and the live Apple base-model signature
  `9799725ff8e851184037110b422d891ad3b92ec1`
- the Apple runtime parity follow-on now uses shared repo-backed `lookup_doc`
  and `lookup_code` tool handlers in
  `apps/autopilot-desktop/src/apple_repo_lookup_tools.rs`, normalizes guided
  generation schemas through
  `psionic_apple_fm::AppleFmGenerationSchema::with_title_hint(...)`, caps live
  eval requests with bounded greedy generation options, and emits full
  benchmark `base_eval_run` and `adapted_eval_run` receipts so sample failures
  are classified as model, model-request, or runtime failures instead of being
  hidden behind aggregate benchmark deltas

The shipped train/export lane is therefore Rust-native end to end.
The legacy toolkit wrapper source now remains only as a quarantined,
non-default compatibility oracle in
`crates/psionic/psionic-train/src/apple_toolkit.rs`; default `psionic-train`
builds do not compile it, and the desktop operator path cannot reach it.

So the honest current answer is:

> the shipped Apple operator lane is now Rust-only, with a release gate that
> refuses toolkit/Python regressions, while the old toolkit oracle remains
> feature-gated and developer-only instead of part of the live contract.

That specific operator-blindness problem is no longer the shipped behavior
after `#3771` and `#3772`:

- the app-owned operator path now emits typed live phase, heartbeat, ETA, and
  recent-event telemetry
- the legacy toolkit wrapper now streams stdout/stderr lines instead of
  waiting on `Command::output()`
- the remaining Python concern is boundary honesty and cleanup, not silent
  thirty-minute waits in the shipped operator lane

## Current Code Reality

### 1. The legacy toolkit wrapper is no longer part of the shipped build

The legacy compatibility source is:

- `crates/psionic/psionic-train/src/apple_toolkit.rs`

That file still contains:

- `AppleAdapterToolkitInstallation`
- `discover()` of `adapter_training_toolkit_v26_0_0`
- `discover_python_path()`
- `run_apple_adapter_toolkit_training(...)`
- `run_apple_adapter_toolkit_export(...)`
- `run_toolkit_command(...)`

Its export command is built as:

- `python -m export.export_fmadapter ...`

But after `#3775`, that code is no longer compiled by default. The retained
crate surface is:

- `crates/psionic/psionic-train/Cargo.toml` defines the non-default
  `legacy-apple-toolkit-oracle` feature
- `crates/psionic/psionic-train/src/lib.rs` only includes and re-exports the
  toolkit module when that feature is enabled

So the source remains in-tree as a developer-only oracle, not as shipped
runtime behavior.

### 2. The app-owned operator flow now uses Rust for both training and export

The relevant orchestrator is:

- `apps/autopilot-desktop/src/apple_adapter_training_control.rs`

The current launch flow now does this:

1. builds a repo-owned `AppleAdapterExecutionConfig`
2. launches `AppleAdapterTrainingExecutionBackend`
3. runs `run_apple_adapter_sft_export(...)`
4. writes repo-owned checkpoints, summaries, receipts, and gradient records
5. stages the resulting `.fmadapter`
6. runs held-out eval and bridge-backed runtime acceptance

So the heavy lifting for both train and export is now Rust in the shipped path,
and the later telemetry, gate, and cleanup issues are also landed.

### 3. Psionic now has authoritative Rust Apple training and export paths

The Rust-side reference surfaces exist in:

- `crates/psionic/psionic-train/src/apple_adapter.rs`
- `crates/psionic/psionic-train/src/apple_adapter_experiment.rs`

Those files are real work, not placeholders. They define:

- repo-owned Apple dataset batching
- gradient-production records
- fixed-budget trainer inputs
- optimizer integration
- reference export metadata
- draft-model-related reference paths

After `#3768`, they are now the authoritative live training path for the
desktop operator lane.

The train system doc now says this precisely:

- `crates/psionic/docs/TRAIN_SYSTEM.md`

The repo-native backend is currently a narrower reference backend, but it is
now also the fully gated and cleanup-complete shipped backend for this Apple
reference lane.

### 4. The operator telemetry is now live and typed

The operator status types in
`apps/autopilot-desktop/src/apple_adapter_training_control.rs` do have:

- stage state
- `last_action`
- `last_error`
- `log_lines`
- post-run local summary fields
- progress snapshots
- typed progress events

The current shipped lane now surfaces:

- current epoch
- current batch or step
- current loss
- current learning rate
- current eval phase
- current export phase
- ETA
- checkpoint completion events

That telemetry contract is now backed by:

- the app-owned progress/event stream in
  `apps/autopilot-desktop/src/apple_adapter_training_control.rs`
- streamed toolkit compatibility output from `#3772` for non-default oracle
  runs
- the WGPUI run-detail work from `#3773`

## Why Python Is Still In Tree At All

The honest current reason is narrow:

- the old toolkit wrapper source remains as a quarantined developer-only oracle
  behind the non-default `legacy-apple-toolkit-oracle` feature

It is no longer part of:

- the shipped desktop operator path
- default `psionic-train` builds
- packaged release validation

`#3774` and `#3775` changed that boundary explicitly.

## Why The Current Experience Used To Be So Bad During Long Runs

The original operator-blindness problem had three parts:

### 1. Blocking subprocess collection

`run_toolkit_command(...)` used to wait for the child process to exit before
surfacing output.

That specific transport problem is now fixed, but it was the reason the older
lane did not emit:

- incremental stdout lines
- incremental stderr lines
- progress markers
- periodic loss values

### 2. No typed training-progress event stream

The status model originally had final summaries, but it did not have a durable
event-stream model for live work such as:

- `training_started`
- `epoch_started`
- `step_completed`
- `loss_observed`
- `checkpoint_written`
- `eval_started`
- `eval_sample_completed`
- `export_started`
- `export_completed`
- `runtime_smoke_started`
- `runtime_smoke_completed`

### 3. No app-owned ETA or heartbeat model

Even if the underlying backend takes a long time, the operator should still be
able to see:

- the worker is alive
- the current phase
- elapsed time
- recent output line
- recent step count
- estimated remaining time

That contract does not exist today for the Apple operator path.

## Additional Codebase Problems Exposed By The Recent Runs

These problems are separate from the Python dependency, but they matter for a
credible Rust-only replacement:

### 1. The tool-eval harness used to be too fake

Before `#3770`, the benchmark and held-out harnesses used recording tools in:

- `apps/autopilot-desktop/src/apple_adapter_training_control.rs`
- `apps/autopilot-desktop/src/apple_architecture_explainer_reference_run.rs`

Those tools used to record the call and echo synthetic JSON.
They did not actually read repo docs or code.

After `#3770`, both harnesses share real repo-backed lookup tools in
`apps/autopilot-desktop/src/apple_repo_lookup_tools.rs`. Benchmark and held-out
receipts now carry the lookup events plus model-request or harness failure
metadata when a tool call goes wrong.

### 2. Structured-generation parity used to be incomplete

Before `#3770`, the recent run still hit invalid structured-generation behavior
on the Apple runtime for the schema-backed sample.

The Rust-only path therefore needed:

- correct guided-generation schema normalization
- runtime-valid structured request construction
- parity coverage for the structured eval lane

After `#3770`, raw eval schemas are normalized through
`AppleFmGenerationSchema::with_title_hint(...)`, live eval requests use bounded
greedy generation options, and the structured sample now terminates as a normal
model mismatch instead of an invalid-schema runtime failure.

### 3. The current operator status surface reports the truth too late

The final summary can parse:

- average loss
- training wall-clock
- memory footprint
- checkpoint size
- export size

But those are all post-run facts.
The missing feature is live visibility while the work is happening.

## Required GitHub Issue Program

The issue program below is the work required to get to a Rust-only Psionic
Apple lane with live telemetry.

These are written as issue-ready titles with concrete deliverables and
acceptance targets.

### 1. Psionic Apple Train: replace `apple_toolkit.rs` training invocation with
an authoritative Rust-native Apple adapter SFT executor

Status:

- implemented in `#3768`

Why:

- the old authoritative training path shelled out to Python
- `AppleAdapterTrainingExecutionBackend` had to become the shipped live
  executor rather than a side reference backend

Deliverables:

- make the Rust executor the authoritative live Apple adapter SFT path
- remove `python -m examples.train_adapter` from the shipped operator flow
- define the exact Rust-side training object model for Apple adapter runs
- surface typed step-level progress from the executor

Acceptance:

- `apps/autopilot-desktop/src/apple_adapter_training_control.rs` no longer
  calls the Python training wrapper for the shipped path
- `psionic-train` can produce the live training artifacts directly
- the first reference run uses the Rust executor, not Python

### 2. Psionic Apple Export: implement a Rust-native Apple-valid runtime-asset
writer and delete Python export from the live path

Status:

- implemented in `#3769`

Why:

- the old authoritative export shelled out to
  `python -m export.export_fmadapter`
- Rust had to own the final Apple-valid adapter payload contract, including the
  Core ML blob-storage container shape and the correct live base-model
  signature

Deliverables:

- implement the Apple-valid runtime asset writer in Rust
- prove byte-level and runtime-level parity against the current accepted export
  path
- keep lineage and package metadata repo-owned
- remove the Python exporter from the shipped operator path

Acceptance:

- `run_apple_adapter_toolkit_export(...)` is no longer required for the live
  lane
- the Rust-exported `.fmadapter` loads through the live bridge
- runtime smoke passes on a Rust-exported package

### 3. Psionic Apple Runtime Parity: make structured-generation and tool-calling
evals fully valid against the Apple runtime

Status:

- implemented in `#3770`

Why:

- the earlier runs exposed invalid structured schema behavior
- tool-backed eval used echo tools instead of real retrieval

Deliverables:

- normalize guided-generation schema exactly as the Apple runtime expects
- add real `lookup_doc` and `lookup_code` tool handlers for the architecture
  explainer eval lane
- make held-out and benchmark tool cases use real repo retrieval instead of
  echo payloads

Acceptance:

- structured eval samples run without invalid-schema failures
- tool-eval samples use real lookup results
- held-out and benchmark receipts surface model, model-request, and runtime
  failure reasons instead of only aggregate deltas

### 4. Psionic Apple Operator Telemetry: add a typed live event stream for
training, eval, export, and smoke

Why:

- the current operator lane goes dark during long-running work
- the app only receives coarse stage transitions and post-run summaries

Deliverables:

- define typed live events for training, eval, export, and runtime smoke
- add heartbeats, elapsed time, and ETA
- expose current phase, epoch, step, sample index, and recent loss
- persist progress snapshots so the app can recover after restart

Acceptance:

- `autopilotctl training status` shows live progress during active work
- WGPUI panes can render streaming status instead of waiting for process exit
- restart-safe progress survives operator reconnect

### 5. Psionic Apple Operator Transport: replace buffered subprocess capture with
streaming process IO or streaming Rust-executor events

Why:

- `Command::output()` is the immediate reason the operator cannot show progress

Deliverables:

- stop buffering child output until process exit
- stream stdout and stderr incrementally into typed operator events
- timestamp and retain recent lines in a bounded log ring
- surface phase transitions immediately

Acceptance:

- long-running work produces live log lines in the operator surface
- loss lines and export milestones appear before the process exits
- no stage waits silently for tens of minutes without a heartbeat

### 6. Psionic Apple Operator UX: add first-class progress panes and run detail
surfaces for long training jobs

Why:

- even with backend telemetry, the app still needs a usable presentation layer

Deliverables:

- add a live run detail pane for Apple adapter training
- show current stage, elapsed time, ETA, loss trend, checkpoint milestones,
  export state, and runtime-smoke state
- surface recent log lines and failure causes inline

Acceptance:

- a user can tell what the run is doing without inspecting raw files
- the pane updates while the run is active
- failures show the exact stage and last backend message

### 7. Psionic Apple De-Pythonization Gate: add a release and acceptance gate
that fails if the shipped Apple lane still requires Python

Status:

- completed as `#3774`

Why:

- without a hard gate, the codebase can drift back into "Rust wrapper around
  Python" again

Deliverables:

- add a release check that asserts the shipped Apple lane does not require a
  toolkit checkout or Python interpreter
- fail if operator launch depends on `adapter_training_toolkit_v26_0_0`
- document the Rust-only requirement in the train system spec

Acceptance:

- release validation fails if Python is required for the shipped Apple path
- the docs stop describing Python as part of the live Apple lane

### 8. Psionic Apple Cleanup: remove the toolkit discovery and Python execution
surface after Rust parity lands

Status:

- completed as `#3775`

Why:

- once Rust parity exists, leaving the Python path around will keep the boundary
  ambiguous

Deliverables:

- delete toolkit discovery from `apple_toolkit.rs`
- delete Python interpreter discovery from the shipped path
- remove toolkit-only operator receipts from the live lane
- leave any remaining compatibility tools as explicitly archived or
  developer-only, not part of the shipped operator contract

Acceptance:

- the shipped Apple lane does not search for toolkit roots or Python
- the operator summary no longer reports a toolkit Python path as a required
  dependency

## Recommended Sequencing

The minimum sane order is:

1. issue 4: typed live telemetry contract
2. issue 5: streaming operator transport
3. issue 3: structured/tool runtime parity
4. issue 1: authoritative Rust-native training executor
5. issue 2: authoritative Rust-native Apple-valid exporter
6. issue 6: WGPUI run-detail UX
7. issue 7: Rust-only acceptance gate
8. issue 8: delete the Python path

The reason to do telemetry first is simple:

- we should not spend another 30 to 90 minutes per run blind
- live instrumentation will make the Rust replacement work far easier to debug

## Bottom Line

We are no longer using Python in the shipped Apple adapter lane.

The shipped state is now:

- Rust-native Psionic training and export
- typed live status updates during long runs
- a release gate that fails if toolkit/Python dependency creeps back in
- a quarantined legacy toolkit oracle that is no longer part of the default
  `psionic-train` build or the desktop operator path

The remaining work after this audit is no longer "make Apple training
Rust-only." That part is complete for the current narrow Apple reference lane.
The next work is higher-order quality and scale: model quality improvement,
broader distributed execution, and future productization beyond this reference
path.
