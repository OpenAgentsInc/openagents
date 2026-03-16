# 2026-03-16 Psionic Rust-Only Apple Training Audit

## Scope

This audit answers four specific questions:

1. Why the current Apple adapter training lane still invokes Python.
2. Why that means the lane is still not honestly Rust-only Psionic.
3. Why the current operator experience goes silent for long stretches instead of
   surfacing live progress.
4. What GitHub issue program is required to replace the remaining Python path
   with Rust-owned Psionic code and add real telemetry.

This is a codebase-grounded audit of the current tree, not a wish list.

## Executive Summary

Update after `#3768` on 2026-03-16:

- the shipped Apple operator path no longer uses Python for the train phase
- `apps/autopilot-desktop/src/apple_adapter_training_control.rs` now launches
  the repo-owned `AppleAdapterTrainingExecutionBackend` through
  `run_apple_adapter_sft_export(...)`
- a real operator run,
  `rust-native-3768-validation-1773641773075`, completed the Rust train step,
  wrote repo-owned checkpoints, and staged a `.fmadapter`
- that same run then failed at the live bridge adapter-load gate, so the lane
  is still not honestly Rust-only end to end

We are therefore still using Python only because the Apple-valid export parity
problem is not closed yet and the toolkit path remains the external oracle in
`crates/psionic/psionic-train/src/apple_toolkit.rs`.

That wrapper is not incidental:

- it discovers a toolkit checkout on disk
- it discovers a Python interpreter inside the toolkit virtualenv
- it launches `python -m export.export_fmadapter`
- the bridge-accepted runtime payload contract is still judged against that
  external path until the native exporter passes the live bridge gate

So the honest current answer is:

> the Apple operator lane is now Rust-executed for training, but it is still
> not honestly Rust-only because the native export/runtime path is not yet
> bridge-accepted without the toolkit oracle.

That is also why the operator UI and CLI feel blind during long runs:

- the toolkit wrapper uses `std::process::Command::output()`
- stdout and stderr are buffered until the child process exits
- the app only sees "training started" and then "training finished"
- progress, loss updates, checkpoint milestones, and export phases are not
  streamed

The current lane is therefore failing two separate goals:

1. It is not Rust-only.
2. It does not expose live step telemetry during long-running work.

## Current Code Reality

### 1. The live Apple export parity oracle is still the Python toolkit wrapper

The current authoritative wrapper is:

- `crates/psionic/psionic-train/src/apple_toolkit.rs`

That file currently owns:

- `AppleAdapterToolkitInstallation`
- `discover()` of `adapter_training_toolkit_v26_0_0`
- `discover_python_path()`
- `run_apple_adapter_toolkit_training(...)`
- `run_apple_adapter_toolkit_export(...)`
- `run_toolkit_command(...)`

The current export command is built as:

- `python -m export.export_fmadapter ...`

This is no longer the live train-phase codepath after `#3768`, but it remains
the external export oracle the native path still has to match.

### 2. The app-owned operator flow now uses Rust for training but still needs
the native exporter to reach bridge acceptance

The relevant orchestrator is:

- `apps/autopilot-desktop/src/apple_adapter_training_control.rs`

The current launch flow now does this:

1. builds a repo-owned `AppleAdapterExecutionConfig`
2. launches `AppleAdapterTrainingExecutionBackend`
3. runs `run_apple_adapter_sft_export(...)`
4. writes repo-owned checkpoints, summaries, receipts, and gradient records
5. stages the resulting `.fmadapter`
6. runs held-out eval and bridge-backed runtime acceptance

So the heavy lifting for the train phase is now Rust. The missing piece is that
the staged native export still failed to load through the live bridge in the
first validation run.

### 3. Psionic now has an authoritative Rust Apple training path, but export
parity is still not complete

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

But they are still not yet the same thing as:

- a Rust-native Apple-valid `.fmadapter` exporter that emits the exact runtime
  payload Apple accepts through the live bridge
- an honestly Rust-only end-to-end Apple lane

The train system doc already says this precisely:

- `crates/psionic/docs/TRAIN_SYSTEM.md`

The repo-native backend is currently a narrower reference backend.
The live Apple-valid runtime asset path is still the toolkit wrapper.

### 4. The operator telemetry is currently coarse and mostly post-hoc

The operator status types in
`apps/autopilot-desktop/src/apple_adapter_training_control.rs` do have:

- stage state
- `last_action`
- `last_error`
- `log_lines`
- post-run local summary fields

But the crucial implementation detail is the toolkit subprocess runner in
`crates/psionic/psionic-train/src/apple_toolkit.rs`:

- `run_toolkit_command(...)` uses `Command::output()`
- stdout is only available after exit
- stderr is only available after exit
- `/usr/bin/time -l` output is only parsed after exit

That means the operator cannot stream:

- current epoch
- current batch or step
- current loss
- current learning rate
- current eval phase
- current export phase
- ETA
- checkpoint completion events

The app logs reflect that limitation exactly:

- one log line before training starts
- one log line after training finishes
- one log line after export finishes

This is why the run feels like "nothing is happening for thirty minutes".
That is not just a UI complaint. It is a real missing contract in the operator
path.

## Why Python Is Still In The Loop

The honest reasons are:

### 1. Apple-valid export is still coupled to the external toolkit path

The current live lane still relies on the toolkit exporter because that is the
only proven Apple-valid runtime-asset path in the current repo workflow.

The Rust operator path stages repo-owned package metadata, but the actual
runtime asset bytes currently come from the toolkit export result.

### 2. The Rust reference backend is not yet Apple-runtime-parity training

The current Rust backend is useful and important, but it is still a narrower
reference backend. It is not yet the authoritative path for:

- the exact Apple adapter weight update contract
- the exact Apple-valid export payload contract
- the exact structured generation and tool-calling parity needed for the real
  runtime lane

### 3. The shipped operator path was optimized for "get one real Apple-valid
path working" before "delete all Python"

That tradeoff was pragmatic at the time, but it is now the wrong steady-state.

If the product claim is "Psionic owns the Apple training lane", then the
current Python dependency is technical debt, not an acceptable final boundary.

## Why The Current Experience Is So Bad During Long Runs

There are three separate problems:

### 1. Blocking subprocess collection

`run_toolkit_command(...)` waits for the child process to exit before surfacing
output.

So the operator does not receive:

- incremental stdout lines
- incremental stderr lines
- progress markers
- periodic loss values

### 2. No typed training-progress event stream

The status model has final summaries, but it does not have a durable
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

### 1. The tool-eval harness is still too fake

The benchmark and held-out harnesses currently use recording tools in:

- `apps/autopilot-desktop/src/apple_adapter_training_control.rs`
- `apps/autopilot-desktop/src/apple_architecture_explainer_reference_run.rs`

Those tools currently record the call and echo synthetic JSON.
They do not actually read repo docs or code.

That means the current `lookup_doc` or `lookup_code` style tasks are not backed
by real retrieval during eval.

### 2. Structured-generation parity is still incomplete

The recent run still hit invalid structured-generation behavior on the Apple
runtime for the schema-backed sample.

That means the Rust-only path cannot stop at "text-only adapter answers".
It also needs:

- correct guided-generation schema normalization
- runtime-valid structured request construction
- parity coverage for the structured eval lane

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

Why:

- the current authoritative export still shells out to
  `python -m export.export_fmadapter`
- Rust does not yet own the final Apple-valid adapter payload contract

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

Why:

- the current runs still expose invalid structured schema behavior
- tool-backed eval currently uses echo tools instead of real retrieval

Deliverables:

- normalize guided-generation schema exactly as the Apple runtime expects
- add real `lookup_doc` and `lookup_code` tool handlers for the architecture
  explainer eval lane
- make held-out and benchmark tool cases use real repo retrieval instead of
  echo payloads

Acceptance:

- structured eval samples run without invalid-schema failures
- tool-eval samples use real lookup results
- benchmark results are measuring the intended behavior, not a fake tool shim

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

We are still using Python because the shipped Apple adapter lane still depends
on the external Apple toolkit for the authoritative train and export steps.

We are still waiting in the dark because the operator path still buffers child
process output and only surfaces coarse stage changes and post-run summaries.

If the requirement is:

> Rust-only Psionic, no Python in the shipped Apple lane, with live status
> updates during long runs

then the correct next move is not another vague tuning pass.

The correct next move is to execute the issue program above and make the Rust
executor, Rust exporter, and typed live telemetry the authoritative path.
