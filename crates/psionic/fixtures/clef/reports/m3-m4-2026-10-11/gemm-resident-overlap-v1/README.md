# Resident gate/up overlap: rejected for speed

One guarded build and one fixed benchmark completed. Every output matched the
serial baseline byte for byte, but the candidate failed the predeclared speed
requirement. No product code changed and no full model ran.

The only scheduling change omitted the barrier between the independent resident
half-weight gate and up projections. The kernels and arithmetic were unchanged.
The barrier before SiLU remained, including protection for the reused activation
buffer. This experiment does not apply to the on-demand Q4 path, which reuses one
weight conversion buffer.

| Tokens | Full-FFN paired ratio | RMS plus gate/up paired ratio |
| --- | ---: | ---: |
| 512 | 1.006125 | 1.069129 |
| 1024 | 0.852447 | 1.046848 |
| 2048 | 1.004241 | 0.993405 |

Ratios are overlap divided by serial GPU command time; lower is faster. Each is
the median of five fixed ABBA round ratios. The required full-FFN ratio was 0.90
or less at **every** size. The 1024-token result did not establish a consistent
win. Round variation is substantial, and this was a shared machine: one-minute
host load was 64.19 before and 61.77 after the run. Aggregate command timestamps
do not prove actual dispatch overlap. No causal or model-latency claim follows.

## Checks and bounds

All 8 sizes, 16 initial full-FFN commands, and 120 timed commands completed:
136 command buffers and 560 dispatches. Initial baseline outputs were finite;
every subsequent gate, up, half activation, and f32 residual output matched its
immutable expectation exactly. Pair-scope runs also preserved the residual input
and unused activation suffix. All redzones and source/expectation hashes passed.
The source checker and 21 model-free summary checks passed.

Build: 5.70 seconds under 4 GiB / start 16 GiB / floor 12 GiB / 180 seconds.
Benchmark: 7.09 seconds under 2 GiB / start 14 GiB / floor 12 GiB / 60 seconds,
with a 50-second internal deadline. Explicit Metal buffers peaked at 587,223,040
bytes; point-in-time device allocation plus owned CPU copies peaked at
940,392,448 bytes. The owned process group's sampled accounted peak was
1,260,815,896 bytes. The lowest recorded available host memory during the benchmark
was 96,512,458,752 bytes. All owned process groups stopped, all four leases were
released, and no monitor or cleanup error occurred. No quiet lease or desktop,
service, power, or priority control was used.

## Evidence and reproduction

- [Complete result and all five ratios](run-1/summary.json).
- [All raw correctness and timing rows](run-1/stdout.jsonl).
- [Run guard and cleanup receipt](run-1/receipt.json).
- [Build guard and cleanup receipt](build-1/receipt.json).
- [Frozen plan](PLAN.md), [source identities](frozen-sources.json),
  and [compiled artifact hashes](build-artifacts.json).
- [Fixed summary exit and verdict](verdict.json).

The original sources and receipts remain unchanged in task scratch. This copy
replaces only local workspace/scratch paths and the task session ID with
`$WORKTREE`, `$SCRATCH`, and `$TASK_SESSION`. `publication-index.json` records each copied
file's original and published hashes. Restore the local paths and original hashes
before trying to execute archived scripts; these source snapshots are evidence,
not a new portable runner. The run needs the same three independently decoded
half-weight assets named in [weights.json](weights.json). No binaries, weights,
raw tensors, credentials, or private sources are included.

The analyzer returned exit 2 because the fixed speed gate failed. The build and
benchmark guards both returned exit 0. There was no retry or increase in bounds.
