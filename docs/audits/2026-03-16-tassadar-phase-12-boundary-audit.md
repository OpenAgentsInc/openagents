# 2026-03-16 Tassadar Phase 12 Boundary Audit

This note audits the first post-reality-audit follow-on run for the learned
4x4 Sudoku-v0 lane:

- baseline run:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_reference_run_v0`
- Phase 12 follow-on run:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_boundary_v1`

The purpose of Phase 12 was narrow and specific:

> stop failing at target token `0`, make that boundary measurable, and persist
> enough evidence to say plainly whether the prompt-to-trace boundary was
> cleared.

## Verdict

Phase 12 clears the token-0 boundary, but it does not yet produce an exact
learned executor trace.

The strongest artifact-backed claim now supported is:

- the selected Phase 12 checkpoint gets the first validation target token right
  on both validation cases
- first divergence moved from target index `0` to target index `1`
- no validation case is yet exact over the full suffix
- the lane is still below the Phase 14 promotion bar

That is real progress. It is not article parity.

## Baseline vs Phase 12

### Baseline reference run

From the committed baseline artifacts:

- `training_report.json`
  - aggregate validation target exactness: `15` bps
  - exact validation traces: `0/2`
  - final-output exact validation cases: `0/2`
  - halt-exact validation cases: `0/2`
- `trace_divergence_report.json`
  - all `8` analyzed cases first diverge at target index `0`
  - reference first token: `<step>`
  - predicted first token: `<byte_50>`

This is the honest "real but failing" baseline.

### Phase 12 boundary run

From the committed Phase 12 artifacts:

- `training_report.json`
  - selected checkpoint:
    `tassadar-executor-transformer-sudoku-v0-boundary-v1.checkpoint.epoch_0005`
  - checkpoint selection basis:
    `boundary_metrics_lexicographic_v1`
  - selected stage: `prompt_to_first_32_tokens`
  - aggregate validation target exactness: `7431` bps
  - validation first-target exactness: `10000` bps
  - validation first-8 exactness: `2500` bps
  - validation first-32 exactness: `5000` bps
  - exact validation traces: `0/2`
  - final-output exact validation cases: `0/2`
  - halt-exact validation cases: `0/2`
- `boundary_exactness_report.json`
  - both validation cases hit `10000` bps first-target exactness
  - both validation cases remain non-exact over the full suffix
- `divergence_histogram.json`
  - first divergence bucket is now target index `1` for both validation cases
- `first_token_confusion_report.json`
  - token-zero divergence case count: `0`
- per-case validation evidence in `training_report.json`
  - `matched_target_token_count = 1` on both validation cases
  - reference token at first divergence: `<step_index>`
  - predicted token at first divergence: `<step>`

So the Phase 12 run does the thing the baseline could not do:

- it crosses the prompt -> first-trace-token boundary
- it does not yet stay aligned once the trace needs the next structural token

## What Phase 12 Added

The landed code changes are not just a stronger run config. They make the run
truthful in a way the baseline was not.

### New eval surfaces

`psionic-eval` now emits:

- `first_target_exactness_bps`
- `first_8_token_exactness_bps`
- `first_32_token_exactness_bps`
- per-case matched-token counts
- per-case first-divergence index
- per-case reference/predicted divergence tokens
- standalone boundary exactness, divergence histogram, and first-token
  confusion reports

Without those reports, this run would still have looked like one coarse
aggregate exactness jump.

### New train surfaces

`psionic-train` now supports:

- explicit boundary curriculum stages:
  - `prompt_to_first_token`
  - `prompt_to_first_2_tokens`
  - `prompt_to_first_4_tokens`
  - `prompt_to_first_8_tokens`
  - `prompt_to_first_16_tokens`
  - `prompt_to_first_32_tokens`
  - `full_trace_supervision`
- per-epoch validation
- checkpoint leaderboard export
- explicit best-checkpoint selection by boundary metrics rather than by loss
  alone

That selection logic mattered immediately.

## Important Phase 12 Finding: Full-Trace Supervision Regressed The Boundary

The epoch-level reports show:

- checkpoint `epoch_0005` at `prompt_to_first_32_tokens`
  - first-target exactness: `10000` bps
  - first-32 exactness: `5000` bps
  - aggregate exactness: `7431` bps
- checkpoint `epoch_0006` at `full_trace_supervision`
  - first-target exactness regressed to `0`
  - first-32 exactness rose to `6250` bps
  - aggregate exactness rose to `7640` bps

If export had been driven only by aggregate exactness, the run would have
selected a checkpoint that reintroduced the exact failure mode this phase was
supposed to remove.

So one of the most important outcomes of Phase 12 is not just "boundary got
better." It is:

- boundary-aware checkpoint selection is necessary
- full-trace training can currently destroy the boundary even while improving
  later-token aggregate scores

That is exactly why the issue required a checkpoint leaderboard and explicit
selection basis.

## What This Does Prove

Phase 12 now proves, with committed artifacts, that:

- the learned 4x4 lane is trainable enough to cross the first token boundary
- the original token-0 failure was not a permanent property of the model family
- curriculum plus budget plus boundary-aware export materially changes the
  result
- the weak baseline has been preserved rather than overwritten

## What This Still Does Not Prove

Phase 12 does not justify any of the following claims:

- "the model solves 4x4 Sudoku exactly"
- "the learned lane is now an exact executor"
- "the repo has article parity"
- "9x9 is now ready"
- "Hungarian is now in scope"

The current evidence still says:

- exact validation traces: `0/2`
- validation first-32 exactness: `5000` bps, not the `>9000` bps promotion bar
- no exact final outputs
- no exact halt markers

## What Should Happen Next

The next honest move is still to widen the trainable surface in a controlled
ablation program.

Why:

- Phase 12 fixed token `0`
- Phase 12 did not produce exact traces
- full-trace supervision currently regresses the boundary

That points directly at the next issue:

- `#3813` widen the trainable surface beyond the output head

The bar for moving past the 4x4 learned lane remains unchanged:

- first-target exactness = `100%`
- first-32 exactness > `9000` bps
- at least `1` exact validation trace

Phase 12 reaches only the first of those three.
