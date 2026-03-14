# Train Benchmark Acceptance Reference

> Status: canonical `PSI-288` / `#3593` reference record, updated 2026-03-14
> after landing the quantitative train acceptance suite in
> `crates/psionic/psionic-train/src/benchmarking.rs`.

This document records the first typed performance acceptance layer for Psionic
train workloads.

## Canonical Runner

Run the acceptance harness from the repo root:

```bash
scripts/release/check-psionic-train-benchmark-acceptance.sh
```

## What Landed

`psionic-train` now owns a benchmark harness with explicit threshold profiles
and machine-legible receipts for the train-side performance claims that were
previously still prose-only.

The new typed surfaces include:

- `TrainBenchmarkThresholdProfile`
- `TrainerThroughputBenchmarkReceipt`
- `RolloutIngestionBenchmarkReceipt`
- `SandboxReuseBenchmarkReceipt`
- `CheckpointRecoveryBenchmarkReceipt`
- `ValidatorCostBenchmarkReceipt`
- `ElasticScalingBenchmarkReceipt`
- `TrainBenchmarkSuiteReceipt`
- `TrainBenchmarkHarness`

## Canonical Threshold Profile

The current reference profile is `psionic-train-reference-acceptance`.

Its thresholds are:

| Category | Thresholds |
| --- | --- |
| Trainer throughput | at least `18` steps/sec, at least `140` samples/sec, mean step duration at most `60 ms`, tail step duration at most `80 ms` |
| Rollout ingestion | at least `20` rollouts/sec, mean submission spacing at most `40 ms`, accepted ratio at least `9000 bps` |
| Sandbox reuse | mean warm latency at most `1500 ms`, mean acquisition latency at most `500 ms`, workspace reuse at least `7000 bps`, ready sessions at least `2` |
| Checkpoint + datastream | pointer restore at most `50 ms`, fallback restore at most `50 ms`, fallback ladder at most `2` attempts, resumed delivery at least `4096 B/s` |
| Validator cost | at least `200` bundles/sec, mean verify latency at most `25 ms`, benchmark-checked share at most `5000 bps` |
| Elastic scaling | two-to-four-member throughput scaling at least `15000 bps`, quantized speedup at least `2500 bps`, degraded global interval at most `4` steps |

## What The Suite Covers

The acceptance harness now answers six concrete production questions:

1. Can the current fixed-budget trainer step fast enough to claim a real local
   reference loop?
2. Can rollout ingestion keep up with the orchestrator window cadence rather
   than collapsing at the admission seam?
3. Does the sandbox pool actually reuse warm workspaces rather than forcing
   cold starts every iteration?
4. Can checkpoint restore stay on the pointer-first ladder and still recover
   through resumable datastream fallback when necessary?
5. Is validator work still cheap enough when duplicate detection, stale-policy
   rejection, and sampled benchmark checks are all active?
6. Does the collective planner still show positive scaling as membership widens,
   while degrading safely under transport pressure?

## Pass Criteria

The suite is green only if all of the following remain true:

- the trainer meets the configured throughput and duration thresholds
- rollout ingestion stays above the configured intake floor with a high
  acceptance ratio
- sandbox warm and acquisition latencies stay under the configured ceilings
  while reuse remains dominant
- pointer restore stays fast, fallback stays bounded, and resumed checkpoint
  delivery remains above the configured throughput floor
- validator verification shows mixed accepted, normalized, and rejected
  outcomes without letting benchmark-check share or latency blow up
- elastic scaling remains positive from two to four members and degraded
  transport widens cadence through the explicit safe fallback path

## Current Limits

This issue does not claim that Psionic now has a full external benchmarking
program. It does not yet include:

- hardware-specific trainer throughput baselines across real GPU fleets
- long-running soak benchmarks over external artifact stores
- market-facing seller-lane perf targets
- cross-host validator or sandbox service latency SLOs

What it does do is give Psionic one owned Rust acceptance profile for trainer,
orchestrator, sandbox, datastream, validator, and collective scaling claims,
with receipts that higher-level docs and release gates can point at directly.
