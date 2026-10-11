# Three-part parallel K reduction result

Reject product integration. The single fixed three-way split passed its numerical checks but did not meet the predeclared 10% speed threshold at any tested size.

| Tokens | Baseline full FFN, median GPU ms | Split3 full FFN, median GPU ms | Median paired split3/baseline |
| --- | ---: | ---: | ---: |
| 512 | 4.51712 | 5.66785 | 1.22316 |
| 1024 | 7.94942 | 7.32202 | 1.00072 |
| 2048 | 13.14142 | 14.33117 | 1.07453 |

The paired projection-sum ratios were 1.05300, 0.98265, and 1.10908. The predeclared gate requires both full-FFN and projection-sum paired medians to improve by at least 10% at every size. The ratio of marginal medians differs from the median of paired ratios; acceptance uses the paired ratios shown in the last column.

All 14 synthetic and five real-weight checks passed the fixed absolute `0.001` bound. Every synthetic output and partial had an independent f64 oracle; each weighted case checked 96 fixed whole-dot and per-partial f64 samples. Every weighted final output was also compared with production, with maximum absolute difference `8.106231689453125e-6`. Gate, up, and SiLU remained bitwise equal. All 240 timed output gates passed, and buffer guards plus immutable input, weight, and expected-output hashes stayed intact. The executed totals matched the frozen plan: 281 command buffers and 579 dispatches.

These are actual Flash layer-0 weights with deterministic synthetic activations. A projection check does not establish full-model weighted-logit or end-to-end latency acceptance. Five fixed ABBA rounds balance early/late positions, but unchanged gate/up controls also varied substantially during this shared-machine run. The evidence shows no useful gain here; it does not identify a thermal, frequency, memory-bandwidth, or scheduling cause. No further partition-count sweep or product change is planned.

The guarded build passed in 4.900 seconds, and the bench finished in 9.606 seconds. Peak owned-process accounting was 1,116,898,696 bytes. Explicit live buffers peaked at 989,863,424 bytes; the largest recorded `device.currentAllocatedSize` was 990,822,400 bytes. The original 2 GiB process cap, 14 GiB starting-memory minimum, 12 GiB available-memory floor, and 60-second deadline remained unchanged. No sampler, memory-limit, or cleanup error occurred. All leases were released; no quiet lease or desktop/system-service control was used.

[The complete ordered output](run-1/stdout.jsonl) preserves every check and timing sample. [The summary](run-1/summary.json) preserves every paired ratio. [The prepared plan](PREPARED_PLAN.md) and historical source-preparation record describe the source before execution; their initial status fields are not the final result. The fixed three partitions each have K extent 4096 with original parent strides. A separate f32 reduction adds the residual once, compiled with `-fno-fast-math` separately from the unchanged matrix compilation options.

[The build record](build-artifacts.json) retains the executed source, executable, and metallib hashes. Binaries, weight payloads, and memory timelines are excluded. To reproduce after an execution grant, set `CLEF_REPO` to the checkout containing the recorded guard dependencies and supply the three hash-matching half-weight files in the documented sibling directory. Run `guard.py build --output build-new` under build and 4 GiB memory leases, then `guard.py bench --output run-new` under GPU and 2 GiB memory leases. Run `summarize.py run-new` afterward. Never use a quiet lease. The published guard replaces only its private checkout constant with that environment variable; rebuilding produces fresh source hashes for that published copy.

[The sanitization manifest](sanitization.json) maps original hashes to published bytes. Hash fields within original receipts remain unchanged and identify the executed source, not a later sanitized copy. Only this experiment's task-owned logs and lease receipts are included. No failed build or execution was discarded; both completed, and the failed performance decision is retained.
