# Fixed 128-thread pipeline limit: speed gate failed

One guarded build and one fixed benchmark completed. All outputs matched exactly,
but the candidate failed the requirement for at least a 10% full-FFN improvement
at every size. Do not integrate this candidate on this evidence. No product code
changed and no full model ran.

The only candidate change set the GEMM pipeline descriptor's maximum threadgroup
size to 128. Both variants dispatched the same 128 threads, used the same shader
library and buffer layout, and kept every barrier. The width-multiple hint stayed
false. The returned GEMM maximum changed from 1,024 to 128; both versions reported
zero static threadgroup bytes and a 32-thread execution width. These values do
not measure register allocation, spills, or NAX utilization.

| Token rows | Full-FFN paired ratio | RMS plus gate/up paired ratio |
| --- | ---: | ---: |
| 512 | 0.767231 | 0.977166 |
| 1,024 | 0.958682 | 0.980703 |
| 2,048 | 1.020236 | 1.063118 |

Each ratio is the median of five fixed ABBA round ratios, candidate divided by
baseline GPU command time. The full-FFN gate was at most 0.90 at all three sizes.
The 512-token improvement was not consistent across the larger sizes. All 120
ordered timing rows, including outliers, remain in [the raw log](run-1/stdout.jsonl).
The complete round ratios are in [the summary](run-1/summary.json). These short
shared-machine microbenchmarks do not establish a full-model gain or a hardware
cause for variation. Host load was about 16 before and after this run.

## Checks and limits

All eight sizes passed finite baseline output, complete bytewise candidate
comparison, source and immutable expectation hashes, redzones, and reset checks.
Every timed call also checked all written outputs. Pair-only calls preserved the
original residual and untouched activation suffix. The run completed exactly
136 command buffers, 560 dispatches, and 120 timing rows. The model-free source
check and 27 summary validation checks passed.

Build took 4.154 seconds under 4 GiB / start 16 GiB / floor 12 GiB / 180 seconds.
The benchmark took 7.675 seconds under 2 GiB / start 14 GiB / floor 12 GiB /
60 seconds externally and 50 seconds internally. Explicit Metal buffers peaked
at 587,223,040 bytes; device allocation plus owned CPU copies peaked at
940,392,448 bytes. The sampled owned-group accounted peak was 1,256,900,072 bytes;
minimum sampled available host memory was 85,407,055,872 bytes. Both owned groups
stopped, all four leases released, and the final lease inventory was empty.
There were no monitor, memory, sampling, or cleanup failures. No quiet lease,
desktop app, service, power, priority, or system setting was changed.

## Evidence and reproduction

- [Plan and fixed acceptance](PLAN.md).
- [Build receipt](build-1/receipt.json) and [run receipt](run-1/receipt.json).
- [Pipeline metadata and fixed verdict](verdict.json).
- [Executed source identities](frozen-sources.json) and [artifact hashes](build-artifacts.json).
- [Host difference from the reviewed prior harness](probe-origin.patch).
- [Source checks](source-check.json) and [summary checks](summary-check.json).

The original sources and receipts remain unchanged in task scratch. The
publication manifest maps their hashes to these copies. Only private local roots
and task session IDs are replaced with `$WORKTREE`, `$SCRATCH`, and
`$TASK_SESSION`. Published scripts are evidence snapshots; restore paths and
original identities before reproduction. No binaries, Metal libraries, model
weight payloads, or raw tensors are included. The build and bench guards exited
zero; the analyzer exited 2 because the frozen speed gate failed. There was no
retry, new size, second flag, or limit change.
