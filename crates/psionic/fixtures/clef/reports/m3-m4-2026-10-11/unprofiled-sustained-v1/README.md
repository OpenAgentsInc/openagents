# Sustained FFN slowdown without a profiler

The slowdown reproduces without `xctrace`, a system trace, or Metal counter
sampling. Four commands in flight reduce typical gaps between GPU commands to
about 15 microseconds, but do not remove the rise in command duration. Simple
host submission starvation is therefore insufficient to explain this run.
These results do not identify a thermal, frequency, or power-policy cause.

The probe executes the unchanged production weight-shape FFN kernels with
actual Flash layer-0 half weights and 2,048 deterministic input rows. Each
command contains gate, up, SiLU, and nonaccumulating down projections. The
fixed sequence has 128 commands each with queue depths 1, 4, 4, and 1. It uses
one tracked-resource queue, shared intermediate buffers, and at most four
separate final-output slots. This is an isolated FFN diagnostic, not a model
request or arbitrary-request concurrency test.

| Phase | Commands in flight | Median GPU command interval | Median gap between GPU commands |
| --- | ---: | ---: | ---: |
| A0 | 1 | 14.821 ms | 0.219 ms |
| B1 | 4 | 27.381 ms | 0.01496 ms |
| B2 | 4 | 26.471 ms | 0.01488 ms |
| A3 | 1 | 26.006 ms | 0.264 ms |

A0's consecutive 16-command medians were 11.002, 10.974, 12.044, 14.231,
16.201, 18.401, 19.804, and 24.361 ms. Both queued phases also began faster
and slowed: their first 16-command medians were 16.046 and 13.899 ms, while
their last windows were 26.232 and 26.002 ms. All 512 timing records remain
in [the output](check-1/stdout.jsonl), with every 16-command window in
[the summary](check-1/summary.json).

The CPU performs complete checks between phases. Those checks interrupt the
stream and may change the next phase's conditions. The first phase also starts
first, so its lower aggregate median is not a balanced estimate of a queue-depth
benefit. Queue depth did reduce the gaps, while GPU intervals still slowed.
No command envelopes overlapped in this run; the summarizer nevertheless keeps
negative gaps if a later reproduction observes them.

## Correctness and resources

All fixed counts completed: 512 timed commands, 514 total commands, and 2,053
dispatches. The small independent double-precision oracle passed its unchanged
scaled 0.00003 bound over 21 values. Its largest absolute error was 0.000036756
(the bound scales with the expected value).

After each phase drained, every active final output matched the immutable
reference bit for bit: 83,886,080 float comparisons across four phases. Complete
hashes covered all gate, up, and activation buffers after each phase, with full
finite-value checks. Input and weight hashes, immutable reference hashes, and
all buffer guards passed. These checks cover the drained outputs, not every
intermediate result overwritten by all 512 repeated commands. No large host
readback or output check ran inside a timed stream.

The check completed in 14.522 seconds under its 60-second outer limit. Explicit
buffers totaled 738,203,648 bytes (704.006 MiB); the largest sampled process
accounting was 867,042,456 bytes. Recorded Metal allocations were 739,115,008
bytes. Available host memory stayed above 109.5 GB. The guard is sampled rather
than a hard allocation limit. Cleanup stopped the owned group and all leases
were released. The single build passed in 3.668 seconds. There was no retry,
resource-bound increase, or source repair.

This was a shared-machine run. The recorded one-minute load average was
10.26–10.53; the observed thermal state stayed fair and low-power mode stayed
off. No quiet lease or system setting change was used. An unchanged thermal
state does not prove or disprove a hardware explanation.

## Reproduce

Copy this bundle into persistent scratch. Set `OPENAGENTS_WORKTREE` to the
repository containing the shared guard utilities, and `CLEF_FFN_ASSETS` to the
directory with verified `gate.f16`, `up.f16`, and `down.f16` assets described in
[weights.json](weights.json). The source files and executable hashes identify
the measured implementation; no weights or binaries are included here.

```sh
openagents lease build --keep-target-dir -- openagents lease memory --amount 4 -- python3 guard.py build --output build-reproduction
openagents lease gpu -- openagents lease memory --amount 2 -- python3 guard.py check --output check-reproduction
python3 summarize.py check-reproduction
```

The check requires 14 GiB available before launch, retains a 12 GiB available
floor, and caps its owned process at 2 GiB for 60 seconds. The probe stops new
submissions at its 50-second internal limit. The build uses 4 GiB and 180 seconds.
Quiet leases are refused. Preserve a refusal or incomplete run without raising
these bounds. There is no automatic counter or profiler fallback.

[The publication manifest](publication-manifest.json) maps original and published
file hashes. Receipts and logs preserve complete small outputs with personal
paths replaced by placeholders. The reproduction guard accepts portable path
variables; numerical Swift and Metal sources are unchanged. [The plan](PLAN.md)
and [prior trace analysis](prior-trace-reanalysis.json) preserve the earlier
source-only reasoning separately from this result.
