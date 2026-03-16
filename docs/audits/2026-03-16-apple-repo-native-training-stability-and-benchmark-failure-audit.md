# Apple Repo-Native Training Stability And Benchmark Failure Audit

Date: 2026-03-16

## Scope

This audit covers the March 16 repo-native Apple adapter training rerun work
that followed repeated local terminal/session crashes and repeated failed
`psionic-architecture-explainer-first-real-run` outcomes.

It covers:

- the stability fixes landed in `149e0dcc5`
- the guarded benchmark-runner follow-on landed in `80d17bd72`
- three local guarded operator runs executed against the live Apple Foundation
  Models bridge
- why those runs no longer crashed the operator session but still failed the
  benchmark completely
- the next steps required to move from "stable/exportable" to "actually
  benchmark-improving"

It does not claim that repo-native Apple training now works end to end. It
does not. The current truthful state is:

- repo-native Apple training is now materially more stable to run locally
- repo-native Apple export still produces runtime-loadable `.fmadapter`
  packages
- runtime smoke still passes
- benchmark quality is still completely broken

## Executive Summary

The immediate crash problem was real and partially fixed.

The most important stability issue inside the current repo-native Apple SFT
lane was that `psionic-train` kept one full explicit gradient tensor set per
completed step in memory via retained `gradient_records`. For longer runs, that
made memory growth scale with `steps x adapter_state`, even though the
gradients were no longer needed once the optimizer step had been applied.

That retention bug is now fixed in
`crates/psionic/psionic-train/src/apple_adapter.rs`. The operator-facing run
record now redacts the full gradient tensors after each step and keeps only the
lightweight per-step summary. Redundant in-memory initial/final safetensor byte
buffers were also dropped from the SFT execution artifacts.

The second concrete bug was operator misconfiguration: the live Apple operator
was silently inflating manifest-backed runs to `128` steps even when the frozen
experiment manifest explicitly requested `8`. That bug is now fixed in
`apps/autopilot-desktop/src/apple_adapter_training_control.rs`. Manifest-backed
runs now respect the manifest step budget.

Those two fixes materially improved operability:

- detached guarded runs completed without taking down the terminal session
- export and runtime smoke still worked
- the runner now has a benchmark-only mode so an existing `.fmadapter` package
  can be evaluated without retraining first

However, the quality result is still flatly failing:

- the `128`-step guarded run scored `0`
- the corrected `8`-step guarded run scored `0`
- an overfit probe that used `benchmark.jsonl` as both train and held-out still
  scored `0`

That means the remaining blocker is not "we need a safer shell loop" and it is
not just "we overtrained the tiny dataset." The current repo-native Apple
training formulation is not learning a signal that moves the live Apple
benchmark.

## Landed Changes

### 1. Bounded gradient retention in the repo-native Apple SFT lane

Landed in `149e0dcc5`.

Changed:

- `crates/psionic/psionic-train/src/apple_adapter.rs`
- `crates/psionic/docs/TRAIN_SYSTEM.md`

What changed:

- `AppleAdapterGradientBatchRecord` now exposes
  `redacted_for_retention()`
- after each optimizer step, the retained run record drops
  `training_batch.gradients`
- redundant in-memory `initial_bundle_bytes` and `final_bundle_bytes` were
  removed from retained SFT execution artifacts

Why this mattered:

- the old path kept one full explicit gradient payload per step for the whole
  run
- that is exactly the wrong retention shape for a local operator path
- the operator only needs the per-step summary after the step is applied

### 2. Manifest step budgets now apply to live Apple operator runs

Also landed in `149e0dcc5`.

Changed:

- `apps/autopilot-desktop/src/apple_adapter_training_control.rs`

What changed:

- manifest-backed runs no longer force `max_steps >= 128`
- the experiment manifest path is carried into the operator launch request
- a regression test now proves that a manifest with `max_steps = 8` produces an
  execution config with `budget.max_steps = 8`

Why this mattered:

- the frozen architecture-explainer experiment manifest asks for `8` steps, not
  `128`
- forcing `128` on a tiny dataset made the run much slower and very likely made
  already-bad training behavior worse

### 3. Benchmark-only runner path and guarded operator tooling

Landed in `80d17bd72`.

Changed:

- `apps/autopilot-desktop/src/apple_architecture_explainer_reference_run.rs`
- `apps/autopilot-desktop/src/bin/apple_architecture_explainer_reference_run.rs`
- `apps/autopilot-desktop/src/apple_repo_lookup_tools.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`

What changed:

- the architecture-explainer runner can now benchmark an already-exported
  `.fmadapter` package without retraining first
- the runner now propagates `experiment_manifest_path` into live operator runs
- repo lookup error surfaces were tightened so tool-call benchmark failures are
  easier to inspect

Why this mattered:

- it made repeated parity probes much cheaper
- it separated "can this package load and benchmark?" from "do we need to
  retrain first?"

## Local Evidence

All three guarded runs were executed against the live Foundation Models bridge
on this Mac. The local reports live under:

- `~/.openagents/logs/codex/apple-guarded-run-20260316T110441/report.json`
- `~/.openagents/logs/codex/apple-guarded-run-20260316T111113/report.json`
- `~/.openagents/logs/codex/apple-benchmark-overfit-20260316T111206/report.json`

The operator-owned step artifacts live under:

- `~/.openagents/logs/autopilot/apple-adapter-training/.../psionic/`

### Run Matrix

| Run | Run ID | Train dataset | Steps | Training wall clock | Gradient-record artifact | Runtime smoke | Benchmark result |
| --- | --- | --- | ---: | ---: | ---: | --- | --- |
| Guarded legacy-budget rerun | `psionic-architecture-explainer-first-real-run-1773677082122` | `train.jsonl` | 128 | 36012 ms | 327160 bytes | passed | rejected, adapted score 0 |
| Guarded manifest-budget rerun | `psionic-architecture-explainer-first-real-run-1773677473552` | `train.jsonl` | 8 | 5933 ms | 20490 bytes | passed | rejected, adapted score 0 |
| Guarded benchmark-overfit probe | `psionic-architecture-explainer-first-real-run-1773677526468` | `benchmark.jsonl` | 8 | 5270 ms | 20447 bytes | passed | rejected, adapted score 0 |

### What materially improved

The stability changes are real, not theoretical:

- the `128`-step run completed training, eval, export, runtime smoke, and
  benchmark reporting without killing the terminal session
- the `8`-step rerun completed the same path in about `5.9s`
- the benchmark-overfit probe also completed the full path in about `5.3s`

The gradient artifact size difference is the clearest repo-owned evidence:

- `128`-step run: `gradient-records.json` was `327160` bytes
- `8`-step run: `gradient-records.json` was `20490` bytes
- `8`-step benchmark-overfit run: `gradient-records.json` was `20447` bytes

That confirms the retained step history is now bounded by lightweight summary
content instead of full gradient tensors.

The runtime/export path also remained real:

- every run exported a `133312320` byte `adapter_weights.bin`
- every run produced a `~15.8 MB` Psionic checkpoint
- runtime smoke passed every time

So the path is now:

- stable enough to run repeatedly
- honest enough to export and reload
- still not useful enough to pass the benchmark

## What Failed

Every run still failed in the same essential way:

- held-out pass rate: `0`
- held-out average score: `0`
- benchmark adapted score: `0`
- benchmark adapted pass rate: `0`
- improved case count: `0`

The benchmark gate rejected every run for the same reasons:

- `adapter_score_below_minimum`
- `adapter_pass_rate_below_minimum`
- `score_delta_below_minimum`
- `pass_rate_delta_below_minimum`
- `improved_case_count_below_minimum`

The critical point is that even the benchmark-overfit probe stayed flat-zero.
That probe deliberately used `benchmark.jsonl` as both the train split and the
held-out split. If the current repo-native Apple lane could even memorize the
benchmark behavior, that run should have moved something. It did not.

## Why It Is Still Failing

There is not one single cause. There is a stack of misalignments.

### 1. The current repo-native backend is explicitly not Apple-native hidden-state training

The current backend says so in its own fidelity plan inside
`crates/psionic/psionic-train/src/apple_adapter.rs`.

Its bounded components include:

- `repo_owned_lexical_tokenizer_not_apple_exact`
- `hashed_token_embeddings_not_native_hidden_states`
- `pooled_sequence_regression_not_full_decoder_loss`
- `single_host_f32_reference_execution`

That is a truthful description of the current backend, and it is also the core
reason the benchmark is not moving.

The current repo-native Apple training path is not training on actual Apple
runtime hidden states or a true decoder-token loss. It is training on a hashed,
pooled, repo-owned surrogate feature representation. That surrogate may be good
enough to prove that the fixed-budget trainer, export path, and operator
receipts are wired together. It is not currently good enough to move the live
Apple benchmark.

### 2. The live exportable target family is narrower than the experiment manifest

The architecture-explainer manifest asks for:

- `decoder.attn.q_proj`
- `decoder.ffn.up_proj`

The current live exportable operator lane only expands the attention-side
`q_proj` symbolic family. It explicitly skips `decoder.ffn.up_proj`.

So the current live operator is already solving a weaker problem than the
frozen experiment manifest describes. That reduces capacity and makes the run
less likely to learn the intended behavior.

### 3. The live exportable geometry is still pinned, not manifest-shaped

The current operator logs and code make this explicit:

- the manifest requests `input_width = 48`, `output_width = 24`, `lora_rank = 4`
- the live runtime-export lane remains pinned to the current export-compatible
  reference geometry instead

That means the real run is not actually traversing the geometry the experiment
manifest describes. Again, that may be acceptable for a temporary reference
lane, but it is not good enough if the goal is "make this benchmark pass."

### 4. The manifest-backed optimizer posture is too blunt

`build_psionic_execution_config(...)` currently uses a more aggressive manifest
path:

- manifest-backed runs use `AdamW` with learning rate `0.05`
- non-manifest runs use learning rate `0.01`

That is an extremely blunt rule for a tiny dataset and a surrogate feature
path. The corrected `8`-step runs still showed very large losses:

- guarded `8`-step run average loss: `28326.0536`
- `8`-step benchmark-overfit run average loss: `37580.6260`

Even without claiming too much from raw loss values, this is not evidence of a
healthy, convergent fit.

### 5. Some benchmark failures are real model-quality failures, but some are also runtime/harness failures

Two benchmark samples are especially diagnostic.

#### Structured-output case failure

`sample-000003` failed in every run with:

- `runtime_failure:bridge_request_failed`
- `Apple FM respond_structured_in_session returned typed Foundation Models error: Failed to deserialize a Generable type from model output`

That means the adapted runtime path is not reliably producing a structured
output that the live bridge can deserialize into the requested schema.

#### Tool-calling case failure

`sample-000004` failed in every run with a mixed model/harness breakdown:

- `runtime_failure:bridge_request_failed`
- `model_behavior:tool_failed:lookup_doc:path_not_file`
- `model_behavior:tool_missing:lookup_code`

The recorded model-request failures show that the model attempted invalid tool
paths such as:

- `crates/psionic/*`
- `apps/autopilot-desktop`

The first is a wildcard path the repo lookup harness does not resolve. The
second is a directory path, not a file path.

This matters for interpretation:

- part of the failure is that the model is not learning the exact required tool
  usage
- part of the failure is that the benchmark/harness expects valid repo-relative
  file paths and currently treats globs/directories as hard failures

Even so, this does not rescue the current backend. The benchmark-overfit probe
still failed completely, which means the current lane still cannot memorize
those exact expectations well enough.

### 6. Runtime smoke passing does not mean the adapter is good

Every run passed runtime smoke because the smoke prompt is intentionally narrow:

- "Explain what a mutex does in one short sentence."

That proves:

- the package loads
- the adapter can be attached
- the runtime can answer a simple text prompt

It does not prove:

- architecture-explainer truthfulness
- structured-output conformance
- tool-calling correctness
- benchmark improvement

That distinction is now obvious in the data: smoke passed every time while the
benchmark remained flat-zero every time.

## The Most Important Conclusion

The repo-native Apple lane is no longer blocked first by local operator
stability.

It is now blocked first by training quality and backend fidelity.

That is a better place to be because it is honest:

- the operator can run
- the export path can run
- the runtime load path can run
- the benchmark still says the trained adapter is useless for the target task

## Recommended Next Steps

These next steps are ordered by leverage, not by convenience.

### 1. Stop treating this as a hyperparameter-only problem

The benchmark-overfit probe staying flat-zero is the key evidence.

Do not spend the next iteration only sweeping:

- step count
- learning rate
- batch size

Those knobs matter, but they are not the primary blocker anymore.

### 2. Make the backend either honor the experiment contract or narrow the contract truthfully

Right now the live lane says:

- it is running the architecture-explainer experiment

But in reality it:

- drops `decoder.ffn.up_proj`
- ignores the manifest geometry
- uses a surrogate feature/training objective

The next implementation step must make one of these true:

1. the live backend really supports the manifest target family and geometry, or
2. the manifest and benchmark program are rewritten to match the actual current
   live lane

Until that mismatch is removed, the benchmark program is measuring one thing
while the backend is training another.

### 3. Replace or substantially strengthen the current surrogate training objective

This is the core repo-native training task.

The current hashed pooled feature path was enough to prove control-plane and
export wiring, but it is not enough to produce useful adapters.

The next real quality milestone should be:

- a repo-native Apple training objective that can overfit the benchmark corpus
  in a controlled local test

If the lane cannot overfit the benchmark corpus, it is not ready for broader
claims.

Possible acceptable directions:

- a more Apple-aligned token-level or sequence-level objective inside Psionic
- a stronger teacher-target/distillation formulation that uses live runtime
  outputs only as an oracle, not as the shipped training backend
- a richer representation than the current hashed lexical pooling path

Unacceptable direction:

- putting Python/toolkit back into the shipped authoritative path

If an external oracle is used at all, it should remain a development parity
tool, not the live production training lane.

### 4. Make manifest-backed optimizer settings explicit and tuneable

Do not keep a hardcoded "manifest means `lr = 0.05`" rule.

The operator needs explicit knobs for:

- learning rate
- max steps
- target family
- rank
- geometry

and those knobs need to be captured in the operator report and benchmark
report.

### 5. Add direct benchmark raw-output capture to the report

The current reports already capture rich failure metadata, but the next debug
loop needs the actual adapted outputs for each benchmark case attached directly
to the operator report.

Without that, every failure triage requires jumping through the eval-state
artifacts instead of reading one run report.

### 6. Decide how strict the tool-calling benchmark should be about invalid repo paths

`sample-000004` is currently useful, but it mixes two different questions:

- did the model choose the right tool?
- did it choose a syntactically valid repo-relative file path?

That is a legitimate test, but if the goal is to isolate training quality, the
benchmark family should have:

- one case for tool selection
- one case for exact path correctness

right now the failure mode is too blended.

### 7. Keep the detached guarded operator flow for all future Apple experiments

The local operating discipline that worked here should remain the default for
future Apple runs:

- build first
- launch detached
- poll short-lived status commands
- keep reports under `~/.openagents/logs/codex/...`
- keep operator receipts under `~/.openagents/logs/autopilot/...`

Do not go back to long blocking foreground shell commands for Apple training
experiments.

## Bottom Line

The work on March 16 did produce something real:

- a materially safer repo-native Apple operator run path
- repeated successful repo-native export and runtime-smoke passes
- a clearer measurement that the current backend still does not learn a useful
  Apple adapter for the architecture-explainer benchmark

That is progress, but it is not success.

The next honest milestone is not "run it again." The next honest milestone is:

- make the repo-native Apple training formulation strong enough to overfit the
  benchmark corpus in one controlled local run

Until that happens, the Apple lane should be described as:

- stable enough to run
- real enough to export
- not yet benchmark-effective
