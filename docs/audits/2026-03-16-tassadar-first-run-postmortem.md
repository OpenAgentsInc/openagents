# 2026-03-16 Tassadar First-Run Postmortem

This document is the human-readable review for the first persisted
`Tassadar` Sudoku-v0 trained-executor run at:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_reference_run_v0`

The machine-readable companion artifacts are:

- `postmortem.json`
- `next_run_plan.json`
- `training_telemetry.json`
- `exactness_curve.json`
- `trace_divergence_report.json`
- `failure_samples.json`

## Bottom Line

The first run is real and reproducible, but it is not close to working.

The dominant failure is immediate prompt-to-trace collapse:

- all 8 analyzed cases diverge at target token 0
- validation exact-trace is still `0/2`
- case exactness is only `9` to `16` bps

That means the next run should not chase hull-cache, 9x9 Sudoku, or larger
claims yet. The correct next step is to fix the boundary transition and make
the model learn the first few trace tokens reliably.

## Findings

### 1. Prompt-to-trace transition is completely broken

`trace_divergence_report.json` shows that every analyzed case diverges at the
first predicted target token.

This is the clearest signal in the whole run. The model is not "almost there"
and then falling apart later in the trace. It is failing before the trace
really starts.

### 2. The first run barely trained

`training_telemetry.json` shows:

- `1` training step
- `1` epoch
- `1024` supervised target tokens
- epoch mean loss `5720` milli-loss

That is enough to prove the Psionic-only training lane works. It is not enough
to support any strong model-quality claim.

### 3. The trace regime is long and needs curriculum

`trace_divergence_report.json` and `exactness_curve.json` show the real target
lengths are large:

- minimum target length: `114,913`
- maximum target length: `205,350`

Even after the boundary failure is fixed, this workload needs staged short-run
curriculum rather than one flat regime.

## Next Run

The machine-readable `next_run_plan.json` is the source of truth. The plan is:

### 1. Boundary curriculum first

- add explicit first-target-token evaluation
- train on very short suffix slices first: `1`, `8`, `32`, `128`
- oversample the prompt-to-`<step>` transition

Target:

- validation first-target exactness reaches `100%`
- validation first-32-token exactness exceeds `8000` bps

### 2. Increase training budget

- move from `1` epoch to at least `8`
- validate every epoch
- checkpoint every epoch

Target:

- materially exceed the current `13` bps aggregate validation baseline
- show monotonic progress on boundary metrics

### 3. Expand the trainable surface only if needed

- if the longer curriculum run still fails at the boundary, unfreeze token
  embeddings
- if that still fails, add one small trainable mixer above the deterministic
  lookup state

Target:

- at least one validation case becomes fully exact
- first-32-token exactness exceeds `9000` bps

### 4. Gate later phases

Do not advance:

- Phase 10 neural hull-cache decode
- Phase 11 9x9 Sudoku-class scale-out

until the 4x4 lane clears the boundary and short-trace exactness gates above.

## Conclusion

The important result is not that the first run is good. It is that the first
run is now honest, reproducible, diagnosable, and capable of driving a real
next-run plan.
