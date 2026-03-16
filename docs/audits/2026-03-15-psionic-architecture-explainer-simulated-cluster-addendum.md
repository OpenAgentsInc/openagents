# `Psionic Architecture Explainer` Simulated-Cluster Addendum

> Status: audit for GitHub issue `#3660`, written on 2026-03-15 after
> measuring the finished Apple operator runs recorded in
> `~/.openagents/logs/autopilot/apple-adapter-training.json`, timing the
> current simulated-cluster reference harnesses in `psionic-train`, and
> reviewing `crates/psionic/docs/TRAIN_SYSTEM.md`,
> `crates/psionic/docs/ARCHITECTURE_EXPLAINER_CLUSTER_BRINGUP_RUNBOOK.md`,
> `crates/psionic/psionic-train/src/adapter_cluster.rs`, and
> `crates/psionic/psionic-train/src/adapter_reference_program.rs`.

## Why This Addendum Exists

The current truthful Apple adapter lane is still single-host in execution
reality.

That means any "cluster speedup" claim on March 15, 2026 has to separate three
things:

1. measured single-host Apple operator timings
2. measured simulated-cluster control-plane timings
3. projected future speedups if live clustered Apple training is implemented

This addendum does exactly that.

## Scope

This is not a claim that live distributed Apple training already exists.

This addendum measures:

- the current real Apple operator timing envelope from completed local runs
- the current simulated-cluster control-plane cost using the existing
  deterministic adapter-cluster and decentralized-adapter reference harnesses

This addendum projects, but does not measure:

- future multi-device Apple training speedup
- future collective-backed gradient exchange
- future mixed Apple Metal plus NVIDIA execution gains

## Baseline: Measured Single-Host Apple Operator Timings

The real operator baseline comes from the three completed toolkit-backed Apple
training attempts recorded in
`~/.openagents/logs/autopilot/apple-adapter-training.json` on March 15, 2026.

Measured training durations from those runs:

- `2,110,292 ms`
- `1,901,643 ms`
- `2,008,866 ms`

Measured export durations from the runs that reached export:

- `11,899 ms`
- `2,828 ms`

Measured eval-phase durations from the runs that reached held-out eval or
runtime smoke before failing:

- `32,438 ms`
- `734 ms`
- `578 ms`
- `6,991 ms`

Median view of the currently measured single-host path:

| Phase | Median |
| --- | ---: |
| Apple toolkit training | `2,008,866 ms` |
| Apple toolkit export | `7,363.5 ms` |
| held-out eval + runtime-smoke phase | `3,862.5 ms` |

Derived median pre-accept total from those measured phases:

- `2,020,092 ms`
- about `33m 40.1s`

The important point is simple:

- training dominates the current wall clock
- export and eval are real, but they are small compared with training

On the current median, the training phase is about `99.45%` of the measured
pre-accept wall clock.

## Simulated-Cluster Measurement Method

I measured the current deterministic simulated-cluster harnesses directly on
this machine after warm compilation.

Commands used:

```bash
cargo test -p psionic-train adapter_reference_program::tests::decentralized_adapter_reference_program_runs_for_apple_family -- --exact --nocapture
cargo test -p psionic-train adapter_reference_program::tests::decentralized_adapter_reference_program_runs_for_open_family -- --exact --nocapture
cargo test -p psionic-train adapter_cluster::tests::adapter_cluster_harness_reselects_after_membership_churn -- --exact --nocapture
```

Each command was run `5` times and timed wall-clock from the shell.

## Simulated-Cluster Measured Results

| Harness | Runs (ms) | Median |
| --- | --- | ---: |
| Apple decentralized-adapter reference program | `1114.60`, `295.35`, `306.35`, `291.60`, `302.13` | `302.13 ms` |
| Open-backend decentralized-adapter reference program | `315.22`, `302.40`, `355.15`, `627.60`, `293.76` | `315.22 ms` |
| Adapter-cluster churn harness | `432.07`, `299.41`, `314.88`, `300.40`, `296.61` | `300.40 ms` |

Interpretation:

- the first cold iteration is visibly slower
- steady-state simulated-cluster control-plane time is roughly `0.30s`
- the Apple and open-backend simulated reference paths are very close in
  steady-state wall clock

Relative to the median real Apple training time:

- simulated cluster-control cost at `302.13 ms` is about `0.015%` of the
  current median Apple training phase

That means the current measured simulated-cluster overhead is negligible
compared with the actual Apple training wall clock.

## Bottleneck Map

The first real operator workflow is not uniformly parallelizable.

| Phase | Current Reality | Parallelizable Later? | Notes |
| --- | --- | --- | --- |
| dataset read + runtime lineage + packing derivation | serial and already cheap | not important | measured in logs as effectively negligible next to training |
| Apple toolkit training | serial on one Apple host today | yes, this is the real target for future cluster speedup | currently the dominant wall-clock phase |
| Apple toolkit export to `.fmadapter` | serial and Apple-runtime-bound | mostly no | still has to produce one final Apple-valid asset |
| package restaging and metadata rewrite | serial | low value to parallelize | not the bottleneck |
| held-out eval | partly parallelizable later | some, but not all | sample-level fan-out is possible later, but current operator flow is still narrow |
| bridge-backed runtime smoke | serial and Apple-bound | mostly no | final package still has to load through one Apple runtime path |
| acceptance or authority projection | serial and cheap | not important | not a speedup target |

Bottom line:

- future cluster work can only win materially if it shortens the training
  phase itself
- it will not make export or final Apple runtime validation disappear

## Projected Speedups For A Future Live Apple Cluster

These are projections only. They are not current measured cluster-training
results.

They assume:

- the same dataset and environment definition
- the same export and eval serial phases as today
- only the training phase benefits from clustering

Using the current measured median:

- training: `2,008,866 ms`
- serial remainder: about `11,226 ms` from median export plus median eval

### Idealized Two-Worker Training Split

If a future live cluster cut the training phase almost exactly in half:

- projected training: about `1,004,433 ms`
- projected total: about `1,015,659 ms`
- projected end-to-end speedup: about `1.99x`

### Idealized Three-Worker Training Split

If a future live cluster cut the training phase to one third:

- projected training: about `669,622 ms`
- projected total: about `680,848 ms`
- projected end-to-end speedup: about `2.97x`

### More Honest Practical Reading

The idealized numbers above are not what the repo should promise today.

Real live clustered Apple training would have additional costs that are not
measured yet:

- collective synchronization
- remote checkpoint or artifact movement
- worker straggler behavior
- retry or replay under disagreement
- coordinator bookkeeping under churn

So the correct current interpretation is:

- the real upside is large because training dominates the current wall clock
- the actual achieved speedup remains unknown until live multi-device Apple
  execution exists

## Heterogeneous Apple Metal Plus NVIDIA Variant

The measured simulated heterogeneous proxy is the current open-backend
decentralized-adapter reference harness.

Measured result:

- open-backend reference-program median: `315.22 ms`
- Apple reference-program median: `302.13 ms`

The tiny difference between those medians is useful, but only for one narrow
conclusion:

- the current cluster-control and receipt machinery is not obviously
  sensitive to whether the simulated worker family is Apple-first or open
  backend

It is not evidence that a real Apple plus NVIDIA mixed cluster can already
accelerate final Apple adapter training.

The honest mixed-role story today remains:

- Apple host keeps coordinator, final export, and final bridge validation
  authority
- NVIDIA host is useful for mixed-hardware cluster, staging, receipt, and open
  backend bring-up
- the repo does not yet have evidence that NVIDIA participants can shorten the
  Apple-valid final training path itself

That is exactly why the mixed-backend execution question remains a separate
issue in `#3662`.

## What This Addendum Says Clearly

Measured today:

- real single-host Apple training takes about `31.7` to `35.2` minutes across
  the completed March 15 attempts
- current simulated-cluster control and receipt harnesses run in about
  `0.30s` steady state
- training is overwhelmingly the dominant current bottleneck

Not measured today:

- real multi-device Apple training throughput
- real collective-backed Apple gradient exchange
- real mixed Apple plus NVIDIA acceleration of one Apple-valid export path

## Recommended Next Steps

1. Finish the live single-host reference run and use that report as the new
   baseline for `#3658` and `#3659`.
2. Land the export-format and export-validation issues `#3663` and `#3664` so
   future timing claims are attached to a fully truthful final package path.
3. Keep the first live clustered attempt scoped to the topology in
   `crates/psionic/docs/ARCHITECTURE_EXPLAINER_CLUSTER_BRINGUP_RUNBOOK.md`.
4. Treat `#3662` as the place where mixed Apple plus NVIDIA execution becomes a
   real experiment instead of a control-plane rehearsal.
