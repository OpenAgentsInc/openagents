# 2026-03-16 Psionic Architecture Explainer First Real Run Benchmark Addendum

## Scope

This addendum closes GitHub issue `#3659` for the first real
`Psionic architecture explainer` Apple adapter run.

It records:

- the baseline host used for the run
- phase-by-phase wall-clock timing
- storage footprint for the run artifacts
- the dominant bottlenecks
- a minimum repeatable machine profile for this current live lane

The machine-readable companion receipt is:

- `crates/psionic/fixtures/apple_adapter/runs/psionic_architecture_explainer_first_real_run_benchmark_addendum.json`

The source run report is:

- `crates/psionic/fixtures/apple_adapter/runs/psionic_architecture_explainer_first_real_run_report.json`

## Baseline Host

- machine: `MacBook Pro (Mac14,10)`
- chip: `Apple M2 Pro`
- CPU cores: `12`
- memory: `16 GB`
- OS: `macOS 26.1 (25B78)`
- kernel: `Darwin 25.1.0`
- Xcode: `26.3 (17C529)`
- local macOS SDK used for bridge/tooling builds: `26.2 (25C58)`

This was a single-host local run with the Foundation Models bridge on
`127.0.0.1:11435`.

## Dataset And Artifact Footprint

- train split: `9` samples, `4463` bytes
- held-out split: `6` samples, `2478` bytes
- benchmark split: `7` samples, `3610` bytes
- corpus manifest: `8702` bytes
- toolkit checkpoint: `266799379` bytes
- toolkit-exported runtime asset: `133312320` bytes
- staged runtime asset: `133312320` bytes
- exported runtime asset: `133312320` bytes
- full run directory footprint: `796572 KiB`
- exported package footprint: `130192 KiB`

The datasets are negligible. The storage footprint is dominated by the
checkpoint plus the multiple runtime-asset copies.

## Timing Breakdown

Run id:

- `psionic-architecture-explainer-first-real-run-1773636619824`

Phase timings:

- setup and dataset import: `9 ms`
- toolkit training: `822355 ms` from the toolkit receipt (`~13m42s`)
- toolkit export: `2216 ms` from the toolkit receipt (`~2.2s`)
- held-out plus runtime-smoke until failure: `8692 ms` (`~8.7s`)
- export copy to the final target package: `3 ms`
- benchmark plus report after export: `19087 ms` (`~19.1s`)
- total run to exported package: `834208 ms` (`~13m54s`)
- total run to final report: `853295 ms` (`~14m13s`)

## Bottlenecks

The dominant bottleneck is straightforward:

- toolkit training consumed `96.38%` of total wall clock
- benchmark plus report materialization consumed `2.24%`
- held-out plus runtime-smoke consumed `1.02%`
- toolkit export consumed `0.3%`

So the practical answer for this current lane is:

- training time dominates everything
- export cost is trivial on this host
- the live bridge validation and benchmark/report phases are noticeable but
  still small compared with training

## What Is Specific To This Current Reference Path

These numbers are not generic Apple-training numbers. They are specific to the
current OpenAgents live lane:

- training and export are toolkit-backed, not a pure Rust-native Apple trainer
- the run is single-host and does not use `psionic-cluster`
- the benchmark/report path uses live bridge round-trips and failure-tolerant
  operator simulation
- the run failed runtime smoke on tokenizer-lineage drift before a full local
  summary object was persisted

That means the timing profile is useful for repeating the current operator
lane, but it should not be overclaimed as a future clustered or pure-Rust
profile.

## Minimum Repeatable Profile

Based on this run, the minimum practical repeat profile for the current live
lane is:

- Apple Silicon Mac with Apple Intelligence enabled
- `16 GB` unified memory
- roughly `1 GB` of free scratch disk for checkpoint plus packaged artifacts
- a live Foundation Models bridge on `127.0.0.1:11435`
- the local Apple adapter toolkit for the current dev-path training/export step

For local development, this machine used Xcode `26.3` with macOS SDK `26.2`.
For a shipped bundled helper, Xcode should not be an end-user requirement, but
that is separate from the current developer-run benchmark path.

## Outcome Caveat

This run still surfaced the Apple Intelligence compatibility popup during live
attempts:

> This app needs an update in order to work with the latest Apple Intelligence models.

The popup did not prevent this benchmarked run from completing training,
export, benchmark evaluation, and durable report generation. It remains a live
bridge/runtime compatibility nuisance, not the dominant time or storage
bottleneck for this run.
