# Fixed four-token dispatch swizzle result

Reject product integration. The candidate preserved every tested output bitwise but did not meet the predeclared 10% speed threshold at any tested size.

| Tokens | Baseline full FFN, median GPU ms | Swizzle full FFN, median GPU ms | Median paired swizzle/baseline |
| --- | ---: | ---: | ---: |
| 512 | 3.02071 | 3.03296 | 1.00094 |
| 1024 | 5.52008 | 5.49221 | 0.98670 |
| 2048 | 10.44348 | 10.51683 | 1.01343 |

The 15 initial checks and all 240 timed relevant-output comparisons passed exact bit equality. Every expected output was checked for finite values; all buffer guards and immutable input/weight/baseline hashes passed. The executed totals matched the frozen plan: 273 command buffers and 492 dispatches. The small independent f64 oracle retained its original scaled tolerance. Actual Flash layer-0 weights were used with deterministic synthetic activations; this is not full-model or end-to-end latency acceptance.

The single run finished in 9.286 seconds. Peak owned-process accounting was 1,016,677,672 bytes, explicit live buffers peaked at 889,198,592 bytes, and the largest recorded `device.currentAllocatedSize` was 890,109,952 bytes. No memory, sampler, or cleanup error occurred. GPU and memory leases were released. No quiet lease, desktop application, or system-service control was used.

Raw timing order and every paired ratio remain in [the run output](run-1/stdout.jsonl) and [summary](run-1/summary.json). Five fixed ABBA rounds balance early/late positions; shared-machine load, cache effects, and temporal drift remain limitations. Differences near 1% do not establish a useful optimization or a hardware cause. There is no planned variant sweep or product change.

[The prepared plan](PREPARED_PLAN.md) describes exact arithmetic, dispatch mapping, source origins, guards, scope, and unchanged 60-second deadline. [The build record](build-artifacts.json) retains original executable and metallib hashes; binaries, weights, and memory timelines are excluded. The source files suffice to rebuild under the same documented leases and guard, but execution still requires authorization. Set `CLEF_REPO` to an appropriate checkout for the guard imports, and provide the three hash-matching half-weight assets in the documented sibling directory. The published guard replaces only the private checkout constant with that environment variable.

[The sanitization manifest](sanitization.json) maps original file hashes to published bytes. Original source/executable hash fields in receipts remain unchanged; they describe the executed source, not a later sanitized copy. Only task-owned logs and lease receipts are included. No failed build or run occurred in this experiment.
