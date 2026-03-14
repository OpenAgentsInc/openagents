# Eval Runtime Reference

> Status: canonical `#3568` eval-runtime record, updated 2026-03-14 after
> landing the runnable harness in
> `scripts/release/check-psionic-eval-runtime.sh`.

This document records the first Psionic-native eval runtime layer.

## What Landed

The issue landed the `psionic-eval` crate with:

- typed `EvalRunContract` and `EvalRunState` objects for held-out or
  benchmark-class local evaluation
- rubric-scored `EvalSampleRecord` construction directly from
  `psionic-environments` session summaries
- durable `EvalSummary` objects with aggregate metrics and aggregate artifacts
- explicit online/offline parity through one shared local sample/runtime path
- validator-style `BenchmarkPackage` contracts with repeat-run aggregation
- `BenchmarkExecutionSession` for both validator execution and operator-local
  validator simulation over the same packaged benchmark
- typed verification facts for timer integrity, token accounting, final-state
  capture, and declared execution strategy

Kernel and Nexus still own canonical eval-run authority truth. This issue lands
the reusable Psionic-side runtime and benchmark-contract layer only.

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-eval-runtime.sh
```

## Workload Shape

The current reference path proves one bounded but real eval workload:

1. define an environment-backed eval run contract
2. score one or more samples from environment session summaries
3. finalize the local eval run into a durable summary
4. define a packaged benchmark contract with verification policy
5. record repeated benchmark rounds and aggregate them robustly
6. run the same benchmark package through operator-local validator simulation

## Pass Criteria

The eval layer is green only if all of the following are true:

- local eval runs are typed and stateful rather than ad hoc script output
- rubric scoring reuses the environment contract instead of a second rubric DSL
- eval summaries are durable, machine-legible, and aggregate metrics cleanly
- benchmark packages can be used by both validator execution and local
  operator simulation without forking the contract
- verification facts are typed and enforceable by benchmark policy

## Expected Signals

The current harness should prove:

- `EvalSampleRecord::from_environment_summary(...)` derives stable scoring from
  environment rubric outcomes
- `EvalRunState` finalizes into deterministic score and pass-rate summaries
- online and offline eval modes preserve the same scoring semantics
- benchmark packages support repeat-run aggregation over one shared contract
- validator policy can require timer, token, final-state, or
  execution-strategy facts and refuse missing evidence

## Current Limitations

This issue intentionally does not claim:

- kernel-side canonical eval authority or registry ownership
- remote validator orchestration or adjudication services
- richer benchmark import adapters beyond the existing kernel adapter layer
- broader synthetic-data generation or research-loop ownership
- final training-orchestrator integration over every eval receipt

Those remain later issues. This issue makes the reusable local eval and
benchmark runtime contract real first.
