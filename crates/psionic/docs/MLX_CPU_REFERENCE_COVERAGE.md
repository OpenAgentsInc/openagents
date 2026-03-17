# Psionic MLX CPU Reference Coverage

> Status: canonical `PMLX-601` / `#3866` reference record, added 2026-03-16
> when `psionic-array` gained the first machine-readable CPU-reference
> coverage report over imported MLX `array_core`, `ops_numeric`, and
> `device_eval_memory` families.

This document records the bounded CPU-reference oracle that now sits underneath
the broader MLX backend-closure work.

It does not claim:

- Metal coverage
- CUDA coverage
- full upstream MLX parity-harness closure
- vendor-native allocator, scheduler, or profiler parity

It does claim:

- the public `psionic-array` surface now carries a machine-readable CPU
  coverage report anchored to imported MLX parity families
- the report distinguishes seeded supported behavior from explicit typed
  refusals
- the bounded public array surface now includes `flatten`, `expand_dims`,
  `squeeze`, and axis-aware `sum`

## Canonical Runner

Run the report from the repo root:

```bash
scripts/release/check-psionic-mlx-cpu-reference-coverage.sh
```

Write the machine-readable report:

```bash
scripts/release/check-psionic-mlx-cpu-reference-coverage.sh \
  --report /tmp/psionic-mlx-cpu-reference-coverage.json
```

Target one or more imported families:

```bash
scripts/release/check-psionic-mlx-cpu-reference-coverage.sh --only array_core
scripts/release/check-psionic-mlx-cpu-reference-coverage.sh --only ops_numeric --only device_eval_memory
```

The report schema lives at
`crates/psionic/docs/mlx_cpu_reference_coverage_report.schema.json`.

## Frozen Oracle Window

The bounded CPU-reference report is tied to the same frozen MLX oracle window
used by the MLX governance docs:

- `ml-explore/mlx`
- `v0.31.0` through `v0.31.1`

## Current Coverage

| Family | Seeded supported scope | Explicit refusal scope | Boundary note |
| --- | --- | --- | --- |
| `array_core` | `flatten`, `expand_dims`, `squeeze`, `slice`, `select`, `concat`, and `transpose` now have seeded CPU-reference cases on the public array surface. | out-of-range `expand_dims` and non-singleton `squeeze` requests refuse explicitly. | This is a bounded public view-semantic oracle, not a claim of full MLX storage, stride, or dtype breadth. |
| `ops_numeric` | dense `add`, `mul`, rank-2 `matmul`, axis-aware `sum`, seeded random-uniform creation plus cast, and bounded creation helpers now have seeded CPU-reference cases. | incompatible dense `matmul` shapes refuse through typed layout errors. | This is a bounded dense public-array oracle, not a claim of full MLX numeric breadth, arg-reduce closure, or broad dtype parity. |
| `device_eval_memory` | explicit `eval`, deferred `async_eval(...).wait()`, bounded cache counters, and backend-debug capture receipts now have seeded CPU-reference cases. | unsupported backend-debug capture formats refuse explicitly on the CPU lane. | This is a bounded explicit-materialization and observability oracle, not a claim of MLX-class allocator, scheduler, GPU, or vendor-profiler parity. |

## Why This Exists

`PMLX-601` closes the CPU-reference prerequisite for later backend-closure
work:

- Metal and CUDA coverage should now widen above a named CPU oracle instead of
  hand-waving from one backend demo
- the later parity-harness port in `PMLX-605` can now consume an existing
  CPU-reference contract instead of inventing one at the same time

The sequencing rule remains:

- close CPU reference truth first
- then widen Metal and CUDA truth
- then port broader upstream MLX test families into the parity harness
